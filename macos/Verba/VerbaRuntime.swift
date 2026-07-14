import Foundation

@MainActor
final class VerbaRuntime {
    private let observer: PopupPresentationObserver
    private var application: ApplicationRuntime?
    private let apiKeySettings: OpenAiApiKeySettings?
    private weak var popupController: PopupController?

    init(
        popupController: PopupController,
        translator: NativeTranslator,
        performance: PerformanceSignposter
    ) {
        let observer = PopupPresentationObserver(
            popupController: popupController,
            performance: performance
        )
        self.observer = observer
        self.popupController = popupController
        apiKeySettings = try? OpenAiApiKeySettings()

        do {
            let application = try ApplicationRuntime(
                observer: observer,
                translator: translator,
                performanceObserver: performance
            )
            self.application = application
            popupController.onDismiss = { [weak application] in
                _ = application?.cancelActive()
            }
            popupController.onProofreadingDisclosureContinue = { [weak application] in
                _ = try? application?.acknowledgeProofreadingDisclosure()
            }
            popupController.onRetry = { [weak application] action in
                application?.retry(action: action)
            }
        } catch let error as ApplicationRuntimeError {
            application = nil
            switch error {
            case .ShortcutRegistrationFailed:
                popupController.present(
                    .error(
                        action: nil,
                        title: LocalizedCopy.text("Shortcuts unavailable"),
                        message: LocalizedCopy.text("Quit other Verba instances and reopen the app."),
                        recovery: .dismiss,
                        diagnosticCode: "runtime.shortcut-registration"
                    )
                )
            case .SettingsUnavailable:
                popupController.present(
                    .error(
                        action: nil,
                        title: LocalizedCopy.text("Settings unavailable"),
                        message: LocalizedCopy.text("Quit and reopen Verba, then try again."),
                        recovery: .dismiss,
                        diagnosticCode: "runtime.settings-unavailable"
                    )
                )
            case .ProofreadingUnavailable:
                popupController.present(
                    .error(
                        action: .proofread,
                        title: LocalizedCopy.text("Proofreading unavailable"),
                        message: LocalizedCopy.text("Quit and reopen Verba, then try again."),
                        recovery: .dismiss,
                        diagnosticCode: "runtime.proofreading-unavailable"
                    )
                )
            }
        } catch {
            application = nil
            popupController.present(
                .error(
                    action: nil,
                    title: LocalizedCopy.text("Verba unavailable"),
                    message: LocalizedCopy.text("Quit and reopen Verba, then try again."),
                    recovery: .dismiss,
                    diagnosticCode: "runtime.unknown"
                )
            )
        }
    }
}

extension VerbaRuntime: ApplicationLifecycleRuntime {
    func prepareForSleep() {
        _ = try? application?.prepareForSleep()
    }

    func resumeAfterWake() {
        do {
            try application?.resumeAfterWake()
        } catch {
            popupController?.present(
                .error(
                    action: nil,
                    title: LocalizedCopy.text("Shortcuts unavailable"),
                    message: LocalizedCopy.text(
                        "Quit and reopen Verba to restore global shortcuts."
                    ),
                    recovery: .dismiss,
                    diagnosticCode: "runtime.wake-shortcut-registration"
                )
            )
        }
    }

    func shutdown() {
        application?.shutdown()
        application = nil
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

extension VerbaRuntime: ShortcutSettingsManaging {
    func shortcutConfiguration() throws -> ShortcutDisplayConfiguration {
        guard let application else {
            throw VerbaRuntimeError.unavailable
        }
        return displayConfiguration(application.shortcutConfiguration())
    }

    func setShortcut(
        _ shortcut: RecordedShortcut,
        for action: ShortcutPreferenceAction
    ) throws -> ShortcutDisplayConfiguration {
        guard let application else {
            throw VerbaRuntimeError.unavailable
        }
        do {
            let configuration = try application.setShortcut(
                action: action == .translate ? .translate : .proofread,
                input: ShortcutInput(
                    key: shortcut.key,
                    command: shortcut.command,
                    control: shortcut.control,
                    option: shortcut.option,
                    shift: shortcut.shift
                )
            )
            return displayConfiguration(configuration)
        } catch {
            throw mapShortcutSettingsError(error)
        }
    }
}

private func displayConfiguration(
    _ configuration: ShortcutConfigurationViewModel
) -> ShortcutDisplayConfiguration {
    ShortcutDisplayConfiguration(
        translate: configuration.translate,
        proofread: configuration.proofread
    )
}

private func mapShortcutSettingsError(_ error: any Error) -> ShortcutSettingsFailure {
    guard let error = error as? ShortcutSettingsError else {
        return .registrationFailed
    }
    return switch error {
    case .InvalidKey: .invalidKey
    case .MissingPrimaryModifier: .missingPrimaryModifier
    case .ReservedShortcut: .reservedShortcut
    case .DuplicateShortcut: .duplicateShortcut
    case .ShortcutUnavailable: .shortcutUnavailable
    case .RegistrationFailed: .registrationFailed
    case .PersistenceFailed: .persistenceFailed
    case .RollbackFailed: .rollbackFailed
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
    private let performance: PerformanceSignposter

    init(popupController: PopupController, performance: PerformanceSignposter) {
        self.popupController = popupController
        self.performance = performance
    }

    func present(requestId: UInt64, presentation: PresentationViewModel) {
        Task { @MainActor [weak popupController, performance] in
            guard let popupController else {
                return
            }
            popupController.present(requestID: requestId, presentation: presentation)
            performance.presentationDidPresent(
                requestID: requestId,
                presentation: presentation
            )
        }
    }
}
