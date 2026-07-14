import SwiftUI

struct ProofreadingResultView: View {
    let correctedText: String
    let explanation: String
    let copyText: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Label("Proofreading", systemImage: "checkmark.circle")
                    .font(.headline)
                    .accessibilityAddTraits(.isHeader)

                Spacer()

                ResultCopyButton(helpText: LocalizedCopy.text("Copy corrected text")) {
                    copyText(correctedText)
                }
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    VStack(alignment: .leading, spacing: 5) {
                        Text("CORRECTED TEXT")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .accessibilityAddTraits(.isHeader)

                        Text(correctedText)
                            .font(.body)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                    }

                    VStack(alignment: .leading, spacing: 5) {
                        Text("WHAT CHANGED")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .accessibilityAddTraits(.isHeader)

                        Text(explanation)
                            .font(.subheadline)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }
}
