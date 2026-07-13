import AppKit
import SwiftUI

@main
@MainActor
struct VerbaApp: App {
    @StateObject private var accessibilityPermission = AccessibilityPermissionController()

    private let initialState = initialPresentation()
    private let popupController: PopupController
    private let runtime: VerbaRuntime

    init() {
        let translationSessions = SystemTranslationSessionProvider()
        let popupController = PopupController(translationSessions: translationSessions)
        let translator = NativeAppleTranslator(
            translator: AppleTranslator(sessions: translationSessions)
        )
        self.popupController = popupController
        runtime = VerbaRuntime(
            popupController: popupController,
            translator: translator
        )
    }

    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            MenuBarContentView(
                initialPresentation: initialState,
                rustCoreVersion: rustCoreVersion(),
                accessibilityPermission: accessibilityPermission,
                presentPopupPreview: { presentation in
                    popupController.present(presentation)
                }
            )
        }
        .menuBarExtraStyle(.menu)
    }
}
