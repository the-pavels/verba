//! This module is available to the crate's unit tests and to dependents that
//! explicitly enable the `test-support` feature.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    capture::{CaptureFailure, CapturedText, TextCapture},
    coordinator::CancellationToken,
    presentation::TextAction,
    shortcut::{
        ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
    },
    translation::{TranslationFailure, TranslationRequest, Translator, TranslatorResponse},
};

pub type CaptureResult = Result<CapturedText, CaptureFailure>;

/// The final result is retained and reused, which keeps repeated invocations
/// deterministic without requiring every test to predict an exact call count.
pub struct FakeTextCapture {
    results: Mutex<VecDeque<CaptureResult>>,
    call_count: AtomicUsize,
}

impl FakeTextCapture {
    #[must_use]
    pub fn new(result: CaptureResult) -> Self {
        Self::with_results([result])
    }

    #[must_use]
    pub fn with_results(results: impl IntoIterator<Item = CaptureResult>) -> Self {
        let results = results.into_iter().collect::<VecDeque<_>>();
        assert!(
            !results.is_empty(),
            "FakeTextCapture requires at least one configured result"
        );

        Self {
            results: Mutex::new(results),
            call_count: AtomicUsize::new(0),
        }
    }

    #[must_use]
    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::Relaxed)
    }
}

impl TextCapture for FakeTextCapture {
    fn capture(&self) -> CaptureResult {
        self.call_count.fetch_add(1, Ordering::Relaxed);
        let mut results = self.results.lock().expect("capture fake lock was poisoned");

        if results.len() > 1 {
            results
                .pop_front()
                .expect("capture fake results were checked as non-empty")
        } else {
            results
                .front()
                .expect("capture fake results were checked as non-empty")
                .clone()
        }
    }
}

pub type TranslatorResult = Result<TranslatorResponse, TranslationFailure>;

pub struct FakeTranslator {
    results: Mutex<VecDeque<TranslatorResult>>,
    requests: Mutex<Vec<TranslationRequest>>,
}

impl FakeTranslator {
    #[must_use]
    pub fn new(result: TranslatorResult) -> Self {
        Self::with_results([result])
    }

