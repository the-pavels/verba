use std::{
    ffi::c_void,
    num::NonZeroUsize,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{Arc, Mutex},
};

use verba_core::{
    presentation::TextAction,
    shortcut::{
        NamedShortcutKey, ShortcutConfiguration, ShortcutEventHandler, ShortcutKey,
        ShortcutModifiers, ShortcutRegistry, ShortcutRegistryError,
    },
};

const HOT_KEY_SIGNATURE: u32 = u32::from_be_bytes(*b"Vrba");
const TRANSLATE_HOT_KEY_ID: u32 = 1;
const PROOFREAD_HOT_KEY_ID: u32 = 2;

const EVENT_CLASS_KEYBOARD: u32 = u32::from_be_bytes(*b"keyb");
const EVENT_HOT_KEY_PRESSED: u32 = 5;
const EVENT_PARAM_DIRECT_OBJECT: u32 = u32::from_be_bytes(*b"----");
const TYPE_EVENT_HOT_KEY_ID: u32 = u32::from_be_bytes(*b"hkid");
const EVENT_NOT_HANDLED: i32 = -9874;
const EVENT_HOT_KEY_EXISTS: i32 = -9878;

const COMMAND_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const OPTION_KEY: u32 = 1 << 11;
const CONTROL_KEY: u32 = 1 << 12;
const HOT_KEY_EXCLUSIVE: u32 = 1;

pub struct MacOsShortcutRegistry {
    inner: Registry<CarbonSystem>,
}

impl MacOsShortcutRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Registry::new(CarbonSystem),
        }
    }
}

impl Default for MacOsShortcutRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutRegistry for MacOsShortcutRegistry {
    fn register(
        &mut self,
        shortcuts: &ShortcutConfiguration,
        event_handler: Arc<dyn ShortcutEventHandler>,
    ) -> Result<(), ShortcutRegistryError> {
        self.inner.register(shortcuts, event_handler)
    }

    fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
        self.inner.unregister_all()
    }
}

struct Registry<S: HotKeySystem> {
    system: S,
    callback: Box<CallbackState>,
    event_handler_ref: Option<S::EventHandlerRef>,
    hot_key_refs: Vec<S::HotKeyRef>,
    configuration: Option<ShortcutConfiguration>,
}

impl<S: HotKeySystem> Registry<S> {
    fn new(system: S) -> Self {
        Self {
            system,
            callback: Box::new(CallbackState::default()),
            event_handler_ref: None,
            hot_key_refs: Vec::new(),
            configuration: None,
        }
    }

    fn ensure_event_handler(&mut self) -> Result<(), ShortcutRegistryError> {
        if self.event_handler_ref.is_some() {
            return Ok(());
        }

        let user_data = (&mut *self.callback as *mut CallbackState).cast::<c_void>();
        self.event_handler_ref = Some(
            self.system
                .install_event_handler(user_data)
                .map_err(|_| ShortcutRegistryError::RegistrationFailed)?,
        );
        Ok(())
    }

    fn register_configuration(
        &mut self,
        configuration: ShortcutConfiguration,
    ) -> Result<Vec<S::HotKeyRef>, ShortcutRegistryError> {
        let mut registered = Vec::with_capacity(2);

        for action in [TextAction::Translate, TextAction::Proofread] {
            let shortcut = configuration.shortcut_for(action);
            let Some(key_code) = key_code(shortcut.key()) else {
                self.cleanup_hot_keys(registered);
                return Err(ShortcutRegistryError::RegistrationFailed);
            };

            match self.system.register_hot_key(
                key_code,
                modifier_flags(shortcut.modifiers()),
                hot_key_id(action),
            ) {
                Ok(reference) => registered.push(reference),
                Err(SystemError::HotKeyExists) => {
                    self.cleanup_hot_keys(registered);
                    return Err(ShortcutRegistryError::ShortcutUnavailable { shortcut });
                }
                Err(SystemError::Failed) => {
                    self.cleanup_hot_keys(registered);
                    return Err(ShortcutRegistryError::RegistrationFailed);
                }
            }
        }

        Ok(registered)
    }

