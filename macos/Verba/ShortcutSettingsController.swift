import SwiftUI

enum ShortcutPreferenceAction {
    case translate
    case proofread
}

struct RecordedShortcut: Equatable {
    let key: String
    let command: Bool
    let control: Bool
    let option: Bool
    let shift: Bool
}

struct ShortcutDisplayConfiguration: Equatable {
    let translate: String
    let proofread: String
}

enum ShortcutSettingsFailure: Error {
    case invalidKey
    case missingPrimaryModifier
    case reservedShortcut
    case duplicateShortcut
    case shortcutUnavailable
    case registrationFailed
    case persistenceFailed
    case rollbackFailed
}

@MainActor
protocol ShortcutSettingsManaging: AnyObject {
    func shortcutConfiguration() throws -> ShortcutDisplayConfiguration
    func setShortcut(
        _ shortcut: RecordedShortcut,
        for action: ShortcutPreferenceAction
    ) throws -> ShortcutDisplayConfiguration
}

@MainActor
final class ShortcutSettingsController: ObservableObject {
    @Published private(set) var translate = ""
    @Published private(set) var proofread = ""
    @Published private(set) var errorMessage: String?

    private let settings: any ShortcutSettingsManaging

    init(settings: any ShortcutSettingsManaging) {
        self.settings = settings
    }

    func load() {
        do {
            apply(try settings.shortcutConfiguration())
            errorMessage = nil
        } catch {
            errorMessage = "Shortcuts are unavailable. Reopen Verba and try again."
        }
    }

    func record(_ shortcut: RecordedShortcut, for action: ShortcutPreferenceAction) {
        do {
            apply(try settings.setShortcut(shortcut, for: action))
            errorMessage = nil
        } catch let failure as ShortcutSettingsFailure {
            errorMessage = message(for: failure)
        } catch {
            errorMessage = "The shortcut couldn’t be changed. Your previous shortcut is still active."
        }
    }

    private func apply(_ configuration: ShortcutDisplayConfiguration) {
        translate = configuration.translate
        proofread = configuration.proofread
    }

    private func message(for failure: ShortcutSettingsFailure) -> String {
        switch failure {
        case .invalidKey:
            "Use a letter, number, punctuation key, function key, or navigation key."
        case .missingPrimaryModifier:
            "Include Command, Control, or Option in the shortcut."
        case .reservedShortcut:
            "That shortcut is reserved by macOS. Choose another combination."
        case .duplicateShortcut:
            "That shortcut is already assigned to another Verba action."
        case .shortcutUnavailable:
            "That shortcut is already used by another app. Your previous shortcut is still active."
        case .registrationFailed:
            "The shortcut couldn’t be registered. Your previous shortcut is still active."
        case .persistenceFailed:
            "The shortcut couldn’t be saved. Your previous shortcut is still active."
        case .rollbackFailed:
            "Shortcut recovery failed. Reopen Verba to restore your saved shortcuts."
        }
    }
}

struct ShortcutSettingsView: View {
    @ObservedObject var controller: ShortcutSettingsController

    var body: some View {
        Section("Shortcuts") {
            LabeledContent("Translate") {
                ShortcutRecorderView(
                    value: controller.translate,
                    accessibilityName: "Translate shortcut"
                ) {
                    controller.record($0, for: .translate)
                }
                .frame(width: 140, height: 24)
            }

            LabeledContent("Proofread") {
                ShortcutRecorderView(
                    value: controller.proofread,
                    accessibilityName: "Proofread shortcut"
                ) {
                    controller.record($0, for: .proofread)
                }
                .frame(width: 140, height: 24)
            }

            Text("Click a shortcut, then press the new key combination.")
                .font(.caption)
                .foregroundStyle(.secondary)

            if let errorMessage = controller.errorMessage {
                Text(errorMessage)
                    .font(.caption)
                    .foregroundStyle(.red)
                    .accessibilityLabel("Shortcut error: \(errorMessage)")
            }
        }
    }
}
