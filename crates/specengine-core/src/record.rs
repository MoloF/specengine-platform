//! A decision record made from the queue (task spec `decision-apply`,
//! "Data"): the project's `[decision_records]` table, the slots of its
//! template and their one-pass render, the record's ID and title, the
//! owner's choice, and the structure a rendered record must have. Pure:
//! nothing is read or written here.
//!
//! Domain-free (ADR-0008): the record's prefix, directory and shape come
//! only from the project's table and its tracked template; the engine
//! brings no template, no heading and no directory of its own. Its words
//! are the slot names, the decision class of the convention and the
//! `accepted` status (ADR-0022).
//!
//! - **Slots**: `{{name}}`, read once left to right; a value is never read
//!   again, so a value holding `{{id}}` stays that text. A `{{` that opens
//!   no slot is a template error. Free text (agent- or owner-written) goes
//!   in the body only, but the title, which the front-matter carries as a
//!   double-quoted string with `\` and `"` backslashed.
//! - **Structure** ([`record_defect`]): one readable front-matter, the
//!   decision class, `id:` the record's ID, `status: accepted`, no other
//!   ID defined, `canon:` read back as its slot.

use std::fmt;
use std::ops::Range;

use serde::ser::SerializeMap as _;
use serde::{Serialize, Serializer};
use serde_json::Value;
use specengine_model::grammar::{self, Canon};
use specengine_model::{CanonTarget, DiagnosticCode, IdScheme, ParsedFile, PrefixSpec};

use crate::check::DocClass;
use crate::front_matter;

/// The most bytes of a template.
pub const TEMPLATE_MAX_BYTES: usize = 64 * 1024;

/// The most bytes of a record's title (a longer one is cut and ends in
/// `…`).
pub const TITLE_MAX_BYTES: usize = 128;

/// The most bytes of `spec approve --canon`.
pub const CANON_MAX_BYTES: usize = 512;

/// The status every record made from the queue is written with.
pub const ACCEPTED: &str = "accepted";

/// The cut title's last character.
const ELLIPSIS: char = '…';

/// `[decision_records]` of `specengine.toml`: where a decision taken in the
/// queue is written and in what shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionRecords {
    /// An `[ids]` prefix of shape `number`, scope `project`.
    pub prefix: String,
    /// Root-relative directory of the records.
    pub dir: String,
    /// Root-relative template file.
    pub template: String,
}

/// One slot a template may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Slot {
    /// The record's ID.
    Id,
    /// `YYYY-MM-DD` of the run.
    Date,
    /// [`ACCEPTED`].
    Status,
    /// The section the record governs: `--canon`, else the first ID target.
    Canon,
    /// The ID targets, `[A, B]`.
    Targets,
    /// The proposal's ID.
    Proposal,
    /// The chosen label, or the answer's first line, normalised.
    Title,
    /// The chosen label, or the answer.
    Choice,
    /// The chosen option's effect.
    Effect,
    /// The chosen option's price, or what another answer costs.
    Cost,
    /// The item's summary (a question's text).
    Summary,
    /// Every option, one line each.
    Options,
    /// Every piece of evidence, one line each.
    Evidence,
    /// The owner's `--note`.
    Note,
    /// The owner's git identity.
    DecidedBy,
}

impl Slot {
    pub const ALL: [Self; 15] = [
        Self::Id,
        Self::Date,
        Self::Status,
        Self::Canon,
        Self::Targets,
        Self::Proposal,
        Self::Title,
        Self::Choice,
        Self::Effect,
        Self::Cost,
        Self::Summary,
        Self::Options,
        Self::Evidence,
        Self::Note,
        Self::DecidedBy,
    ];