    fn unregister_hot_keys(&mut self) -> Result<(), ShortcutRegistryError> {
        let references = std::mem::take(&mut self.hot_key_refs);
        let mut failed = Vec::new();

        for reference in references {
            if self.system.unregister_hot_key(reference).is_err() {
                failed.push(reference);
            }
        }

        self.hot_key_refs = failed;
        if self.hot_key_refs.is_empty() {
            Ok(())
        } else {
            Err(ShortcutRegistryError::UnregistrationFailed)
        }
    }

    fn cleanup_hot_keys(&mut self, references: Vec<S::HotKeyRef>) {
        for reference in references {
            let _ = self.system.unregister_hot_key(reference);
        }
    }

    fn remove_event_handler(&mut self) -> Result<(), ShortcutRegistryError> {
        let Some(reference) = self.event_handler_ref else {
            return Ok(());
        };

        self.system
            .remove_event_handler(reference)
            .map_err(|_| ShortcutRegistryError::UnregistrationFailed)?;
        self.event_handler_ref = None;
        Ok(())
    }
}

impl<S: HotKeySystem> ShortcutRegistry for Registry<S> {
    fn register(
        &mut self,
        shortcuts: &ShortcutConfiguration,
        event_handler: Arc<dyn ShortcutEventHandler>,
    ) -> Result<(), ShortcutRegistryError> {
        self.ensure_event_handler()?;

        if self.configuration == Some(*shortcuts) {
            self.callback.set_handler(event_handler);
            return Ok(());
        }

        let previous_configuration = self.configuration.take();
        self.unregister_hot_keys()?;

        match self.register_configuration(*shortcuts) {
            Ok(references) => {
                self.hot_key_refs = references;
                self.configuration = Some(*shortcuts);
                self.callback.set_handler(event_handler);
                Ok(())
            }
            Err(error) => {
                if let Some(previous) = previous_configuration {
                    match self.register_configuration(previous) {
                        Ok(references) => {
                            self.hot_key_refs = references;
                            self.configuration = Some(previous);
                        }
                        Err(_) => {
                            self.hot_key_refs.clear();
                            self.callback.clear_handler();
                        }
                    }
                }
                Err(error)
            }
        }
    }

    fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
        self.configuration = None;
        self.unregister_hot_keys()?;
        self.callback.clear_handler();
        self.remove_event_handler()
    }
}

impl<S: HotKeySystem> Drop for Registry<S> {
    fn drop(&mut self) {
        self.configuration = None;
        let _ = self.unregister_hot_keys();
        self.callback.clear_handler();
        let _ = self.remove_event_handler();
    }
}

#[derive(Default)]
struct CallbackState {
    handler: Mutex<Option<Arc<dyn ShortcutEventHandler>>>,
}

impl CallbackState {
    fn set_handler(&self, handler: Arc<dyn ShortcutEventHandler>) {
        *self.handler.lock().expect("shortcut handler lock poisoned") = Some(handler);
    }

    fn clear_handler(&self) {
        *self.handler.lock().expect("shortcut handler lock poisoned") = None;
    }

    fn dispatch(&self, id: EventHotKeyId) -> bool {
        if id.signature != HOT_KEY_SIGNATURE {
            return false;
        }

        let action = match id.id {
            TRANSLATE_HOT_KEY_ID => TextAction::Translate,
            PROOFREAD_HOT_KEY_ID => TextAction::Proofread,
            _ => return false,
        };
        let handler = self
            .handler
            .lock()
            .expect("shortcut handler lock poisoned")
            .clone();

        if let Some(handler) = handler {
            handler.on_shortcut(action);
            true
        } else {
            false
        }
    }
}

fn hot_key_id(action: TextAction) -> EventHotKeyId {
    EventHotKeyId {
        signature: HOT_KEY_SIGNATURE,
        id: match action {
            TextAction::Translate => TRANSLATE_HOT_KEY_ID,
            TextAction::Proofread => PROOFREAD_HOT_KEY_ID,
        },
    }
}

fn modifier_flags(modifiers: ShortcutModifiers) -> u32 {
    let mut flags = 0;
    if modifiers.command() {
        flags |= COMMAND_KEY;
    }
    if modifiers.control() {
        flags |= CONTROL_KEY;
    }
    if modifiers.option() {
        flags |= OPTION_KEY;
    }
    if modifiers.shift() {
        flags |= SHIFT_KEY;
    }
    flags
}

