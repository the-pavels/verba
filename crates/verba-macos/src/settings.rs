use std::sync::Arc;

use objc2_foundation::{NSString, NSUserDefaults};
use verba_core::proofreading::{ProofreadingConsentStore, ProofreadingConsentStoreError};
use verba_core::shortcut::{
    NamedShortcutKey, Shortcut, ShortcutConfiguration, ShortcutKey, ShortcutModifiers,
    ShortcutSettingsStore, ShortcutSettingsStoreError,
};
use verba_core::translation::{
    LanguageIdentifier, TranslationSettingsStore, TranslationSettingsStoreError,
};

const TARGET_LANGUAGE_KEY: &str = "translation.targetLanguageIdentifier";
const PROOFREADING_DISCLOSURE_KEY: &str = "proofreading.disclosureAcknowledged";
const TRANSLATE_SHORTCUT_KEY: &str = "shortcuts.translate";
const PROOFREAD_SHORTCUT_KEY: &str = "shortcuts.proofread";

trait StringPreferences: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: Option<&str>);
}

struct UserDefaultsPreferences;

impl StringPreferences for UserDefaultsPreferences {
    fn get(&self, key: &str) -> Option<String> {
        NSUserDefaults::standardUserDefaults()
            .stringForKey(&NSString::from_str(key))
            .map(|value| value.to_string())
    }

    fn set(&self, key: &str, value: Option<&str>) {
        let defaults = NSUserDefaults::standardUserDefaults();
        let key = NSString::from_str(key);
        if let Some(value) = value {
            let value = NSString::from_str(value);
            unsafe {
                defaults.setObject_forKey(Some(&value), &key);
            }
        } else {
            defaults.removeObjectForKey(&key);
        }
    }
}

fn set_and_verify(preferences: &dyn StringPreferences, key: &str, value: &str) -> bool {
    let previous = preferences.get(key);
    preferences.set(key, Some(value));
    if preferences.get(key).as_deref() == Some(value) {
        return true;
    }

    restore(preferences, key, previous.as_deref());
    false
}

fn restore(preferences: &dyn StringPreferences, key: &str, value: Option<&str>) {
    preferences.set(key, value);
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
        set_and_verify(
            self.preferences.as_ref(),
            TARGET_LANGUAGE_KEY,
            target_language.as_str(),
        )
        .then_some(())
        .ok_or(TranslationSettingsStoreError::Unavailable)
    }
}

pub struct MacOsProofreadingConsentStore {
    preferences: Arc<dyn StringPreferences>,
}

impl MacOsProofreadingConsentStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            preferences: Arc::new(UserDefaultsPreferences),
        }
    }
}

impl Default for MacOsProofreadingConsentStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProofreadingConsentStore for MacOsProofreadingConsentStore {
    fn load_acknowledged(&self) -> Result<bool, ProofreadingConsentStoreError> {
        Ok(self
            .preferences
            .get(PROOFREADING_DISCLOSURE_KEY)
            .is_some_and(|value| value == "true"))
    }

    fn save_acknowledged(&self) -> Result<(), ProofreadingConsentStoreError> {
        set_and_verify(
            self.preferences.as_ref(),
            PROOFREADING_DISCLOSURE_KEY,
            "true",
        )
        .then_some(())
        .ok_or(ProofreadingConsentStoreError)
    }
}

pub struct MacOsShortcutSettingsStore {
    preferences: Arc<dyn StringPreferences>,
}

impl MacOsShortcutSettingsStore {
    #[must_use]
    pub fn new() -> Self {
        Self {
            preferences: Arc::new(UserDefaultsPreferences),
        }
    }
}

impl Default for MacOsShortcutSettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutSettingsStore for MacOsShortcutSettingsStore {
    fn load(&self) -> Result<Option<ShortcutConfiguration>, ShortcutSettingsStoreError> {
        let Some(translate) = self.preferences.get(TRANSLATE_SHORTCUT_KEY) else {
            return Ok(None);
        };
        let Some(proofread) = self.preferences.get(PROOFREAD_SHORTCUT_KEY) else {
            return Ok(None);
        };

        Ok(decode_shortcut(&translate)
            .zip(decode_shortcut(&proofread))
            .and_then(|(translate, proofread)| {
                ShortcutConfiguration::new(translate, proofread).ok()
            }))
    }

