import Foundation
import SwiftUI

struct TranslationResultView: View {
    let originalText: String
    let languagePair: LanguagePairViewModel
    let translatedText: String
    let copyText: (String) -> Void
    var targetLanguages: TargetLanguageSettingsController?
    var selectTargetLanguage: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            PopupResultHeader(
                title: LocalizedCopy.text("Translation"),
                systemImage: "character.bubble.fill",
                detail: targetLanguages == nil ? languagePairDetail : nil,
                detailAccessibilityLabel: languagePairAccessibilityLabel,
                copyHelpText: LocalizedCopy.text("Copy translation"),
                copyAction: {
                    copyText(translatedText)
                }
            )

            if let targetLanguages {
                TranslationLanguageRow(
                    controller: targetLanguages,
                    sourceLanguage: localizedSourceLanguage,
                    fallbackDetail: languagePairDetail,
                    fallbackAccessibilityLabel: languagePairAccessibilityLabel,
                    select: selectTargetLanguage
                )
            }

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

    private var languagePairDetail: String {
        "\(localizedSourceLanguage)  →  \(localizedTargetLanguage)"
    }

    private var languagePairAccessibilityLabel: String {
        LocalizedCopy.format(
            "From %@ to %@",
            localizedSourceLanguage,
            localizedTargetLanguage
        )
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

private struct TranslationLanguageRow: View {
    @ObservedObject var controller: TargetLanguageSettingsController
    let sourceLanguage: String
    let fallbackDetail: String
    let fallbackAccessibilityLabel: String
    let select: (String) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 5) {
            HStack(spacing: 6) {
                if controller.options.isEmpty {
                    Text(fallbackDetail)
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                        .accessibilityLabel(fallbackAccessibilityLabel)

                    if controller.isLoading {
                        ProgressView()
                            .controlSize(.mini)
                            .accessibilityLabel(
                                LocalizedCopy.text("Loading supported languages...")
                            )
                    }
                } else {
                    Text("\(sourceLanguage)  →")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                        .accessibilityLabel(
                            LocalizedCopy.format("Translated from %@", sourceLanguage)
                        )

                    Picker(
                        LocalizedCopy.text("Target language"),
                        selection: Binding(
                            get: { controller.selectedIdentifier },
                            set: { identifier in select(identifier) }
                        )
                    ) {
                        ForEach(controller.options) { option in
                            Text(option.name).tag(option.id)
                        }
                    }
                    .labelsHidden()
                    .pickerStyle(.menu)
                    .controlSize(.small)
                    .accessibilityLabel(LocalizedCopy.text("Target language"))
                    .help(LocalizedCopy.text("Translate into a different language"))
                }
            }

            if let errorMessage = controller.errorMessage {
                HStack(spacing: 8) {
                    Label(errorMessage, systemImage: "exclamationmark.triangle")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .fixedSize(horizontal: false, vertical: true)

                    if controller.options.isEmpty, !controller.isLoading {
                        Button(LocalizedCopy.text("Retry")) {
                            Task {
                                await controller.load()
                            }
                        }
                        .buttonStyle(.link)
                        .controlSize(.small)
                    }
                }
            }
        }
        .task {
            await controller.load()
        }
    }
}
