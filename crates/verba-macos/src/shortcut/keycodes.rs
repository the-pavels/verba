use verba_core::shortcut::{NamedShortcutKey, ShortcutKey, ShortcutModifiers};

const COMMAND_KEY: u32 = 1 << 8;
const SHIFT_KEY: u32 = 1 << 9;
const OPTION_KEY: u32 = 1 << 11;
const CONTROL_KEY: u32 = 1 << 12;

pub(super) fn modifier_flags(modifiers: ShortcutModifiers) -> u32 {
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

pub(super) fn key_code(key: ShortcutKey) -> Option<u32> {
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
