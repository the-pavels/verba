import AppKit
import SwiftUI

@MainActor
final class PopupController {
    private static let clickEventMask: NSEvent.EventTypeMask = [
        .leftMouseDown,
        .rightMouseDown,
        .otherMouseDown,
    ]

    private let hostingController: NSHostingController<TranslationPopupHost>
    private let panel: PopupPanel
    private let pasteboardWriter: PasteboardWriter
    private let focusRestorer = PopupFocusRestorer<NSWindow>()
    private var clickMonitors: [ClickMonitor] = []
    private var focusGeneration: UInt64 = 0
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
        panel = PopupPanel(
            contentSize: PopupSizePolicy.size(for: .idle, textScale: 1)
        )
        panel.setAccessibilityLabel(LocalizedCopy.text("Verba result"))
        panel.contentViewController = hostingController
        panel.onDismissRequest = { [weak self] in
            self?.dismiss()
        }
    }

    func present(_ presentation: PresentationViewModel) {
        let presentation = presentation.localizedForDisplay
        guard !presentation.isIdle else {
            hide()
            return
        }

        if case let .error(_, _, _, _, diagnosticCode) = presentation {
            onDiagnosticCode?(diagnosticCode)
        }

        let contentSize = PopupSizePolicy.size(
            for: presentation,
            textScale: Self.preferredTextScale
        )
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
        panel.animationBehavior = PopupAnimationPolicy.behavior(
            reduceMotion: NSWorkspace.shared.accessibilityDisplayShouldReduceMotion
        )
        panel.setFrameOrigin(
            PopupPositioner.origin(
                popupSize: contentSize,
                pointer: NSEvent.mouseLocation,
                screens: NSScreen.screens
            )
        )
        if !panel.isVisible {
            focusRestorer.capture(NSApplication.shared.keyWindow, excluding: panel)
        }
        panel.orderFrontRegardless()
        scheduleKeyboardFocus(for: presentation)
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
        focusGeneration &+= 1
        stopClickAwayMonitoring()
        let shouldRestoreFocus = panel.isKeyWindow
        let previousKeyWindow = focusRestorer.take()
        panel.orderOut(nil)
        if shouldRestoreFocus {
            previousKeyWindow?.makeKey()
            if let previousKeyWindow {
                NSAccessibility.post(
                    element: previousKeyWindow,
                    notification: .focusedWindowChanged
                )
            }
        }
    }

    private func scheduleKeyboardFocus(for presentation: PresentationViewModel) {
        focusGeneration &+= 1
        let generation = focusGeneration
        let delay = PopupKeyboardFocusPolicy.delay(for: presentation)

        guard delay > 0 else {
            takeKeyboardFocus()
            return
        }

        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self, self.focusGeneration == generation else {
                return
            }
            self.takeKeyboardFocus()
        }
    }

    private func takeKeyboardFocus() {
        guard panel.isVisible else {
            return
        }
        panel.makeKey()
        panel.makeFirstResponder(hostingController.view)
        NSAccessibility.post(element: panel, notification: .focusedWindowChanged)
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

    private static var preferredTextScale: CGFloat {
        let preferredSize = NSFont.preferredFont(forTextStyle: .body).pointSize
        return preferredSize / NSFont.systemFontSize
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
        .translationTask(translationSessions.configuration) { session in
            await translationSessions.run(session)
        }
    }
}

enum PopupSizePolicy {
    static func size(for presentation: PresentationViewModel, textScale: CGFloat) -> NSSize {
        let baseSize = switch presentation {
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
        let scale = min(max(textScale, 1), 1.5)
        return NSSize(
            width: (baseSize.width * scale).rounded(),
            height: (baseSize.height * scale).rounded()
        )
    }
}

enum PopupAnimationPolicy {
    static func behavior(reduceMotion: Bool) -> NSWindow.AnimationBehavior {
        reduceMotion ? .none : .utilityWindow
    }
}

enum PopupKeyboardFocusPolicy {
    private static let captureWindow: TimeInterval = 0.65

    static func delay(for presentation: PresentationViewModel) -> TimeInterval {
        if case .loading = presentation {
            // Synthetic Copy must reach the source app during the bounded capture window.
            captureWindow
        } else {
            0
        }
    }
}

final class PopupFocusRestorer<Window: AnyObject> {
    private weak var previous: Window?

    func capture(_ candidate: Window?, excluding current: Window) {
        previous = candidate === current ? nil : candidate
    }

    func take() -> Window? {
        defer { previous = nil }
        return previous
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
