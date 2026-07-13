use std::sync::{Arc, Mutex};

use super::*;

#[test]
fn public_capture_is_send_sync_and_constructible_without_side_effects() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<MacOsTextCapture>();
    let _capture = MacOsTextCapture::new();
}

#[test]
fn captures_text_and_restores_the_original_clipboard() {
    let fixture = Fixture::new(Some("original"), Some("selected"));

    let captured = fixture.capture.capture().unwrap();

    assert_eq!(captured.as_str(), "selected");
    assert_eq!(fixture.clipboard_text(), Some("original".to_owned()));
    assert_eq!(fixture.restore_calls(), 1);
}

#[test]
fn restores_after_empty_selected_text() {
    let fixture = Fixture::new(Some("original"), Some("  \n"));

    assert_eq!(fixture.capture.capture(), Err(CaptureFailure::NoSelection));
    assert_eq!(fixture.clipboard_text(), Some("original".to_owned()));
    assert_eq!(fixture.restore_calls(), 1);
}

#[test]
fn restores_after_unsupported_content() {
    let fixture = Fixture::new(Some("original"), None);

    assert_eq!(
        fixture.capture.capture(),
        Err(CaptureFailure::UnsupportedContent)
    );
    assert_eq!(fixture.clipboard_text(), Some("original".to_owned()));
    assert_eq!(fixture.restore_calls(), 1);
}

#[test]
fn reports_permission_denial_without_touching_the_clipboard() {
    let mut fixture = Fixture::new(Some("original"), Some("selected"));
    fixture.capture.accessibility.trusted = false;

    assert_eq!(
        fixture.capture.capture(),
        Err(CaptureFailure::PermissionDenied)
    );
    assert_eq!(fixture.snapshot_calls(), 0);
    assert_eq!(fixture.post_calls(), 0);
}

#[test]
fn reports_a_secure_field_without_posting_copy() {
    let mut fixture = Fixture::new(Some("original"), Some("selected"));
    fixture.capture.accessibility.secure = true;

    assert_eq!(fixture.capture.capture(), Err(CaptureFailure::SecureField));
    assert_eq!(fixture.snapshot_calls(), 0);
    assert_eq!(fixture.post_calls(), 0);
}

#[test]
fn times_out_at_the_configured_deadline() {
    let fixture = Fixture::without_copy_change(Some("original"));

    assert_eq!(fixture.capture.capture(), Err(CaptureFailure::TimedOut));
    assert_eq!(fixture.elapsed(), COPY_TIMEOUT);
    assert_eq!(fixture.clipboard_text(), Some("original".to_owned()));
    assert_eq!(fixture.restore_calls(), 0);
}

#[test]
fn does_not_overwrite_a_later_clipboard_change() {
    let fixture = Fixture::new(Some("original"), Some("selected"));
    fixture.state.lock().unwrap().replace_before_restore = Some(Some("external".to_owned()));

    let captured = fixture.capture.capture().unwrap();

    assert_eq!(captured.as_str(), "selected");
    assert_eq!(fixture.clipboard_text(), Some("external".to_owned()));
}

#[test]
fn cancels_if_the_clipboard_changes_while_text_is_read() {
    let fixture = Fixture::new(Some("original"), Some("selected"));
    fixture.state.lock().unwrap().replace_after_read = Some(Some("external".to_owned()));

    assert_eq!(fixture.capture.capture(), Err(CaptureFailure::Cancelled));
    assert_eq!(fixture.clipboard_text(), Some("external".to_owned()));
    assert_eq!(fixture.restore_calls(), 0);
}

#[test]
fn reports_clipboard_failure_when_restoration_fails() {
    let fixture = Fixture::new(Some("original"), Some("selected"));
    fixture.state.lock().unwrap().restore_fails = true;

    assert_eq!(
        fixture.capture.capture(),
        Err(CaptureFailure::ClipboardUnavailable)
    );
}

type TestCapture = SelectionCapture<FakePasteboard, FakeAccessibility, FakeCopy, FakeClock>;

struct Fixture {
    capture: TestCapture,
    state: Arc<Mutex<FakePasteboardState>>,
    clock: Arc<Mutex<Duration>>,
}

impl Fixture {
    fn new(original: Option<&str>, copied: Option<&str>) -> Self {
        Self::build(original, Some(copied.map(str::to_owned)))
    }

    fn without_copy_change(original: Option<&str>) -> Self {
        Self::build(original, None)
    }