    /// The name written between `{{` and `}}`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Date => "date",
            Self::Status => "status",
            Self::Canon => "canon",
            Self::Targets => "targets",
            Self::Proposal => "proposal",
            Self::Title => "title",
            Self::Choice => "choice",
            Self::Effect => "effect",
            Self::Cost => "cost",
            Self::Summary => "summary",
            Self::Options => "options",
            Self::Evidence => "evidence",
            Self::Note => "note",
            Self::DecidedBy => "decided_by",
        }
    }

    /// The slot named exactly `name`.
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|slot| slot.name() == name)
    }

    /// Text an agent or the owner wrote: the body's only, but the title.
    pub const fn is_free_text(self) -> bool {
        !matches!(
            self,
            Self::Id | Self::Date | Self::Status | Self::Canon | Self::Targets | Self::Proposal
        )
    }
}

/// A slot's values for one render. Lists are joined by LF (`targets`:
/// `[A, B]`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SlotValues {
    pub id: String,
    pub date: String,
    pub status: String,
    pub canon: String,
    pub targets: Vec<String>,
    pub proposal: String,
    pub title: String,
    pub choice: String,
    pub effect: String,
    pub cost: String,
    pub summary: String,
    pub options: Vec<String>,
    pub evidence: Vec<String>,
    pub note: String,
    pub decided_by: String,
}

/// A template's defect: its 1-based line and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for TemplateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TemplateError {}

/// A part of a template.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// Template text, copied.
    Text(Range<usize>),
    /// A slot; `true` inside the front-matter.
    Slot(Slot, bool),
}

/// A checked template: its text cut at its slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    text: String,
    pieces: Vec<Piece>,
}

impl Template {
    /// Reads `text` (see the module documentation): at most
    /// [`TEMPLATE_MAX_BYTES`], opening with a closed front-matter, every
    /// `{{` opening a known slot, no free text but the title in the
    /// front-matter.
    pub fn parse(text: &str) -> Result<Self, TemplateError> {
        if text.len() > TEMPLATE_MAX_BYTES {
            return Err(TemplateError {
                line: 1,
                message: format!(
                    "{} bytes; a template has at most {TEMPLATE_MAX_BYTES}",
                    text.len()
                ),
            });
        }
        let layout = front_matter::split(text);
        let Some(block) = layout.block else {
            return Err(TemplateError {
                line: 1,
                message: "a template opens with a front-matter (a `---` line, its keys, a \
                          closing `---` line), as the record it renders"
                    .to_owned(),
            });
        };
        let front_end = block.span.end;
        let mut pieces = Vec::new();
        let mut copied = 0;
        let mut at = 0;
        while let Some(found) = text[at..].find("{{") {
            let start = at + found;
            let rest = &text[start + 2..];
            let length = rest
                .bytes()
                .take_while(|byte| byte.is_ascii_lowercase() || *byte == b'_')
                .count();
            let name = &rest[..length];
            let closed = rest[length..].starts_with("}}");
            let line = line_of(text, start);
            let slot = match (closed, Slot::parse(name)) {
                (true, Some(slot)) => slot,
                (true, None) => {
                    return Err(TemplateError {
                        line,
                        message: format!("`{{{{{name}}}}}` is no slot: {}", known_slots()),
                    });
                }
                (false, _) => {
                    return Err(TemplateError {
                        line,
                        message: format!(
                            "`{{{{` opens no slot (a template holds no literal `{{{{`): {}",
                            known_slots()
                        ),
                    });
                }
            };
            let in_front = start < front_end;
            if in_front && slot.is_free_text() && slot != Slot::Title {
                return Err(TemplateError {
                    line,
                    message: format!(
                        "`{{{{{name}}}}}` is free text, and free text goes in the body only (the \
                         title excepted)"
                    ),
                });
            }
            pieces.push(Piece::Text(copied..start));
            pieces.push(Piece::Slot(slot, in_front));
            at = start + 2 + length + 2;
            copied = at;
        }
        pieces.push(Piece::Text(copied..text.len()));
        Ok(Self {
            text: text.to_owned(),
            pieces,
        })
    }

