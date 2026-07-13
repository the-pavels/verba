import Foundation

@MainActor
protocol ApiKeySettingsManaging: AnyObject {
    func isApiKeyConfigured() throws -> Bool
    func saveApiKey(_ apiKey: String) throws
    func deleteApiKey() throws
    func testApiKeyConnection() async throws
}

enum ApiKeySettingsFailure: Error {
    case invalidApiKey
    case notConfigured
    case keychainUnavailable
    case authentication
    case rateLimited
    case quotaExceeded
    case offline
    case timedOut
    case serviceUnavailable
    case invalidResponse
    case connectionFailed
}

struct ApiKeySettingsFeedback: Equatable {
    enum Kind: Equatable {
        case success
        case error
    }

    let kind: Kind
    let message: String
}

@MainActor
final class ApiKeySettingsController: ObservableObject {
    @Published var apiKeyInput = ""
    @Published private(set) var isConfigured = false
    @Published private(set) var isLoading = false
    @Published private(set) var isTesting = false
    @Published private(set) var feedback: ApiKeySettingsFeedback?

    private let settings: any ApiKeySettingsManaging

    init(settings: any ApiKeySettingsManaging) {
        self.settings = settings
    }

    var canSave: Bool {
        !apiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
            && !isLoading
            && !isTesting
    }

    func load() async {
        guard !isLoading else {
            return
        }

        isLoading = true
        defer { isLoading = false }
        do {
            isConfigured = try settings.isApiKeyConfigured()
            feedback = nil
        } catch {
            feedback = feedback(for: error)
        }
    }

    func save() {
        guard canSave else {
            if apiKeyInput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                feedback = ApiKeySettingsFeedback(
                    kind: .error,
                    message: "Enter an OpenAI API key first."
                )
            }
            return
        }

        do {
            try settings.saveApiKey(apiKeyInput)
            apiKeyInput = ""
            isConfigured = true
            feedback = ApiKeySettingsFeedback(
                kind: .success,
                message: "API key saved in Keychain."
            )
        } catch {
            feedback = feedback(for: error)
        }
    }

    func delete() {
        guard isConfigured, !isTesting else {
            return
        }

        do {
            try settings.deleteApiKey()
            apiKeyInput = ""
            isConfigured = false
            feedback = ApiKeySettingsFeedback(
                kind: .success,
                message: "API key deleted from Keychain."
            )
        } catch {
            feedback = feedback(for: error)
        }
    }

    func testConnection() async {
        guard isConfigured, !isTesting else {
            if !isConfigured {
                feedback = feedback(for: ApiKeySettingsFailure.notConfigured)
            }
            return
        }

        isTesting = true
        defer { isTesting = false }
        do {
            try await settings.testApiKeyConnection()
            feedback = ApiKeySettingsFeedback(
                kind: .success,
                message: "Connection to OpenAI succeeded."
            )
        } catch {
            feedback = feedback(for: error)
        }
    }

    private func feedback(for error: any Error) -> ApiKeySettingsFeedback {
        let message = switch error as? ApiKeySettingsFailure {
        case .invalidApiKey:
            "Enter a valid OpenAI API key."
        case .notConfigured:
            "Save an API key before testing the connection."
        case .keychainUnavailable:
            "Keychain is unavailable. Quit and reopen Verba, then try again."
        case .authentication:
            "OpenAI rejected this key. Replace it and try again."
        case .rateLimited:
            "OpenAI is rate limiting requests. Wait a moment and try again."
        case .quotaExceeded:
            "This OpenAI account has no available quota. Check its billing and usage limits."
        case .offline:
            "No internet connection is available. Reconnect and try again."
        case .timedOut:
            "The connection to OpenAI timed out. Try again."
        case .serviceUnavailable:
            "OpenAI is temporarily unavailable. Try again later."
        case .invalidResponse:
            "OpenAI returned an unexpected response. Try again."
        case .connectionFailed, .none:
            "The OpenAI connection couldn’t be tested. Try again."
        }
        return ApiKeySettingsFeedback(kind: .error, message: message)
    }
}
