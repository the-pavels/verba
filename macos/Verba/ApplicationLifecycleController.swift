import AppKit

@MainActor
protocol ApplicationLifecycleRuntime: AnyObject {
    func prepareForSleep()
    func resumeAfterWake()
    func shutdown()
}

@MainActor
protocol ApplicationLifecyclePopup: AnyObject {
    func repositionForScreenChange()
}

@MainActor
protocol AccessibilityPermissionRefreshing: AnyObject {
    func refresh()
}

@MainActor
protocol ApplicationLifecycleEventSource: AnyObject {
    var onApplicationDidBecomeActive: (@MainActor @Sendable () -> Void)? { get set }
    var onMenuDidBeginTracking: (@MainActor @Sendable () -> Void)? { get set }
    var onApplicationWillTerminate: (@MainActor @Sendable () -> Void)? { get set }
    var onScreensDidChange: (@MainActor @Sendable () -> Void)? { get set }
    var onSystemWillSleep: (@MainActor @Sendable () -> Void)? { get set }
    var onSystemDidWake: (@MainActor @Sendable () -> Void)? { get set }

    func start()
}

@MainActor
final class ApplicationLifecycleController {
    private let runtime: any ApplicationLifecycleRuntime
    private let popup: any ApplicationLifecyclePopup
    private let accessibilityPermission: any AccessibilityPermissionRefreshing
    private let events: any ApplicationLifecycleEventSource

    init(
        runtime: any ApplicationLifecycleRuntime,
        popup: any ApplicationLifecyclePopup,
        accessibilityPermission: any AccessibilityPermissionRefreshing,
        events: any ApplicationLifecycleEventSource = SystemApplicationLifecycleEventSource()
    ) {
        self.runtime = runtime
        self.popup = popup
        self.accessibilityPermission = accessibilityPermission
        self.events = events

        events.onApplicationDidBecomeActive = { [weak self] in
            self?.accessibilityPermission.refresh()
        }
        events.onMenuDidBeginTracking = { [weak self] in
            self?.accessibilityPermission.refresh()
        }
        events.onApplicationWillTerminate = { [weak self] in
            self?.runtime.shutdown()
        }
        events.onScreensDidChange = { [weak self] in
            self?.popup.repositionForScreenChange()
        }
        events.onSystemWillSleep = { [weak self] in
            self?.runtime.prepareForSleep()
        }
        events.onSystemDidWake = { [weak self] in
            self?.runtime.resumeAfterWake()
            self?.accessibilityPermission.refresh()
        }
        events.start()
    }
}

@MainActor
final class SystemApplicationLifecycleEventSource: ApplicationLifecycleEventSource {
    var onApplicationDidBecomeActive: (@MainActor @Sendable () -> Void)?
    var onMenuDidBeginTracking: (@MainActor @Sendable () -> Void)?
    var onApplicationWillTerminate: (@MainActor @Sendable () -> Void)?
    var onScreensDidChange: (@MainActor @Sendable () -> Void)?
    var onSystemWillSleep: (@MainActor @Sendable () -> Void)?
    var onSystemDidWake: (@MainActor @Sendable () -> Void)?

    private var observations: [NotificationObservation] = []

    func start() {
        guard observations.isEmpty else {
            return
        }

        observe(
            center: .default,
            name: NSApplication.didBecomeActiveNotification,
            handler: { [weak self] in self?.onApplicationDidBecomeActive?() }
        )
        observe(
            center: .default,
            name: NSMenu.didBeginTrackingNotification,
            handler: { [weak self] in self?.onMenuDidBeginTracking?() }
        )
        observe(
            center: .default,
            name: NSApplication.willTerminateNotification,
            handler: { [weak self] in self?.onApplicationWillTerminate?() }
        )
        observe(
            center: .default,
            name: NSApplication.didChangeScreenParametersNotification,
            handler: { [weak self] in self?.onScreensDidChange?() }
        )
        observe(
            center: NSWorkspace.shared.notificationCenter,
            name: NSWorkspace.willSleepNotification,
            handler: { [weak self] in self?.onSystemWillSleep?() }
        )
        observe(
            center: NSWorkspace.shared.notificationCenter,
            name: NSWorkspace.didWakeNotification,
            handler: { [weak self] in self?.onSystemDidWake?() }
        )
    }

    private func observe(
        center: NotificationCenter,
        name: Notification.Name,
        handler: @MainActor @escaping @Sendable () -> Void
    ) {
        let observation = center.addObserver(forName: name, object: nil, queue: .main) { _ in
            MainActor.assumeIsolated {
                handler()
            }
        }
        observations.append(NotificationObservation(center: center, token: observation))
    }
}

private final class NotificationObservation: @unchecked Sendable {
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
