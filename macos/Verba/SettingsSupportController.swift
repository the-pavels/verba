import AppKit
import SwiftUI

struct SupportDiagnosticsSnapshot: Equatable {
    let accessibility: AccessibilityPermissionStatus
    let targetLanguage: String
    let translateShortcut: String
    let proofreadShortcut: String
    let isApiKeyConfigured: Bool
}

@MainActor
protocol SupportDiagnosticsWriting {
    func write(_ diagnostics: String) -> Bool
}

@MainActor
final class SettingsSupportController: ObservableObject {
    @Published private(set) var feedback: String?
    @Published private(set) var latestErrorCode: String?

    let appVersion: String
    let buildVersion: String
    let rustCoreVersion: String

    private let operatingSystem: String
    private let architecture: String
    private let writer: any SupportDiagnosticsWriting

    convenience init(rustCoreVersion: String) {
        let bundle = Bundle.main
        self.init(
            appVersion: bundle.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String
                ?? LocalizedCopy.text("Unknown"),
            buildVersion: bundle.object(forInfoDictionaryKey: "CFBundleVersion") as? String
                ?? LocalizedCopy.text("Unknown"),
            rustCoreVersion: rustCoreVersion,
            operatingSystem: ProcessInfo.processInfo.operatingSystemVersionString,
            architecture: Self.currentArchitecture,
            writer: SystemSupportDiagnosticsWriter()
        )
    }

    init(
        appVersion: String,
        buildVersion: String,
        rustCoreVersion: String,
        operatingSystem: String,
        architecture: String,
        writer: any SupportDiagnosticsWriting
    ) {
        self.appVersion = appVersion
        self.buildVersion = buildVersion
        self.rustCoreVersion = rustCoreVersion
        self.operatingSystem = operatingSystem
        self.architecture = architecture
        self.writer = writer
    }

    var versionSummary: String {
        "\(appVersion) (\(buildVersion))"
    }

    func diagnostics(for snapshot: SupportDiagnosticsSnapshot) -> String {
        """
        Verba support diagnostics
        App: \(versionSummary)
        Rust core: \(rustCoreVersion)
        macOS: \(operatingSystem)
        Architecture: \(architecture)
        Accessibility: \(snapshot.accessibility.diagnosticName)
        Target language: \(snapshot.targetLanguage.isEmpty ? "unavailable" : snapshot.targetLanguage)
        Translate shortcut: \(snapshot.translateShortcut.isEmpty ? "unavailable" : snapshot.translateShortcut)
        Proofread shortcut: \(snapshot.proofreadShortcut.isEmpty ? "unavailable" : snapshot.proofreadShortcut)
        OpenAI API key configured: \(snapshot.isApiKeyConfigured ? "yes" : "no")
        Latest error: \(latestErrorCode ?? "none")
        Privacy: no API key, selected text, or document content included
        """
    }

    func recordDiagnosticCode(_ code: String) {
        let allowed = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789.-")
        let isSafe = !code.isEmpty
            && code.utf8.count <= 80
            && code.unicodeScalars.allSatisfy(allowed.contains)
        latestErrorCode = isSafe ? code : "redacted.invalid-diagnostic-code"
    }

    func copyDiagnostics(for snapshot: SupportDiagnosticsSnapshot) {
        feedback = writer.write(diagnostics(for: snapshot))
            ? LocalizedCopy.text("Support diagnostics copied.")
            : LocalizedCopy.text("Support diagnostics couldn’t be copied.")
    }

    private static var currentArchitecture: String {
#if arch(arm64)
        "arm64"
#elseif arch(x86_64)
        "x86_64"
#else
        "unknown"
#endif
    }
}

@MainActor
private struct SystemSupportDiagnosticsWriter: SupportDiagnosticsWriting {
    func write(_ diagnostics: String) -> Bool {
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        return pasteboard.setString(diagnostics, forType: .string)
    }
}

struct PrivacyAndSupportSettingsView: View {
    @ObservedObject var accessibility: AccessibilityPermissionController
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController
    @ObservedObject var support: SettingsSupportController

    var body: some View {
        accessibilitySection
        privacySection
        supportSection
    }

    private var accessibilitySection: some View {
        Section("Accessibility") {
            Label(accessibility.status.title, systemImage: accessibility.status.systemImage)

            Text(accessibility.status.explanation)
                .foregroundStyle(.secondary)

            switch accessibility.status {
            case .notRequested:
                Button("Request Accessibility Access…") {
                    accessibility.requestPermission()
                }
            case .denied:
                Button("Open Accessibility Settings…") {
                    accessibility.openSystemSettings()
                }
            case .granted:
                EmptyView()
            }
        }
    }

    private var privacySection: some View {
        Section("Privacy") {
            Text("Translation runs with Apple Translation on this Mac.")
            Text("Proofreading sends only the selected text to OpenAI when you invoke Proofread. Your API key remains in Keychain.")
            Text("Verba does not keep selected-text history or include private content in support diagnostics.")
                .foregroundStyle(.secondary)
        }
    }

    private var supportSection: some View {
        Section("About & Support") {
            LabeledContent("Verba", value: support.versionSummary)
            LabeledContent("Rust core", value: support.rustCoreVersion)

            HStack {
                Button("Copy Support Diagnostics") {
                    support.copyDiagnostics(for: diagnosticsSnapshot)
                }
                Spacer()
                if let feedback = support.feedback {
                    Text(feedback)
                        .foregroundStyle(.secondary)
                }
            }
        }
    }

    private var diagnosticsSnapshot: SupportDiagnosticsSnapshot {
        SupportDiagnosticsSnapshot(
            accessibility: accessibility.status,
            targetLanguage: targetLanguage.selectedIdentifier,
            translateShortcut: shortcuts.translate,
            proofreadShortcut: shortcuts.proofread,
            isApiKeyConfigured: apiKey.isConfigured
        )
    }
}
