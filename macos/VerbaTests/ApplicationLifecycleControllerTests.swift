import XCTest
@testable import Verba

@MainActor
final class ApplicationLifecycleControllerTests: XCTestCase {
    func testLaunchAtLoginRemainsDeferredForVersionOne() {
        XCTAssertEqual(ApplicationLifecycleController.launchAtLoginPolicy, .deferred)
    }

    func testSleepWakeScreenAndTerminationEventsReachTheirOwners() {
        let runtime = FakeLifecycleRuntime()
        let popup = FakeLifecyclePopup()
        let accessibility = FakeAccessibilityRefresher()
        let events = FakeLifecycleEventSource()
        let controller = ApplicationLifecycleController(
            runtime: runtime,
            popup: popup,
            accessibilityPermission: accessibility,
            events: events
        )

        events.onSystemWillSleep?()
        events.onSystemDidWake?()
        events.onScreensDidChange?()
        events.onApplicationWillTerminate?()

        XCTAssertNotNil(controller)
        XCTAssertEqual(runtime.prepareForSleepCount, 1)
        XCTAssertEqual(runtime.resumeAfterWakeCount, 1)
        XCTAssertEqual(runtime.shutdownCount, 1)
        XCTAssertEqual(popup.repositionCount, 1)
        XCTAssertEqual(accessibility.refreshCount, 1)
    }

    func testRepeatedActivationEventsAlwaysRefreshPermissionState() {
        let accessibility = FakeAccessibilityRefresher()
        let events = FakeLifecycleEventSource()
        let controller = ApplicationLifecycleController(
            runtime: FakeLifecycleRuntime(),
            popup: FakeLifecyclePopup(),
            accessibilityPermission: accessibility,
            events: events
        )

        events.onApplicationDidBecomeActive?()
        events.onApplicationDidBecomeActive?()
        events.onApplicationDidBecomeActive?()

        XCTAssertNotNil(controller)
        XCTAssertEqual(accessibility.refreshCount, 3)
    }

    func testOpeningTheMenuRefreshesPermissionState() {
        let accessibility = FakeAccessibilityRefresher()
        let events = FakeLifecycleEventSource()
        let controller = ApplicationLifecycleController(
            runtime: FakeLifecycleRuntime(),
            popup: FakeLifecyclePopup(),
            accessibilityPermission: accessibility,
            events: events
        )

        events.onMenuDidBeginTracking?()

        XCTAssertNotNil(controller)
        XCTAssertEqual(accessibility.refreshCount, 1)
    }

    func testSystemEventSourcePublishesMenuTrackingEvents() {
        let events = SystemApplicationLifecycleEventSource()
        var menuTrackingCount = 0
        events.onMenuDidBeginTracking = {
            menuTrackingCount += 1
        }
        events.start()

        NotificationCenter.default.post(
            name: NSMenu.didBeginTrackingNotification,
            object: nil
        )

        XCTAssertEqual(menuTrackingCount, 1)
    }

    func testEventCallbacksDoNotRetainTheControllerOrItsOwners() {
        let events = FakeLifecycleEventSource()
        weak var controller: ApplicationLifecycleController?
        weak var runtime: FakeLifecycleRuntime?

        do {
            let ownedRuntime = FakeLifecycleRuntime()
            let ownedController = ApplicationLifecycleController(
                runtime: ownedRuntime,
                popup: FakeLifecyclePopup(),
                accessibilityPermission: FakeAccessibilityRefresher(),
                events: events
            )
            runtime = ownedRuntime
            controller = ownedController
        }

        XCTAssertNil(controller)
        XCTAssertNil(runtime)
        events.onApplicationWillTerminate?()
    }
}

@MainActor
private final class FakeLifecycleRuntime: ApplicationLifecycleRuntime {
    private(set) var prepareForSleepCount = 0
    private(set) var resumeAfterWakeCount = 0
    private(set) var shutdownCount = 0

    func prepareForSleep() {
        prepareForSleepCount += 1
    }

    func resumeAfterWake() {
        resumeAfterWakeCount += 1
    }

    func shutdown() {
        shutdownCount += 1
    }
}

@MainActor
private final class FakeLifecyclePopup: ApplicationLifecyclePopup {
    private(set) var repositionCount = 0

    func repositionForScreenChange() {
        repositionCount += 1
    }
}

@MainActor
private final class FakeAccessibilityRefresher: AccessibilityPermissionRefreshing {
    private(set) var refreshCount = 0

    func refresh() {
        refreshCount += 1
    }
}

@MainActor
private final class FakeLifecycleEventSource: ApplicationLifecycleEventSource {
    var onApplicationDidBecomeActive: (@MainActor @Sendable () -> Void)?
    var onMenuDidBeginTracking: (@MainActor @Sendable () -> Void)?
    var onApplicationWillTerminate: (@MainActor @Sendable () -> Void)?
    var onScreensDidChange: (@MainActor @Sendable () -> Void)?
    var onSystemWillSleep: (@MainActor @Sendable () -> Void)?
    var onSystemDidWake: (@MainActor @Sendable () -> Void)?

    func start() {}
}
