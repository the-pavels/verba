import AppKit
import SwiftUI

struct MenuBarContentView: View {
    let rustCoreVersion: String

    var body: some View {
        Button("Translate Selected Text", action: unavailableCommand)
            .disabled(true)

        Button("Proofread Selected Text", action: unavailableCommand)
            .disabled(true)

        Divider()

        Button("Settings...", action: unavailableCommand)
            .disabled(true)

        Button("About Verba") {
            NSApplication.shared.orderFrontStandardAboutPanel()
        }

        Divider()

        Text("Rust core \(rustCoreVersion)")

        Button("Quit Verba") {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }

    private func unavailableCommand() {
        // Disabled commands are enabled only when their feature is connected.
    }
}
