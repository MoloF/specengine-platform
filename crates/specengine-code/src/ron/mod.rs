//! RON support (`docs/canon/code-identity.md` "Marker grammar", "RON binding";
//! 05 §9 "AST" row): a marker in a `.ron` comment attaches to an entry, element
//! or root value; its path becomes the `qpath`, e.g.
//! `data/movement.ron#root.stamina.regen_per_second`. Only the
//! comment's position counts, and the first rule that applies decides: a
//! block comment attaches to the value that starts on the line of its `*/`
//! (`pos: (/* m */ 10, 20)`); else a comment that starts on the line where a
//! value ends (`speed: 1, // m`) or right after a container's opener
//! (`player: Player( // m`) attaches to that value; else a comment on its
//! own line attaches to the value that begins next; else it is
//! `unanchored`. The rules with their examples and the fallback in broken
//! files are on [`Anchor`]; every edge case is in the `structure` module.
//!
//! [`analyze`] is the own lexer ([`lexer`]) plus a tolerant structure walker,
//! with no dependencies: the Phase 0 verdict (05 §9 "AST" row, 04 §6) is
//! `lexer`, and the `tree-sitter-ron` comparison path was removed after it.
//!
//! **Path format.** `root` is the file's value; a struct field appends
//! `.name`, a list element `[index]`, a tuple element `.index` (a tuple struct
//! such as `Some(x)` counts as a tuple), a map entry `{key}` with the key's
//! source text (string keys keep their quotes; whitespace runs collapse to
//! one space, inside strings too). Depth is the number of segments
//! after `root`: `root.player.speed` and `root.waves[2]` are both depth 2.
//! A field name or a map key longer than [`MAX_SEGMENT_BYTES`] source bytes
//! (an unbalanced key runs to the end of input) keeps its first bytes, cut at
//! a character boundary, plus [`TRUNCATION_MARK`]: a path stays bounded and
//! deterministic whatever the file.
//!
//! **Ambiguity.** Two siblings of one struct or map whose segments render
//! alike — cut at the cap, whitespace collapsed, or a field or key written
//! twice — give one path to two values: every marker under either of them
//! (on them or deeper) is [`Anchor::Ambiguous`], whether or not the other
//! sibling has a marker; decided when the container closes. Both markers
//! stay, never merged. [`RonAnalysis::colliding_groups`] counts such sibling
//! groups, marked or not.

pub mod lexer;
mod structure;

use std::collections::BTreeSet;
use std::ops::Range;

pub use structure::MAX_DEPTH;

/// Source bytes of a field name or a map key kept in a path segment.
pub const MAX_SEGMENT_BYTES: usize = 128;

/// Ends a segment cut at [`MAX_SEGMENT_BYTES`].
pub const TRUNCATION_MARK: char = '…';

use crate::markers::{Marker, markers_in};

