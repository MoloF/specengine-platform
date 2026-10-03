//! Tolerant structure walk over the lexer's tokens: enough of RON's shape
//! (struct bodies, tuples, lists, maps) to name the entry a marker belongs to.
//!
//! Adjacency (`docs/canon/code-identity.md` "RON binding", rules 1–5; owner
//! decisions 2026-09-28). The markers of the comments crossed on the way to a
//! token are *pending*; each resolves by where its comment ends, relative to
//! the token after it, else by where it starts, relative to the last token
//! before it. "Value" below is an entry, an element or the root value; the
//! comment kind (`//` or `/* */`) never matters, only positions do. The rules
//! apply in this order:
//!
//! - **Leading a value.** A comment followed, on the line where it ends
//!   (its `*/`), by the token a value begins with — only whitespace and
//!   other comments between — binds to that value, before any trailing
//!   rule: `pos: (/* A */ 10, /* B */ 20)` → `….pos.0`, `….pos.1`;
//!   `a: 1, /* m */ b: 2` → `….b`; `d: (/* m */ 1, 2)` → `….d.0`;
//!   `speed: /* m */ 4.5` → `….speed`; `{ /* m */ "fire": 3 }` →
//!   `root{"fire"}`. A value begins where the walk begins an entry (at its
//!   field name or map key: the comment binds to the entry), an element or
//!   the root value, and where it begins an entry's value after the `:`.
//!   Only a block comment can lead: nothing follows a `//` on its line. A
//!   multi-line block comment leads by the line of its `*/` and trails by
//!   the line of its `/*`. Followed on that line by a token no value begins
//!   with (a closer, `,`, `:`, an attribute), by a token error recovery
//!   consumes instead, or by nothing, the comment takes the rules below.
//! - **Trailing a value.** A comment that starts on the line where a value
//!   ends (the line of the value's last token) binds to that value:
//!   `speed: 1, // m` → `….speed`; `[1, 2, // m` → `…[1]`;
//!   `waves: [1, 2, 3], // m` → `….waves` (the `]` ends `waves`, not
//!   `[2]`); `[1, 2, 3 /* m */]` → `…[2]`; `Config(..) // m` → `root`. The
//!   `,` after the value is transparent on either side of the comment:
//!   `a: 1 // m` with the `,` on the next line binds to `a`. The last entry
//!   before a closer is no exception: `a: 1, // m` then `)` binds to `a`.
//! - **Trailing an opener.** A comment that starts on the line of a
//!   container's opener, right after it, binds to the value that container
//!   is: `player: Player( // m` → `….player`, `waves: [ // m` → `….waves`,
//!   `Config( // m` → `root`. The opening line of a multi-line value reads
//!   like its closing line (`), // m` → `….player`), never as the first field
//!   below it.
//! - **Own line.** A comment with no token before it on its line binds to the
//!   value that begins at the next token (only whitespace and comments in
//!   between). Extension attributes before the root value are transparent: a
//!   comment after `#![enable(..)]`, even on its line, binds to `root`.
//! - **Otherwise `unanchored`**: an own-line comment before a closer or a
//!   `,`; a comment after a `:` or inside a value already begun
//!   (`speed: // m` then `4.5` on the next line); a comment after a `,` on a
//!   line where no value ended (`, // m` under `a: 1`); a comment trailing a
//!   token that error recovery consumed.
//!
//! So a same-line comment annotates the value that follows it on the line,
//! else what precedes it there: on the one line
//! `/* A1 */ Config( /* A2 */ x: 1 /* A3 */ ) /* A4 */` `A1` binds to `root`,
//! `A2` and `A3` to `root.x`, `A4` to `root`.
//!
//! A leading comment is decided when crossed (its `*/` line against the next
//! token's line and kind). When it also trails a value, that value's path is
//! kept as its fallback: the comment binds to the value the walk begins at
//! the next token, else — that token consumed without a value begun there —
//! to the fallback. Each marker is resolved exactly once.
//!
//! Every loop consumes a token or returns, so a broken file terminates.
//!
//! Depth: the walk recurses once per container, so it is capped at
//! [`MAX_DEPTH`] open containers. A container that would open past the cap is
//! a `nesting_too_deep` rejection and is skipped whole, opener through its
//! matching closer (to the end of input when unbalanced); the markers in the
//! comments inside it resolve to `cannot_verify`, and the walk goes on after
//! the closer. The stack a walk needs is therefore bounded whatever the file.
//!
//! Ambiguity (`docs/canon/code-identity.md`). Each open struct body or map
//! keeps one set of its entries' segment texts; an entry whose segment is
//! already there joins that sibling *group*, and a group of two or more
//! collides. Every group records the group of the entry enclosing its
//! container, so a resolved marker remembers only the innermost entry it lies
//! under; after the walk — every container closed, the unclosed ones at the
//! end of input — a marker is [`Anchor::Ambiguous`] when that group or any
//! enclosing one collides. Siblings sharing a segment are thus flagged
//! wholesale, marked or not, with no pairwise comparison and no path per
//! value.
//!
//! Cost: linear in the tokens. Per entry the walk reads at most
//! [`MAX_SEGMENT_BYTES`] of a field name or a map key (the key's extent is
//! looked for no further) and looks that bounded text up once in its
//! container's set; it renders a path only when a marker is pending
//! (and, for a trailing comment, only when one starts on the value's line).
//! Lines are looked up only for marker comments: a fixed few when a comment
//! is crossed (its two ends, the tokens on either side), once per pending
//! check at a value's end or an opener.

