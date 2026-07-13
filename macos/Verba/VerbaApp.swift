import AppKit
import SwiftUI

@main
struct VerbaApp: App {
    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            MenuBarContentView(rustCoreVersion: rustCoreVersion())
        }
        .menuBarExtraStyle(.menu)
    }
}
