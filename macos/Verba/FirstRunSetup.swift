import AppKit
import SwiftUI

enum FirstRunSetupStep: Int, CaseIterable, Equatable {
    case welcome
    case accessibility
    case essentials
    case proofreading
    case ready

    var next: Self? {
        Self(rawValue: rawValue + 1)
    }

    var previous: Self? {
        Self(rawValue: rawValue - 1)
    }
}

protocol FirstRunSetupPersisting: AnyObject {
    var isCompleted: Bool { get }
    func markCompleted()
}

final class UserDefaultsFirstRunSetupStore: FirstRunSetupPersisting {
    private static let completionKey = "firstRunSetupCompleted.v1"

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    var isCompleted: Bool {
        defaults.bool(forKey: Self.completionKey)
    }

    func markCompleted() {
        defaults.set(true, forKey: Self.completionKey)
    }
}

enum FirstRunSetupLaunchPolicy {
    static func shouldPresent(isCompleted: Bool, isRunningTests: Bool) -> Bool {
        !isCompleted && !isRunningTests
    }
}

@MainActor
final class FirstRunSetupModel: ObservableObject {
    @Published private(set) var step: FirstRunSetupStep = .welcome

    private let store: any FirstRunSetupPersisting

    init(store: any FirstRunSetupPersisting) {
        self.store = store
    }

    var isCompleted: Bool {
        store.isCompleted
    }

    func advance() {
        guard let next = step.next else {
            return
        }
        step = next
    }

    func goBack() {
        guard let previous = step.previous else {
            return
        }
        step = previous
    }

    func complete() {
        store.markCompleted()
    }
}

@MainActor
final class FirstRunSetupWindowController: NSObject, NSWindowDelegate {
    static let contentSize = NSSize(width: 520, height: 430)

    private let model: FirstRunSetupModel
    private let accessibility: AccessibilityPermissionController
    private let targetLanguage: TargetLanguageSettingsController
    private let apiKey: ApiKeySettingsController
    private let shortcuts: ShortcutSettingsController
    private var windowController: NSWindowController?

    init(
        accessibility: AccessibilityPermissionController,
        targetLanguage: TargetLanguageSettingsController,
        apiKey: ApiKeySettingsController,
        shortcuts: ShortcutSettingsController,
        store: any FirstRunSetupPersisting = UserDefaultsFirstRunSetupStore()
    ) {
        model = FirstRunSetupModel(store: store)
        self.accessibility = accessibility
        self.targetLanguage = targetLanguage
        self.apiKey = apiKey
        self.shortcuts = shortcuts
        super.init()
    }

    func presentIfNeeded(
        isRunningTests: Bool = ProcessInfo.processInfo.environment[
            "XCTestConfigurationFilePath"
        ] != nil
    ) {
        guard FirstRunSetupLaunchPolicy.shouldPresent(
            isCompleted: model.isCompleted,
            isRunningTests: isRunningTests
        ) else {
            return
        }

        if let window = windowController?.window {
            NSApplication.shared.activate()
            window.makeKeyAndOrderFront(nil)
            return
        }

        let setupView = FirstRunSetupView(
            model: model,
            accessibility: accessibility,
            targetLanguage: targetLanguage,
            apiKey: apiKey,
            shortcuts: shortcuts,
            finish: { [weak self] in
                self?.completeAndClose()
            },
            setUpLater: { [weak self] in
                self?.completeAndClose()
            }
        )
        let hostingController = NSHostingController(rootView: setupView)
        let window = NSWindow(contentViewController: hostingController)
        window.contentMinSize = Self.contentSize
        window.contentMaxSize = Self.contentSize
        window.setContentSize(Self.contentSize)
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.styleMask = [.titled, .closable]
        window.title = LocalizedCopy.text("Set Up Verba")
        window.center()

        let windowController = NSWindowController(window: window)
        self.windowController = windowController
        NSApplication.shared.activate()
        windowController.showWindow(nil)
    }

    func windowWillClose(_ notification: Notification) {
        model.complete()
        windowController = nil
    }

    private func completeAndClose() {
        guard let windowController else {
            model.complete()
            return
        }
        windowController.close()
    }
}

private struct FirstRunSetupView: View {
    @ObservedObject var model: FirstRunSetupModel
    @ObservedObject var accessibility: AccessibilityPermissionController
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController

    let finish: () -> Void
    let setUpLater: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            progressHeader

            Divider()
                .padding(.vertical, 18)

            page
                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)

            Divider()
                .padding(.vertical, 16)

            controls
        }
        .padding(24)
        .frame(
            width: FirstRunSetupWindowController.contentSize.width,
            height: FirstRunSetupWindowController.contentSize.height
        )
        .task {
            accessibility.refresh()
            shortcuts.load()
            await targetLanguage.load()
            await apiKey.load()
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in
            accessibility.refresh()
        }
    }

    private var progressHeader: some View {
        VStack(alignment: .leading, spacing: 7) {
            HStack {
                Text("Set Up Verba")
                    .font(.headline)

                Spacer()

                Text("Step \(model.step.rawValue + 1) of \(FirstRunSetupStep.allCases.count)")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            ProgressView(
                value: Double(model.step.rawValue + 1),
                total: Double(FirstRunSetupStep.allCases.count)
            )
            .accessibilityLabel(LocalizedCopy.text("Setup progress"))
        }
    }

    private var page: AnyView {
        switch model.step {
        case .welcome:
            AnyView(FirstRunWelcomePage())
        case .accessibility:
            AnyView(FirstRunAccessibilityPage(accessibility: accessibility))
        case .essentials:
            AnyView(
                FirstRunEssentialsPage(
                    targetLanguage: targetLanguage,
                    shortcuts: shortcuts
                )
            )
        case .proofreading:
            AnyView(FirstRunProofreadingPage(apiKey: apiKey))
        case .ready:
            AnyView(FirstRunReadyPage(apiKey: apiKey, shortcuts: shortcuts))
        }
    }

    private var controls: some View {
        HStack {
            Button("Set Up Later", action: setUpLater)

            Spacer()

            Button("Back") {
                model.goBack()
            }
            .disabled(model.step.previous == nil)

            Button(primaryButtonTitle) {
                if model.step == .ready {
                    finish()
                } else {
                    model.advance()
                }
            }
            .buttonStyle(.borderedProminent)
            .keyboardShortcut(.defaultAction)
            .disabled(!canContinue)
        }
    }

    private var canContinue: Bool {
        model.step != .accessibility || accessibility.status == .granted
    }

    private var primaryButtonTitle: String {
        model.step == .ready
            ? LocalizedCopy.text("Start Using Verba")
            : LocalizedCopy.text("Continue")
    }

}
