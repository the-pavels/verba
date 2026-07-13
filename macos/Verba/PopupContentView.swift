import SwiftUI

struct PopupContentView: View {
    let presentation: PresentationViewModel
    let copyText: (String) -> Void
    let continueProofreading: () -> Void
    let cancelProofreading: () -> Void

    var body: some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .padding(18)
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 14))
            .overlay {
                RoundedRectangle(cornerRadius: 14)
                    .stroke(.separator.opacity(0.6), lineWidth: 1)
            }
            .padding(1)
    }

    @ViewBuilder
    private var content: some View {
        switch presentation {
        case .idle:
            EmptyView()
        case let .loading(action):
            HStack(spacing: 12) {
                ProgressView()
                    .controlSize(.small)

                Text(action.loadingTitle)
                    .font(.headline)
            }
        case .proofreadingDisclosure:
            VStack(alignment: .leading, spacing: 10) {
                Label("Send selected text to OpenAI?", systemImage: "hand.raised.fill")
                    .font(.headline)
                    .foregroundStyle(.orange)

                Text(
                    "Proofreading sends the selected text to OpenAI using your API key. "
                        + "Translation remains on this Mac."
                )
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)

                HStack {
                    Spacer()

                    Button("Cancel", action: cancelProofreading)
                        .keyboardShortcut(.cancelAction)

                    Button("Continue", action: continueProofreading)
                        .keyboardShortcut(.defaultAction)
                }
            }
        case let .translation(originalText, languagePair, translatedText):
            TranslationResultView(
                originalText: originalText,
                languagePair: languagePair,
                translatedText: translatedText,
                copyText: copyText
            )
        case let .proofreading(correctedText, explanation):
            ProofreadingResultView(
                correctedText: correctedText,
                explanation: explanation,
                copyText: copyText
            )
        case .noIssues:
            VStack(alignment: .leading, spacing: 7) {
                Label("No issues found", systemImage: "checkmark.circle.fill")
                    .font(.headline)
                    .foregroundStyle(.green)

                Text("The selected text looks good. No corrections are needed.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        case let .error(_, title, message):
            VStack(alignment: .leading, spacing: 7) {
                HStack(alignment: .firstTextBaseline, spacing: 8) {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.orange)

                    Text(title)
                        .font(.headline)
                }

                Text(message)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

private extension PresentationAction {
    var loadingTitle: String {
        switch self {
        case .translate:
            "Translating selected text..."
        case .proofread:
            "Proofreading selected text..."
        }
    }
}
