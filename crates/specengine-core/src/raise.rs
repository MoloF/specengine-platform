//! Revisions at apply (ADR-0043; `docs/canon/node-revision.md` "Raising";
//! task spec `rev-discipline`, slice 4b-1): the pure half. Nothing is read,
//! written or hashed here: the CLI reads the file, hashes the meaning
//! inputs (`specengine_store::b3_hash`) and writes.
//!
//! - **Pairs** ([`paired`]): the nodes with an ID in two parses of one
//!   file, the k-th holder of an ID before with the k-th after, in the
//!   order of the second; a node in one parse only (a create's new
//!   section) has none.
//! - **A revision changed** ([`revision_change`]): creation's check 3 and
//!   apply's step 6 refuse a text changing a revision as [`revision`] reads
//!   it, the variant and its value both (`Unwritten` against `Written(1)`
//!   too): approval sets revisions, a proposal never.
//! - **Meaning changed** ([`meaning_changes`]): a pair whose meaning input
//!   ([`norm_input`]) differs; a null input on both sides compares the
//!   node's own text ([`own_spans`]) instead, so a change is never lost to
//!   a null hash.
//! - **The raise** ([`raised`]): the current revision plus one, never
//!   lowered; refused for an unreadable revision, one at [`REV_MAX`], a
//!   document of a closed class that admits no `rev`
//!   ([`closed_against_rev`]).
//! - **Writing** ([`write_revisions`]): a section's readable `rev=N`
//!   replaced in place, else ` rev=N` appended before its heading's `}`; a
//!   document's `rev:` value replaced, else `rev: N` inserted after its
//!   `id:` line with that line's ending. The result is parsed again and
//!   kept only when every raised node reads its new revision, every other
//!   node its old one, and the structure and every node's meaning input are
//!   as before: a revision written where the reader does not find it (a
//!   flow-style front-matter, an attribute block it cannot locate) is
//!   refused, never written half, the refusal naming each raised node
//!   whose revision cannot be written ([`RaiseRefusal::Unwritable`]).
//! - **Rows** ([`RevisionRow`]): the review document's `revisions`.
//!
//! Genre-neutral: keyed only on IDs as the parse gives them, meaning
//! inputs, revisions and `[classes]`; no prefix or kind literal.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::ops::Range;

use serde::Serialize;
use specengine_model::{IdScheme, ParsedFile, Span};

use crate::check::{Classes, DocClass};
use crate::front_matter::rev_sites;
use crate::lock::REV_MAX;
use crate::norm::norm_input;
use crate::own_text::own_spans;
use crate::patch::structure;
use crate::revision::{REV_KEY, Revision, revision};

/// One node with an ID in two parses of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paired {
    /// The Latin ID as the parse gives it (no `slug/`).
    pub id: String,
    /// Its position in the first parse.
    pub before: usize,
    /// Its position in the second.
    pub after: usize,
}

/// The nodes with an ID in both `before` and `after`, in `after`'s order:
/// the k-th node holding an ID in `before` paired with the k-th holding it
/// in `after`.
pub fn paired(before: &ParsedFile, after: &ParsedFile) -> Vec<Paired> {
    let mut holders: BTreeMap<&str, VecDeque<usize>> = BTreeMap::new();
    for (ord, node) in before.nodes.iter().enumerate() {
        if let Some(id) = node.id.as_deref() {
            holders.entry(id).or_default().push_back(ord);
        }
    }
    let mut pairs = Vec::new();
    for (ord, node) in after.nodes.iter().enumerate() {
        let Some(id) = node.id.as_deref() else {
            continue;
        };
        if let Some(was) = holders.get_mut(id).and_then(VecDeque::pop_front) {
            pairs.push(Paired {
                id: id.to_owned(),
                before: was,
                after: ord,
            });
        }
    }
    pairs
}

/// A revision as a message names it: `3`, `none written`, `unreadable`.
pub fn revision_text(rev: Revision) -> String {
    match rev {
        Revision::Written(rev) => rev.to_string(),
        Revision::Unwritten => "none written".to_owned(),
        Revision::Unreadable => "unreadable".to_owned(),
    }
}

/// A text that changes a node's revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionChange {
    pub id: String,
    /// Its position in the changed parse.
    pub after: usize,
    pub was: Revision,
    pub now: Revision,
}

