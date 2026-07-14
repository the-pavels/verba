import XCTest
@testable import Verba

@MainActor
final class FirstRunSetupTests: XCTestCase {
    func testFreshSetupStartsAtWelcomeAndMovesThroughEveryStep() {
        let model = FirstRunSetupModel(store: FakeFirstRunSetupStore())

        XCTAssertEqual(model.step, .welcome)
        model.advance()
        XCTAssertEqual(model.step, .accessibility)
        model.advance()
        XCTAssertEqual(model.step, .essentials)
        model.advance()
        XCTAssertEqual(model.step, .proofreading)
        model.advance()
        XCTAssertEqual(model.step, .ready)
        model.advance()
        XCTAssertEqual(model.step, .ready)
    }

    func testBackNavigationStopsAtWelcome() {
        let model = FirstRunSetupModel(store: FakeFirstRunSetupStore())

        model.goBack()
        XCTAssertEqual(model.step, .welcome)

        model.advance()
        model.advance()
        model.goBack()
        XCTAssertEqual(model.step, .accessibility)
    }

    func testFinishingPersistsCompletion() {
        let store = FakeFirstRunSetupStore()
        let model = FirstRunSetupModel(store: store)

        model.complete()

        XCTAssertTrue(model.isCompleted)
        XCTAssertEqual(store.markCompletedCount, 1)
    }

    func testLaunchPolicyShowsOnlyFreshProductionSetup() {
        XCTAssertTrue(
            FirstRunSetupLaunchPolicy.shouldPresent(
                isCompleted: false,
                isRunningTests: false
            )
        )
        XCTAssertFalse(
            FirstRunSetupLaunchPolicy.shouldPresent(
                isCompleted: true,
                isRunningTests: false
            )
        )
        XCTAssertFalse(
            FirstRunSetupLaunchPolicy.shouldPresent(
                isCompleted: false,
                isRunningTests: true
            )
        )
    }

    func testSetupWindowUsesTheCompactFixedSize() {
        XCTAssertEqual(
            FirstRunSetupWindowController.contentSize,
            NSSize(width: 520, height: 430)
        )
    }
}

private final class FakeFirstRunSetupStore: FirstRunSetupPersisting {
    private(set) var isCompleted = false
    private(set) var markCompletedCount = 0

    func markCompleted() {
        isCompleted = true
        markCompletedCount += 1
    }
}
