import Foundation

@MainActor
final class VerbaRuntime {
    private let observer: PopupPresentationObserver
    private let application: ApplicationRuntime?
    private let apiKeySettings: OpenAiApiKeySettings?

    init(popupController: PopupController, translator: NativeTranslator) {
        let observer = PopupPresentationObserver(popupController: popupController)
        self.observer = observer
        apiKeySettings = try? OpenAiApiKeySettings()

        do {
            let application = try ApplicationRuntime(
                observer: observer,
                translator: translator
            )
            self.application = application
            popupController.onDismiss = { [weak application] in
                _ = application?.cancelActive()
            }
        } catch let error as ApplicationRuntimeError {
            application = nil
            switch error {
            case .ShortcutRegistrationFailed:
                popupController.present(
                    .error(
                        action: nil,
                        title: "Shortcuts unavailable",
                        message: "Quit other Verba instances and reopen the app."
                    )
                )
            case .SettingsUnavailable:
                popupController.present(
                    .error(
                        action: nil,
                        title: "Settings unavailable",
                        message: "Quit and reopen Verba, then try again."
                    )
                )
            }
        } catch {
            application = nil
            popupController.present(
                .error(
                    action: nil,
                    title: "Verba unavailable",
                    message: "Quit and reopen Verba, then try again."
                )
            )
        }
    }
}

extension VerbaRuntime: TargetLanguagePreferenceManaging {
    func configureSupportedTargetLanguages(_ identifiers: [String]) throws -> String {
        guard let application else {
            throw VerbaRuntimeError.unavailable
        }
        return try application.configureSupportedTargetLanguages(identifiers: identifiers)
    }

    func setTargetLanguage(_ identifier: String) throws {
        guard let application else {
            throw VerbaRuntimeError.unavailable
        }
        try application.setTargetLanguage(identifier: identifier)
    }
}

extension VerbaRuntime: ApiKeySettingsManaging {
    func isApiKeyConfigured() throws -> Bool {
        guard let apiKeySettings else {
            throw ApiKeySettingsFailure.connectionFailed
        }
        do {
            return try apiKeySettings.isConfigured()
        } catch {
            throw mapApiKeySettingsError(error)
        }
    }

    func saveApiKey(_ apiKey: String) throws {
        guard let apiKeySettings else {
            throw ApiKeySettingsFailure.connectionFailed
        }
        do {
            try apiKeySettings.save(apiKey: apiKey)
        } catch {
            throw mapApiKeySettingsError(error)
        }
    }

    func deleteApiKey() throws {
        guard let apiKeySettings else {
            throw ApiKeySettingsFailure.connectionFailed
        }
        do {
            try apiKeySettings.delete()
        } catch {
            throw mapApiKeySettingsError(error)
        }
    }

    func testApiKeyConnection() async throws {
        guard let apiKeySettings else {
            throw ApiKeySettingsFailure.connectionFailed
        }
        do {
            try await apiKeySettings.testConnection()
        } catch {
            throw mapApiKeySettingsError(error)
        }
    }
}

private func mapApiKeySettingsError(_ error: any Error) -> ApiKeySettingsFailure {
    guard let error = error as? OpenAiApiKeyError else {
        return .connectionFailed
    }
    return switch error {
    case .InvalidApiKey: .invalidApiKey
    case .NotConfigured: .notConfigured
    case .KeychainUnavailable: .keychainUnavailable
    case .Authentication: .authentication
    case .RateLimited: .rateLimited
    case .QuotaExceeded: .quotaExceeded
    case .Offline: .offline
    case .TimedOut: .timedOut
    case .ServiceUnavailable: .serviceUnavailable
    case .InvalidResponse: .invalidResponse
    case .ConnectionFailed: .connectionFailed
    }
}

private enum VerbaRuntimeError: Error {
    case unavailable
}

private final class PopupPresentationObserver: PresentationObserver, @unchecked Sendable {
    private weak var popupController: PopupController?

    init(popupController: PopupController) {
        self.popupController = popupController
    }

    func present(requestId: UInt64, presentation: PresentationViewModel) {
        Task { @MainActor [weak popupController] in
            popupController?.present(requestID: requestId, presentation: presentation)
        }
    }
}
