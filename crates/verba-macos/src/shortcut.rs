mod carbon;
mod keycodes;

use std::{
    ffi::c_void,
    sync::{Arc, Mutex},
};

use verba_core::{
    presentation::TextAction,
    shortcut::{
        ShortcutConfiguration, ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
    },
};

use carbon::{CarbonSystem, EventHotKeyId, HotKeySystem, SystemError};
use keycodes::{key_code, modifier_flags};

const HOT_KEY_SIGNATURE: u32 = u32::from_be_bytes(*b"Vrba");
const TRANSLATE_HOT_KEY_ID: u32 = 1;
const PROOFREAD_HOT_KEY_ID: u32 = 2;

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

#[cfg(test)]
mod tests;
