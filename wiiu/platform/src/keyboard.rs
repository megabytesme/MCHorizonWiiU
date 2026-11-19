//! Cafe `nsyskbd` events normalized to Horizon's shared keyboard contract.

use horizon_client_platform::{
    ClientWindowEvent, KeyCode, KeyLocation, KeyState, KeyboardEvent, LogicalKey, Modifiers,
};

const CHANNELS: usize = 256;
const USAGES_PER_CHANNEL: usize = 256;
const WORDS_PER_CHANNEL: usize = USAGES_PER_CHANNEL / u64::BITS as usize;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RawKeyboardEvent {
    kind: u8,
    channel: u8,
    hid_code: u8,
    is_pressed_down: u8,
    utf16_character: u16,
    reserved: u16,
}

const _: () = {
    assert!(core::mem::size_of::<RawKeyboardEvent>() == 8);
    assert!(core::mem::offset_of!(RawKeyboardEvent, utf16_character) == 4);
    assert!(core::mem::offset_of!(RawKeyboardEvent, reserved) == 6);
};

const ATTACHED: u8 = 1;
const DETACHED: u8 = 2;
const KEY: u8 = 3;

unsafe extern "C" {
    fn horizon_wiiu_keyboard_init() -> i32;
    fn horizon_wiiu_keyboard_teardown() -> i32;
    fn horizon_wiiu_keyboard_pop(event: *mut RawKeyboardEvent) -> i32;
    fn horizon_wiiu_keyboard_dropped() -> u32;
}

/// Initialize `nsyskbd` and register the event-copying callbacks.
pub fn initialize() -> Result<(), i32> {
    match unsafe { horizon_wiiu_keyboard_init() } {
        0 => Ok(()),
        error => Err(error),
    }
}

/// Stop `nsyskbd` and unregister the event-copying callbacks.
pub fn shutdown() -> i32 {
    unsafe { horizon_wiiu_keyboard_teardown() }
}

/// Converts queued USB keyboard records and tracks held keys by device channel.
pub struct KeyboardAdapter {
    pressed: [[u64; WORDS_PER_CHANNEL]; CHANNELS],
    held_characters: [u16; USAGES_PER_CHANNEL],
    observed_drops: u32,
    modifiers: Modifiers,
}

impl Default for KeyboardAdapter {
    fn default() -> Self {
        Self {
            pressed: [[0; WORDS_PER_CHANNEL]; CHANNELS],
            held_characters: [0; USAGES_PER_CHANNEL],
            observed_drops: 0,
            modifiers: Modifiers::empty(),
        }
    }
}

impl KeyboardAdapter {
    /// Emit synthetic releases for all currently held USB keys.
    pub fn release_all_into(&mut self, output: &mut Vec<ClientWindowEvent>) {
        self.release_all(output);
    }

    /// Drain queued events into the caller's reusable output vector.
    pub fn drain_into(&mut self, output: &mut Vec<ClientWindowEvent>) {
        let dropped = unsafe { horizon_wiiu_keyboard_dropped() };
        if dropped != self.observed_drops {
            self.observed_drops = dropped;
            self.release_all(output);
            // Discard records queued before recovery. Their transitions may no
            // longer describe the current held state after an overflow.
            let mut ignored = RawKeyboardEvent::default();
            while unsafe { horizon_wiiu_keyboard_pop(&mut ignored) } != 0 {}
            return;
        }

        loop {
            let mut raw = RawKeyboardEvent::default();
            if unsafe { horizon_wiiu_keyboard_pop(&mut raw) } == 0 {
                break;
            }
            match raw.kind {
                ATTACHED => {}
                DETACHED => self.release_channel(raw.channel, output),
                KEY => self.key_event(raw, output),
                _ => {}
            }
        }
    }

