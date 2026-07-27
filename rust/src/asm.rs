//! A small two pass assembler for the standard CHIP-8 instruction set.
//!
//! It exists so the games in `programs/` can be kept as readable source
//! instead of as a wall of hex, and so a change to one of them is a change to
//! a text file that the test suite can re-assemble and check.
//!
//! # Syntax
//!
//! ```text
//! ; comment                    comments run to the end of the line
//! NAME = 12 + 4                constants, which may also refer to labels
//! label:                       a label, alone on a line or in front of code
//! LD V0, $1F                   one instruction per line, operands separated
//! DB $FF, 128, label           raw bytes
//! ```
//!
//! Numbers may be written as `31`, `0x1F`, `$1F` or `#1F`, and any value may
//! be a sum or difference of numbers, constants and labels.
//!
//! Only the 35 opcodes of the original CHIP-8 are accepted, so anything this
//! assembles runs on any interpreter. `SHR Vx` and `SHL Vx` assemble to `8xx6`
//! and `8xxE`, which do the same thing whether or not the interpreter has the
//! "shift in place" quirk.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::cpu::{MEMORY_SIZE, PROGRAM_START};

/// A problem with the source, reported against the line it was found on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for AsmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.message)
    }
}

impl Error for AsmError {}

fn error<T>(line: usize, message: impl Into<String>) -> Result<T, AsmError> {
    Err(AsmError {
        line,
        message: message.into(),
    })
}

/// One assembled line, kept so a listing can be printed next to the source.
#[derive(Debug, Clone)]
pub struct Line {
    pub address: u16,
    pub bytes: Vec<u8>,
    pub source: String,
}

/// The result of a successful assembly.
#[derive(Debug, Clone)]
pub struct Assembly {
    /// The bytes to write out, which load at [`PROGRAM_START`].
    pub rom: Vec<u8>,
    pub listing: Vec<Line>,
    /// Every label and constant, for tests and for error messages.
    pub symbols: HashMap<String, i32>,
}

/// A parsed line, before any value has been worked out.
struct Statement<'a> {
    line: usize,
    address: u16,
    mnemonic: String,
    operands: Vec<&'a str>,
}

impl Statement<'_> {
    fn size(&self) -> u16 {
        if self.mnemonic == "DB" {
            u16::try_from(self.operands.len()).unwrap_or(u16::MAX)
        } else {
            2
        }
    }

    fn source(&self) -> String {
        if self.operands.is_empty() {
            self.mnemonic.clone()
        } else {
            format!("{} {}", self.mnemonic, self.operands.join(", "))
        }
    }
}

