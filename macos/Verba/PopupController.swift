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

    private let hostingController: NSHostingController<TranslationPopupHost>
    private let panel: PopupPanel
    private let pasteboardWriter: PasteboardWriter
    private var clickMonitors: [ClickMonitor] = []
    private var latestRequestID: UInt64 = 0

    var onDismiss: (() -> Void)?
    var onProofreadingDisclosureContinue: (() -> Void)?
    var onRetry: ((PresentationAction) -> Void)?
    var onGrantAccessibility: (() -> Void)?
    var onDiagnosticCode: ((String) -> Void)?

    init(
        translationSessions: SystemTranslationSessionProvider,
        pasteboardWriter: PasteboardWriter = PasteboardWriter()
    ) {
        self.pasteboardWriter = pasteboardWriter
        hostingController = NSHostingController(
            rootView: TranslationPopupHost(
                presentation: .idle,
                copyText: pasteboardWriter.copy,
                continueProofreading: {},
                cancelProofreading: {},
                recover: { _, _ in },
                translationSessions: translationSessions
            )
        )
        panel = PopupPanel(contentSize: Self.compactContentSize)
        panel.contentViewController = hostingController
        panel.onDismissRequest = { [weak self] in
            self?.dismiss()
        }
    }

    func present(_ presentation: PresentationViewModel) {
        guard !presentation.isIdle else {
            hide()
            return
        }

        if case let .error(_, _, _, _, diagnosticCode) = presentation {
            onDiagnosticCode?(diagnosticCode)
        }

        let contentSize = presentation.contentSize
        hostingController.rootView = TranslationPopupHost(
            presentation: presentation,
            copyText: pasteboardWriter.copy,
            continueProofreading: { [weak self] in
                self?.onProofreadingDisclosureContinue?()
            },
            cancelProofreading: { [weak self] in
                self?.dismiss()
            },
            recover: { [weak self] recovery, action in
                self?.perform(recovery.command(for: action))
            },
            translationSessions: hostingController.rootView.translationSessions
        )
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

    func present(requestID: UInt64, presentation: PresentationViewModel) {
        guard requestID >= latestRequestID else {
            return
        }

        latestRequestID = requestID
        present(presentation)
    }

    func dismiss() {
        hide()
        onDismiss?()
    }

    func repositionForScreenChange() {
        guard panel.isVisible else {
            return
        }
        panel.setFrameOrigin(
            PopupPositioner.origin(
                popupSize: panel.frame.size,
                pointer: NSEvent.mouseLocation,
                screens: NSScreen.screens
            )
        )
    }

    private func hide() {
        stopClickAwayMonitoring()
        panel.orderOut(nil)
    }

    private func perform(_ command: PopupRecoveryCommand) {
        switch command {
        case let .retry(action):
            hide()
            onRetry?(action)
        case .openSettings:
            NSApplication.shared.activate()
            NSApplication.shared.sendAction(
                Selector(("showSettingsWindow:")),
                to: nil,
                from: nil
            )
        case .grantAccessibility:
            onGrantAccessibility?()
        case .dismiss:
            dismiss()
        }
    }

    private func startClickAwayMonitoring() {
        guard clickMonitors.isEmpty else {
            return
        }

        // App-local clicks include the menu-bar Settings action and the Settings window itself.
        // Keep the current result visible there; the global monitor handles other applications.
        if let globalMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.dismiss()
                }
            }
        ) {
            clickMonitors.append(ClickMonitor(token: globalMonitor))
        }
    }

    private func stopClickAwayMonitoring() {
        clickMonitors.removeAll()
    }
}

extension PopupController: ApplicationLifecyclePopup {}

private final class ClickMonitor: @unchecked Sendable {
    private let token: Any

    init(token: Any) {
        self.token = token
    }

    deinit {
        NSEvent.removeMonitor(token)
    }
}

private struct TranslationPopupHost: View {
    let presentation: PresentationViewModel
    let copyText: (String) -> Void
    let continueProofreading: () -> Void
    let cancelProofreading: () -> Void
    let recover: (RecoveryActionViewModel, PresentationAction?) -> Void
    @ObservedObject var translationSessions: SystemTranslationSessionProvider

    var body: some View {
        PopupContentView(
            presentation: presentation,
            copyText: copyText,
            continueProofreading: continueProofreading,
            cancelProofreading: cancelProofreading,
            recover: recover
        )
        .background {
            TranslationSessionHost(sessions: translationSessions)
        }
    }
}

private extension PresentationViewModel {
    var contentSize: NSSize {
        switch self {
        case .translation:
            NSSize(width: 420, height: 300)
        case .proofreading:
            NSSize(width: 420, height: 260)
        case .proofreadingDisclosure:
            NSSize(width: 420, height: 190)
        case .error:
            NSSize(width: 380, height: 170)
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
