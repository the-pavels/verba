mod accessibility;
mod synthetic_copy;

use std::time::{Duration, Instant};

use verba_core::capture::{CaptureFailure, CapturedText, TextCapture};

use crate::{
    MacOsPasteboard, PasteboardRestoreOutcome, PasteboardSnapshot, PasteboardSnapshotError,
};
use accessibility::{AccessibilityStatus, FocusedElementSecurity, SystemAccessibility};
use synthetic_copy::{CopyPoster, CoreGraphicsCopy};

const COPY_TIMEOUT: Duration = Duration::from_millis(500);
const EMPTY_COPY_RETRY_DELAY: Duration = Duration::from_millis(50);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub struct MacOsTextCapture {
    inner: SelectionCapture<MacOsPasteboard, SystemAccessibility, CoreGraphicsCopy, SystemClock>,
}

impl MacOsTextCapture {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: SelectionCapture::new(
                MacOsPasteboard::general(),
                SystemAccessibility,
                CoreGraphicsCopy,
                SystemClock,
                COPY_TIMEOUT,
                POLL_INTERVAL,
            ),
        }
    }
}

impl Default for MacOsTextCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl TextCapture for MacOsTextCapture {
    fn capture(&self) -> Result<CapturedText, CaptureFailure> {
        self.inner.capture()
    }

    fn capture_with_language_detection_context(&self) -> Result<CapturedText, CaptureFailure> {
        self.inner.capture_with_language_detection_context()
    }
}

struct SelectionCapture<P, A, C, K> {
    pasteboard: P,
    accessibility: A,
    copy: C,
    clock: K,
    timeout: Duration,
    poll_interval: Duration,
}

