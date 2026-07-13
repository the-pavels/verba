import SwiftUI

struct TranslationResultView: View {
    let originalText: String
    let languagePair: LanguagePairViewModel
    let translatedText: String

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Label("Translation", systemImage: "character.bubble")
                    .font(.headline)

                Spacer()

                Text("\(languagePair.source) → \(languagePair.target)")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .accessibilityLabel(
                        "From \(languagePair.source) to \(languagePair.target)"
                    )
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    textSection(title: "Original", text: originalText, isSecondary: true)
                    textSection(title: "Translation", text: translatedText, isSecondary: false)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private func textSection(
        title: String,
        text: String,
        isSecondary: Bool
    ) -> some View {
        VStack(alignment: .leading, spacing: 5) {
            Text(title.uppercased())
                .font(.caption2.weight(.semibold))
                .foregroundStyle(.secondary)

            Text(text)
                .font(.body)
                .foregroundStyle(isSecondary ? .secondary : .primary)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
    }
}
