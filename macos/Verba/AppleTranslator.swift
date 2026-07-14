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
    func identify(_ text: String) -> Locale.Language?
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
        targetLanguageIdentifier: String
    ) async throws -> AppleTranslationResult {
        let target = Locale.Language(identifier: targetLanguageIdentifier)

        do {
            guard let source = languageIdentifier.identify(text) else {
                throw AppleTranslationError.unableToIdentifyLanguage
            }

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
                throw AppleTranslationError.unsupportedPair(
                    targetLanguageIdentifier: target.minimalIdentifier
                )
            }
        } catch {
            throw mapTranslationError(error, target: target)
        }
    }
}

@MainActor
private struct SystemTranslationLanguageIdentifier: TranslationLanguageIdentifying {
    func identify(_ text: String) -> Locale.Language? {
        guard let language = NLLanguageRecognizer.dominantLanguage(for: text),
              language != .undetermined
        else {
            return nil
        }
        return Locale.Language(identifier: language.rawValue)
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
    if #available(macOS 26.0, *), TranslationError.notInstalled ~= error {
        return true
    }
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
    if #available(macOS 26.0, *), TranslationError.alreadyCancelled ~= error {
        return .cancelled
    }
    if #available(macOS 26.0, *), TranslationError.notInstalled ~= error {
        return .languageAssetsRequired(
            targetLanguageIdentifier: target.minimalIdentifier
        )
    }
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
