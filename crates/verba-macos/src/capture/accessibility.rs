use std::{
    ffi::{CStr, c_void},
    ptr,
};

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const AX_SUCCESS: i32 = 0;

pub(super) trait AccessibilityStatus {
    fn is_trusted(&self) -> bool;
    fn focused_element_is_secure(&self) -> bool;
}

pub(super) struct SystemAccessibility;

impl AccessibilityStatus for SystemAccessibility {
    fn is_trusted(&self) -> bool {
        unsafe { ax_is_process_trusted() }
    }

    fn focused_element_is_secure(&self) -> bool {
        unsafe { focused_element_is_secure() }
    }
}

struct OwnedAxValue(*const c_void);

impl OwnedAxValue {
    fn new(value: *const c_void) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn as_ptr(&self) -> *const c_void {
        self.0
    }
}

impl Drop for OwnedAxValue {
    fn drop(&mut self) {
        unsafe { cf_release(self.0) };
    }
}

unsafe fn focused_element_is_secure() -> bool {
    let Some(system_wide) = OwnedAxValue::new(unsafe { ax_ui_element_create_system_wide() }) else {
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

fn cf_string(value: &CStr) -> Option<OwnedAxValue> {
    OwnedAxValue::new(unsafe {
        cf_string_create_with_c_string(ptr::null(), value.as_ptr(), CF_STRING_ENCODING_UTF8)
    })
}

fn copy_ax_attribute(element: &OwnedAxValue, attribute: &OwnedAxValue) -> Option<OwnedAxValue> {
    let mut value = ptr::null();
    let result = unsafe {
        ax_ui_element_copy_attribute_value(element.as_ptr(), attribute.as_ptr(), &mut value)
    };

    (result == AX_SUCCESS)
        .then(|| OwnedAxValue::new(value))
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