impl RevisionChange {
    /// `` `<name>`'s revision changed (3 -> 4): keep rev as read; approval
    /// sets revisions ``, `name` the node's canonical ID.
    pub fn refusal(&self, name: &str) -> String {
        format!(
            "`{name}`'s revision changed ({} -> {}): keep rev as read; approval sets revisions",
            revision_text(self.was),
            revision_text(self.now)
        )
    }
}

/// The first node with an ID in both parses whose revision differs, the
/// variant and its value both compared; `None` when none does.
pub fn revision_change(before: &ParsedFile, after: &ParsedFile) -> Option<RevisionChange> {
    paired(before, after).into_iter().find_map(|pair| {
        let was = revision(before, pair.before);
        let now = revision(after, pair.after);
        (was != now).then_some(RevisionChange {
            id: pair.id,
            after: pair.after,
            was,
            now,
        })
    })
}

/// What a node's meaning is compared by.
#[derive(Debug, PartialEq, Eq)]
enum Meaning {
    /// Its meaning hash's input.
    Input(Vec<u8>),
    /// A null hash: its own text's pieces.
    Own(Vec<Vec<u8>>),
}

fn meaning(parsed: &ParsedFile, bytes: &[u8], ord: usize) -> Meaning {
    match norm_input(parsed, bytes, ord) {
        Some(input) => Meaning::Input(input),
        None => Meaning::Own(
            own_spans(parsed, ord)
                .into_iter()
                .map(|span| bytes.get(span.range()).unwrap_or_default().to_vec())
                .collect(),
        ),
    }
}

/// A node whose meaning a change alters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeaningChange {
    pub id: String,
    pub before: usize,
    pub after: usize,
    /// Its revision as read before the change.
    pub current: Revision,
}

/// The nodes with an ID in both parses ([`paired`]) whose meaning differs,
/// in `after`'s order; `before` and `after` the parses of `before_bytes`
/// and `after_bytes`.
pub fn meaning_changes(
    before: &ParsedFile,
    before_bytes: &[u8],
    after: &ParsedFile,
    after_bytes: &[u8],
) -> Vec<MeaningChange> {
    paired(before, after)
        .into_iter()
        .filter(|pair| {
            meaning(before, before_bytes, pair.before) != meaning(after, after_bytes, pair.after)
        })
        .map(|pair| MeaningChange {
            current: revision(before, pair.before),
            id: pair.id,
            before: pair.before,
            after: pair.after,
        })
        .collect()
}

/// Why a node's revision cannot be raised. An unreadable one refuses any
/// approve; the other three a meaning approve only, each message naming
/// `--editorial`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RaiseRefusal {
    /// Its current revision is unreadable.
    Unreadable,
    /// It is at [`REV_MAX`].
    AtMax,
    /// A document of a closed class whose contract admits no `rev`.
    ClosedClass { class: String },
    /// Its new revision cannot be written ([`write_revisions`]).
    Unwritable { why: String },
}

impl RaiseRefusal {
    /// The refusal, `name` the node's canonical ID.
    pub fn message(&self, name: &str) -> String {
        match self {
            Self::Unreadable => format!(
                "`{name}`'s revision is unreadable (a `bad-rev`, or a front-matter that fails): \
                 write a readable revision by hand and commit it, then approve"
            ),
            Self::AtMax => format!(
                "`{name}` is at rev {REV_MAX}, the largest revision: it cannot be raised; approve \
                 with --editorial"
            ),
            Self::ClosedClass { class } => format!(
                "`{name}`: class `{class}` is closed and does not admit `{REV_KEY}`: admit it in \
                 [classes], or approve with --editorial"
            ),
            Self::Unwritable { why } => format!(
                "`{name}`: its revision cannot be written: {why}; approve with --editorial to \
                 keep it"
            ),
        }
    }
}

impl fmt::Display for RaiseRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable => f.write_str("its revision is unreadable"),
            Self::AtMax => write!(f, "it is at rev {REV_MAX}"),
            Self::ClosedClass { class } => {
                write!(
                    f,
                    "class `{class}` is closed and does not admit `{REV_KEY}`"
                )
            }
            Self::Unwritable { why } => write!(f, "its revision cannot be written: {why}"),
        }
    }
}

/// The revision a meaning change raises `current` to: one more, never
/// lower; an unreadable one or one at [`REV_MAX`] refused.
pub fn raised(current: Revision) -> Result<u32, RaiseRefusal> {
    match current.number() {
        None => Err(RaiseRefusal::Unreadable),
        Some(rev) if rev >= REV_MAX => Err(RaiseRefusal::AtMax),
        Some(rev) => Ok(rev + 1),
    }
}

