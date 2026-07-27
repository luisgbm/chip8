//! The CHIP-8 virtual machine: memory, registers, timers and the instruction
//! decoder. Ported from `chip8.js`.
//!
//! The interpreter is completely free of I/O: it owns a [`Keypad`] and a
//! monochrome framebuffer that the front end reads once a frame, and it never
//! touches SDL. That keeps the whole instruction set testable without a window.

use std::error::Error;
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::keypad::Keypad;

/// Size of the addressable memory, in bytes.
pub const MEMORY_SIZE: usize = 4096;

/// Where a program is loaded and where the program counter starts.
///
/// Everything below this address belonged to the interpreter itself on the
/// COSMAC VIP; here it holds the hexadecimal font.
pub const PROGRAM_START: u16 = 0x200;

/// Where the built-in hexadecimal font lives.
pub const FONT_START: u16 = 0x000;

/// Bytes per hexadecimal font character.
pub const FONT_CHAR_SIZE: u16 = 5;

/// Framebuffer width, in pixels.
pub const SCREEN_WIDTH: usize = 64;

/// Framebuffer height, in pixels.
pub const SCREEN_HEIGHT: usize = 32;

/// Number of pixels in the framebuffer.
pub const SCREEN_PIXELS: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

/// Number of general purpose registers, `V0` through `VF`.
pub const REGISTER_COUNT: usize = 16;

/// Maximum call depth.
pub const STACK_SIZE: usize = 16;

/// The rate at which the delay and sound timers count down.
pub const TIMER_HZ: u32 = 60;

/// The largest program that fits in memory.
pub const MAX_PROGRAM_SIZE: usize = MEMORY_SIZE - PROGRAM_START as usize;

/// Sprites for the digits `0` through `F`, five bytes each.
#[rustfmt::skip]
pub const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
    0x20, 0x60, 0x20, 0x20, 0x70, // 1
    0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
    0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
    0x90, 0x90, 0xF0, 0x10, 0x10, // 4
    0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
    0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
    0xF0, 0x10, 0x20, 0x40, 0x40, // 7
    0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
    0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
    0xF0, 0x90, 0xF0, 0x90, 0x90, // A
    0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
    0xF0, 0x80, 0x80, 0x80, 0xF0, // C
    0xE0, 0x90, 0x90, 0x90, 0xE0, // D
    0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
    0xF0, 0x80, 0xF0, 0x80, 0x80, // F
];

/// Everything that can stop the interpreter mid-program.
///
/// The JavaScript version silently ignored an opcode of `0x0000` and let bad
/// addresses read `undefined`; here a program that runs off the rails says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// The opcode at `pc` is not part of the instruction set.
    UnknownOpcode { pc: u16, opcode: u16 },
    /// `CALL` was executed with a full stack.
    StackOverflow { pc: u16 },
    /// `RET` was executed with an empty stack.
    StackUnderflow { pc: u16 },
    /// An instruction touched memory outside of the 4 KiB address space.
    AddressOutOfRange { pc: u16, address: u32 },
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownOpcode { pc, opcode } => {
                write!(f, "unknown opcode {opcode:#06X} at {pc:#05X}")
            }
            Self::StackOverflow { pc } => write!(f, "stack overflow at {pc:#05X}"),
            Self::StackUnderflow { pc } => write!(f, "stack underflow at {pc:#05X}"),
            Self::AddressOutOfRange { pc, address } => {
                write!(f, "address {address:#X} out of range at {pc:#05X}")
            }
        }
    }
}

impl Error for Fault {}

impl Fault {
    /// A plain-language guess at what went wrong, for the front end to show.
    #[must_use]
    pub fn hint(self) -> &'static str {
        match self {
            // Memory starts out zeroed, so this is what a program that runs
            // past its own last instruction hits.
            Self::UnknownOpcode { opcode: 0x0000, .. } => {
                "the program most likely ran past its last instruction"
            }
            Self::UnknownOpcode { .. } => "this program may expect an extended CHIP-8",
            Self::StackOverflow { .. } => "more than 16 nested subroutine calls",
            Self::StackUnderflow { .. } => "a return without a matching call",
            Self::AddressOutOfRange { .. } => "an instruction reached past the end of memory",
        }
    }
}

/// Why a program could not be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgramTooLarge {
    pub size: usize,
}

