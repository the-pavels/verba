import XCTest
@testable import Verba

@MainActor
final class LaunchAtLoginControllerTests: XCTestCase {
    func testControllerReflectsTheSystemRegistrationState() {
        let service = FakeLaunchAtLoginService(status: .disabled)
        let controller = LaunchAtLoginController(service: service)

        XCTAssertFalse(controller.isRequested)
        XCTAssertTrue(controller.canChange)
        XCTAssertEqual(controller.statusMessage, "Verba opens only when you start it.")

        service.status = .enabled
        controller.refresh()

        XCTAssertTrue(controller.isRequested)
        XCTAssertEqual(
            controller.statusMessage,
            "Verba will open automatically when you log in."
        )
    }

    func testEnablingAndDisablingUseTheSystemService() {
        let service = FakeLaunchAtLoginService(status: .disabled)
        service.statusAfterRegister = .enabled
        service.statusAfterUnregister = .disabled
        let controller = LaunchAtLoginController(service: service)

        controller.setRequested(true)

        XCTAssertEqual(service.registerCount, 1)
        XCTAssertEqual(controller.status, .enabled)

        controller.setRequested(false)

        XCTAssertEqual(service.unregisterCount, 1)
        XCTAssertEqual(controller.status, .disabled)
    }

    func testApprovalStateRemainsRequestedAndOpensSystemSettings() {
        let service = FakeLaunchAtLoginService(status: .requiresApproval)
        let controller = LaunchAtLoginController(service: service)

        XCTAssertTrue(controller.isRequested)
        XCTAssertEqual(
            controller.statusMessage,
            "Approve Verba in System Settings to launch it at login."
        )

        controller.openSystemSettings()

        XCTAssertEqual(service.openSystemSettingsCount, 1)
    }

    func testUnavailableServiceCannotBeChanged() {
        let service = FakeLaunchAtLoginService(status: .unavailable)
        let controller = LaunchAtLoginController(service: service)

        XCTAssertFalse(controller.canChange)
        XCTAssertFalse(controller.isRequested)
        XCTAssertEqual(
            controller.statusMessage,
            "Launch at login is unavailable for this copy of Verba."
        )

        controller.setRequested(true)

        XCTAssertEqual(service.registerCount, 0)
    }

    func testRegistrationFailureKeepsSystemStateAndShowsSafeFeedback() {
        let service = FakeLaunchAtLoginService(status: .disabled)
        service.registerError = TestError.failed
        let controller = LaunchAtLoginController(service: service)

        controller.setRequested(true)

        XCTAssertEqual(controller.status, .disabled)
        XCTAssertEqual(
            controller.feedback,
            "Launch at login couldn’t be changed. Try again."
        )
    }
}

@MainActor
private final class FakeLaunchAtLoginService: LaunchAtLoginServicing {
    var status: LaunchAtLoginStatus
    var statusAfterRegister: LaunchAtLoginStatus?
    var statusAfterUnregister: LaunchAtLoginStatus?
    var registerError: Error?
    var unregisterError: Error?
    private(set) var registerCount = 0
    private(set) var unregisterCount = 0
    private(set) var openSystemSettingsCount = 0

    init(status: LaunchAtLoginStatus) {
        self.status = status
    }

    func register() throws {
        registerCount += 1
        if let registerError {
            throw registerError
        }
        if let statusAfterRegister {
            status = statusAfterRegister
        }
    }

    func unregister() throws {
        unregisterCount += 1
        if let unregisterError {
            throw unregisterError
        }
        if let statusAfterUnregister {
            status = statusAfterUnregister
        }
    }

    func openSystemSettings() {
        openSystemSettingsCount += 1
    }
}

private enum TestError: Error {
    case failed
}
