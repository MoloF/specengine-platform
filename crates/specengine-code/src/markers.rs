//! Marker syntax shared by `.rs` and `.ron` comments (05 §5.3, ADR-0010, ADR-0018):
//! `@implements ID@rev`, `@verifies ID`, `@configures ID@rev`, `@assumes ID`,
//! any number per comment, an optional free-text note after the ID.
//!
//! Only the syntax lives here; what a marker attaches to is the business of
//! the language walker (`items` for Rust, `ron` for RON).

/// The relation a marker declares between code and a spec node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Relation {
    Implements,
    Verifies,
    Configures,
    Assumes,
}

impl Relation {
    /// The keyword after `@`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implements => "implements",
            Self::Verifies => "verifies",
            Self::Configures => "configures",
            Self::Assumes => "assumes",
        }
    }

    const ALL: [Self; 4] = [
        Self::Implements,
        Self::Verifies,
        Self::Configures,
        Self::Assumes,
    ];
}

/// One marker as written in a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub relation: Relation,
    /// The ID verbatim; may be empty when the keyword ends the line.
    pub id: String,
    /// `@rev` after the ID (ADR-0018); `None` for a weak binding.
    pub rev: Option<u32>,
    /// Free text after the ID on the same line, if any.
    pub note: Option<String>,
    /// `false` when the ID contains anything outside Latin letters, digits,
    /// `-`, `_`, `.` and `:` — a mixed-script ID (ADR-0009), reported, never fatal.
    pub id_latin: bool,
    /// Byte offset of the `@` inside the comment text.
    pub offset: usize,
}

/// Every marker in one comment's text, in order of appearance.
///
/// A keyword counts only when `@` precedes it directly and whitespace (or the
/// end of the text) follows it, so `user@implements.example` is not a marker.
#[must_use]
pub fn markers_in(comment: &str) -> Vec<Marker> {
    let mut found = Vec::new();
    let mut at = 0;
    while let Some(relative) = comment[at..].find('@') {
        let start = at + relative;
        at = start + 1;
        let Some((relation, keyword_end)) = keyword_at(comment, at) else {
            continue;
        };
        let line_end = comment[keyword_end..]
            .find('\n')
            .map_or(comment.len(), |n| keyword_end + n);
        let line = &comment[keyword_end..line_end];
        let id_start = keyword_end + (line.len() - line.trim_start().len());
        let id_len = comment[id_start..line_end]
            .find(|c: char| c.is_whitespace() || c == '@')
            .unwrap_or(line_end - id_start);
        let id_end = id_start + id_len;
        let id = comment[id_start..id_end].to_owned();
        let mut cursor = id_end;
        let mut rev = None;
        if comment[cursor..line_end].starts_with('@') {
            let digits_len = comment[cursor + 1..line_end]
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(line_end - cursor - 1);
            if digits_len > 0 {
                rev = comment[cursor + 1..cursor + 1 + digits_len].parse().ok();
                cursor += 1 + digits_len;
            }
        }
        let note_end = next_keyword_start(comment, cursor, line_end);
        let note = comment[cursor..note_end]
            .trim()
            .trim_end_matches("*/")
            .trim();
        found.push(Marker {
            relation,
            id_latin: is_latin_id(&id),
            id,
            rev,
            note: (!note.is_empty()).then(|| note.to_owned()),
            offset: start,
        });
        at = cursor;
    }
    found
}

/// The relation keyword starting at `at`, with the offset just after it.
fn keyword_at(text: &str, at: usize) -> Option<(Relation, usize)> {
    let rest = &text[at..];
    Relation::ALL.iter().copied().find_map(|relation| {
        let word = relation.as_str();
        let after = rest.strip_prefix(word)?;
        after
            .chars()
            .next()
            .is_none_or(char::is_whitespace)
            .then_some((relation, at + word.len()))
    })
}

/// Where the next `@keyword` begins between `from` and `to`, or `to`.
fn next_keyword_start(text: &str, from: usize, to: usize) -> usize {
    let mut at = from;
    while let Some(relative) = text[at..to].find('@') {
        let start = at + relative;
        if keyword_at(text, start + 1).is_some() {
            return start;
        }
        at = start + 1;
    }
    to
}

fn is_latin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
}