impl fmt::Display for ProgramTooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "program is {} bytes, but only {MAX_PROGRAM_SIZE} bytes fit in memory",
            self.size
        )
    }
}

impl Error for ProgramTooLarge {}

/// The handful of behaviours interpreters disagree on.
///
/// The defaults follow what the majority of programs written for the original
/// CHIP-8 expect, which is also what the JavaScript version did — except for
/// [`Quirks::clip_sprites`], where the JavaScript version *meant* to wrap but
/// got the comparison wrong and corrupted the framebuffer instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quirks {
    /// Clip sprites at the screen edges instead of wrapping them around.
    ///
    /// The starting position is always taken modulo the screen size; this only
    /// controls what happens to the rest of the sprite.
    pub clip_sprites: bool,
    /// `8xy6`/`8xyE` shift `Vx` in place rather than shifting `Vy` into `Vx`.
    pub shift_in_place: bool,
    /// `Bnnn` jumps to `nnn + Vx` rather than `nnn + V0`.
    pub jump_with_vx: bool,
    /// `Fx55`/`Fx65` leave `I` pointing past the bytes they touched.
    pub increment_i_on_load_store: bool,
    /// `8xy1`/`8xy2`/`8xy3` clear `VF`, as the COSMAC VIP did.
    pub reset_vf_on_logic: bool,
}

impl Default for Quirks {
    fn default() -> Self {
        Self {
            clip_sprites: true,
            shift_in_place: true,
            jump_with_vx: false,
            increment_i_on_load_store: false,
            reset_vf_on_logic: false,
        }
    }
}

/// A CHIP-8 interpreter.
pub struct Chip8 {
    memory: [u8; MEMORY_SIZE],
    v: [u8; REGISTER_COUNT],
    i: u16,
    pc: u16,
    sp: usize,
    stack: [u16; STACK_SIZE],
    delay_timer: u8,
    sound_timer: u8,
    framebuffer: [bool; SCREEN_PIXELS],
    keypad: Keypad,
    quirks: Quirks,
    /// Set by `Fx0A` while it waits for a key, cleared once one arrives.
    awaiting_key: bool,
    /// Set by any instruction that touched the framebuffer.
    redraw: bool,
    /// The bytes handed to [`Chip8::load`], kept so [`Chip8::reset`] can reload.
    program: Vec<u8>,
    rng: u64,
}

impl Default for Chip8 {
    fn default() -> Self {
        Self::new()
    }
}

