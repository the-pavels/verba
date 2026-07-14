import Foundation
import SwiftUI

struct TranslationResultView: View {
    let originalText: String
    let languagePair: LanguagePairViewModel
    let translatedText: String
    let copyText: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 12) {
                Label("Translation", systemImage: "character.bubble")
                    .font(.headline)
                    .accessibilityAddTraits(.isHeader)

                Spacer()

                Text("\(localizedSourceLanguage) → \(localizedTargetLanguage)")
                    .font(.caption.weight(.medium))
                    .foregroundStyle(.secondary)
                    .accessibilityLabel(
                        LocalizedCopy.format(
                            "From %@ to %@",
                            localizedSourceLanguage,
                            localizedTargetLanguage
                        )
                    )

                ResultCopyButton(helpText: LocalizedCopy.text("Copy translation")) {
                    copyText(translatedText)
                }
            }

            Divider()

            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    textSection(
                        title: LocalizedCopy.text("Original"),
                        text: originalText,
                        isSecondary: true
                    )
                    textSection(
                        title: LocalizedCopy.text("Translation"),
                        text: translatedText,
                        isSecondary: false
                    )
                }
                .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
    }

    private var localizedSourceLanguage: String {
        Locale.current.localizedString(forIdentifier: languagePair.source)
            ?? languagePair.source
    }

    private var localizedTargetLanguage: String {
        Locale.current.localizedString(forIdentifier: languagePair.target)
            ?? languagePair.target
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
                .accessibilityAddTraits(.isHeader)

            Text(text)
                .font(.body)
                .foregroundStyle(isSecondary ? .secondary : .primary)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
    }
}
