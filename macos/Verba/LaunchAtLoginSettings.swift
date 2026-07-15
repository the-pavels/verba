import ServiceManagement
import SwiftUI

enum LaunchAtLoginStatus: Equatable {
    case disabled
    case enabled
    case requiresApproval
    case unavailable
}

@MainActor
protocol LaunchAtLoginServicing: AnyObject {
    var status: LaunchAtLoginStatus { get }
    func register() throws
    func unregister() throws
    func openSystemSettings()
}

@MainActor
final class LaunchAtLoginController: ObservableObject {
    @Published private(set) var status: LaunchAtLoginStatus
    @Published private(set) var feedback: String?

    private let service: any LaunchAtLoginServicing

    convenience init() {
        self.init(service: SystemLaunchAtLoginService())
    }

    init(service: any LaunchAtLoginServicing) {
        self.service = service
        status = service.status
    }

    var isRequested: Bool {
        status == .enabled || status == .requiresApproval
    }

    var canChange: Bool {
        status != .unavailable
    }

    var statusMessage: String {
        switch status {
        case .disabled:
            LocalizedCopy.text("Verba opens only when you start it.")
        case .enabled:
            LocalizedCopy.text("Verba will open automatically when you log in.")
        case .requiresApproval:
            LocalizedCopy.text("Approve Verba in System Settings to launch it at login.")
        case .unavailable:
            LocalizedCopy.text("Launch at login is unavailable for this copy of Verba.")
        }
    }

    func refresh() {
        status = service.status
    }

    func setRequested(_ requested: Bool) {
        feedback = nil
        guard canChange else {
            return
        }

        do {
            if requested {
                guard !isRequested else {
                    return
                }
                try service.register()
            } else {
                guard isRequested else {
                    return
                }
                try service.unregister()
            }
            refresh()
        } catch {
            refresh()
            feedback = LocalizedCopy.text("Launch at login couldn’t be changed. Try again.")
        }
    }

    func openSystemSettings() {
        service.openSystemSettings()
    }
}

@MainActor
private final class SystemLaunchAtLoginService: LaunchAtLoginServicing {
    private let service = SMAppService.mainApp

    var status: LaunchAtLoginStatus {
        switch service.status {
        case .notRegistered:
            .disabled
        case .enabled:
            .enabled
        case .requiresApproval:
            .requiresApproval
        case .notFound:
            .unavailable
        @unknown default:
            .unavailable
        }
    }

    func register() throws {
        try service.register()
    }

    func unregister() throws {
        try service.unregister()
    }

    func openSystemSettings() {
        SMAppService.openSystemSettingsLoginItems()
    }
}

struct LaunchAtLoginSettingsView: View {
    @ObservedObject var controller: LaunchAtLoginController

    var body: some View {
        Section("General") {
            Toggle(
                LocalizedCopy.text("Launch Verba at login"),
                isOn: Binding(
                    get: { controller.isRequested },
                    set: { controller.setRequested($0) }
                )
            )
            .disabled(!controller.canChange)

            Text(controller.statusMessage)
                .foregroundStyle(.secondary)

            if controller.status == .requiresApproval {
                Button("Open Login Items Settings…") {
                    controller.openSystemSettings()
                }
            }

            if let feedback = controller.feedback {
                Label(feedback, systemImage: "exclamationmark.triangle")
                    .accessibilityLabel(
                        LocalizedCopy.format("Launch at login error: %@", feedback)
                    )
            }
        }
    }
}
