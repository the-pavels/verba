import Sparkle
import SwiftUI

struct AutomaticUpdateConfiguration: Equatable {
    let feedURL: URL
    let publicKey: String

    init?(feedURLString: String?, publicKey: String?) {
        guard let feedURLString,
              let feedURL = URL(string: feedURLString),
              feedURL.scheme == "https",
              let publicKey,
              Data(base64Encoded: publicKey)?.count == 32 else {
            return nil
        }
        self.feedURL = feedURL
        self.publicKey = publicKey
    }

    init?(bundle: Bundle) {
        self.init(
            feedURLString: bundle.object(forInfoDictionaryKey: "SUFeedURL") as? String,
            publicKey: bundle.object(forInfoDictionaryKey: "SUPublicEDKey") as? String
        )
    }
}

@MainActor
protocol AutomaticUpdateEngine: AnyObject {
    var canCheckForUpdates: Bool { get }
    var automaticallyChecksForUpdates: Bool { get set }
    func checkForUpdates()
}

@MainActor
final class AutomaticUpdateController: ObservableObject {
    @Published private(set) var canCheckForUpdates = false
    @Published private(set) var automaticallyChecksForUpdates = false

    private let engine: (any AutomaticUpdateEngine)?

    convenience init(bundle: Bundle = .main) {
        let engine: (any AutomaticUpdateEngine)? =
            AutomaticUpdateConfiguration(bundle: bundle) == nil
                ? nil
                : SparkleAutomaticUpdateEngine()
        self.init(engine: engine)
    }

    init(engine: (any AutomaticUpdateEngine)?) {
        self.engine = engine
        refresh()
    }

    var isAvailable: Bool {
        engine != nil
    }

    func refresh() {
        canCheckForUpdates = engine?.canCheckForUpdates ?? false
        automaticallyChecksForUpdates = engine?.automaticallyChecksForUpdates ?? false
    }

    func checkForUpdates() {
        guard let engine, engine.canCheckForUpdates else {
            refresh()
            return
        }
        engine.checkForUpdates()
        refresh()
    }

    func setAutomaticallyChecksForUpdates(_ enabled: Bool) {
        guard let engine else {
            return
        }
        engine.automaticallyChecksForUpdates = enabled
        refresh()
    }
}

@MainActor
private final class SparkleAutomaticUpdateEngine: AutomaticUpdateEngine {
    private let controller = SPUStandardUpdaterController(
        startingUpdater: true,
        updaterDelegate: nil,
        userDriverDelegate: nil
    )

    var canCheckForUpdates: Bool {
        controller.updater.canCheckForUpdates
    }

    var automaticallyChecksForUpdates: Bool {
        get { controller.updater.automaticallyChecksForUpdates }
        set { controller.updater.automaticallyChecksForUpdates = newValue }
    }

    func checkForUpdates() {
        controller.checkForUpdates(nil)
    }
}

struct AutomaticUpdateSettingsView: View {
    @ObservedObject var controller: AutomaticUpdateController

    var body: some View {
        Section("Updates") {
            Toggle(
                LocalizedCopy.text("Check for updates automatically"),
                isOn: Binding(
                    get: { controller.automaticallyChecksForUpdates },
                    set: { controller.setAutomaticallyChecksForUpdates($0) }
                )
            )
            .disabled(!controller.isAvailable)

            Button("Check for Updates…") {
                controller.checkForUpdates()
            }
            .disabled(!controller.canCheckForUpdates)

            Text(statusMessage)
                .foregroundStyle(.secondary)
        }
    }

    private var statusMessage: String {
        controller.isAvailable
            ? LocalizedCopy.text(
                "Updates are verified with Developer ID and a separate update-signing key."
            )
            : LocalizedCopy.text("Update checking is unavailable in this build.")
    }
}
