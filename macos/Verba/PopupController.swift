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
    private var menuTrackingMonitors: [NotificationMonitor] = []
    private var menuTrackingDepth = 0
    private var copyableText: String?
    private var captureFocusState = PopupCaptureFocusState()
    private var sourceApplication: NSRunningApplication?

    var onDismiss: (() -> Void)?
    var onProofreadingDisclosureContinue: (() -> Void)?
    var onRetry: ((PresentationAction) -> Void)?
    var onGrantAccessibility: (() -> Void)?
    var onDiagnosticCode: ((String) -> Void)?
    var onTargetLanguageChanged: ((String) -> Void)?
    var targetLanguageSettings: TargetLanguageSettingsController?

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
                targetLanguages: nil,
                selectTargetLanguage: { _, _ in },
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
        present(presentation, shouldTakeKeyboardFocus: true)
    }

    private func present(
        _ presentation: PresentationViewModel,
        shouldTakeKeyboardFocus: Bool
    ) {
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
            targetLanguages: targetLanguageSettings,
            selectTargetLanguage: { [weak self] identifier, originalText in
                self?.selectTargetLanguage(identifier, originalText: originalText)
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
        if shouldTakeKeyboardFocus {
            takeKeyboardFocus()
        }
        startClickAwayMonitoring()
    }

    func present(requestID: UInt64, presentation: PresentationViewModel) {
        guard let decision = captureFocusState.receive(
            requestID: requestID,
            isLoading: presentation.isLoading
        ) else {
            return
        }

        if decision.beginsCapture {
            preserveSourceFocusForCapture()
        }
        present(
            presentation,
            shouldTakeKeyboardFocus: decision.shouldTakeKeyboardFocus
        )
    }

    func captureDidComplete(requestID: UInt64) {
        guard captureFocusState.captureDidComplete(requestID: requestID),
              panel.isVisible
        else {
            return
        }
        takeKeyboardFocus()
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
        captureFocusState.dismiss()
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

    private func preserveSourceFocusForCapture() {
        let frontmostApplication = NSWorkspace.shared.frontmostApplication
        if PopupSourceApplicationPolicy.shouldCapture(
            candidateProcessIdentifier: frontmostApplication?.processIdentifier,
            currentProcessIdentifier: NSRunningApplication.current.processIdentifier
        ) {
            sourceApplication = frontmostApplication
        }
        if panel.isKeyWindow, let sourceApplication {
            restoreFocus(to: sourceApplication)
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

    func selectTargetLanguage(_ identifier: String, originalText: String) {
        guard let targetLanguageSettings,
              identifier != targetLanguageSettings.selectedIdentifier,
              targetLanguageSettings.select(identifier)
        else {
            return
        }
        onTargetLanguageChanged?(originalText)
    }

    private func startClickAwayMonitoring() {
        guard clickMonitors.isEmpty else {
            return
        }
        startMenuTrackingMonitoring()

        if let localMonitor = NSEvent.addLocalMonitorForEvents(
            matching: Self.clickEventMask,
            handler: { [weak self] event in
                let screenLocation = event.window?.convertPoint(
                    toScreen: event.locationInWindow
                ) ?? NSEvent.mouseLocation
                let menuWasTracking = MainActor.assumeIsolated {
                    (self?.menuTrackingDepth ?? 0) > 0
                }

                Task { @MainActor [weak self] in
                    guard let self,
                          PopupClickAwayPolicy.shouldDismiss(
                              clickLocation: screenLocation,
                              popupFrame: self.panel.frame,
                              menuWasTracking: menuWasTracking
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
        menuTrackingMonitors.removeAll()
        menuTrackingDepth = 0
    }

    private func startMenuTrackingMonitoring() {
        guard menuTrackingMonitors.isEmpty else {
            return
        }

        let center = NotificationCenter.default
        let begin = center.addObserver(
            forName: NSMenu.didBeginTrackingNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.menuTrackingDepth += 1
            }
        }
        let end = center.addObserver(
            forName: NSMenu.didEndTrackingNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else {
                    return
                }
                self.menuTrackingDepth = max(0, self.menuTrackingDepth - 1)
            }
        }
        menuTrackingMonitors = [
            NotificationMonitor(center: center, token: begin),
            NotificationMonitor(center: center, token: end),
        ]
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

private final class NotificationMonitor: @unchecked Sendable {
    private let center: NotificationCenter
    private let token: NSObjectProtocol

    init(center: NotificationCenter, token: NSObjectProtocol) {
        self.center = center
        self.token = token
    }

    deinit {
        center.removeObserver(token)
    }
}

private struct TranslationPopupHost: View {
    let presentation: PresentationViewModel
    let contentSize: NSSize
    let copyText: (String) -> Void
    let continueProofreading: () -> Void
    let cancelProofreading: () -> Void
    let recover: (RecoveryActionViewModel, PresentationAction?) -> Void
    let targetLanguages: TargetLanguageSettingsController?
    let selectTargetLanguage: (String, String) -> Void
    @ObservedObject var translationSessions: SystemTranslationSessionProvider

    var body: some View {
        PopupContentView(
            presentation: presentation,
            copyText: copyText,
            continueProofreading: continueProofreading,
            cancelProofreading: cancelProofreading,
            recover: recover,
            targetLanguages: targetLanguages,
            selectTargetLanguage: selectTargetLanguage
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
        case let .translation(originalText, _, translatedText):
            resultSize(originalText: originalText, resultText: translatedText)
        case let .proofreading(originalText, correctedText):
            resultSize(originalText: originalText, resultText: correctedText)
        case .proofreadingDisclosure:
            NSSize(width: 420, height: 190)
        case let .error(_, title, message, _, _):
            errorSize(title: title, message: message)
        case .noIssues:
            NSSize(width: 380, height: 140)
        default:
            NSSize(width: 380, height: 112)
        }
        let scale = min(max(textScale, 1), 1.5)
        return NSSize(
            width: (baseSize.width * scale).rounded(),
            height: min((baseSize.height * scale).rounded(), maximumHeight)
        )
    }

    private static let resultMinimumHeight: CGFloat = 250
    private static let resultMaximumHeight: CGFloat = 480
    private static let errorMinimumHeight: CGFloat = 170
    private static let errorMaximumHeight: CGFloat = 280
    private static let maximumHeight: CGFloat = 560
    private static let resultColumns = 46

    private static func resultSize(originalText: String, resultText: String) -> NSSize {
        let lineCount = estimatedLineCount(in: originalText, columns: resultColumns)
            + estimatedLineCount(in: resultText, columns: resultColumns)
        let height = 210 + CGFloat(max(0, lineCount - 2)) * 20

        return NSSize(
            width: 420,
            height: min(max(height, resultMinimumHeight), resultMaximumHeight)
        )
    }

    private static func errorSize(title: String, message: String) -> NSSize {
        let lineCount = estimatedLineCount(in: title, columns: 32)
            + estimatedLineCount(in: message, columns: 42)
        let height = errorMinimumHeight + CGFloat(max(0, lineCount - 2)) * 20

        return NSSize(
            width: 380,
            height: min(height, errorMaximumHeight)
        )
    }

    private static func estimatedLineCount(in text: String, columns: Int) -> Int {
        text.split(separator: "\n", omittingEmptySubsequences: false).reduce(0) {
            lineCount, paragraph in
            lineCount + max(1, Int(ceil(Double(paragraph.count) / Double(columns))))
        }
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

struct PopupCaptureFocusDecision: Equatable {
    let beginsCapture: Bool
    let shouldTakeKeyboardFocus: Bool
}

struct PopupCaptureFocusState {
    private var latestRequestID: UInt64 = 0
    private var capturingRequestID: UInt64?
    private var completedBeforePresentation: Set<UInt64> = []

    mutating func receive(
        requestID: UInt64,
        isLoading: Bool
    ) -> PopupCaptureFocusDecision? {
        guard requestID >= latestRequestID,
              requestID >= (completedBeforePresentation.max() ?? 0)
        else {
            return nil
        }

        let captureAlreadyCompleted = completedBeforePresentation.remove(requestID) != nil
        let beginsCapture = requestID > latestRequestID
            && isLoading
            && !captureAlreadyCompleted
        if requestID > latestRequestID {
            latestRequestID = requestID
            capturingRequestID = beginsCapture ? requestID : nil
            completedBeforePresentation = Set(
                completedBeforePresentation.filter { $0 >= requestID }
            )
        }
        if !isLoading, capturingRequestID == requestID {
            capturingRequestID = nil
        }

        return PopupCaptureFocusDecision(
            beginsCapture: beginsCapture,
            shouldTakeKeyboardFocus: capturingRequestID != requestID
        )
    }

    mutating func captureDidComplete(requestID: UInt64) -> Bool {
        guard requestID >= latestRequestID else {
            return false
        }
        guard requestID == latestRequestID else {
            completedBeforePresentation.insert(requestID)
            return false
        }
        guard capturingRequestID == requestID else {
            return false
        }
        capturingRequestID = nil
        return true
    }

    mutating func dismiss() {
        capturingRequestID = nil
    }
}

enum PopupClickAwayPolicy {
    static func shouldDismiss(
        clickLocation: NSPoint,
        popupFrame: NSRect,
        menuWasTracking: Bool = false
    ) -> Bool {
        !menuWasTracking && !popupFrame.contains(clickLocation)
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

    var isLoading: Bool {
        if case .loading = self {
            true
        } else {
            false
        }
    }
}
