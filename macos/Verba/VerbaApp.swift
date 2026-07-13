import AppKit
import SwiftUI

@main
struct VerbaApp: App {
    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            Button("Quit Verba") {
                NSApplication.shared.terminate(nil)
            }
            .keyboardShortcut("q")
        }
        .menuBarExtraStyle(.menu)
    }
}
