use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use super::*;

#[test]
fn validates_the_keychain_scope() {
    assert!(matches!(
        MacOsSecretStore::new("", "openai-api-key"),
        Err(MacOsSecretStoreBuildError::EmptyService)
    ));
    assert!(matches!(
        MacOsSecretStore::new("com.example.verba", "  "),
        Err(MacOsSecretStoreBuildError::EmptyAccount)
    ));
}

#[test]
fn adds_updates_retrieves_and_deletes_the_scoped_secret() {
    let keychain = Arc::new(MemoryKeychain::default());
    let store =
        MacOsSecretStore::with_keychain("com.example.verba", "openai-api-key", keychain.clone());

    assert_eq!(store.load(), Ok(None));
    store.save("first-test-value").unwrap();
    assert_eq!(store.load(), Ok(Some("first-test-value".to_owned())));
    store.save("replacement-test-value").unwrap();
    assert_eq!(store.load(), Ok(Some("replacement-test-value".to_owned())));

    store.delete().unwrap();
    assert_eq!(store.load(), Ok(None));
    store.delete().unwrap();
    assert!(
        keychain
            .items
            .lock()
            .unwrap()
            .get(&("com.example.verba".to_owned(), "openai-api-key".to_owned()))
            .is_none()
    );
}

#[test]
fn rejects_empty_secrets_without_writing_to_keychain() {
    let keychain = Arc::new(MemoryKeychain::default());
    let store = MacOsSecretStore::with_keychain("service", "account", keychain.clone());

    assert_eq!(store.save(" \n "), Err(SecretStoreError::InvalidSecret));
    assert!(keychain.items.lock().unwrap().is_empty());
}

#[test]
fn maps_keychain_and_data_failures_without_secret_details() {
    let unavailable =
        MacOsSecretStore::with_keychain("service", "account", Arc::new(FailingKeychain));
    assert_eq!(
        unavailable.save("test-value"),
        Err(SecretStoreError::Unavailable)
    );
    assert_eq!(unavailable.load(), Err(SecretStoreError::Unavailable));
    assert_eq!(unavailable.delete(), Err(SecretStoreError::Unavailable));

    let corrupted =
        MacOsSecretStore::with_keychain("service", "account", Arc::new(CorruptedKeychain));
    assert_eq!(corrupted.load(), Err(SecretStoreError::Corrupted));
}

#[derive(Default)]
struct MemoryKeychain {
    items: Mutex<HashMap<(String, String), Vec<u8>>>,
}

impl GenericPasswordKeychain for MemoryKeychain {
    fn save(&self, service: &str, account: &str, secret: &[u8]) -> Result<(), KeychainError> {
        self.items
            .lock()
            .unwrap()
            .insert((service.to_owned(), account.to_owned()), secret.to_owned());
        Ok(())
    }

    fn load(&self, service: &str, account: &str) -> Result<Option<Vec<u8>>, KeychainError> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .get(&(service.to_owned(), account.to_owned()))
            .cloned())
    }

    fn delete(&self, service: &str, account: &str) -> Result<(), KeychainError> {
        self.items
            .lock()
            .unwrap()
            .remove(&(service.to_owned(), account.to_owned()));
        Ok(())
    }
}

struct FailingKeychain;

impl GenericPasswordKeychain for FailingKeychain {
    fn save(&self, _service: &str, _account: &str, _secret: &[u8]) -> Result<(), KeychainError> {
        Err(KeychainError)
    }

    fn load(&self, _service: &str, _account: &str) -> Result<Option<Vec<u8>>, KeychainError> {
        Err(KeychainError)
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), KeychainError> {
        Err(KeychainError)
    }
}

struct CorruptedKeychain;

impl GenericPasswordKeychain for CorruptedKeychain {
    fn save(&self, _service: &str, _account: &str, _secret: &[u8]) -> Result<(), KeychainError> {
        Ok(())
    }

    fn load(&self, _service: &str, _account: &str) -> Result<Option<Vec<u8>>, KeychainError> {
        Ok(Some(vec![0xff]))
    }

    fn delete(&self, _service: &str, _account: &str) -> Result<(), KeychainError> {
        Ok(())
    }
}
