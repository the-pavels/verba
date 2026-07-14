import SwiftUI

struct PopupContentView: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let presentation: PresentationViewModel
    let copyText: (String) -> Void
    let continueProofreading: () -> Void
    let cancelProofreading: () -> Void
    let recover: (RecoveryActionViewModel, PresentationAction?) -> Void

    var body: some View {
        content
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
            .padding(20)
            .background {
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(.regularMaterial)

                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(
                        LinearGradient(
                            colors: [
                                Color.accentColor.opacity(0.075),
                                Color.clear,
                                Color.clear,
                            ],
                            startPoint: .topLeading,
                            endPoint: .bottomTrailing
                        )
                    )
            }
            .overlay {
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .stroke(.separator.opacity(0.5), lineWidth: 1)
            }
            .padding(1)
            .focusSection()
            .accessibilityElement(children: .contain)
            .transaction { transaction in
                if reduceMotion {
                    transaction.animation = nil
                }
            }
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
                    .accessibilityHidden(true)

                Text(action.loadingTitle)
                    .font(.headline)
            }
            .accessibilityElement(children: .combine)
            .accessibilityLabel(action.loadingTitle)
        case .proofreadingDisclosure:
            VStack(alignment: .leading, spacing: 12) {
                PopupStatusHeader(
                    title: LocalizedCopy.text("Send selected text to OpenAI?"),
                    systemImage: "hand.raised.fill"
                )

                Text(LocalizedCopy.text(
                    "Proofreading sends the selected text to OpenAI using your API key. Translation remains on this Mac."
                ))
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
        case let .proofreading(originalText, correctedText):
            ProofreadingResultView(
                originalText: originalText,
                correctedText: correctedText,
                copyText: copyText
            )
        case .noIssues:
            VStack(alignment: .leading, spacing: 10) {
                PopupStatusHeader(
                    title: LocalizedCopy.text("No issues found"),
                    systemImage: "checkmark.circle.fill"
                )

                Text("The selected text looks good. No corrections are needed.")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
            }
        case let .error(action, title, message, recovery, _):
            VStack(alignment: .leading, spacing: 10) {
                PopupStatusHeader(
                    title: title,
                    systemImage: "exclamationmark.triangle.fill"
                )

                Text(message)
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)

                HStack {
                    Spacer()
                    Button(recovery.buttonTitle) {
                        recover(recovery, action)
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
        }
    }
}

private struct PopupStatusHeader: View {
    let title: String
    let systemImage: String

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: systemImage)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Color.accentColor)
                .frame(width: 28, height: 28)
                .background(Color.accentColor.opacity(0.12), in: RoundedRectangle(cornerRadius: 8))
                .accessibilityHidden(true)

            Text(title)
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
        }
    }
}

extension RecoveryActionViewModel {
    var buttonTitle: String {
        switch self {
        case .retry:
            LocalizedCopy.text("Retry")
        case .openSettings:
            LocalizedCopy.text("Open Settings")
        case .grantAccessibility:
            LocalizedCopy.text("Grant Access")
        case .changeLanguage:
            LocalizedCopy.text("Change Language")
        case .dismiss:
            LocalizedCopy.text("Dismiss")
        }
    }

    func command(for action: PresentationAction?) -> PopupRecoveryCommand {
        switch self {
        case .retry:
            action.map(PopupRecoveryCommand.retry) ?? .dismiss
        case .openSettings, .changeLanguage:
            .openSettings
        case .grantAccessibility:
            .grantAccessibility
        case .dismiss:
            .dismiss
        }
    }
}

enum PopupRecoveryCommand: Equatable {
    case retry(PresentationAction)
    case openSettings
    case grantAccessibility
    case dismiss
}

private extension PresentationAction {
    var loadingTitle: String {
        switch self {
        case .translate:
            LocalizedCopy.text("Translating selected text...")
        case .proofread:
            LocalizedCopy.text("Proofreading selected text...")
        }
    }
}
