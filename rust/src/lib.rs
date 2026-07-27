//! A CHIP-8 interpreter, ported from the JavaScript version in the repository
//! root.
//!
//! The module layout follows the original files:
//!
//! | JavaScript    | Rust                                          |
//! |---------------|-----------------------------------------------|
//! | `chip8.js`    | [`cpu`]                                       |
//! | `keyboard.js` | [`keypad`]                                    |
//! | `video.js`    | [`video`]                                     |
//! | `audio.js`    | [`audio`]                                     |
//! | `index.html`  | `main.rs` and [`menu`]                        |
//! | `programs.txt`| [`programs`] and the `roms/` directory        |
//!
//! [`cpu`] is where the interpreter lives, and it is deliberately free of I/O:
//! it owns its memory, registers and framebuffer, and the front end reads them
//! once a frame. Everything that talks to SDL2 is in [`video`], [`audio`],
//! [`keypad`] and the binary.
//!
//! # Differences from the JavaScript version
//!
//! The port fixes the bugs the original carried:
//!
//! * `Dxyn` set `VF` from the wrong expression and recomputed it for every
//!   pixel, so collisions were reported almost every time anything was drawn.
//!   It also compared against the screen size with `>` instead of `>=`, so
//!   wrapped sprites landed one pixel outside the framebuffer.
//! * `8xy5`, `8xy7` and `8xyE` left values outside `0..=255` in a register.
//! * `Cxkk` could never produce `255`.
//! * The keypad numbered its keys `1` to `16`, so `V` mapped to a key that does
//!   not exist and no key at all produced `0x0`.
//! * The timers ran off a 15 ms interval, about 11% faster than the 60 Hz the
//!   hardware used.
//! * An opcode of `0x0000` was silently ignored, so a program that ran off the
//!   end of its code kept going instead of reporting the problem.
//!
//! See the crate README for the full list.

pub mod asm;
pub mod audio;
pub mod cpu;
pub mod font;
pub mod keypad;
pub mod menu;
pub mod programs;
pub mod theme;
pub mod video;
