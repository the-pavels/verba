import AppKit
import SwiftUI

@main
@MainActor
struct VerbaApp: App {
    @StateObject private var accessibilityPermission: AccessibilityPermissionController
    @StateObject private var targetLanguageSettings: TargetLanguageSettingsController
    @StateObject private var apiKeySettings: ApiKeySettingsController
    @StateObject private var shortcutSettings: ShortcutSettingsController
    @StateObject private var settingsSupport: SettingsSupportController

    private let popupController: PopupController
    private let runtime: VerbaRuntime
    private let lifecycle: ApplicationLifecycleController
    private let performance: PerformanceSignposter

    init() {
        let performance = PerformanceSignposter()
        self.performance = performance
        let accessibilityPermission = AccessibilityPermissionController()
        let settingsSupport = SettingsSupportController(rustCoreVersion: rustCoreVersion())
        _accessibilityPermission = StateObject(wrappedValue: accessibilityPermission)
        _settingsSupport = StateObject(wrappedValue: settingsSupport)

        let translationSessions = SystemTranslationSessionProvider()
        let popupController = PopupController(translationSessions: translationSessions)
        popupController.onGrantAccessibility = { [weak accessibilityPermission] in
            guard let accessibilityPermission else {
                return
            }
            switch accessibilityPermission.status {
            case .notRequested:
                accessibilityPermission.requestPermission()
            case .denied:
                accessibilityPermission.openSystemSettings()
            case .granted:
                break
            }
        }
        popupController.onDiagnosticCode = { [weak settingsSupport] code in
            settingsSupport?.recordDiagnosticCode(code)
        }
        let translator = NativeAppleTranslator(
            translator: AppleTranslator(sessions: translationSessions)
        )
        self.popupController = popupController
        let runtime = VerbaRuntime(
            popupController: popupController,
            translator: translator,
            performance: performance
        )
        self.runtime = runtime
        lifecycle = ApplicationLifecycleController(
            runtime: runtime,
            popup: popupController,
            accessibilityPermission: accessibilityPermission
        )
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
        performance.startupReady()
    }

    var body: some Scene {
        MenuBarExtra("Verba", systemImage: "character.cursor.ibeam") {
            MenuBarContentView(
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
