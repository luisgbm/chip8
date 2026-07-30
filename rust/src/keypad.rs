//! The sixteen key hexadecimal keypad. Ported from `js/keyboard.js`.

use sdl2::keyboard::Keycode;

/// Number of keys on a CHIP-8 keypad.
pub const KEY_COUNT: usize = 16;

/// The host keys that stand in for the CHIP-8 keypad, in keypad order.
///
/// The COSMAC VIP keypad was laid out in a four by four grid, so the left hand
/// block of a QWERTY keyboard is mapped onto it geometrically:
///
/// ```text
///   1 2 3 4          1 2 3 C
///   Q W E R    ->    4 5 6 D
///   A S D F          7 8 9 E
///   Z X C V          A 0 B F
/// ```
///
/// The JavaScript version instead numbered the same sixteen keys `1` to `16` in
/// reading order, which put `V` on key `0x10` — a key that does not exist, so
/// `V` did nothing and no key produced `0x0` at all.
pub const KEY_BINDINGS: [(Keycode, u8); KEY_COUNT] = [
    (Keycode::X, 0x0),
    (Keycode::Num1, 0x1),
    (Keycode::Num2, 0x2),
    (Keycode::Num3, 0x3),
    (Keycode::Q, 0x4),
    (Keycode::W, 0x5),
    (Keycode::E, 0x6),
    (Keycode::A, 0x7),
    (Keycode::S, 0x8),
    (Keycode::D, 0x9),
    (Keycode::Z, 0xA),
    (Keycode::C, 0xB),
    (Keycode::Num4, 0xC),
    (Keycode::R, 0xD),
    (Keycode::F, 0xE),
    (Keycode::V, 0xF),
];

/// Which keys are held, plus the last one that was let go.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Keypad {
    keys: [bool; KEY_COUNT],
    released: Option<u8>,
}

impl Keypad {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The keypad key `keycode` stands for, if any.
    #[must_use]
    pub fn key_for(keycode: Keycode) -> Option<u8> {
        KEY_BINDINGS
            .iter()
            .find(|(bound, _)| *bound == keycode)
            .map(|&(_, key)| key)
    }

    /// The host key bound to keypad key `key`, if any.
    #[must_use]
    pub fn keycode_for(key: u8) -> Option<Keycode> {
        KEY_BINDINGS
            .iter()
            .find(|(_, bound)| *bound == key)
            .map(|&(keycode, _)| keycode)
    }

    /// Records a press or a release; keys outside `0x0..=0xF` are ignored.
    pub fn set_pressed(&mut self, key: u8, pressed: bool) {
        let Some(state) = self.keys.get_mut(usize::from(key)) else {
            return;
        };

        let was_pressed = *state;
        *state = pressed;

        if was_pressed && !pressed {
            self.released = Some(key);
        }
    }

    /// Whether `key` is currently held down.
    #[must_use]
    pub fn is_down(&self, key: u8) -> bool {
        self.keys.get(usize::from(key)).copied().unwrap_or(false)
    }

    /// Takes the last released key, clearing it.
    ///
    /// This is what `Fx0A` waits on.
    pub fn take_released(&mut self) -> Option<u8> {
        self.released.take()
    }

    /// Lets go of every key.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
