import Foundation

enum LocalizedCopy {
    static func text(_ key: String) -> String {
        Bundle.main.localizedString(forKey: key, value: key, table: nil)
    }

    static func format(_ key: String, _ arguments: CVarArg...) -> String {
        String(format: text(key), locale: .current, arguments: arguments)
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
