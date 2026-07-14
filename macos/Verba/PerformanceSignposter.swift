import Foundation
import os

enum PerformanceBudget {
    static let appInitializationMilliseconds: UInt64 = 750
    static let shortcutToLoadingPopupMilliseconds: UInt64 = 100
    static let shortcutToCaptureCompletedMilliseconds: UInt64 = 650
    static let processingToTerminalPopupMilliseconds: UInt64 = 50
}

enum PerformancePresentation: String, Equatable, Sendable {
    case idle
    case loading
    case proofreadingDisclosure = "proofreading-disclosure"
    case translation
    case proofreading
    case noIssues = "no-issues"
    case error

    var isTerminal: Bool {
        self != .loading && self != .proofreadingDisclosure
    }
}

enum PerformanceTraceMilestone: Equatable, Sendable {
    case startupStarted
    case startupReady
    case requestStarted
    case captureCompleted
    case processingCompleted
    case popupPresented(PerformancePresentation)
    case requestCancelled
}

struct PerformanceTraceRecord: Equatable, Sendable {
    let milestone: PerformanceTraceMilestone
    let requestID: UInt64?
    let action: PresentationAction?
}

final class PerformanceSignposter: PerformanceObserver, @unchecked Sendable {
    typealias Recorder = @Sendable (PerformanceTraceRecord) -> Void

    private struct ActiveWorkflow {
        let action: PresentationAction
        let id: OSSignpostID
        let interval: OSSignpostIntervalState
    }

    private let signposter: OSSignposter
    private let recorder: Recorder
    private let lock = NSLock()
    private var startupInterval: OSSignpostIntervalState?
    private var workflows: [UInt64: ActiveWorkflow] = [:]

    init(
        signposter: OSSignposter = OSSignposter(
            subsystem: Bundle.main.bundleIdentifier ?? "io.github.the-pavels.verba",
            category: "Performance"
        ),
        recorder: @escaping Recorder = { _ in }
    ) {
        self.signposter = signposter
        self.recorder = recorder
        startupInterval = signposter.beginInterval(
            "Startup",
            "budget_ms=\(PerformanceBudget.appInitializationMilliseconds, privacy: .public)"
        )
        record(.startupStarted)
    }

    func startupReady() {
        lock.lock()
        let interval = startupInterval
        startupInterval = nil
        lock.unlock()
        guard let interval else {
            return
        }

        signposter.endInterval(
            "Startup",
            interval,
            "budget_ms=\(PerformanceBudget.appInitializationMilliseconds, privacy: .public)"
        )
        record(.startupReady)
    }

    func requestStarted(requestId: UInt64, action: PresentationAction) {
        let id = signposter.makeSignpostID()
        let interval = signposter.beginInterval(
            "TextAction",
            id: id,
            "request_id=\(requestId, privacy: .public) action=\(action.metricName, privacy: .public)"
        )
        lock.lock()
        let replaced = workflows.updateValue(
            ActiveWorkflow(action: action, id: id, interval: interval),
            forKey: requestId
        )
        lock.unlock()
        if let replaced {
            signposter.endInterval("TextAction", replaced.interval, "outcome=replaced")
        }
        record(.requestStarted, requestID: requestId, action: action)
    }

    func captureCompleted(requestId: UInt64) {
        guard let workflow = workflow(for: requestId) else {
            return
        }
        signposter.emitEvent(
            "CaptureCompleted",
            id: workflow.id,
            "request_id=\(requestId, privacy: .public) budget_ms=\(PerformanceBudget.shortcutToCaptureCompletedMilliseconds, privacy: .public)"
        )
        record(.captureCompleted, requestID: requestId, action: workflow.action)
    }

    func processingCompleted(requestId: UInt64) {
        guard let workflow = workflow(for: requestId) else {
            return
        }
        signposter.emitEvent(
            "ProcessingCompleted",
            id: workflow.id,
            "request_id=\(requestId, privacy: .public)"
        )
        record(.processingCompleted, requestID: requestId, action: workflow.action)
    }

    func requestCancelled(requestId: UInt64) {
        guard let workflow = removeWorkflow(for: requestId) else {
            return
        }
        signposter.endInterval(
            "TextAction",
            workflow.interval,
            "request_id=\(requestId, privacy: .public) outcome=cancelled"
        )
        record(.requestCancelled, requestID: requestId, action: workflow.action)
    }

    func presentationDidPresent(
        requestID: UInt64,
        presentation: PresentationViewModel
    ) {
        let kind = presentation.performancePresentation
        guard let workflow = kind.isTerminal
            ? removeWorkflow(for: requestID)
            : workflow(for: requestID)
        else {
            return
        }

        let budget = kind == .loading
            ? PerformanceBudget.shortcutToLoadingPopupMilliseconds
            : PerformanceBudget.processingToTerminalPopupMilliseconds
        signposter.emitEvent(
            "PopupPresented",
            id: workflow.id,
            "request_id=\(requestID, privacy: .public) state=\(kind.rawValue, privacy: .public) budget_ms=\(budget, privacy: .public)"
        )
        record(.popupPresented(kind), requestID: requestID, action: workflow.action)

        if kind.isTerminal {
            signposter.endInterval(
                "TextAction",
                workflow.interval,
                "request_id=\(requestID, privacy: .public) outcome=\(kind.rawValue, privacy: .public)"
            )
        }
    }

    private func workflow(for requestID: UInt64) -> ActiveWorkflow? {
        lock.lock()
        defer { lock.unlock() }
        return workflows[requestID]
    }

    private func removeWorkflow(for requestID: UInt64) -> ActiveWorkflow? {
        lock.lock()
        defer { lock.unlock() }
        return workflows.removeValue(forKey: requestID)
    }

    private func record(
        _ milestone: PerformanceTraceMilestone,
        requestID: UInt64? = nil,
        action: PresentationAction? = nil
    ) {
        recorder(
            PerformanceTraceRecord(
                milestone: milestone,
                requestID: requestID,
                action: action
            )
        )
    }
}

private extension PresentationAction {
    var metricName: String {
        switch self {
        case .translate:
            "translate"
        case .proofread:
            "proofread"
        }
    }
}

private extension PresentationViewModel {
    var performancePresentation: PerformancePresentation {
        switch self {
        case .idle:
            .idle
        case .loading:
            .loading
        case .proofreadingDisclosure:
            .proofreadingDisclosure
        case .translation:
            .translation
        case .proofreading:
            .proofreading
        case .noIssues:
            .noIssues
        case .error:
            .error
        }
    }
}