    fn save(
        &self,
        configuration: &ShortcutConfiguration,
    ) -> Result<(), ShortcutSettingsStoreError> {
        let previous_translate = self.preferences.get(TRANSLATE_SHORTCUT_KEY);
        let previous_proofread = self.preferences.get(PROOFREAD_SHORTCUT_KEY);
        let translate = encode_shortcut(
            configuration.shortcut_for(verba_core::presentation::TextAction::Translate),
        );
        let proofread = encode_shortcut(
            configuration.shortcut_for(verba_core::presentation::TextAction::Proofread),
        );

        self.preferences
            .set(TRANSLATE_SHORTCUT_KEY, Some(&translate));
        self.preferences
            .set(PROOFREAD_SHORTCUT_KEY, Some(&proofread));
        if self.preferences.get(TRANSLATE_SHORTCUT_KEY).as_deref() == Some(translate.as_str())
            && self.preferences.get(PROOFREAD_SHORTCUT_KEY).as_deref() == Some(proofread.as_str())
        {
            return Ok(());
        }

        restore(
            self.preferences.as_ref(),
            TRANSLATE_SHORTCUT_KEY,
            previous_translate.as_deref(),
        );
        restore(
            self.preferences.as_ref(),
            PROOFREAD_SHORTCUT_KEY,
            previous_proofread.as_deref(),
        );
        Err(ShortcutSettingsStoreError)
    }
}

fn encode_shortcut(shortcut: Shortcut) -> String {
    let key = if let Some(character) = shortcut.key().character_value() {
        format!("character:{character}")
    } else if let Some(number) = shortcut.key().function_number() {
        format!("function:{number}")
    } else {
        format!(
            "named:{}",
            named_key_identifier(shortcut.key().named_value().expect("shortcut key kind"))
        )
    };
    let modifiers = shortcut.modifiers();
    format!(
        "{key}|{}{}{}{}",
        u8::from(modifiers.command()),
        u8::from(modifiers.control()),
        u8::from(modifiers.option()),
        u8::from(modifiers.shift())
    )
}

fn decode_shortcut(value: &str) -> Option<Shortcut> {
    let (key, modifiers) = value.split_once('|')?;
    let flags = modifiers.chars().collect::<Vec<_>>();
    if flags.len() != 4 || flags.iter().any(|flag| !matches!(flag, '0' | '1')) {
        return None;
    }
    let modifiers = ShortcutModifiers::new(
        flags[0] == '1',
        flags[1] == '1',
        flags[2] == '1',
        flags[3] == '1',
    );

    let (kind, value) = key.split_once(':')?;
    let key = match kind {
        "character" => ShortcutKey::character(value.parse().ok()?).ok()?,
        "function" => ShortcutKey::function(value.parse().ok()?).ok()?,
        "named" => ShortcutKey::named(parse_named_key(value)?),
        _ => return None,
    };
    Shortcut::new(key, modifiers).ok()
}

const fn named_key_identifier(key: NamedShortcutKey) -> &'static str {
    match key {
        NamedShortcutKey::Space => "space",
        NamedShortcutKey::Return => "return",
        NamedShortcutKey::Tab => "tab",
        NamedShortcutKey::Escape => "escape",
        NamedShortcutKey::Delete => "delete",
        NamedShortcutKey::ArrowUp => "arrow-up",
        NamedShortcutKey::ArrowDown => "arrow-down",
        NamedShortcutKey::ArrowLeft => "arrow-left",
        NamedShortcutKey::ArrowRight => "arrow-right",
    }
}

