use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
};

use crate::{
    capture::{CaptureFailure, CapturedText, TextCapture},
    presentation::{
        ErrorPresentation, PresentationState, ProofreadingPresentation, TextAction,
        TranslationPresentation,
    },
    shortcut::{
        ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
    },
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RequestId(u64);

impl RequestId {
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessingRequest {
    pub request_id: RequestId,
    pub action: TextAction,
    pub text: CapturedText,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessingOutcome {
    Translation(TranslationPresentation),
    Proofreading(ProofreadingPresentation),
    NoIssues,
}

impl ProcessingOutcome {
    fn into_presentation(self, action: TextAction) -> Result<PresentationState, ProcessingFailure> {
        match (action, self) {
            (TextAction::Translate, Self::Translation(result)) => {
                Ok(PresentationState::Translation(result))
            }
            (TextAction::Proofread, Self::Proofreading(result)) => {
                Ok(PresentationState::Proofreading(result))
            }
            (TextAction::Proofread, Self::NoIssues) => Ok(PresentationState::NoIssues),
            _ => Err(ProcessingFailure::InvalidOutput),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessingFailure {
    Failed,
    Cancelled,
    InvalidOutput,
}

pub trait TextActionProcessor: Send + Sync {
    fn process(
        &self,
        request: ProcessingRequest,
        cancellation: &CancellationToken,
    ) -> Result<ProcessingOutcome, ProcessingFailure>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentationUpdate {
    pub request_id: RequestId,
    pub state: PresentationState,
}

pub trait ResultPresenter: Send + Sync {
    fn present(&self, update: PresentationUpdate);
}

#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

pub struct ShortcutCoordinator {
    inner: Arc<CoordinatorInner>,
}

impl ShortcutCoordinator {
    #[must_use]
    pub fn new(
        capture: Arc<dyn TextCapture>,
        processor: Arc<dyn TextActionProcessor>,
        presenter: Arc<dyn ResultPresenter>,
    ) -> Self {
        Self {
            inner: Arc::new(CoordinatorInner {
                capture,
                processor,
                presenter,
                next_request_id: AtomicU64::new(1),
                active: Mutex::new(None),
                capture_order: Mutex::new(()),
                presentation_order: Mutex::new(()),
            }),
        }
    }

    pub fn register_shortcuts<R: ShortcutRegistry + ?Sized>(
        self: &Arc<Self>,
        registry: &mut R,
        shortcuts: &ShortcutConfiguration,
    ) -> Result<(), ShortcutRegistryError> {
        let handler: Arc<dyn ShortcutEventHandler> = self.clone();
        registry.register(shortcuts, handler)
    }

    pub fn cancel_active(&self) -> bool {
        self.inner.cancel_active(true)
    }

    pub fn shutdown(&self) {
        self.inner.cancel_active(false);
    }
}

impl ShortcutEventHandler for ShortcutCoordinator {
    fn on_shortcut(&self, action: TextAction) {
        self.inner.start(action);
    }
}

struct CoordinatorInner {
    capture: Arc<dyn TextCapture>,
    processor: Arc<dyn TextActionProcessor>,
    presenter: Arc<dyn ResultPresenter>,
    next_request_id: AtomicU64,
    active: Mutex<Option<ActiveRequest>>,
    capture_order: Mutex<()>,
    presentation_order: Mutex<()>,
}

impl CoordinatorInner {
    fn start(self: &Arc<Self>, action: TextAction) {
        let request = {
            let _presentation_guard = self
                .presentation_order
                .lock()
                .expect("presentation order lock poisoned");
            let mut active = self.active.lock().expect("active request lock poisoned");

            if active
                .as_ref()
                .is_some_and(|request| request.action == action)
            {
                return;
            }

            if let Some(request) = active.take() {
                request.cancellation.cancel();
            }

            let request = ActiveRequest {
                id: RequestId(self.next_request_id.fetch_add(1, Ordering::Relaxed)),
                action,
                cancellation: CancellationToken::default(),
            };
            *active = Some(request.clone());
            drop(active);

            self.presenter.present(PresentationUpdate {
                request_id: request.id,
                state: PresentationState::Loading { action },
            });
            request
        };

        let coordinator = Arc::clone(self);
        let worker_request = request.clone();
        if thread::Builder::new()
            .name("verba-action".to_owned())
            .spawn(move || coordinator.run(worker_request))
            .is_err()
        {
            self.complete(
                &request,
                processing_failure_presentation(action, ProcessingFailure::Failed),
            );
        }
    }

    fn run(self: Arc<Self>, request: ActiveRequest) {
        let capture_result = {
            let _capture_guard = self
                .capture_order
                .lock()
                .expect("capture order lock poisoned");
            if request.cancellation.is_cancelled() {
                return;
            }
            self.capture.capture()
        };
        let captured = match capture_result {
            Ok(text) => text,
            Err(failure) => {
                self.complete(
                    &request,
                    capture_failure_presentation(request.action, failure),
                );
                return;
            }
        };

        if request.cancellation.is_cancelled() {
            return;
        }

        let processing_request = ProcessingRequest {
            request_id: request.id,
            action: request.action,
            text: captured,
        };
        let state = match self
            .processor
            .process(processing_request, &request.cancellation)
        {
            Ok(outcome) => outcome
                .into_presentation(request.action)
                .unwrap_or_else(|failure| processing_failure_presentation(request.action, failure)),
            Err(ProcessingFailure::Cancelled) => PresentationState::Idle,
            Err(failure) => processing_failure_presentation(request.action, failure),
        };

        self.complete(&request, state);
    }

    fn complete(&self, request: &ActiveRequest, state: PresentationState) {
        let _presentation_guard = self
            .presentation_order
            .lock()
            .expect("presentation order lock poisoned");
        let mut active = self.active.lock().expect("active request lock poisoned");

        if request.cancellation.is_cancelled()
            || active.as_ref().is_none_or(|active| active.id != request.id)
        {
            return;
        }

        *active = None;
        drop(active);
        self.presenter.present(PresentationUpdate {
            request_id: request.id,
            state,
        });
    }

    fn cancel_active(&self, present_idle: bool) -> bool {
        let _presentation_guard = self
            .presentation_order
            .lock()
            .expect("presentation order lock poisoned");
        let request = self
            .active
            .lock()
            .expect("active request lock poisoned")
            .take();
        let Some(request) = request else {
            return false;
        };

        request.cancellation.cancel();
        if present_idle {
            self.presenter.present(PresentationUpdate {
                request_id: request.id,
                state: PresentationState::Idle,
            });
        }
        true
    }
}

#[derive(Clone)]
struct ActiveRequest {
    id: RequestId,
    action: TextAction,
    cancellation: CancellationToken,
}

fn capture_failure_presentation(action: TextAction, failure: CaptureFailure) -> PresentationState {
    let (title, message) = match failure {
        CaptureFailure::NoSelection => ("No text selected", "Select some text and try again."),
        CaptureFailure::TimedOut => (
            "Selection timed out",
            "Keep the source app active and try again.",
        ),
        CaptureFailure::PermissionDenied => (
            "Accessibility access required",
            "Enable Verba in System Settings and try again.",
        ),
        CaptureFailure::SecureField => (
            "Secure text can’t be captured",
            "Select text outside the secure field and try again.",
        ),
        CaptureFailure::UnsupportedContent => (
            "Text selection required",
            "The selected content does not contain readable text.",
        ),
        CaptureFailure::ClipboardUnavailable => (
            "Clipboard unavailable",
            "Verba could not safely capture the selection. Try again.",
        ),
        CaptureFailure::Cancelled => (
            "Capture cancelled",
            "The clipboard changed before capture finished. Try again.",
        ),
    };

    PresentationState::Error(ErrorPresentation {
        action: Some(action),
        title: title.to_owned(),
        message: message.to_owned(),
    })
}

fn processing_failure_presentation(
    action: TextAction,
    failure: ProcessingFailure,
) -> PresentationState {
    let (title, message) = match (action, failure) {
        (_, ProcessingFailure::Cancelled) => ("Request cancelled", "Try again."),
        (TextAction::Translate, ProcessingFailure::InvalidOutput) => (
            "Translation unavailable",
            "The translation result was invalid. Try again.",
        ),
        (TextAction::Proofread, ProcessingFailure::InvalidOutput) => (
            "Proofreading unavailable",
            "The proofreading result was invalid. Try again.",
        ),
        (TextAction::Translate, ProcessingFailure::Failed) => ("Translation failed", "Try again."),
        (TextAction::Proofread, ProcessingFailure::Failed) => ("Proofreading failed", "Try again."),
    };

    PresentationState::Error(ErrorPresentation {
        action: Some(action),
        title: title.to_owned(),
        message: message.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{
            Condvar,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::*;
    use crate::{
        presentation::{LanguagePair, ProofreadingPresentation, TranslationPresentation},
        testing::{FakeShortcutRegistry, FakeTextCapture},
    };

    #[test]
    fn registered_shortcut_captures_processes_and_presents_a_result() {
        let capture = Arc::new(FakeTextCapture::new(captured("Hallo")));
        let processor = Arc::new(QueueProcessor::new([Ok(translation())]));
        let presenter = Arc::new(RecordingPresenter::default());
        let coordinator = Arc::new(ShortcutCoordinator::new(
            capture.clone(),
            processor.clone(),
            presenter.clone(),
        ));
        let mut registry = FakeShortcutRegistry::default();

        coordinator
            .register_shortcuts(&mut registry, &ShortcutConfiguration::default())
            .unwrap();
        assert!(registry.trigger(TextAction::Translate));

        let updates = presenter.wait_for(2);
        assert_eq!(
            updates,
            vec![
                PresentationUpdate {
                    request_id: RequestId(1),
                    state: PresentationState::Loading {
                        action: TextAction::Translate,
                    },
                },
                PresentationUpdate {
                    request_id: RequestId(1),
                    state: translation_state(),
                },
            ]
        );
        assert_eq!(capture.call_count(), 1);
        assert_eq!(
            processor.requests(),
            vec![ProcessingRequest {
                request_id: RequestId(1),
                action: TextAction::Translate,
                text: CapturedText::new("Hallo").unwrap(),
            }]
        );
    }

    #[test]
    fn capture_failures_become_actionable_presentations_without_processing() {
        let cases = [
            (CaptureFailure::NoSelection, "No text selected"),
            (CaptureFailure::TimedOut, "Selection timed out"),
            (
                CaptureFailure::PermissionDenied,
                "Accessibility access required",
            ),
            (CaptureFailure::SecureField, "Secure text can’t be captured"),
            (
                CaptureFailure::UnsupportedContent,
                "Text selection required",
            ),
            (
                CaptureFailure::ClipboardUnavailable,
                "Clipboard unavailable",
            ),
            (CaptureFailure::Cancelled, "Capture cancelled"),
        ];

        for (failure, expected_title) in cases {
            let state = capture_failure_presentation(TextAction::Proofread, failure);
            let PresentationState::Error(error) = state else {
                panic!("capture failure should produce an error presentation");
            };
            assert_eq!(error.action, Some(TextAction::Proofread));
            assert_eq!(error.title, expected_title);
            assert!(!error.message.is_empty());
        }

        let capture = Arc::new(FakeTextCapture::new(Err(CaptureFailure::PermissionDenied)));
        let processor = Arc::new(QueueProcessor::new([Ok(proofreading())]));
        let presenter = Arc::new(RecordingPresenter::default());
        let coordinator = ShortcutCoordinator::new(capture, processor.clone(), presenter.clone());

        coordinator.on_shortcut(TextAction::Proofread);

        let updates = presenter.wait_for(2);
        assert!(matches!(updates[1].state, PresentationState::Error(_)));
        assert!(processor.requests().is_empty());
    }

    #[test]
    fn suppresses_a_duplicate_while_the_same_action_is_active() {
        let capture = Arc::new(BlockingCapture::new());
        let processor = Arc::new(QueueProcessor::new([Ok(translation())]));
        let presenter = Arc::new(RecordingPresenter::default());
        let coordinator = ShortcutCoordinator::new(capture.clone(), processor, presenter.clone());

        coordinator.on_shortcut(TextAction::Translate);
        capture.wait_until_started();
        coordinator.on_shortcut(TextAction::Translate);

        assert_eq!(capture.call_count.load(Ordering::Relaxed), 1);
        assert_eq!(presenter.updates().len(), 1);

        capture.release();
        let updates = presenter.wait_for(2);
        assert_eq!(updates[0].request_id, RequestId(1));
        assert_eq!(updates[1].request_id, RequestId(1));
    }

    #[test]
    fn a_different_action_waits_for_the_old_capture_and_discards_its_result() {
        let capture = Arc::new(FirstCaptureBlocking::new());
        let processor = Arc::new(QueueProcessor::new([Ok(proofreading())]));
        let presenter = Arc::new(RecordingPresenter::default());
        let coordinator = ShortcutCoordinator::new(capture.clone(), processor, presenter.clone());

        coordinator.on_shortcut(TextAction::Translate);
        capture.wait_until_first_started();
        let old_request = coordinator.inner.active.lock().unwrap().clone().unwrap();

        coordinator.on_shortcut(TextAction::Proofread);

        assert_eq!(capture.call_count.load(Ordering::Relaxed), 1);
        assert_eq!(presenter.wait_for(2).len(), 2);
        assert!(old_request.cancellation.is_cancelled());

        capture.release_first();

        let updates = presenter.wait_for(3);
        assert_eq!(
            updates
                .iter()
                .map(|update| update.request_id)
                .collect::<Vec<_>>(),
            vec![RequestId(1), RequestId(2), RequestId(2)]
        );
        assert_eq!(capture.call_count.load(Ordering::Relaxed), 2);

        coordinator
            .inner
            .complete(&old_request, translation_state());
        assert_eq!(presenter.updates().len(), 3);
    }

    #[test]
    fn explicit_cancellation_hides_loading_and_ignores_late_work() {
        let capture = Arc::new(BlockingCapture::new());
        let processor = Arc::new(QueueProcessor::new([Ok(translation())]));
        let presenter = Arc::new(RecordingPresenter::default());
        let coordinator = ShortcutCoordinator::new(capture.clone(), processor, presenter.clone());

        coordinator.on_shortcut(TextAction::Translate);
        capture.wait_until_started();
        let request = coordinator.inner.active.lock().unwrap().clone().unwrap();

        assert!(coordinator.cancel_active());
        assert!(!coordinator.cancel_active());
        assert_eq!(
            presenter.wait_for(2),
            vec![
                PresentationUpdate {
                    request_id: RequestId(1),
                    state: PresentationState::Loading {
                        action: TextAction::Translate,
                    },
                },
                PresentationUpdate {
                    request_id: RequestId(1),
                    state: PresentationState::Idle,
                },
            ]
        );
        coordinator.inner.complete(&request, translation_state());
        assert_eq!(presenter.updates().len(), 2);

        capture.release();
    }

    #[test]
    fn rejects_processor_output_for_the_wrong_action() {
        assert_eq!(
            proofreading().into_presentation(TextAction::Translate),
            Err(ProcessingFailure::InvalidOutput)
        );
        assert_eq!(
            translation().into_presentation(TextAction::Proofread),
            Err(ProcessingFailure::InvalidOutput)
        );
    }

    fn captured(text: &str) -> Result<CapturedText, CaptureFailure> {
        Ok(CapturedText::new(text).unwrap())
    }

    fn translation() -> ProcessingOutcome {
        ProcessingOutcome::Translation(TranslationPresentation {
            original_text: "Hallo".to_owned(),
            language_pair: LanguagePair {
                source: "German".to_owned(),
                target: "English".to_owned(),
            },
            translated_text: "Hello".to_owned(),
        })
    }

    fn proofreading() -> ProcessingOutcome {
        ProcessingOutcome::Proofreading(ProofreadingPresentation {
            corrected_text: "This is correct.".to_owned(),
            explanation: "Added the missing verb.".to_owned(),
        })
    }

    fn translation_state() -> PresentationState {
        match translation() {
            ProcessingOutcome::Translation(result) => PresentationState::Translation(result),
            _ => unreachable!(),
        }
    }

    struct QueueProcessor {
        outcomes: Mutex<VecDeque<Result<ProcessingOutcome, ProcessingFailure>>>,
        requests: Mutex<Vec<ProcessingRequest>>,
    }

    impl QueueProcessor {
        fn new(
            outcomes: impl IntoIterator<Item = Result<ProcessingOutcome, ProcessingFailure>>,
        ) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<ProcessingRequest> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl TextActionProcessor for QueueProcessor {
        fn process(
            &self,
            request: ProcessingRequest,
            _cancellation: &CancellationToken,
        ) -> Result<ProcessingOutcome, ProcessingFailure> {
            self.requests.lock().unwrap().push(request);
            self.outcomes
                .lock()
                .unwrap()
                .pop_front()
                .expect("processor outcome not configured")
        }
    }

    #[derive(Default)]
    struct RecordingPresenter {
        updates: Mutex<Vec<PresentationUpdate>>,
        updated: Condvar,
    }

    impl RecordingPresenter {
        fn updates(&self) -> Vec<PresentationUpdate> {
            self.updates.lock().unwrap().clone()
        }

        fn wait_for(&self, count: usize) -> Vec<PresentationUpdate> {
            let (updates, timeout) = self
                .updated
                .wait_timeout_while(
                    self.updates.lock().unwrap(),
                    Duration::from_secs(2),
                    |updates| updates.len() < count,
                )
                .unwrap();
            assert!(!timeout.timed_out(), "presentation update timed out");
            updates.clone()
        }
    }

    impl ResultPresenter for RecordingPresenter {
        fn present(&self, update: PresentationUpdate) {
            self.updates.lock().unwrap().push(update);
            self.updated.notify_all();
        }
    }

    struct BlockingCapture {
        started: (Mutex<bool>, Condvar),
        released: (Mutex<bool>, Condvar),
        call_count: AtomicUsize,
    }

    impl BlockingCapture {
        fn new() -> Self {
            Self {
                started: (Mutex::new(false), Condvar::new()),
                released: (Mutex::new(false), Condvar::new()),
                call_count: AtomicUsize::new(0),
            }
        }

        fn wait_until_started(&self) {
            let (started, timeout) = self
                .started
                .1
                .wait_timeout_while(
                    self.started.0.lock().unwrap(),
                    Duration::from_secs(2),
                    |started| !*started,
                )
                .unwrap();
            assert!(*started && !timeout.timed_out());
        }

        fn release(&self) {
            *self.released.0.lock().unwrap() = true;
            self.released.1.notify_all();
        }
    }

    impl TextCapture for BlockingCapture {
        fn capture(&self) -> Result<CapturedText, CaptureFailure> {
            self.call_count.fetch_add(1, Ordering::Relaxed);
            *self.started.0.lock().unwrap() = true;
            self.started.1.notify_all();
            let released = self
                .released
                .1
                .wait_while(self.released.0.lock().unwrap(), |released| !*released)
                .unwrap();
            drop(released);
            captured("Hallo")
        }
    }

    struct FirstCaptureBlocking {
        first_started: (Mutex<bool>, Condvar),
        first_released: (Mutex<bool>, Condvar),
        call_count: AtomicUsize,
    }

    impl FirstCaptureBlocking {
        fn new() -> Self {
            Self {
                first_started: (Mutex::new(false), Condvar::new()),
                first_released: (Mutex::new(false), Condvar::new()),
                call_count: AtomicUsize::new(0),
            }
        }

        fn wait_until_first_started(&self) {
            let (started, timeout) = self
                .first_started
                .1
                .wait_timeout_while(
                    self.first_started.0.lock().unwrap(),
                    Duration::from_secs(2),
                    |started| !*started,
                )
                .unwrap();
            assert!(*started && !timeout.timed_out());
        }

        fn release_first(&self) {
            *self.first_released.0.lock().unwrap() = true;
            self.first_released.1.notify_all();
        }
    }

    impl TextCapture for FirstCaptureBlocking {
        fn capture(&self) -> Result<CapturedText, CaptureFailure> {
            if self.call_count.fetch_add(1, Ordering::Relaxed) == 0 {
                *self.first_started.0.lock().unwrap() = true;
                self.first_started.1.notify_all();
                let released = self
                    .first_released
                    .1
                    .wait_while(self.first_released.0.lock().unwrap(), |released| !*released)
                    .unwrap();
                drop(released);
                captured("stale")
            } else {
                captured("current")
            }
        }
    }
}
