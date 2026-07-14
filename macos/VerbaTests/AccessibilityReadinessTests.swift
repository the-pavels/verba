import AppKit
import XCTest
@testable import Verba

@MainActor
final class AccessibilityReadinessTests: XCTestCase {
    func testEnglishLocalizationResourceIsBundled() {
        XCTAssertNotNil(
            Bundle.main.url(
                forResource: "Localizable",
                withExtension: "strings",
                subdirectory: nil,
                localization: "en"
            )
        )
        XCTAssertEqual(LocalizedCopy.text("Retry"), "Retry")
    }

    func testRustErrorCopyIsLocalizedWithoutChangingItsRecoveryContext() {
        let presentation = PresentationViewModel.error(
            action: .translate,
            title: "Translation failed",
            message: "Try translating the selection again.",
            recovery: .retry,
            diagnosticCode: "translation.failed"
        ).localizedForDisplay

        XCTAssertEqual(
            presentation,
            .error(
                action: .translate,
                title: "Translation failed",
                message: "Try translating the selection again.",
                recovery: .retry,
                diagnosticCode: "translation.failed"
            )
        )
    }

    func testPopupSizingScalesTextWithoutUnboundedGrowth() {
        let regular = PopupSizePolicy.size(for: .proofreadingDisclosure, textScale: 1)
        let large = PopupSizePolicy.size(for: .proofreadingDisclosure, textScale: 1.3)
        let capped = PopupSizePolicy.size(for: .proofreadingDisclosure, textScale: 4)

        XCTAssertEqual(regular, NSSize(width: 420, height: 190))
        XCTAssertEqual(large, NSSize(width: 546, height: 247))
        XCTAssertEqual(capped, NSSize(width: 630, height: 285))
    }

    func testReducedMotionDisablesPanelAnimation() {
        XCTAssertEqual(PopupAnimationPolicy.behavior(reduceMotion: true), .none)
        XCTAssertEqual(PopupAnimationPolicy.behavior(reduceMotion: false), .utilityWindow)
    }

    func testHostingControllerDoesNotOverrideThePopupSize() {
        XCTAssertTrue(PopupHostingSizingPolicy.options.isEmpty)
    }

    func testPanelConstrainsEveryPresentationToItsPolicySize() {
        let panel = PopupPanel(contentSize: NSSize(width: 380, height: 112))
        let disclosureSize = NSSize(width: 420, height: 190)

        panel.setFixedContentSize(disclosureSize)

        XCTAssertEqual(panel.contentMinSize, disclosureSize)
        XCTAssertEqual(panel.contentMaxSize, disclosureSize)
        XCTAssertEqual(panel.contentView?.frame.size, disclosureSize)
    }

    func testPopupKeyboardCommandsRecognizeEscapeAndCommandC() throws {
        let escape = try XCTUnwrap(keyEvent(keyCode: 53, characters: "\u{1b}"))
        let copy = try XCTUnwrap(
            keyEvent(keyCode: 8, characters: "c", modifiers: .command)
        )
        let plainC = try XCTUnwrap(keyEvent(keyCode: 8, characters: "c"))

        XCTAssertEqual(PopupKeyboardCommand.command(for: escape), .dismiss)
        XCTAssertEqual(PopupKeyboardCommand.command(for: copy), .copy)
        XCTAssertNil(PopupKeyboardCommand.command(for: plainC))
    }

    func testLoadingPopupPreservesSourceFocusThroughCaptureWindow() {
        let loadingDelay = PopupKeyboardFocusPolicy.delay(
            for: .loading(action: .translate)
        )
        let errorDelay = PopupKeyboardFocusPolicy.delay(
            for: .error(
                action: .translate,
                title: "Selection timed out",
                message: "Try again.",
                recovery: .retry,
                diagnosticCode: "capture.timed-out"
            )
        )

        XCTAssertGreaterThan(loadingDelay, 0.5)
        XCTAssertEqual(errorDelay, 0)
    }

    func testClickAwayDismissesOnlyOutsideThePopupFrame() {
        let popupFrame = NSRect(x: 100, y: 200, width: 420, height: 300)

        XCTAssertFalse(
            PopupClickAwayPolicy.shouldDismiss(
                clickLocation: NSPoint(x: 200, y: 300),
                popupFrame: popupFrame
            )
        )
        XCTAssertTrue(
            PopupClickAwayPolicy.shouldDismiss(
                clickLocation: NSPoint(x: 99, y: 300),
                popupFrame: popupFrame
            )
        )
    }

    func testFocusRestorationIsWeakAndOneShot() {
        let restorer = PopupFocusRestorer<FocusOwner>()
        let popup = FocusOwner()
        var previous: FocusOwner? = FocusOwner()
        weak var weakPrevious: FocusOwner?
        weakPrevious = previous

        restorer.capture(previous, excluding: popup)
        XCTAssertTrue(restorer.take() === previous)
        XCTAssertNil(restorer.take())

        restorer.capture(previous, excluding: popup)
        previous = nil
        XCTAssertNil(weakPrevious)
        XCTAssertNil(restorer.take())
    }

    private func keyEvent(
        keyCode: UInt16,
        characters: String,
        modifiers: NSEvent.ModifierFlags = []
    ) -> NSEvent? {
        NSEvent.keyEvent(
            with: .keyDown,
            location: .zero,
            modifierFlags: modifiers,
            timestamp: 0,
            windowNumber: 0,
            context: nil,
            characters: characters,
            charactersIgnoringModifiers: characters,
            isARepeat: false,
            keyCode: keyCode
        )
    }
}

private final class FocusOwner {}
