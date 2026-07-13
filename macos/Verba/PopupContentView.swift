import SwiftUI

struct PopupContentView: View {
    let presentation: PresentationViewModel

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
        case let .translation(originalText, languagePair, translatedText):
            TranslationResultView(
                originalText: originalText,
                languagePair: languagePair,
                translatedText: translatedText
            )
        case let .proofreading(correctedText, explanation):
            ProofreadingResultView(
                correctedText: correctedText,
                explanation: explanation
            )
        case .noIssues:
            placeholder(title: "No Issues Found")
        case .error:
            placeholder(title: "Unable to Complete Request")
        }
    }

    private func placeholder(title: String) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(title)
                .font(.headline)

            Text("Popup content will be added in a later step.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
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
