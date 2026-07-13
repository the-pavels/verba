use std::{
    ffi::c_void,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

use super::CallbackState;

const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const EVENT_NOT_HANDLED: i32 = -9874;
const EVENT_HOT_KEY_EXISTS: i32 = -9878;
const HOT_KEY_EXCLUSIVE: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SystemError {
    HotKeyExists,
    Failed,
}

pub(super) trait HotKeySystem {
    type HotKeyRef: Copy;
    type EventHandlerRef: Copy;

    fn install_event_handler(
        &mut self,
        user_data: *mut c_void,
    ) -> Result<Self::EventHandlerRef, SystemError>;
    fn remove_event_handler(&mut self, reference: Self::EventHandlerRef)
    -> Result<(), SystemError>;
    fn register_hot_key(
        &mut self,
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
    ) -> Result<Self::HotKeyRef, SystemError>;
    fn unregister_hot_key(&mut self, reference: Self::HotKeyRef) -> Result<(), SystemError>;
}

pub(super) struct CarbonSystem;

impl HotKeySystem for CarbonSystem {
    type HotKeyRef = NonZeroUsize;
    type EventHandlerRef = NonZeroUsize;

    fn install_event_handler(
        &mut self,
        user_data: *mut c_void,
    ) -> Result<Self::EventHandlerRef, SystemError> {
        let event_type = EventTypeSpec {
            event_class: EVENT_CLASS_KEYBOARD,
            event_kind: EVENT_HOT_KEY_PRESSED,
        };
        let mut reference = ptr::null_mut();
        let status = unsafe {
            install_event_handler(
                get_application_event_target(),
                hot_key_event_handler,
                1,
                &event_type,
                user_data,
                &mut reference,
            )
        };

        status_result(status)?;
        NonZeroUsize::new(reference as usize).ok_or(SystemError::Failed)
    }

    fn remove_event_handler(
        &mut self,
        reference: Self::EventHandlerRef,
    ) -> Result<(), SystemError> {
        status_result(unsafe { remove_event_handler(reference.get() as *mut c_void) })
    }

    fn register_hot_key(
        &mut self,
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
    ) -> Result<Self::HotKeyRef, SystemError> {
        let mut reference = ptr::null_mut();
        let status = unsafe {
            register_event_hot_key(
                key_code,
                modifiers,
                id,
                get_application_event_target(),
                HOT_KEY_EXCLUSIVE,
                &mut reference,
            )
        };

        status_result(status)?;
        NonZeroUsize::new(reference as usize).ok_or(SystemError::Failed)
    }

    fn unregister_hot_key(&mut self, reference: Self::HotKeyRef) -> Result<(), SystemError> {
        status_result(unsafe { unregister_event_hot_key(reference.get() as *mut c_void) })
    }
}

fn status_result(status: i32) -> Result<(), SystemError> {
    match status {
        0 => Ok(()),
        EVENT_HOT_KEY_EXISTS => Err(SystemError::HotKeyExists),
        _ => Err(SystemError::Failed),
    }
}

unsafe extern "C" fn hot_key_event_handler(
    _handler_call: *mut c_void,
    event: *mut c_void,
    user_data: *mut c_void,
) -> i32 {
    if event.is_null() || user_data.is_null() {
        return EVENT_NOT_HANDLED;
    }

    catch_unwind(AssertUnwindSafe(|| {
        let mut id = EventHotKeyId {
            signature: 0,
            id: 0,
        };
        let status = unsafe {
            get_event_parameter(
                event,
                EVENT_PARAM_DIRECT_OBJECT,
                TYPE_EVENT_HOT_KEY_ID,
                ptr::null_mut(),
                size_of::<EventHotKeyId>(),
                ptr::null_mut(),
                (&mut id as *mut EventHotKeyId).cast::<c_void>(),
            )
        };
        if status != 0 {
            return EVENT_NOT_HANDLED;
        }

        let callback = unsafe { &*(user_data.cast::<CallbackState>()) };
        if callback.dispatch(id) {
            0
        } else {
            EVENT_NOT_HANDLED
        }
    }))
    .unwrap_or(EVENT_NOT_HANDLED)
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct EventHotKeyId {
    pub(super) signature: u32,
    pub(super) id: u32,
}

#[repr(C)]
struct EventTypeSpec {
    event_class: u32,
    event_kind: u32,
}

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    #[link_name = "GetApplicationEventTarget"]
    fn get_application_event_target() -> *mut c_void;

    #[link_name = "InstallEventHandler"]
    fn install_event_handler(
        target: *mut c_void,
        handler: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
        event_type_count: usize,
        event_types: *const EventTypeSpec,
        user_data: *mut c_void,
        reference: *mut *mut c_void,
    ) -> i32;

    #[link_name = "RemoveEventHandler"]
    fn remove_event_handler(reference: *mut c_void) -> i32;

    #[link_name = "RegisterEventHotKey"]
    fn register_event_hot_key(
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
        target: *mut c_void,
        options: u32,
        reference: *mut *mut c_void,
    ) -> i32;

    #[link_name = "UnregisterEventHotKey"]
    fn unregister_event_hot_key(reference: *mut c_void) -> i32;

    #[link_name = "GetEventParameter"]
    fn get_event_parameter(
        event: *mut c_void,
        name: u32,
        desired_type: u32,
        actual_type: *mut u32,
        buffer_size: usize,
        actual_size: *mut usize,
        data: *mut c_void,
    ) -> i32;
}
