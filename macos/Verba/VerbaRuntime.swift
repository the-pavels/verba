import Foundation

@MainActor
final class VerbaRuntime {
    private let observer: PopupPresentationObserver
    private let application: ApplicationRuntime?

    init(popupController: PopupController, translator: NativeTranslator) {
        let observer = PopupPresentationObserver(popupController: popupController)
        self.observer = observer

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
