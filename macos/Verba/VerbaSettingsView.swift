import SwiftUI

struct VerbaSettingsView: View {
    @ScaledMetric private var idealWidth = 520
    @ScaledMetric private var idealHeight = 680
    @ScaledMetric private var minimumWidth = 460
    @ScaledMetric private var minimumHeight = 560

    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController
    @ObservedObject var accessibility: AccessibilityPermissionController
    @ObservedObject var support: SettingsSupportController
    @ObservedObject var launchAtLogin: LaunchAtLoginController

    var body: some View {
        Form {
            LaunchAtLoginSettingsView(controller: launchAtLogin)
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
        .frame(
            minWidth: minimumWidth,
            idealWidth: idealWidth,
            minHeight: minimumHeight,
            idealHeight: idealHeight
        )
        .task {
            await targetLanguage.load()
            await apiKey.load()
            shortcuts.load()
            accessibility.refresh()
            launchAtLogin.refresh()
        }
        .onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
        ) { _ in
            accessibility.refresh()
            launchAtLogin.refresh()
        }
    }
}
