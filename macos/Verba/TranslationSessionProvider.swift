import SwiftUI
@preconcurrency import Translation

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

    private var pendingRequest: PendingRequest?

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

    nonisolated func run(_ session: TranslationSession) async {
        guard let request = await requestSnapshot() else {
            return
        }

        do {
            if request.preparation == .required {
                try await session.prepareTranslation()
                guard await hasPendingRequest(request.id) else {
                    return
                }
            }

            let response = try await session.translate(request.text)
            await complete(
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
            await complete(request.id, with: .failure(error))
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

struct TranslationSessionHost: View {
    @ObservedObject var sessions: SystemTranslationSessionProvider

    var body: some View {
        Color.clear
            .frame(width: 0, height: 0)
            .translationTask(sessions.configuration) { session in
                await sessions.run(session)
            }
    }
}
