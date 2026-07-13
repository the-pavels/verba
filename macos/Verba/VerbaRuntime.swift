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
        } catch {
            application = nil
            popupController.present(
                .error(
                    action: nil,
                    title: "Shortcuts unavailable",
                    message: "Quit other Verba instances and reopen the app."
                )
            )
        }
    }
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