use std::collections::HashMap;
use std::ops::Range;

use super::lexer::{Delim, Token, TokenKind};
use super::{Anchor, LineIndex, MAX_SEGMENT_BYTES, RonMarker, Segment, render_path, segment_text};
use crate::markers::Marker;

/// Containers open at once before the walk stops descending. Real RON data
/// sits far below it; each level costs three or four stack frames, so 512
/// levels fit a 2 MB thread stack with a wide margin even in a debug build
/// (the uncapped walk still passed at 5 000 levels there).
pub const MAX_DEPTH: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum StructureErrorKind {
    EmptyFile,
    ExpectedValue,
    ExpectedField,
    ExpectedColon,
    ExpectedSeparator,
    UnbalancedDelimiter,
    TrailingContent,
    MisplacedAttribute,
    NestingTooDeep,
}

impl StructureErrorKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::EmptyFile => "empty_file",
            Self::ExpectedValue => "expected_value",
            Self::ExpectedField => "expected_field",
            Self::ExpectedColon => "expected_colon",
            Self::ExpectedSeparator => "expected_separator",
            Self::UnbalancedDelimiter => "unbalanced_delimiter",
            Self::TrailingContent => "trailing_content",
            Self::MisplacedAttribute => "misplaced_attribute",
            Self::NestingTooDeep => "nesting_too_deep",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructureError {
    pub kind: StructureErrorKind,
    pub offset: usize,
}

/// What a walk yields.
pub(super) struct Walked {
    /// Resolved markers, in source order.
    pub markers: Vec<RonMarker>,
    /// Structural errors met on the way.
    pub errors: Vec<StructureError>,
    /// Sibling groups sharing a segment text.
    pub colliding_groups: usize,
}

