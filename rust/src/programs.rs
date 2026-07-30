//! The programs bundled with the interpreter.
//!
//! Most of these are the programs the JavaScript version listed in
//! `../../js/programs.txt`, converted once from its JavaScript byte arrays into
//! the `.ch8` files in `roms/`. The rest were written for this port and live as
//! assembly in `programs/`, built by `cargo run --bin asm`. Either way they are
//! compiled straight into the executable.

/// What a program is for, shown next to its name in the menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Game,
    Demo,
    Test,
}

impl Category {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Game => "GAME",
            Self::Demo => "DEMO",
            Self::Test => "TEST",
        }
    }
}

/// A program compiled into the executable.
#[derive(Debug, Clone, Copy)]
pub struct BuiltinProgram {
    pub name: &'static str,
    pub category: Category,
    pub description: &'static str,
    /// How the program is played, in terms of host keys.
    pub controls: Option<&'static str>,
    /// How fast this program wants to be run, in instructions per frame.
    pub cycles_per_frame: u32,
    pub rom: &'static [u8],
}

/// The default speed, which is what the JavaScript version ran at: ten
/// instructions per animation frame, so about 600 Hz.
pub const DEFAULT_CYCLES_PER_FRAME: u32 = 10;

/// Every bundled program, in menu order.
pub const BUILTIN_PROGRAMS: &[BuiltinProgram] = &[
    BuiltinProgram {
        name: "Leap",
        category: Category::Game,
        description: "Jump the pit in the middle of the floor. Miss it and the fall is fatal.",
        controls: Some("Q walk left, E walk right, W jump"),
        cycles_per_frame: 60,
        rom: include_bytes!("../roms/leap.ch8"),
    },
    BuiltinProgram {
        name: "Pong",
        category: Category::Game,
        description: "Two player tennis, the program the JavaScript version booted into.",
        controls: Some("Player 1: 1 up, Q down    Player 2: 4 up, R down"),
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/pong.ch8"),
    },
    BuiltinProgram {
        name: "Space Invaders",
        category: Category::Game,
        description: "Space Invaders v0.9 by David Winter.",
        controls: Some("Q move left, E move right, W fire (W also starts)"),
        cycles_per_frame: 15,
        rom: include_bytes!("../roms/space_invaders.ch8"),
    },
    BuiltinProgram {
        name: "Guess",
        category: Category::Game,
        description: "Think of a number, then say whether it is in each set shown.",
        controls: Some("W for yes, any other keypad key for no"),
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/guess.ch8"),
    },
    BuiltinProgram {
        name: "Computer",
        category: Category::Demo,
        description: "An animated drawing of a computer.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/computer.ch8"),
    },
    BuiltinProgram {
        name: "IBM Logo",
        category: Category::Demo,
        description: "Draws a logo and stops. The traditional first test of a new interpreter.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/ibm_logo.ch8"),
    },
    BuiltinProgram {
        name: "ABC 123",
        category: Category::Demo,
        description: "Writes ABC123 with the built in font, the example from programs/TUTORIAL.md.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/abc123.ch8"),
    },
    BuiltinProgram {
        name: "Next",
        category: Category::Demo,
        description: "A digit walks around the border of the screen, corner to corner.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/next.ch8"),
    },
    BuiltinProgram {
        name: "Mirror",
        category: Category::Demo,
        description: "Draws four sprites that mirror each other.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/mirror.ch8"),
    },
    BuiltinProgram {
        name: "Collide",
        category: Category::Test,
        description: "Draws overlapping sprites and shows the collision flag VF.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/collide.ch8"),
    },
    BuiltinProgram {
        name: "Branch",
        category: Category::Test,
        description: "Checks the conditional skips and the flag left by SUBN.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/branch.ch8"),
    },
    BuiltinProgram {
        name: "Compare",
        category: Category::Test,
        description: "Checks SUB and SUBN and the borrow flag each one leaves in VF.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/compare.ch8"),
    },
    BuiltinProgram {
        name: "Loop",
        category: Category::Test,
        description: "Draws columns of digits with loops that exit on a comparison.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/loop.ch8"),
    },
    BuiltinProgram {
        name: "Stack 1",
        category: Category::Test,
        description: "Saves and restores registers around nested subroutine calls.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/stack1.ch8"),
    },
    BuiltinProgram {
        name: "Stack 2",
        category: Category::Test,
        description: "Uses I as a stack pointer to push and pop registers.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/stack2.ch8"),
    },
    BuiltinProgram {
        name: "Vertical Clip",
        category: Category::Test,
        description: "Checks that sprites are clipped at the screen edges, not wrapped.",
        controls: None,
        cycles_per_frame: DEFAULT_CYCLES_PER_FRAME,
        rom: include_bytes!("../roms/vertical_clip.ch8"),
    },
];
