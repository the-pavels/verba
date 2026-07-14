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
    private let windowFocusRestorer = PopupFocusRestorer<NSWindow>()
    private var clickMonitors: [ClickMonitor] = []
    private var copyableText: String?
    private var focusGeneration: UInt64 = 0
    private var latestRequestID: UInt64 = 0
    private var sourceApplication: NSRunningApplication?

    var onDismiss: (() -> Void)?
    var onProofreadingDisclosureContinue: (() -> Void)?
    var onRetry: ((PresentationAction) -> Void)?
    var onGrantAccessibility: (() -> Void)?
    var onDiagnosticCode: ((String) -> Void)?

    init(
        translationSessions: SystemTranslationSessionProvider,
        pasteboardWriter: PasteboardWriter = PasteboardWriter()
    ) {
        let initialContentSize = PopupSizePolicy.size(for: .idle, textScale: 1)
        self.pasteboardWriter = pasteboardWriter
        hostingController = NSHostingController(
            rootView: TranslationPopupHost(
                presentation: .idle,
                contentSize: initialContentSize,
                copyText: pasteboardWriter.copy,
                continueProofreading: {},
                cancelProofreading: {},
                recover: { _, _ in },
                translationSessions: translationSessions
            )
        )
        hostingController.sizingOptions = PopupHostingSizingPolicy.options
        panel = PopupPanel(contentSize: initialContentSize)
        panel.setAccessibilityLabel(LocalizedCopy.text("Verba result"))
        panel.contentViewController = hostingController
        panel.onDismissRequest = { [weak self] in
            self?.dismiss()
        }
        panel.onCopyRequest = { [weak self] in
            guard let self, let copyableText = self.copyableText else {
                return
            }
            self.copyAndDismiss(copyableText)
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
        copyableText = presentation.copyableResultText
        hostingController.rootView = TranslationPopupHost(
            presentation: presentation,
            contentSize: contentSize,
            copyText: { [weak self] text in
                self?.copyAndDismiss(text)
            },
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
        panel.setFixedContentSize(contentSize)
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
            windowFocusRestorer.capture(NSApplication.shared.keyWindow, excluding: panel)
            let frontmostApplication = NSWorkspace.shared.frontmostApplication
            sourceApplication = PopupSourceApplicationPolicy.shouldCapture(
                candidateProcessIdentifier: frontmostApplication?.processIdentifier,
                currentProcessIdentifier: NSRunningApplication.current.processIdentifier
            )
                ? frontmostApplication
                : nil
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
        dismiss(focusDisposition: .restoreSource)
    }

    private func dismiss(focusDisposition: PopupFocusDisposition) {
        hide(focusDisposition: focusDisposition)
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

    private func hide(focusDisposition: PopupFocusDisposition = .restoreSource) {
        focusGeneration &+= 1
        copyableText = nil
        stopClickAwayMonitoring()
        let shouldRestoreFocus = focusDisposition.shouldRestoreSource(
            panelWasKey: panel.isKeyWindow
        )
        let previousKeyWindow = windowFocusRestorer.take()
        let sourceApplication = sourceApplication
        self.sourceApplication = nil
        panel.orderOut(nil)
        if shouldRestoreFocus {
            if let previousKeyWindow {
                previousKeyWindow.makeKey()
                NSAccessibility.post(
                    element: previousKeyWindow,
                    notification: .focusedWindowChanged
                )
            } else if let sourceApplication {
                restoreFocus(to: sourceApplication)
            }
        }
    }

    private func restoreFocus(to application: NSRunningApplication) {
        NSApplication.shared.yieldActivation(to: application)
        application.activate(from: NSRunningApplication.current, options: [])
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

        NSApplication.shared.activate()
        panel.makeKeyAndOrderFront(nil)
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

    private func copyAndDismiss(_ text: String) {
        pasteboardWriter.copy(text)
        dismiss()
    }

    private func startClickAwayMonitoring() {
        guard clickMonitors.isEmpty else {
            return
        }

        if let localMonitor = NSEvent.addLocalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] event in
                let screenLocation = event.window?.convertPoint(
                    toScreen: event.locationInWindow
                ) ?? NSEvent.mouseLocation

                Task { @MainActor [weak self] in
                    guard let self,
                          PopupClickAwayPolicy.shouldDismiss(
                              clickLocation: screenLocation,
                              popupFrame: self.panel.frame
                          )
                    else {
                        return
                    }
                    self.dismiss(focusDisposition: .preserveCurrent)
                }
                return event
            }
        ) {
            clickMonitors.append(ClickMonitor(token: localMonitor))
        }

        if let globalMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] _ in
                Task { @MainActor [weak self] in
                    self?.dismiss(focusDisposition: .preserveCurrent)
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
    let contentSize: NSSize
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
        .frame(width: contentSize.width, height: contentSize.height)
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

enum PopupHostingSizingPolicy {
    static let options: NSHostingSizingOptions = []
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

enum PopupClickAwayPolicy {
    static func shouldDismiss(clickLocation: NSPoint, popupFrame: NSRect) -> Bool {
        !popupFrame.contains(clickLocation)
    }
}

enum PopupFocusDisposition: Equatable {
    case restoreSource
    case preserveCurrent

    func shouldRestoreSource(panelWasKey: Bool) -> Bool {
        self == .restoreSource && panelWasKey
    }
}

enum PopupSourceApplicationPolicy {
    static func shouldCapture(
        candidateProcessIdentifier: pid_t?,
        currentProcessIdentifier: pid_t
    ) -> Bool {
        candidateProcessIdentifier.map { $0 != currentProcessIdentifier } ?? false
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

    var copyableResultText: String? {
        switch self {
        case let .translation(_, _, translatedText):
            translatedText
        case let .proofreading(_, correctedText):
            correctedText
        default:
            nil
        }
    }
}
