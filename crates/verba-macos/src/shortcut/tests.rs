use std::{
    collections::VecDeque,
    ffi::c_void,
    sync::{Arc, Mutex},
};

use super::{
    CallbackState, EventHotKeyId, HOT_KEY_SIGNATURE, HotKeySystem, MacOsShortcutRegistry,
    PROOFREAD_HOT_KEY_ID, Registry, SystemError, TRANSLATE_HOT_KEY_ID, key_code, modifier_flags,
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
