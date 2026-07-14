import XCTest
@testable import Verba

@MainActor
final class ShortcutSettingsControllerTests: XCTestCase {
    func testLoadsAndUpdatesTheDisplayedShortcuts() {
        let settings = FakeShortcutSettings()
        let controller = ShortcutSettingsController(settings: settings)

        controller.load()
        controller.record(sampleShortcut, for: .translate)

        XCTAssertEqual(controller.translate, "⌃⌥L")
        XCTAssertEqual(controller.proofread, "⌃⌥P")
        XCTAssertEqual(settings.recordedActions, [.translate])
        XCTAssertNil(controller.errorMessage)
    }

    func testRejectedShortcutKeepsThePreviousDisplay() {
        let settings = FakeShortcutSettings(failure: .duplicateShortcut)
        let controller = ShortcutSettingsController(settings: settings)
        controller.load()

        controller.record(sampleShortcut, for: .proofread)

        XCTAssertEqual(controller.translate, "⌃⌥T")
        XCTAssertEqual(controller.proofread, "⌃⌥P")
        XCTAssertEqual(
            controller.errorMessage,
            "That shortcut is already assigned to another Verba action."
        )
    }

    func testRegistrationFailureExplainsThatThePreviousShortcutRemainsActive() {
        let settings = FakeShortcutSettings(failure: .shortcutUnavailable)
        let controller = ShortcutSettingsController(settings: settings)
        controller.load()

        controller.record(sampleShortcut, for: .translate)

        XCTAssertTrue(controller.errorMessage?.contains("previous shortcut is still active") == true)
        XCTAssertEqual(controller.translate, "⌃⌥T")
    }

    private var sampleShortcut: RecordedShortcut {
        RecordedShortcut(
            key: "L",
            command: false,
            control: true,
            option: true,
            shift: false
        )
    }
}

@MainActor
private final class FakeShortcutSettings: ShortcutSettingsManaging {
    private let failure: ShortcutSettingsFailure?
    private(set) var recordedActions: [ShortcutPreferenceAction] = []

    init(failure: ShortcutSettingsFailure? = nil) {
        self.failure = failure
    }

    func shortcutConfiguration() throws -> ShortcutDisplayConfiguration {
        ShortcutDisplayConfiguration(translate: "⌃⌥T", proofread: "⌃⌥P")
    }

    func setShortcut(
        _ shortcut: RecordedShortcut,
        for action: ShortcutPreferenceAction
    ) throws -> ShortcutDisplayConfiguration {
        if let failure {
            throw failure
        }
        recordedActions.append(action)
        return ShortcutDisplayConfiguration(translate: "⌃⌥L", proofread: "⌃⌥P")
    }
}
