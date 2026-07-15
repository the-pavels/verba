import SwiftUI

struct ResultCopyButton: View {
    let helpText: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label("Copy", systemImage: "doc.on.doc")
        }
        .buttonStyle(.borderedProminent)
        .controlSize(.regular)
        .help(helpText)
        .accessibilityLabel(helpText)
        .accessibilityHint(
            LocalizedCopy.text("Copies the result and closes the popup.")
        )
        .keyboardShortcut("c", modifiers: EventModifiers.command)
    }
}

struct PopupResultHeader: View {
    let title: String
    let systemImage: String
    let detail: String?
    let detailAccessibilityLabel: String?
    let copyHelpText: String
    let copyAction: () -> Void

    var body: some View {
        HStack(spacing: 11) {
            Image(systemName: systemImage)
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(Color.accentColor)
                .frame(width: 32, height: 32)
                .background(
                    Color.accentColor.opacity(0.12),
                    in: RoundedRectangle(cornerRadius: 9, style: .continuous)
                )
                .accessibilityHidden(true)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.headline)
                    .accessibilityAddTraits(.isHeader)

                if let detail {
                    Text(detail)
                        .font(.caption.weight(.medium))
                        .foregroundStyle(.secondary)
                        .accessibilityLabel(detailAccessibilityLabel ?? detail)
                }
            }

            Spacer(minLength: 12)

            ResultCopyButton(helpText: copyHelpText, action: copyAction)
        }
    }
}

struct PopupResultSection<Content: View>: View {
    let title: String
    let isEmphasized: Bool
    private let content: Content

    init(
        title: String,
        isEmphasized: Bool = false,
        @ViewBuilder content: () -> Content
    ) {
        self.title = title
        self.isEmphasized = isEmphasized
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.caption.weight(.semibold))
                .foregroundStyle(isEmphasized ? Color.accentColor : .secondary)
                .accessibilityAddTraits(.isHeader)

            content
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(12)
        .background(
            isEmphasized
                ? Color.accentColor.opacity(0.085)
                : Color.primary.opacity(0.04),
            in: RoundedRectangle(cornerRadius: 10, style: .continuous)
        )
        .overlay {
            RoundedRectangle(cornerRadius: 10, style: .continuous)
                .stroke(
                    isEmphasized
                        ? Color.accentColor.opacity(0.18)
                        : Color.primary.opacity(0.065),
                    lineWidth: 1
                )
        }
    }
}
