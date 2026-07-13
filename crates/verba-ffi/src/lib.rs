//! Swift-facing bridge for the Verba Rust application.

#[cfg(target_os = "macos")]
mod api_key_settings;
#[cfg(target_os = "macos")]
mod application;
mod presentation;
#[cfg(target_os = "macos")]
mod processor;
mod translation;

#[cfg(target_os = "macos")]
pub use api_key_settings::{OpenAiApiKeyError, OpenAiApiKeySettings};
#[cfg(target_os = "macos")]
pub use application::{
    ApplicationRuntime, ApplicationRuntimeError, PresentationObserver,
    TargetLanguagePreferenceError,
};
pub use presentation::{
    LanguagePairViewModel, PresentationAction, PresentationViewModel, initial_presentation,
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
