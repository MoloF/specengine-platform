//! A light RON lexer: comments, extension attributes, literals, identifiers
//! and punctuation — enough structure to place markers, none of the value
//! semantics (those are the `ron` crate's job when values are needed).
//!
//! Never fails: an unterminated construct or a stray byte becomes a
//! [`LexError`] and lexing continues, so a broken file still yields its
//! comments and markers.

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delim {
    /// `(` `)`: struct body, tuple or unit.
    Paren,
    /// `[` `]`: list.
    Bracket,
    /// `{` `}`: map.
    Brace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    LineComment,
    BlockComment,
    /// `#![enable(...)]` extension attribute.
    Attribute,
    Ident,
    /// Plain, byte or raw string.
    Str,
    Char,
    /// Integer or float, sign included; also `inf` / `NaN` after a sign.
    Number,
    Open(Delim),
    Close(Delim),
    Comma,
    Colon,
    /// One character the lexer cannot place; always paired with a `StrayToken` error.
    Unknown,
}

impl TokenKind {
    #[must_use]
    pub fn is_comment(self) -> bool {
        matches!(self, Self::LineComment | Self::BlockComment)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LexErrorKind {
    UnterminatedString,
    UnterminatedBlockComment,
    UnterminatedChar,
    UnterminatedAttribute,
    StrayToken,
}

impl LexErrorKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnterminatedString => "unterminated_string",
            Self::UnterminatedBlockComment => "unterminated_block_comment",
            Self::UnterminatedChar => "unterminated_char",
            Self::UnterminatedAttribute => "unterminated_attribute",
            Self::StrayToken => "stray_token",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    /// Byte offset where the construct starts.
    pub offset: usize,
}

/// Tokens and lexical errors of `source`, in source order.
#[must_use]
pub fn lex(source: &str) -> (Vec<Token>, Vec<LexError>) {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut errors = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        let kind = match b {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
                continue;
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                i = memchr_newline(bytes, i);
                TokenKind::LineComment
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                match block_comment_end(bytes, i + 2) {
                    Some(end) => i = end,
                    None => {
                        errors.push(LexError {
                            kind: LexErrorKind::UnterminatedBlockComment,
                            offset: start,
                        });
                        i = bytes.len();
                    }
                }
                TokenKind::BlockComment
            }
            b'#' if bytes.get(i + 1) == Some(&b'!') && bytes.get(i + 2) == Some(&b'[') => {
                match bracket_end(bytes, i + 3) {
                    Some(end) => i = end,
                    None => {
                        errors.push(LexError {
                            kind: LexErrorKind::UnterminatedAttribute,
                            offset: start,
                        });
                        i = bytes.len();
                    }
                }
                TokenKind::Attribute
            }
            b'"' => {
                i = string_end(bytes, i + 1, &mut errors, start);
                TokenKind::Str
            }
            b'\'' => {
                i = char_end(bytes, i + 1, &mut errors, start);
                TokenKind::Char
            }
            b'(' => {
                i += 1;
                TokenKind::Open(Delim::Paren)
            }
            b')' => {
                i += 1;
                TokenKind::Close(Delim::Paren)
            }
            b'[' => {
                i += 1;
                TokenKind::Open(Delim::Bracket)
            }
            b']' => {
                i += 1;
                TokenKind::Close(Delim::Bracket)
            }
            b'{' => {
                i += 1;
                TokenKind::Open(Delim::Brace)
            }
            b'}' => {
                i += 1;
                TokenKind::Close(Delim::Brace)
            }
            b',' => {
                i += 1;
                TokenKind::Comma
            }
            b':' => {
                i += 1;
                TokenKind::Colon
            }
            b'0'..=b'9' => {
                i = number_end(bytes, i);
                TokenKind::Number
            }
            b'-' | b'+'
                if bytes
                    .get(i + 1)
                    .is_some_and(|n| n.is_ascii_digit() || *n == b'.') =>
            {
                i = number_end(bytes, i + 1);
                TokenKind::Number
            }
            b'-' | b'+' if bytes.get(i + 1).is_some_and(|n| n.is_ascii_alphabetic()) => {
                i = ident_end(source, i + 1);
                TokenKind::Number
            }
            b'.' if bytes.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                i = number_end(bytes, i + 1);
                TokenKind::Number
            }
            _ => {
                if let Some(end) = prefixed_string_end(bytes, i, &mut errors) {
                    i = end;
                    TokenKind::Str
                } else if b == b'b' && bytes.get(i + 1) == Some(&b'\'') {
                    i = char_end(bytes, i + 2, &mut errors, start);
                    TokenKind::Char
                } else if is_ident_start(source, i) {
                    i = ident_end(source, i);
                    TokenKind::Ident
                } else {
                    errors.push(LexError {
                        kind: LexErrorKind::StrayToken,
                        offset: start,
                    });
                    i += source[i..].chars().next().map_or(1, char::len_utf8);
                    TokenKind::Unknown
                }
            }
        };
        tokens.push(Token {
            kind,
            range: start..i,
        });
    }
    (tokens, errors)
}

