import SwiftUI

struct VerbaSettingsView: View {
    @ObservedObject var targetLanguage: TargetLanguageSettingsController
    @ObservedObject var apiKey: ApiKeySettingsController

    var body: some View {
        Form {
            TargetLanguageSettingsView(controller: targetLanguage)
            ApiKeySettingsView(controller: apiKey)
        }
        .formStyle(.grouped)
        .frame(width: 460, height: 350)
        .task {
            await targetLanguage.load()
            await apiKey.load()
        }
    }
}
