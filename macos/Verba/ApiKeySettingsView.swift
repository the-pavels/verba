import SwiftUI

struct ApiKeySettingsView: View {
    @ObservedObject var controller: ApiKeySettingsController

    var body: some View {
        Section("Proofreading") {
            SecureField("OpenAI API key", text: $controller.apiKeyInput)
                .disabled(controller.isLoading || controller.isTesting)
                .accessibilityHint(
                    LocalizedCopy.text("Stored only in your macOS Keychain.")
                )

            HStack {
                Button(
                    controller.isConfigured
                        ? LocalizedCopy.text("Replace")
                        : LocalizedCopy.text("Save")
                ) {
                    controller.save()
                }
                .disabled(!controller.canSave)

                Button("Delete", role: .destructive) {
                    controller.delete()
                }
                .disabled(!controller.isConfigured || controller.isTesting)

                Spacer()

                Button("Test Connection") {
                    Task {
                        await controller.testConnection()
                    }
                }
                .disabled(!controller.isConfigured || controller.isLoading || controller.isTesting)
            }

            HStack(spacing: 8) {
                if controller.isLoading || controller.isTesting {
                    ProgressView()
                        .controlSize(.small)
                }

                Text(
                    controller.isConfigured
                        ? LocalizedCopy.text("••••••••  Stored in Keychain")
                        : LocalizedCopy.text("No API key configured")
                )
                .foregroundStyle(.secondary)
                .accessibilityLabel(
                    AccessibilityCopy.apiKeyStatus(isConfigured: controller.isConfigured)
                )
            }

            if let feedback = controller.feedback {
                Label(
                    feedback.message,
                    systemImage: feedback.kind == .success
                        ? "checkmark.circle"
                        : "exclamationmark.triangle"
                )
                .foregroundStyle(.primary)
            } else {
                Text("Verba stores the key only in your macOS Keychain.")
                    .foregroundStyle(.secondary)
            }
        }
    }
}
