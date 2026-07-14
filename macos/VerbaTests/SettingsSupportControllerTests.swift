import XCTest
@testable import Verba

@MainActor
final class SettingsSupportControllerTests: XCTestCase {
    func testDiagnosticsContainOnlyRedactedConfigurationState() {
        let writer = RecordingDiagnosticsWriter()
        let controller = makeController(writer: writer)
        let snapshot = SupportDiagnosticsSnapshot(
            accessibility: .granted,
            targetLanguage: "de",
            translateShortcut: "⌃⌥T",
            proofreadShortcut: "⌃⌥P",
            isApiKeyConfigured: true
        )

        let diagnostics = controller.diagnostics(for: snapshot)

        XCTAssertEqual(
            diagnostics,
            """
            Verba support diagnostics
            App: 0.1.0 (7)
            Rust core: 0.1.0
            macOS: macOS 15.5
            Architecture: arm64
            Accessibility: granted
            Target language: de
            Translate shortcut: ⌃⌥T
            Proofread shortcut: ⌃⌥P
            OpenAI API key configured: yes
            Privacy: no API key, selected text, or document content included
            """
        )
    }

    func testCopyUsesTheInjectedWriterAndReportsTheOutcome() {
        let writer = RecordingDiagnosticsWriter()
        let controller = makeController(writer: writer)
        let snapshot = SupportDiagnosticsSnapshot(
            accessibility: .denied,
            targetLanguage: "",
            translateShortcut: "",
            proofreadShortcut: "",
            isApiKeyConfigured: false
        )

        controller.copyDiagnostics(for: snapshot)

        XCTAssertEqual(writer.values.count, 1)
        XCTAssertTrue(writer.values[0].contains("Target language: unavailable"))
        XCTAssertTrue(writer.values[0].contains("OpenAI API key configured: no"))
        XCTAssertEqual(controller.feedback, "Support diagnostics copied.")

        writer.succeeds = false
        controller.copyDiagnostics(for: snapshot)
        XCTAssertEqual(controller.feedback, "Support diagnostics couldn’t be copied.")
    }

    private func makeController(
        writer: RecordingDiagnosticsWriter
    ) -> SettingsSupportController {
        SettingsSupportController(
            appVersion: "0.1.0",
            buildVersion: "7",
            rustCoreVersion: "0.1.0",
            operatingSystem: "macOS 15.5",
            architecture: "arm64",
            writer: writer
        )
    }
}

@MainActor
private final class RecordingDiagnosticsWriter: SupportDiagnosticsWriting {
    var succeeds = true
    private(set) var values: [String] = []

    func write(_ diagnostics: String) -> Bool {
        values.append(diagnostics)
        return succeeds
    }
}