/// The class of `parsed`'s document when its contract in `classes` is
/// closed and lists `rev` in neither `required` nor `optional`: a raise
/// writing its `rev:` is refused. `None`: no class, an unknown one, an
/// open contract, or one admitting `rev`.
pub fn closed_against_rev(parsed: &ParsedFile, classes: &Classes) -> Option<String> {
    let class = parsed.document()?.fields.as_ref()?.class.as_deref()?;
    let contract = classes.get(DocClass::parse(class)?);
    (!contract.allows(REV_KEY)).then(|| class.to_owned())
}

/// Why a node's revision could not be written into a file: the node (its
/// position in the parse given, one of the raises) and what went wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionWriteError {
    pub ord: usize,
    pub why: String,
}

impl RevisionWriteError {
    fn at(ord: usize, why: impl Into<String>) -> Self {
        Self {
            ord,
            why: why.into(),
        }
    }
}

/// The file written with revisions: its bytes and their parse.
#[derive(Debug, Clone, PartialEq)]
pub struct Written {
    pub bytes: Vec<u8>,
    pub parsed: ParsedFile,
}

/// `bytes` (parsed as `parsed`, named `path`, under `scheme`) with each
/// `(ord, rev)` of `raises` written as node `ord`'s revision (see the module
/// documentation), parsed again and checked; `raises` empty: the bytes as
/// they are. Else each raised node whose revision cannot be written, in
/// `raises`' order: the raises that fail written alone; when each passes
/// alone and only their sum fails, every one of them, with the sum's
/// reason. A refusal never names a node that was not raised.
pub fn write_revisions(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    scheme: &IdScheme,
    raises: &[(usize, u32)],
) -> Result<Written, Vec<RevisionWriteError>> {
    if raises.is_empty() {
        return Ok(Written {
            bytes: bytes.to_vec(),
            parsed: parsed.clone(),
        });
    }
    let together = match written_checked(path, bytes, parsed, scheme, raises) {
        Ok(written) => return Ok(written),
        Err(why) => why,
    };
    let every = || {
        raises
            .iter()
            .map(|&(ord, _)| RevisionWriteError::at(ord, together.clone()))
            .collect::<Vec<_>>()
    };
    if raises.len() == 1 {
        return Err(every());
    }
    let alone: Vec<RevisionWriteError> = raises
        .iter()
        .filter_map(|&raise| {
            written_checked(path, bytes, parsed, scheme, &[raise])
                .err()
                .map(|why| RevisionWriteError::at(raise.0, why))
        })
        .collect();
    Err(if alone.is_empty() { every() } else { alone })
}

/// [`write_revisions`] of `raises` together: the file written and parsed
/// again, else why it is refused.
fn written_checked(
    path: &str,
    bytes: &[u8],
    parsed: &ParsedFile,
    scheme: &IdScheme,
    raises: &[(usize, u32)],
) -> Result<Written, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "the file is not UTF-8".to_owned())?;
    let mut edits: Vec<(Range<usize>, String)> = Vec::with_capacity(raises.len());
    for &(ord, rev) in raises {
        edits.push(if ord == 0 {
            document_edit(text, parsed, rev)?
        } else {
            section_edit(text, parsed, ord, rev)?
        });
    }
    // From the end, so earlier offsets hold.
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
    let mut out = text.to_owned();
    for (range, replacement) in edits {
        out.replace_range(range, &replacement);
    }
    let out = out.into_bytes();
    let reparsed = crate::parse(path, &out, scheme);
    if structure(parsed) != structure(&reparsed) {
        return Err("writing the revision changes the file's structure".to_owned());
    }
    for ord in 0..parsed.nodes.len() {
        let raise = raises.iter().find(|(raised, _)| *raised == ord);
        let wanted =
            raise.map_or_else(|| revision(parsed, ord), |(_, rev)| Revision::Written(*rev));
        let found = revision(&reparsed, ord);
        if found != wanted {
            let whose = if raise.is_some() {
                "the revision written"
            } else {
                "another node's revision"
            };
            return Err(format!(
                "{whose} reads back as {}, not {}",
                revision_text(found),
                revision_text(wanted)
            ));
        }
        if norm_input(parsed, bytes, ord) != norm_input(&reparsed, &out, ord) {
            let whose = if raise.is_some() {
                "the node's"
            } else {
                "another node's"
            };
            return Err(format!(
                "writing the revision changes {whose} meaning input"
            ));
        }
    }
    Ok(Written {
        bytes: out,
        parsed: reparsed,
    })
}