    fn key_event(&mut self, raw: RawKeyboardEvent, output: &mut Vec<ClientWindowEvent>) {
        let usage = raw.hid_code as usize;
        let channel = raw.channel as usize;
        let was_pressed = self.is_pressed(channel, usage);
        let was_visible = self.is_visible(usage);
        let pressed = raw.is_pressed_down != 0;
        if !pressed && !was_pressed {
            return;
        }
        if pressed {
            self.set_pressed(channel, usage, true);
            if !was_visible {
                self.held_characters[usage] = raw.utf16_character;
            }
        } else {
            self.set_pressed(channel, usage, false);
        }

        if pressed && was_pressed {
            output.push(keyboard_event(
                raw.hid_code,
                KeyState::Pressed,
                self.held_characters[usage],
                true,
                false,
            ));
            return;
        }

        let is_visible = self.is_visible(usage);
        if was_visible == is_visible {
            self.update_modifiers(output);
            return;
        }

        let character = if pressed {
            raw.utf16_character
        } else {
            self.held_characters[usage]
        };
        output.push(keyboard_event(
            raw.hid_code,
            if pressed {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
            character,
            pressed && was_pressed,
            false,
        ));
        self.update_modifiers(output);
    }

    fn release_channel(&mut self, channel: u8, output: &mut Vec<ClientWindowEvent>) {
        let channel = channel as usize;
        for usage in 0..USAGES_PER_CHANNEL {
            if self.is_pressed(channel, usage) {
                self.set_pressed(channel, usage, false);
                let usage = usage as u8;
                if !self.is_visible(usage as usize) {
                    output.push(keyboard_event(
                        usage,
                        KeyState::Released,
                        self.held_characters[usage as usize],
                        false,
                        true,
                    ));
                }
            }
        }
        self.update_modifiers(output);
    }

    fn release_all(&mut self, output: &mut Vec<ClientWindowEvent>) {
        for channel in 0..CHANNELS {
            for usage in 0..USAGES_PER_CHANNEL {
                if self.is_pressed(channel, usage) {
                    self.set_pressed(channel, usage, false);
                    if !self.is_visible(usage) {
                        output.push(keyboard_event(
                            usage as u8,
                            KeyState::Released,
                            self.held_characters[usage],
                            false,
                            true,
                        ));
                    }
                }
            }
        }
        self.update_modifiers(output);
    }

    fn is_pressed(&self, channel: usize, usage: usize) -> bool {
        self.pressed[channel][usage / 64] & (1u64 << (usage % 64)) != 0
    }

    fn is_visible(&self, usage: usize) -> bool {
        (0..CHANNELS).any(|channel| self.is_pressed(channel, usage))
    }

    fn set_pressed(&mut self, channel: usize, usage: usize, pressed: bool) {
        let bit = 1u64 << (usage % 64);
        let word = &mut self.pressed[channel][usage / 64];
        if pressed {
            *word |= bit;
        } else {
            *word &= !bit;
        }
    }

    fn update_modifiers(&mut self, output: &mut Vec<ClientWindowEvent>) {
        let mut next = Modifiers::empty();
        for channel in 0..CHANNELS {
            for (usage, modifier) in [
                (0xE0, Modifiers::CONTROL),
                (0xE1, Modifiers::SHIFT),
                (0xE2, Modifiers::ALT),
                (0xE3, Modifiers::SUPER),
                (0xE4, Modifiers::CONTROL),
                (0xE5, Modifiers::SHIFT),
                (0xE6, Modifiers::ALT),
                (0xE7, Modifiers::SUPER),
            ] {
                if self.is_pressed(channel, usage) {
                    next.set(modifier, true);
                }
            }
        }
        if next != self.modifiers {
            self.modifiers = next;
            output.push(ClientWindowEvent::ModifiersChanged(next));
        }
    }
}

fn keyboard_event(
    usage: u8,
    state: KeyState,
    utf16_character: u16,
    repeat: bool,
    is_synthetic: bool,
) -> ClientWindowEvent {
    let key = key_code(usage);
    let text = if state == KeyState::Pressed {
        char::from_u32(utf16_character as u32).map(|c| c.to_string())
    } else {
        None
    };
    let logical_key = char::from_u32(utf16_character as u32)
        .map(|c| LogicalKey::Character(c.to_string()))
        .unwrap_or_else(|| logical_key(key));
    ClientWindowEvent::KeyboardInput {
        event: KeyboardEvent {
            physical_key: Some(key),
            logical_key,
            location: key_location(usage),
            state,
            logical_text: char::from_u32(utf16_character as u32).map(|c| c.to_string()),
            text,
            unmodified_text: None,
            repeat,
        },
        is_synthetic,
    }
}

fn key_location(usage: u8) -> KeyLocation {
    match usage {
        0xE0 | 0xE2 | 0xE4 | 0xE6 => KeyLocation::Left,
        0xE1 | 0xE3 | 0xE5 | 0xE7 => KeyLocation::Right,
        0x53..=0x63 => KeyLocation::Numpad,
        _ => KeyLocation::Standard,
    }
}

fn logical_key(key: KeyCode) -> LogicalKey {
    let name = match key {
        KeyCode::Enter => "Enter",
        KeyCode::Escape => "Escape",
        KeyCode::Backspace => "Backspace",
        KeyCode::Tab => "Tab",
        KeyCode::Space => " ",
        KeyCode::ArrowDown => "ArrowDown",
        KeyCode::ArrowLeft => "ArrowLeft",
        KeyCode::ArrowRight => "ArrowRight",
        KeyCode::ArrowUp => "ArrowUp",
        KeyCode::Delete => "Delete",
        KeyCode::Insert => "Insert",
        KeyCode::Home => "Home",
        KeyCode::End => "End",
        KeyCode::PageUp => "PageUp",
        KeyCode::PageDown => "PageDown",
        KeyCode::ShiftLeft => "Shift",
        KeyCode::ShiftRight => "Shift",
        KeyCode::ControlLeft | KeyCode::ControlRight => "Control",
        KeyCode::AltLeft | KeyCode::AltRight => "Alt",
        KeyCode::SuperLeft | KeyCode::SuperRight => "Super",
        KeyCode::CapsLock => "CapsLock",
        KeyCode::F1 => "F1",
        KeyCode::F2 => "F2",
        KeyCode::F3 => "F3",
        KeyCode::F4 => "F4",
        KeyCode::F5 => "F5",
        KeyCode::F6 => "F6",
        KeyCode::F7 => "F7",
        KeyCode::F8 => "F8",
        KeyCode::F9 => "F9",
        KeyCode::F10 => "F10",
        KeyCode::F11 => "F11",
        KeyCode::F12 => "F12",
        _ => return LogicalKey::Unidentified,
    };
    LogicalKey::Named(name.to_owned())
}

fn key_code(usage: u8) -> KeyCode {
    use KeyCode::*;
    match usage {
        0x04 => KeyA,
        0x05 => KeyB,
        0x06 => KeyC,
        0x07 => KeyD,
        0x08 => KeyE,
        0x09 => KeyF,
        0x0A => KeyG,
        0x0B => KeyH,
        0x0C => KeyI,
        0x0D => KeyJ,
        0x0E => KeyK,
        0x0F => KeyL,
        0x10 => KeyM,
        0x11 => KeyN,
        0x12 => KeyO,
        0x13 => KeyP,
        0x14 => KeyQ,
        0x15 => KeyR,
        0x16 => KeyS,
        0x17 => KeyT,
        0x18 => KeyU,
        0x19 => KeyV,
        0x1A => KeyW,
        0x1B => KeyX,
        0x1C => KeyY,
        0x1D => KeyZ,
        0x1E => Digit1,
        0x1F => Digit2,
        0x20 => Digit3,
        0x21 => Digit4,
        0x22 => Digit5,
        0x23 => Digit6,
        0x24 => Digit7,
        0x25 => Digit8,
        0x26 => Digit9,
        0x27 => Digit0,
        0x28 => Enter,
        0x29 => Escape,
        0x2A => Backspace,
        0x2B => Tab,
        0x2C => Space,
        0x2D => Minus,
        0x2E => Equal,
        0x2F => BracketLeft,
        0x30 => BracketRight,
        0x31 => Backslash,
        0x32 | 0x64 => IntlBackslash,
        0x33 => Semicolon,
        0x34 => Quote,
        0x35 => Backquote,
        0x36 => Comma,
        0x37 => Period,
        0x38 => Slash,
        0x39 => CapsLock,
        0x3A => F1,
        0x3B => F2,
        0x3C => F3,
        0x3D => F4,
        0x3E => F5,
        0x3F => F6,
        0x40 => F7,
        0x41 => F8,
        0x42 => F9,
        0x43 => F10,
        0x44 => F11,
        0x45 => F12,
        0x46 => PrintScreen,
        0x47 => ScrollLock,
        0x48 => Pause,
        0x49 => Insert,
        0x4A => Home,
        0x4B => PageUp,
        0x4C => Delete,
        0x4D => End,
        0x4E => PageDown,
        0x4F => ArrowRight,
        0x50 => ArrowLeft,
        0x51 => ArrowDown,
        0x52 => ArrowUp,
        0x53 => NumLock,
        0x54 => NumpadDivide,
        0x55 => NumpadMultiply,
        0x56 => NumpadSubtract,
        0x57 => NumpadAdd,
        0x58 => NumpadEnter,
        0x59 => Numpad1,
        0x5A => Numpad2,
        0x5B => Numpad3,
        0x5C => Numpad4,
        0x5D => Numpad5,
        0x5E => Numpad6,
        0x5F => Numpad7,
        0x60 => Numpad8,
        0x61 => Numpad9,
        0x62 => Numpad0,
        0x63 => NumpadDecimal,
        0x65 => ContextMenu,
        0xE0 => ControlLeft,
        0xE1 => ShiftLeft,
        0xE2 => AltLeft,
        0xE3 => SuperLeft,
        0xE4 => ControlRight,
        0xE5 => ShiftRight,
        0xE6 => AltRight,
        0xE7 => SuperRight,
        _ => Other,
    }
}
