//! A user's search text into an FTS5 query that is never FTS5 syntax.

use crate::MIN_TERM_CHARS;

/// Whitespace splits `text`; each term of at least [`MIN_TERM_CHARS`]
/// characters becomes an FTS5 string ([`fts_string`]), the strings ANDed;
/// shorter terms are dropped. `None` when no term is left.
///
/// With the trigram tokenizer a string matches as a substring, case-folded:
/// `RULE-STAM` finds `RULE-STAM-REGEN`, `R-12` also `R-123`.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let terms: Vec<String> = text
        .split_whitespace()
        .filter(|term| term.chars().count() >= MIN_TERM_CHARS)
        .map(fts_string)
        .collect();
    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" AND "))
    }
}

/// One term as an FTS5 string: quoted, `"` doubled inside, so no term is
/// ever read as FTS5 syntax.
pub(crate) fn fts_string(term: &str) -> String {
    format!("\"{}\"", term.replace('"', "\"\""))
}
