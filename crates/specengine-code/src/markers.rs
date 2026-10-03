//! Marker syntax shared by `.rs` and `.ron` comments (`docs/canon/code-identity.md`;
//! canon form `docs/canon/architecture.md#markers`, ADR-0010, ADR-0018):
//! `@implements|@verifies|@configures|@assumes ID[@rev] [levels] note`, any
//! number per comment.
//!
//! - **ID**: the first non-blank after the keyword on its line, up to
//!   whitespace, `@`, `[` or `*/`; Latin = ASCII letters, digits, `-_.:/`
//!   (ADR-0009, ADR-0026 `<slug>/ID`). An empty ID (the keyword ends the line)
//!   is its own case, never "not Latin".
//! - **rev**: `@` and ASCII digits right after the ID.
//! - **levels**: optional spaces/tabs, then `[`…`]` on the marker's line,
//!   before a `*/` and the next `@keyword`; items `path|sig|body|deps`,
//!   lowercase, `,`-separated, spaces/tabs around, one trailing `,`
//!   tolerated; kept in canonical order. No list → [`Levels::Default`]
//!   (`[sig, body]`), told apart from a declared one. A malformed list is
//!   [`Levels::Invalid`] with a fixed [`LevelError`] — never the default,
//!   never fatal, never text from the file.
//! - **note**: the text after `]` (or after `ID[@rev]`), trimmed, `*/`
//!   stripped, up to the next keyword; a later `[` is note text. An
//!   `unclosed` list leaves no note.
//!
//! Cost: one pass over the comment; the level scan never leaves the
//! marker's line, and each line end is looked for once.
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

/// A fingerprint level a binding depends on (05 §5.2), in canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Path,
    Sig,
    Body,
    Deps,
}

impl Level {
    /// Every level, in canonical order.
    pub const ALL: [Self; 4] = [Self::Path, Self::Sig, Self::Body, Self::Deps];

    /// The item as written in a level list.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Sig => "sig",
            Self::Body => "body",
            Self::Deps => "deps",
        }
    }

    fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.as_str() == word)
    }
}

/// The levels of a marker without a list.
pub const DEFAULT_LEVELS: [Level; 2] = [Level::Sig, Level::Body];

/// Why a level list is not trusted; the first that applies, in this order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LevelError {
    /// No `]` before the line end, a `*/` or the next `@keyword`.
    Unclosed,
    /// An item that is not exactly `path`, `sig`, `body` or `deps`; an
    /// empty item (`[sig,,body]`, `[,]`) included.
    Unknown,
    /// One level written twice.
    Duplicate,
    /// No item at all: `[]`, `[ ]`.
    Empty,
}

impl LevelError {
    /// The fixed category name; never text from the file.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Duplicate => "duplicate",
            Self::Unknown => "unknown",
            Self::Unclosed => "unclosed",
        }
    }
}

/// The level list of a marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Levels {
    /// No list: the binding depends on [`DEFAULT_LEVELS`].
    Default,
    /// A well-formed list, in canonical order, without repeats.
    Declared(Vec<Level>),
    /// A malformed list: the levels are untrusted (Phase 3: `cannot_verify`).
    Invalid(LevelError),
}

impl Levels {
    /// The levels the binding depends on: the declared ones or
    /// [`DEFAULT_LEVELS`]; `None` when the list is invalid.
    #[must_use]
    pub fn effective(&self) -> Option<&[Level]> {
        match self {
            Self::Default => Some(&DEFAULT_LEVELS),
            Self::Declared(levels) => Some(levels),
            Self::Invalid(_) => None,
        }
    }

    /// `default`, `declared` or the [`LevelError`] category.
    #[must_use]
    pub fn state(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Declared(_) => "declared",
            Self::Invalid(error) => error.as_str(),
        }
    }
}

/// One marker as written in a comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub relation: Relation,
    /// The ID verbatim; empty when the keyword ends the line (or is followed
    /// directly by a level list or `*/`).
    pub id: String,
    /// `@rev` after the ID (ADR-0018); `None` for a weak binding.
    pub rev: Option<u32>,
    /// The `[levels]` after `ID[@rev]`.
    pub levels: Levels,
    /// Free text after the ID (and the level list) on the same line, if any.
    pub note: Option<String>,
    /// `false` when the ID contains anything outside Latin letters, digits,
    /// `-`, `_`, `.`, `:` and `/` — a mixed-script ID (ADR-0009), reported,
    /// never fatal. `true` for an empty ID.
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
    let mut lines = LineEnds::default();
    let mut at = 0;
    while let Some(relative) = comment[at..].find('@') {
        let start = at + relative;
        at = start + 1;
        let Some((relation, keyword_end)) = keyword_at(comment, at) else {
            continue;
        };
        let line_end = lines.end_from(comment, keyword_end);
        let line = &comment[keyword_end..line_end];
        let id_start = keyword_end + (line.len() - line.trim_start().len());
        let id_end = id_end(comment, id_start, line_end);
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
        let (levels, note_from) = match level_list(comment, cursor, line_end) {
            None => (Levels::Default, Some(cursor)),
            Some(Ok((levels, after))) => (levels, Some(after)),
            Some(Err(stop)) => {
                cursor = stop;
                (Levels::Invalid(LevelError::Unclosed), None)
            }
        };
        let note = note_from.and_then(|from| {
            cursor = from;
            let note_end = next_keyword_start(comment, from, line_end);
            let note = comment[from..note_end].trim().trim_end_matches("*/").trim();
            (!note.is_empty()).then(|| note.to_owned())
        });
        found.push(Marker {
            relation,
            id_latin: is_latin_id(&id),
            id,
            rev,
            levels,
            note,
            offset: start,
        });
        at = cursor;
    }
    found
}

