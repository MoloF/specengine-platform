//! The token estimator: `ceil(Σ weight(char))`, one weight per character
//! class, conservative until calibrated against `fixtures/token-calibration/`
//! (the calibration itself is pending the owner, Q4 of the spec).
//!
//! Weights are integers in thousandths of a token, so the sum is exact and
//! the same on every machine.

/// Thousandths of a token per character of each class.
mod weight {
    /// `[A-Za-z0-9]`: English runs about four characters to a token.
    pub const ASCII_ALNUM: u64 = 270;
    /// Other visible ASCII: punctuation and Markdown markup, often a token each.
    pub const ASCII_OTHER: u64 = 500;
    /// Whitespace: mostly merged into the next word's token.
    pub const WHITESPACE: u64 = 150;
    /// Cyrillic letters: about two characters to a token.
    pub const CYRILLIC: u64 = 500;
    /// Letters of other scripts (accented Latin, Greek, CJK, …).
    pub const OTHER_LETTER: u64 = 1000;
    /// Everything else: symbols, non-ASCII punctuation, emoji.
    pub const REST: u64 = 1000;
}

/// Estimated tokens of `text`; 0 for an empty text, saturating at `u32::MAX`.
pub fn tokens_est(text: &str) -> u32 {
    let mut thousandths: u64 = 0;
    for c in text.chars() {
        thousandths = thousandths.saturating_add(weight_of(c));
    }
    u32::try_from(thousandths.div_ceil(1000)).unwrap_or(u32::MAX)
}

fn weight_of(c: char) -> u64 {
    if c.is_ascii_alphanumeric() {
        weight::ASCII_ALNUM
    } else if c.is_whitespace() {
        weight::WHITESPACE
    } else if c.is_ascii() {
        weight::ASCII_OTHER
    } else if is_cyrillic(c) {
        weight::CYRILLIC
    } else if c.is_alphabetic() {
        weight::OTHER_LETTER
    } else {
        weight::REST
    }
}

/// Cyrillic and Cyrillic Supplement letters.
fn is_cyrillic(c: char) -> bool {
    matches!(u32::from(c), 0x0400..=0x052F) && c.is_alphabetic()
}
