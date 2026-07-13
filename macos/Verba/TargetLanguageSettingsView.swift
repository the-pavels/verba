import SwiftUI

struct TargetLanguageSettingsView: View {
    @ObservedObject var controller: TargetLanguageSettingsController

    var body: some View {
        Section("Translation") {
            if controller.options.isEmpty {
                HStack(spacing: 10) {
                    if controller.isLoading {
                        ProgressView()
                            .controlSize(.small)
                    }

                    Text(controller.errorMessage ?? "Loading supported languages...")
                        .foregroundStyle(.secondary)

                    if controller.errorMessage != nil, !controller.isLoading {
                        Spacer()
                        Button("Retry") {
                            Task {
                                await controller.load()
                            }
                        }
                    }
                }
            } else {
                Picker(
                    "Translate to",
                    selection: Binding(
                        get: { controller.selectedIdentifier },
                        set: { identifier in
                            controller.select(identifier)
                        }
                    )
                ) {
                    ForEach(controller.options) { option in
                        Text(option.name).tag(option.id)
                    }
                }

                if let errorMessage = controller.errorMessage {
                    Text(errorMessage)
                        .foregroundStyle(.red)
                } else {
                    Text("Only languages supported by Apple Translation are shown.")
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}
