use std::sync::{Arc, RwLock};

use super::{LanguageIdentifier, TranslationSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationSettingsStoreError {
    Unavailable,
}

pub trait TranslationSettingsStore: Send + Sync {
    fn load_target_language(
        &self,
    ) -> Result<Option<LanguageIdentifier>, TranslationSettingsStoreError>;

    fn save_target_language(
        &self,
        target_language: &LanguageIdentifier,
    ) -> Result<(), TranslationSettingsStoreError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationPreferenceFailure {
    NoSupportedTargets,
    UnsupportedTarget,
    PersistenceFailed,
}

pub struct TranslationPreferences {
    store: Arc<dyn TranslationSettingsStore>,
    state: RwLock<TranslationPreferenceState>,
}

struct TranslationPreferenceState {
    settings: TranslationSettings,
    supported_targets: Vec<LanguageIdentifier>,
}

impl TranslationPreferences {
    pub fn load(
        store: Arc<dyn TranslationSettingsStore>,
    ) -> Result<Self, TranslationSettingsStoreError> {
        let settings = store
            .load_target_language()?
            .map(TranslationSettings::new)
            .unwrap_or_default();

        Ok(Self {
            store,
            state: RwLock::new(TranslationPreferenceState {
                settings,
                supported_targets: Vec::new(),
            }),
        })
    }

    #[must_use]
    pub fn settings(&self) -> TranslationSettings {
        self.state
            .read()
            .expect("translation preference lock poisoned")
            .settings
            .clone()
    }

    pub fn set_supported_targets(
        &self,
        targets: impl IntoIterator<Item = LanguageIdentifier>,
    ) -> Result<LanguageIdentifier, TranslationPreferenceFailure> {
        let targets = targets.into_iter().fold(Vec::new(), |mut targets, target| {
            if !targets.contains(&target) {
                targets.push(target);
            }
            targets
        });
        if targets.is_empty() {
            return Err(TranslationPreferenceFailure::NoSupportedTargets);
        }

        let mut state = self
            .state
            .write()
            .expect("translation preference lock poisoned");
        if !targets.contains(state.settings.target_language()) {
            let default = TranslationSettings::default().target_language().clone();
            let fallback = targets
                .iter()
                .find(|target| **target == default)
                .cloned()
                .unwrap_or_else(|| targets[0].clone());
            self.store
                .save_target_language(&fallback)
                .map_err(|_| TranslationPreferenceFailure::PersistenceFailed)?;
            state.settings.set_target_language(fallback);
        }

        state.supported_targets = targets;
        Ok(state.settings.target_language().clone())
    }

    pub fn set_target_language(
        &self,
        target: LanguageIdentifier,
    ) -> Result<(), TranslationPreferenceFailure> {
        let mut state = self
            .state
            .write()
            .expect("translation preference lock poisoned");
        if !state.supported_targets.contains(&target) {
            return Err(TranslationPreferenceFailure::UnsupportedTarget);
        }
        if state.settings.target_language() == &target {
            return Ok(());
        }

        self.store
            .save_target_language(&target)
            .map_err(|_| TranslationPreferenceFailure::PersistenceFailed)?;
        state.settings.set_target_language(target);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct FakeStore {
        loaded: Result<Option<LanguageIdentifier>, TranslationSettingsStoreError>,
        saves: Mutex<Vec<LanguageIdentifier>>,
        save_error: bool,
    }

    impl FakeStore {
        fn new(target: Option<&str>) -> Arc<Self> {
            Arc::new(Self {
                loaded: Ok(target.map(language)),
                saves: Mutex::new(Vec::new()),
                save_error: false,
            })
        }
    }

    impl TranslationSettingsStore for FakeStore {
        fn load_target_language(
            &self,
        ) -> Result<Option<LanguageIdentifier>, TranslationSettingsStoreError> {
            self.loaded.clone()
        }

        fn save_target_language(
            &self,
            target_language: &LanguageIdentifier,
        ) -> Result<(), TranslationSettingsStoreError> {
            if self.save_error {
                return Err(TranslationSettingsStoreError::Unavailable);
            }
            self.saves.lock().unwrap().push(target_language.clone());
            Ok(())
        }
    }

    #[test]
    fn loads_the_persisted_target_language() {
        let preferences = TranslationPreferences::load(FakeStore::new(Some("fr"))).unwrap();

        assert_eq!(preferences.settings().target_language(), &language("fr"));
    }

    #[test]
    fn keeps_a_supported_selection_and_applies_changes_immediately() {
        let store = FakeStore::new(Some("fr"));
        let preferences = TranslationPreferences::load(store.clone()).unwrap();

        let selected = preferences
            .set_supported_targets([language("de"), language("fr")])
            .unwrap();
        assert_eq!(selected, language("fr"));
        assert!(store.saves.lock().unwrap().is_empty());

        preferences.set_target_language(language("de")).unwrap();
        assert_eq!(preferences.settings().target_language(), &language("de"));
        assert_eq!(store.saves.lock().unwrap().as_slice(), &[language("de")]);
    }

    #[test]
    fn falls_back_to_english_then_the_first_supported_target() {
        let english_store = FakeStore::new(Some("ga"));
        let english_preferences = TranslationPreferences::load(english_store.clone()).unwrap();
        assert_eq!(
            english_preferences
                .set_supported_targets([language("fr"), language("en")])
                .unwrap(),
            language("en")
        );
        assert_eq!(
            english_store.saves.lock().unwrap().as_slice(),
            &[language("en")]
        );

        let first_store = FakeStore::new(Some("ga"));
        let first_preferences = TranslationPreferences::load(first_store.clone()).unwrap();
        assert_eq!(
            first_preferences
                .set_supported_targets([language("de"), language("fr")])
                .unwrap(),
            language("de")
        );
        assert_eq!(
            first_store.saves.lock().unwrap().as_slice(),
            &[language("de")]
        );
    }

    #[test]
    fn rejects_empty_unsupported_and_unpersisted_updates() {
        let store = Arc::new(FakeStore {
            loaded: Ok(None),
            saves: Mutex::new(Vec::new()),
            save_error: true,
        });
        let preferences = TranslationPreferences::load(store).unwrap();

        assert_eq!(
            preferences.set_supported_targets([]),
            Err(TranslationPreferenceFailure::NoSupportedTargets)
        );
        assert_eq!(
            preferences.set_target_language(language("de")),
            Err(TranslationPreferenceFailure::UnsupportedTarget)
        );
        assert_eq!(
            preferences.set_supported_targets([language("de")]),
            Err(TranslationPreferenceFailure::PersistenceFailed)
        );
        assert_eq!(preferences.settings(), TranslationSettings::default());
    }

    fn language(identifier: &str) -> LanguageIdentifier {
        LanguageIdentifier::new(identifier).unwrap()
    }
}
