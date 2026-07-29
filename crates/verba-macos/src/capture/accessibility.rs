use std::{
    ffi::{CStr, c_void},
    ptr,
};

use objc2_app_kit::NSWorkspace;

const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const AX_SUCCESS: i32 = 0;
const AX_ERROR_NO_VALUE: i32 = -25_212;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FocusedElementSecurity {
    NotSecure,
    Secure,
    Unknown,
}

enum AttributeValue<T> {
    Value(T),
    NoValue,
    Failed,
}

pub(super) trait AccessibilityStatus {
    fn is_trusted(&self) -> bool;
    fn focused_element_security(&self) -> FocusedElementSecurity;
}

pub(super) struct SystemAccessibility;

impl AccessibilityStatus for SystemAccessibility {
    fn is_trusted(&self) -> bool {
        unsafe { ax_is_process_trusted() }
    }

    fn focused_element_security(&self) -> FocusedElementSecurity {
        focused_element_security(&SystemAccessibilityReader)
    }
}

trait AccessibilityReader {
    type Value;

    fn system_wide_element(&self) -> Option<Self::Value>;
    fn frontmost_application(&self) -> Option<Self::Value>;
    fn string(&self, value: &CStr) -> Option<Self::Value>;
    fn attribute(
        &self,
        element: &Self::Value,
        attribute: &Self::Value,
    ) -> AttributeValue<Self::Value>;
    fn is_string(&self, value: &Self::Value) -> bool;
    fn equal(&self, first: &Self::Value, second: &Self::Value) -> bool;
}

fn focused_element_security(reader: &impl AccessibilityReader) -> FocusedElementSecurity {
    let Some(focused_attribute) = reader.string(c"AXFocusedUIElement") else {
        return FocusedElementSecurity::Unknown;
    };
    let system_focused_element = reader
        .system_wide_element()
        .map(|system_wide| reader.attribute(&system_wide, &focused_attribute))
        .unwrap_or(AttributeValue::Failed);
    let focused_element = match system_focused_element {
        AttributeValue::Value(focused_element) => focused_element,
        AttributeValue::NoValue | AttributeValue::Failed => {
            let frontmost_focused_element = reader
                .frontmost_application()
                .map(|application| reader.attribute(&application, &focused_attribute))
                .unwrap_or(AttributeValue::Failed);
            match frontmost_focused_element {
                AttributeValue::Value(focused_element) => focused_element,
                AttributeValue::NoValue => {
                    // An explicit no-value response from the frontmost app means
                    // there is no source field. Other AX errors remain fail-closed.
                    return FocusedElementSecurity::NotSecure;
                }
                AttributeValue::Failed => return FocusedElementSecurity::Unknown,
            }
        }
    };
    let Some(role_attribute) = reader.string(c"AXRole") else {
        return FocusedElementSecurity::Unknown;
    };
    let AttributeValue::Value(role) = reader.attribute(&focused_element, &role_attribute) else {
        return FocusedElementSecurity::Unknown;
    };
    if !reader.is_string(&role) {
        return FocusedElementSecurity::Unknown;
    }
    let Some(text_field_role) = reader.string(c"AXTextField") else {
        return FocusedElementSecurity::Unknown;
    };
    let Some(subrole_attribute) = reader.string(c"AXSubrole") else {
        return FocusedElementSecurity::Unknown;
    };
    let Some(secure_subrole) = reader.string(c"AXSecureTextField") else {
        return FocusedElementSecurity::Unknown;
    };
    let is_text_field = reader.equal(&role, &text_field_role);

    match reader.attribute(&focused_element, &subrole_attribute) {
        AttributeValue::Value(subrole) if reader.is_string(&subrole) => {
            if reader.equal(&subrole, &secure_subrole) {
                FocusedElementSecurity::Secure
            } else {
                FocusedElementSecurity::NotSecure
            }
        }
        AttributeValue::Value(_) => FocusedElementSecurity::Unknown,
        AttributeValue::NoValue | AttributeValue::Failed if is_text_field => {
            FocusedElementSecurity::Unknown
        }
        AttributeValue::NoValue | AttributeValue::Failed => FocusedElementSecurity::NotSecure,
    }
}

struct SystemAccessibilityReader;

impl AccessibilityReader for SystemAccessibilityReader {
    type Value = OwnedAxValue;

    fn system_wide_element(&self) -> Option<Self::Value> {
        OwnedAxValue::new(unsafe { ax_ui_element_create_system_wide() })
    }

    fn frontmost_application(&self) -> Option<Self::Value> {
        let process_identifier = NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .processIdentifier();
        OwnedAxValue::new(unsafe { ax_ui_element_create_application(process_identifier) })
    }