    /// The slots the template holds, in order (repeats kept).
    pub fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.pieces.iter().filter_map(|piece| match piece {
            Piece::Slot(slot, _) => Some(*slot),
            Piece::Text(_) => None,
        })
    }

    /// Whether the template holds `slot`.
    pub fn holds(&self, slot: Slot) -> bool {
        self.slots().any(|held| held == slot)
    }

    /// Whether the template's front-matter holds `slot`.
    pub fn holds_in_front_matter(&self, slot: Slot) -> bool {
        self.pieces.contains(&Piece::Slot(slot, true))
    }

    /// The record: one pass over the template, each slot replaced by its
    /// value, values never read again.
    pub fn render(&self, values: &SlotValues) -> String {
        let mut out = String::with_capacity(self.text.len() + 1024);
        for piece in &self.pieces {
            match piece {
                Piece::Text(range) => out.push_str(&self.text[range.clone()]),
                Piece::Slot(slot, in_front) => push_value(&mut out, *slot, *in_front, values),
            }
        }
        out
    }
}

/// `slot`'s value written into `out`.
fn push_value(out: &mut String, slot: Slot, in_front: bool, values: &SlotValues) {
    let text = match slot {
        Slot::Id => &values.id,
        Slot::Date => &values.date,
        Slot::Status => &values.status,
        Slot::Canon => &values.canon,
        Slot::Proposal => &values.proposal,
        Slot::Title if in_front => {
            out.push('"');
            for c in values.title.chars() {
                if matches!(c, '\\' | '"') {
                    out.push('\\');
                }
                out.push(c);
            }
            out.push('"');
            return;
        }
        Slot::Title => &values.title,
        Slot::Choice => &values.choice,
        Slot::Effect => &values.effect,
        Slot::Cost => &values.cost,
        Slot::Summary => &values.summary,
        Slot::Note => &values.note,
        Slot::DecidedBy => &values.decided_by,
        Slot::Targets => {
            out.push('[');
            out.push_str(&values.targets.join(", "));
            out.push(']');
            return;
        }
        Slot::Options => {
            out.push_str(&values.options.join("\n"));
            return;
        }
        Slot::Evidence => {
            out.push_str(&values.evidence.join("\n"));
            return;
        }
    };
    out.push_str(text);
}

/// The slot names, for a template error.
fn known_slots() -> String {
    let names: Vec<String> = Slot::ALL
        .iter()
        .map(|slot| format!("`{{{{{}}}}}`", slot.name()))
        .collect();
    format!("the slots are {}", names.join(", "))
}

/// The 1-based line of byte `offset` in `text`.
fn line_of(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

/// A record's title from `text` (a label, or an answer's first line):
/// whitespace runs one space, trimmed, at most [`TITLE_MAX_BYTES`] (a
/// longer one cut at a character and ending in `…`).
pub fn record_title(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.len() <= TITLE_MAX_BYTES {
        return joined;
    }
    let room = TITLE_MAX_BYTES - ELLIPSIS.len_utf8();
    let mut end = room;
    while !joined.is_char_boundary(end) {
        end -= 1;
    }
    let mut cut = joined[..end].trim_end().to_owned();
    cut.push(ELLIPSIS);
    cut
}

/// The record ID numbered `number` of a `number`-shape prefix: the prefix,
/// `-`, the number zero-padded to `width` (more digits past it).
pub fn record_id(prefix: &str, width: u32, number: u64) -> String {
    format!("{prefix}-{number:0width$}", width = width as usize)
}

/// The number of `id` when it is an ID of `spec`'s prefix or of one of its
/// `aliases_from`: the prefix, `-`, ASCII digits.
pub fn record_number(id: &str, spec: &PrefixSpec) -> Option<u64> {
    let (prefix, digits) = id.split_once('-')?;
    let ours = prefix == spec.prefix || spec.aliases_from.iter().any(|alias| alias == prefix);
    if !ours || digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// The highest number of `spec`'s prefix or its aliases defined in `files`
/// (a document's `id:`, a section's `{#ID}`) or listed in a document's
/// `aliases:`; 0 when none.
pub fn highest_record_number<'a>(
    files: impl IntoIterator<Item = &'a ParsedFile>,
    spec: &PrefixSpec,
) -> u64 {
    let mut highest = 0;
    for parsed in files {
        for node in &parsed.nodes {
            let ids = node.id.iter();
            let aliases = node
                .fields
                .as_ref()
                .and_then(|fields| fields.aliases.as_ref())
                .into_iter()
                .flatten();
            for id in ids.chain(aliases) {
                if let Some(number) = record_number(id, spec) {
                    highest = highest.max(number);
                }
            }
        }
    }
    highest
}

/// The owner's choice, as the queue stores it (JSON with one key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A discrepancy's option, by index: `{"option":1}`.
    Option(u64),
    /// A question's working answer: `{"working_answer":true}`.
    WorkingAnswer,
    /// Another answer to a question: `{"answer":"…"}`.
    Answer(String),
}

