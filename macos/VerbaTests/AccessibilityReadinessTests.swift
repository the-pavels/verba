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

    func testAccessibilityCopyDescribesProgressSettingsAndResults() {
        XCTAssertEqual(
            AccessibilityCopy.setupProgress(current: 2, total: 5),
            "Step 2 of 5"
        )
        XCTAssertEqual(
            AccessibilityCopy.setting(title: "Translate", value: "Control T"),
            "Translate: Control T"
        )
        XCTAssertEqual(AccessibilityCopy.originalText("Hello"), "Original text: Hello")
        XCTAssertEqual(
            AccessibilityCopy.translationText("Bonjour"),
            "Translation text: Bonjour"
        )
        XCTAssertEqual(
            AccessibilityCopy.correctedText("Hello."),
            "Corrected text: Hello."
        )
        XCTAssertEqual(
            AccessibilityCopy.apiKeyStatus(isConfigured: true),
            "API key stored in Keychain"
        )
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

    func testResultPopupHeightTracksContentWithinBounds() {
        let languagePair = LanguagePairViewModel(source: "German", target: "English")
        let short = PopupSizePolicy.size(
            for: .translation(
                originalText: "Guten Morgen.",
                languagePair: languagePair,
                translatedText: "Good morning."
            ),
            textScale: 1
        )
        let medium = PopupSizePolicy.size(
            for: .translation(
                originalText: String(repeating: "Ausgangstext ", count: 16),
                languagePair: languagePair,
                translatedText: String(repeating: "Translated text ", count: 16)
            ),
            textScale: 1
        )
        let long = PopupSizePolicy.size(
            for: .translation(
                originalText: String(repeating: "Ausgangstext ", count: 100),
                languagePair: languagePair,
                translatedText: String(repeating: "Translated text ", count: 100)
            ),
            textScale: 1
        )

        XCTAssertEqual(short, NSSize(width: 420, height: 250))
        XCTAssertGreaterThan(medium.height, short.height)
        XCTAssertLessThan(medium.height, long.height)
        XCTAssertEqual(long, NSSize(width: 420, height: 480))
    }

    func testResultPopupRespectsAbsoluteHeightCapAtLargeTextSizes() {
        let presentation = PresentationViewModel.proofreading(
            originalText: String(repeating: "Original text ", count: 100),
            correctedText: String(repeating: "Corrected text ", count: 100)
        )

        XCTAssertEqual(
            PopupSizePolicy.size(for: presentation, textScale: 4),
            NSSize(width: 630, height: 560)
        )
    }

    func testExplicitLineBreaksContributeToPopupHeight() {
        let languagePair = LanguagePairViewModel(source: "German", target: "English")
        let singleLine = PresentationViewModel.translation(
            originalText: "One line",
            languagePair: languagePair,
            translatedText: "One line"
        )
        let multipleLines = PresentationViewModel.translation(
            originalText: "One\nTwo\nThree",
            languagePair: languagePair,
            translatedText: "One\nTwo\nThree"
        )

        XCTAssertGreaterThan(
            PopupSizePolicy.size(for: multipleLines, textScale: 1).height,
            PopupSizePolicy.size(for: singleLine, textScale: 1).height
        )
    }

    func testErrorPopupGrowsForLongRecoveryCopyAndStopsAtItsBound() {
        let short = PresentationViewModel.error(
            action: .translate,
            title: "Translation failed",
            message: "Try again.",
            recovery: .retry,
            diagnosticCode: "translation.failed"
        )
        let long = PresentationViewModel.error(
            action: .translate,
            title: String(repeating: "Translation failed ", count: 30),
            message: String(repeating: "Check the connection and try again. ", count: 50),
            recovery: .retry,
            diagnosticCode: "translation.failed"
        )

        XCTAssertEqual(
            PopupSizePolicy.size(for: short, textScale: 1),
            NSSize(width: 380, height: 170)
        )
        XCTAssertEqual(
            PopupSizePolicy.size(for: long, textScale: 1),
            NSSize(width: 380, height: 280)
        )
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

    func testLoadingPopupWaitsForCaptureCompletionWithoutATimer() {
        var state = PopupCaptureFocusState()

        XCTAssertEqual(
            state.receive(requestID: 1, isLoading: true),
            PopupCaptureFocusDecision(
                beginsCapture: true,
                shouldTakeKeyboardFocus: false
            )
        )
        XCTAssertEqual(
            state.receive(requestID: 1, isLoading: true),
            PopupCaptureFocusDecision(
                beginsCapture: false,
                shouldTakeKeyboardFocus: false
            )
        )
        XCTAssertTrue(state.captureDidComplete(requestID: 1))
        XCTAssertEqual(
            state.receive(requestID: 1, isLoading: true),
            PopupCaptureFocusDecision(
                beginsCapture: false,
                shouldTakeKeyboardFocus: true
            )
        )
    }

    func testCaptureFailureMakesTheErrorInteractiveAndIgnoresLateCompletion() {
        var state = PopupCaptureFocusState()
        _ = state.receive(requestID: 1, isLoading: true)

        XCTAssertEqual(
            state.receive(requestID: 1, isLoading: false),
            PopupCaptureFocusDecision(
                beginsCapture: false,
                shouldTakeKeyboardFocus: true
            )
        )
        XCTAssertFalse(state.captureDidComplete(requestID: 1))
    }

    func testCaptureCompletionCanArriveBeforeTheLoadingPresentation() {
        var state = PopupCaptureFocusState()

        XCTAssertFalse(state.captureDidComplete(requestID: 1))
        XCTAssertEqual(
            state.receive(requestID: 1, isLoading: true),
            PopupCaptureFocusDecision(
                beginsCapture: false,
                shouldTakeKeyboardFocus: true
            )
        )
    }

    func testEarlyCompletionFromANewerRequestRejectsOlderQueuedPresentation() {
        var state = PopupCaptureFocusState()

        XCTAssertFalse(state.captureDidComplete(requestID: 2))
        XCTAssertNil(state.receive(requestID: 1, isLoading: true))
        XCTAssertEqual(
            state.receive(requestID: 2, isLoading: true),
            PopupCaptureFocusDecision(
                beginsCapture: false,
                shouldTakeKeyboardFocus: true
            )
        )
    }

    func testNewCaptureInvalidatesCompletionFromThePreviousRequest() {
        var state = PopupCaptureFocusState()
        _ = state.receive(requestID: 1, isLoading: true)
        _ = state.receive(requestID: 2, isLoading: true)

        XCTAssertFalse(state.captureDidComplete(requestID: 1))
        XCTAssertTrue(state.captureDidComplete(requestID: 2))
        XCTAssertNil(state.receive(requestID: 1, isLoading: false))
    }

    func testDismissedCaptureCannotRegainKeyboardFocus() {
        var state = PopupCaptureFocusState()
        _ = state.receive(requestID: 1, isLoading: true)

        state.dismiss()

        XCTAssertFalse(state.captureDidComplete(requestID: 1))
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

    func testOnlyExplicitKeyWindowDismissalRestoresSourceFocus() {
        XCTAssertTrue(
            PopupFocusDisposition.restoreSource.shouldRestoreSource(panelWasKey: true)
        )
        XCTAssertFalse(
            PopupFocusDisposition.restoreSource.shouldRestoreSource(panelWasKey: false)
        )
        XCTAssertFalse(
            PopupFocusDisposition.preserveCurrent.shouldRestoreSource(panelWasKey: true)
        )
    }

    func testSourceApplicationCaptureExcludesTheCurrentProcess() {
        XCTAssertFalse(
            PopupSourceApplicationPolicy.shouldCapture(
                candidateProcessIdentifier: nil,
                currentProcessIdentifier: 42
            )
        )
        XCTAssertFalse(
            PopupSourceApplicationPolicy.shouldCapture(
                candidateProcessIdentifier: 42,
                currentProcessIdentifier: 42
            )
        )
        XCTAssertTrue(
            PopupSourceApplicationPolicy.shouldCapture(
                candidateProcessIdentifier: 7,
                currentProcessIdentifier: 42
            )
        )
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