/// What a marker resolved to.
///
/// A *value* is an entry (a struct field or a map entry), a list or tuple
/// element, or the root value. Only the comment's position counts, not its
/// kind — except that a `//` comment never leads (nothing follows it on its
/// line). A comment's markers bind by the first rule that applies:
///
/// 1. **Leading.** A block comment followed, on the line of its `*/`, by the
///    start of a value (only whitespace and comments between) binds to that
///    value, ahead of rules 2–3: `pos: (/* A */ 10, /* B */ 20)` →
///    `root.pos.0`, `root.pos.1`; `a: 1, /* m */ b: 2` → `root.b`. A value
///    starts at a field name or a map key (the comment binds to the entry),
///    an element, the root value, or an entry's value after its `:` — so
///    `speed: /* m */ 4.5` → `root.speed`, also with `speed:` alone on the
///    line above. Only an identifier, an opener, a string, a char or a
///    number starts one; a closer, `,`, `:` or an extension attribute never
///    does.
/// 2. **Trailing a value.** A comment that starts on the line of a value's
///    last token binds to that value, with the `,` on either side of the
///    comment; of several values closing on that line, to the one whose last
///    token comes right before the comment: `speed: 1, // m` → `root.speed`;
///    `waves: [1, 2, 3], // m` → `root.waves`; `[1, 2, 3 /* m */]` →
///    `root[2]`; `Config(..) // m` → `root`.
/// 3. **Trailing an opener.** A comment right after `(`, `[` or `{` on its
///    line binds to the container's value: `player: Player( // m` →
///    `root.player`, `Config( // m` → `root`.
/// 4. **Own line.** A comment with no token before it on its line binds to
///    the value beginning at the next token; extension attributes before the
///    root value are transparent (`#![enable(..)] // m` → `root`).
/// 5. Otherwise [`Anchor::Unanchored`].
///
/// A multi-line block comment trails by the line of its `/*` and leads by
/// the line of its `*/`; an unclosed container at the end of input has no
/// end, so nothing trails it. In a broken file, when the token a block
/// comment leads begins no value after all (error recovery consumes it, or
/// it is trailing content), the comment falls back to the value it trails:
/// `a: 1 /* m */ 2` → `root.a`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Anchor {
    /// The value the comment binds to by rules 1–4, or by the fallback.
    Path {
        path: String,
        /// Segments after `root`; `root` itself is depth 0.
        depth: usize,
    },
    /// Bound like [`Anchor::Path`], but the path names more than one value:
    /// a segment of it (the value's own or an ancestor's) equals a sibling's
    /// in the same struct or map. The path text is kept; the binding is not
    /// trusted.
    Ambiguous {
        path: String,
        /// Segments after `root`.
        depth: usize,
    },
    /// No value to attach to (rule 5): an own-line comment before a closer
    /// or a `,`; a comment after a map key; a comment after a `:` or inside
    /// a value already begun that does not lead the value (`speed:`, then
    /// `// m`, then `4.5` on separate lines; a block comment whose value
    /// starts on a later line than its `*/`); a comment after a `,` on a
    /// line where no value ended; one between a type name and its `(`
    /// (`Player /* m */ (`); one after tokens error recovery consumed, with
    /// no fallback; one after trailing content.
    Unanchored,
    /// The structure around the comment is not known: the comment lies
    /// inside a container skipped as `nesting_too_deep`.
    CannotVerify,
}

impl Anchor {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Path { path, .. } | Self::Ambiguous { path, .. } => path,
            Self::Unanchored => "unanchored",
            Self::CannotVerify => "cannot_verify",
        }
    }
}

/// A marker found in a `.ron` comment, with its resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RonMarker {
    pub marker: Marker,
    /// 1-based line of the comment's first byte.
    pub line: usize,
    /// Byte range of the whole comment.
    pub comment: Range<usize>,
    pub anchor: Anchor,
}

/// A construct the approach could not parse: a category and where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejected {
    /// A fixed name such as `raw_string` or `unbalanced_delimiter`; never text from the file.
    pub category: &'static str,
    pub offset: usize,
}

/// One file's analysis.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RonAnalysis {
    /// The file did not parse cleanly under this approach.
    pub has_error: bool,
    /// Categories of the rejected constructs, sorted, deduplicated.
    pub error_categories: Vec<&'static str>,
    /// Every rejected construct, in source order.
    pub rejected: Vec<Rejected>,
    /// Byte ranges of every comment, in source order.
    pub comments: Vec<Range<usize>>,
    /// Every marker in every comment, in source order.
    pub markers: Vec<RonMarker>,
    /// Sibling groups of a struct or map sharing one segment text, marked
    /// or not: each group once, however many siblings it holds.
    pub colliding_groups: usize,
}

impl RonAnalysis {
    /// Assembles the analysis from its parts; categories derived from `rejected`.
    #[must_use]
    pub(crate) fn assemble(
        mut rejected: Vec<Rejected>,
        comments: Vec<Range<usize>>,
        markers: Vec<RonMarker>,
        colliding_groups: usize,
    ) -> Self {
        rejected.sort_by_key(|r| (r.offset, r.category));
        let categories: BTreeSet<&'static str> = rejected.iter().map(|r| r.category).collect();
        Self {
            has_error: !rejected.is_empty(),
            error_categories: categories.into_iter().collect(),
            rejected,
            comments,
            markers,
            colliding_groups,
        }
    }

