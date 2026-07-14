use std::{error::Error, fmt, sync::Arc};

use verba_core::{
    presentation::TextAction,
    shortcut::{
        NamedShortcutKey, Shortcut, ShortcutConfiguration, ShortcutConfigurationError,
        ShortcutEventHandler, ShortcutKey, ShortcutModifiers, ShortcutRegistry,
        ShortcutRegistryError, ShortcutSettingsStore, ShortcutValidationError,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Enum)]
pub enum ShortcutSettingsAction {
    Translate,
    Proofread,
}

impl From<ShortcutSettingsAction> for TextAction {
    fn from(action: ShortcutSettingsAction) -> Self {
        match action {
            ShortcutSettingsAction::Translate => Self::Translate,
            ShortcutSettingsAction::Proofread => Self::Proofread,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ShortcutInput {
    pub key: String,
    pub command: bool,
    pub control: bool,
    pub option: bool,
    pub shift: bool,
}

impl TryFrom<ShortcutInput> for Shortcut {
    type Error = ShortcutSettingsError;

    fn try_from(input: ShortcutInput) -> Result<Self, Self::Error> {
        let key = parse_key(&input.key)?;
        Shortcut::new(
            key,
            ShortcutModifiers::new(input.command, input.control, input.option, input.shift),
        )
        .map_err(Into::into)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, uniffi::Record)]
pub struct ShortcutConfigurationViewModel {
    pub translate: String,
    pub proofread: String,
}

impl From<ShortcutConfiguration> for ShortcutConfigurationViewModel {
    fn from(configuration: ShortcutConfiguration) -> Self {
        Self {
            translate: display_shortcut(configuration.shortcut_for(TextAction::Translate)),
            proofread: display_shortcut(configuration.shortcut_for(TextAction::Proofread)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, uniffi::Error)]
pub enum ShortcutSettingsError {
    InvalidKey,
    MissingPrimaryModifier,
    ReservedShortcut,
    DuplicateShortcut,
    ShortcutUnavailable,
    RegistrationFailed,
    PersistenceFailed,
    RollbackFailed,
}

impl From<ShortcutValidationError> for ShortcutSettingsError {
    fn from(error: ShortcutValidationError) -> Self {
        match error {
            ShortcutValidationError::InvalidCharacter
            | ShortcutValidationError::InvalidFunctionKey => Self::InvalidKey,
            ShortcutValidationError::MissingPrimaryModifier => Self::MissingPrimaryModifier,
            ShortcutValidationError::ReservedShortcut => Self::ReservedShortcut,
        }
    }
}

impl fmt::Display for ShortcutSettingsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidKey => "invalid shortcut key",
            Self::MissingPrimaryModifier => "shortcut requires Command, Control, or Option",
            Self::ReservedShortcut => "shortcut is reserved by macOS",
            Self::DuplicateShortcut => "shortcut is already assigned",
            Self::ShortcutUnavailable => "shortcut is already in use",
            Self::RegistrationFailed => "shortcut registration failed",
            Self::PersistenceFailed => "shortcut could not be saved",
            Self::RollbackFailed => "previous shortcut could not be restored",
        };
        formatter.write_str(message)
    }
}

impl Error for ShortcutSettingsError {}

pub(crate) fn replacement_configuration(
    current: ShortcutConfiguration,
    action: ShortcutSettingsAction,
    input: ShortcutInput,
) -> Result<ShortcutConfiguration, ShortcutSettingsError> {
    current
        .with_shortcut(action.into(), input.try_into()?)
        .map_err(|error| match error {
            ShortcutConfigurationError::Collision { .. } => {
                ShortcutSettingsError::DuplicateShortcut
            }
        })
}

pub(crate) fn register_and_save<R: ShortcutRegistry + ?Sized>(
    registry: &mut R,
    event_handler: Arc<dyn ShortcutEventHandler>,
    store: &dyn ShortcutSettingsStore,
    current: ShortcutConfiguration,
    replacement: ShortcutConfiguration,
) -> Result<(), ShortcutSettingsError> {
    registry
        .register(&replacement, Arc::clone(&event_handler))
        .map_err(registration_error)?;

    if store.save(&replacement).is_err() {
        registry
            .register(&current, event_handler)
            .map_err(|_| ShortcutSettingsError::RollbackFailed)?;
        return Err(ShortcutSettingsError::PersistenceFailed);
    }

    Ok(())
}

fn registration_error(error: ShortcutRegistryError) -> ShortcutSettingsError {
    match error {
        ShortcutRegistryError::ShortcutUnavailable { .. } => {
            ShortcutSettingsError::ShortcutUnavailable
        }
        ShortcutRegistryError::RegistrationFailed | ShortcutRegistryError::UnregistrationFailed => {
            ShortcutSettingsError::RegistrationFailed
        }
    }
}

fn parse_key(value: &str) -> Result<ShortcutKey, ShortcutSettingsError> {
    if value.chars().count() == 1 {
        return ShortcutKey::character(value.chars().next().expect("one character"))
            .map_err(Into::into);
    }
    if let Some(number) = value.strip_prefix('F') {
        return ShortcutKey::function(
            number
                .parse()
                .map_err(|_| ShortcutSettingsError::InvalidKey)?,
        )
        .map_err(Into::into);
    }

    let named = match value {
        "space" => NamedShortcutKey::Space,
        "return" => NamedShortcutKey::Return,
        "tab" => NamedShortcutKey::Tab,
        "escape" => NamedShortcutKey::Escape,
        "delete" => NamedShortcutKey::Delete,
        "arrow-up" => NamedShortcutKey::ArrowUp,
        "arrow-down" => NamedShortcutKey::ArrowDown,
        "arrow-left" => NamedShortcutKey::ArrowLeft,
        "arrow-right" => NamedShortcutKey::ArrowRight,
        _ => return Err(ShortcutSettingsError::InvalidKey),
    };
    Ok(ShortcutKey::named(named))
}

fn display_shortcut(shortcut: Shortcut) -> String {
    let modifiers = shortcut.modifiers();
    let mut display = String::new();
    if modifiers.control() {
        display.push('⌃');
    }
    if modifiers.option() {
        display.push('⌥');
    }
    if modifiers.shift() {
        display.push('⇧');
    }
    if modifiers.command() {
        display.push('⌘');
    }
    if let Some(character) = shortcut.key().character_value() {
        display.push(character);
    } else if let Some(number) = shortcut.key().function_number() {
        display.push_str(&format!("F{number}"));
    } else {
        display.push_str(
            match shortcut.key().named_value().expect("shortcut key kind") {
                NamedShortcutKey::Space => "Space",
                NamedShortcutKey::Return => "↩",
                NamedShortcutKey::Tab => "⇥",
                NamedShortcutKey::Escape => "Esc",
                NamedShortcutKey::Delete => "⌫",
                NamedShortcutKey::ArrowUp => "↑",
                NamedShortcutKey::ArrowDown => "↓",
                NamedShortcutKey::ArrowLeft => "←",
                NamedShortcutKey::ArrowRight => "→",
            },
        );
    }
    display
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use verba_core::{
        presentation::TextAction,
        shortcut::{
            ShortcutEventHandler, ShortcutRegistry, ShortcutRegistryError,
            ShortcutSettingsStoreError,
        },
    };

    use super::*;

    #[derive(Default)]
    struct RecordingStore {
        saved: Mutex<Vec<ShortcutConfiguration>>,
        fail: bool,
    }

    impl ShortcutSettingsStore for RecordingStore {
        fn load(&self) -> Result<Option<ShortcutConfiguration>, ShortcutSettingsStoreError> {
            Ok(None)
        }

        fn save(
            &self,
            configuration: &ShortcutConfiguration,
        ) -> Result<(), ShortcutSettingsStoreError> {
            if self.fail {
                return Err(ShortcutSettingsStoreError);
            }
            self.saved.lock().unwrap().push(*configuration);
            Ok(())
        }
    }

    struct NoopHandler;

    impl ShortcutEventHandler for NoopHandler {
        fn on_shortcut(&self, _action: TextAction) {}
    }

    #[derive(Default)]
    struct RecordingRegistry {
        configuration: Option<ShortcutConfiguration>,
    }

    impl ShortcutRegistry for RecordingRegistry {
        fn register(
            &mut self,
            shortcuts: &ShortcutConfiguration,
            _event_handler: Arc<dyn ShortcutEventHandler>,
        ) -> Result<(), ShortcutRegistryError> {
            self.configuration = Some(*shortcuts);
            Ok(())
        }

        fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
            self.configuration = None;
            Ok(())
        }
    }

    fn input(key: &str, control: bool, option: bool) -> ShortcutInput {
        ShortcutInput {
            key: key.to_owned(),
            command: false,
            control,
            option,
            shift: false,
        }
    }

    #[test]
    fn validates_and_formats_recorded_shortcuts() {
        let current = ShortcutConfiguration::default();
        assert_eq!(
            ShortcutConfigurationViewModel::from(current),
            ShortcutConfigurationViewModel {
                translate: "⌃⌥T".to_owned(),
                proofread: "⌃⌥P".to_owned(),
            }
        );
        assert_eq!(
            replacement_configuration(
                current,
                ShortcutSettingsAction::Translate,
                input("P", true, true)
            ),
            Err(ShortcutSettingsError::DuplicateShortcut)
        );
        assert_eq!(
            replacement_configuration(
                current,
                ShortcutSettingsAction::Translate,
                ShortcutInput {
                    key: "space".to_owned(),
                    command: true,
                    control: false,
                    option: false,
                    shift: false,
                }
            ),
            Err(ShortcutSettingsError::ReservedShortcut)
        );
    }

    #[test]
    fn persistence_failure_restores_the_previous_registration() {
        let current = ShortcutConfiguration::default();
        let replacement = replacement_configuration(
            current,
            ShortcutSettingsAction::Translate,
            input("L", true, true),
        )
        .unwrap();
        let mut registry = RecordingRegistry::default();
        registry.register(&current, Arc::new(NoopHandler)).unwrap();

        assert_eq!(
            register_and_save(
                &mut registry,
                Arc::new(NoopHandler),
                &RecordingStore {
                    fail: true,
                    ..Default::default()
                },
                current,
                replacement,
            ),
            Err(ShortcutSettingsError::PersistenceFailed)
        );
        assert_eq!(registry.configuration, Some(current));
    }

    struct FailedRollbackRegistry {
        calls: usize,
    }

    impl ShortcutRegistry for FailedRollbackRegistry {
        fn register(
            &mut self,
            _shortcuts: &ShortcutConfiguration,
            _event_handler: Arc<dyn ShortcutEventHandler>,
        ) -> Result<(), ShortcutRegistryError> {
            self.calls += 1;
            if self.calls == 1 {
                Ok(())
            } else {
                Err(ShortcutRegistryError::RegistrationFailed)
            }
        }

        fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError> {
            Ok(())
        }
    }

    #[test]
    fn failed_persistence_reports_a_failed_rollback() {
        let current = ShortcutConfiguration::default();
        let replacement = replacement_configuration(
            current,
            ShortcutSettingsAction::Translate,
            input("L", true, true),
        )
        .unwrap();

        assert_eq!(
            register_and_save(
                &mut FailedRollbackRegistry { calls: 0 },
                Arc::new(NoopHandler),
                &RecordingStore {
                    fail: true,
                    ..Default::default()
                },
                current,
                replacement,
            ),
            Err(ShortcutSettingsError::RollbackFailed)
        );
    }
}