    fn string(&self, value: &CStr) -> Option<Self::Value> {
        OwnedAxValue::new(unsafe {
            cf_string_create_with_c_string(ptr::null(), value.as_ptr(), CF_STRING_ENCODING_UTF8)
        })
    }

    fn attribute(
        &self,
        element: &Self::Value,
        attribute: &Self::Value,
    ) -> AttributeValue<Self::Value> {
        let mut value = ptr::null();
        let result = unsafe {
            ax_ui_element_copy_attribute_value(element.as_ptr(), attribute.as_ptr(), &mut value)
        };

        match result {
            AX_SUCCESS => OwnedAxValue::new(value)
                .map(AttributeValue::Value)
                .unwrap_or(AttributeValue::Failed),
            AX_ERROR_NO_VALUE => AttributeValue::NoValue,
            _ => AttributeValue::Failed,
        }
    }

    fn is_string(&self, value: &Self::Value) -> bool {
        unsafe { cf_get_type_id(value.as_ptr()) == cf_string_get_type_id() }
    }

    fn equal(&self, first: &Self::Value, second: &Self::Value) -> bool {
        unsafe { cf_equal(first.as_ptr(), second.as_ptr()) }
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

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    #[link_name = "AXIsProcessTrusted"]
    fn ax_is_process_trusted() -> bool;

    #[link_name = "AXUIElementCreateSystemWide"]
    fn ax_ui_element_create_system_wide() -> *const c_void;

    #[link_name = "AXUIElementCreateApplication"]
    fn ax_ui_element_create_application(process_identifier: i32) -> *const c_void;

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

    #[link_name = "CFGetTypeID"]
    fn cf_get_type_id(value: *const c_void) -> usize;

    #[link_name = "CFStringGetTypeID"]
    fn cf_string_get_type_id() -> usize;

    #[link_name = "CFEqual"]
    fn cf_equal(first: *const c_void, second: *const c_void) -> bool;

    #[link_name = "CFRelease"]
    fn cf_release(value: *const c_void);
}

#[cfg(test)]
mod tests {
    use super::{
        AccessibilityReader, AttributeValue, FocusedElementSecurity, focused_element_security,
    };
    use std::ffi::CStr;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Value {
        SystemWide,
        FrontmostApplication,
        FocusedAttribute,
        FocusedElement,
        RoleAttribute,
        TextFieldRole,
        OtherRole,
        SubroleAttribute,
        Subrole,
        SecureSubrole,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FailurePoint {
        SystemWide,
        FrontmostApplication,
        FrontmostFocusedElement,
        FocusedAttribute,
        RoleAttribute,
        Role,
        RoleType,
        TextFieldRole,
        SubroleAttribute,
        Subrole,
        SubroleType,
        SecureSubrole,
    }

    struct FakeReader {
        failure: Option<FailurePoint>,
        text_field: bool,
        secure: bool,
        system_focus_available: bool,
        frontmost_focus_available: bool,
    }

    impl FakeReader {
        fn new(failure: Option<FailurePoint>, text_field: bool, secure: bool) -> Self {
            Self {
                failure,
                text_field,
                secure,
                system_focus_available: true,
                frontmost_focus_available: true,
            }
        }

        fn without_system_focus(mut self) -> Self {
            self.system_focus_available = false;
            self
        }

        fn without_any_focus(mut self) -> Self {
            self.system_focus_available = false;
            self.frontmost_focus_available = false;
            self
        }
    }

    impl AccessibilityReader for FakeReader {
        type Value = Value;

        fn system_wide_element(&self) -> Option<Self::Value> {
            (self.failure != Some(FailurePoint::SystemWide)).then_some(Value::SystemWide)
        }

        fn frontmost_application(&self) -> Option<Self::Value> {
            (self.failure != Some(FailurePoint::FrontmostApplication))
                .then_some(Value::FrontmostApplication)
        }

        fn string(&self, value: &CStr) -> Option<Self::Value> {
            match value.to_bytes() {
                b"AXFocusedUIElement" => (self.failure != Some(FailurePoint::FocusedAttribute))
                    .then_some(Value::FocusedAttribute),
                b"AXRole" => (self.failure != Some(FailurePoint::RoleAttribute))
                    .then_some(Value::RoleAttribute),
                b"AXTextField" => (self.failure != Some(FailurePoint::TextFieldRole))
                    .then_some(Value::TextFieldRole),
                b"AXSubrole" => (self.failure != Some(FailurePoint::SubroleAttribute))
                    .then_some(Value::SubroleAttribute),
                b"AXSecureTextField" => (self.failure != Some(FailurePoint::SecureSubrole))
                    .then_some(Value::SecureSubrole),
                _ => None,
            }
        }

        fn attribute(
            &self,
            element: &Self::Value,
            attribute: &Self::Value,
        ) -> AttributeValue<Self::Value> {
            match (element, attribute) {
                (Value::SystemWide, Value::FocusedAttribute) if self.system_focus_available => {
                    AttributeValue::Value(Value::FocusedElement)
                }
                (Value::SystemWide, Value::FocusedAttribute) => AttributeValue::NoValue,
                (Value::FrontmostApplication, Value::FocusedAttribute)
                    if self.failure == Some(FailurePoint::FrontmostFocusedElement) =>
                {
                    AttributeValue::Failed
                }
                (Value::FrontmostApplication, Value::FocusedAttribute)
                    if self.frontmost_focus_available =>
                {
                    AttributeValue::Value(Value::FocusedElement)
                }
                (Value::FrontmostApplication, Value::FocusedAttribute) => AttributeValue::NoValue,
                (Value::FocusedElement, Value::RoleAttribute) => {
                    if self.failure == Some(FailurePoint::Role) {
                        AttributeValue::Failed
                    } else {
                        AttributeValue::Value(if self.text_field {
                            Value::TextFieldRole
                        } else {
                            Value::OtherRole
                        })
                    }
                }
                (Value::FocusedElement, Value::SubroleAttribute) => {
                    if self.failure == Some(FailurePoint::Subrole) {
                        AttributeValue::NoValue
                    } else {
                        AttributeValue::Value(Value::Subrole)
                    }
                }
                _ => AttributeValue::Failed,
            }
        }

        fn is_string(&self, value: &Self::Value) -> bool {
            match value {
                Value::TextFieldRole | Value::OtherRole => {
                    self.failure != Some(FailurePoint::RoleType)
                }
                Value::Subrole => self.failure != Some(FailurePoint::SubroleType),
                _ => false,
            }
        }

        fn equal(&self, first: &Self::Value, second: &Self::Value) -> bool {
            match (first, second) {
                (Value::Subrole, Value::SecureSubrole) => self.secure,
                (Value::TextFieldRole, Value::TextFieldRole) => true,
                _ => false,
            }
        }
    }

    #[test]
    fn classifies_secure_and_non_secure_elements() {
        assert_eq!(
            focused_element_security(&FakeReader::new(None, true, true)),
            FocusedElementSecurity::Secure
        );
        assert_eq!(
            focused_element_security(&FakeReader::new(None, true, false)),
            FocusedElementSecurity::NotSecure
        );
    }

    #[test]
    fn falls_back_to_frontmost_application_when_system_focus_is_missing() {
        assert_eq!(
            focused_element_security(&FakeReader::new(None, true, true).without_system_focus()),
            FocusedElementSecurity::Secure
        );
        assert_eq!(
            focused_element_security(&FakeReader::new(None, true, false).without_system_focus()),
            FocusedElementSecurity::NotSecure
        );
    }

    #[test]
    fn falls_back_to_frontmost_application_when_system_wide_element_is_missing() {
        assert_eq!(
            focused_element_security(&FakeReader::new(Some(FailurePoint::SystemWide), true, true)),
            FocusedElementSecurity::Secure
        );
    }

    #[test]
    fn missing_focused_element_from_both_sources_is_unknown() {
        for failure in [
            FailurePoint::FrontmostApplication,
            FailurePoint::FrontmostFocusedElement,
        ] {
            assert_eq!(
                focused_element_security(
                    &FakeReader::new(Some(failure), false, false).without_system_focus()
                ),
                FocusedElementSecurity::Unknown,
                "{failure:?} should fail closed"
            );
        }
    }

    #[test]
    fn no_focused_element_in_frontmost_application_means_no_source_field() {
        assert_eq!(
            focused_element_security(&FakeReader::new(None, false, false).without_any_focus()),
            FocusedElementSecurity::NotSecure
        );
    }

    #[test]
    fn missing_subrole_is_allowed_for_non_text_fields() {
        assert_eq!(
            focused_element_security(&FakeReader::new(Some(FailurePoint::Subrole), false, false)),
            FocusedElementSecurity::NotSecure
        );
    }

    #[test]
    fn missing_subrole_fails_closed_for_text_fields() {
        assert_eq!(
            focused_element_security(&FakeReader::new(Some(FailurePoint::Subrole), true, false)),
            FocusedElementSecurity::Unknown
        );
    }

    #[test]
    fn mandatory_query_failures_are_unknown() {
        for failure in [
            FailurePoint::FocusedAttribute,
            FailurePoint::RoleAttribute,
            FailurePoint::Role,
            FailurePoint::RoleType,
            FailurePoint::TextFieldRole,
            FailurePoint::SubroleAttribute,
            FailurePoint::SubroleType,
            FailurePoint::SecureSubrole,
        ] {
            assert_eq!(
                focused_element_security(&FakeReader::new(Some(failure), false, false)),
                FocusedElementSecurity::Unknown,
                "{failure:?} should fail closed"
            );
        }
    }
}
