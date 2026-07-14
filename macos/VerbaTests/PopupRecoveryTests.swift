import XCTest
@testable import Verba

final class PopupRecoveryTests: XCTestCase {
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
