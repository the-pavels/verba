use std::sync::Arc;

use crate::presentation::TextAction;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NamedShortcutKey {
    Space,
    Return,
    Tab,
    Escape,
    Delete,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ShortcutKeyKind {
    Character(char),
    Function(u8),
    Named(NamedShortcutKey),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ShortcutKey(ShortcutKeyKind);

impl ShortcutKey {
    pub fn character(character: char) -> Result<Self, ShortcutValidationError> {
        let character = character.to_ascii_uppercase();
        if !matches!(
            character,
            'A'..='Z'
                | '0'..='9'
                | '-'
                | '='
                | '['
                | ']'
                | ';'
                | '\''
                | '\\'
                | ','
                | '.'
                | '/'
                | '`'
        ) {
            return Err(ShortcutValidationError::InvalidCharacter);
        }

        Ok(Self(ShortcutKeyKind::Character(character)))
    }

    pub fn function(number: u8) -> Result<Self, ShortcutValidationError> {
        if !(1..=20).contains(&number) {
            return Err(ShortcutValidationError::InvalidFunctionKey);
        }

        Ok(Self(ShortcutKeyKind::Function(number)))
    }

    #[must_use]
    pub const fn named(key: NamedShortcutKey) -> Self {
        Self(ShortcutKeyKind::Named(key))
    }

    #[must_use]
    pub const fn character_value(self) -> Option<char> {
        match self.0 {
            ShortcutKeyKind::Character(character) => Some(character),
            _ => None,
        }
    }

    #[must_use]
    pub const fn function_number(self) -> Option<u8> {
        match self.0 {
            ShortcutKeyKind::Function(number) => Some(number),
            _ => None,
        }
    }

    #[must_use]
    pub const fn named_value(self) -> Option<NamedShortcutKey> {
        match self.0 {
            ShortcutKeyKind::Named(key) => Some(key),
            _ => None,
        }
    }

    const fn is_function(self) -> bool {
        matches!(self.0, ShortcutKeyKind::Function(_))
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ShortcutModifiers {
    command: bool,
    control: bool,
    option: bool,
    shift: bool,
}

impl ShortcutModifiers {
    #[must_use]
    pub const fn new(command: bool, control: bool, option: bool, shift: bool) -> Self {
        Self {
            command,
            control,
            option,
            shift,
        }
    }

    #[must_use]
    pub const fn command(self) -> bool {
        self.command
    }

    #[must_use]
    pub const fn control(self) -> bool {
        self.control
    }

    #[must_use]
    pub const fn option(self) -> bool {
        self.option
    }

    #[must_use]
    pub const fn shift(self) -> bool {
        self.shift
    }

    const fn has_primary_modifier(self) -> bool {
        self.command || self.control || self.option
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Shortcut {
    key: ShortcutKey,
    modifiers: ShortcutModifiers,
}

impl Shortcut {
    pub fn new(
        key: ShortcutKey,
        modifiers: ShortcutModifiers,
    ) -> Result<Self, ShortcutValidationError> {
        if !key.is_function() && !modifiers.has_primary_modifier() {
            return Err(ShortcutValidationError::MissingPrimaryModifier);
        }

        Ok(Self { key, modifiers })
    }

    #[must_use]
    pub const fn key(self) -> ShortcutKey {
        self.key
    }

    #[must_use]
    pub const fn modifiers(self) -> ShortcutModifiers {
        self.modifiers
    }

    const fn new_unchecked(key: ShortcutKey, modifiers: ShortcutModifiers) -> Self {
        Self { key, modifiers }
    }
}

pub const DEFAULT_TRANSLATE_SHORTCUT: Shortcut = Shortcut::new_unchecked(
    ShortcutKey(ShortcutKeyKind::Character('T')),
    ShortcutModifiers::new(false, true, true, false),
);

pub const DEFAULT_PROOFREAD_SHORTCUT: Shortcut = Shortcut::new_unchecked(
    ShortcutKey(ShortcutKeyKind::Character('P')),
    ShortcutModifiers::new(false, true, true, false),
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutValidationError {
    InvalidCharacter,
    InvalidFunctionKey,
    MissingPrimaryModifier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShortcutConfiguration {
    translate: Shortcut,
    proofread: Shortcut,
}

impl ShortcutConfiguration {
    pub fn new(
        translate: Shortcut,
        proofread: Shortcut,
    ) -> Result<Self, ShortcutConfigurationError> {
        if translate == proofread {
            return Err(ShortcutConfigurationError::Collision {
                shortcut: translate,
            });
        }

        Ok(Self {
            translate,
            proofread,
        })
    }

    #[must_use]
    pub const fn shortcut_for(self, action: TextAction) -> Shortcut {
        match action {
            TextAction::Translate => self.translate,
            TextAction::Proofread => self.proofread,
        }
    }

    pub fn with_shortcut(
        self,
        action: TextAction,
        shortcut: Shortcut,
    ) -> Result<Self, ShortcutConfigurationError> {
        match action {
            TextAction::Translate => Self::new(shortcut, self.proofread),
            TextAction::Proofread => Self::new(self.translate, shortcut),
        }
    }
}

impl Default for ShortcutConfiguration {
    fn default() -> Self {
        Self {
            translate: DEFAULT_TRANSLATE_SHORTCUT,
            proofread: DEFAULT_PROOFREAD_SHORTCUT,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutConfigurationError {
    Collision { shortcut: Shortcut },
}

pub trait ShortcutEventHandler: Send + Sync {
    fn on_shortcut(&self, action: TextAction);
}

pub trait ShortcutRegistry {
    fn register(
        &mut self,
        shortcuts: &ShortcutConfiguration,
        event_handler: Arc<dyn ShortcutEventHandler>,
    ) -> Result<(), ShortcutRegistryError>;

    fn unregister_all(&mut self) -> Result<(), ShortcutRegistryError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShortcutRegistryError {
    ShortcutUnavailable { shortcut: Shortcut },
    RegistrationFailed,
    UnregistrationFailed,
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_PROOFREAD_SHORTCUT, DEFAULT_TRANSLATE_SHORTCUT, NamedShortcutKey, Shortcut,
        ShortcutConfiguration, ShortcutConfigurationError, ShortcutKey, ShortcutModifiers,
        ShortcutValidationError,
    };
    use crate::presentation::TextAction;

    #[test]
    fn character_keys_are_ascii_and_case_normalized() {
        assert_eq!(
            ShortcutKey::character('t').expect("ASCII letters should be valid"),
            ShortcutKey::character('T').expect("ASCII letters should be valid")
        );
        assert_eq!(
            ShortcutKey::character('T')
                .expect("ASCII letters should be valid")
                .character_value(),
            Some('T')
        );
        assert_eq!(
            ShortcutKey::character('é'),
            Err(ShortcutValidationError::InvalidCharacter)
        );
        assert_eq!(
            ShortcutKey::character(' '),
            Err(ShortcutValidationError::InvalidCharacter)
        );
        assert_eq!(
            ShortcutKey::character('!'),
            Err(ShortcutValidationError::InvalidCharacter)
        );
    }

    #[test]
    fn function_key_range_is_validated() {
        assert_eq!(
            ShortcutKey::function(0),
            Err(ShortcutValidationError::InvalidFunctionKey)
        );
        assert_eq!(
            ShortcutKey::function(21),
            Err(ShortcutValidationError::InvalidFunctionKey)
        );
        assert_eq!(
            ShortcutKey::function(12)
                .expect("F12 should be valid")
                .function_number(),
            Some(12)
        );
    }

    #[test]
    fn ordinary_keys_require_a_primary_modifier() {
        let key = ShortcutKey::character('T').expect("T should be valid");

        assert_eq!(
            Shortcut::new(key, ShortcutModifiers::default()),
            Err(ShortcutValidationError::MissingPrimaryModifier)
        );
        assert_eq!(
            Shortcut::new(key, ShortcutModifiers::new(false, false, false, true)),
            Err(ShortcutValidationError::MissingPrimaryModifier)
        );
        assert!(Shortcut::new(key, ShortcutModifiers::new(false, true, false, false)).is_ok());
        assert!(
            Shortcut::new(
                ShortcutKey::named(NamedShortcutKey::Space),
                ShortcutModifiers::new(true, false, false, false)
            )
            .is_ok()
        );
        assert!(
            Shortcut::new(
                ShortcutKey::function(8).expect("F8 should be valid"),
                ShortcutModifiers::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn defaults_assign_distinct_shortcuts_to_both_actions() {
        let shortcuts = ShortcutConfiguration::default();

        assert_eq!(
            shortcuts.shortcut_for(TextAction::Translate),
            DEFAULT_TRANSLATE_SHORTCUT
        );
        assert_eq!(
            shortcuts.shortcut_for(TextAction::Proofread),
            DEFAULT_PROOFREAD_SHORTCUT
        );
        assert_ne!(DEFAULT_TRANSLATE_SHORTCUT, DEFAULT_PROOFREAD_SHORTCUT);
    }

    #[test]
    fn configuration_rejects_initial_and_updated_collisions() {
        assert_eq!(
            ShortcutConfiguration::new(DEFAULT_TRANSLATE_SHORTCUT, DEFAULT_TRANSLATE_SHORTCUT),
            Err(ShortcutConfigurationError::Collision {
                shortcut: DEFAULT_TRANSLATE_SHORTCUT,
            })
        );

        assert_eq!(
            ShortcutConfiguration::default()
                .with_shortcut(TextAction::Proofread, DEFAULT_TRANSLATE_SHORTCUT),
            Err(ShortcutConfigurationError::Collision {
                shortcut: DEFAULT_TRANSLATE_SHORTCUT,
            })
        );
    }
}
