import Foundation
import Testing
@testable import Verba

@Suite("Performance signposts")
struct PerformanceSignposterTests {
    @Test("records fixed workflow metadata without presentation contents")
    func recordsPrivacySafeMilestones() {
        let records = LockedPerformanceRecords()
        let signposter = PerformanceSignposter(signposter: .disabled) { record in
            records.append(record)
        }
        let selectedText = "private selected text"

        signposter.startupReady()
        signposter.requestStarted(requestId: 41, action: .translate)
        signposter.captureCompleted(requestId: 41)
        signposter.processingCompleted(requestId: 41)
        signposter.presentationDidPresent(
            requestID: 41,
            presentation: .loading(action: .translate)
        )
        signposter.presentationDidPresent(
            requestID: 41,
            presentation: .translation(
                originalText: selectedText,
                languagePair: LanguagePairViewModel(source: "German", target: "English"),
                translatedText: "also private"
            )
        )

        let snapshot = records.snapshot()
        #expect(
            snapshot == [
                PerformanceTraceRecord(
                    milestone: .startupStarted,
                    requestID: nil,
                    action: nil
                ),
                PerformanceTraceRecord(
                    milestone: .startupReady,
                    requestID: nil,
                    action: nil
                ),
                PerformanceTraceRecord(
                    milestone: .requestStarted,
                    requestID: 41,
                    action: .translate
                ),
                PerformanceTraceRecord(
                    milestone: .captureCompleted,
                    requestID: 41,
                    action: .translate
                ),
                PerformanceTraceRecord(
                    milestone: .processingCompleted,
                    requestID: 41,
                    action: .translate
                ),
                PerformanceTraceRecord(
                    milestone: .popupPresented(.loading),
                    requestID: 41,
                    action: .translate
                ),
                PerformanceTraceRecord(
                    milestone: .popupPresented(.translation),
                    requestID: 41,
                    action: .translate
                ),
            ]
        )
        #expect(!String(describing: snapshot).contains(selectedText))
        #expect(!String(describing: snapshot).contains("also private"))
    }

    @Test("cancellation closes a workflow and ignores stale milestones")
    func cancellationClosesWorkflow() {
        let records = LockedPerformanceRecords()
        let signposter = PerformanceSignposter(signposter: .disabled) { record in
            records.append(record)
        }

        signposter.requestStarted(requestId: 9, action: .proofread)
        signposter.requestCancelled(requestId: 9)
        signposter.captureCompleted(requestId: 9)
        signposter.processingCompleted(requestId: 9)
        signposter.presentationDidPresent(
            requestID: 9,
            presentation: .proofreading(
                originalText: "secret",
                correctedText: "secret",
                explanation: "secret"
            )
        )

        #expect(
            records.snapshot().map(\.milestone) == [
                .startupStarted,
                .requestStarted,
                .requestCancelled,
            ]
        )
    }

    @Test("budgets match the documented thresholds")
    func documentedBudgets() {
        #expect(PerformanceBudget.appInitializationMilliseconds == 750)
        #expect(PerformanceBudget.shortcutToLoadingPopupMilliseconds == 100)
        #expect(PerformanceBudget.shortcutToCaptureCompletedMilliseconds == 650)
        #expect(PerformanceBudget.processingToTerminalPopupMilliseconds == 50)
    }
}

private final class LockedPerformanceRecords: @unchecked Sendable {
    private let lock = NSLock()
    private var records: [PerformanceTraceRecord] = []

    func append(_ record: PerformanceTraceRecord) {
        lock.lock()
        records.append(record)
        lock.unlock()
    }

    func snapshot() -> [PerformanceTraceRecord] {
        lock.lock()
        defer { lock.unlock() }
        return records
    }
}