impl Chip8 {
    /// A machine with no program loaded and a clock-seeded random generator.
    #[must_use]
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0x2BAD_C0DE_DEAD_BEEF, |elapsed| elapsed.as_nanos() as u64);

        Self::with_seed(seed)
    }

    /// A machine whose `Cxkk` sequence is reproducible, for tests.
    #[must_use]
    pub fn with_seed(seed: u64) -> Self {
        let mut chip8 = Self {
            memory: [0; MEMORY_SIZE],
            v: [0; REGISTER_COUNT],
            i: 0,
            pc: PROGRAM_START,
            sp: 0,
            stack: [0; STACK_SIZE],
            delay_timer: 0,
            sound_timer: 0,
            framebuffer: [false; SCREEN_PIXELS],
            keypad: Keypad::new(),
            quirks: Quirks::default(),
            awaiting_key: false,
            redraw: true,
            program: Vec::new(),
            // xorshift stalls on a seed of zero.
            rng: seed | 1,
        };

        chip8.reset();
        chip8
    }

    /// Clears the machine and reloads the program passed to [`Chip8::load`].
    ///
    /// The random generator keeps its state so a reset does not replay the same
    /// sequence of `Cxkk` values.
    pub fn reset(&mut self) {
        self.memory = [0; MEMORY_SIZE];
        self.memory[..FONT.len()].copy_from_slice(&FONT);

        self.v = [0; REGISTER_COUNT];
        self.i = 0;
        self.pc = PROGRAM_START;
        self.sp = 0;
        self.stack = [0; STACK_SIZE];
        self.delay_timer = 0;
        self.sound_timer = 0;
        self.framebuffer = [false; SCREEN_PIXELS];
        self.keypad.reset();
        self.awaiting_key = false;
        self.redraw = true;

        let start = PROGRAM_START as usize;
        let end = start + self.program.len();
        self.memory[start..end].copy_from_slice(&self.program);
    }

    /// Resets the machine and loads `program` at [`PROGRAM_START`].
    ///
    /// # Errors
    ///
    /// Returns [`ProgramTooLarge`] when the program does not fit in the
    /// [`MAX_PROGRAM_SIZE`] bytes above [`PROGRAM_START`].
    pub fn load(&mut self, program: &[u8]) -> Result<(), ProgramTooLarge> {
        if program.len() > MAX_PROGRAM_SIZE {
            return Err(ProgramTooLarge {
                size: program.len(),
            });
        }

        self.program.clear();
        self.program.extend_from_slice(program);
        self.reset();

        Ok(())
    }

    /// The bytes of the loaded program.
    #[must_use]
    pub fn program(&self) -> &[u8] {
        &self.program
    }

    #[must_use]
    pub fn quirks(&self) -> Quirks {
        self.quirks
    }

    pub fn set_quirks(&mut self, quirks: Quirks) {
        self.quirks = quirks;
    }

    #[must_use]
    pub fn keypad(&self) -> &Keypad {
        &self.keypad
    }

    #[must_use]
    pub fn keypad_mut(&mut self) -> &mut Keypad {
        &mut self.keypad
    }

    /// The framebuffer, row-major, one entry per pixel.
    #[must_use]
    pub fn framebuffer(&self) -> &[bool; SCREEN_PIXELS] {
        &self.framebuffer
    }

    /// Whether the pixel at `(x, y)` is lit; `false` outside the screen.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> bool {
        if x >= SCREEN_WIDTH || y >= SCREEN_HEIGHT {
            return false;
        }

        self.framebuffer[y * SCREEN_WIDTH + x]
    }

    /// Reports whether the framebuffer changed since the last call, and clears
    /// the flag.
    pub fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }

    #[must_use]
    pub fn pc(&self) -> u16 {
        self.pc
    }

    #[must_use]
    pub fn index(&self) -> u16 {
        self.i
    }

    pub fn set_index(&mut self, value: u16) {
        self.i = value & 0x0FFF;
    }

    /// The value of `Vx`; `x` is masked to a nibble.
    #[must_use]
    pub fn register(&self, x: u8) -> u8 {
        self.v[usize::from(x & 0xF)]
    }

    pub fn set_register(&mut self, x: u8, value: u8) {
        self.v[usize::from(x & 0xF)] = value;
    }

    #[must_use]
    pub fn delay_timer(&self) -> u8 {
        self.delay_timer
    }

    #[must_use]
    pub fn sound_timer(&self) -> u8 {
        self.sound_timer
    }

    /// Whether the buzzer should be sounding.
    #[must_use]
    pub fn is_beeping(&self) -> bool {
        self.sound_timer > 0
    }

    /// Whether the machine is parked on an `Fx0A` waiting for a key.
    #[must_use]
    pub fn is_awaiting_key(&self) -> bool {
        self.awaiting_key
    }

    #[must_use]
    pub fn memory(&self) -> &[u8; MEMORY_SIZE] {
        &self.memory
    }

    /// Writes a byte straight into memory, ignoring addresses out of range.
    pub fn write_memory(&mut self, address: u16, value: u8) {
        if let Some(cell) = self.memory.get_mut(usize::from(address)) {
            *cell = value;
        }
    }

    /// Counts both timers down one tick; call this at [`TIMER_HZ`].
    ///
    /// The JavaScript version drove this from a 15 ms `setInterval`, which runs
    /// about 11% fast; the front end now ticks it off the frame clock instead.
    pub fn tick_timers(&mut self) {
        self.delay_timer = self.delay_timer.saturating_sub(1);
        self.sound_timer = self.sound_timer.saturating_sub(1);
    }

    /// Runs `cycles` instructions, stopping early on the first fault.
    ///
    /// # Errors
    ///
    /// Returns the [`Fault`] that stopped the program; the instructions before
    /// it have already been executed.
    pub fn step_many(&mut self, cycles: u32) -> Result<(), Fault> {
        for _ in 0..cycles {
            self.step()?;
        }

        Ok(())
    }

    /// Fetches, decodes and executes a single instruction.
    ///
    /// # Errors
    ///
    /// Returns a [`Fault`] when the instruction is not part of the instruction
    /// set, unbalances the stack, or reaches outside of memory.
    pub fn step(&mut self) -> Result<(), Fault> {
        let pc = self.pc;
        let high = self.read(pc, pc)?;
        let low = self.read(pc.wrapping_add(1), pc)?;
        let opcode = u16::from(high) << 8 | u16::from(low);

        self.pc = pc.wrapping_add(2) & 0x0FFF;
        self.execute(opcode, pc)
    }

    fn execute(&mut self, opcode: u16, pc: u16) -> Result<(), Fault> {
        // nnn: the lowest 12 bits, an address.
        let nnn = opcode & 0x0FFF;
        // kk: the lowest 8 bits, a constant.
        let kk = (opcode & 0x00FF) as u8;
        // n: the lowest 4 bits, a nibble.
        let n = (opcode & 0x000F) as u8;
        // x: the low nibble of the high byte, a register.
        let x = ((opcode & 0x0F00) >> 8) as usize;
        // y: the high nibble of the low byte, a register.
        let y = ((opcode & 0x00F0) >> 4) as usize;
        // u: the high nibble, the opcode group.
        let u = ((opcode & 0xF000) >> 12) as u8;

        match (u, x, y, n) {
            // 00E0 - CLS - Clear the display.
            (0x0, 0x0, 0xE, 0x0) => {
                self.framebuffer = [false; SCREEN_PIXELS];
                self.redraw = true;
            }
            // 00EE - RET - Return from a subroutine.
            (0x0, 0x0, 0xE, 0xE) => {
                self.sp = self.sp.checked_sub(1).ok_or(Fault::StackUnderflow { pc })?;
                self.pc = self.stack[self.sp];
            }
            // 1nnn - JP addr - Jump to nnn.
            (0x1, ..) => self.pc = nnn,
            // 2nnn - CALL addr - Call the subroutine at nnn.
            (0x2, ..) => {
                if self.sp >= STACK_SIZE {
                    return Err(Fault::StackOverflow { pc });
                }

                self.stack[self.sp] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            // 3xkk - SE Vx, byte - Skip the next instruction if Vx == kk.
            (0x3, ..) => self.skip_if(self.v[x] == kk),
            // 4xkk - SNE Vx, byte - Skip the next instruction if Vx != kk.
            (0x4, ..) => self.skip_if(self.v[x] != kk),
            // 5xy0 - SE Vx, Vy - Skip the next instruction if Vx == Vy.
            (0x5, _, _, 0x0) => self.skip_if(self.v[x] == self.v[y]),
            // 6xkk - LD Vx, byte - Set Vx = kk.
            (0x6, ..) => self.v[x] = kk,
            // 7xkk - ADD Vx, byte - Set Vx = Vx + kk, without touching VF.
            (0x7, ..) => self.v[x] = self.v[x].wrapping_add(kk),
            // 8xy0 - LD Vx, Vy - Set Vx = Vy.
            (0x8, _, _, 0x0) => self.v[x] = self.v[y],
            // 8xy1 - OR Vx, Vy - Set Vx = Vx OR Vy.
            (0x8, _, _, 0x1) => {
                self.v[x] |= self.v[y];
                self.reset_vf_on_logic();
            }
            // 8xy2 - AND Vx, Vy - Set Vx = Vx AND Vy.
            (0x8, _, _, 0x2) => {
                self.v[x] &= self.v[y];
                self.reset_vf_on_logic();
            }
            // 8xy3 - XOR Vx, Vy - Set Vx = Vx XOR Vy.
            (0x8, _, _, 0x3) => {
                self.v[x] ^= self.v[y];
                self.reset_vf_on_logic();
            }
            // 8xy4 - ADD Vx, Vy - Set Vx = Vx + Vy, VF = carry.
            (0x8, _, _, 0x4) => {
                let (sum, carry) = self.v[x].overflowing_add(self.v[y]);
                self.set_result(x, sum, u8::from(carry));
            }
            // 8xy5 - SUB Vx, Vy - Set Vx = Vx - Vy, VF = NOT borrow.
            (0x8, _, _, 0x5) => {
                let (difference, borrow) = self.v[x].overflowing_sub(self.v[y]);
                self.set_result(x, difference, u8::from(!borrow));
            }
            // 8xy6 - SHR Vx {, Vy} - Set Vx = Vx SHR 1, VF = the lost bit.
            (0x8, _, _, 0x6) => {
                let source = if self.quirks.shift_in_place {
                    self.v[x]
                } else {
                    self.v[y]
                };
                self.set_result(x, source >> 1, source & 0x1);
            }
            // 8xy7 - SUBN Vx, Vy - Set Vx = Vy - Vx, VF = NOT borrow.
            (0x8, _, _, 0x7) => {
                let (difference, borrow) = self.v[y].overflowing_sub(self.v[x]);
                self.set_result(x, difference, u8::from(!borrow));
            }
            // 8xyE - SHL Vx {, Vy} - Set Vx = Vx SHL 1, VF = the lost bit.
            (0x8, _, _, 0xE) => {
                let source = if self.quirks.shift_in_place {
                    self.v[x]
                } else {
                    self.v[y]
                };
                self.set_result(x, source << 1, source >> 7);
            }
            // 9xy0 - SNE Vx, Vy - Skip the next instruction if Vx != Vy.
            (0x9, _, _, 0x0) => self.skip_if(self.v[x] != self.v[y]),
            // Annn - LD I, addr - Set I = nnn.
            (0xA, ..) => self.i = nnn,
            // Bnnn - JP V0, addr - Jump to nnn + V0.
            (0xB, ..) => {
                let offset = if self.quirks.jump_with_vx {
                    self.v[x]
                } else {
                    self.v[0]
                };
                self.pc = nnn.wrapping_add(u16::from(offset)) & 0x0FFF;
            }
            // Cxkk - RND Vx, byte - Set Vx = a random byte AND kk.
            (0xC, ..) => self.v[x] = self.next_random() & kk,
            // Dxyn - DRW Vx, Vy, nibble - Draw an n-byte sprite at (Vx, Vy).
            (0xD, ..) => self.draw_sprite(self.v[x], self.v[y], n, pc)?,
            // Ex9E - SKP Vx - Skip the next instruction if key Vx is down.
            (0xE, _, 0x9, 0xE) => self.skip_if(self.keypad.is_down(self.v[x])),
            // ExA1 - SKNP Vx - Skip the next instruction if key Vx is up.
            (0xE, _, 0xA, 0x1) => self.skip_if(!self.keypad.is_down(self.v[x])),
            // Fx07 - LD Vx, DT - Set Vx = the delay timer.
            (0xF, _, 0x0, 0x7) => self.v[x] = self.delay_timer,
            // Fx0A - LD Vx, K - Wait for a key, then store it in Vx.
            (0xF, _, 0x0, 0xA) => self.wait_for_key(x, pc),
            // Fx15 - LD DT, Vx - Set the delay timer = Vx.
            (0xF, _, 0x1, 0x5) => self.delay_timer = self.v[x],
            // Fx18 - LD ST, Vx - Set the sound timer = Vx.
            (0xF, _, 0x1, 0x8) => self.sound_timer = self.v[x],
            // Fx1E - ADD I, Vx - Set I = I + Vx.
            (0xF, _, 0x1, 0xE) => self.i = self.i.wrapping_add(u16::from(self.v[x])) & 0x0FFF,
            // Fx29 - LD F, Vx - Set I to the sprite for the digit in Vx.
            (0xF, _, 0x2, 0x9) => {
                self.i = FONT_START + u16::from(self.v[x] & 0xF) * FONT_CHAR_SIZE;
            }
            // Fx33 - LD B, Vx - Store the BCD of Vx at I, I+1 and I+2.
            (0xF, _, 0x3, 0x3) => {
                let value = self.v[x];
                self.write(self.i, value / 100, pc)?;
                self.write(self.i.wrapping_add(1), value / 10 % 10, pc)?;
                self.write(self.i.wrapping_add(2), value % 10, pc)?;
            }
            // Fx55 - LD [I], Vx - Store V0 through Vx at I.
            (0xF, _, 0x5, 0x5) => {
                for offset in 0..=x {
                    let value = self.v[offset];
                    self.write(self.i.wrapping_add(offset as u16), value, pc)?;
                }

                self.advance_index_after_load_store(x);
            }
            // Fx65 - LD Vx, [I] - Read V0 through Vx from I.
            (0xF, _, 0x6, 0x5) => {
                for offset in 0..=x {
                    self.v[offset] = self.read(self.i.wrapping_add(offset as u16), pc)?;
                }

                self.advance_index_after_load_store(x);
            }
            _ => return Err(Fault::UnknownOpcode { pc, opcode }),
        }

        Ok(())
    }

    /// Skips the next instruction when `condition` holds.
    fn skip_if(&mut self, condition: bool) {
        if condition {
            self.pc = self.pc.wrapping_add(2) & 0x0FFF;
        }
    }

    /// Stores the result of an arithmetic instruction and then its flag.
    ///
    /// The order matters: when `x` is `0xF` the flag has to win, which is what
    /// programs like the bundled `compare` test rely on.
    fn set_result(&mut self, x: usize, value: u8, flag: u8) {
        self.v[x] = value;
        self.v[0xF] = flag;
    }

    fn reset_vf_on_logic(&mut self) {
        if self.quirks.reset_vf_on_logic {
            self.v[0xF] = 0;
        }
    }

    fn advance_index_after_load_store(&mut self, x: usize) {
        if self.quirks.increment_i_on_load_store {
            self.i = self.i.wrapping_add(x as u16 + 1) & 0x0FFF;
        }
    }

    /// `Fx0A`: parks the program counter on this instruction until a key that
    /// was pressed while waiting is let go again.
    ///
    /// Waiting for the release, rather than firing on the press as the
    /// JavaScript version's callback did, stops a single keystroke from
    /// satisfying this instruction and immediately triggering an `Ex9E` further
    /// down the program.
    fn wait_for_key(&mut self, x: usize, pc: u16) {
        if !self.awaiting_key {
            self.awaiting_key = true;
            // Ignore keys that were let go before the program started waiting.
            self.keypad.take_released();
        }

        match self.keypad.take_released() {
            Some(key) => {
                self.v[x] = key;
                self.awaiting_key = false;
            }
            // Nothing yet: run this instruction again on the next cycle.
            None => self.pc = pc,
        }
    }

    /// `Dxyn`: XORs an `n`-row sprite onto the framebuffer at `(x, y)`.
    ///
    /// `VF` is set once, after the whole sprite has been drawn, and only if a
    /// lit pixel was turned off. The JavaScript version recomputed the flag for
    /// every pixel — including the transparent ones — so it reported collisions
    /// almost every time it drew anything.
    fn draw_sprite(&mut self, x: u8, y: u8, rows: u8, pc: u16) -> Result<(), Fault> {
        // The starting position always wraps, even when sprites are clipped.
        let origin_x = usize::from(x) % SCREEN_WIDTH;
        let origin_y = usize::from(y) % SCREEN_HEIGHT;
        let mut collision = false;

        for row in 0..usize::from(rows) {
            let Some(pixel_y) = self.wrap_or_clip(origin_y + row, SCREEN_HEIGHT) else {
                break;
            };

            let bits = self.read(self.i.wrapping_add(row as u16), pc)?;

            for column in 0..8 {
                if bits & (0x80 >> column) == 0 {
                    continue;
                }

                let Some(pixel_x) = self.wrap_or_clip(origin_x + column, SCREEN_WIDTH) else {
                    break;
                };

                let pixel = &mut self.framebuffer[pixel_y * SCREEN_WIDTH + pixel_x];
                collision |= *pixel;
                *pixel = !*pixel;
            }
        }

        self.v[0xF] = u8::from(collision);
        self.redraw = true;

        Ok(())
    }

    /// Maps a sprite coordinate onto the screen, or `None` when it falls off a
    /// clipping edge.
    fn wrap_or_clip(&self, coordinate: usize, limit: usize) -> Option<usize> {
        if coordinate < limit {
            Some(coordinate)
        } else if self.quirks.clip_sprites {
            None
        } else {
            Some(coordinate % limit)
        }
    }

    fn read(&self, address: u16, pc: u16) -> Result<u8, Fault> {
        self.memory
            .get(usize::from(address))
            .copied()
            .ok_or(Fault::AddressOutOfRange {
                pc,
                address: u32::from(address),
            })
    }

    fn write(&mut self, address: u16, value: u8, pc: u16) -> Result<(), Fault> {
        match self.memory.get_mut(usize::from(address)) {
            Some(cell) => {
                *cell = value;
                Ok(())
            }
            None => Err(Fault::AddressOutOfRange {
                pc,
                address: u32::from(address),
            }),
        }
    }

    /// xorshift64*, which is plenty for `Cxkk` and keeps the crate free of a
    /// random number dependency.
    fn next_random(&mut self) -> u8 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;

        // The high bits are the well-mixed ones.
        (self.rng >> 56) as u8
    }
}
