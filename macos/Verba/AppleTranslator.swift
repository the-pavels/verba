import Foundation
import NaturalLanguage
@preconcurrency import Translation

enum TranslationPairStatus: Equatable, Sendable {
    case installed
    case supported
    case unsupported
}

enum TranslationPreparation: Equatable, Sendable {
    case none
    case required
}

struct AppleTranslationResult: Equatable, Sendable {
    let sourceLanguageIdentifier: String
    let targetLanguageIdentifier: String
    let translatedText: String
}

enum AppleTranslationError: Error, Equatable, Sendable {
    case languageAssetsRequired(targetLanguageIdentifier: String)
    case unsupportedPair(targetLanguageIdentifier: String)
    case unableToIdentifyLanguage
    case cancelled
    case failed
}

@MainActor
protocol TranslationLanguageIdentifying {
    func identify(_ text: String) -> [Locale.Language]
}

@MainActor
protocol TranslationAvailabilityChecking {
    func status(
        from source: Locale.Language,
        target: Locale.Language
    ) async throws -> TranslationPairStatus
}

@MainActor
protocol TranslationSessionProviding {
    func translate(
        _ text: String,
        source: Locale.Language?,
        target: Locale.Language,
        preparation: TranslationPreparation
    ) async throws -> AppleTranslationResult
}

@MainActor
struct AppleTranslator {
    private let languageIdentifier: any TranslationLanguageIdentifying
    private let availability: any TranslationAvailabilityChecking
    private let sessions: any TranslationSessionProviding

    init(
        languageIdentifier: any TranslationLanguageIdentifying = SystemTranslationLanguageIdentifier(),
        availability: any TranslationAvailabilityChecking = SystemTranslationAvailability(),
        sessions: any TranslationSessionProviding
    ) {
        self.languageIdentifier = languageIdentifier
        self.availability = availability
        self.sessions = sessions
    }

    func translate(
        _ text: String,
        languageDetectionContext: String? = nil,
        targetLanguageIdentifier: String
    ) async throws -> AppleTranslationResult {
        let target = Locale.Language(identifier: targetLanguageIdentifier)

        do {
            let context = languageDetectionContext.flatMap {
                $0.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? nil : $0
            }
            var sources: [Locale.Language] = []
            for detectionText in [context, text].compactMap({ $0 }) {
                for source in languageIdentifier.identify(detectionText)
                where !sources.contains(where: {
                    $0.minimalIdentifier == source.minimalIdentifier
                }) {
                    sources.append(source)
                }
            }
            guard !sources.isEmpty else {
                throw AppleTranslationError.unableToIdentifyLanguage
            }

            for source in sources {
                switch try await availability.status(from: source, target: target) {
                case .installed:
                    do {
                        return try await sessions.translate(
                            text,
                            source: source,
                            target: target,
                            preparation: .none
                        )
                    } catch where translationRequiresPreparation(error) {
                        return try await sessions.translate(
                            text,
                            source: source,
                            target: target,
                            preparation: .required
                        )
                    }
                case .supported:
                    return try await sessions.translate(
                        text,
                        source: source,
                        target: target,
                        preparation: .required
                    )
                case .unsupported:
                    continue
                }
            }

            throw AppleTranslationError.unsupportedPair(
                targetLanguageIdentifier: target.minimalIdentifier
            )
        } catch {
            throw mapTranslationError(error, target: target)
        }
    }
}

@MainActor
struct SystemTranslationLanguageIdentifier: TranslationLanguageIdentifying {
    private let preferredLanguageIdentifiers: [String]

    init(preferredLanguageIdentifiers: [String] = Locale.preferredLanguages) {
        self.preferredLanguageIdentifiers = preferredLanguageIdentifiers
    }

    func identify(_ text: String) -> [Locale.Language] {
        var candidates: [Locale.Language] = []

        if let language = dominantLanguage(for: text) {
            candidates.append(language)
        }
        if containsSingleWord(text),
           let preferredLanguage = dominantLanguage(
               for: text,
               hints: preferredLanguageHints()
           ),
           !candidates.contains(where: {
               $0.minimalIdentifier == preferredLanguage.minimalIdentifier
           })
        {
            candidates.append(preferredLanguage)
        }

        return candidates
    }

    private func dominantLanguage(
        for text: String,
        hints: [NLLanguage: Double] = [:]
    ) -> Locale.Language? {
        let recognizer = NLLanguageRecognizer()
        recognizer.languageHints = hints
        recognizer.processString(text)

        guard let language = recognizer.dominantLanguage,
              language != .undetermined
        else {
            return nil
        }
        return Locale.Language(identifier: language.rawValue)
    }

    private func preferredLanguageHints() -> [NLLanguage: Double] {
        var hints: [NLLanguage: Double] = [:]
        for identifier in preferredLanguageIdentifiers {
            guard let languageCode = Locale.Language(identifier: identifier)
                .languageCode?.identifier
            else {
                continue
            }
            hints[NLLanguage(rawValue: languageCode)] = 1
        }
        return hints
    }

    private func containsSingleWord(_ text: String) -> Bool {
        let tokenizer = NLTokenizer(unit: .word)
        tokenizer.string = text
        var wordCount = 0
        tokenizer.enumerateTokens(in: text.startIndex..<text.endIndex) { _, _ in
            wordCount += 1
            return wordCount < 2
        }
        return wordCount == 1
    }
}

@MainActor
private struct SystemTranslationAvailability: TranslationAvailabilityChecking {
    func status(
        from source: Locale.Language,
        target: Locale.Language
    ) async throws -> TranslationPairStatus {
        switch await LanguageAvailability().status(from: source, to: target) {
        case .installed:
            .installed
        case .supported:
            .supported
        case .unsupported:
            .unsupported
        @unknown default:
            throw AppleTranslationError.failed
        }
    }
}

private func translationRequiresPreparation(_ error: any Error) -> Bool {
    if case .languageAssetsRequired = error as? AppleTranslationError {
        return true
    }
#if compiler(>=6.2)
    if #available(macOS 26.0, *), TranslationError.notInstalled ~= error {
        return true
    }
#endif
    return false
}

private func mapTranslationError(
    _ error: any Error,
    target: Locale.Language
) -> AppleTranslationError {
    if let error = error as? AppleTranslationError {
        return error
    }
    if error is CancellationError {
        return .cancelled
    }
#if compiler(>=6.2)
    if #available(macOS 26.0, *), TranslationError.alreadyCancelled ~= error {
        return .cancelled
    }
    if #available(macOS 26.0, *), TranslationError.notInstalled ~= error {
        return .languageAssetsRequired(
            targetLanguageIdentifier: target.minimalIdentifier
        )
    }
#endif
    if TranslationError.unsupportedSourceLanguage ~= error
        || TranslationError.unsupportedTargetLanguage ~= error
        || TranslationError.unsupportedLanguagePairing ~= error
    {
        return .unsupportedPair(targetLanguageIdentifier: target.minimalIdentifier)
    }
    if TranslationError.unableToIdentifyLanguage ~= error {
        return .unableToIdentifyLanguage
    }
    return .failed
}
