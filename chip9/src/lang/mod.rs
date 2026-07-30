//! C9: a small C-like language that compiles to CHIP-9.
//!
//! The assembler in [`crate::asm`] made programs writable; this makes them
//! readable. A C9 program is compiled to assembly text and then handed to that
//! assembler, so the two share one instruction encoder and one set of tests.
//!
//! ```text
//! const HOLE_X = 28;
//!
//! var x @ V2, state @ V6;
//!
//! sprite player = { $70, $70, $F8, $70, $50, $50 };
//!
//! fn main() {
//!     clear();
//!     x = 4;
//!     loop {
//!         if (pressed(6)) x += 1;
//!         draw(x, 20, player);
//!     }
//! }
//! ```
//!
//! # What the machine can do, and so what the language can do
//!
//! There is no multiplication, no division, no stack for values and no memory
//! to spill into, so there are no types, no locals beyond the sixteen
//! registers, and functions take no arguments. What is left is close to C:
//! expressions, `if`/`else`, `while`, `do`/`while`, `loop`, `goto` and
//! functions.
//!
//! See `programs/LANGUAGE.md` for the tutorial.

pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;

use std::error::Error;
use std::fmt;

use crate::asm::{assemble, Assembly};

/// A problem with the source, reported against the line it was found on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl Error for CompileError {}

impl From<lexer::LexError> for CompileError {
    fn from(error: lexer::LexError) -> Self {
        Self {
            line: error.line,
            message: error.message,
        }
    }
}

/// A compiled program: the assembly the compiler wrote, and what the assembler
/// made of it.
#[derive(Debug, Clone)]
pub struct Compiled {
    pub assembly: String,
    pub rom: Vec<u8>,
}

/// Compiles C9 source all the way to a ROM.
///
/// # Errors
///
/// Returns the line and a description of the first thing that is wrong. A
/// failure from the assembler means the compiler emitted something it should
/// not have, so it is reported as coming from the generated assembly.
pub fn compile(source: &str) -> Result<Compiled, CompileError> {
    let assembly = compile_to_assembly(source)?;
    let assembled = assemble_generated(&assembly)?;

    Ok(Compiled {
        assembly,
        rom: assembled.rom,
    })
}

/// Compiles C9 source as far as assembly text.
///
/// # Errors
///
/// Returns the line and a description of the first thing that is wrong.
pub fn compile_to_assembly(source: &str) -> Result<String, CompileError> {
    let tokens = lexer::tokenize(source)?;
    let program = parser::Parser::new(tokens).program()?;

    codegen::Compiler::new().compile(&program)
}

fn assemble_generated(assembly: &str) -> Result<Assembly, CompileError> {
    assemble(assembly).map_err(|error| {
        let line = assembly
            .lines()
            .nth(error.line.saturating_sub(1))
            .unwrap_or_default()
            .trim();

        CompileError {
            line: error.line,
            message: format!(
                "the compiler produced assembly that does not assemble: {} (`{line}`)",
                error.message
            ),
        }
    })
}
