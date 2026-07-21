import Foundation
import XCTest
@testable import Verba

@MainActor
final class TargetLanguageSettingsControllerTests: XCTestCase {
    func testLoadsOnlySupportedLanguagesAndUsesTheRustSelection() async {
        let preferences = FakeTargetLanguagePreferences(selected: "en")
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(
                identifiers: ["de", "en", "de"]
            ),
            locale: Locale(identifier: "en")
        )

        await controller.load()
        await controller.load()

        XCTAssertEqual(controller.options.map(\.id), ["en", "de"])
        XCTAssertEqual(controller.options.map(\.name), ["English", "German"])
        XCTAssertEqual(controller.selectedIdentifier, "en")
        XCTAssertEqual(preferences.configuredIdentifiers, [["en", "de"]])
        XCTAssertNil(controller.errorMessage)
    }

    func testSelectionUpdatesTheRunningRustPreference() async {
        let preferences = FakeTargetLanguagePreferences(selected: "en")
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(identifiers: ["en", "fr"]),
            locale: Locale(identifier: "en")
        )
        await controller.load()

        XCTAssertTrue(controller.select("fr"))

        XCTAssertEqual(controller.selectedIdentifier, "fr")
        XCTAssertEqual(preferences.selections, ["fr"])
    }

    func testFailedSelectionKeepsTheCurrentValue() async {
        let preferences = FakeTargetLanguagePreferences(
            selected: "en",
            selectionError: TestFailure()
        )
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(identifiers: ["en", "fr"]),
            locale: Locale(identifier: "en")
        )
        await controller.load()

        XCTAssertFalse(controller.select("fr"))

        XCTAssertEqual(controller.selectedIdentifier, "en")
        XCTAssertNotNil(controller.errorMessage)
    }

    func testPopupSelectionPersistsAndRequestsARetranslation() async {
        let preferences = FakeTargetLanguagePreferences(selected: "en")
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(identifiers: ["en", "fr"]),
            locale: Locale(identifier: "en")
        )
        await controller.load()
        let popup = PopupController(
            translationSessions: SystemTranslationSessionProvider()
        )
        popup.targetLanguageSettings = controller
        var retranslatedTexts: [String] = []
        popup.onTargetLanguageChanged = { retranslatedTexts.append($0) }

        popup.selectTargetLanguage("fr", originalText: "Bonjour")

        XCTAssertEqual(controller.selectedIdentifier, "fr")
        XCTAssertEqual(preferences.selections, ["fr"])
        XCTAssertEqual(retranslatedTexts, ["Bonjour"])
    }

    func testPopupSelectionIgnoresTheCurrentAndUnknownLanguages() async {
        let preferences = FakeTargetLanguagePreferences(selected: "en")
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(identifiers: ["en", "fr"]),
            locale: Locale(identifier: "en")
        )
        await controller.load()
        let popup = PopupController(
            translationSessions: SystemTranslationSessionProvider()
        )
        popup.targetLanguageSettings = controller
        var retranslations = 0
        popup.onTargetLanguageChanged = { _ in retranslations += 1 }

        popup.selectTargetLanguage("en", originalText: "Hello")
        popup.selectTargetLanguage("xx", originalText: "Hello")

        XCTAssertEqual(controller.selectedIdentifier, "en")
        XCTAssertEqual(preferences.selections, [])
        XCTAssertEqual(retranslations, 0)
    }

    func testPopupSelectionDoesNotRetranslateWhenPersistenceFails() async {
        let preferences = FakeTargetLanguagePreferences(
            selected: "en",
            selectionError: TestFailure()
        )
        let controller = TargetLanguageSettingsController(
            preferences: preferences,
            languages: FakeSupportedLanguages(identifiers: ["en", "fr"]),
            locale: Locale(identifier: "en")
        )
        await controller.load()
        let popup = PopupController(
            translationSessions: SystemTranslationSessionProvider()
        )
        popup.targetLanguageSettings = controller
        var retranslations = 0
        popup.onTargetLanguageChanged = { _ in retranslations += 1 }

        popup.selectTargetLanguage("fr", originalText: "Bonjour")

        XCTAssertEqual(controller.selectedIdentifier, "en")
        XCTAssertEqual(retranslations, 0)
    }
}

@MainActor
private final class FakeTargetLanguagePreferences: TargetLanguagePreferenceManaging {
    let selected: String
    let selectionError: (any Error)?
    private(set) var configuredIdentifiers: [[String]] = []
    private(set) var selections: [String] = []

    init(selected: String, selectionError: (any Error)? = nil) {
        self.selected = selected
        self.selectionError = selectionError
    }

    func configureSupportedTargetLanguages(_ identifiers: [String]) throws -> String {
        configuredIdentifiers.append(identifiers)
        return selected
    }

    func setTargetLanguage(_ identifier: String) throws {
        if let selectionError {
            throw selectionError
        }
        selections.append(identifier)
    }
}

@MainActor
private struct FakeSupportedLanguages: SupportedTranslationLanguagesProviding {
    let identifiers: [String]

    func supportedLanguages() async -> [Locale.Language] {
        identifiers.map(Locale.Language.init(identifier:))
    }
}

private struct TestFailure: Error {}
