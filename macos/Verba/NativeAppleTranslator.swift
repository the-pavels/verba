import Foundation

@MainActor
final class NativeAppleTranslator: NativeTranslator {
    private let translator: AppleTranslator

    init(translator: AppleTranslator) {
        self.translator = translator
    }

    func translate(
        request: NativeTranslationRequest
    ) async throws -> NativeTranslationResponse {
        do {
            let result = try await translator.translate(
                request.text,
                targetLanguageIdentifier: request.targetLanguageIdentifier
            )
            return NativeTranslationResponse(
                sourceLanguageIdentifier: result.sourceLanguageIdentifier,
                translatedText: result.translatedText
            )
        } catch let error as AppleTranslationError {
            throw error.nativeTranslationError
        } catch is CancellationError {
            throw NativeTranslationError.Cancelled
        } catch {
            throw NativeTranslationError.Failed
        }
    }
}

private extension AppleTranslationError {
    var nativeTranslationError: NativeTranslationError {
        switch self {
        case .unsupportedPair:
            .UnsupportedPair
        case .unableToIdentifyLanguage:
            .UnableToIdentifyLanguage
        case .cancelled:
            .Cancelled
        case .languageAssetsRequired, .failed:
            .Failed
        }
    }
}
