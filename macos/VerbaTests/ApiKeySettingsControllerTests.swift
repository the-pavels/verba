import XCTest
@testable import Verba

@MainActor
final class ApiKeySettingsControllerTests: XCTestCase {
    func testLoadsOnlyTheMaskedConfigurationState() async {
        let settings = FakeApiKeySettings(isConfigured: true)
        let controller = ApiKeySettingsController(settings: settings)

        await controller.load()

        XCTAssertTrue(controller.isConfigured)
        XCTAssertEqual(controller.apiKeyInput, "")
        XCTAssertNil(controller.feedback)
    }

    func testSavesAndReplacesWithoutKeepingTheKeyInViewState() {
        let settings = FakeApiKeySettings(isConfigured: false)
        let controller = ApiKeySettingsController(settings: settings)
        controller.apiKeyInput = "test-key"

        controller.save()

        XCTAssertEqual(settings.savedKeys, ["test-key"])
        XCTAssertEqual(controller.apiKeyInput, "")
        XCTAssertTrue(controller.isConfigured)
        XCTAssertEqual(controller.feedback?.kind, .success)

        controller.apiKeyInput = "replacement-key"
        controller.save()
        XCTAssertEqual(settings.savedKeys, ["test-key", "replacement-key"])
        XCTAssertEqual(controller.apiKeyInput, "")
    }

    func testBlankKeyIsRejectedBeforeStorage() {
        let settings = FakeApiKeySettings(isConfigured: false)
        let controller = ApiKeySettingsController(settings: settings)
        controller.apiKeyInput = " \n "

        controller.save()

        XCTAssertTrue(settings.savedKeys.isEmpty)
        XCTAssertEqual(controller.feedback?.kind, .error)
    }

    func testDeletesTheConfiguredKeyAndClearsInput() async {
        let settings = FakeApiKeySettings(isConfigured: true)
        let controller = ApiKeySettingsController(settings: settings)
        await controller.load()
        controller.apiKeyInput = "unsaved-value"

        controller.delete()

        XCTAssertEqual(settings.deleteCount, 1)
        XCTAssertFalse(controller.isConfigured)
        XCTAssertEqual(controller.apiKeyInput, "")
    }

    func testConnectionSuccessAndProviderFailuresHaveActionableFeedback() async {
        let successSettings = FakeApiKeySettings(isConfigured: true)
        let successController = ApiKeySettingsController(settings: successSettings)
        await successController.load()

        await successController.testConnection()

        XCTAssertEqual(successSettings.testCount, 1)
        XCTAssertEqual(
            successController.feedback,
            ApiKeySettingsFeedback(
                kind: .success,
                message: "Connection to OpenAI succeeded."
            )
        )

        let rejectedSettings = FakeApiKeySettings(
            isConfigured: true,
            testError: ApiKeySettingsFailure.authentication
        )
        let rejectedController = ApiKeySettingsController(settings: rejectedSettings)
        await rejectedController.load()
        await rejectedController.testConnection()

        XCTAssertEqual(rejectedController.feedback?.kind, .error)
        XCTAssertEqual(
            rejectedController.feedback?.message,
            "OpenAI rejected this key. Replace it and try again."
        )
    }
}

@MainActor
private final class FakeApiKeySettings: ApiKeySettingsManaging {
    let configured: Bool
    let testError: (any Error)?
    private(set) var savedKeys: [String] = []
    private(set) var deleteCount = 0
    private(set) var testCount = 0

    init(isConfigured: Bool, testError: (any Error)? = nil) {
        configured = isConfigured
        self.testError = testError
    }

    func isApiKeyConfigured() throws -> Bool {
        configured
    }

    func saveApiKey(_ apiKey: String) throws {
        savedKeys.append(apiKey)
    }

    func deleteApiKey() throws {
        deleteCount += 1
    }

    func testApiKeyConnection() async throws {
        testCount += 1
        if let testError {
            throw testError
        }
    }
}
