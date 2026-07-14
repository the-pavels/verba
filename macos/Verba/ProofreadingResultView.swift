import SwiftUI

struct ProofreadingResultView: View {
    let originalText: String
    let correctedText: String
    let copyText: (String) -> Void

    var body: some View {
        let diff = ProofreadingDiff(original: originalText, corrected: correctedText)

        VStack(alignment: .leading, spacing: 14) {
            PopupResultHeader(
                title: LocalizedCopy.text("Proofreading"),
                systemImage: "checkmark.circle.fill",
                detail: nil,
                detailAccessibilityLabel: nil,
                copyHelpText: LocalizedCopy.text("Copy corrected text"),
                copyAction: {
                    copyText(correctedText)
                }
            )

            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    PopupResultSection(title: LocalizedCopy.text("Original")) {
                        ProofreadingDiffText(
                            segments: diff.original,
                            accessibilityLabel: "Original: \(originalText)"
                        )
                    }

                    PopupResultSection(
                        title: LocalizedCopy.text("Corrected text"),
                        isEmphasized: true
                    ) {
                        ProofreadingDiffText(
                            segments: diff.corrected,
                            accessibilityLabel: "Corrected text: \(correctedText)"
                        )
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

struct ProofreadingDiffText: View {
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
            text.foregroundColor = Color(nsColor: .labelColor)
            text.backgroundColor = Color(nsColor: .systemRed).opacity(0.16)
            text.strikethroughStyle = .single
        case .added:
            text.foregroundColor = Color(nsColor: .labelColor)
            text.backgroundColor = Color(nsColor: .systemGreen).opacity(0.18)
            text.font = .body.bold()
        }

        return Text(text)
    }
}
