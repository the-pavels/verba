use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    task::{Context, Poll, Waker},
    thread,
};

use crate::{
    capture::{CaptureFailure, CapturedText, TextCapture},
    presentation::{
        ErrorPresentation, PresentationState, ProofreadingPresentation, TextAction,
        TranslationPresentation,
    },
    proofreading::{ProofreaderError, ProofreadingConsentGate, ProofreadingConsentStoreError},
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
    UnsupportedConfiguration,
    ProofreadingInputTooLong,
    ProofreadingProvider(ProofreaderError),
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

#[derive(Clone)]
pub struct CancellationToken {
    state: Arc<CancellationState>,
}

struct CancellationState {
    cancelled: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self {
            state: Arc::new(CancellationState {
                cancelled: AtomicBool::new(false),
                wakers: Mutex::new(Vec::new()),
            }),
        }
    }
}

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.state.cancelled.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn cancelled(&self) -> CancellationFuture {
        CancellationFuture {
            state: Arc::clone(&self.state),
        }
    }

    pub fn cancel(&self) {
        if self.state.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }

        let wakers = self
            .state
            .wakers
            .lock()
            .expect("cancellation waker lock poisoned")
            .drain(..)
            .collect::<Vec<_>>();
        for waker in wakers {
            waker.wake();
        }
    }
}

pub struct CancellationFuture {
    state: Arc<CancellationState>,
}