/// A section's `rev=N` replaced in place, else ` rev=N` appended before its
/// heading's `}`.
fn section_edit(
    text: &str,
    parsed: &ParsedFile,
    ord: usize,
    rev: u32,
) -> Result<(Range<usize>, String), String> {
    let node = parsed
        .nodes
        .get(ord)
        .ok_or_else(|| "no node at this position".to_owned())?;
    let heading: Span = node
        .heading
        .ok_or_else(|| "the section has no heading".to_owned())?;
    let line = text
        .get(heading.range())
        .ok_or_else(|| "the heading is no range of the file".to_owned())?;
    // The attribute block is the heading text's last `{…}`: its content
    // holds no brace (the reader's rule).
    let open = line
        .rfind('{')
        .ok_or_else(|| "its heading has no attribute block".to_owned())?;
    let close = open
        + line[open..]
            .find('}')
            .ok_or_else(|| "its heading's attribute block does not close".to_owned())?;
    let content_start = heading.start + open + 1;
    let content = &line[open + 1..close];
    let mut found: Vec<Range<usize>> = Vec::new();
    let mut at = 0;
    for token in content.split(|c: char| c.is_ascii_whitespace()) {
        let start = at;
        at += token.len() + 1;
        if let Some(value) = token
            .strip_prefix(REV_KEY)
            .and_then(|rest| rest.strip_prefix('='))
        {
            let value_start = start + token.len() - value.len();
            found.push(content_start + value_start..content_start + start + token.len());
        }
    }
    match (found.as_slice(), revision(parsed, ord)) {
        ([value], Revision::Written(_)) => Ok((value.clone(), rev.to_string())),
        ([], Revision::Unwritten) => {
            let at = heading.start + close;
            Ok((at..at, format!(" {REV_KEY}={rev}")))
        }
        _ => Err("its heading's `rev` attribute cannot be located".to_owned()),
    }
}

/// A document's `rev:` value replaced, else `rev: N` inserted after its
/// `id:` line, with that line's ending.
fn document_edit(
    text: &str,
    parsed: &ParsedFile,
    rev: u32,
) -> Result<(Range<usize>, String), String> {
    let sites = rev_sites(text).ok_or_else(|| "its front-matter cannot be read".to_owned())?;
    match revision(parsed, 0) {
        Revision::Written(_) => {
            let span = sites
                .rev
                .ok_or_else(|| "its `rev:` value cannot be located".to_owned())?;
            let written = text.get(span.clone()).unwrap_or_default();
            let trimmed = written.trim_end();
            let digits = trimmed.trim_start();
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err("its `rev:` is not written as plain digits".to_owned());
            }
            let start = span.start + (trimmed.len() - digits.len());
            Ok((start..start + digits.len(), rev.to_string()))
        }
        Revision::Unwritten => {
            let end = sites
                .id_end
                .ok_or_else(|| "its `id:` line cannot be located".to_owned())?;
            let newline = text[end..]
                .find('\n')
                .map(|at| end + at)
                .ok_or_else(|| "its `id:` line has no line ending".to_owned())?;
            let ending = if text[..newline].ends_with('\r') {
                "\r\n"
            } else {
                "\n"
            };
            let at = newline + 1;
            Ok((at..at, format!("{REV_KEY}: {rev}{ending}")))
        }
        Revision::Unreadable => Err("its revision is unreadable".to_owned()),
    }
}

/// One row of the review document's `revisions`: a node whose meaning the
/// proposal changes, its revision now (`rev`, read from the file at the
/// preview), what a meaning approve raises it to (`to`; `None` when it is
/// refused), how many `spec.lock` entries an editorial one refreshes
/// (`lock_entries`: 0 without a lock; `None` when the lock cannot be read)
/// and why a meaning approve refuses to raise it (`refused`: step 6's
/// reason, [`RaiseRefusal::message`], naming `--editorial`; `None` when it
/// raises). Serialized in that key order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RevisionRow {
    pub id: String,
    pub rev: u32,
    pub to: Option<u32>,
    pub lock_entries: Option<usize>,
    pub refused: Option<String>,
}
