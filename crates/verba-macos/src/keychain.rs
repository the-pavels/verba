use std::sync::Arc;

use security_framework::passwords::{
    PasswordOptions, delete_generic_password, generic_password, set_generic_password,
};
use security_framework_sys::base::errSecItemNotFound;
use verba_core::secrets::{SecretStore, SecretStoreError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacOsSecretStoreBuildError {
    EmptyService,
    EmptyAccount,
}

pub struct MacOsSecretStore {
    service: String,
    account: String,
    keychain: Arc<dyn GenericPasswordKeychain>,
}

impl MacOsSecretStore {
    pub fn new(
        service: impl Into<String>,
        account: impl Into<String>,
    ) -> Result<Self, MacOsSecretStoreBuildError> {
        let service = service.into();
        let service = service.trim();
        if service.is_empty() {
            return Err(MacOsSecretStoreBuildError::EmptyService);
        }
        let account = account.into();
        let account = account.trim();
        if account.is_empty() {
            return Err(MacOsSecretStoreBuildError::EmptyAccount);
        }

        Ok(Self {
            service: service.to_owned(),
            account: account.to_owned(),
            keychain: Arc::new(SystemKeychain),
        })
    }

    #[cfg(test)]
    fn with_keychain(
        service: impl Into<String>,
        account: impl Into<String>,
        keychain: Arc<dyn GenericPasswordKeychain>,
    ) -> Self {
        Self {
            service: service.into(),
            account: account.into(),
            keychain,
        }
    }
}

impl SecretStore for MacOsSecretStore {
    fn save(&self, secret: &str) -> Result<(), SecretStoreError> {
        if secret.trim().is_empty() {
            return Err(SecretStoreError::InvalidSecret);
        }

        self.keychain
            .save(&self.service, &self.account, secret.as_bytes())
            .map_err(|_| SecretStoreError::Unavailable)
    }

    fn load(&self) -> Result<Option<String>, SecretStoreError> {
        self.keychain
            .load(&self.service, &self.account)
            .map_err(|_| SecretStoreError::Unavailable)?
            .map(String::from_utf8)
            .transpose()
            .map_err(|_| SecretStoreError::Corrupted)
    }

    fn delete(&self) -> Result<(), SecretStoreError> {
        self.keychain
            .delete(&self.service, &self.account)
            .map_err(|_| SecretStoreError::Unavailable)
    }
}

trait GenericPasswordKeychain: Send + Sync {
    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), KeychainError>;

    fn load(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, KeychainError>;

    fn delete(&self, service: &str, account: &str) -> Result<(), KeychainError>;
}

struct SystemKeychain;

impl GenericPasswordKeychain for SystemKeychain {
    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), KeychainError> {
        set_generic_password(service, account, secret).map_err(|_| KeychainError)
    }

    fn load(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, KeychainError> {
        match generic_password(PasswordOptions::new_generic_password(service, account)) {
            Ok(secret) => Ok(Some(secret)),
            Err(error) if error.code() == errSecItemNotFound => Ok(None),
            Err(_) => Err(KeychainError),
        }
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), KeychainError> {
        match delete_generic_password(service, account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == errSecItemNotFound => Ok(()),
            Err(_) => Err(KeychainError),
        }
    }
}

#[derive(Clone, Copy)]
struct KeychainError;

#[cfg(test)]
mod tests;
