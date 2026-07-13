import AppKit
import SwiftUI

@MainActor
final class PopupController {
    private static let compactContentSize = NSSize(width: 380, height: 112)
    private static let clickEventMask: NSEvent.EventTypeMask = [
        .leftMouseDown,
        .rightMouseDown,
        .otherMouseDown,
    ]

    private let hostingController: NSHostingController<PopupContentView>
    private let panel: PopupPanel
    private var clickMonitors: [Any] = []

    init() {
        hostingController = NSHostingController(
            rootView: PopupContentView(presentation: .idle)
        )
        panel = PopupPanel(contentSize: Self.compactContentSize)
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

        let contentSize = presentation.contentSize
        hostingController.rootView = PopupContentView(presentation: presentation)
        panel.setContentSize(contentSize)
        panel.setFrameOrigin(
            PopupPositioner.origin(
                popupSize: contentSize,
                pointer: NSEvent.mouseLocation,
                screens: NSScreen.screens
            )
        )
        panel.orderFrontRegardless()
        panel.makeKey()
        startClickAwayMonitoring()
    }

    func dismiss() {
        stopClickAwayMonitoring()
        panel.orderOut(nil)
    }

    private func startClickAwayMonitoring() {
        guard clickMonitors.isEmpty else {
            return
        }

        if let localMonitor = NSEvent.addLocalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] event in
                guard let self, self.panel.isVisible, event.window !== self.panel else {
                    return event
                }

                self.dismiss()
                return event
            }
        ) {
            clickMonitors.append(localMonitor)
        }

        if let globalMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.dismiss()
                }
            }
        ) {
            clickMonitors.append(globalMonitor)
        }
    }

    private func stopClickAwayMonitoring() {
        clickMonitors.forEach(NSEvent.removeMonitor)
        clickMonitors.removeAll()
    }
}

private extension PresentationViewModel {
    var contentSize: NSSize {
        switch self {
        case .translation:
            NSSize(width: 420, height: 300)
        case .proofreading:
            NSSize(width: 420, height: 260)
        case .error:
            NSSize(width: 380, height: 136)
        default:
            NSSize(width: 380, height: 112)
        }
    }

    var isIdle: Bool {
        if case .idle = self {
            true
        } else {
            false
        }
    }
}