impl<P, A, C, K> SelectionCapture<P, A, C, K> {
    fn new(
        pasteboard: P,
        accessibility: A,
        copy: C,
        clock: K,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Self {
        Self {
            pasteboard,
            accessibility,
            copy,
            clock,
            timeout,
            poll_interval,
        }
    }
}

impl<P, A, C, K> SelectionCapture<P, A, C, K>
where
    P: CapturePasteboard,
    A: AccessibilityStatus,
    C: CopyPoster,
    K: Clock,
{
    fn capture(&self) -> Result<CapturedText, CaptureFailure> {
        self.capture_selection(false)
    }

    fn capture_with_language_detection_context(&self) -> Result<CapturedText, CaptureFailure> {
        self.capture_selection(true)
    }

    fn capture_selection(
        &self,
        include_language_detection_context: bool,
    ) -> Result<CapturedText, CaptureFailure> {
        if !self.accessibility.is_trusted() {
            return Err(CaptureFailure::PermissionDenied);
        }
        match self.accessibility.focused_element_security() {
            FocusedElementSecurity::NotSecure => {}
            FocusedElementSecurity::Secure => return Err(CaptureFailure::SecureField),
            FocusedElementSecurity::Unknown => {
                return Err(CaptureFailure::FieldSecurityUnavailable);
            }
        }

        let snapshot = self
            .pasteboard
            .snapshot()
            .map_err(|_| CaptureFailure::ClipboardUnavailable)?;
        let initial_change_count = snapshot.change_count();
        let started = self.clock.now();

        self.copy.post_copy()?;

        let Some(mut copy_change_count) = self.wait_for_change(initial_change_count, started)
        else {
            // Copy leaves the pasteboard unchanged when the focused source has
            // no selection. Treat that normal user state as actionable guidance
            // instead of presenting it as an infrastructure timeout.
            return Err(CaptureFailure::NoSelection);
        };

        let mut result = self.read_selection();
        if matches!(result, Err(CaptureFailure::NoSelection)) {
            let elapsed = self.clock.elapsed_since(started);
            if elapsed < self.timeout {
                self.clock
                    .sleep(EMPTY_COPY_RETRY_DELAY.min(self.timeout - elapsed));

                if self.pasteboard.change_count() != copy_change_count {
                    return Err(CaptureFailure::Cancelled);
                }

                if self.clock.elapsed_since(started) < self.timeout {
                    match self.copy.post_copy() {
                        Ok(()) => {
                            if let Some(retry_change_count) =
                                self.wait_for_change(copy_change_count, started)
                            {
                                copy_change_count = retry_change_count;
                                result = self.read_selection();
                            }
                        }
                        Err(error) => result = Err(error),
                    }
                }
            }
        }

        if self.pasteboard.change_count() != copy_change_count {
            return Err(CaptureFailure::Cancelled);
        }

        self.pasteboard
            .restore(&snapshot, copy_change_count)
            .map_err(|_| CaptureFailure::ClipboardUnavailable)?;

        if !include_language_detection_context {
            return result;
        }

        result.map(|captured| {
            let is_single_short_word = captured.as_str().split_whitespace().count() == 1
                && captured.as_str().chars().count() <= 128;
            if !is_single_short_word {
                return captured;
            }

            let selected_text = captured.as_str().trim();
            match self.accessibility.selected_text_context() {
                Some(context) if context.contains(selected_text) => {
                    captured.with_language_detection_context(context)
                }
                None => captured,
                Some(_) => captured,
            }
        })
    }

    fn wait_for_change(&self, initial_change_count: i64, started: K::Instant) -> Option<i64> {
        loop {
            let change_count = self.pasteboard.change_count();
            if change_count != initial_change_count {
                return Some(change_count);
            }

            let elapsed = self.clock.elapsed_since(started);
            if elapsed >= self.timeout {
                return None;
            }

            self.clock
                .sleep(self.poll_interval.min(self.timeout - elapsed));
        }
    }

    fn read_selection(&self) -> Result<CapturedText, CaptureFailure> {
        self.pasteboard
            .plain_text()
            .ok_or(CaptureFailure::UnsupportedContent)
            .and_then(CapturedText::new)
    }
}

impl<P, A, C, K> TextCapture for SelectionCapture<P, A, C, K>
where
    P: CapturePasteboard + Send + Sync,
    A: AccessibilityStatus + Send + Sync,
    C: CopyPoster + Send + Sync,
    K: Clock + Send + Sync,
{
    fn capture(&self) -> Result<CapturedText, CaptureFailure> {
        SelectionCapture::capture(self)
    }

    fn capture_with_language_detection_context(&self) -> Result<CapturedText, CaptureFailure> {
        SelectionCapture::capture_with_language_detection_context(self)
    }
}

trait CaptureSnapshot {
    fn change_count(&self) -> i64;
}

impl CaptureSnapshot for PasteboardSnapshot {
    fn change_count(&self) -> i64 {
        self.change_count()
    }
}

trait CapturePasteboard {
    type Snapshot: CaptureSnapshot;

    fn snapshot(&self) -> Result<Self::Snapshot, PasteboardSnapshotError>;
    fn change_count(&self) -> i64;
    fn plain_text(&self) -> Option<String>;
    fn restore(
        &self,
        snapshot: &Self::Snapshot,
        expected_change_count: i64,
    ) -> Result<PasteboardRestoreOutcome, PasteboardSnapshotError>;
}

impl CapturePasteboard for MacOsPasteboard {
    type Snapshot = PasteboardSnapshot;

    fn snapshot(&self) -> Result<Self::Snapshot, PasteboardSnapshotError> {
        self.snapshot()
    }

    fn change_count(&self) -> i64 {
        self.change_count()
    }

    fn plain_text(&self) -> Option<String> {
        self.plain_text()
    }

    fn restore(
        &self,
        snapshot: &Self::Snapshot,
        expected_change_count: i64,
    ) -> Result<PasteboardRestoreOutcome, PasteboardSnapshotError> {
        self.restore(snapshot, expected_change_count)
    }
}

trait Clock {
    type Instant: Copy;

    fn now(&self) -> Self::Instant;
    fn elapsed_since(&self, instant: Self::Instant) -> Duration;
    fn sleep(&self, duration: Duration);
}

struct SystemClock;

impl Clock for SystemClock {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        Instant::now()
    }

    fn elapsed_since(&self, instant: Self::Instant) -> Duration {
        instant.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

#[cfg(test)]
mod tests;
