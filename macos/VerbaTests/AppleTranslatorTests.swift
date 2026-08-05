import Foundation
import Translation
import XCTest
@testable import Verba

@MainActor
final class AppleTranslatorTests: XCTestCase {
    func testInstalledPairTranslatesWithIdentifiedSource() async throws {
        let languageIdentifier = FakeTranslationLanguageIdentifier(identifier: "de")
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
            languageIdentifier: languageIdentifier,
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
        XCTAssertEqual(languageIdentifier.requests, ["Hallo"])
        XCTAssertEqual(availability.requests.count, 1)
        XCTAssertEqual(availability.requests[0].source.minimalIdentifier, "de")
        XCTAssertEqual(availability.requests[0].target.minimalIdentifier, "en")
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "de")
        XCTAssertEqual(sessions.requests[0].target.minimalIdentifier, "en")
        XCTAssertEqual(sessions.requests[0].preparation, .none)
    }

    func testSystemLanguageIdentifierDetectsSelectedGermanText() async throws {
        let availability = FakeTranslationAvailability(status: .installed)
        let translator = AppleTranslator(
            availability: availability,
            sessions: FakeTranslationSessions(
                result: .success(
                    AppleTranslationResult(
                        sourceLanguageIdentifier: "de",
                        targetLanguageIdentifier: "en",
                        translatedText: "The boss spoke"
                    )
                )
            )
        )

        _ = try await translator.translate(
            "Der Chef sprach",
            targetLanguageIdentifier: "en"
        )

        XCTAssertEqual(availability.requests.first?.source.minimalIdentifier, "de")
    }

    func testSystemLanguageIdentifierIncludesPreferredFallbackForAmbiguousWord() {
        let identifier = SystemTranslationLanguageIdentifier(
            preferredLanguageIdentifiers: ["de"]
        )

        let candidates = identifier.identify("bergen")

        XCTAssertEqual(candidates.first?.minimalIdentifier, "de")
    }

    func testSystemLanguageIdentifierPrioritizesPreferredEnglishForApple() {
        let identifier = SystemTranslationLanguageIdentifier(
            preferredLanguageIdentifiers: ["en", "de"]
        )

        let candidates = identifier.identify("Apple")

        XCTAssertEqual(candidates.first?.minimalIdentifier, "en")
    }

    func testSurroundingContextIdentifiesGermanButOnlyTranslatesSelectedWord() async throws {
        let availability = FakeTranslationAvailability(status: .installed)
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "de",
                    targetLanguageIdentifier: "ru",
                    translatedText: "спасать"
                )
            )
        )
        let translator = AppleTranslator(
            availability: availability,
            sessions: sessions
        )

        _ = try await translator.translate(
            "bergen",
            languageDetectionContext: "Rega muss sie bergen, bevor Hilfe eintrifft.",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(availability.requests[0].source.minimalIdentifier, "de")
        XCTAssertEqual(sessions.requests[0].text, "bergen")
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "de")
    }

    func testContextualDownloadableCandidateWinsBeforeInstalledIsolatedGuess() async throws {
        let availability = FakeTranslationAvailability(
            statuses: [.supported, .installed]
        )
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "de",
                    targetLanguageIdentifier: "ru",
                    translatedText: "спасать"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(
                identifierResponses: [["de"], ["nb"]]
            ),
            availability: availability,
            sessions: sessions
        )

        _ = try await translator.translate(
            "bergen",
            languageDetectionContext: "Rega muss sie bergen, bevor Hilfe eintrifft.",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(availability.requests.count, 1)
        XCTAssertEqual(availability.requests[0].source.minimalIdentifier, "de")
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "de")
        XCTAssertEqual(sessions.requests[0].preparation, .required)
    }

    func testAmbiguousWordUsesPreferredCandidateWhenDominantPairIsUnsupported() async throws {
        let availability = FakeTranslationAvailability(
            statuses: [.unsupported, .installed]
        )
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "de",
                    targetLanguageIdentifier: "ru",
                    translatedText: "спасать"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(
                identifiers: ["nb", "de"]
            ),
            availability: availability,
            sessions: sessions
        )

        let result = try await translator.translate(
            "bergen",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(result.sourceLanguageIdentifier, "de")
        XCTAssertEqual(
            availability.requests.map(\.source.minimalIdentifier),
            ["nb", "de"]
        )
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "de")
    }

    func testInstalledCandidateWinsBeforeAnEarlierDownloadableCandidate() async throws {
        let availability = FakeTranslationAvailability(
            statuses: [.supported, .installed]
        )
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "en",
                    targetLanguageIdentifier: "ru",
                    translatedText: "Яблоко"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(
                identifiers: ["tr", "en"]
            ),
            availability: availability,
            sessions: sessions
        )

        let result = try await translator.translate(
            "Apple",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(result.sourceLanguageIdentifier, "en")
        XCTAssertEqual(
            availability.requests.map(\.source.minimalIdentifier),
            ["tr", "en"]
        )
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "en")
        XCTAssertEqual(sessions.requests[0].preparation, .none)
    }

    func testDownloadableCandidateIsPreparedOnlyAfterInstalledCandidatesAreExhausted()
        async throws
    {
        let availability = FakeTranslationAvailability(
            statuses: [.supported, .unsupported]
        )
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "tr",
                    targetLanguageIdentifier: "ru",
                    translatedText: "Эппл"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(
                identifiers: ["tr", "en"]
            ),
            availability: availability,
            sessions: sessions
        )

        _ = try await translator.translate(
            "Apple",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(
            availability.requests.map(\.source.minimalIdentifier),
            ["tr", "en"]
        )
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "tr")
        XCTAssertEqual(sessions.requests[0].preparation, .required)
    }

    func testInstalledTurkishCandidateKeepsPriorityForTurkishText() async throws {
        let availability = FakeTranslationAvailability(
            statuses: [.installed, .installed]
        )
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "tr",
                    targetLanguageIdentifier: "ru",
                    translatedText: "Привет"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(
                identifiers: ["tr", "en"]
            ),
            availability: availability,
            sessions: sessions
        )

        _ = try await translator.translate(
            "Merhaba",
            targetLanguageIdentifier: "ru"
        )

        XCTAssertEqual(availability.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "tr")
        XCTAssertEqual(sessions.requests[0].preparation, .none)
    }

    func testSupportedPairPreparesAndResumesTranslation() async throws {
        let sessions = FakeTranslationSessions(
            result: .success(
                AppleTranslationResult(
                    sourceLanguageIdentifier: "en",
                    targetLanguageIdentifier: "fr",
                    translatedText: "Bonjour"
                )
            )
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "en"),
            availability: FakeTranslationAvailability(status: .supported),
            sessions: sessions
        )

        let result = try await translator.translate(
            "Hello",
            targetLanguageIdentifier: "fr"
        )

        XCTAssertEqual(result.translatedText, "Bonjour")
        XCTAssertEqual(sessions.requests.count, 1)
        XCTAssertEqual(sessions.requests[0].source?.minimalIdentifier, "en")
        XCTAssertEqual(sessions.requests[0].target.minimalIdentifier, "fr")
        XCTAssertEqual(sessions.requests[0].preparation, .required)
    }

    func testUnsupportedPairDoesNotCreateASession() async {
        let sessions = FakeTranslationSessions(result: .failure(TestFailure()))
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "en"),
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

    func testUnidentifiedSourceDoesNotCheckAvailabilityOrCreateASession() async {
        let availability = FakeTranslationAvailability(status: .installed)
        let sessions = FakeTranslationSessions(result: .failure(TestFailure()))
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: nil),
            availability: availability,
            sessions: sessions
        )

        await assertTranslationError(.unableToIdentifyLanguage) {
            try await translator.translate("...", targetLanguageIdentifier: "en")
        }
        XCTAssertTrue(availability.requests.isEmpty)
        XCTAssertTrue(sessions.requests.isEmpty)
    }

    func testCancellationIsMappedWithoutLosingItsMeaning() async {
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "de"),
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
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "de"),
            availability: FakeTranslationAvailability(
                result: .failure(TestFailure())
            ),
            sessions: FakeTranslationSessions(result: .failure(TestFailure()))
        )

        await assertTranslationError(.failed) {
            try await translator.translate("Hallo", targetLanguageIdentifier: "en")
        }
    }

