use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use verba_core::{
    coordinator::{PresentationUpdate, ResultPresenter, ShortcutCoordinator},
    proofreading::ProofreadingConsentPreferences,
    shortcut::{
        ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutSettingsStore,
    },
    translation::{LanguageIdentifier, TranslationPreferenceFailure, TranslationPreferences},
};
use verba_macos::{
    MacOsProofreadingConsentStore, MacOsShortcutRegistry, MacOsShortcutSettingsStore,
    MacOsTextCapture, MacOsTranslationSettingsStore,
};
use verba_openai::{OpenAiClient, OpenAiConfig, OpenAiProofreader};

use crate::{
    PresentationAction, PresentationViewModel,
    api_key_settings::{SecretStoreApiKeyProvider, openai_secret_store},
    processor::ApplicationProcessor,
    shortcut_settings::{
        ShortcutConfigurationViewModel, ShortcutInput, ShortcutSettingsAction,
        ShortcutSettingsError, register_and_save, replacement_configuration,
    },
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
    shortcut_configuration: Mutex<ShortcutConfiguration>,
    shortcut_settings_store: Arc<dyn ShortcutSettingsStore>,
    translation_preferences: Arc<TranslationPreferences>,
    lifecycle: Mutex<ApplicationLifecycle>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ApplicationLifecycle {
    Running,
    Suspended,
    ShutDown,
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
        let shortcut_settings_store: Arc<dyn ShortcutSettingsStore> =
            Arc::new(MacOsShortcutSettingsStore::new());
        let shortcut_configuration = shortcut_settings_store
            .load()
            .map_err(|_| ApplicationRuntimeError::SettingsUnavailable)?
            .unwrap_or_default();
        let mut shortcut_registry = MacOsShortcutRegistry::new();
        coordinator
            .register_shortcuts(&mut shortcut_registry, &shortcut_configuration)
            .map_err(|_| ApplicationRuntimeError::ShortcutRegistrationFailed)?;

        Ok(Arc::new(Self {
            coordinator,
            shortcut_registry: Mutex::new(shortcut_registry),
            shortcut_configuration: Mutex::new(shortcut_configuration),
            shortcut_settings_store,
            translation_preferences,
            lifecycle: Mutex::new(ApplicationLifecycle::Running),
        }))
    }

    pub fn cancel_active(&self) -> bool {
        self.coordinator.cancel_active()
    }

    pub fn prepare_for_sleep(&self) -> Result<(), RuntimeLifecycleError> {
        self.stop(ApplicationLifecycle::Suspended)
    }

    pub fn resume_after_wake(&self) -> Result<(), RuntimeLifecycleError> {
        let mut lifecycle = self.lifecycle.lock().expect("lifecycle lock poisoned");
        match *lifecycle {
            ApplicationLifecycle::Running => return Ok(()),
            ApplicationLifecycle::ShutDown => return Err(RuntimeLifecycleError::ShutDown),
            ApplicationLifecycle::Suspended => {}
        }

        let configuration = *self
            .shortcut_configuration
            .lock()
            .expect("shortcut configuration lock poisoned");
        let mut registry = self
            .shortcut_registry
            .lock()
            .expect("shortcut registry lock poisoned");
        self.coordinator
            .register_shortcuts(&mut *registry, &configuration)
            .map_err(|_| RuntimeLifecycleError::ShortcutRegistrationFailed)?;
        *lifecycle = ApplicationLifecycle::Running;
        Ok(())
    }

    pub fn shutdown(&self) {
        let _ = self.stop(ApplicationLifecycle::ShutDown);
    }

    pub fn retry(&self, action: PresentationAction) {
        self.coordinator.on_shortcut(action.into());
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

    pub fn shortcut_configuration(&self) -> ShortcutConfigurationViewModel {
        (*self
            .shortcut_configuration
            .lock()
            .expect("shortcut configuration lock poisoned"))
        .into()
    }

    pub fn set_shortcut(
        &self,
        action: ShortcutSettingsAction,
        input: ShortcutInput,
    ) -> Result<ShortcutConfigurationViewModel, ShortcutSettingsError> {
        let mut configuration = self
            .shortcut_configuration
            .lock()
            .expect("shortcut configuration lock poisoned");
        let replacement = replacement_configuration(*configuration, action, input)?;
        let mut registry = self
            .shortcut_registry
            .lock()
            .expect("shortcut registry lock poisoned");
        let event_handler: Arc<dyn ShortcutEventHandler> = self.coordinator.clone();
        register_and_save(
            &mut *registry,
            event_handler,
            self.shortcut_settings_store.as_ref(),
            *configuration,
            replacement,
        )?;
        *configuration = replacement;
        Ok(replacement.into())
    }
}

impl ApplicationRuntime {
    fn stop(&self, target: ApplicationLifecycle) -> Result<(), RuntimeLifecycleError> {
        let mut lifecycle = self.lifecycle.lock().expect("lifecycle lock poisoned");
        if *lifecycle == ApplicationLifecycle::ShutDown
            || (*lifecycle == ApplicationLifecycle::Suspended
                && target == ApplicationLifecycle::Suspended)
        {
            return Ok(());
        }

        if target == ApplicationLifecycle::Suspended {
            self.coordinator.cancel_active();
        } else {
            self.coordinator.shutdown();
        }
        let result = self
            .shortcut_registry
            .lock()
            .expect("shortcut registry lock poisoned")
            .unregister_all()
            .map_err(|_| RuntimeLifecycleError::ShortcutUnregistrationFailed);
        *lifecycle = target;
        result
    }
}

impl Drop for ApplicationRuntime {
    fn drop(&mut self) {
        let _ = self.stop(ApplicationLifecycle::ShutDown);
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
pub enum RuntimeLifecycleError {
    ShortcutRegistrationFailed,
    ShortcutUnregistrationFailed,
    ShutDown,
}

impl fmt::Display for RuntimeLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ShortcutRegistrationFailed => "shortcut registration failed after wake",
            Self::ShortcutUnregistrationFailed => "shortcut unregistration failed",
            Self::ShutDown => "application runtime is shut down",
        };
        formatter.write_str(message)
    }
}

impl Error for RuntimeLifecycleError {}

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
