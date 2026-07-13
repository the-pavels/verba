import XCTest
@testable import Verba

@MainActor
final class AccessibilityPermissionControllerTests: XCTestCase {
    func testStatusReflectsTrustAndPromptHistory() {
        let notRequested = makeController(trusted: false, hasRequested: false)
        let denied = makeController(trusted: false, hasRequested: true)
        let granted = makeController(trusted: true, hasRequested: false)

        XCTAssertEqual(notRequested.controller.status, .notRequested)
        XCTAssertEqual(denied.controller.status, .denied)
        XCTAssertEqual(granted.controller.status, .granted)
    }

    func testPermissionPromptIsRequestedOnlyOnce() {
        let fixture = makeController(trusted: false, hasRequested: false)

        fixture.controller.requestPermission()
        fixture.controller.requestPermission()

        XCTAssertEqual(fixture.controller.status, .denied)
        XCTAssertTrue(fixture.history.hasRequestedPermission)
        XCTAssertEqual(fixture.checker.promptValues.filter { $0 }, [true])
    }

    func testRefreshObservesPermissionGrantedInSystemSettings() {
        let fixture = makeController(trusted: false, hasRequested: true)
        XCTAssertEqual(fixture.controller.status, .denied)

        fixture.checker.trusted = true
        fixture.controller.refresh()

        XCTAssertEqual(fixture.controller.status, .granted)
    }

    func testOpenSystemSettingsUsesInjectedRoute() {
        let fixture = makeController(trusted: false, hasRequested: true)

        fixture.controller.openSystemSettings()

        XCTAssertEqual(fixture.settingsOpener.openCount, 1)
    }

    private func makeController(
        trusted: Bool,
        hasRequested: Bool
    ) -> (
        controller: AccessibilityPermissionController,
        checker: FakeTrustChecker,
        history: FakePromptHistory,
        settingsOpener: FakeSettingsOpener
    ) {
        let checker = FakeTrustChecker(trusted: trusted)
        let history = FakePromptHistory(hasRequestedPermission: hasRequested)
        let settingsOpener = FakeSettingsOpener()
        let controller = AccessibilityPermissionController(
            trustChecker: checker,
            promptHistory: history,
            settingsOpener: settingsOpener
        )
        return (controller, checker, history, settingsOpener)
    }
}

private final class FakeTrustChecker: AccessibilityTrustChecking {
    var trusted: Bool
    private(set) var promptValues: [Bool] = []

    init(trusted: Bool) {
        self.trusted = trusted
    }

    func isTrusted(prompt: Bool) -> Bool {
        promptValues.append(prompt)
        return trusted
    }
}

private final class FakePromptHistory: AccessibilityPromptHistory {
    var hasRequestedPermission: Bool

    init(hasRequestedPermission: Bool) {
        self.hasRequestedPermission = hasRequestedPermission
    }

    func markPermissionRequested() {
        hasRequestedPermission = true
    }
}

private final class FakeSettingsOpener: AccessibilitySettingsOpening {
    private(set) var openCount = 0

    func openAccessibilitySettings() {
        openCount += 1
    }
}