impl Future for CancellationFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.cancelled.load(Ordering::Acquire) {
            return Poll::Ready(());
        }

        let mut wakers = self
            .state
            .wakers
            .lock()
            .expect("cancellation waker lock poisoned");
        if self.state.cancelled.load(Ordering::Acquire) {
            Poll::Ready(())
        } else {
            if !wakers.iter().any(|waker| waker.will_wake(context.waker())) {
                wakers.push(context.waker().clone());
            }
            Poll::Pending
        }
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
        Self::with_proofreading_consent(
            capture,
            processor,
            presenter,
            Arc::new(AlwaysGrantedProofreadingConsent),
        )
    }

    #[must_use]
    pub fn with_proofreading_consent(
        capture: Arc<dyn TextCapture>,
        processor: Arc<dyn TextActionProcessor>,
        presenter: Arc<dyn ResultPresenter>,
        proofreading_consent: Arc<dyn ProofreadingConsentGate>,
    ) -> Self {
        Self {
            inner: Arc::new(CoordinatorInner {
                capture,
                processor,
                presenter,
                proofreading_consent,
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

    pub fn resolve_proofreading_disclosure(
        &self,
        accepted: bool,
    ) -> Result<bool, ProofreadingConsentStoreError> {
        self.inner.resolve_proofreading_disclosure(accepted)
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
    proofreading_consent: Arc<dyn ProofreadingConsentGate>,
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
                pending_text: Arc::new(Mutex::new(None)),
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

        if request.action == TextAction::Proofread && !self.proofreading_consent.is_granted() {
            self.present_proofreading_disclosure(&request, captured);
            return;
        }

        self.process_captured(request, captured);
    }

    fn process_captured(self: Arc<Self>, request: ActiveRequest, captured: CapturedText) {
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

    fn present_proofreading_disclosure(&self, request: &ActiveRequest, captured: CapturedText) {
        let _presentation_guard = self
            .presentation_order
            .lock()
            .expect("presentation order lock poisoned");
        let active = self.active.lock().expect("active request lock poisoned");
        if request.cancellation.is_cancelled()
            || active.as_ref().is_none_or(|active| active.id != request.id)
        {
            return;
        }

        *request
            .pending_text
            .lock()
            .expect("pending proofreading text lock poisoned") = Some(captured);
        self.presenter.present(PresentationUpdate {
            request_id: request.id,
            state: PresentationState::ProofreadingDisclosure,
        });
    }

    fn resolve_proofreading_disclosure(
        self: &Arc<Self>,
        accepted: bool,
    ) -> Result<bool, ProofreadingConsentStoreError> {
        let request = {
            let active = self.active.lock().expect("active request lock poisoned");
            active
                .as_ref()
                .filter(|request| {
                    request.action == TextAction::Proofread
                        && request
                            .pending_text
                            .lock()
                            .expect("pending proofreading text lock poisoned")
                            .is_some()
                })
                .cloned()
        };
        let Some(request) = request else {
            return Ok(false);
        };

        if !accepted {
            self.cancel_active(true);
            return Ok(true);
        }

        if let Err(error) = self.proofreading_consent.grant() {
            self.complete(
                &request,
                PresentationState::Error(ErrorPresentation {
                    action: Some(TextAction::Proofread),
                    title: "Privacy setting unavailable".to_owned(),
                    message: "Verba couldn’t save your acknowledgement. Try again.".to_owned(),
                }),
            );
            return Err(error);
        }

        let captured = {
            let _presentation_guard = self
                .presentation_order
                .lock()
                .expect("presentation order lock poisoned");
            let active = self.active.lock().expect("active request lock poisoned");
            if request.cancellation.is_cancelled()
                || active.as_ref().is_none_or(|active| active.id != request.id)
            {
                return Ok(false);
            }
            let captured = request
                .pending_text
                .lock()
                .expect("pending proofreading text lock poisoned")
                .take();
            if captured.is_some() {
                self.presenter.present(PresentationUpdate {
                    request_id: request.id,
                    state: PresentationState::Loading {
                        action: TextAction::Proofread,
                    },
                });
            }
            captured
        };
        let Some(captured) = captured else {
            return Ok(false);
        };

        let coordinator = Arc::clone(self);
        let worker_request = request.clone();
        if thread::Builder::new()
            .name("verba-action".to_owned())
            .spawn(move || coordinator.process_captured(worker_request, captured))
            .is_err()
        {
            self.complete(
                &request,
                processing_failure_presentation(TextAction::Proofread, ProcessingFailure::Failed),
            );
        }
        Ok(true)
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
    pending_text: Arc<Mutex<Option<CapturedText>>>,
}

struct AlwaysGrantedProofreadingConsent;

impl ProofreadingConsentGate for AlwaysGrantedProofreadingConsent {
    fn is_granted(&self) -> bool {
        true
    }

    fn grant(&self) -> Result<(), ProofreadingConsentStoreError> {
        Ok(())
    }
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
        (TextAction::Translate, ProcessingFailure::UnsupportedConfiguration) => (
            "Language pair unavailable",
            "Choose a different target language in Settings and try again.",
        ),
        (TextAction::Proofread, ProcessingFailure::UnsupportedConfiguration) => (
            "Proofreading unavailable",
            "Check your settings and try again.",
        ),
        (TextAction::Proofread, ProcessingFailure::ProofreadingInputTooLong) => (
            "Selection too long",
            "Select at most 10,000 characters and invoke Proofread again.",
        ),
        (TextAction::Proofread, ProcessingFailure::ProofreadingProvider(error)) => {
            proofreading_provider_failure(error)
        }
        (
            TextAction::Translate,
            ProcessingFailure::ProofreadingInputTooLong
            | ProcessingFailure::ProofreadingProvider(_),
        ) => ("Translation failed", "Invoke Translate again."),
        (TextAction::Translate, ProcessingFailure::Failed) => ("Translation failed", "Try again."),
        (TextAction::Proofread, ProcessingFailure::Failed) => ("Proofreading failed", "Try again."),
    };

    PresentationState::Error(ErrorPresentation {
        action: Some(action),
        title: title.to_owned(),
        message: message.to_owned(),
    })
}

fn proofreading_provider_failure(error: ProofreaderError) -> (&'static str, &'static str) {
    match error {
        ProofreaderError::MissingCredential => (
            "OpenAI API key required",
            "Add your API key in Settings, then invoke Proofread again.",
        ),
        ProofreaderError::Authentication => (
            "OpenAI API key rejected",
            "Replace the key in Settings before invoking Proofread again.",
        ),
        ProofreaderError::RateLimited => (
            "OpenAI rate limit reached",
            "Wait a moment, then invoke Proofread again.",
        ),
        ProofreaderError::QuotaExceeded => (
            "OpenAI quota unavailable",
            "Check your OpenAI billing and usage limits before trying again.",
        ),
        ProofreaderError::Offline => (
            "No internet connection",
            "Reconnect to the internet, then invoke Proofread again.",
        ),
        ProofreaderError::TimedOut => (
            "OpenAI request timed out",
            "Check your connection, then invoke Proofread again.",
        ),
        ProofreaderError::Refused => (
            "Proofreading refused",
            "Select different text before trying again.",
        ),
        ProofreaderError::Incomplete | ProofreaderError::MalformedResponse => (
            "Invalid proofreading response",
            "OpenAI returned an unusable result. Invoke Proofread again.",
        ),
        ProofreaderError::ServiceUnavailable => (
            "OpenAI unavailable",
            "Wait a moment, then invoke Proofread again.",
        ),
        ProofreaderError::Cancelled => ("Request cancelled", "Invoke Proofread to try again."),
        ProofreaderError::Failed => (
            "Proofreading failed",
            "Check your connection and settings, then invoke Proofread again.",
        ),
    }
}

#[cfg(test)]
mod tests;
