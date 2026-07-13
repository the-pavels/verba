use std::{
    error::Error,
    fmt,
    sync::{Arc, Mutex},
};

use verba_core::{
    coordinator::{
        CancellationToken, PresentationUpdate, ProcessingFailure, ProcessingOutcome,
        ProcessingRequest, ResultPresenter, ShortcutCoordinator, TextActionProcessor,
    },
    presentation::{LanguagePair, ProofreadingPresentation, TextAction, TranslationPresentation},
    shortcut::{ShortcutConfiguration, ShortcutRegistry},
};
use verba_macos::{MacOsShortcutRegistry, MacOsTextCapture};

use crate::PresentationViewModel;

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
    ) -> Result<Arc<Self>, ApplicationRuntimeError> {
        let presenter = Arc::new(ForeignPresenter { observer });
        let coordinator = Arc::new(ShortcutCoordinator::new(
            Arc::new(MacOsTextCapture::new()),
            Arc::new(PreviewProcessor),
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

struct PreviewProcessor;

impl TextActionProcessor for PreviewProcessor {
    fn process(
        &self,
        request: ProcessingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure> {
        if cancellation.is_cancelled() {
            return Err(ProcessingFailure::Cancelled);
        }

        Ok(preview_outcome(request.action, request.text.into_string()))
    }
}

fn preview_outcome(action: TextAction, text: String) -> ProcessingOutcome {
    match action {
        TextAction::Translate => ProcessingOutcome::Translation(TranslationPresentation {
            original_text: text.clone(),
            language_pair: LanguagePair {
                source: "Detected".to_owned(),
                target: "English".to_owned(),
            },
            translated_text: format!("Translation preview: {text}"),
        }),
        TextAction::Proofread => ProcessingOutcome::Proofreading(ProofreadingPresentation {
            corrected_text: text,
            explanation: "Proofreading preview".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_results_match_the_requested_action() {
        assert!(matches!(
            preview_outcome(TextAction::Translate, "Hallo".to_owned()),
            ProcessingOutcome::Translation(_)
        ));
        assert!(matches!(
            preview_outcome(TextAction::Proofread, "Text".to_owned()),
            ProcessingOutcome::Proofreading(_)
        ));
    }
}
