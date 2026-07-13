import AppKit
import SwiftUI

@main
struct VerbaApp: App {
    private let initialState = initialPresentation()

    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            MenuBarContentView(
                initialPresentation: initialState,
                rustCoreVersion: rustCoreVersion()
            )
        }
        .menuBarExtraStyle(.menu)
    }
}
