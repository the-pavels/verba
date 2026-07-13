//! Swift-facing bridge for the Verba Rust application.

mod presentation;

pub use presentation::{
    LanguagePairViewModel, PresentationAction, PresentationViewModel, initial_presentation,
};

/// Returns the version of the Rust application embedded in Verba.
#[uniffi::export]
pub fn rust_core_version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

uniffi::setup_scaffolding!();
