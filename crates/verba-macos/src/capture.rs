use std::{
    ffi::{CStr, c_void},
    ptr,
    time::{Duration, Instant},
};

use verba_core::capture::{CaptureFailure, CapturedText, TextCapture};

use crate::{
    MacOsPasteboard, PasteboardRestoreOutcome, PasteboardSnapshot, PasteboardSnapshotError,
};

const COPY_KEY_CODE: u16 = 0x08;
const COMMAND_FLAG: u64 = 0x0010_0000;
const HID_EVENT_TAP: u32 = 0;
const COPY_TIMEOUT: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const AX_SUCCESS: i32 = 0;

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
        if !self.accessibility.is_trusted() {
            return Err(CaptureFailure::PermissionDenied);
        }
        if self.accessibility.focused_element_is_secure() {
            return Err(CaptureFailure::SecureField);
        }

        let snapshot = self
            .pasteboard
            .snapshot()
            .map_err(|_| CaptureFailure::ClipboardUnavailable)?;
        let initial_change_count = snapshot.change_count();

        self.copy.post_copy()?;

        let Some(copy_change_count) = self.wait_for_change(initial_change_count) else {
            return Err(CaptureFailure::TimedOut);
        };

        let result = self
            .pasteboard
            .plain_text()
            .ok_or(CaptureFailure::UnsupportedContent)
            .and_then(CapturedText::new);

        if self.pasteboard.change_count() != copy_change_count {
            return Err(CaptureFailure::Cancelled);
        }

        self.pasteboard
            .restore(&snapshot, copy_change_count)
            .map_err(|_| CaptureFailure::ClipboardUnavailable)?;

        result
    }

    fn wait_for_change(&self, initial_change_count: i64) -> Option<i64> {
        let started = self.clock.now();

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

trait AccessibilityStatus {
    fn is_trusted(&self) -> bool;
    fn focused_element_is_secure(&self) -> bool;
}

struct SystemAccessibility;

impl AccessibilityStatus for SystemAccessibility {
    fn is_trusted(&self) -> bool {
        unsafe { ax_is_process_trusted() }
    }

    fn focused_element_is_secure(&self) -> bool {
        unsafe { focused_element_is_secure() }
    }
}

trait CopyPoster {
    fn post_copy(&self) -> Result<(), CaptureFailure>;
}

struct CoreGraphicsCopy;

impl CopyPoster for CoreGraphicsCopy {
    fn post_copy(&self) -> Result<(), CaptureFailure> {
        let key_down = OwnedCf::new(unsafe {
            cg_event_create_keyboard_event(ptr::null(), COPY_KEY_CODE, true)
        })
        .ok_or(CaptureFailure::ClipboardUnavailable)?;
        let key_up = OwnedCf::new(unsafe {
            cg_event_create_keyboard_event(ptr::null(), COPY_KEY_CODE, false)
        })
        .ok_or(CaptureFailure::ClipboardUnavailable)?;

        unsafe {
            cg_event_set_flags(key_down.as_ptr(), COMMAND_FLAG);
            cg_event_set_flags(key_up.as_ptr(), COMMAND_FLAG);
            cg_event_post(HID_EVENT_TAP, key_down.as_ptr());
            cg_event_post(HID_EVENT_TAP, key_up.as_ptr());
        }

        Ok(())
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

struct OwnedCf(*const c_void);

impl OwnedCf {
    fn new(value: *const c_void) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn as_ptr(&self) -> *const c_void {
        self.0
    }
}

impl Drop for OwnedCf {
    fn drop(&mut self) {
        unsafe { cf_release(self.0) };
    }
}

unsafe fn focused_element_is_secure() -> bool {
    let Some(system_wide) = OwnedCf::new(unsafe { ax_ui_element_create_system_wide() }) else {
        return false;
    };
    let Some(focused_attribute) = cf_string(c"AXFocusedUIElement") else {
        return false;
    };
    let Some(focused_element) = copy_ax_attribute(&system_wide, &focused_attribute) else {
        return false;
    };
    let Some(subrole_attribute) = cf_string(c"AXSubrole") else {
        return false;
    };
    let Some(subrole) = copy_ax_attribute(&focused_element, &subrole_attribute) else {
        return false;
    };
    let Some(secure_subrole) = cf_string(c"AXSecureTextField") else {
        return false;
    };

    unsafe { cf_equal(subrole.as_ptr(), secure_subrole.as_ptr()) }
}

fn cf_string(value: &CStr) -> Option<OwnedCf> {
    OwnedCf::new(unsafe {
        cf_string_create_with_c_string(ptr::null(), value.as_ptr(), CF_STRING_ENCODING_UTF8)
    })
}

fn copy_ax_attribute(element: &OwnedCf, attribute: &OwnedCf) -> Option<OwnedCf> {
    let mut value = ptr::null();
    let result = unsafe {
        ax_ui_element_copy_attribute_value(element.as_ptr(), attribute.as_ptr(), &mut value)
    };

    (result == AX_SUCCESS)
        .then(|| OwnedCf::new(value))
        .flatten()
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    #[link_name = "AXIsProcessTrusted"]
    fn ax_is_process_trusted() -> bool;

    #[link_name = "AXUIElementCreateSystemWide"]
    fn ax_ui_element_create_system_wide() -> *const c_void;

    #[link_name = "AXUIElementCopyAttributeValue"]
    fn ax_ui_element_copy_attribute_value(
        element: *const c_void,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> i32;
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CGEventCreateKeyboardEvent"]
    fn cg_event_create_keyboard_event(
        source: *const c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> *const c_void;

    #[link_name = "CGEventSetFlags"]
    fn cg_event_set_flags(event: *const c_void, flags: u64);

    #[link_name = "CGEventPost"]
    fn cg_event_post(tap: u32, event: *const c_void);
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    #[link_name = "CFStringCreateWithCString"]
    fn cf_string_create_with_c_string(
        allocator: *const c_void,
        value: *const std::ffi::c_char,
        encoding: u32,
    ) -> *const c_void;

    #[link_name = "CFEqual"]
    fn cf_equal(first: *const c_void, second: *const c_void) -> bool;

    #[link_name = "CFRelease"]
    fn cf_release(value: *const c_void);
}

#[cfg(test)]
mod tests {
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
}
