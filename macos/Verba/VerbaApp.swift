import AppKit
import SwiftUI

@main
@MainActor
struct VerbaApp: App {
    @StateObject private var accessibilityPermission = AccessibilityPermissionController()
    @StateObject private var targetLanguageSettings: TargetLanguageSettingsController
    @StateObject private var apiKeySettings: ApiKeySettingsController
    @StateObject private var shortcutSettings: ShortcutSettingsController
    @StateObject private var settingsSupport: SettingsSupportController

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
        let runtime = VerbaRuntime(
            popupController: popupController,
            translator: translator
        )
        self.runtime = runtime
        let targetLanguageSettings = TargetLanguageSettingsController(preferences: runtime)
        _targetLanguageSettings = StateObject(
            wrappedValue: targetLanguageSettings
        )
        _apiKeySettings = StateObject(
            wrappedValue: ApiKeySettingsController(settings: runtime)
        )
        _shortcutSettings = StateObject(
            wrappedValue: ShortcutSettingsController(settings: runtime)
        )
        _settingsSupport = StateObject(
            wrappedValue: SettingsSupportController(rustCoreVersion: rustCoreVersion())
        )
        Task {
            await targetLanguageSettings.load()
        }
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

        Settings {
            VerbaSettingsView(
                targetLanguage: targetLanguageSettings,
                apiKey: apiKeySettings,
                shortcuts: shortcutSettings,
                accessibility: accessibilityPermission,
                support: settingsSupport
            )
        }
    }
}
