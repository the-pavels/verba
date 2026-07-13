import SwiftUI

struct ApiKeySettingsView: View {
    @ObservedObject var controller: ApiKeySettingsController

    var body: some View {
        Section("Proofreading") {
            SecureField("OpenAI API key", text: $controller.apiKeyInput)
                .disabled(controller.isLoading || controller.isTesting)

            HStack {
                Button(controller.isConfigured ? "Replace" : "Save") {
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
                        ? "••••••••  Stored in Keychain"
                        : "No API key configured"
                )
                .foregroundStyle(.secondary)
            }

            if let feedback = controller.feedback {
                Text(feedback.message)
                    .foregroundStyle(feedback.kind == .success ? .green : .red)
            } else {
                Text("Verba stores the key only in your macOS Keychain.")
                    .foregroundStyle(.secondary)
            }
        }
    }
}
