import AppKit
import SwiftUI

@main
@MainActor
struct VerbaApp: App {
    private let initialState = initialPresentation()
    private let popupController = PopupController()

    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            MenuBarContentView(
                initialPresentation: initialState,
                rustCoreVersion: rustCoreVersion(),
                showPopupPreview: {
                    popupController.present(.loading(action: .translate))
                }
            )
        }
        .menuBarExtraStyle(.menu)
    }
}
