import SwiftUI

struct FirstRunWelcomePage: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            SetupPageHeader(
                systemImage: "character.cursor.ibeam",
                title: LocalizedCopy.text("Write with less friction"),
                message: LocalizedCopy.text(
                    "Select text in any app, then translate or proofread it with a shortcut."
                )
            )

            SetupFeatureRow(
                systemImage: "lock.shield",
                title: LocalizedCopy.text("Private by default"),
                message: LocalizedCopy.text(
                    "Translation stays on this Mac. Verba does not keep selected-text history."
                )
            )

            SetupFeatureRow(
                systemImage: "menubar.rectangle",
                title: LocalizedCopy.text("Always nearby"),
                message: LocalizedCopy.text(
                    "Verba lives in the menu bar and returns focus to the app you were using."
                )
            )
        }
    }
}

struct FirstRunAccessibilityPage: View {
    @ObservedObject var accessibility: AccessibilityPermissionController

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            SetupPageHeader(
                systemImage: "hand.raised.fill",
                title: LocalizedCopy.text("Allow selected-text access"),
                message: LocalizedCopy.text(
                    "macOS requires Accessibility permission before Verba can copy your selection."
                )
            )

            SetupStatusCard(
                systemImage: accessibility.status.systemImage,
                title: accessibility.status == .granted
                    ? LocalizedCopy.text("Accessibility is enabled")
                    : LocalizedCopy.text("Accessibility access required"),
                message: accessibility.status == .granted
                    ? LocalizedCopy.text("Verba can now read selected text when you use a shortcut.")
                    : accessibility.status.explanation
            )

            if accessibility.status != .granted {
                Button(actionTitle) {
                    performAction()
                }
                .buttonStyle(.borderedProminent)
            }
        }
    }

    private var actionTitle: String {
        accessibility.status == .notRequested
            ? LocalizedCopy.text("Enable Accessibility…")
            : LocalizedCopy.text("Open Accessibility Settings…")
    }

    private func performAction() {
        if accessibility.status == .notRequested {
            accessibility.requestPermission()
        } else {
            accessibility.openSystemSettings()
        }
    }
}

struct FirstRunEssentialsPage: View {
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SetupPageHeader(
                systemImage: "globe",
                title: LocalizedCopy.text("Choose your translation setup"),
                message: LocalizedCopy.text(
                    "Pick a target language and review the shortcuts available in every app."
                )
            )

            SetupCard {
                languageControl
                Divider()
                SetupValueRow(title: LocalizedCopy.text("Translate"), value: shortcuts.translate)
                SetupValueRow(title: LocalizedCopy.text("Proofread"), value: shortcuts.proofread)
            }

            Text("You can change the language and shortcuts later in Settings.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    @ViewBuilder
    private var languageControl: some View {
        if targetLanguage.options.isEmpty {
            HStack(spacing: 10) {
                if targetLanguage.isLoading {
                    ProgressView()
                        .controlSize(.small)
                }
                Text(
                    targetLanguage.errorMessage
                        ?? LocalizedCopy.text("Loading supported languages...")
                )
                .foregroundStyle(.secondary)
            }
        } else {
            Picker(
                LocalizedCopy.text("Translate to"),
                selection: Binding(
                    get: { targetLanguage.selectedIdentifier },
                    set: { identifier in
                        targetLanguage.select(identifier)
                    }
                )
            ) {
                ForEach(targetLanguage.options) { option in
                    Text(option.name).tag(option.id)
                }
            }
        }
    }
}

struct FirstRunProofreadingPage: View {
    @ObservedObject var apiKey: ApiKeySettingsController

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            SetupPageHeader(
                systemImage: "checkmark.circle.fill",
                title: LocalizedCopy.text("Set up proofreading"),
                message: LocalizedCopy.text(
                    "Proofreading is optional and uses your own OpenAI API key."
                )
            )

