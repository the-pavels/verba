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

    pub(crate) fn cancel(&self) {
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
mod tests;