fn parse_named_key(value: &str) -> Option<NamedShortcutKey> {
    Some(match value {
        "space" => NamedShortcutKey::Space,
        "return" => NamedShortcutKey::Return,
        "tab" => NamedShortcutKey::Tab,
        "escape" => NamedShortcutKey::Escape,
        "delete" => NamedShortcutKey::Delete,
        "arrow-up" => NamedShortcutKey::ArrowUp,
        "arrow-down" => NamedShortcutKey::ArrowDown,
        "arrow-left" => NamedShortcutKey::ArrowLeft,
        "arrow-right" => NamedShortcutKey::ArrowRight,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Mutex};

    use super::*;

    #[derive(Default)]
    struct MemoryPreferences {
        values: Mutex<HashMap<String, String>>,
        writes: Mutex<Vec<(String, String)>>,
        ignored_writes: Mutex<Vec<String>>,
    }

    impl StringPreferences for MemoryPreferences {
        fn get(&self, key: &str) -> Option<String> {
            self.values.lock().unwrap().get(key).cloned()
        }

        fn set(&self, key: &str, value: Option<&str>) {
            let mut ignored_writes = self.ignored_writes.lock().unwrap();
            if let Some(index) = ignored_writes.iter().position(|ignored| ignored == key) {
                ignored_writes.remove(index);
                return;
            }
            drop(ignored_writes);

            let mut values = self.values.lock().unwrap();
            if let Some(value) = value {
                values.insert(key.to_owned(), value.to_owned());
                self.writes
                    .lock()
                    .unwrap()
                    .push((key.to_owned(), value.to_owned()));
            } else {
                values.remove(key);
            }
        }
    }

    #[test]
    fn loads_valid_identifiers_and_ignores_invalid_values() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsTranslationSettingsStore {
            preferences: preferences.clone(),
        };

        preferences
            .values
            .lock()
            .unwrap()
            .insert(TARGET_LANGUAGE_KEY.to_owned(), "FR".to_owned());
        assert_eq!(
            store.load_target_language().unwrap(),
            Some(LanguageIdentifier::new("fr").unwrap())
        );

        preferences
            .values
            .lock()
            .unwrap()
            .insert(TARGET_LANGUAGE_KEY.to_owned(), "not_a_language".to_owned());
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

    #[test]
    fn target_language_save_fails_when_user_defaults_does_not_observe_the_write() {
        let preferences = Arc::new(MemoryPreferences::default());
        preferences
            .values
            .lock()
            .unwrap()
            .insert(TARGET_LANGUAGE_KEY.to_owned(), "en".to_owned());
        preferences
            .ignored_writes
            .lock()
            .unwrap()
            .push(TARGET_LANGUAGE_KEY.to_owned());
        let store = MacOsTranslationSettingsStore { preferences };

        assert_eq!(
            store.save_target_language(&LanguageIdentifier::new("de").unwrap()),
            Err(TranslationSettingsStoreError::Unavailable)
        );
        assert_eq!(
            store.load_target_language().unwrap(),
            Some(LanguageIdentifier::new("en").unwrap())
        );
    }

    #[test]
    fn loads_and_persists_the_non_secret_proofreading_acknowledgement() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsProofreadingConsentStore {
            preferences: preferences.clone(),
        };

        assert!(!store.load_acknowledged().unwrap());
        preferences
            .values
            .lock()
            .unwrap()
            .insert(PROOFREADING_DISCLOSURE_KEY.to_owned(), "false".to_owned());
        assert!(!store.load_acknowledged().unwrap());
        preferences
            .values
            .lock()
            .unwrap()
            .insert(PROOFREADING_DISCLOSURE_KEY.to_owned(), "true".to_owned());
        assert!(store.load_acknowledged().unwrap());

        store.save_acknowledged().unwrap();
        assert_eq!(
            preferences.writes.lock().unwrap().as_slice(),
            &[(PROOFREADING_DISCLOSURE_KEY.to_owned(), "true".to_owned())]
        );
    }

    #[test]
    fn consent_save_fails_when_user_defaults_does_not_observe_the_write() {
        let preferences = Arc::new(MemoryPreferences::default());
        preferences
            .values
            .lock()
            .unwrap()
            .insert(PROOFREADING_DISCLOSURE_KEY.to_owned(), "false".to_owned());
        preferences
            .ignored_writes
            .lock()
            .unwrap()
            .push(PROOFREADING_DISCLOSURE_KEY.to_owned());
        let store = MacOsProofreadingConsentStore { preferences };

        assert_eq!(
            store.save_acknowledged(),
            Err(ProofreadingConsentStoreError)
        );
        assert!(!store.load_acknowledged().unwrap());
    }

    #[test]
    fn shortcut_settings_round_trip_and_invalid_values_fall_back() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsShortcutSettingsStore {
            preferences: preferences.clone(),
        };
        let configuration = ShortcutConfiguration::default();

        assert_eq!(store.load().unwrap(), None);
        store.save(&configuration).unwrap();
        assert_eq!(store.load().unwrap(), Some(configuration));

        preferences.values.lock().unwrap().insert(
            TRANSLATE_SHORTCUT_KEY.to_owned(),
            "character:Q|1000".to_owned(),
        );
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn shortcut_readback_mismatch_restores_the_previous_configuration() {
        let preferences = Arc::new(MemoryPreferences::default());
        let store = MacOsShortcutSettingsStore {
            preferences: preferences.clone(),
        };
        let current = ShortcutConfiguration::default();
        store.save(&current).unwrap();

        let replacement = current
            .with_shortcut(
                verba_core::presentation::TextAction::Translate,
                Shortcut::new(
                    ShortcutKey::character('L').unwrap(),
                    ShortcutModifiers::new(false, true, true, false),
                )
                .unwrap(),
            )
            .unwrap()
            .with_shortcut(
                verba_core::presentation::TextAction::Proofread,
                Shortcut::new(
                    ShortcutKey::character('O').unwrap(),
                    ShortcutModifiers::new(false, true, true, false),
                )
                .unwrap(),
            )
            .unwrap();
        preferences
            .ignored_writes
            .lock()
            .unwrap()
            .push(PROOFREAD_SHORTCUT_KEY.to_owned());

        assert_eq!(store.save(&replacement), Err(ShortcutSettingsStoreError));
        assert_eq!(store.load().unwrap(), Some(current));
    }
}
