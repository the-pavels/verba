import AppKit
import SwiftUI

@MainActor
final class PopupController {
    private static let contentSize = NSSize(width: 380, height: 112)

    private let hostingController: NSHostingController<PopupContentView>
    private let panel: PopupPanel

    init() {
        hostingController = NSHostingController(
            rootView: PopupContentView(presentation: .idle)
        )
        panel = PopupPanel(contentSize: Self.contentSize)
        panel.contentViewController = hostingController
        panel.onDismissRequest = { [weak self] in
            self?.dismiss()
        }
    }

    func present(_ presentation: PresentationViewModel) {
        guard !presentation.isIdle else {
            dismiss()
            return
        }

        hostingController.rootView = PopupContentView(presentation: presentation)
        panel.setContentSize(Self.contentSize)
        panel.setFrameOrigin(
            PopupPositioner.origin(
                popupSize: Self.contentSize,
                pointer: NSEvent.mouseLocation,
                screens: NSScreen.screens
            )
        )
        panel.orderFrontRegardless()
        panel.makeKey()
    }

    func dismiss() {
        panel.orderOut(nil)
    }
}

private extension PresentationViewModel {
    var isIdle: Bool {
        if case .idle = self {
            true
        } else {
            false
        }
    }
}
