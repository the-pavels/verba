use std::{ffi::c_void, ptr};

use verba_core::capture::CaptureFailure;

const COPY_KEY_CODE: u16 = 0x08;
const COMMAND_FLAG: u64 = 0x0010_0000;
const HID_EVENT_TAP: u32 = 0;

pub(super) trait CopyPoster {
    fn post_copy(&self) -> Result<(), CaptureFailure>;
}

pub(super) struct CoreGraphicsCopy;

impl CopyPoster for CoreGraphicsCopy {
    fn post_copy(&self) -> Result<(), CaptureFailure> {
        let key_down = OwnedCgEvent::new(unsafe {
            cg_event_create_keyboard_event(ptr::null(), COPY_KEY_CODE, true)
        })
        .ok_or(CaptureFailure::ClipboardUnavailable)?;
        let key_up = OwnedCgEvent::new(unsafe {
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

struct OwnedCgEvent(*const c_void);

impl OwnedCgEvent {
    fn new(value: *const c_void) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn as_ptr(&self) -> *const c_void {
        self.0
    }
}

impl Drop for OwnedCgEvent {
    fn drop(&mut self) {
        unsafe { cf_release(self.0) };
    }
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
    #[link_name = "CFRelease"]
    fn cf_release(value: *const c_void);
}