    /// `true` when at least one marker resolved to a path of depth ≥ 2
    /// (an ambiguous one does not count).
    #[must_use]
    pub fn has_nested_anchor(&self) -> bool {
        self.markers
            .iter()
            .any(|m| matches!(m.anchor, Anchor::Path { depth, .. } if depth >= 2))
    }
}

/// One segment of a path; rendered by [`render_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Segment {
    Field(String),
    Index(usize),
    Tuple(usize),
    Key(String),
}

/// `root` plus every segment; depth = number of segments.
pub(crate) fn render_path(segments: &[Segment]) -> Anchor {
    let mut path = String::from("root");
    for segment in segments {
        match segment {
            Segment::Field(name) => {
                path.push('.');
                path.push_str(name);
            }
            Segment::Index(index) => path.push_str(&format!("[{index}]")),
            Segment::Tuple(index) => path.push_str(&format!(".{index}")),
            Segment::Key(key) => path.push_str(&format!("{{{key}}}")),
        }
    }
    Anchor::Path {
        path,
        depth: segments.len(),
    }
}

/// Byte offsets of every `\n` of a file, found once: a line lookup is a
/// binary search, not a scan from the start of the file.
#[derive(Debug, Default)]
pub(crate) struct LineIndex {
    newlines: Vec<usize>,
}

impl LineIndex {
    pub(crate) fn new(source: &str) -> Self {
        Self {
            newlines: source
                .bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(at, _)| at)
                .collect(),
        }
    }

    /// 1-based line of a byte offset (an offset past the end counts every line).
    pub(crate) fn line_of(&self, offset: usize) -> usize {
        self.newlines.partition_point(|at| *at < offset) + 1
    }
}

/// Whitespace runs collapsed to one space, ends trimmed: a map key's identity.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The segment text of `source[start..end]`: whole when it spans at most
/// [`MAX_SEGMENT_BYTES`], otherwise its first bytes cut at a character
/// boundary plus [`TRUNCATION_MARK`]; whitespace collapsed either way.
/// Only the kept bytes are read, so the cost is bounded by the cap.
pub(crate) fn segment_text(source: &str, start: usize, end: usize) -> String {
    if end.saturating_sub(start) <= MAX_SEGMENT_BYTES {
        return collapse_whitespace(&source[start..end]);
    }
    let mut cut = start + MAX_SEGMENT_BYTES;
    while !source.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut text = collapse_whitespace(&source[start..cut]);
    text.push(TRUNCATION_MARK);
    text
}

/// The lexer approach: tokens, a tolerant structure walk, markers resolved
/// to paths. Never fails; a broken file yields its comments and markers plus
/// the categories of what was rejected.
#[must_use]
pub fn analyze(source: &str) -> RonAnalysis {
    let (tokens, lex_errors) = lexer::lex(source);
    let comments: Vec<Range<usize>> = tokens
        .iter()
        .filter(|t| t.kind.is_comment())
        .map(|t| t.range.clone())
        .collect();
    let raw_markers: Vec<(usize, Marker)> = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind.is_comment())
        .flat_map(|(index, t)| {
            markers_in(&source[t.range.clone()])
                .into_iter()
                .map(move |marker| (index, marker))
        })
        .collect();
    let walked = structure::walk(source, &tokens, raw_markers);
    let structure_errors = walked.errors;
    let rejected = lex_errors
        .iter()
        .map(|e| Rejected {
            category: e.kind.as_str(),
            offset: e.offset,
        })
        .chain(structure_errors.iter().map(|e| Rejected {
            category: e.kind.as_str(),
            offset: e.offset,
        }))
        .collect();
    RonAnalysis::assemble(rejected, comments, walked.markers, walked.colliding_groups)
}
