import Foundation
import SwiftUI

struct TranslationResultView: View {
    let originalText: String
    let languagePair: LanguagePairViewModel
    let translatedText: String
    let copyText: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupResultHeader(
                title: LocalizedCopy.text("Translation"),
                systemImage: "character.bubble.fill",
                detail: "\(localizedSourceLanguage)  →  \(localizedTargetLanguage)",
                detailAccessibilityLabel: LocalizedCopy.format(
                    "From %@ to %@",
                    localizedSourceLanguage,
                    localizedTargetLanguage
                ),
                copyHelpText: LocalizedCopy.text("Copy translation"),
                copyAction: {
                    copyText(translatedText)
                }
            )

            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    PopupResultSection(title: LocalizedCopy.text("Original")) {
                        Text(originalText)
                            .foregroundStyle(.secondary)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                            .accessibilityLabel(AccessibilityCopy.originalText(originalText))
                    }

                    PopupResultSection(
                        title: LocalizedCopy.text("Translation"),
                        isEmphasized: true
                    ) {
                        Text(translatedText)
                            .foregroundStyle(.primary)
                            .fixedSize(horizontal: false, vertical: true)
                            .textSelection(.enabled)
                            .accessibilityLabel(
                                AccessibilityCopy.translationText(translatedText)
                            )
                    }
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

}
