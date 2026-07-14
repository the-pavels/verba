import XCTest
@testable import Verba

final class MenuBarContentTests: XCTestCase {
    func testNotRequestedPermissionOffersAConciseEnableAction() {
        let presentation = AccessibilityPermissionStatus.notRequested.menuPresentation

        XCTAssertEqual(presentation.title, "Accessibility access required")
        XCTAssertEqual(presentation.systemImage, "hand.raised")
        XCTAssertEqual(
            presentation.message,
            "Verba needs Accessibility access to read selected text in other apps."
        )
        XCTAssertEqual(presentation.action, .requestAccess)
        XCTAssertEqual(presentation.action?.title, "Enable Accessibility…")
    }

    func testDeniedPermissionLinksDirectlyToSystemSettings() {
        let presentation = AccessibilityPermissionStatus.denied.menuPresentation

        XCTAssertEqual(presentation.title, "Accessibility access required")
        XCTAssertEqual(presentation.systemImage, "exclamationmark.triangle")
        XCTAssertEqual(
            presentation.message,
            "Enable Verba in Privacy & Security to read selected text."
        )
        XCTAssertEqual(presentation.action, .openSettings)
        XCTAssertEqual(presentation.action?.title, "Open Accessibility Settings…")
    }

    func testGrantedPermissionShowsOnlyTheReadyStatus() {
        let presentation = AccessibilityPermissionStatus.granted.menuPresentation

        XCTAssertEqual(
            presentation,
            MenuBarPermissionPresentation(
                title: "Verba is ready",
                systemImage: "checkmark.circle",
                message: nil,
                action: nil
            )
        )
    }
}
