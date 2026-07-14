import SwiftUI

struct ResultCopyButton: View {
    let helpText: String
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label("Copy", systemImage: "doc.on.doc")
        }
        .buttonStyle(.bordered)
        .controlSize(.small)
        .help(helpText)
        .keyboardShortcut("c", modifiers: EventModifiers.command)
    }
}
