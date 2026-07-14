import AppKit
import SwiftUI

struct MenuBarContentView: View {
    @Environment(\.openSettings) private var openSettings

    let initialPresentation: PresentationViewModel
    let rustCoreVersion: String
    @ObservedObject var accessibilityPermission: AccessibilityPermissionController
    let presentPopupPreview: (PresentationViewModel) -> Void

    var body: some View {
        menuContent
            .onAppear {
                accessibilityPermission.refresh()
            }
            .onReceive(
                NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification)
            ) { _ in
                accessibilityPermission.refresh()
            }
    }

    @ViewBuilder
    private var menuContent: some View {
        Button("Translate Selected Text") {}
            .disabled(true)

        Button("Proofread Selected Text") {}
            .disabled(true)

        Divider()

        Label(
            "Accessibility: \(accessibilityPermission.status.title)",
            systemImage: accessibilityPermission.status.systemImage
        )

        switch accessibilityPermission.status {
        case .notRequested:
            Text("Required to copy selected text from other applications.")

            Button("Request Accessibility Access...") {
                accessibilityPermission.requestPermission()
            }
        case .denied:
            Text("Allow Verba in Privacy & Security to capture selected text.")

            Button("Open Accessibility Settings...") {
                accessibilityPermission.openSystemSettings()
            }
        case .granted:
            EmptyView()
        }

        Divider()

        Button("Settings...") {
            NSApplication.shared.activate()
            openSettings()
        }

        Button("About Verba") {
            NSApplication.shared.orderFrontStandardAboutPanel()
        }

#if DEBUG
        Menu("Preview Popup") {
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

        Text("Rust core \(rustCoreVersion) - \(initialPresentation.diagnosticName)")

        Button("Quit Verba") {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }
}

private extension PresentationViewModel {
    var diagnosticName: String {
        switch self {
        case .idle:
            "idle"
        case .loading:
            "loading"
        case .proofreadingDisclosure:
            "proofreading disclosure"
        case .translation:
            "translation"
        case .proofreading:
            "proofreading"
        case .noIssues:
            "no issues"
        case .error:
            "error"
        }
    }
}