/// The first `\n` at or after a position, found once per line: positions
/// only grow, so a line end found earlier stays valid until it is passed.
#[derive(Default)]
struct LineEnds {
    /// `(from, end)`: no `\n` in `from..end`; `end` is one or the text's end.
    known: Option<(usize, usize)>,
}

impl LineEnds {
    fn end_from(&mut self, text: &str, at: usize) -> usize {
        if let Some((from, end)) = self.known
            && from <= at
            && at <= end
        {
            return end;
        }
        let end = text[at..].find('\n').map_or(text.len(), |n| at + n);
        self.known = Some((at, end));
        end
    }
}

/// Where the ID starting at `from` ends: at whitespace, `@`, `[`, `*/` or `to`.
fn id_end(text: &str, from: usize, to: usize) -> usize {
    let rest = &text[from..to];
    for (index, c) in rest.char_indices() {
        if c.is_whitespace() || c == '@' || c == '[' {
            return from + index;
        }
        // `*` is one byte, so `index + 1` is a character boundary.
        if c == '*' && rest[index + 1..].starts_with('/') {
            return from + index;
        }
    }
    to
}

/// The level list after `ID[@rev]` (ending at `from`): `None` when no `[`
/// follows on the line after spaces/tabs; `Ok((levels, after_bracket))` when
/// a `]` closes it; `Err(stop)` when the line end, a `*/` or the next
/// `@keyword` (at `stop`) comes first.
fn level_list(text: &str, from: usize, line_end: usize) -> Option<Result<(Levels, usize), usize>> {
    let bytes = text.as_bytes();
    let mut open = from;
    while open < line_end && matches!(bytes[open], b' ' | b'\t') {
        open += 1;
    }
    if open >= line_end || bytes[open] != b'[' {
        return None;
    }
    let inner = open + 1;
    // Only ASCII bytes are compared, so every stop is a character boundary.
    let mut at = inner;
    while at < line_end {
        match bytes[at] {
            b']' => return Some(Ok((parse_levels(&text[inner..at]), at + 1))),
            b'*' if bytes.get(at + 1) == Some(&b'/') => return Some(Err(at)),
            b'@' if keyword_at(text, at + 1).is_some() => return Some(Err(at)),
            _ => at += 1,
        }
    }
    Some(Err(line_end))
}

/// The items between `[` and `]`; errors by priority `unknown`, `duplicate`,
/// `empty` (`unclosed` is decided by the caller).
fn parse_levels(inner: &str) -> Levels {
    let blank = |c: char| c == ' ' || c == '\t';
    if inner.trim_matches(blank).is_empty() {
        return Levels::Invalid(LevelError::Empty);
    }
    let mut seen = [false; Level::ALL.len()];
    let mut duplicate = false;
    let mut items = inner.split(',').peekable();
    let mut first = true;
    while let Some(item) = items.next() {
        let word = item.trim_matches(blank);
        // One trailing `,` after at least one item is tolerated.
        if word.is_empty() && !first && items.peek().is_none() {
            break;
        }
        let Some(level) = Level::parse(word) else {
            return Levels::Invalid(LevelError::Unknown);
        };
        let slot = &mut seen[level as usize];
        duplicate |= *slot;
        *slot = true;
        first = false;
    }
    if duplicate {
        return Levels::Invalid(LevelError::Duplicate);
    }
    Levels::Declared(
        Level::ALL
            .into_iter()
            .filter(|level| seen[*level as usize])
            .collect(),
    )
}

/// The relation keyword starting at `at`, with the offset just after it.
fn keyword_at(text: &str, at: usize) -> Option<(Relation, usize)> {
    let rest = text.get(at..)?;
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

/// Latin: ASCII letters, digits, `-`, `_`, `.`, `:`, `/`; an empty ID is not
/// a script mix (it is counted apart by the callers).
fn is_latin_id(id: &str) -> bool {
    id.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':' | '/'))
}
