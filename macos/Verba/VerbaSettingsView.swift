import SwiftUI

struct VerbaSettingsView: View {
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController
    @ObservedObject var accessibility: AccessibilityPermissionController
    @ObservedObject var support: SettingsSupportController

    var body: some View {
        Form {
            TargetLanguageSettingsView(controller: targetLanguage)
            ShortcutSettingsView(controller: shortcuts)
            ApiKeySettingsView(controller: apiKey)
            PrivacyAndSupportSettingsView(
                accessibility: accessibility,
                targetLanguage: targetLanguage,
                apiKey: apiKey,
                shortcuts: shortcuts,
                support: support
            )
        }
        .formStyle(.grouped)
        .frame(width: 520, height: 680)
        .task {
            await targetLanguage.load()
            await apiKey.load()
            shortcuts.load()
            accessibility.refresh()
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in
            accessibility.refresh()
        }
    }
}
