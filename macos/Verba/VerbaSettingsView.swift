import SwiftUI

struct VerbaSettingsView: View {
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController
    @ObservedObject var shortcuts: ShortcutSettingsController

    var body: some View {
        Form {
            TargetLanguageSettingsView(controller: targetLanguage)
            ShortcutSettingsView(controller: shortcuts)
            ApiKeySettingsView(controller: apiKey)
        }
        .formStyle(.grouped)
        .frame(width: 460, height: 500)
        .task {
            await targetLanguage.load()
            await apiKey.load()
            shortcuts.load()
        }
    }
}