fn key_code(key: ShortcutKey) -> Option<u32> {
    if let Some(character) = key.character_value() {
        return match character {
            'A' => Some(0x00),
            'S' => Some(0x01),
            'D' => Some(0x02),
            'F' => Some(0x03),
            'H' => Some(0x04),
            'G' => Some(0x05),
            'Z' => Some(0x06),
            'X' => Some(0x07),
            'C' => Some(0x08),
            'V' => Some(0x09),
            'B' => Some(0x0B),
            'Q' => Some(0x0C),
            'W' => Some(0x0D),
            'E' => Some(0x0E),
            'R' => Some(0x0F),
            'Y' => Some(0x10),
            'T' => Some(0x11),
            '1' => Some(0x12),
            '2' => Some(0x13),
            '3' => Some(0x14),
            '4' => Some(0x15),
            '6' => Some(0x16),
            '5' => Some(0x17),
            '=' => Some(0x18),
            '9' => Some(0x19),
            '7' => Some(0x1A),
            '-' => Some(0x1B),
            '8' => Some(0x1C),
            '0' => Some(0x1D),
            ']' => Some(0x1E),
            'O' => Some(0x1F),
            'U' => Some(0x20),
            '[' => Some(0x21),
            'I' => Some(0x22),
            'P' => Some(0x23),
            'L' => Some(0x25),
            'J' => Some(0x26),
            '\'' => Some(0x27),
            'K' => Some(0x28),
            ';' => Some(0x29),
            '\\' => Some(0x2A),
            ',' => Some(0x2B),
            '/' => Some(0x2C),
            'N' => Some(0x2D),
            'M' => Some(0x2E),
            '.' => Some(0x2F),
            '`' => Some(0x32),
            _ => None,
        };
    }

    if let Some(number) = key.function_number() {
        return match number {
            1 => Some(0x7A),
            2 => Some(0x78),
            3 => Some(0x63),
            4 => Some(0x76),
            5 => Some(0x60),
            6 => Some(0x61),
            7 => Some(0x62),
            8 => Some(0x64),
            9 => Some(0x65),
            10 => Some(0x6D),
            11 => Some(0x67),
            12 => Some(0x6F),
            13 => Some(0x69),
            14 => Some(0x6B),
            15 => Some(0x71),
            16 => Some(0x6A),
            17 => Some(0x40),
            18 => Some(0x4F),
            19 => Some(0x50),
            20 => Some(0x5A),
            _ => None,
        };
    }

    match key.named_value()? {
        NamedShortcutKey::Space => Some(0x31),
        NamedShortcutKey::Return => Some(0x24),
        NamedShortcutKey::Tab => Some(0x30),
        NamedShortcutKey::Escape => Some(0x35),
        NamedShortcutKey::Delete => Some(0x33),
        NamedShortcutKey::ArrowUp => Some(0x7E),
        NamedShortcutKey::ArrowDown => Some(0x7D),
        NamedShortcutKey::ArrowLeft => Some(0x7B),
        NamedShortcutKey::ArrowRight => Some(0x7C),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SystemError {
    HotKeyExists,
    Failed,
}

trait HotKeySystem {
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

struct CarbonSystem;

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
struct EventHotKeyId {
    signature: u32,
    id: u32,
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

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        ffi::c_void,
        sync::{Arc, Mutex},
    };

    use super::{
        CallbackState, EventHotKeyId, HOT_KEY_SIGNATURE, HotKeySystem, MacOsShortcutRegistry,
        PROOFREAD_HOT_KEY_ID, Registry, SystemError, TRANSLATE_HOT_KEY_ID, key_code,
        modifier_flags,
    };
    use verba_core::{
        presentation::TextAction,
        shortcut::{
            NamedShortcutKey, Shortcut, ShortcutConfiguration, ShortcutEventHandler, ShortcutKey,
            ShortcutModifiers, ShortcutRegistry, ShortcutRegistryError,
        },
    };

    #[derive(Default)]
    struct RecordingHandler(Mutex<Vec<TextAction>>);

    impl ShortcutEventHandler for RecordingHandler {
        fn on_shortcut(&self, action: TextAction) {
            self.0.lock().expect("action lock poisoned").push(action);
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct RegisterCall {
        key_code: u32,
        modifiers: u32,
        id: EventHotKeyId,
    }

    #[derive(Default)]
    struct FakeSystemState {
        install_count: usize,
        removed_handlers: Vec<u32>,
        register_calls: Vec<RegisterCall>,
        unregistered_hot_keys: Vec<u32>,
        registration_failures: VecDeque<(usize, SystemError)>,
        next_reference: u32,
    }

    #[derive(Default)]
    struct FakeSystem {
        state: Arc<Mutex<FakeSystemState>>,
    }

    impl FakeSystem {
        fn state(&self) -> Arc<Mutex<FakeSystemState>> {
            self.state.clone()
        }
    }

    impl HotKeySystem for FakeSystem {
        type HotKeyRef = u32;
        type EventHandlerRef = u32;

        fn install_event_handler(
            &mut self,
            _user_data: *mut c_void,
        ) -> Result<Self::EventHandlerRef, SystemError> {
            let mut state = self.state.lock().expect("system lock poisoned");
            state.install_count += 1;
            Ok(100)
        }

        fn remove_event_handler(
            &mut self,
            reference: Self::EventHandlerRef,
        ) -> Result<(), SystemError> {
            self.state
                .lock()
                .expect("system lock poisoned")
                .removed_handlers
                .push(reference);
            Ok(())
        }

        fn register_hot_key(
            &mut self,
            key_code: u32,
            modifiers: u32,
            id: EventHotKeyId,
        ) -> Result<Self::HotKeyRef, SystemError> {
            let mut state = self.state.lock().expect("system lock poisoned");
            state.register_calls.push(RegisterCall {
                key_code,
                modifiers,
                id,
            });
            let call_number = state.register_calls.len();

            if state
                .registration_failures
                .front()
                .is_some_and(|(failure_call, _)| *failure_call == call_number)
            {
                let (_, error) = state
                    .registration_failures
                    .pop_front()
                    .expect("matching failure should exist");
                return Err(error);
            }

            state.next_reference += 1;
            Ok(state.next_reference)
        }

        fn unregister_hot_key(&mut self, reference: Self::HotKeyRef) -> Result<(), SystemError> {
            self.state
                .lock()
                .expect("system lock poisoned")
                .unregistered_hot_keys
                .push(reference);
            Ok(())
        }
    }

    fn shortcut(character: char) -> Shortcut {
        Shortcut::new(
            ShortcutKey::character(character).expect("fixture key should be valid"),
            ShortcutModifiers::new(false, true, true, false),
        )
        .expect("fixture shortcut should be valid")
    }

    #[test]
    fn maps_core_keys_and_modifiers_to_carbon_values() {
        assert_eq!(key_code(ShortcutKey::character('T').unwrap()), Some(0x11));
        assert_eq!(key_code(ShortcutKey::character('P').unwrap()), Some(0x23));
        assert_eq!(key_code(ShortcutKey::function(12).unwrap()), Some(0x6F));
        assert_eq!(
            key_code(ShortcutKey::named(NamedShortcutKey::ArrowLeft)),
            Some(0x7B)
        );
        assert_eq!(
            modifier_flags(ShortcutModifiers::new(true, true, true, true)),
            (1 << 8) | (1 << 9) | (1 << 11) | (1 << 12)
        );
    }

    #[test]
    fn callback_dispatches_only_verba_hot_key_ids() {
        let callback = CallbackState::default();
        let handler = Arc::new(RecordingHandler::default());
        callback.set_handler(handler.clone());

        assert!(callback.dispatch(EventHotKeyId {
            signature: HOT_KEY_SIGNATURE,
            id: TRANSLATE_HOT_KEY_ID,
        }));
        assert!(callback.dispatch(EventHotKeyId {
            signature: HOT_KEY_SIGNATURE,
            id: PROOFREAD_HOT_KEY_ID,
        }));
        assert!(!callback.dispatch(EventHotKeyId {
            signature: u32::from_be_bytes(*b"Else"),
            id: TRANSLATE_HOT_KEY_ID,
        }));
        assert_eq!(
            *handler.0.lock().expect("action lock poisoned"),
            vec![TextAction::Translate, TextAction::Proofread]
        );
    }

    #[test]
    fn registration_replaces_changed_shortcuts_and_handler() {
        let system = FakeSystem::default();
        let state = system.state();
        let mut registry = Registry::new(system);
        let original = ShortcutConfiguration::default();
        let first_handler = Arc::new(RecordingHandler::default());

        registry
            .register(&original, first_handler)
            .expect("initial registration should succeed");
        let replacement_handler = Arc::new(RecordingHandler::default());
        registry
            .register(&original, replacement_handler.clone())
            .expect("unchanged shortcuts should update their handler");

        let replacement = ShortcutConfiguration::new(shortcut('A'), shortcut('B')).unwrap();
        registry
            .register(&replacement, replacement_handler.clone())
            .expect("replacement registration should succeed");
        assert!(registry.callback.dispatch(EventHotKeyId {
            signature: HOT_KEY_SIGNATURE,
            id: TRANSLATE_HOT_KEY_ID,
        }));

        let state = state.lock().expect("system lock poisoned");
        assert_eq!(state.install_count, 1);
        assert_eq!(state.register_calls.len(), 4);
        assert_eq!(state.unregistered_hot_keys, vec![1, 2]);
        assert_eq!(registry.configuration, Some(replacement));
        assert_eq!(
            *replacement_handler.0.lock().expect("action lock poisoned"),
            vec![TextAction::Translate]
        );
    }

    #[test]
    fn failed_replacement_restores_previous_shortcuts_and_handler() {
        let system = FakeSystem::default();
        let state = system.state();
        let mut registry = Registry::new(system);
        let original = ShortcutConfiguration::default();
        let original_handler = Arc::new(RecordingHandler::default());
        registry
            .register(&original, original_handler.clone())
            .expect("initial registration should succeed");

        state
            .lock()
            .expect("system lock poisoned")
            .registration_failures
            .push_back((4, SystemError::HotKeyExists));
        let replacement = ShortcutConfiguration::new(shortcut('A'), shortcut('B')).unwrap();
        let replacement_handler = Arc::new(RecordingHandler::default());

        assert_eq!(
            registry.register(&replacement, replacement_handler),
            Err(ShortcutRegistryError::ShortcutUnavailable {
                shortcut: replacement.shortcut_for(TextAction::Proofread),
            })
        );
        assert_eq!(registry.configuration, Some(original));
        assert!(registry.callback.dispatch(EventHotKeyId {
            signature: HOT_KEY_SIGNATURE,
            id: PROOFREAD_HOT_KEY_ID,
        }));
        assert_eq!(
            *original_handler.0.lock().expect("action lock poisoned"),
            vec![TextAction::Proofread]
        );

        let state = state.lock().expect("system lock poisoned");
        assert_eq!(state.register_calls.len(), 6);
        assert_eq!(state.unregistered_hot_keys, vec![1, 2, 3]);
    }

    #[test]
    fn unregister_all_removes_hot_keys_and_event_handler() {
        let system = FakeSystem::default();
        let state = system.state();
        let mut registry = Registry::new(system);
        registry
            .register(
                &ShortcutConfiguration::default(),
                Arc::new(RecordingHandler::default()),
            )
            .expect("registration should succeed");

        registry
            .unregister_all()
            .expect("unregistration should succeed");
        let state = state.lock().expect("system lock poisoned");
        assert_eq!(state.unregistered_hot_keys, vec![1, 2]);
        assert_eq!(state.removed_handlers, vec![100]);
        assert_eq!(registry.configuration, None);
        assert!(registry.event_handler_ref.is_none());
    }

    #[test]
    fn drop_releases_native_registrations() {
        let system = FakeSystem::default();
        let state = system.state();
        {
            let mut registry = Registry::new(system);
            registry
                .register(
                    &ShortcutConfiguration::default(),
                    Arc::new(RecordingHandler::default()),
                )
                .expect("registration should succeed");
        }

        let state = state.lock().expect("system lock poisoned");
        assert_eq!(state.unregistered_hot_keys, vec![1, 2]);
        assert_eq!(state.removed_handlers, vec![100]);
    }

    #[test]
    fn public_registry_is_constructible_without_native_side_effects() {
        let _registry = MacOsShortcutRegistry::new();
    }
}
