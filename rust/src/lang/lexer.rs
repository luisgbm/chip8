//! Turns C8 source into a flat list of tokens.

use std::fmt;

/// A problem with the source, reported against the line it was found on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub line: usize,
    pub message: String,
}

/// One token, and the line it came from so errors can point at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// A name: a variable, a function, a label or a keyword.
    Name(String),
    /// A number, already parsed. Kept wide so `64 - 5` can be worked out
    /// before it has to fit in a byte.
    Number(i32),
    /// A register written out in full, as in `@ V2`.
    Register(u8),
    Symbol(&'static str),
    End,
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(name) => write!(formatter, "`{name}`"),
            Self::Number(value) => write!(formatter, "`{value}`"),
            Self::Register(index) => write!(formatter, "`V{index:X}`"),
            Self::Symbol(symbol) => write!(formatter, "`{symbol}`"),
            Self::End => formatter.write_str("end of file"),
        }
    }
}

/// The symbols, longest first so that `>>=` is preferred over `>>`, and `>>`
/// over `>`.
const SYMBOLS: &[&str] = &[
    ">>=", "<<=", "==", "!=", "<=", ">=", "+=", "-=", "&=", "|=", "^=", ">>", "<<", "=", "+", "-",
    "&", "|", "^", "<", ">", "!", "(", ")", "{", "}", "[", "]", ",", ";", ":", "@",
];

/// # Errors
///
/// Returns the line and a description of the first thing that is not a token.
pub fn tokenize(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < bytes.len() {
        let byte = bytes[index];

        if byte == b'\n' {
            line += 1;
            index += 1;
            continue;
        }

        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }

        // `//` runs to the end of the line, `/* */` may span lines.
        if source[index..].starts_with("//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }

        if source[index..].starts_with("/*") {
            let start = line;
            index += 2;

            loop {
                if index >= bytes.len() {
                    return Err(LexError {
                        line: start,
                        message: "unterminated `/*` comment".into(),
                    });
                }
                if source[index..].starts_with("*/") {
                    index += 2;
                    break;
                }
                if bytes[index] == b'\n' {
                    line += 1;
                }
                index += 1;
            }

            continue;
        }

        if byte.is_ascii_digit() || byte == b'$' || byte == b'#' {
            let (value, length) = number(&source[index..], line)?;
            tokens.push(Token {
                kind: TokenKind::Number(value),
                line,
            });
            index += length;
            continue;
        }

        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }

            let word = &source[start..index];
            let kind = register(word)
                .map_or_else(|| TokenKind::Name(word.to_owned()), TokenKind::Register);

            tokens.push(Token { kind, line });
            continue;
        }

        let Some(symbol) = SYMBOLS
            .iter()
            .find(|symbol| source[index..].starts_with(**symbol))
        else {
            return Err(LexError {
                line,
                message: format!("stray `{}` in the source", byte as char),
            });
        };

        tokens.push(Token {
            kind: TokenKind::Symbol(symbol),
            line,
        });
        index += symbol.len();
    }

    tokens.push(Token {
        kind: TokenKind::End,
        line,
    });

    Ok(tokens)
}

/// `V0` to `VF`, upper case only, so that a variable called `vy` is still a
/// variable.
fn register(word: &str) -> Option<u8> {
    let digit = word.strip_prefix('V')?;
    let mut characters = digit.chars();
    let digit = characters.next()?;

    if characters.next().is_some() || !digit.is_ascii_uppercase() && !digit.is_ascii_digit() {
        return None;
    }

    digit.to_digit(16).map(|digit| digit as u8)
}

/// Reads `31`, `0x1F`, `$1F`, `#1F` or `0b11111`, returning the value and how
/// many bytes of source it took.
fn number(text: &str, line: usize) -> Result<(i32, usize), LexError> {
    let (radix, rest, prefix) = if let Some(rest) = text.strip_prefix("0x") {
        (16, rest, 2)
    } else if let Some(rest) = text.strip_prefix("0b") {
        (2, rest, 2)
    } else if let Some(rest) = text.strip_prefix(['$', '#']) {
        (16, rest, 1)
    } else {
        (10, text, 0)
    };

    let length = rest
        .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .unwrap_or(rest.len());
    let digits = rest[..length].replace('_', "");

    if digits.is_empty() {
        return Err(LexError {
            line,
            message: format!("`{}` is not a number", &text[..prefix]),
        });
    }

    i32::from_str_radix(&digits, radix)
        .map(|value| (value, prefix + length))
        .map_err(|_| LexError {
            line,
            message: format!("`{digits}` is not a base {radix} number"),
        })
}

#[cfg(test)]
mod tests {
    use super::{tokenize, TokenKind};

    fn kinds(source: &str) -> Vec<TokenKind> {
        let mut kinds: Vec<TokenKind> = tokenize(source)
            .expect("it lexes")
            .into_iter()
            .map(|token| token.kind)
            .collect();

        kinds.pop();
        kinds
    }

    #[test]
    fn numbers_can_be_written_five_ways() {
        assert_eq!(
            kinds("31 0x1F $1F #1F 0b11111"),
            vec![TokenKind::Number(31); 5]
        );
    }

    #[test]
    fn registers_are_upper_case_so_variables_are_still_variables() {
        assert_eq!(
            kinds("V2 VF"),
            vec![TokenKind::Register(2), TokenKind::Register(15)]
        );
        assert_eq!(kinds("vy"), vec![TokenKind::Name("vy".into())]);
        assert_eq!(kinds("V20"), vec![TokenKind::Name("V20".into())]);
    }

    #[test]
    fn the_longest_symbol_wins() {
        assert_eq!(
            kinds(">>= >> >"),
            vec![
                TokenKind::Symbol(">>="),
                TokenKind::Symbol(">>"),
                TokenKind::Symbol(">"),
            ]
        );
    }

    #[test]
    fn comments_are_skipped_but_their_lines_are_still_counted() {
        let tokens = tokenize("// one\n/* two\nthree */ x").expect("it lexes");

        assert_eq!(tokens[0].kind, TokenKind::Name("x".into()));
        assert_eq!(tokens[0].line, 3);
    }

    #[test]
    fn an_unterminated_comment_is_reported_where_it_started() {
        let error = tokenize("\n\n/* on and on").expect_err("it should not lex");

        assert_eq!(error.line, 3);
        assert!(error.message.contains("unterminated"), "{error:?}");
    }

    #[test]
    fn a_stray_character_is_an_error() {
        let error = tokenize("x = 1 ? 2").expect_err("it should not lex");

        assert!(error.message.contains('?'), "{error:?}");
        assert_eq!(error.line, 1);
    }
}
