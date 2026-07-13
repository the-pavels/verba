import Foundation
import Translation
import XCTest
@testable import Verba

@MainActor
final class AppleTranslatorTests: XCTestCase {
    func testInstalledPairTranslatesWithAutomaticSourceDetection() async throws {
        let availability = FakeTranslationAvailability(status: .installed)
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "de",
                    targetLanguageIdentifier: "en",
                    translatedText: "Hello"
                )
            )
        )
        let translator = AppleTranslator(
            availability: availability,
            sessions: sessions
        )

        let result = try await translator.translate(
            "Hallo",
            targetLanguageIdentifier: "en"
        )

        XCTAssertEqual(
            result,
            AppleTranslationResult(
                sourceLanguageIdentifier: "de",
                targetLanguageIdentifier: "en",
                translatedText: "Hello"
            )
        )
        XCTAssertEqual(availability.requests.count, 1)
        XCTAssertEqual(availability.requests[0].text, "Hallo")
        XCTAssertEqual(availability.requests[0].target.minimalIdentifier, "en")
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertNil(sessions.requests[0].source)
        XCTAssertEqual(sessions.requests[0].target.minimalIdentifier, "en")
    }

    func testSupportedPairRequiresLanguageAssets() async {
        let sessions = FakeTranslationSessions(result: .failure(TestFailure()))
        let translator = AppleTranslator(
            availability: FakeTranslationAvailability(status: .supported),
            sessions: sessions
        )

        await assertTranslationError(
            .languageAssetsRequired(targetLanguageIdentifier: "fr")
        ) {
            try await translator.translate("Hello", targetLanguageIdentifier: "fr")
        }
        XCTAssertTrue(sessions.requests.isEmpty)
    }

    func testUnsupportedPairDoesNotCreateASession() async {
        let sessions = FakeTranslationSessions(result: .failure(TestFailure()))
        let translator = AppleTranslator(
            availability: FakeTranslationAvailability(status: .unsupported),
            sessions: sessions
        )

        await assertTranslationError(
            .unsupportedPair(targetLanguageIdentifier: "ga")
        ) {
            try await translator.translate("Hello", targetLanguageIdentifier: "ga")
        }
        XCTAssertTrue(sessions.requests.isEmpty)
    }

    func testCancellationIsMappedWithoutLosingItsMeaning() async {
        let translator = AppleTranslator(
            availability: FakeTranslationAvailability(
                result: .failure(CancellationError())
            ),
            sessions: FakeTranslationSessions(result: .failure(TestFailure()))
        )

        await assertTranslationError(.cancelled) {
            try await translator.translate("Hallo", targetLanguageIdentifier: "en")
        }
    }

    func testUnknownFailuresAreRedacted() async {
        let translator = AppleTranslator(
            availability: FakeTranslationAvailability(
                result: .failure(TestFailure())
            ),
            sessions: FakeTranslationSessions(result: .failure(TestFailure()))
        )

        await assertTranslationError(.failed) {
            try await translator.translate("Hallo", targetLanguageIdentifier: "en")
        }
    }

    func testNotInstalledSessionErrorRequiresLanguageAssets() async {
        guard #available(macOS 26.0, *) else {
            return
        }

        let translator = AppleTranslator(
            availability: FakeTranslationAvailability(status: .installed),
            sessions: FakeTranslationSessions(
                result: .failure(TranslationError.notInstalled)
            )
        )

        await assertTranslationError(
            .languageAssetsRequired(targetLanguageIdentifier: "en")
        ) {
            try await translator.translate("Hallo", targetLanguageIdentifier: "en")
        }
    }

    func testSessionBrokerConfiguresAutomaticSourceAndCancelsPendingWork() async {
        let sessions = SystemTranslationSessionProvider()
        let task = Task { @MainActor in
            try await sessions.translate(
                "Hallo",
                source: nil,
                target: Locale.Language(identifier: "en")
            )
        }

        await Task.yield()

        XCTAssertNil(sessions.configuration?.source)
        XCTAssertEqual(sessions.configuration?.target?.minimalIdentifier, "en")

        task.cancel()
        do {
            _ = try await task.value
            XCTFail("Expected the pending translation to be cancelled")
        } catch {
            XCTAssertTrue(error is CancellationError)
        }
        XCTAssertNil(sessions.configuration)
    }

    private func assertTranslationError(
        _ expected: AppleTranslationError,
        operation: () async throws -> AppleTranslationResult
    ) async {
        do {
            _ = try await operation()
            XCTFail("Expected translation to fail")
        } catch let error as AppleTranslationError {
            XCTAssertEqual(error, expected)
        } catch {
            XCTFail("Unexpected error: \(type(of: error))")
        }
    }
}

@MainActor
private final class FakeTranslationAvailability: TranslationAvailabilityChecking {
    struct Request {
        let text: String
        let target: Locale.Language
    }

    private let result: Result<TranslationPairStatus, any Error>
    private(set) var requests: [Request] = []

    convenience init(status: TranslationPairStatus) {
        self.init(result: .success(status))
    }

    init(result: Result<TranslationPairStatus, any Error>) {
        self.result = result
    }

    func status(
        for text: String,
        target: Locale.Language
    ) async throws -> TranslationPairStatus {
        requests.append(Request(text: text, target: target))
        return try result.get()
    }
}

@MainActor
private final class FakeTranslationSessions: TranslationSessionProviding {
    struct Request {
        let text: String
        let source: Locale.Language?
        let target: Locale.Language
    }

    private let result: Result<AppleTranslationResult, any Error>
    private(set) var requests: [Request] = []

    init(result: Result<AppleTranslationResult, any Error>) {
        self.result = result
    }

    func translate(
        _ text: String,
        source: Locale.Language?,
        target: Locale.Language
    ) async throws -> AppleTranslationResult {
        requests.append(Request(text: text, source: source, target: target))
        return try result.get()
    }
}

private struct TestFailure: Error {}