impl Choice {
    /// The choice of a stored JSON value; `None` for any other shape.
    pub fn from_json(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        if object.len() != 1 {
            return None;
        }
        let (key, value) = object.iter().next()?;
        match (key.as_str(), value) {
            ("option", Value::Number(number)) => number.as_u64().map(Self::Option),
            ("working_answer", Value::Bool(true)) => Some(Self::WorkingAnswer),
            ("answer", Value::String(text)) => Some(Self::Answer(text.clone())),
            _ => None,
        }
    }

    /// How the consent question names it: `option 1`, `the working answer`,
    /// `the given answer`.
    pub fn described(&self) -> String {
        match self {
            Self::Option(index) => format!("option {index}"),
            Self::WorkingAnswer => "the working answer".to_owned(),
            Self::Answer(_) => "the given answer".to_owned(),
        }
    }
}

impl Serialize for Choice {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Option(index) => map.serialize_entry("option", index)?,
            Self::WorkingAnswer => map.serialize_entry("working_answer", &true)?,
            Self::Answer(text) => map.serialize_entry("answer", text)?,
        }
        map.end()
    }
}

/// A character a record never carries: one the queue's terminal output
/// escapes (every C0 control but LF and TAB, CR included; DEL; every C1;
/// the bidirectional marks, embeddings, overrides and isolates U+061C,
/// U+200E, U+200F, U+202A–U+202E, U+2066–U+2069).
pub fn is_escaped(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            c,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

/// The first character of `text` a record never carries ([`is_escaped`]).
pub fn refused_char(text: &str) -> Option<char> {
    text.chars().find(|&c| is_escaped(c))
}

/// Why a rendered record does not have a record's structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordDefect {
    /// No front-matter, or one that does not read as a mapping.
    FrontMatter,
    /// `class:` is not the decision class (what it is).
    Class(Option<String>),
    /// `id:` is not the record's ID.
    Id(Option<String>),
    /// `status:` is not [`ACCEPTED`].
    Status(Option<String>),
    /// The IDs defined, when they are not exactly the record's.
    Definitions(Vec<String>),
    /// `canon:` does not read back as its slot's value.
    Canon,
}

