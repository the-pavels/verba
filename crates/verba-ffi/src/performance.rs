use std::sync::Arc;

use verba_core::coordinator::{WorkflowMetrics, WorkflowMilestone};

use crate::PresentationAction;

#[uniffi::export(with_foreign)]
pub trait PerformanceObserver: Send + Sync {
    fn request_started(&self, request_id: u64, action: PresentationAction);
    fn capture_completed(&self, request_id: u64);
    fn processing_completed(&self, request_id: u64);
    fn request_cancelled(&self, request_id: u64);
}

pub(crate) struct ForeignWorkflowMetrics {
    observer: Arc<dyn PerformanceObserver>,
}

impl ForeignWorkflowMetrics {
    pub(crate) fn new(observer: Arc<dyn PerformanceObserver>) -> Self {
        Self { observer }
    }
}

impl WorkflowMetrics for ForeignWorkflowMetrics {
    fn record(&self, milestone: WorkflowMilestone) {
        match milestone {
            WorkflowMilestone::RequestStarted { request_id, action } => self
                .observer
                .request_started(request_id.value(), action.into()),
            WorkflowMilestone::CaptureCompleted { request_id } => {
                self.observer.capture_completed(request_id.value());
            }
            WorkflowMilestone::ProcessingCompleted { request_id } => {
                self.observer.processing_completed(request_id.value());
            }
            WorkflowMilestone::RequestCancelled { request_id } => {
                self.observer.request_cancelled(request_id.value());
            }
        }
    }
}
