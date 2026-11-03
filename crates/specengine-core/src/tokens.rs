//! The token estimator: `ceil(Σ weight(char))`, one weight per character
//! class, calibrated against the tokenizer of the agents' model
//! `claude-opus-5-5` (counts of 2026-10-04, recorded in
//! `fixtures/token-calibration/reference.json`). A new tokenizer is a new
//! reference and a new index format stamp.
//!
//! Weights are integers in thousandths of a token, so the sum is exact and
//! the same on every machine. The fit minimises the worst relative error
//! over the five samples, in multiples of 50, with `WHITESPACE` held, the
//! estimated sum not below the reference sum, and `CYRILLIC` above
//! `ASCII_ALNUM`.

/// Thousandths of a token per character of each class.
mod weight {
    /// `[A-Za-z0-9]`: letters and digits run about 2.9 characters to a token.
    pub const ASCII_ALNUM: u64 = 350;
    /// Other visible ASCII: punctuation and Markdown markup are a token of
    /// their own and split the next word off its leading space.
    pub const ASCII_OTHER: u64 = 1400;
    /// Whitespace: mostly merged into the next word's token; held at its
    /// previous value by the fit.
    pub const WHITESPACE: u64 = 150;
    /// Cyrillic letters: about 2.2 characters to a token.
    pub const CYRILLIC: u64 = 450;
    /// Letters of other scripts (accented Latin, Greek, CJK, …): a token
    /// each, conservatively; no sample exercises them.
    pub const OTHER_LETTER: u64 = 1000;
    /// Everything else: symbols, non-ASCII punctuation, emoji: a token each,
    /// conservatively; no sample fits them.
    pub const REST: u64 = 1000;
}

/// Estimated tokens of `text`; 0 for an empty text, saturating at `u32::MAX`.
/// Always `tokens_of(thousandths(text))`: a caller that sums the parts'
/// [`thousandths`] and rounds once gets exactly the concatenation's estimate.
pub fn tokens_est(text: &str) -> u32 {
    tokens_of(thousandths(text))
}

/// Σ weight(char) of `text`, in thousandths of a token, saturating. Additive:
/// `thousandths(a + b) == thousandths(a).saturating_add(thousandths(b))`.
pub fn thousandths(text: &str) -> u64 {
    let mut sum: u64 = 0;
    for c in text.chars() {
        sum = sum.saturating_add(weight_of(c));
    }
    sum
}

/// Thousandths rounded up once to tokens, saturating at `u32::MAX`.
pub fn tokens_of(thousandths: u64) -> u32 {
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
