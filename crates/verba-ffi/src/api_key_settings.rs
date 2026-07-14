use std::{error::Error, fmt, sync::Arc};

use verba_core::{
    coordinator::CancellationToken,
    proofreading::ProofreaderError,
    secrets::{SecretStore, SecretStoreError},
};
use verba_macos::{MacOsSecretStore, MacOsSecretStoreBuildError};
use verba_openai::{
    ApiKeyProvider, ApiKeyProviderError, OpenAiClient, OpenAiConfig, OpenAiConnectionTester,
};

const KEYCHAIN_SERVICE: &str = "io.github.the-pavels.verba";
const KEYCHAIN_ACCOUNT: &str = "openai-api-key";

pub(crate) fn openai_secret_store() -> Result<Arc<MacOsSecretStore>, MacOsSecretStoreBuildError> {
    MacOsSecretStore::new(KEYCHAIN_SERVICE, KEYCHAIN_ACCOUNT).map(Arc::new)
}

pub(crate) struct SecretStoreApiKeyProvider {
    secret_store: Arc<dyn SecretStore>,
}

impl SecretStoreApiKeyProvider {
    pub(crate) fn new(secret_store: Arc<dyn SecretStore>) -> Self {
        Self { secret_store }
    }
}

impl ApiKeyProvider for SecretStoreApiKeyProvider {
    fn load_api_key(&self) -> Result<String, ApiKeyProviderError> {
        self.secret_store
            .load()
            .map_err(|_| ApiKeyProviderError::Unavailable)?
            .ok_or(ApiKeyProviderError::Missing)
    }
}

#[derive(uniffi::Object)]
pub struct OpenAiApiKeySettings {
    secret_store: Arc<dyn SecretStore>,
    connection_tester: Arc<dyn ConnectionTesting>,
}

#[uniffi::export]
impl OpenAiApiKeySettings {
    #[uniffi::constructor]
    pub fn new() -> Result<Arc<Self>, OpenAiApiKeyError> {
        let secret_store =
            openai_secret_store().map_err(|_| OpenAiApiKeyError::KeychainUnavailable)?;
        let client = OpenAiClient::new(OpenAiConfig::default())
            .map_err(|_| OpenAiApiKeyError::ConnectionFailed)?;

        Ok(Arc::new(Self {
            secret_store,
            connection_tester: Arc::new(ProductionConnectionTester(OpenAiConnectionTester::new(
                Arc::new(client),
            ))),
        }))
    }

    pub fn is_configured(&self) -> Result<bool, OpenAiApiKeyError> {
        self.secret_store
            .load()
            .map(|secret| secret.is_some())
            .map_err(map_secret_store_error)
    }

    pub fn save(&self, api_key: String) -> Result<(), OpenAiApiKeyError> {
        let api_key = api_key.trim();
        if api_key.is_empty() {
            return Err(OpenAiApiKeyError::InvalidApiKey);
        }
        self.secret_store
            .save(api_key)
            .map_err(map_secret_store_error)
    }

    pub fn delete(&self) -> Result<(), OpenAiApiKeyError> {
        self.secret_store.delete().map_err(map_secret_store_error)
    }

    pub async fn test_connection(&self) -> Result<(), OpenAiApiKeyError> {
        let api_key = self
            .secret_store
            .load()
            .map_err(map_secret_store_error)?
            .ok_or(OpenAiApiKeyError::NotConfigured)?;
        self.connection_tester
            .test(&api_key, &CancellationToken::default())
            .await
            .map_err(map_connection_error)
    }
}

