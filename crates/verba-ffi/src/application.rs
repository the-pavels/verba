use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use verba_core::{
    coordinator::{PresentationUpdate, ResultPresenter, ShortcutCoordinator},
    shortcut::{ShortcutConfiguration, ShortcutRegistry},
};
use verba_macos::{MacOsShortcutRegistry, MacOsTextCapture};

use crate::{
    PresentationViewModel, processor::ApplicationProcessor, translation::NativeTranslator,
};

#[uniffi::export(with_foreign)]
pub trait PresentationObserver: Send + Sync {
    fn present(&self, request_id: u64, presentation: PresentationViewModel);
}

#[derive(uniffi::Object)]
pub struct ApplicationRuntime {
    coordinator: Arc<ShortcutCoordinator>,
    shortcut_registry: Mutex<MacOsShortcutRegistry>,
}

#[uniffi::export]
impl ApplicationRuntime {
    #[uniffi::constructor]
    pub fn new(
        observer: Arc<dyn PresentationObserver>,
        translator: Arc<dyn NativeTranslator>,
    ) -> Result<Arc<Self>, ApplicationRuntimeError> {
        let presenter = Arc::new(ForeignPresenter { observer });
        let coordinator = Arc::new(ShortcutCoordinator::new(
            Arc::new(MacOsTextCapture::new()),
            Arc::new(ApplicationProcessor::new(translator)),
            presenter,
        ));
        let mut shortcut_registry = MacOsShortcutRegistry::new();
        coordinator
            .register_shortcuts(&mut shortcut_registry, &ShortcutConfiguration::default())
            .map_err(|_| ApplicationRuntimeError::ShortcutRegistrationFailed)?;

        Ok(Arc::new(Self {
            coordinator,
            shortcut_registry: Mutex::new(shortcut_registry),
        }))
    }

    pub fn cancel_active(&self) -> bool {
        self.coordinator.cancel_active()
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
}

impl fmt::Display for ApplicationRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortcutRegistrationFailed => formatter.write_str("shortcut registration failed"),
        }
    }
}

impl Error for ApplicationRuntimeError {}

struct ForeignPresenter {
    observer: Arc<dyn PresentationObserver>,
}

impl ResultPresenter for ForeignPresenter {
    fn present(&self, update: PresentationUpdate) {
        self.observer
            .present(update.request_id.value(), update.state.into());
    }
}
