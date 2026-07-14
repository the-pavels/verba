import AppKit
import SwiftUI
@preconcurrency import Translation

@MainActor
protocol TranslationSessionExecuting {
    func prepareTranslation() async throws
    func translate(_ text: String) async throws -> TranslationSession.Response
}

extension TranslationSession: TranslationSessionExecuting {}

@MainActor
final class SystemTranslationSessionProvider: ObservableObject, TranslationSessionProviding {
    @Published private(set) var configuration: TranslationSession.Configuration?

    private struct PendingRequest {
        let id: UUID
        let text: String
        let preparation: TranslationPreparation
        let continuation: CheckedContinuation<AppleTranslationResult, any Error>
    }

    private struct RequestSnapshot: Sendable {
        let id: UUID
        let text: String
        let preparation: TranslationPreparation
    }

    private let downloadPromptActivator: () -> Void
    private var pendingRequest: PendingRequest?

    init(downloadPromptActivator: @escaping () -> Void = {
        NSApplication.shared.activate()
    }) {
        self.downloadPromptActivator = downloadPromptActivator
    }

    func translate(
        _ text: String,
        source: Locale.Language?,
        target: Locale.Language,
        preparation: TranslationPreparation
    ) async throws -> AppleTranslationResult {
        guard pendingRequest == nil else {
            throw AppleTranslationError.failed
        }

        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                guard !Task.isCancelled else {
                    continuation.resume(throwing: CancellationError())
                    return
                }

                pendingRequest = PendingRequest(
                    id: UUID(),
                    text: text,
                    preparation: preparation,
                    continuation: continuation
                )
                configuration = TranslationSession.Configuration(
                    source: source,
                    target: target
                )
            }
        } onCancel: {
            Task { @MainActor [weak self] in
                self?.cancelPendingRequest()
            }
        }
    }

    func run(_ session: any TranslationSessionExecuting) async {
        guard let request = requestSnapshot() else {
            return
        }

        do {
            if request.preparation == .required {
                activateForDownloadPrompt()
                try await session.prepareTranslation()
                guard hasPendingRequest(request.id) else {
                    return
                }
            }

            let response = try await session.translate(request.text)
            complete(
                request.id,
                with: .success(
                    AppleTranslationResult(
                        sourceLanguageIdentifier: response.sourceLanguage.minimalIdentifier,
                        targetLanguageIdentifier: response.targetLanguage.minimalIdentifier,
                        translatedText: response.targetText
                    )
                )
            )
        } catch {
            complete(request.id, with: .failure(error))
        }
    }

    private func requestSnapshot() -> RequestSnapshot? {
        pendingRequest.map {
            RequestSnapshot(
                id: $0.id,
                text: $0.text,
                preparation: $0.preparation
            )
        }
    }

    private func hasPendingRequest(_ requestID: UUID) -> Bool {
        pendingRequest?.id == requestID
    }

    private func activateForDownloadPrompt() {
        downloadPromptActivator()
    }

    private func cancelPendingRequest() {
        guard let request = pendingRequest else {
            return
        }

        pendingRequest = nil
        configuration = nil
        request.continuation.resume(throwing: CancellationError())
    }

    private func complete(
        _ requestID: UUID,
        with result: Result<AppleTranslationResult, any Error>
    ) {
        guard let request = pendingRequest, request.id == requestID else {
            return
        }

        pendingRequest = nil
        configuration = nil
        request.continuation.resume(with: result)
    }
}
