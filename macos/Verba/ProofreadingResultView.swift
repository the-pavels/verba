import SwiftUI

struct ProofreadingResultView: View {
    let originalText: String
    let correctedText: String
    let explanation: String
    let copyText: (String) -> Void

    var body: some View {
        let diff = ProofreadingDiff(original: originalText, corrected: correctedText)

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
                        Text(LocalizedCopy.text("Original").uppercased())
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .accessibilityAddTraits(.isHeader)

                        ProofreadingDiffText(
                            segments: diff.original,
                            accessibilityLabel: "Original: \(originalText)"
                        )
                    }

                    VStack(alignment: .leading, spacing: 5) {
                        Text("CORRECTED TEXT")
                            .font(.caption2.weight(.semibold))
                            .foregroundStyle(.secondary)
                            .accessibilityAddTraits(.isHeader)

                        ProofreadingDiffText(
                            segments: diff.corrected,
                            accessibilityLabel: "Corrected text: \(correctedText)"
                        )
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

struct ProofreadingDiff: Equatable {
    enum Change: Equatable {
        case unchanged
        case removed
        case added
    }

    struct Segment: Equatable {
        let text: String
        let change: Change
    }

    let original: [Segment]
    let corrected: [Segment]

    init(original: String, corrected: String) {
        let originalTokens = Self.tokens(in: original)
        let correctedTokens = Self.tokens(in: corrected)
        let difference = correctedTokens.difference(from: originalTokens)
        var removedOffsets = Set<Int>()
        var addedOffsets = Set<Int>()

        for change in difference {
            switch change {
            case let .remove(offset, _, _):
                removedOffsets.insert(offset)
            case let .insert(offset, _, _):
                addedOffsets.insert(offset)
            }
        }

        self.original = Self.segments(
            from: originalTokens,
            changedOffsets: removedOffsets,
            changedKind: .removed
        )
        self.corrected = Self.segments(
            from: correctedTokens,
            changedOffsets: addedOffsets,
            changedKind: .added
        )
    }

    private static func tokens(in text: String) -> [String] {
        var tokens: [String] = []
        var current = ""
        var currentKind: TokenKind?

        for character in text {
            let kind = TokenKind(character)
            if kind != currentKind, !current.isEmpty {
                tokens.append(current)
                current = ""
            }
            current.append(character)
            currentKind = kind
        }

        if !current.isEmpty {
            tokens.append(current)
        }
        return tokens
    }

    private static func segments(
        from tokens: [String],
        changedOffsets: Set<Int>,
        changedKind: Change
    ) -> [Segment] {
        tokens.enumerated().reduce(into: []) { segments, item in
            let change: Change = changedOffsets.contains(item.offset) ? changedKind : .unchanged
            if segments.last?.change == change {
                let previous = segments.removeLast()
                segments.append(Segment(text: previous.text + item.element, change: change))
            } else {
                segments.append(Segment(text: item.element, change: change))
            }
        }
    }

    private enum TokenKind: Equatable {
        case word
        case whitespace
        case punctuation

        init(_ character: Character) {
            if character.isWhitespace {
                self = .whitespace
            } else if character.isLetter || character.isNumber {
                self = .word
            } else {
                self = .punctuation
            }
        }
    }
}

private struct ProofreadingDiffText: View {
    let segments: [ProofreadingDiff.Segment]
    let accessibilityLabel: String

    var body: some View {
        segments.reduce(Text("")) { result, segment in
            result + styledText(for: segment)
        }
        .font(.body)
        .fixedSize(horizontal: false, vertical: true)
        .textSelection(.enabled)
        .accessibilityLabel(accessibilityLabel)
    }

    private func styledText(for segment: ProofreadingDiff.Segment) -> Text {
        var text = AttributedString(segment.text)

        switch segment.change {
        case .unchanged:
            break
        case .removed:
            text.foregroundColor = .red
            text.backgroundColor = Color.red.opacity(0.14)
            text.strikethroughStyle = .single
        case .added:
            text.foregroundColor = .green
            text.backgroundColor = Color.green.opacity(0.14)
            text.font = .body.bold()
        }

        return Text(text)
    }
}