            SetupCard {
                SecureField(
                    LocalizedCopy.text("OpenAI API key"),
                    text: $apiKey.apiKeyInput
                )
                .disabled(apiKey.isLoading || apiKey.isTesting)
                .accessibilityHint(
                    LocalizedCopy.text("Stored only in your macOS Keychain.")
                )

                HStack {
                    Label(statusTitle, systemImage: statusImage)
                        .foregroundStyle(.secondary)

                    Spacer()

                    Button(apiKey.isConfigured ? "Replace" : "Save") {
                        apiKey.save()
                    }
                    .disabled(!apiKey.canSave)
                }
            }

            feedback
        }
    }

    @ViewBuilder
    private var feedback: some View {
        if let feedback = apiKey.feedback {
            Label(
                feedback.message,
                systemImage: feedback.kind == .success
                    ? "checkmark.circle"
                    : "exclamationmark.triangle"
            )
            .font(.caption)
        } else {
            Text("Continue without a key to set up proofreading later in Settings.")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
    }

    private var statusTitle: String {
        apiKey.isConfigured
            ? LocalizedCopy.text("Stored securely in Keychain")
            : LocalizedCopy.text("No API key configured")
    }

    private var statusImage: String {
        apiKey.isConfigured ? "checkmark.circle" : "key"
    }
}

struct FirstRunReadyPage: View {
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            SetupPageHeader(
                systemImage: "checkmark.seal.fill",
                title: LocalizedCopy.text("Verba is ready"),
                message: LocalizedCopy.text(
                    "Select text in any app and press one of your shortcuts."
                )
            )

            SetupCard {
                SetupValueRow(title: LocalizedCopy.text("Translate"), value: shortcuts.translate)
                SetupValueRow(title: LocalizedCopy.text("Proofread"), value: shortcuts.proofread)

                Divider()

                Label(proofreadingStatusTitle, systemImage: proofreadingStatusImage)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var proofreadingStatusTitle: String {
        apiKey.isConfigured
            ? LocalizedCopy.text("Proofreading is enabled")
            : LocalizedCopy.text("Proofreading can be enabled later in Settings")
    }

    private var proofreadingStatusImage: String {
        apiKey.isConfigured ? "checkmark.circle" : "info.circle"
    }
}

struct SetupPageHeader: View {
    let systemImage: String
    let title: String
    let message: String

    var body: some View {
        HStack(alignment: .top, spacing: 14) {
            Image(systemName: systemImage)
                .font(.system(size: 22, weight: .semibold))
                .foregroundStyle(Color.accentColor)
                .frame(width: 44, height: 44)
                .background(
                    Color.accentColor.opacity(0.12),
                    in: RoundedRectangle(cornerRadius: 12, style: .continuous)
                )
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 5) {
                Text(title)
                    .font(.title2.weight(.semibold))
                    .accessibilityAddTraits(.isHeader)

                Text(message)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct SetupFeatureRow: View {
    let systemImage: String
    let title: String
    let message: String

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: systemImage)
                .foregroundStyle(Color.accentColor)
                .frame(width: 22)
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 3) {
                Text(title)
                    .font(.headline)
                Text(message)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

struct SetupStatusCard: View {
    let systemImage: String
    let title: String
    let message: String

    var body: some View {
        SetupCard {
            Label(title, systemImage: systemImage)
                .font(.headline)
            Text(message)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }
}

struct SetupValueRow: View {
    let title: String
    let value: String

    var body: some View {
        HStack {
            Text(title)
            Spacer()
            Text(value)
                .font(.system(.body, design: .monospaced))
                .foregroundStyle(.secondary)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(AccessibilityCopy.setting(title: title, value: value))
    }
}

struct SetupCard<Content: View>: View {
    let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(14)
        .background(
            Color.primary.opacity(0.045),
            in: RoundedRectangle(cornerRadius: 12, style: .continuous)
        )
        .overlay {
            RoundedRectangle(cornerRadius: 12, style: .continuous)
                .stroke(Color.primary.opacity(0.07), lineWidth: 1)
        }
    }
}
