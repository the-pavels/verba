import AppKit
import Combine
@preconcurrency import ApplicationServices

enum AccessibilityPermissionStatus: Equatable {
    case notRequested
    case denied
    case granted
}

protocol AccessibilityTrustChecking {
    func isTrusted(prompt: Bool) -> Bool
}

protocol AccessibilityPromptHistory {
    var hasRequestedPermission: Bool { get }
    func markPermissionRequested()
}

protocol AccessibilitySettingsOpening {
    func openAccessibilitySettings()
}

@MainActor
final class AccessibilityPermissionController: ObservableObject {
    @Published private(set) var status: AccessibilityPermissionStatus = .notRequested

    private let trustChecker: any AccessibilityTrustChecking
    private let promptHistory: any AccessibilityPromptHistory
    private let settingsOpener: any AccessibilitySettingsOpening

    convenience init() {
        self.init(
            trustChecker: SystemAccessibilityTrustChecker(),
            promptHistory: UserDefaultsAccessibilityPromptHistory(),
            settingsOpener: SystemAccessibilitySettingsOpener()
        )
    }

    init(
        trustChecker: any AccessibilityTrustChecking,
        promptHistory: any AccessibilityPromptHistory,
        settingsOpener: any AccessibilitySettingsOpening
    ) {
        self.trustChecker = trustChecker
        self.promptHistory = promptHistory
        self.settingsOpener = settingsOpener
        refresh()
    }

    func refresh() {
        if trustChecker.isTrusted(prompt: false) {
            status = .granted
        } else if promptHistory.hasRequestedPermission {
            status = .denied
        } else {
            status = .notRequested
        }
    }

    func requestPermission() {
        guard status == .notRequested else {
            return
        }

        promptHistory.markPermissionRequested()
        _ = trustChecker.isTrusted(prompt: true)
        refresh()
    }

    func openSystemSettings() {
        settingsOpener.openAccessibilitySettings()
    }
}

private struct SystemAccessibilityTrustChecker: AccessibilityTrustChecking {
    func isTrusted(prompt: Bool) -> Bool {
        guard prompt else {
            return AXIsProcessTrustedWithOptions(nil)
        }

        let options = [
            kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true,
        ] as CFDictionary
        return AXIsProcessTrustedWithOptions(options)
    }
}

private struct UserDefaultsAccessibilityPromptHistory: AccessibilityPromptHistory {
    private static let key = "hasRequestedAccessibilityPermission"

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    var hasRequestedPermission: Bool {
        defaults.bool(forKey: Self.key)
    }

    func markPermissionRequested() {
        defaults.set(true, forKey: Self.key)
    }
}

private struct SystemAccessibilitySettingsOpener: AccessibilitySettingsOpening {
    private static let url = URL(
        string: "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
    )

    func openAccessibilitySettings() {
        guard let url = Self.url else {
            return
        }

        NSWorkspace.shared.open(url)
    }
}
