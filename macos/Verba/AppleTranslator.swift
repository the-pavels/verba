import Foundation
@preconcurrency import Translation

enum TranslationPairStatus: Equatable, Sendable {
    case installed
    case supported
    case unsupported
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
protocol TranslationAvailabilityChecking {
    func status(
        for text: String,
        target: Locale.Language
    ) async throws -> TranslationPairStatus
}

@MainActor
protocol TranslationSessionProviding {
    func translate(
        _ text: String,
        source: Locale.Language?,
        target: Locale.Language
    ) async throws -> AppleTranslationResult
}

@MainActor
struct AppleTranslator {
    private let availability: any TranslationAvailabilityChecking
    private let sessions: any TranslationSessionProviding

    init(
        availability: any TranslationAvailabilityChecking = SystemTranslationAvailability(),
        sessions: any TranslationSessionProviding
    ) {
        self.availability = availability
        self.sessions = sessions
    }

    func translate(
        _ text: String,
        targetLanguageIdentifier: String
    ) async throws -> AppleTranslationResult {
        let target = Locale.Language(identifier: targetLanguageIdentifier)

        do {
            switch try await availability.status(for: text, target: target) {
            case .installed:
                return try await sessions.translate(text, source: nil, target: target)
            case .supported:
                throw AppleTranslationError.languageAssetsRequired(
                    targetLanguageIdentifier: target.minimalIdentifier
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
private struct SystemTranslationAvailability: TranslationAvailabilityChecking {
    func status(
        for text: String,
        target: Locale.Language
    ) async throws -> TranslationPairStatus {
        switch try await LanguageAvailability().status(for: text, to: target) {
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
