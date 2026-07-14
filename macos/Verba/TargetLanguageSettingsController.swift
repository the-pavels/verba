import Foundation
@preconcurrency import Translation

@MainActor
protocol TargetLanguagePreferenceManaging: AnyObject {
    func configureSupportedTargetLanguages(_ identifiers: [String]) throws -> String
    func setTargetLanguage(_ identifier: String) throws
}

@MainActor
protocol SupportedTranslationLanguagesProviding {
    func supportedLanguages() async -> [Locale.Language]
}

struct TargetLanguageOption: Equatable, Identifiable {
    let id: String
    let name: String
}

@MainActor
final class TargetLanguageSettingsController: ObservableObject {
    @Published private(set) var options: [TargetLanguageOption] = []
    @Published private(set) var selectedIdentifier = ""
    @Published private(set) var isLoading = false
    @Published private(set) var errorMessage: String?

    private let preferences: any TargetLanguagePreferenceManaging
    private let languages: any SupportedTranslationLanguagesProviding
    private let locale: Locale

    init(
        preferences: any TargetLanguagePreferenceManaging,
        languages: any SupportedTranslationLanguagesProviding = SystemSupportedTranslationLanguages(),
        locale: Locale = .current
    ) {
        self.preferences = preferences
        self.languages = languages
        self.locale = locale
    }

    func load() async {
        guard !isLoading, options.isEmpty else {
            return
        }

        isLoading = true
        errorMessage = nil
        let options = languageOptions(await languages.supportedLanguages())

        do {
            guard !options.isEmpty else {
                throw TargetLanguageSettingsError.noSupportedLanguages
            }
            let selected = try preferences.configureSupportedTargetLanguages(
                options.map(\.id)
            )
            guard options.contains(where: { $0.id == selected }) else {
                throw TargetLanguageSettingsError.invalidSelection
            }

            self.options = options
            selectedIdentifier = selected
        } catch {
            errorMessage = LocalizedCopy.text("Target languages are unavailable. Try again.")
        }

        isLoading = false
    }

    func select(_ identifier: String) {
        guard options.contains(where: { $0.id == identifier }) else {
            return
        }

        do {
            try preferences.setTargetLanguage(identifier)
            selectedIdentifier = identifier
            errorMessage = nil
        } catch {
            errorMessage = LocalizedCopy.text("The target language couldn’t be saved.")
        }
    }

    private func languageOptions(
        _ languages: [Locale.Language]
    ) -> [TargetLanguageOption] {
        var identifiers = Set<String>()
        return languages
            .compactMap { language in
                let identifier = language.minimalIdentifier
                guard identifiers.insert(identifier).inserted else {
                    return nil
                }
                return TargetLanguageOption(
                    id: identifier,
                    name: locale.localizedString(forIdentifier: identifier) ?? identifier
                )
            }
            .sorted {
                let order = $0.name.localizedStandardCompare($1.name)
                return order == .orderedSame ? $0.id < $1.id : order == .orderedAscending
            }
    }
}

@MainActor
private struct SystemSupportedTranslationLanguages: SupportedTranslationLanguagesProviding {
    func supportedLanguages() async -> [Locale.Language] {
        await LanguageAvailability().supportedLanguages
    }
}

private enum TargetLanguageSettingsError: Error {
    case noSupportedLanguages
    case invalidSelection
}
