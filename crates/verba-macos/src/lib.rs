//! macOS-specific adapters implemented in Rust.

#[cfg(target_os = "macos")]
mod capture;
#[cfg(target_os = "macos")]
mod pasteboard;
#[cfg(target_os = "macos")]
mod settings;
#[cfg(target_os = "macos")]
mod shortcut;

#[cfg(target_os = "macos")]
pub use capture::MacOsTextCapture;
#[cfg(target_os = "macos")]
pub use pasteboard::{
    MacOsPasteboard, PasteboardRestoreOutcome, PasteboardSnapshot, PasteboardSnapshotError,
};
#[cfg(target_os = "macos")]
pub use settings::MacOsTranslationSettingsStore;
#[cfg(target_os = "macos")]
pub use shortcut::MacOsShortcutRegistry;