#[async_trait::async_trait]
trait ConnectionTesting: Send + Sync {
    async fn test(
        &self,
        api_key: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ProofreaderError>;
}

struct ProductionConnectionTester(OpenAiConnectionTester);

#[async_trait::async_trait]
impl ConnectionTesting for ProductionConnectionTester {
    async fn test(
        &self,
        api_key: &str,
        cancellation: &CancellationToken,
    ) -> Result<(), ProofreaderError> {
        self.0.test(api_key, cancellation).await
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum OpenAiApiKeyError {
    InvalidApiKey,
    NotConfigured,
    KeychainUnavailable,
    Authentication,
    RateLimited,
    QuotaExceeded,
    Offline,
    TimedOut,
    ServiceUnavailable,
    InvalidResponse,
    ConnectionFailed,
}

impl fmt::Display for OpenAiApiKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidApiKey => "invalid API key",
            Self::NotConfigured => "API key not configured",
            Self::KeychainUnavailable => "Keychain unavailable",
            Self::Authentication => "authentication failed",
            Self::RateLimited => "rate limited",
            Self::QuotaExceeded => "quota exceeded",
            Self::Offline => "network offline",
            Self::TimedOut => "connection timed out",
            Self::ServiceUnavailable => "service unavailable",
            Self::InvalidResponse => "invalid provider response",
            Self::ConnectionFailed => "connection failed",
        };
        formatter.write_str(message)
    }
}

impl Error for OpenAiApiKeyError {}

fn map_secret_store_error(error: SecretStoreError) -> OpenAiApiKeyError {
    match error {
        SecretStoreError::InvalidSecret => OpenAiApiKeyError::InvalidApiKey,
        SecretStoreError::Corrupted | SecretStoreError::Unavailable => {
            OpenAiApiKeyError::KeychainUnavailable
        }
    }
}

