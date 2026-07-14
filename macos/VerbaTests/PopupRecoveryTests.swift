import XCTest
@testable import Verba

final class PopupRecoveryTests: XCTestCase {
    func testProofreadingDiffMarksReplacedWords() {
        let diff = ProofreadingDiff(
            original: "Der Vater satzte das Kind auf den Stuhl.",
            corrected: "Der Vater setzte das Kind auf den Stuhl."
        )

        XCTAssertEqual(
            diff.original,
            [
                .init(text: "Der Vater ", change: .unchanged),
                .init(text: "satzte", change: .removed),
                .init(text: " das Kind auf den Stuhl.", change: .unchanged),
            ]
        )
        XCTAssertEqual(
            diff.corrected,
            [
                .init(text: "Der Vater ", change: .unchanged),
                .init(text: "setzte", change: .added),
                .init(text: " das Kind auf den Stuhl.", change: .unchanged),
            ]
        )
    }

    func testProofreadingDiffMarksInsertionsWithoutChangingSurroundingText() {
        let diff = ProofreadingDiff(
            original: "This sentence now correct",
            corrected: "This sentence is now correct."
        )

        XCTAssertEqual(
            diff.original,
            [.init(text: "This sentence now correct", change: .unchanged)]
        )
        XCTAssertEqual(
            diff.corrected,
            [
                .init(text: "This sentence ", change: .unchanged),
                .init(text: "is ", change: .added),
                .init(text: "now correct", change: .unchanged),
                .init(text: ".", change: .added),
            ]
        )
    }

    func testEveryRecoveryActionHasAConciseButtonAndCommand() {
        let cases: [(RecoveryActionViewModel, String, PopupRecoveryCommand)] = [
            (.retry, "Retry", .retry(.translate)),
            (.openSettings, "Open Settings", .openSettings),
            (.grantAccessibility, "Grant Access", .grantAccessibility),
            (.changeLanguage, "Change Language", .openSettings),
            (.dismiss, "Dismiss", .dismiss),
        ]

        for (recovery, expectedTitle, expectedCommand) in cases {
            XCTAssertEqual(recovery.buttonTitle, expectedTitle)
            XCTAssertEqual(recovery.command(for: .translate), expectedCommand)
        }
    }

    func testRetryWithoutAnActionFallsBackToDismiss() {
        XCTAssertEqual(
            RecoveryActionViewModel.retry.command(for: nil),
            .dismiss
        )
    }
}