    fn build(original: Option<&str>, copied: Option<Option<String>>) -> Self {
        let state = Arc::new(Mutex::new(FakePasteboardState {
            text: original.map(str::to_owned),
            change_count: 1,
            snapshot_calls: 0,
            restore_calls: 0,
            replace_after_read: None,
            replace_before_restore: None,
            restore_fails: false,
        }));
        let clock = Arc::new(Mutex::new(Duration::ZERO));
        let pasteboard = FakePasteboard {
            state: Arc::clone(&state),
        };
        let copy = FakeCopy {
            state: Arc::clone(&state),
            copied,
            post_calls: Arc::new(Mutex::new(0)),
        };
        let capture = SelectionCapture::new(
            pasteboard,
            FakeAccessibility {
                trusted: true,
                secure: false,
            },
            copy,
            FakeClock {
                elapsed: Arc::clone(&clock),
            },
            COPY_TIMEOUT,
            POLL_INTERVAL,
        );

        Self {
            capture,
            state,
            clock,
        }
    }

    fn clipboard_text(&self) -> Option<String> {
        self.state.lock().unwrap().text.clone()
    }

    fn snapshot_calls(&self) -> usize {
        self.state.lock().unwrap().snapshot_calls
    }

    fn restore_calls(&self) -> usize {
        self.state.lock().unwrap().restore_calls
    }

    fn post_calls(&self) -> usize {
        *self.capture.copy.post_calls.lock().unwrap()
    }

    fn elapsed(&self) -> Duration {
        *self.clock.lock().unwrap()
    }
}

struct FakeSnapshot {
    text: Option<String>,
    change_count: i64,
}

impl CaptureSnapshot for FakeSnapshot {
    fn change_count(&self) -> i64 {
        self.change_count
    }
}

struct FakePasteboard {
    state: Arc<Mutex<FakePasteboardState>>,
}

struct FakePasteboardState {
    text: Option<String>,
    change_count: i64,
    snapshot_calls: usize,
    restore_calls: usize,
    replace_after_read: Option<Option<String>>,
    replace_before_restore: Option<Option<String>>,
    restore_fails: bool,
}

impl CapturePasteboard for FakePasteboard {
    type Snapshot = FakeSnapshot;

    fn snapshot(&self) -> Result<Self::Snapshot, PasteboardSnapshotError> {
        let mut state = self.state.lock().unwrap();
        state.snapshot_calls += 1;
        Ok(FakeSnapshot {
            text: state.text.clone(),
            change_count: state.change_count,
        })
    }

    fn change_count(&self) -> i64 {
        self.state.lock().unwrap().change_count
    }

    fn plain_text(&self) -> Option<String> {
        let mut state = self.state.lock().unwrap();
        let text = state.text.clone();
        if let Some(external) = state.replace_after_read.take() {
            state.text = external;
            state.change_count += 1;
        }
        text
    }

    fn restore(
        &self,
        snapshot: &Self::Snapshot,
        expected_change_count: i64,
    ) -> Result<PasteboardRestoreOutcome, PasteboardSnapshotError> {
        let mut state = self.state.lock().unwrap();
        state.restore_calls += 1;

        if state.restore_fails {
            return Err(PasteboardSnapshotError::WriteFailed);
        }
        if let Some(external) = state.replace_before_restore.take() {
            state.text = external;
            state.change_count += 1;
        }
        if state.change_count != expected_change_count {
            return Ok(PasteboardRestoreOutcome::SkippedDueToConflict);
        }

        state.text.clone_from(&snapshot.text);
        state.change_count += 1;
        Ok(PasteboardRestoreOutcome::Restored {
            change_count: state.change_count,
        })
    }
}

struct FakeAccessibility {
    trusted: bool,
    secure: bool,
}

impl AccessibilityStatus for FakeAccessibility {
    fn is_trusted(&self) -> bool {
        self.trusted
    }

    fn focused_element_is_secure(&self) -> bool {
        self.secure
    }
}

struct FakeCopy {
    state: Arc<Mutex<FakePasteboardState>>,
    copied: Option<Option<String>>,
    post_calls: Arc<Mutex<usize>>,
}

impl CopyPoster for FakeCopy {
    fn post_copy(&self) -> Result<(), CaptureFailure> {
        *self.post_calls.lock().unwrap() += 1;
        if let Some(copied) = &self.copied {
            let mut state = self.state.lock().unwrap();
            state.text.clone_from(copied);
            state.change_count += 1;
        }
        Ok(())
    }
}

struct FakeClock {
    elapsed: Arc<Mutex<Duration>>,
}

impl Clock for FakeClock {
    type Instant = Duration;

    fn now(&self) -> Self::Instant {
        *self.elapsed.lock().unwrap()
    }

    fn elapsed_since(&self, instant: Self::Instant) -> Duration {
        *self.elapsed.lock().unwrap() - instant
    }

    fn sleep(&self, duration: Duration) {
        *self.elapsed.lock().unwrap() += duration;
    }
}
