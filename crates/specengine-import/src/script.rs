//! Scripts of identifiers (ADR-0009): look-alike normalization and the
//! Latin / mixed-script / non-Latin classification.
//!
//! "Latin" here means the ASCII letters an ID is allowed to use. Any other
//! letter or digit — a look-alike from another script, a fullwidth form, a
//! letter of another alphabet, an accented Latin letter — is *foreign*.

use std::ops::Range;

use serde::Serialize;

/// Script class of one ID occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdScript {
    /// Only ASCII letters (digits and punctuation are script-neutral).
    Latin,
    /// ASCII letters next to at least one foreign letter or digit: a Latin
    /// letter replaced by a look-alike (AC-11 of 08 §3). Autofix candidate.
    MixedScript,
    /// Foreign letters or digits and no ASCII letter: a legacy non-Latin ID,
    /// which becomes an alias (ADR-0009).
    NonLatin,
}

impl IdScript {
    /// Classifies the verbatim text of an ID.
    pub fn of(text: &str) -> Self {
        let mut ascii = false;
        let mut foreign = false;
        for c in text.chars() {
            if c.is_ascii_alphabetic() {
                ascii = true;
            } else if !c.is_ascii() && c.is_alphanumeric() {
                foreign = true;
            }
        }
        match (ascii, foreign) {
            (_, false) => Self::Latin,
            (true, true) => Self::MixedScript,
            (false, true) => Self::NonLatin,
        }
    }
}

/// Text with every look-alike replaced by its ASCII counterpart, one char for
/// one char, so a range in the normalized text maps back to the original.
pub struct Normalized {
    pub text: String,
    /// `(normalized offset, original offset)` at every char start, plus the ends.
    boundaries: Vec<(usize, usize)>,
}

impl Normalized {
    pub fn new(original: &str) -> Self {
        let mut text = String::with_capacity(original.len());
        let mut boundaries = Vec::with_capacity(original.len() + 1);
        for (offset, c) in original.char_indices() {
            boundaries.push((text.len(), offset));
            text.push(ascii_look_alike(c).unwrap_or(c));
        }
        boundaries.push((text.len(), original.len()));
        Self { text, boundaries }
    }

    /// The original byte range of a normalized range that starts and ends on
    /// char boundaries (every regex match does).
    pub fn original_range(&self, normalized: Range<usize>) -> Range<usize> {
        let find = |offset: usize| {
            self.boundaries
                .binary_search_by_key(&offset, |&(n, _)| n)
                .map(|index| self.boundaries[index].1)
                .unwrap_or(offset)
        };
        find(normalized.start)..find(normalized.end)
    }
}

/// The ASCII character a look-alike stands for; `None` when `c` is not one.
///
/// Covers fullwidth ASCII and the Cyrillic and Greek letters that are
/// visually identical to a Latin letter. Escapes only: the repository holds no
/// raw non-Latin letters (ADR-0024).
pub fn ascii_look_alike(c: char) -> Option<char> {
    let code = u32::from(c);
    if (0xFF01..=0xFF5E).contains(&code) {
        return char::from_u32(code - 0xFEE0);
    }
    let latin = match c {
        // Cyrillic capitals.
        '\u{0405}' => 'S',
        '\u{0406}' | '\u{04C0}' => 'I',
        '\u{0408}' => 'J',
        '\u{0410}' => 'A',
        '\u{0412}' => 'B',
        '\u{0415}' => 'E',
        '\u{041A}' => 'K',
        '\u{041C}' => 'M',
        '\u{041D}' => 'H',
        '\u{041E}' => 'O',
        '\u{0420}' => 'P',
        '\u{0421}' => 'C',
        '\u{0422}' => 'T',
        '\u{0423}' | '\u{04AE}' => 'Y',
        '\u{0425}' => 'X',
        '\u{051A}' => 'Q',
        '\u{051C}' => 'W',
        // Cyrillic small letters.
        '\u{0430}' => 'a',
        '\u{0435}' => 'e',
        '\u{043E}' => 'o',
        '\u{0440}' => 'p',
        '\u{0441}' => 'c',
        '\u{0443}' => 'y',
        '\u{0445}' => 'x',
        '\u{0455}' => 's',
        '\u{0456}' => 'i',
        '\u{0458}' => 'j',
        '\u{04BB}' => 'h',
        '\u{04CF}' => 'l',
        '\u{0501}' => 'd',
        '\u{051B}' => 'q',
        '\u{051D}' => 'w',
        // Greek capitals.
        '\u{0391}' => 'A',
        '\u{0392}' => 'B',
        '\u{0395}' => 'E',
        '\u{0396}' => 'Z',
        '\u{0397}' => 'H',
        '\u{0399}' => 'I',
        '\u{039A}' => 'K',
        '\u{039C}' => 'M',
        '\u{039D}' => 'N',
        '\u{039F}' => 'O',
        '\u{03A1}' => 'P',
        '\u{03A4}' => 'T',
        '\u{03A5}' => 'Y',
        '\u{03A7}' => 'X',
        // Greek small letters.
        '\u{03B9}' => 'i',
        '\u{03BD}' => 'v',
        '\u{03BF}' => 'o',
        _ => return None,
    };
    Some(latin)
}
