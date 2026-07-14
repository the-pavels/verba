use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use verba_core::{
    coordinator::{PresentationUpdate, ResultPresenter, ShortcutCoordinator},
    proofreading::ProofreadingConsentPreferences,
    shortcut::{ShortcutConfiguration, ShortcutRegistry},
    translation::{LanguageIdentifier, TranslationPreferenceFailure, TranslationPreferences},
};
use verba_macos::{
    MacOsProofreadingConsentStore, MacOsShortcutRegistry, MacOsTextCapture,
    MacOsTranslationSettingsStore,
};
use verba_openai::{OpenAiClient, OpenAiConfig, OpenAiProofreader};

use crate::{
    PresentationViewModel,
    api_key_settings::{SecretStoreApiKeyProvider, openai_secret_store},
    processor::ApplicationProcessor,
    translation::NativeTranslator,
};

#[uniffi::export(with_foreign)]
pub trait PresentationObserver: Send + Sync {
    fn present(&self, request_id: u64, presentation: PresentationViewModel);
}

#[derive(uniffi::Object)]
pub struct ApplicationRuntime {
    coordinator: Arc<ShortcutCoordinator>,
    shortcut_registry: Mutex<MacOsShortcutRegistry>,
    translation_preferences: Arc<TranslationPreferences>,
}

#[uniffi::export]
impl ApplicationRuntime {
    #[uniffi::constructor]
    pub fn new(
        observer: Arc<dyn PresentationObserver>,
        translator: Arc<dyn NativeTranslator>,
    ) -> Result<Arc<Self>, ApplicationRuntimeError> {
        let translation_preferences = Arc::new(
            TranslationPreferences::load(Arc::new(MacOsTranslationSettingsStore::new()))
                .map_err(|_| ApplicationRuntimeError::SettingsUnavailable)?,
        );
        let presenter = Arc::new(ForeignPresenter { observer });
        let proofreading_consent = Arc::new(
            ProofreadingConsentPreferences::load(Arc::new(MacOsProofreadingConsentStore::new()))
                .map_err(|_| ApplicationRuntimeError::SettingsUnavailable)?,
        );
        let api_key_provider = Arc::new(SecretStoreApiKeyProvider::new(
            openai_secret_store().map_err(|_| ApplicationRuntimeError::SettingsUnavailable)?,
        ));
        let openai_client = Arc::new(
            OpenAiClient::new(OpenAiConfig::default())
                .map_err(|_| ApplicationRuntimeError::ProofreadingUnavailable)?,
        );
        let proofreader = Arc::new(OpenAiProofreader::new(openai_client, api_key_provider));
        let coordinator = Arc::new(ShortcutCoordinator::with_proofreading_consent(
            Arc::new(MacOsTextCapture::new()),
            Arc::new(ApplicationProcessor::new(
                translator,
                Arc::clone(&translation_preferences),
                proofreader,
            )),
            presenter,
            proofreading_consent,
        ));
        let mut shortcut_registry = MacOsShortcutRegistry::new();
        coordinator
            .register_shortcuts(&mut shortcut_registry, &ShortcutConfiguration::default())
            .map_err(|_| ApplicationRuntimeError::ShortcutRegistrationFailed)?;

        Ok(Arc::new(Self {
            coordinator,
            shortcut_registry: Mutex::new(shortcut_registry),
            translation_preferences,
        }))
    }

    pub fn cancel_active(&self) -> bool {
        self.coordinator.cancel_active()
    }

    pub fn acknowledge_proofreading_disclosure(&self) -> Result<bool, ProofreadingDisclosureError> {
        self.coordinator
            .resolve_proofreading_disclosure(true)
            .map_err(|_| ProofreadingDisclosureError::PersistenceFailed)
    }

    pub fn configure_supported_target_languages(
        &self,
        identifiers: Vec<String>,
    ) -> Result<String, TargetLanguagePreferenceError> {
        let targets = identifiers
            .into_iter()
            .map(|identifier| {
                LanguageIdentifier::new(identifier)
                    .map_err(|_| TargetLanguagePreferenceError::InvalidIdentifier)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.translation_preferences
            .set_supported_targets(targets)
            .map(LanguageIdentifier::into_string)
            .map_err(Into::into)
    }

    pub fn set_target_language(
        &self,
        identifier: String,
    ) -> Result<(), TargetLanguagePreferenceError> {
        let identifier = LanguageIdentifier::new(identifier)
            .map_err(|_| TargetLanguagePreferenceError::InvalidIdentifier)?;
        self.translation_preferences
            .set_target_language(identifier)
            .map_err(Into::into)
    }
}

impl Drop for ApplicationRuntime {
    fn drop(&mut self) {
        self.coordinator.shutdown();
        let _ = self
            .shortcut_registry
            .get_mut()
            .expect("shortcut registry lock poisoned")
            .unregister_all();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum ApplicationRuntimeError {
    ShortcutRegistrationFailed,
    SettingsUnavailable,
    ProofreadingUnavailable,
}

impl fmt::Display for ApplicationRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortcutRegistrationFailed => formatter.write_str("shortcut registration failed"),
            Self::SettingsUnavailable => formatter.write_str("settings unavailable"),
            Self::ProofreadingUnavailable => formatter.write_str("proofreading unavailable"),
        }
    }
}

impl Error for ApplicationRuntimeError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum ProofreadingDisclosureError {
    PersistenceFailed,
}

impl fmt::Display for ProofreadingDisclosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("proofreading acknowledgement could not be saved")
    }
}

impl Error for ProofreadingDisclosureError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum TargetLanguagePreferenceError {
    InvalidIdentifier,
    NoSupportedTargets,
    UnsupportedTarget,
    PersistenceFailed,
}

impl From<TranslationPreferenceFailure> for TargetLanguagePreferenceError {
    fn from(failure: TranslationPreferenceFailure) -> Self {
        match failure {
            TranslationPreferenceFailure::NoSupportedTargets => Self::NoSupportedTargets,
            TranslationPreferenceFailure::UnsupportedTarget => Self::UnsupportedTarget,
            TranslationPreferenceFailure::PersistenceFailed => Self::PersistenceFailed,
        }
    }
}

impl fmt::Display for TargetLanguagePreferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidIdentifier => "invalid language identifier",
            Self::NoSupportedTargets => "no supported target languages",
            Self::UnsupportedTarget => "unsupported target language",
            Self::PersistenceFailed => "target language could not be saved",
        };
        formatter.write_str(message)
    }
}

impl Error for TargetLanguagePreferenceError {}

struct ForeignPresenter {
    observer: Arc<dyn PresentationObserver>,
}

impl ResultPresenter for ForeignPresenter {
    fn present(&self, update: PresentationUpdate) {
        self.observer
            .present(update.request_id.value(), update.state.into());
    }
}