impl fmt::Display for RecordDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shown = |value: &Option<String>| match value {
            Some(value) => format!("`{value}`"),
            None => "absent".to_owned(),
        };
        match self {
            Self::FrontMatter => {
                f.write_str("the record has no readable front-matter mapping at its start")
            }
            Self::Class(found) => write!(
                f,
                "the record's `class:` is {}, not `{}`",
                shown(found),
                DocClass::Decision.as_str()
            ),
            Self::Id(found) => write!(f, "the record's `id:` is {}", shown(found)),
            Self::Status(found) => {
                write!(
                    f,
                    "the record's `status:` is {}, not `{ACCEPTED}`",
                    shown(found)
                )
            }
            Self::Definitions(ids) => write!(
                f,
                "the record defines {}; it defines its own ID only",
                if ids.is_empty() {
                    "no ID".to_owned()
                } else {
                    ids.iter()
                        .map(|id| format!("`{id}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ),
            Self::Canon => f.write_str("the record's `canon:` does not read back as its slot"),
        }
    }
}

/// The first structural defect of `parsed`, a rendered record (see the
/// module documentation); `None` when it has a record's structure.
/// `canon`: the `canon` slot's value, read back from `canon:` when given.
pub fn record_defect(
    parsed: &ParsedFile,
    id: &str,
    canon: Option<&str>,
    scheme: &IdScheme,
) -> Option<RecordDefect> {
    let unreadable = parsed.diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code,
            DiagnosticCode::FrontmatterUnclosed
                | DiagnosticCode::FrontmatterYaml
                | DiagnosticCode::FrontmatterNotMapping
        )
    });
    let document = parsed.document();
    let fields = document.and_then(|node| node.fields.as_ref());
    let (Some(document), Some(fields), false) = (document, fields, unreadable) else {
        return Some(RecordDefect::FrontMatter);
    };
    if parsed.front_matter.is_none() {
        return Some(RecordDefect::FrontMatter);
    }
    if fields.class.as_deref().and_then(DocClass::parse) != Some(DocClass::Decision) {
        return Some(RecordDefect::Class(fields.class.clone()));
    }
    if document.id.as_deref() != Some(id) {
        return Some(RecordDefect::Id(document.id.clone()));
    }
    if fields.status.as_deref() != Some(ACCEPTED) {
        return Some(RecordDefect::Status(fields.status.clone()));
    }
    let defined: Vec<String> = parsed
        .nodes
        .iter()
        .filter_map(|node| node.id.clone())
        .collect();
    if defined != [id] {
        return Some(RecordDefect::Definitions(defined));
    }
    if let Some(canon) = canon
        && !reads_back_canon(parsed, canon, scheme)
    {
        return Some(RecordDefect::Canon);
    }
    None
}

/// `canon:` of `parsed` reads back as `canon` (a `canon:` value: `ID`,
/// `ID#SECTION`, `path#anchor`), wherever either was written; `false` when
/// `canon` is no such value or `parsed` has no `canon:`.
pub fn reads_back_canon(parsed: &ParsedFile, canon: &str, scheme: &IdScheme) -> bool {
    let expected = match grammar::parse_canon(canon, 0, scheme) {
        Some(Canon::Reference(found)) => CanonTarget::Reference(found.reference),
        Some(Canon::Path(path)) => CanonTarget::Path(path),
        None => return false,
    };
    let read = parsed
        .document()
        .and_then(|node| node.fields.as_ref())
        .and_then(|fields| fields.canon.clone());
    read.map(spanless) == Some(spanless(expected))
}

/// `target` without its source span (where it was written).
fn spanless(target: CanonTarget) -> CanonTarget {
    match target {
        CanonTarget::Reference(mut reference) => {
            reference.span = None;
            CanonTarget::Reference(reference)
        }
        CanonTarget::Path(mut path) => {
            path.span = None;
            CanonTarget::Path(path)
        }
    }
}

/// `canon:` of `parsed` as text: a reference as `[project:][scope/]ID[#SECTION]`,
/// a path as `path[#anchor]`; `None` without one.
pub fn canon_text(parsed: &ParsedFile) -> Option<String> {
    let canon = parsed.document()?.fields.as_ref()?.canon.as_ref()?;
    Some(match canon {
        CanonTarget::Reference(reference) => {
            let mut text = String::new();
            if let Some(project) = &reference.project {
                text.push_str(project);
                text.push(':');
            }
            if let Some(scope) = &reference.scope {
                text.push_str(scope);
                text.push('/');
            }
            text.push_str(&reference.id);
            if let Some(section) = &reference.section {
                text.push('#');
                text.push_str(section);
            }
            text
        }
        CanonTarget::Path(path) => match &path.anchor {
            Some(anchor) => format!("{}#{anchor}", path.path),
            None => path.path.clone(),
        },
    })
}