/// Resolves `markers` (comment token index, marker) against the structure of
/// `tokens`.
pub(super) fn walk(source: &str, tokens: &[Token], markers: Vec<(usize, Marker)>) -> Walked {
    // Lines are needed only to resolve markers.
    let lines = if markers.is_empty() {
        LineIndex::default()
    } else {
        LineIndex::new(source)
    };
    let mut walker = Walker {
        source,
        tokens,
        lines,
        pos: 0,
        last: None,
        depth: 0,
        markers,
        next_marker: 0,
        pending: Vec::new(),
        resolved: Vec::new(),
        errors: Vec::new(),
        sibling_sets: Vec::new(),
        open_entries: Vec::new(),
        groups: Vec::new(),
        colliding_groups: 0,
    };
    walker.file();
    // Every container is closed now: a group is flagged when it or an
    // enclosing group collides (an enclosing group is always created first).
    let mut flagged = vec![false; walker.groups.len()];
    for (index, group) in walker.groups.iter().enumerate() {
        flagged[index] = group.collides
            || group
                .parent
                .is_some_and(|parent| parent < index && flagged[parent]);
    }
    let mut resolved: Vec<RonMarker> = walker
        .resolved
        .into_iter()
        .map(|(mut marker, group)| {
            if group.is_some_and(|group| flagged[group])
                && let Anchor::Path { path, depth } = marker.anchor
            {
                marker.anchor = Anchor::Ambiguous { path, depth };
            }
            marker
        })
        .collect();
    resolved.sort_by_key(|m| (m.comment.start, m.marker.offset));
    Walked {
        markers: resolved,
        errors: walker.errors,
        colliding_groups: walker.colliding_groups,
    }
}

/// Innermost struct or map entry a path lies under: an index into
/// [`Walker::groups`]; `None` at the root and under elements only.
type GroupId = Option<usize>;

/// The siblings of one struct body or map sharing one segment text.
struct Group {
    /// The group of the entry whose value holds this group's container.
    parent: GroupId,
    /// Two or more siblings share the segment.
    collides: bool,
}

/// A marker of a crossed comment, not resolved yet.
struct Pending {
    marker: Marker,
    /// Byte range of the whole comment.
    comment: Range<usize>,
    /// 1-based line of the comment's first byte.
    line: usize,
    /// 1-based line of the comment's last byte (a block comment's `*/`).
    end_line: usize,
    /// No token before the comment on its line (extension attributes before
    /// the root value aside): it may bind to the value that begins next.
    own_line: bool,
    /// The next token is of a kind a value begins with and starts on
    /// `end_line`: the comment binds to the value the walk begins there, if
    /// one does.
    leads: bool,
    /// Set on a leading comment that trails a value: its anchor (and the
    /// innermost entry it lies under) when no value begins at the next token
    /// after all.
    fallback: Option<(Anchor, GroupId)>,
}

/// Token kinds a value can begin with ([`Walker::value`]); extension
/// attributes are transparent or misplaced, never a value's start.
fn begins_value(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Ident
            | TokenKind::Open(_)
            | TokenKind::Str
            | TokenKind::Char
            | TokenKind::Number
    )
}

struct Walker<'a> {
    source: &'a str,
    tokens: &'a [Token],
    lines: LineIndex,
    pos: usize,
    /// Index of the last consumed non-comment token: the token right before
    /// every comment crossed next. The extension attributes before the root
    /// value do not count (they are transparent for adjacency).
    last: Option<usize>,
    /// Containers currently open in the walk; never above [`MAX_DEPTH`].
    depth: usize,
    /// Sorted by comment token index.
    markers: Vec<(usize, Marker)>,
    next_marker: usize,
    /// Markers of the comments crossed since the last consumed token.
    pending: Vec<Pending>,
    /// Resolved markers with the innermost entry their anchor lies under.
    resolved: Vec<(RonMarker, GroupId)>,
    errors: Vec<StructureError>,
    /// One set per open struct body or map: segment text → its group.
    sibling_sets: Vec<HashMap<String, usize>>,
    /// The group of every open struct or map entry, outermost first.
    open_entries: Vec<usize>,
    groups: Vec<Group>,
    colliding_groups: usize,
}

