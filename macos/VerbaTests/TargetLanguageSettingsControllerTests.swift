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

        controller.select("fr")

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

        controller.select("fr")

        XCTAssertEqual(controller.selectedIdentifier, "en")
        XCTAssertNotNil(controller.errorMessage)
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
