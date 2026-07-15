import AppKit
import SwiftUI

struct MenuBarContentView: View {
    @Environment(\.openSettings) private var openSettings

    @ObservedObject var accessibilityPermission: AccessibilityPermissionController
    @ObservedObject var automaticUpdates: AutomaticUpdateController
    let presentPopupPreview: (PresentationViewModel) -> Void

    var body: some View {
        menuContent
            .onAppear {
                accessibilityPermission.refresh()
                automaticUpdates.refresh()
            }
            .onReceive(
                NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
            ) { _ in
                accessibilityPermission.refresh()
            }
    }

    @ViewBuilder
    private var menuContent: some View {
        let permission = accessibilityPermission.status.menuPresentation

        Label(permission.title, systemImage: permission.systemImage)

        if let message = permission.message {
            Text(message)
        }

        if let action = permission.action {
            Button(action.title) {
                perform(action)
            }
        }

        Divider()

        Button("Settings…") {
            NSApplication.shared.activate()
            openSettings()
        }

        Button("Check for Updates…") {
            automaticUpdates.checkForUpdates()
        }
        .disabled(!automaticUpdates.canCheckForUpdates)

        Button("About Verba") {
            NSApplication.shared.orderFrontStandardAboutPanel()
        }

#if DEBUG
        Menu("Popup Previews") {
            Button("Translation Loading") {
                presentPopupPreview(.loading(action: .translate))
            }

            Button("Proofreading Loading") {
                presentPopupPreview(.loading(action: .proofread))
            }

            Button("Proofreading Disclosure") {
                presentPopupPreview(.proofreadingDisclosure)
            }

            Divider()

            Button("Translation Result") {
                presentPopupPreview(.translationPreview)
            }

            Button("Proofreading Result") {
                presentPopupPreview(.proofreadingPreview)
            }

            Button("No Issues Found") {
                presentPopupPreview(.noIssues)
            }

            Button("Error") {
                presentPopupPreview(.errorPreview)
            }
        }
#endif

        Divider()

        Button("Quit Verba") {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }
    private func perform(_ action: MenuBarPermissionAction) {
        switch action {
        case .requestAccess:
            accessibilityPermission.requestPermission()
        case .openSettings:
            accessibilityPermission.openSystemSettings()
        }
    }
}

struct MenuBarPermissionPresentation: Equatable {
    let title: String
    let systemImage: String
    let message: String?
    let action: MenuBarPermissionAction?
}

enum MenuBarPermissionAction: Equatable {
    case requestAccess
    case openSettings

    var title: String {
        switch self {
        case .requestAccess:
            LocalizedCopy.text("Enable Accessibility…")
        case .openSettings:
            LocalizedCopy.text("Open Accessibility Settings…")
        }
    }
}

extension AccessibilityPermissionStatus {
    var menuPresentation: MenuBarPermissionPresentation {
        switch self {
        case .notRequested:
            MenuBarPermissionPresentation(
                title: LocalizedCopy.text("Accessibility access required"),
                systemImage: systemImage,
                message: LocalizedCopy.text(
                    "Verba needs Accessibility access to read selected text in other apps."
                ),
                action: .requestAccess
            )
        case .denied:
            MenuBarPermissionPresentation(
                title: LocalizedCopy.text("Accessibility access required"),
                systemImage: systemImage,
                message: LocalizedCopy.text(
                    "Enable Verba in Privacy & Security to read selected text."
                ),
                action: .openSettings
            )
        case .granted:
            MenuBarPermissionPresentation(
                title: LocalizedCopy.text("Verba is ready"),
                systemImage: systemImage,
                message: nil,
                action: nil
            )
        }
    }
}