impl Walker<'_> {
    fn file(&mut self) {
        // Extension attributes before the value are transparent for pending
        // markers: skipped without becoming `last`.
        while self.peek().is_some_and(|t| t.kind == TokenKind::Attribute) {
            self.pos += 1;
        }
        let mut path = Vec::new();
        match self.peek() {
            None => self.error(StructureErrorKind::EmptyFile, self.source.len()),
            Some(_) => {
                self.anchor_pending(&path);
                if self.value(&mut path)
                    && let Some(end) = self.last
                {
                    self.claim_trailing(&path, end);
                }
            }
        }
        if let Some(start) = self.peek().map(|t| t.range.start) {
            self.error(StructureErrorKind::TrailingContent, start);
        }
        while self.peek().is_some() {
            self.bump();
        }
        self.settle_pending(Anchor::Unanchored);
    }

    /// The value at the next token. `true` when one was consumed through its
    /// end: its last token is then `self.last`, and comments trailing it on
    /// that token's line belong to the value's entry (see the module docs).
    /// `false` when no value starts there (an `expected_value` error, nothing
    /// but misplaced attributes consumed) or its container runs to the end
    /// of input.
    fn value(&mut self, path: &mut Vec<Segment>) -> bool {
        // Extension attributes are misplaced inside the value: one error
        // each, then the value after them (a loop: their number is unbounded).
        let (kind, start) = loop {
            let Some(token) = self.peek() else {
                self.error(StructureErrorKind::ExpectedValue, self.source.len());
                return false;
            };
            let (kind, start) = (token.kind, token.range.start);
            if kind != TokenKind::Attribute {
                break (kind, start);
            }
            self.error(StructureErrorKind::MisplacedAttribute, start);
            self.bump();
        };
        match kind {
            TokenKind::Ident => {
                self.bump();
                if self
                    .peek()
                    .is_some_and(|t| t.kind == TokenKind::Open(Delim::Paren))
                {
                    self.container(path, Delim::Paren)
                } else {
                    true
                }
            }
            TokenKind::Open(delim) => self.container(path, delim),
            TokenKind::Str | TokenKind::Char | TokenKind::Number => {
                self.bump();
                true
            }
            // `Attribute` cannot reach here: the loop above consumed it.
            TokenKind::Attribute
            | TokenKind::Close(_)
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Unknown
            | TokenKind::LineComment
            | TokenKind::BlockComment => {
                self.error(StructureErrorKind::ExpectedValue, start);
                false
            }
        }
    }

    /// The container whose opener is the next token: its body one level
    /// deeper, or — at [`MAX_DEPTH`] — a `nesting_too_deep` skip. `path` is
    /// the value the container is: comments trailing the opener on its line
    /// bind to it. `true` when a closer ended it (then `self.last`); `false`
    /// when it ran to the end of input — the value has no end for a trailing
    /// comment to follow.
    fn container(&mut self, path: &mut Vec<Segment>, delim: Delim) -> bool {
        if self.depth >= MAX_DEPTH {
            let at = self.offset();
            self.error(StructureErrorKind::NestingTooDeep, at);
            return self.skip_container();
        }
        self.depth += 1;
        self.bump();
        if let Some(opener) = self.last {
            self.claim_trailing(path, opener);
        }
        let closed = match delim {
            Delim::Paren => self.paren_body(path),
            Delim::Bracket => self.sequence(path, Delim::Bracket, false),
            Delim::Brace => self.map(path),
        };
        self.depth -= 1;
        closed
    }

    /// Consumes the container whose opener is the next token through its
    /// matching closer, without recursion; any closer closes (as in
    /// [`Self::recover`]). Markers pending before the opener settle as for
    /// any consumed token (`unanchored`); markers inside the container,
    /// including one trailing the opener, are `cannot_verify`. An unclosed
    /// container runs to the end of input, which is also an
    /// `unbalanced_delimiter`. `true` when the closer was found: it becomes
    /// `last`, and a comment trailing it binds to the container's value.
    fn skip_container(&mut self) -> bool {
        self.bump();
        let mut depth = 1usize;
        while let Some(kind) = self.peek().map(|t| t.kind) {
            self.last = Some(self.pos);
            self.pos += 1;
            self.settle_pending(Anchor::CannotVerify);
            match kind {
                TokenKind::Open(_) => depth += 1,
                TokenKind::Close(_) => {
                    depth -= 1;
                    if depth == 0 {
                        return true;
                    }
                }
                _ => {}
            }
        }
        // Comments after the last token still lie inside the open container.
        self.settle_pending(Anchor::CannotVerify);
        self.error(StructureErrorKind::UnbalancedDelimiter, self.source.len());
        false
    }

    /// After a `(` (consumed): a named struct body when `ident :` follows,
    /// else a tuple (`()` is an empty tuple, a unit). `true` when closed.
    fn paren_body(&mut self, path: &mut Vec<Segment>) -> bool {
        let named = self
            .lookahead(0)
            .is_some_and(|t| t.kind == TokenKind::Ident)
            && self
                .lookahead(1)
                .is_some_and(|t| t.kind == TokenKind::Colon);
        if named {
            self.struct_entries(path)
        } else {
            self.sequence(path, Delim::Paren, true)
        }
    }

    /// Fields of a named struct; `true` when a closer ended it.
    fn struct_entries(&mut self, path: &mut Vec<Segment>) -> bool {
        self.sibling_sets.push(HashMap::new());
        let closed = self.struct_fields(path);
        self.sibling_sets.pop();
        closed
    }

    fn struct_fields(&mut self, path: &mut Vec<Segment>) -> bool {
        loop {
            let Some(token) = self.peek() else {
                self.error(StructureErrorKind::UnbalancedDelimiter, self.source.len());
                return false;
            };
            let (kind, range) = (token.kind, token.range.clone());
            match kind {
                TokenKind::Close(Delim::Paren) => {
                    self.bump();
                    return true;
                }
                TokenKind::Close(_) => {
                    self.error(StructureErrorKind::UnbalancedDelimiter, range.start);
                    self.bump();
                    return true;
                }
                TokenKind::Ident => {
                    let segment = segment_text(self.source, range.start, range.end);
                    self.enter_entry(&segment);
                    path.push(Segment::Field(segment));
                    self.anchor_pending(path);
                    self.bump();
                    if self.peek().is_some_and(|t| t.kind == TokenKind::Colon) {
                        self.bump();
                    } else {
                        let at = self.offset();
                        self.error(StructureErrorKind::ExpectedColon, at);
                    }
                    self.anchor_leading(path);
                    let has_value = self.value(path);
                    self.separator(path, Delim::Paren, has_value);
                    path.pop();
                    self.open_entries.pop();
                }
                _ => {
                    self.error(StructureErrorKind::ExpectedField, range.start);
                    self.recover(Delim::Paren);
                }
            }
        }
    }

    /// Elements of a list (`[index]`) or a tuple (`.index`); `true` when a
    /// closer ended it.
    fn sequence(&mut self, path: &mut Vec<Segment>, delim: Delim, tuple: bool) -> bool {
        let mut index = 0;
        loop {
            let Some(token) = self.peek() else {
                self.error(StructureErrorKind::UnbalancedDelimiter, self.source.len());
                return false;
            };
            let (kind, start) = (token.kind, token.range.start);
            match kind {
                TokenKind::Close(d) if d == delim => {
                    self.bump();
                    return true;
                }
                TokenKind::Close(_) => {
                    self.error(StructureErrorKind::UnbalancedDelimiter, start);
                    self.bump();
                    return true;
                }
                TokenKind::Comma => {
                    self.error(StructureErrorKind::ExpectedValue, start);
                    self.bump();
                }
                _ => {
                    path.push(if tuple {
                        Segment::Tuple(index)
                    } else {
                        Segment::Index(index)
                    });
                    self.anchor_pending(path);
                    let has_value = self.value(path);
                    self.separator(path, delim, has_value);
                    path.pop();
                    index += 1;
                }
            }
        }
    }

    /// Entries of a map; `true` when a closer ended it.
    fn map(&mut self, path: &mut Vec<Segment>) -> bool {
        self.sibling_sets.push(HashMap::new());
        let closed = self.map_entries(path);
        self.sibling_sets.pop();
        closed
    }

    fn map_entries(&mut self, path: &mut Vec<Segment>) -> bool {
        loop {
            let Some(token) = self.peek() else {
                self.error(StructureErrorKind::UnbalancedDelimiter, self.source.len());
                return false;
            };
            let (kind, start) = (token.kind, token.range.start);
            match kind {
                TokenKind::Close(Delim::Brace) => {
                    self.bump();
                    return true;
                }
                TokenKind::Close(_) => {
                    self.error(StructureErrorKind::UnbalancedDelimiter, start);
                    self.bump();
                    return true;
                }
                TokenKind::Comma => {
                    self.error(StructureErrorKind::ExpectedValue, start);
                    self.bump();
                }
                TokenKind::Colon | TokenKind::Unknown | TokenKind::Attribute => {
                    self.error(StructureErrorKind::ExpectedValue, start);
                    self.recover(Delim::Brace);
                }
                _ => {
                    let limit = start.saturating_add(MAX_SEGMENT_BYTES);
                    // Past the cap the key's end is not looked for: any end
                    // beyond `limit` truncates alike, and the file's end is one.
                    let key_end = self.key_end(limit).unwrap_or(self.source.len());
                    let segment = segment_text(self.source, start, key_end);
                    self.enter_entry(&segment);
                    path.push(Segment::Key(segment));
                    self.anchor_pending(path);
                    // The key is not the entry's end: a comment trailing it
                    // is followed by `:` and stays `unanchored`.
                    self.value(path);
                    if self.peek().is_some_and(|t| t.kind == TokenKind::Colon) {
                        self.bump();
                    } else {
                        let at = self.offset();
                        self.error(StructureErrorKind::ExpectedColon, at);
                    }
                    self.anchor_leading(path);
                    let has_value = self.value(path);
                    self.separator(path, Delim::Brace, has_value);
                    path.pop();
                    self.open_entries.pop();
                }
            }
        }
    }

    /// End offset of the value that starts at the next token, without
    /// consuming it, when that end lies at or before `limit`; `None` when it
    /// lies past. A container value ends at its matching closer (any closer
    /// closes), an unbalanced one at the end of input. The scan stops at the
    /// first token ending past `limit`, so it reads a bounded span of the
    /// file however far the value runs.
    fn key_end(&self, limit: usize) -> Option<usize> {
        let within = |end: usize| (end <= limit).then_some(end);
        let Some(first) = self.lookahead_index(0) else {
            return within(self.source.len());
        };
        let open = match self.tokens[first].kind {
            TokenKind::Open(_) => Some(first),
            TokenKind::Ident => self
                .lookahead_index(1)
                .filter(|i| self.tokens[*i].kind == TokenKind::Open(Delim::Paren)),
            _ => None,
        };
        let Some(open) = open else {
            return within(self.tokens[first].range.end);
        };
        let mut depth = 0usize;
        for token in &self.tokens[open..] {
            if token.range.end > limit {
                return None;
            }
            match token.kind {
                TokenKind::Open(_) => depth += 1,
                TokenKind::Close(_) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(token.range.end);
                    }
                }
                _ => {}
            }
        }
        within(self.source.len())
    }

    /// After the value of the entry or element at `path`: comments trailing
    /// the value's last token on its line, before or after the `,`, bind to
    /// `path` (when `has_value`). Then `,` between entries: consumed; the
    /// container's closer: left for the loop; anything else: an error and a
    /// skip to the next `,` or closer.
    fn separator(&mut self, path: &[Segment], delim: Delim, has_value: bool) {
        let end = if has_value { self.last } else { None };
        if let Some(end) = end {
            self.claim_trailing(path, end);
        }
        let Some(token) = self.peek() else {
            return;
        };
        let (kind, start) = (token.kind, token.range.start);
        match kind {
            TokenKind::Comma => {
                self.bump();
                if let Some(end) = end {
                    self.claim_trailing(path, end);
                }
            }
            TokenKind::Close(d) if d == delim => {}
            _ => {
                self.error(StructureErrorKind::ExpectedSeparator, start);
                self.recover(delim);
            }
        }
    }

    /// Skips to the next `,` at this depth (consumed) or the next closer at
    /// this depth (left for the loop); consumes at least one token unless a
    /// closer or the end of input is next.
    fn recover(&mut self, _delim: Delim) {
        let mut depth = 0usize;
        while let Some(token) = self.peek() {
            match token.kind {
                TokenKind::Open(_) => {
                    depth += 1;
                    self.bump();
                }
                TokenKind::Close(_) if depth == 0 => return,
                TokenKind::Close(_) => {
                    depth -= 1;
                    self.bump();
                }
                TokenKind::Comma if depth == 0 => {
                    self.bump();
                    return;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    /// The next non-comment token; the markers of the comments crossed on the
    /// way become pending, each with its comment's lines, whether a token
    /// precedes it on its first line and whether it leads the next token
    /// (three line lookups per marker comment, one more per crossing).
    fn peek(&mut self) -> Option<&Token> {
        let crossed = self.pending.len();
        while let Some(token) = self.tokens.get(self.pos) {
            if !token.kind.is_comment() {
                break;
            }
            let index = self.pos;
            if self
                .markers
                .get(self.next_marker)
                .is_some_and(|(at, _)| *at == index)
            {
                let comment = token.range.clone();
                let line = self.lines.line_of(comment.start);
                let end_line = self.token_line(index);
                let own_line = self.last.is_none_or(|last| self.token_line(last) < line);
                while self
                    .markers
                    .get(self.next_marker)
                    .is_some_and(|(at, _)| *at == index)
                {
                    let (_, marker) = self.markers[self.next_marker].clone();
                    self.pending.push(Pending {
                        marker,
                        comment: comment.clone(),
                        line,
                        end_line,
                        own_line,
                        leads: false,
                        fallback: None,
                    });
                    self.next_marker += 1;
                }
            }
            self.pos += 1;
        }
        // The comments just crossed all precede the same token.
        if self.pending.len() > crossed
            && let Some(next) = self
                .tokens
                .get(self.pos)
                .filter(|t| begins_value(t.kind))
                .map(|t| self.lines.line_of(t.range.start))
        {
            for pending in &mut self.pending[crossed..] {
                pending.leads = pending.end_line == next;
            }
        }
        self.tokens.get(self.pos)
    }

    /// 1-based line of the last byte of token `index`: where it ends (a
    /// string may span lines).
    fn token_line(&self, index: usize) -> usize {
        let range = &self.tokens[index].range;
        self.lines
            .line_of(range.end.saturating_sub(1).max(range.start))
    }

    /// The `n`-th non-comment token from the current position, not consumed.
    fn lookahead(&self, n: usize) -> Option<&Token> {
        self.lookahead_index(n).map(|i| &self.tokens[i])
    }

    fn lookahead_index(&self, n: usize) -> Option<usize> {
        self.tokens[self.pos..]
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.kind.is_comment())
            .nth(n)
            .map(|(offset, _)| self.pos + offset)
    }

    /// Consumes the next non-comment token. Markers still pending were not
    /// taken by a value: they are `unanchored`.
    fn bump(&mut self) {
        if self.peek().is_some() {
            self.last = Some(self.pos);
            self.pos += 1;
        }
        self.settle_pending(Anchor::Unanchored);
    }

    /// An entry, element or the root value begins at the next token (already
    /// peeked): it takes every pending marker whose comment is on its own
    /// line or leads it; any other pending comment trails a token no value
    /// claimed and is `unanchored`.
    fn anchor_pending(&mut self, path: &[Segment]) {
        self.take_pending(path, |p| p.own_line || p.leads);
        self.settle_pending(Anchor::Unanchored);
    }

    /// An entry's value begins at the next token, after the `:`: it takes the
    /// pending markers whose comments lead it. The others stay pending and
    /// settle `unanchored` with the value's first token (an own-line comment
    /// after a `:` included).
    fn anchor_leading(&mut self, path: &[Segment]) {
        self.peek();
        self.take_pending(path, |p| p.leads);
    }

    /// Resolves to `path` the pending markers `takes` selects; the others
    /// stay pending. The path is rendered only when one is taken.
    fn take_pending(&mut self, path: &[Segment], takes: impl Fn(&Pending) -> bool) {
        if !self.pending.iter().any(&takes) {
            return;
        }
        let anchor = render_path(path);
        let group = self.innermost_entry();
        let (taken, rest): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
            .into_iter()
            .partition(|p| takes(p));
        self.pending = rest;
        for pending in taken {
            self.resolve(pending, anchor.clone(), group);
        }
    }

    /// The value at `path` ends at, or opens with, token `end` (already
    /// consumed): the comments after it that start on the line where `end`
    /// ends bind to `path` — except one that leads the next token, which
    /// stays pending with `path` as its fallback. Crosses the comments up to
    /// the next token first; the others stay pending. The path is rendered
    /// only when a comment starts on that line.
    fn claim_trailing(&mut self, path: &[Segment], end: usize) {
        self.peek();
        if self.pending.is_empty() {
            return;
        }
        let line = self.token_line(end);
        if !self.pending.iter().any(|p| p.line == line) {
            return;
        }
        let anchor = render_path(path);
        let group = self.innermost_entry();
        for mut pending in std::mem::take(&mut self.pending) {
            if pending.line != line {
                self.pending.push(pending);
            } else if pending.leads {
                pending.fallback = Some((anchor.clone(), group));
                self.pending.push(pending);
            } else {
                self.resolve(pending, anchor.clone(), group);
            }
        }
    }

    /// Resolves every pending marker: to its fallback when it has one (it
    /// led a token no value began at), else to `anchor`.
    fn settle_pending(&mut self, anchor: Anchor) {
        for mut pending in std::mem::take(&mut self.pending) {
            let (anchor, group) = pending
                .fallback
                .take()
                .unwrap_or_else(|| (anchor.clone(), None));
            self.resolve(pending, anchor, group);
        }
    }

    fn resolve(&mut self, pending: Pending, anchor: Anchor, group: GroupId) {
        self.resolved.push((
            RonMarker {
                marker: pending.marker,
                line: pending.line,
                comment: pending.comment,
                anchor,
            },
            group,
        ));
    }

    /// A struct or map entry with this segment text begins in the innermost
    /// open container: it joins the group of an earlier sibling with the same
    /// text (which then collides) or starts one, and stays open until its
    /// value is done. One bounded lookup; the text is copied only for a new
    /// group.
    fn enter_entry(&mut self, segment: &str) {
        let parent = self.innermost_entry();
        let known = self
            .sibling_sets
            .last()
            .and_then(|set| set.get(segment).copied());
        let group = match known {
            Some(group) => {
                if !self.groups[group].collides {
                    self.groups[group].collides = true;
                    self.colliding_groups += 1;
                }
                group
            }
            None => {
                let group = self.groups.len();
                self.groups.push(Group {
                    parent,
                    collides: false,
                });
                if let Some(set) = self.sibling_sets.last_mut() {
                    set.insert(segment.to_owned(), group);
                }
                group
            }
        };
        self.open_entries.push(group);
    }

    /// The group of the innermost open struct or map entry.
    fn innermost_entry(&self) -> GroupId {
        self.open_entries.last().copied()
    }

    fn offset(&self) -> usize {
        self.tokens
            .get(self.pos)
            .map_or(self.source.len(), |t| t.range.start)
    }

    fn error(&mut self, kind: StructureErrorKind, offset: usize) {
        self.errors.push(StructureError { kind, offset });
    }
}
