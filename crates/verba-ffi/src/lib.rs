//! Swift-facing bridge for the Verba Rust application.

#[cfg(target_os = "macos")]
mod api_key_settings;
#[cfg(target_os = "macos")]
mod application;
mod presentation;
#[cfg(target_os = "macos")]
mod processor;
#[cfg(target_os = "macos")]
mod shortcut_settings;
mod translation;
#[cfg(all(test, target_os = "macos"))]
mod workflow_smoke_tests;

#[cfg(target_os = "macos")]
pub use api_key_settings::{OpenAiApiKeyError, OpenAiApiKeySettings};
#[cfg(target_os = "macos")]
pub use application::{
    ApplicationRuntime, ApplicationRuntimeError, PresentationObserver, ProofreadingDisclosureError,
    TargetLanguagePreferenceError,
};
pub use presentation::{
    LanguagePairViewModel, PresentationAction, PresentationViewModel, RecoveryActionViewModel,
    initial_presentation,
};
#[cfg(target_os = "macos")]
pub use shortcut_settings::{
    ShortcutConfigurationViewModel, ShortcutInput, ShortcutSettingsAction, ShortcutSettingsError,
};
pub use translation::{
    NativeTranslationError, NativeTranslationRequest, NativeTranslationResponse, NativeTranslator,
};

/// Returns the version of the Rust application embedded in Verba.
#[uniffi::export]
pub fn rust_core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

uniffi::setup_scaffolding!();
