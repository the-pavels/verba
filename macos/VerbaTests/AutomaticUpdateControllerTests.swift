import XCTest
@testable import Verba

@MainActor
final class AutomaticUpdateControllerTests: XCTestCase {
    func testConfigurationRequiresHTTPSAndA32BytePublicKey() {
        let key = Data(repeating: 7, count: 32).base64EncodedString()

        XCTAssertNotNil(
            AutomaticUpdateConfiguration(
                feedURLString: "https://example.com/appcast.xml",
                publicKey: key
            )
        )
        XCTAssertNil(
            AutomaticUpdateConfiguration(
                feedURLString: "http://example.com/appcast.xml",
                publicKey: key
            )
        )
        XCTAssertNil(
            AutomaticUpdateConfiguration(
                feedURLString: "https://example.com/appcast.xml",
                publicKey: "invalid"
            )
        )
    }

    func testControllerReflectsAndChangesAutomaticCheckPreference() {
        let engine = FakeAutomaticUpdateEngine()
        engine.canCheckForUpdates = true
        let controller = AutomaticUpdateController(engine: engine)

        XCTAssertTrue(controller.isAvailable)
        XCTAssertTrue(controller.canCheckForUpdates)
        XCTAssertFalse(controller.automaticallyChecksForUpdates)

        controller.setAutomaticallyChecksForUpdates(true)

        XCTAssertTrue(engine.automaticallyChecksForUpdates)
        XCTAssertTrue(controller.automaticallyChecksForUpdates)
    }

    func testManualCheckRunsOnlyWhenSparkleIsReady() {
        let engine = FakeAutomaticUpdateEngine()
        let controller = AutomaticUpdateController(engine: engine)

        controller.checkForUpdates()
        XCTAssertEqual(engine.checkCount, 0)

        engine.canCheckForUpdates = true
        controller.refresh()
        controller.checkForUpdates()

        XCTAssertEqual(engine.checkCount, 1)
    }

    func testMissingConfigurationDisablesUpdateControls() {
        let controller = AutomaticUpdateController(engine: nil)

        XCTAssertFalse(controller.isAvailable)
        XCTAssertFalse(controller.canCheckForUpdates)
        XCTAssertFalse(controller.automaticallyChecksForUpdates)

        controller.setAutomaticallyChecksForUpdates(true)
        controller.checkForUpdates()

        XCTAssertFalse(controller.automaticallyChecksForUpdates)
    }
}

@MainActor
private final class FakeAutomaticUpdateEngine: AutomaticUpdateEngine {
    var canCheckForUpdates = false
    var automaticallyChecksForUpdates = false
    private(set) var checkCount = 0

    func checkForUpdates() {
        checkCount += 1
    }
}
