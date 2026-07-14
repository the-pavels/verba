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
}

private final class FocusOwner {}