fn map_connection_error(error: ProofreaderError) -> OpenAiApiKeyError {
    match error {
        ProofreaderError::MissingCredential => OpenAiApiKeyError::NotConfigured,
        ProofreaderError::Authentication => OpenAiApiKeyError::Authentication,
        ProofreaderError::RateLimited => OpenAiApiKeyError::RateLimited,
        ProofreaderError::QuotaExceeded => OpenAiApiKeyError::QuotaExceeded,
        ProofreaderError::Offline => OpenAiApiKeyError::Offline,
        ProofreaderError::TimedOut => OpenAiApiKeyError::TimedOut,
        ProofreaderError::ServiceUnavailable => OpenAiApiKeyError::ServiceUnavailable,
        ProofreaderError::Refused
        | ProofreaderError::Incomplete
        | ProofreaderError::MalformedResponse => OpenAiApiKeyError::InvalidResponse,
        ProofreaderError::Cancelled | ProofreaderError::Failed => {
            OpenAiApiKeyError::ConnectionFailed
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use futures::executor::block_on;

    use super::*;

    #[test]
    fn production_keychain_scope_matches_the_permanent_bundle_identifier() {
        assert_eq!(KEYCHAIN_SERVICE, "io.github.the-pavels.verba");
    }

    #[test]
    fn exposes_only_configuration_state_and_normalizes_saved_keys() {
        let store = Arc::new(MemorySecretStore::default());
        let settings = test_settings(store.clone(), Ok(()));

        assert_eq!(settings.is_configured(), Ok(false));
        assert_eq!(settings.save("  test-key  ".to_owned()), Ok(()));
        assert_eq!(settings.is_configured(), Ok(true));
        assert_eq!(store.secret.lock().unwrap().as_deref(), Some("test-key"));
        assert_eq!(settings.delete(), Ok(()));
        assert_eq!(settings.is_configured(), Ok(false));
    }

    #[test]
    fn rejects_blank_keys_without_touching_the_store() {
        let store = Arc::new(MemorySecretStore::default());
        let settings = test_settings(store.clone(), Ok(()));

        assert_eq!(
            settings.save(" \n ".to_owned()),
            Err(OpenAiApiKeyError::InvalidApiKey)
        );
        assert!(store.secret.lock().unwrap().is_none());
    }

    #[test]
    fn tests_only_the_key_loaded_from_keychain() {
        let store = Arc::new(MemorySecretStore {
            secret: Mutex::new(Some("stored-test-key".to_owned())),
        });
        let tester = Arc::new(FakeConnectionTester::new(Ok(())));
        let settings = OpenAiApiKeySettings {
            secret_store: store,
            connection_tester: tester.clone(),
        };

        assert_eq!(block_on(settings.test_connection()), Ok(()));
        assert_eq!(
            tester.api_keys.lock().unwrap().as_slice(),
            ["stored-test-key"]
        );
    }

    #[test]
    fn maps_missing_storage_and_provider_failures_to_safe_errors() {
        let settings = test_settings(Arc::new(MemorySecretStore::default()), Ok(()));
        assert_eq!(
            block_on(settings.test_connection()),
            Err(OpenAiApiKeyError::NotConfigured)
        );

        for (provider_error, expected) in [
            (
                ProofreaderError::Authentication,
                OpenAiApiKeyError::Authentication,
            ),
            (
                ProofreaderError::RateLimited,
                OpenAiApiKeyError::RateLimited,
            ),
            (
                ProofreaderError::QuotaExceeded,
                OpenAiApiKeyError::QuotaExceeded,
            ),
            (ProofreaderError::Offline, OpenAiApiKeyError::Offline),
            (ProofreaderError::TimedOut, OpenAiApiKeyError::TimedOut),
            (
                ProofreaderError::ServiceUnavailable,
                OpenAiApiKeyError::ServiceUnavailable,
            ),
            (
                ProofreaderError::MalformedResponse,
                OpenAiApiKeyError::InvalidResponse,
            ),
        ] {
            let store = Arc::new(MemorySecretStore {
                secret: Mutex::new(Some("test-key".to_owned())),
            });
            let settings = test_settings(store, Err(provider_error));
            assert_eq!(block_on(settings.test_connection()), Err(expected));
        }
    }

    #[test]
    fn key_provider_reads_each_request_from_secret_storage() {
        let store = Arc::new(MemorySecretStore::default());
        let provider = SecretStoreApiKeyProvider::new(store.clone());

        assert_eq!(provider.load_api_key(), Err(ApiKeyProviderError::Missing));
        store.save("first-key").unwrap();
        assert_eq!(provider.load_api_key(), Ok("first-key".to_owned()));
        store.save("replacement-key").unwrap();
        assert_eq!(provider.load_api_key(), Ok("replacement-key".to_owned()));

        let unavailable = SecretStoreApiKeyProvider::new(Arc::new(UnavailableSecretStore));
        assert_eq!(
            unavailable.load_api_key(),
            Err(ApiKeyProviderError::Unavailable)
        );
    }

    fn test_settings(
        store: Arc<dyn SecretStore>,
        test_result: Result<(), ProofreaderError>,
    ) -> OpenAiApiKeySettings {
        OpenAiApiKeySettings {
            secret_store: store,
            connection_tester: Arc::new(FakeConnectionTester::new(test_result)),
        }
    }

    #[derive(Default)]
    struct MemorySecretStore {
        secret: Mutex<Option<String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn save(&self, secret: &str) -> Result<(), SecretStoreError> {
            *self.secret.lock().unwrap() = Some(secret.to_owned());
            Ok(())
        }

        fn load(&self) -> Result<Option<String>, SecretStoreError> {
            Ok(self.secret.lock().unwrap().clone())
        }

        fn delete(&self) -> Result<(), SecretStoreError> {
            *self.secret.lock().unwrap() = None;
            Ok(())
        }
    }

    struct UnavailableSecretStore;

    impl SecretStore for UnavailableSecretStore {
        fn save(&self, _secret: &str) -> Result<(), SecretStoreError> {
            Err(SecretStoreError::Unavailable)
        }

        fn load(&self) -> Result<Option<String>, SecretStoreError> {
            Err(SecretStoreError::Unavailable)
        }

        fn delete(&self) -> Result<(), SecretStoreError> {
            Err(SecretStoreError::Unavailable)
        }
    }

    struct FakeConnectionTester {
        result: Mutex<Option<Result<(), ProofreaderError>>>,
        api_keys: Mutex<Vec<String>>,
    }

    impl FakeConnectionTester {
        fn new(result: Result<(), ProofreaderError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                api_keys: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ConnectionTesting for FakeConnectionTester {
        async fn test(
            &self,
            api_key: &str,
            _cancellation: &CancellationToken,
        ) -> Result<(), ProofreaderError> {
            self.api_keys.lock().unwrap().push(api_key.to_owned());
            self.result
                .lock()
                .unwrap()
                .take()
                .expect("the connection test should execute once")
        }
    }
}