#if compiler(>=6.2)
    func testNotInstalledSessionErrorRetriesWithPreparation() async throws {
        guard #available(macOS 26.0, *) else {
            return
        }

        let sessions = FakeTranslationSessions(
            results: [
                .failure(TranslationError.notInstalled),
                .success(
                    AppleTranslationResult(
                        sourceLanguageIdentifier: "de",
                        targetLanguageIdentifier: "en",
                        translatedText: "Hello"
                    )
                ),
            ]
        )
        let translator = AppleTranslator(
            languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "de"),
            availability: FakeTranslationAvailability(status: .installed),
            sessions: sessions
        )

        let result = try await translator.translate(
            "Hallo",
            targetLanguageIdentifier: "en"
        )

        XCTAssertEqual(result.translatedText, "Hello")
        XCTAssertEqual(sessions.requests.map(\.preparation), [.none, .required])
    }
#endif

    func testSessionBrokerConfiguresIdentifiedSourceAndCancelsPendingWork() async {
        let sessions = SystemTranslationSessionProvider()
        let task = Task { @MainActor in
            try await sessions.translate(
                "Hallo",
                source: Locale.Language(identifier: "de"),
                target: Locale.Language(identifier: "en"),
                preparation: .required
            )
        }

        await Task.yield()

        XCTAssertEqual(sessions.configuration?.source?.minimalIdentifier, "de")
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

    func testSessionBrokerActivatesAndPreparesBeforeDownloadingLanguages() async throws {
        var activationCount = 0
        let sessions = SystemTranslationSessionProvider {
            activationCount += 1
        }
        let task = Task { @MainActor in
            try await sessions.translate(
                "Hallo",
                source: Locale.Language(identifier: "de"),
                target: Locale.Language(identifier: "en"),
                preparation: .required
            )
        }
        await Task.yield()
        let executor = FakeTranslationSessionExecutor()

        await sessions.run(executor)
        let result = try await task.value

        XCTAssertEqual(activationCount, 1)
        XCTAssertEqual(executor.calls, ["prepare", "translate"])
        XCTAssertEqual(result.translatedText, "Hello")
    }

    func testSessionBrokerInvalidatesConfigurationForRepeatedLanguagePair() async throws {
        let sessions = SystemTranslationSessionProvider()
        let firstTask = Task { @MainActor in
            try await sessions.translate(
                "Hallo",
                source: Locale.Language(identifier: "de"),
                target: Locale.Language(identifier: "en"),
                preparation: .none
            )
        }
        await Task.yield()
        let initialVersion = try XCTUnwrap(sessions.configuration).version

        await sessions.run(FakeTranslationSessionExecutor())
        _ = try await firstTask.value

        let secondTask = Task { @MainActor in
            try await sessions.translate(
                "Guten Tag",
                source: Locale.Language(identifier: "de"),
                target: Locale.Language(identifier: "en"),
                preparation: .none
            )
        }
        await Task.yield()

        XCTAssertGreaterThan(try XCTUnwrap(sessions.configuration).version, initialVersion)
        await sessions.run(FakeTranslationSessionExecutor())
        _ = try await secondTask.value
    }

    func testNativeAdapterConvertsTheTranslationResult() async throws {
        let adapter = NativeAppleTranslator(
            translator: AppleTranslator(
                languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "de"),
                availability: FakeTranslationAvailability(status: .installed),
                sessions: FakeTranslationSessions(
                    result: .success(
                        AppleTranslationResult(
                            sourceLanguageIdentifier: "de",
                            targetLanguageIdentifier: "en",
                            translatedText: "Hello"
                        )
                    )
                )
            )
        )

        let result = try await adapter.translate(
            request: NativeTranslationRequest(
                text: "Hallo",
                targetLanguageIdentifier: "en",
                languageDetectionContext: nil
            )
        )

        XCTAssertEqual(
            result,
            NativeTranslationResponse(
                sourceLanguageIdentifier: "de",
                translatedText: "Hello"
            )
        )
    }

    func testNativeAdapterPreservesUnsupportedPairs() async {
        let adapter = NativeAppleTranslator(
            translator: AppleTranslator(
                languageIdentifier: FakeTranslationLanguageIdentifier(identifier: "en"),
                availability: FakeTranslationAvailability(status: .unsupported),
                sessions: FakeTranslationSessions(result: .failure(TestFailure()))
            )
        )

        do {
            _ = try await adapter.translate(
                request: NativeTranslationRequest(
                    text: "Hello",
                    targetLanguageIdentifier: "ga",
                    languageDetectionContext: nil
                )
            )
            XCTFail("Expected the native adapter to reject the language pair")
        } catch let error as NativeTranslationError {
            XCTAssertEqual(error, .UnsupportedPair)
        } catch {
            XCTFail("Unexpected error: \(type(of: error))")
        }
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
private final class FakeTranslationLanguageIdentifier: TranslationLanguageIdentifying {
    private var responses: [[Locale.Language]]
    private(set) var requests: [String] = []

    init(identifier: String?) {
        responses = [identifier.map {
            [Locale.Language(identifier: $0)]
        } ?? []]
    }

    init(identifiers: [String]) {
        responses = [identifiers.map(Locale.Language.init(identifier:))]
    }

    init(identifierResponses: [[String]]) {
        precondition(!identifierResponses.isEmpty)
        responses = identifierResponses.map { identifiers in
            identifiers.map(Locale.Language.init(identifier:))
        }
    }

    func identify(_ text: String) -> [Locale.Language] {
        requests.append(text)
        let response = responses[0]
        if responses.count > 1 {
            responses.removeFirst()
        }
        return response
    }
}

@MainActor
private final class FakeTranslationAvailability: TranslationAvailabilityChecking {
    struct Request {
        let source: Locale.Language
        let target: Locale.Language
    }

    private var results: [Result<TranslationPairStatus, any Error>]
    private(set) var requests: [Request] = []

    convenience init(status: TranslationPairStatus) {
        self.init(result: .success(status))
    }

    init(result: Result<TranslationPairStatus, any Error>) {
        results = [result]
    }

    init(statuses: [TranslationPairStatus]) {
        precondition(!statuses.isEmpty)
        results = statuses.map(Result.success)
    }

    func status(
        from source: Locale.Language,
        target: Locale.Language
    ) async throws -> TranslationPairStatus {
        requests.append(Request(source: source, target: target))
        let result = results[0]
        if results.count > 1 {
            results.removeFirst()
        }
        return try result.get()
    }
}

@MainActor
private final class FakeTranslationSessions: TranslationSessionProviding {
    struct Request {
        let text: String
        let source: Locale.Language?
        let target: Locale.Language
        let preparation: TranslationPreparation
    }

    private var results: [Result<AppleTranslationResult, any Error>]
    private(set) var requests: [Request] = []

    init(result: Result<AppleTranslationResult, any Error>) {
        results = [result]
    }

    init(results: [Result<AppleTranslationResult, any Error>]) {
        precondition(!results.isEmpty)
        self.results = results
    }

    func translate(
        _ text: String,
        source: Locale.Language?,
        target: Locale.Language,
        preparation: TranslationPreparation
    ) async throws -> AppleTranslationResult {
        requests.append(
            Request(
                text: text,
                source: source,
                target: target,
                preparation: preparation
            )
        )
        let result = results[0]
        if results.count > 1 {
            results.removeFirst()
        }
        return try result.get()
    }
}

@MainActor
private final class FakeTranslationSessionExecutor: TranslationSessionExecuting {
    private(set) var calls: [String] = []

    func prepareTranslation() async throws {
        calls.append("prepare")
    }

    func translate(_ text: String) async throws -> TranslationSession.Response {
        calls.append("translate")
        return TranslationSession.Response(
            sourceLanguage: Locale.Language(identifier: "de"),
            targetLanguage: Locale.Language(identifier: "en"),
            sourceText: text,
            targetText: "Hello"
        )
    }
}

private struct TestFailure: Error {}
