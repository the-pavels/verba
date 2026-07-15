import Foundation

enum LocalizedCopy {
    static func text(_ key: String) -> String {
        Bundle.main.localizedString(forKey: key, value: key, table: nil)
    }

    static func format(_ key: String, _ arguments: CVarArg...) -> String {
        String(format: text(key), locale: .current, arguments: arguments)
    }
}

enum AccessibilityCopy {
    static func setupProgress(current: Int, total: Int) -> String {
        LocalizedCopy.format("Step %d of %d", current, total)
    }

    static func setting(title: String, value: String) -> String {
        LocalizedCopy.format("%@: %@", title, value)
    }

    static func originalText(_ text: String) -> String {
        LocalizedCopy.format("Original text: %@", text)
    }

    static func translationText(_ text: String) -> String {
        LocalizedCopy.format("Translation text: %@", text)
    }

    static func correctedText(_ text: String) -> String {
        LocalizedCopy.format("Corrected text: %@", text)
    }

    static func apiKeyStatus(isConfigured: Bool) -> String {
        LocalizedCopy.text(
            isConfigured ? "API key stored in Keychain" : "No API key configured"
        )
    }
}

extension PresentationViewModel {
    var localizedForDisplay: PresentationViewModel {
        guard case let .error(action, title, message, recovery, diagnosticCode) = self else {
            return self
        }
        return .error(
            action: action,
            title: LocalizedCopy.text(title),
            message: LocalizedCopy.text(message),
            recovery: recovery,
            diagnosticCode: diagnosticCode
        )
    }
}
