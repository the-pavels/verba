use std::sync::Arc;

use objc2_foundation::{NSString, NSUserDefaults};
use verba_core::translation::{
    LanguageIdentifier, TranslationSettingsStore, TranslationSettingsStoreError,
};

const TARGET_LANGUAGE_KEY: &str = "translation.targetLanguageIdentifier";

trait StringPreferences: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
}

struct UserDefaultsPreferences;

impl StringPreferences for UserDefaultsPreferences {
    fn get(&self, key: &str) -> Option<String> {
        NSUserDefaults::standardUserDefaults()
            .stringForKey(&NSString::from_str(key))
            .map(|value| value.to_string())
    }

    fn set(&self, key: &str, value: &str) {
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(key);
        let value = NSString::from_str(value);
        unsafe {
            defaults.setObject_forKey(Some(&value), &key);
        }
    }
}

pub struct MacOsTranslationSettingsStore {
    preferences: Arc<dyn StringPreferences>,
}

impl MacOsTranslationSettingsStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            preferences: Arc::new(UserDefaultsPreferences),
        }
    }
}

impl Default for MacOsTranslationSettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslationSettingsStore for MacOsTranslationSettingsStore {
    fn load_target_language(
        &self,
    ) -> Result<Option<LanguageIdentifier>, TranslationSettingsStoreError> {
        Ok(self
            .preferences
            .get(TARGET_LANGUAGE_KEY)
            .and_then(|identifier| LanguageIdentifier::new(identifier).ok()))
    }

    fn save_target_language(
        &self,
        target_language: &LanguageIdentifier,
    ) -> Result<(), TranslationSettingsStoreError> {
        self.preferences
            .set(TARGET_LANGUAGE_KEY, target_language.as_str());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct MemoryPreferences {
        value: Mutex<Option<String>>,
        writes: Mutex<Vec<(String, String)>>,
    }

    impl StringPreferences for MemoryPreferences {
        fn get(&self, _key: &str) -> Option<String> {
            self.value.lock().unwrap().clone()
        }

        fn set(&self, key: &str, value: &str) {
            self.writes
                .lock()
                .unwrap()
                .push((key.to_owned(), value.to_owned()));
        }
    }

    #[test]
    fn loads_valid_identifiers_and_ignores_invalid_values() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsTranslationSettingsStore {
            preferences: preferences.clone(),
        };

        *preferences.value.lock().unwrap() = Some("FR".to_owned());
        assert_eq!(
            store.load_target_language().unwrap(),
            Some(LanguageIdentifier::new("fr").unwrap())
        );

        *preferences.value.lock().unwrap() = Some("not_a_language".to_owned());
        assert_eq!(store.load_target_language().unwrap(), None);
    }

    #[test]
    fn saves_the_normalized_identifier() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsTranslationSettingsStore {
            preferences: preferences.clone(),
        };

        store
            .save_target_language(&LanguageIdentifier::new("DE").unwrap())
            .unwrap();

        assert_eq!(
            preferences.writes.lock().unwrap().as_slice(),
            &[(TARGET_LANGUAGE_KEY.to_owned(), "de".to_owned())]
        );
    }
}
