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
        ErrorPresentation, PresentationState, ProofreadingPresentation, RecoveryAction, TextAction,
        TranslationPresentation,
    },
    proofreading::{
        ProofreaderError, ProofreadingConsentGate, ProofreadingConsentStoreError,
        ProofreadingPolicyViolation,
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
    EmptyInput,
    InputTooLong,
    SameLanguage,
    InvalidOutput,
    ProofreadingPolicyViolation(ProofreadingPolicyViolation),
    UnsupportedConfiguration,
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

    fn capture_completed(&self, _request_id: RequestId) {}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowMilestone {
    RequestStarted {
        request_id: RequestId,
        action: TextAction,
    },
    CaptureCompleted {
        request_id: RequestId,
    },
    ProcessingCompleted {
        request_id: RequestId,
    },
    RequestCancelled {
        request_id: RequestId,
    },
}

pub trait WorkflowMetrics: Send + Sync {
    fn record(&self, milestone: WorkflowMilestone);
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
        Self::with_proofreading_consent_and_metrics(
            capture,
            processor,
            presenter,
            proofreading_consent,
            Arc::new(NoopWorkflowMetrics),
        )
    }

    #[must_use]
    pub fn with_metrics(
        capture: Arc<dyn TextCapture>,
        processor: Arc<dyn TextActionProcessor>,
        presenter: Arc<dyn ResultPresenter>,
        metrics: Arc<dyn WorkflowMetrics>,
    ) -> Self {
        Self::with_proofreading_consent_and_metrics(
            capture,
            processor,
            presenter,
            Arc::new(AlwaysGrantedProofreadingConsent),
            metrics,
        )
    }

    #[must_use]
    pub fn with_proofreading_consent_and_metrics(
        capture: Arc<dyn TextCapture>,
        processor: Arc<dyn TextActionProcessor>,
        presenter: Arc<dyn ResultPresenter>,
        proofreading_consent: Arc<dyn ProofreadingConsentGate>,
        metrics: Arc<dyn WorkflowMetrics>,
    ) -> Self {
        Self {
            inner: Arc::new(CoordinatorInner {
                capture,
                processor,
                presenter,
                proofreading_consent,
                metrics,
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
    metrics: Arc<dyn WorkflowMetrics>,
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
                self.metrics.record(WorkflowMilestone::RequestCancelled {
                    request_id: request.id,
                });
            }

            let request = ActiveRequest {
                id: RequestId(self.next_request_id.fetch_add(1, Ordering::Relaxed)),
                action,
                cancellation: CancellationToken::default(),
                pending_text: Arc::new(Mutex::new(None)),
            };
            *active = Some(request.clone());
            drop(active);

            self.metrics.record(WorkflowMilestone::RequestStarted {
                request_id: request.id,
                action,
            });

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
        self.presenter.capture_completed(request.id);
        self.metrics.record(WorkflowMilestone::CaptureCompleted {
            request_id: request.id,
        });
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
        let processing_result = self
            .processor
            .process(processing_request, &request.cancellation);
        self.metrics.record(WorkflowMilestone::ProcessingCompleted {
            request_id: request.id,
        });
        let state = match processing_result {
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
                    recovery: RecoveryAction::Retry,
                    diagnostic_code: "proofreading.consent-persistence".to_owned(),
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
        self.metrics.record(WorkflowMilestone::RequestCancelled {
            request_id: request.id,
        });
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

struct NoopWorkflowMetrics;

impl WorkflowMetrics for NoopWorkflowMetrics {
    fn record(&self, _milestone: WorkflowMilestone) {}
}

impl ProofreadingConsentGate for AlwaysGrantedProofreadingConsent {
    fn is_granted(&self) -> bool {
        true
    }

    fn grant(&self) -> Result<(), ProofreadingConsentStoreError> {
        Ok(())
    }
}

fn capture_failure_presentation(action: TextAction, failure: CaptureFailure) -> PresentationState {
    let (title, message, recovery, diagnostic_code) = match failure {
        CaptureFailure::NoSelection => (
            "No text selected",
            "Select some text, then invoke the action again.",
            RecoveryAction::Dismiss,
            "capture.no-selection",
        ),
        CaptureFailure::TimedOut => (
            "Selection timed out",
            "Keep the source app active and try again.",
            RecoveryAction::Retry,
            "capture.timed-out",
        ),
        CaptureFailure::PermissionDenied => (
            "Accessibility access required",
            "Allow Verba to capture selected text, then try again.",
            RecoveryAction::GrantAccessibility,
            "capture.permission-denied",
        ),
        CaptureFailure::SecureField => (
            "Secure text can’t be captured",
            "Select text outside the secure field, then invoke the action again.",
            RecoveryAction::Dismiss,
            "capture.secure-field",
        ),
        CaptureFailure::FieldSecurityUnavailable => (
            "Selection safety couldn’t be verified",
            "Keep the source app active, select text in a standard field, and try again.",
            RecoveryAction::Retry,
            "capture.field-security-unavailable",
        ),
        CaptureFailure::UnsupportedContent => (
            "Text selection required",
            "The selected content does not contain readable text.",
            RecoveryAction::Dismiss,
            "capture.unsupported-content",
        ),
        CaptureFailure::ClipboardUnavailable => (
            "Clipboard unavailable",
            "Verba could not safely capture the selection. Try again.",
            RecoveryAction::Retry,
            "capture.clipboard-unavailable",
        ),
        CaptureFailure::Cancelled => (
            "Capture cancelled",
            "The clipboard changed before capture finished. Try again.",
            RecoveryAction::Retry,
            "capture.cancelled",
        ),
    };

    PresentationState::Error(ErrorPresentation {
        action: Some(action),
        title: title.to_owned(),
        message: message.to_owned(),
        recovery,
        diagnostic_code: diagnostic_code.to_owned(),
    })
}

fn processing_failure_presentation(
    action: TextAction,
    failure: ProcessingFailure,
) -> PresentationState {
    let (title, message, recovery, diagnostic_code) = match (action, failure) {
        (_, ProcessingFailure::Cancelled) => (
            "Request cancelled",
            "Invoke the action again when you’re ready.",
            RecoveryAction::Dismiss,
            "processing.cancelled",
        ),
        (TextAction::Translate, ProcessingFailure::InvalidOutput) => (
            "Translation unavailable",
            "Verba couldn’t use the translation result. Try again.",
            RecoveryAction::Retry,
            "translation.invalid-output",
        ),
        (TextAction::Proofread, ProcessingFailure::InvalidOutput) => (
            "Proofreading unavailable",
            "Verba couldn’t use the proofreading result. Try again.",
            RecoveryAction::Retry,
            "proofreading.invalid-output",
        ),
        (TextAction::Proofread, ProcessingFailure::ProofreadingPolicyViolation(violation)) => {
            proofreading_policy_violation(violation)
        }
        (TextAction::Translate, ProcessingFailure::ProofreadingPolicyViolation(_)) => (
            "Translation failed",
            "Try translating the selection again.",
            RecoveryAction::Retry,
            "translation.unexpected-failure-kind",
        ),
        (TextAction::Translate, ProcessingFailure::UnsupportedConfiguration) => (
            "Language pair unavailable",
            "Choose a different target language, then try again.",
            RecoveryAction::ChangeLanguage,
            "translation.unsupported-configuration",
        ),
        (TextAction::Proofread, ProcessingFailure::UnsupportedConfiguration) => (
            "Proofreading setup required",
            "Review the proofreading settings, then try again.",
            RecoveryAction::OpenSettings,
            "proofreading.unsupported-configuration",
        ),
        (_, ProcessingFailure::EmptyInput) => (
            "No text to process",
            "Select some text, then invoke the action again.",
            RecoveryAction::Dismiss,
            "processing.empty-input",
        ),
        (TextAction::Translate, ProcessingFailure::InputTooLong) => (
            "Selection too long",
            "Select at most 10,000 characters and invoke Translate again.",
            RecoveryAction::Dismiss,
            "translation.input-too-long",
        ),
        (TextAction::Proofread, ProcessingFailure::InputTooLong) => (
            "Selection too long",
            "Select at most 10,000 characters and invoke Proofread again.",
            RecoveryAction::Dismiss,
            "proofreading.input-too-long",
        ),
        (TextAction::Translate, ProcessingFailure::SameLanguage) => (
            "Text is already in the target language",
            "Choose a different target language or select different text.",
            RecoveryAction::ChangeLanguage,
            "translation.same-language",
        ),
        (TextAction::Proofread, ProcessingFailure::SameLanguage) => (
            "Proofreading failed",
            "Try proofreading the selection again.",
            RecoveryAction::Retry,
            "proofreading.unexpected-same-language",
        ),
        (TextAction::Proofread, ProcessingFailure::ProofreadingProvider(error)) => {
            proofreading_provider_failure(error)
        }
        (TextAction::Translate, ProcessingFailure::ProofreadingProvider(_)) => (
            "Translation failed",
            "Try translating the selection again.",
            RecoveryAction::Retry,
            "translation.unexpected-failure-kind",
        ),
        (TextAction::Translate, ProcessingFailure::Failed) => (
            "Translation failed",
            "Try translating the selection again.",
            RecoveryAction::Retry,
            "translation.failed",
        ),
        (TextAction::Proofread, ProcessingFailure::Failed) => (
            "Proofreading failed",
            "Try proofreading the selection again.",
            RecoveryAction::Retry,
            "proofreading.failed",
        ),
    };

    PresentationState::Error(ErrorPresentation {
        action: Some(action),
        title: title.to_owned(),
        message: message.to_owned(),
        recovery,
        diagnostic_code: diagnostic_code.to_owned(),
    })
}

fn proofreading_policy_violation(
    violation: ProofreadingPolicyViolation,
) -> (&'static str, &'static str, RecoveryAction, &'static str) {
    let diagnostic_code = match violation {
        ProofreadingPolicyViolation::OuterWhitespace => {
            "proofreading.policy-violation.outer-whitespace"
        }
        ProofreadingPolicyViolation::LineStructure => {
            "proofreading.policy-violation.line-structure"
        }
        ProofreadingPolicyViolation::FormattingMarkers => {
            "proofreading.policy-violation.formatting-markers"
        }
    };
    (
        "Proofreading result rejected",
        "The result changed protected text formatting. Try proofreading again.",
        RecoveryAction::Retry,
        diagnostic_code,
    )
}

fn proofreading_provider_failure(
    error: ProofreaderError,
) -> (&'static str, &'static str, RecoveryAction, &'static str) {
    match error {
        ProofreaderError::MissingCredential => (
            "OpenAI API key required",
            "Add your API key, then invoke Proofread again.",
            RecoveryAction::OpenSettings,
            "proofreading.provider.missing-credential",
        ),
        ProofreaderError::Authentication => (
            "OpenAI API key rejected",
            "Replace the key before invoking Proofread again.",
            RecoveryAction::OpenSettings,
            "proofreading.provider.authentication",
        ),
        ProofreaderError::RateLimited => (
            "OpenAI rate limit reached",
            "Wait a moment, then invoke Proofread again.",
            RecoveryAction::Retry,
            "proofreading.provider.rate-limited",
        ),
        ProofreaderError::QuotaExceeded => (
            "OpenAI quota unavailable",
            "Check your OpenAI account limits or replace the API key.",
            RecoveryAction::OpenSettings,
            "proofreading.provider.quota-exceeded",
        ),
        ProofreaderError::Offline => (
            "No internet connection",
            "Reconnect to the internet, then invoke Proofread again.",
            RecoveryAction::Retry,
            "proofreading.provider.offline",
        ),
        ProofreaderError::TimedOut => (
            "OpenAI request timed out",
            "Check your connection, then invoke Proofread again.",
            RecoveryAction::Retry,
            "proofreading.provider.timed-out",
        ),
        ProofreaderError::Refused => (
            "Proofreading refused",
            "Select different text, then invoke Proofread again.",
            RecoveryAction::Dismiss,
            "proofreading.provider.refused",
        ),
        ProofreaderError::Incomplete => (
            "Invalid proofreading response",
            "Verba couldn’t use the response. Try again.",
            RecoveryAction::Retry,
            "proofreading.provider.incomplete",
        ),
        ProofreaderError::MalformedResponse => (
            "Invalid proofreading response",
            "Verba couldn’t use the response. Try again.",
            RecoveryAction::Retry,
            "proofreading.provider.malformed-response",
        ),
        ProofreaderError::ServiceUnavailable => (
            "OpenAI unavailable",
            "Wait a moment, then invoke Proofread again.",
            RecoveryAction::Retry,
            "proofreading.provider.service-unavailable",
        ),
        ProofreaderError::Cancelled => (
            "Request cancelled",
            "Invoke Proofread again when you’re ready.",
            RecoveryAction::Dismiss,
            "proofreading.provider.cancelled",
        ),
        ProofreaderError::Failed => (
            "Proofreading failed",
            "Check your connection, then try again.",
            RecoveryAction::Retry,
            "proofreading.provider.failed",
        ),
    }
}

#[cfg(test)]
mod tests;
