//! A user's search text into an FTS5 query that is never FTS5 syntax.

use crate::MIN_TERM_CHARS;

/// Whitespace splits `text`; each term of at least [`MIN_TERM_CHARS`]
/// characters becomes an FTS5 string (`"` doubled inside), the strings
/// ANDed; shorter terms are dropped. `None` when no term is left.
///
/// With the trigram tokenizer a string matches as a substring, case-folded:
/// `RULE-STAM` finds `RULE-STAM-REGEN`, `R-12` also `R-123`.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .filter(|term| term.chars().count() >= MIN_TERM_CHARS)
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}
