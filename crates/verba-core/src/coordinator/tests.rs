use std::{
    collections::VecDeque,
    future::Future,
    sync::{
        Condvar,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
    time::Duration,
};

use futures::task::{ArcWake, waker_ref};

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
fn first_proofreading_waits_for_disclosure_and_resumes_after_acknowledgement() {
    let capture = Arc::new(FakeTextCapture::new(captured("Text")));
    let processor = Arc::new(QueueProcessor::new([
        Ok(proofreading()),
        Ok(proofreading()),
    ]));
    let presenter = Arc::new(RecordingPresenter::default());
    let consent = Arc::new(TestProofreadingConsent::new(false, false));
    let coordinator = ShortcutCoordinator::with_proofreading_consent(
        capture,
        processor.clone(),
        presenter.clone(),
        consent.clone(),
    );

    coordinator.on_shortcut(TextAction::Proofread);
    assert_eq!(
        presenter.wait_for(2),
        vec![
            PresentationUpdate {
                request_id: RequestId(1),
                state: PresentationState::Loading {
                    action: TextAction::Proofread,
                },
            },
            PresentationUpdate {
                request_id: RequestId(1),
                state: PresentationState::ProofreadingDisclosure,
            },
        ]
    );
    assert!(processor.requests().is_empty());

    assert_eq!(coordinator.resolve_proofreading_disclosure(true), Ok(true));
    let updates = presenter.wait_for(4);
    assert_eq!(
        updates[2].state,
        PresentationState::Loading {
            action: TextAction::Proofread,
        }
    );
    assert!(matches!(
        updates[3].state,
        PresentationState::Proofreading(_)
    ));
    assert_eq!(processor.requests().len(), 1);
    assert!(consent.is_granted());

    coordinator.on_shortcut(TextAction::Proofread);
    let updates = presenter.wait_for(6);
    assert!(matches!(
        updates[4].state,
        PresentationState::Loading {
            action: TextAction::Proofread
        }
    ));
    assert!(matches!(
        updates[5].state,
        PresentationState::Proofreading(_)
    ));
    assert_eq!(processor.requests().len(), 2);
}

#[test]
fn cancelling_the_disclosure_drops_captured_text_without_persisting_or_processing() {
    let processor = Arc::new(QueueProcessor::new([Ok(proofreading())]));
    let presenter = Arc::new(RecordingPresenter::default());
    let consent = Arc::new(TestProofreadingConsent::new(false, false));
    let coordinator = ShortcutCoordinator::with_proofreading_consent(
        Arc::new(FakeTextCapture::new(captured("Private text"))),
        processor.clone(),
        presenter.clone(),
        consent.clone(),
    );

    coordinator.on_shortcut(TextAction::Proofread);
    presenter.wait_for(2);
    assert_eq!(coordinator.resolve_proofreading_disclosure(false), Ok(true));

    let updates = presenter.wait_for(3);
    assert_eq!(updates[2].state, PresentationState::Idle);
    assert!(processor.requests().is_empty());
    assert!(!consent.is_granted());
    assert_eq!(coordinator.resolve_proofreading_disclosure(true), Ok(false));
}

#[test]
fn failed_disclosure_persistence_prevents_processing() {
    let processor = Arc::new(QueueProcessor::new([Ok(proofreading())]));
    let presenter = Arc::new(RecordingPresenter::default());
    let coordinator = ShortcutCoordinator::with_proofreading_consent(
        Arc::new(FakeTextCapture::new(captured("Private text"))),
        processor.clone(),
        presenter.clone(),
        Arc::new(TestProofreadingConsent::new(false, true)),
    );

    coordinator.on_shortcut(TextAction::Proofread);
    presenter.wait_for(2);
    assert_eq!(
        coordinator.resolve_proofreading_disclosure(true),
        Err(ProofreadingConsentStoreError)
    );

    let updates = presenter.wait_for(3);
    assert!(matches!(updates[2].state, PresentationState::Error(_)));
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
fn cancellation_wakes_pending_async_work() {
    #[derive(Default)]
    struct WakeCounter(AtomicUsize);

    impl ArcWake for WakeCounter {
        fn wake_by_ref(counter: &Arc<Self>) {
            counter.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let cancellation = CancellationToken::default();
    let counter = Arc::new(WakeCounter::default());
    let waker = waker_ref(&counter);
    let mut context = Context::from_waker(&waker);
    let mut future = Box::pin(cancellation.cancelled());

    assert_eq!(future.as_mut().poll(&mut context), Poll::Pending);
    cancellation.cancel();
    assert_eq!(counter.0.load(Ordering::Relaxed), 1);
    assert_eq!(future.as_mut().poll(&mut context), Poll::Ready(()));
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

#[test]
fn unsupported_translation_points_to_the_target_language_setting() {
    let PresentationState::Error(error) = processing_failure_presentation(
        TextAction::Translate,
        ProcessingFailure::UnsupportedConfiguration,
    ) else {
        panic!("processing failure should produce an error presentation");
    };

    assert_eq!(error.title, "Language pair unavailable");
    assert!(error.message.contains("Settings"));
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

struct TestProofreadingConsent {
    granted: AtomicBool,
    fail_grant: bool,
}

impl TestProofreadingConsent {
    fn new(granted: bool, fail_grant: bool) -> Self {
        Self {
            granted: AtomicBool::new(granted),
            fail_grant,
        }
    }
}

impl ProofreadingConsentGate for TestProofreadingConsent {
    fn is_granted(&self) -> bool {
        self.granted.load(Ordering::Acquire)
    }

    fn grant(&self) -> Result<(), ProofreadingConsentStoreError> {
        if self.fail_grant {
            return Err(ProofreadingConsentStoreError);
        }
        self.granted.store(true, Ordering::Release);
        Ok(())
    }
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