fn is_identifier(token: &str) -> bool {
    let mut characters = token.chars();
    let first = characters.next();

    matches!(first, Some(first) if first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// `V0` to `VF`, in either case.
fn parse_register(token: &str) -> Option<u8> {
    let mut characters = token.chars();

    if !matches!(characters.next(), Some('V' | 'v')) {
        return None;
    }

    let digit = characters.next()?;
    if characters.next().is_some() {
        return None;
    }

    digit.to_digit(16).map(|digit| digit as u8)
}

/// `31`, `0x1F`, `$1F` or `#1F`.
fn parse_number(token: &str) -> Option<i32> {
    let (radix, digits) = match token.strip_prefix(['$', '#']) {
        Some(digits) => (16, digits),
        None => match token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
        {
            Some(digits) => (16, digits),
            None => (10, token),
        },
    };

    if digits.is_empty() {
        return None;
    }

    i32::from_str_radix(digits, radix).ok()
}

/// Split an expression into its terms and the `+` or `-` between them.
fn split_terms(expression: &str) -> Vec<&str> {
    let mut terms = Vec::new();
    let mut start = None;

    for (offset, character) in expression.char_indices() {
        let boundary = character == '+' || character == '-' || character.is_whitespace();
        if boundary {
            if let Some(term) = start.take() {
                terms.push(&expression[term..offset]);
            }
            if character != ' ' && !character.is_whitespace() {
                terms.push(&expression[offset..offset + 1]);
            }
        } else if start.is_none() {
            start = Some(offset);
        }
    }

    if let Some(term) = start {
        terms.push(&expression[term..]);
    }

    terms
}

/// Work out a sum or difference of numbers, constants and labels.
fn value(line: usize, expression: &str, symbols: &HashMap<String, i32>) -> Result<i32, AsmError> {
    let terms = split_terms(expression);
    let mut total = 0;
    let mut sign = 1;
    let mut wants_term = true;

    for term in terms {
        if term == "+" || term == "-" {
            if wants_term {
                return error(line, format!("stray {term:?} in {expression:?}"));
            }
            sign = if term == "+" { 1 } else { -1 };
            wants_term = true;
        } else {
            if !wants_term {
                return error(line, format!("missing an operator in {expression:?}"));
            }
            let Some(number) = parse_number(term).or_else(|| symbols.get(term).copied()) else {
                return error(line, format!("unknown name {term:?}"));
            };
            total += sign * number;
            wants_term = false;
        }
    }

    if wants_term {
        return error(line, format!("cannot work out {expression:?}"));
    }

    Ok(total)
}

fn fits(line: usize, what: &str, got: i32, bits: u32) -> Result<u16, AsmError> {
    let limit = 1 << bits;

    if got < 0 || got >= limit {
        return error(line, format!("{what} must fit in {bits} bits, got {got}"));
    }

    Ok(got as u16)
}

/// Split the source into statements, and note where every label landed.
fn parse(source: &str) -> Result<(Vec<Statement<'_>>, HashMap<String, i32>), AsmError> {
    let mut statements = Vec::new();
    let mut symbols: HashMap<String, i32> = HashMap::new();
    let mut constants: Vec<(usize, &str, &str)> = Vec::new();
    let mut address = PROGRAM_START;

    for (index, raw) in source.lines().enumerate() {
        let line = index + 1;
        let mut text = raw.split(';').next().unwrap_or_default().trim();

        // Any number of labels may sit in front of the code on a line.
        loop {
            let end = text.find(char::is_whitespace).unwrap_or(text.len());
            let Some(name) = text[..end].strip_suffix(':') else {
                break;
            };

            if !is_identifier(name) {
                return error(line, format!("{name:?} is not a valid label"));
            }
            if symbols
                .insert(name.to_owned(), i32::from(address))
                .is_some()
            {
                return error(line, format!("duplicate label {name:?}"));
            }

            text = text[end..].trim_start();
        }

        if text.is_empty() {
            continue;
        }

        if let Some(equals) = text.find('=') {
            let name = text[..equals].trim();
            if !is_identifier(name) {
                return error(line, format!("{name:?} is not a valid name"));
            }
            constants.push((line, name, text[equals + 1..].trim()));
            continue;
        }

        let end = text.find(char::is_whitespace).unwrap_or(text.len());
        let mnemonic = text[..end].to_ascii_uppercase();
        let rest = text[end..].trim();
        let operands: Vec<&str> = if rest.is_empty() {
            Vec::new()
        } else {
            rest.split(',').map(str::trim).collect()
        };

        if operands.iter().any(|operand| operand.is_empty()) {
            return error(line, "empty operand");
        }

        let statement = Statement {
            line,
            address,
            mnemonic,
            operands,
        };

        address += statement.size();
        if usize::from(address) > MEMORY_SIZE {
            return error(line, "the program runs past the end of memory");
        }

        statements.push(statement);
    }

    // Constants are worked out once the labels are all known, in the order
    // they were written, so one may be built out of those above it.
    for (line, name, expression) in constants {
        let resolved = value(line, expression, &symbols)?;
        if symbols.insert(name.to_owned(), resolved).is_some() {
            return error(line, format!("duplicate name {name:?}"));
        }
    }

    Ok((statements, symbols))
}

#[allow(clippy::too_many_lines)]
fn encode(statement: &Statement, symbols: &HashMap<String, i32>) -> Result<u16, AsmError> {
    let line = statement.line;
    let operands = &statement.operands;

    let count = |wanted: usize| -> Result<(), AsmError> {
        if operands.len() == wanted {
            Ok(())
        } else {
            error(
                line,
                format!(
                    "{} takes {wanted} operands, got {}",
                    statement.mnemonic,
                    operands.len()
                ),
            )
        }
    };

    let register = |index: usize| -> Result<u16, AsmError> {
        match parse_register(operands[index]) {
            Some(register) => Ok(u16::from(register)),
            None => error(
                line,
                format!("expected a register, got {:?}", operands[index]),
            ),
        }
    };

    let address = |index: usize| -> Result<u16, AsmError> {
        fits(
            line,
            "an address",
            value(line, operands[index], symbols)?,
            12,
        )
    };

    let byte = |index: usize| -> Result<u16, AsmError> {
        fits(line, "a byte", value(line, operands[index], symbols)?, 8)
    };

    let nibble = |index: usize| -> Result<u16, AsmError> {
        fits(line, "a nibble", value(line, operands[index], symbols)?, 4)
    };

    let is = |index: usize, name: &str| operands[index].eq_ignore_ascii_case(name);

    match statement.mnemonic.as_str() {
        "CLS" => {
            count(0)?;
            Ok(0x00E0)
        }
        "RET" => {
            count(0)?;
            Ok(0x00EE)
        }
        "SYS" => {
            count(1)?;
            address(0)
        }
        "JP" => {
            if operands.len() == 2 {
                if !is(0, "V0") {
                    return error(line, "an indexed jump has to be JP V0, addr");
                }
                return Ok(0xB000 | address(1)?);
            }
            count(1)?;
            Ok(0x1000 | address(0)?)
        }
        "CALL" => {
            count(1)?;
            Ok(0x2000 | address(0)?)
        }
        "SE" | "SNE" => {
            count(2)?;
            let skip_if_equal = statement.mnemonic == "SE";
            if parse_register(operands[1]).is_some() {
                let opcode = if skip_if_equal { 0x5000 } else { 0x9000 };
                Ok(opcode | register(0)? << 8 | register(1)? << 4)
            } else {
                let opcode = if skip_if_equal { 0x3000 } else { 0x4000 };
                Ok(opcode | register(0)? << 8 | byte(1)?)
            }
        }
        "LD" => {
            count(2)?;
            if is(0, "I") {
                return Ok(0xA000 | address(1)?);
            }
            for (name, opcode) in [("DT", 0xF015), ("ST", 0xF018), ("F", 0xF029), ("B", 0xF033)] {
                if is(0, name) {
                    return Ok(opcode | register(1)? << 8);
                }
            }
            if is(0, "[I]") {
                return Ok(0xF055 | register(1)? << 8);
            }
            for (name, opcode) in [("DT", 0xF007), ("K", 0xF00A), ("[I]", 0xF065)] {
                if is(1, name) {
                    return Ok(opcode | register(0)? << 8);
                }
            }
            if parse_register(operands[1]).is_some() {
                return Ok(0x8000 | register(0)? << 8 | register(1)? << 4);
            }
            Ok(0x6000 | register(0)? << 8 | byte(1)?)
        }
        "ADD" => {
            count(2)?;
            if is(0, "I") {
                return Ok(0xF01E | register(1)? << 8);
            }
            if parse_register(operands[1]).is_some() {
                return Ok(0x8004 | register(0)? << 8 | register(1)? << 4);
            }
            Ok(0x7000 | register(0)? << 8 | byte(1)?)
        }
        "OR" | "AND" | "XOR" | "SUB" | "SUBN" => {
            count(2)?;
            let low = match statement.mnemonic.as_str() {
                "OR" => 0x1,
                "AND" => 0x2,
                "XOR" => 0x3,
                "SUB" => 0x5,
                _ => 0x7,
            };
            Ok(0x8000 | register(0)? << 8 | register(1)? << 4 | low)
        }
        "SHR" | "SHL" => {
            if !matches!(operands.len(), 1 | 2) {
                return error(
                    line,
                    format!("{} takes 1 or 2 operands", statement.mnemonic),
                );
            }
            let low = if statement.mnemonic == "SHR" {
                0x6
            } else {
                0xE
            };
            let x = register(0)?;
            let y = if operands.len() == 2 { register(1)? } else { x };
            Ok(0x8000 | x << 8 | y << 4 | low)
        }
        "RND" => {
            count(2)?;
            Ok(0xC000 | register(0)? << 8 | byte(1)?)
        }
        "DRW" => {
            count(3)?;
            Ok(0xD000 | register(0)? << 8 | register(1)? << 4 | nibble(2)?)
        }
        "SKP" => {
            count(1)?;
            Ok(0xE09E | register(0)? << 8)
        }
        "SKNP" => {
            count(1)?;
            Ok(0xE0A1 | register(0)? << 8)
        }
        other => error(line, format!("unknown instruction {other:?}")),
    }
}

/// Assemble a program.
///
/// # Errors
///
/// Returns the first [`AsmError`] found, which carries the line it is on.
pub fn assemble(source: &str) -> Result<Assembly, AsmError> {
    let (statements, symbols) = parse(source)?;

    let mut rom = Vec::new();
    let mut listing = Vec::with_capacity(statements.len());

    for statement in &statements {
        let bytes = if statement.mnemonic == "DB" {
            statement
                .operands
                .iter()
                .map(|operand| {
                    let byte = value(statement.line, operand, &symbols)?;
                    Ok(fits(statement.line, "a byte", byte, 8)? as u8)
                })
                .collect::<Result<Vec<u8>, AsmError>>()?
        } else {
            encode(statement, &symbols)?.to_be_bytes().to_vec()
        };

        rom.extend_from_slice(&bytes);
        listing.push(Line {
            address: statement.address,
            bytes,
            source: statement.source(),
        });
    }

    Ok(Assembly {
        rom,
        listing,
        symbols,
    })
}

#[cfg(test)]
mod tests {
    use super::{assemble, split_terms};

    fn rom(source: &str) -> Vec<u8> {
        assemble(source).expect("it assembles").rom
    }

    fn opcode(source: &str) -> u16 {
        let bytes = rom(source);
        assert_eq!(bytes.len(), 2, "{source} did not assemble to one opcode");
        u16::from_be_bytes([bytes[0], bytes[1]])
    }

    #[test]
    fn every_opcode_has_a_mnemonic() {
        assert_eq!(opcode("CLS"), 0x00E0);
        assert_eq!(opcode("RET"), 0x00EE);
        assert_eq!(opcode("SYS $123"), 0x0123);
        assert_eq!(opcode("JP $234"), 0x1234);
        assert_eq!(opcode("CALL $345"), 0x2345);
        assert_eq!(opcode("SE V1, $22"), 0x3122);
        assert_eq!(opcode("SNE V1, $22"), 0x4122);
        assert_eq!(opcode("SE V1, V2"), 0x5120);
        assert_eq!(opcode("LD V3, $44"), 0x6344);
        assert_eq!(opcode("ADD V3, $44"), 0x7344);
        assert_eq!(opcode("LD V5, V6"), 0x8560);
        assert_eq!(opcode("OR V5, V6"), 0x8561);
        assert_eq!(opcode("AND V5, V6"), 0x8562);
        assert_eq!(opcode("XOR V5, V6"), 0x8563);
        assert_eq!(opcode("ADD V5, V6"), 0x8564);
        assert_eq!(opcode("SUB V5, V6"), 0x8565);
        assert_eq!(opcode("SHR V5, V6"), 0x8566);
        assert_eq!(opcode("SUBN V5, V6"), 0x8567);
        assert_eq!(opcode("SHL V5, V6"), 0x856E);
        assert_eq!(opcode("SNE V7, V8"), 0x9780);
        assert_eq!(opcode("LD I, $456"), 0xA456);
        assert_eq!(opcode("JP V0, $567"), 0xB567);
        assert_eq!(opcode("RND V9, $0F"), 0xC90F);
        assert_eq!(opcode("DRW VA, VB, 6"), 0xDAB6);
        assert_eq!(opcode("SKP VC"), 0xEC9E);
        assert_eq!(opcode("SKNP VC"), 0xECA1);
        assert_eq!(opcode("LD VD, DT"), 0xFD07);
        assert_eq!(opcode("LD VD, K"), 0xFD0A);
        assert_eq!(opcode("LD DT, VD"), 0xFD15);
        assert_eq!(opcode("LD ST, VD"), 0xFD18);
        assert_eq!(opcode("ADD I, VD"), 0xFD1E);
        assert_eq!(opcode("LD F, VD"), 0xFD29);
        assert_eq!(opcode("LD B, VD"), 0xFD33);
        assert_eq!(opcode("LD [I], VE"), 0xFE55);
        assert_eq!(opcode("LD VE, [I]"), 0xFE65);
    }

    #[test]
    fn a_bare_shift_uses_the_same_register_twice() {
        // 8xx6 and 8xxE mean the same thing with or without the shift quirk.
        assert_eq!(opcode("SHR V5"), 0x8556);
        assert_eq!(opcode("SHL V5"), 0x855E);
    }

    #[test]
    fn mnemonics_and_registers_are_case_insensitive() {
        assert_eq!(opcode("ld v3, dt"), 0xF307);
        assert_eq!(opcode("Drw v1, V2, 3"), 0xD123);
    }

    #[test]
    fn labels_are_addresses_and_may_be_used_before_they_are_written() {
        let bytes = rom("    JP later\n    CLS\nlater:\n    RET\n");

        assert_eq!(bytes, [0x12, 0x04, 0x00, 0xE0, 0x00, 0xEE]);
    }

    #[test]
    fn a_label_may_share_a_line_with_code() {
        let assembly = assemble("start: CLS\n").expect("it assembles");

        assert_eq!(assembly.symbols["start"], 0x200);
        assert_eq!(assembly.rom, [0x00, 0xE0]);
    }

    #[test]
    fn constants_may_be_built_from_other_constants_and_labels() {
        let assembly = assemble(
            "FIRST = 4\n\
             SECOND = FIRST + 2\n\
             AFTER = here - 1\n\
             here:\n\
             LD V0, SECOND\n",
        )
        .expect("it assembles");

        assert_eq!(assembly.symbols["SECOND"], 6);
        assert_eq!(assembly.symbols["AFTER"], 0x1FF);
        assert_eq!(assembly.rom, [0x60, 0x06]);
    }

    #[test]
    fn numbers_may_be_decimal_or_hexadecimal() {
        assert_eq!(opcode("LD V0, 255"), 0x60FF);
        assert_eq!(opcode("LD V0, 0xFF"), 0x60FF);
        assert_eq!(opcode("LD V0, $ff"), 0x60FF);
        assert_eq!(opcode("LD V0, #FF"), 0x60FF);
    }

    #[test]
    fn db_lays_down_bytes() {
        assert_eq!(rom("DB $F0, 15, 0"), [0xF0, 0x0F, 0x00]);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        assert_eq!(rom("; nothing\n\n   CLS ; clear\n"), [0x00, 0xE0]);
    }

    #[test]
    fn an_odd_number_of_bytes_still_lines_the_next_statement_up() {
        let assembly = assemble("DB 1\nhere:\nCLS\n").expect("it assembles");

        assert_eq!(assembly.symbols["here"], 0x201);
    }

    #[test]
    fn split_terms_keeps_the_operators() {
        assert_eq!(split_terms("a + b - 1"), ["a", "+", "b", "-", "1"]);
        assert_eq!(split_terms("a+b"), ["a", "+", "b"]);
        assert_eq!(split_terms("  a  "), ["a"]);
    }

    #[test]
    fn mistakes_are_reported_with_a_line_number() {
        let cases = [
            ("CLS\nFOO V0\n", "unknown instruction"),
            ("CLS\nLD V0, missing\n", "unknown name"),
            ("CLS\nLD V0, 256\n", "must fit in 8 bits"),
            ("CLS\nJP $1000\n", "must fit in 12 bits"),
            ("CLS\nDRW V0, V1\n", "takes 3 operands"),
            ("CLS\nADD V0\n", "takes 2 operands"),
            ("CLS\nLD V0, V1, V2\n", "takes 2 operands"),
            ("CLS\nSHR V0, V1, V2\n", "takes 1 or 2 operands"),
            ("CLS\nJP V1, $200\n", "indexed jump"),
            ("CLS\nSKP 4\n", "expected a register"),
            ("here:\nhere:\nCLS\n", "duplicate label"),
            ("CLS\nLD V0, 1 2\n", "missing an operator"),
            ("CLS\nLD V0, 1 +\n", "cannot work out"),
            ("CLS\nLD V0,\n", "empty operand"),
        ];

        for (source, expected) in cases {
            let error = assemble(source).expect_err(source);
            assert_eq!(error.line, 2, "{source}");
            assert!(
                error.message.contains(expected),
                "{source} gave {:?}, expected {expected:?}",
                error.message
            );
        }
    }

    #[test]
    fn a_program_cannot_run_past_the_end_of_memory() {
        let source = "CLS\n".repeat(2000);

        assert!(assemble(&source)
            .expect_err("it is too big")
            .message
            .contains("end of memory"));
    }
}