    #[must_use]
    pub fn with_results(results: impl IntoIterator<Item = TranslatorResult>) -> Self {
        let results = results.into_iter().collect::<VecDeque<_>>();
        assert!(
            !results.is_empty(),
            "FakeTranslator requires at least one configured result"
        );

        Self {
            results: Mutex::new(results),
            requests: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn requests(&self) -> Vec<TranslationRequest> {
        self.requests
            .lock()
            .expect("translator fake request lock was poisoned")
            .clone()
    }
}

#[async_trait::async_trait]
impl Translator for FakeTranslator {
    async fn translate(
        &self,
        request: &TranslationRequest,
        _cancellation: &CancellationToken,
    ) -> TranslatorResult {
        self.requests
            .lock()
            .expect("translator fake request lock was poisoned")
            .push(request.clone());
        let mut results = self
            .results
            .lock()
            .expect("translator fake result lock was poisoned");

        if results.len() > 1 {
            results
                .pop_front()
                .expect("translator fake results were checked as non-empty")
        } else {
            results
                .front()
                .expect("translator fake results were checked as non-empty")
                .clone()
        }
    }
}

#[derive(Default)]
pub struct FakeShortcutRegistry {
    registered_shortcuts: Option<ShortcutConfiguration>,
    event_handler: Option<Arc<dyn ShortcutEventHandler>>,
    register_error: Option<ShortcutRegistryError>,
    unregister_error: Option<ShortcutRegistryError>,
    register_count: usize,
    unregister_count: usize,
}

impl FakeShortcutRegistry {
    pub fn fail_next_register_with(&mut self, error: ShortcutRegistryError) {
        self.register_error = Some(error);
    }

    pub fn fail_next_unregister_with(&mut self, error: ShortcutRegistryError) {
        self.unregister_error = Some(error);
    }

    #[must_use]
    pub const fn registered_shortcuts(&self) -> Option<ShortcutConfiguration> {
        self.registered_shortcuts
    }

    #[must_use]
    pub const fn register_count(&self) -> usize {
        self.register_count
    }

    #[must_use]
    pub const fn unregister_count(&self) -> usize {
        self.unregister_count
    }

    #[must_use]
    pub fn trigger(&self, action: TextAction) -> bool {
        let Some(event_handler) = &self.event_handler else {
            return false;
        };

        event_handler.on_shortcut(action);
        true
    }
}

impl ShortcutRegistry for FakeShortcutRegistry {
    fn register(
        &mut self,
        shortcuts: &ShortcutConfiguration,
        event_handler: Arc<dyn ShortcutEventHandler>,
    ) -> Result<(), ShortcutRegistryError> {
        self.register_count += 1;

        if let Some(error) = self.register_error.take() {
            return Err(error);
        }

        self.registered_shortcuts = Some(*shortcuts);
        self.event_handler = Some(event_handler);
        Ok(())
    }

    fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
        self.unregister_count += 1;

        if let Some(error) = self.unregister_error.take() {
            return Err(error);
        }

        self.registered_shortcuts = None;
        self.event_handler = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{FakeShortcutRegistry, FakeTextCapture};
    use crate::{
        capture::{CaptureFailure, CapturedText, TextCapture},
        presentation::TextAction,
        shortcut::{
            ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
        },
    };

    #[derive(Default)]
    struct RecordingEventHandler(Mutex<Vec<TextAction>>);

    impl ShortcutEventHandler for RecordingEventHandler {
        fn on_shortcut(&self, action: TextAction) {
            self.0.lock().expect("event lock was poisoned").push(action);
        }
    }

    #[test]
    fn capture_fake_returns_results_in_order_then_reuses_the_last() {
        let captured = CapturedText::new("Selected text").expect("fixture should be valid");
        let fake =
            FakeTextCapture::with_results([Err(CaptureFailure::TimedOut), Ok(captured.clone())]);

        assert_eq!(fake.capture(), Err(CaptureFailure::TimedOut));
        assert_eq!(fake.capture(), Ok(captured.clone()));
        assert_eq!(fake.capture(), Ok(captured));
        assert_eq!(fake.call_count(), 3);
    }

    #[test]
    fn shortcut_fake_records_lifecycle_and_delivers_events() {
        let shortcuts = ShortcutConfiguration::default();
        let handler = Arc::new(RecordingEventHandler::default());
        let mut fake = FakeShortcutRegistry::default();

        fake.register(&shortcuts, handler.clone())
            .expect("registration should succeed");
        assert!(fake.trigger(TextAction::Translate));
        assert_eq!(
            *handler.0.lock().expect("event lock was poisoned"),
            vec![TextAction::Translate]
        );
        assert_eq!(fake.registered_shortcuts(), Some(shortcuts));
        assert_eq!(fake.register_count(), 1);

        fake.unregister_all()
            .expect("unregistration should succeed");
        assert!(!fake.trigger(TextAction::Proofread));
        assert_eq!(fake.registered_shortcuts(), None);
        assert_eq!(fake.unregister_count(), 1);
    }

    #[test]
    fn shortcut_fake_can_fail_one_operation_without_changing_registration() {
        let shortcuts = ShortcutConfiguration::default();
        let handler = Arc::new(RecordingEventHandler::default());
        let mut fake = FakeShortcutRegistry::default();
        fake.fail_next_register_with(ShortcutRegistryError::RegistrationFailed);

        assert_eq!(
            fake.register(&shortcuts, handler.clone()),
            Err(ShortcutRegistryError::RegistrationFailed)
        );
        assert_eq!(fake.registered_shortcuts(), None);

        fake.register(&shortcuts, handler)
            .expect("the configured failure should be consumed");
        fake.fail_next_unregister_with(ShortcutRegistryError::UnregistrationFailed);
        assert_eq!(
            fake.unregister_all(),
            Err(ShortcutRegistryError::UnregistrationFailed)
        );
        assert_eq!(fake.registered_shortcuts(), Some(shortcuts));
    }
}