/// Offset of the next `\n` at or after `from`, or the end of input.
fn memchr_newline(bytes: &[u8], from: usize) -> usize {
    bytes[from..]
        .iter()
        .position(|b| *b == b'\n')
        .map_or(bytes.len(), |n| from + n)
}

/// End of a block comment whose body starts at `from`; nesting honoured.
fn block_comment_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = from;
    while i + 1 < bytes.len() {
        match (bytes[i], bytes[i + 1]) {
            (b'/', b'*') => {
                depth += 1;
                i += 2;
            }
            (b'*', b'/') => {
                depth -= 1;
                i += 2;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => i += 1,
        }
    }
    None
}

/// End of an attribute whose body starts at `from` (after `#![`).
fn bracket_end(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 1;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// End of a plain string whose body starts at `from` (after the opening quote).
fn string_end(bytes: &[u8], from: usize, errors: &mut Vec<LexError>, start: usize) -> usize {
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    errors.push(LexError {
        kind: LexErrorKind::UnterminatedString,
        offset: start,
    });
    bytes.len()
}

/// End of a raw string whose body starts at `from` (after `r#*"`), closed by
/// `"` followed by `hashes` `#`.
fn raw_string_end(
    bytes: &[u8],
    from: usize,
    hashes: usize,
    errors: &mut Vec<LexError>,
    start: usize,
) -> usize {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'"'
            && bytes[i + 1..]
                .iter()
                .take(hashes)
                .filter(|b| **b == b'#')
                .count()
                == hashes
        {
            return i + 1 + hashes;
        }
        i += 1;
    }
    errors.push(LexError {
        kind: LexErrorKind::UnterminatedString,
        offset: start,
    });
    bytes.len()
}

/// `b"…"`, `r"…"`, `r#"…"#`, `br"…"`, `br#"…"#` starting at `at`; `None` when
/// `at` starts something else (an identifier such as `radius`).
fn prefixed_string_end(bytes: &[u8], at: usize, errors: &mut Vec<LexError>) -> Option<usize> {
    let mut i = at;
    if bytes.get(i) == Some(&b'b') {
        i += 1;
    }
    match bytes.get(i) {
        Some(b'"') if i > at => Some(string_end(bytes, i + 1, errors, at)),
        Some(b'r') => {
            i += 1;
            let hashes = bytes[i..].iter().take_while(|b| **b == b'#').count();
            i += hashes;
            (bytes.get(i) == Some(&b'"')).then(|| raw_string_end(bytes, i + 1, hashes, errors, at))
        }
        _ => None,
    }
}

/// End of a char literal whose body starts at `from` (after the opening quote).
fn char_end(bytes: &[u8], from: usize, errors: &mut Vec<LexError>, start: usize) -> usize {
    let mut i = from;
    if bytes.get(i) == Some(&b'\\') {
        i += 1;
        if bytes.get(i) == Some(&b'u') && bytes.get(i + 1) == Some(&b'{') {
            // A quote before the `}` means the escape never closes; stopping
            // there keeps a run of broken escapes linear, not quadratic.
            i = bytes[i..]
                .iter()
                .position(|b| *b == b'}' || *b == b'\'')
                .filter(|n| bytes[i + n] == b'}')
                .map_or(bytes.len(), |n| i + n + 1);
        } else {
            i += 1;
        }
    } else if let Some(b) = bytes.get(i) {
        i += utf8_len(*b);
    }
    if bytes.get(i) == Some(&b'\'') {
        i + 1
    } else {
        errors.push(LexError {
            kind: LexErrorKind::UnterminatedChar,
            offset: start,
        });
        from
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    }
}

/// End of a number starting at `from`: digits, letters, `_`, `.`, and a sign
/// right after an exponent marker (`1e-5`; not for `0x` literals).
fn number_end(bytes: &[u8], from: usize) -> usize {
    let hex = bytes.get(from) == Some(&b'0') && matches!(bytes.get(from + 1), Some(b'x' | b'X'));
    let mut i = from;
    while i < bytes.len() {
        let b = bytes[i];
        let sign_after_exponent =
            !hex && matches!(b, b'+' | b'-') && i > from && matches!(bytes[i - 1], b'e' | b'E');
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || sign_after_exponent {
            i += 1;
        } else {
            break;
        }
    }
    i
}

fn is_ident_start(source: &str, at: usize) -> bool {
    source[at..]
        .chars()
        .next()
        .is_some_and(|c| c == '_' || c.is_alphabetic())
}

/// End of an identifier starting at `from`; `r#raw` identifiers included.
fn ident_end(source: &str, from: usize) -> usize {
    let mut i = from;
    if source[i..].starts_with("r#") {
        i += 2;
    }
    i + source[i..]
        .char_indices()
        .find(|(_, c)| !(*c == '_' || c.is_alphanumeric()))
        .map_or(source.len() - i, |(offset, _)| offset)
}
