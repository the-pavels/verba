import AppKit
import SwiftUI

struct MenuBarContentView: View {
    let initialPresentation: PresentationViewModel
    let rustCoreVersion: String
    let showPopupPreview: () -> Void

    var body: some View {
        Button("Translate Selected Text", action: unavailableCommand)
            .disabled(true)

        Button("Proofread Selected Text", action: unavailableCommand)
            .disabled(true)

        Divider()

        Button("Settings...", action: unavailableCommand)
            .disabled(true)

        Button("About Verba") {
            NSApplication.shared.orderFrontStandardAboutPanel()
        }

#if DEBUG
        Button("Show Loading Popup", action: showPopupPreview)
#endif

        Divider()

        Text("Rust core \(rustCoreVersion) - \(initialPresentation.diagnosticName)")

        Button("Quit Verba") {
            NSApplication.shared.terminate(nil)
        }
        .keyboardShortcut("q")
    }

    private func unavailableCommand() {
        // Disabled commands are enabled only when their feature is connected.
    }
}

private extension PresentationViewModel {
    var diagnosticName: String {
        switch self {
        case .idle:
            "idle"
        case .loading:
            "loading"
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
