//! `spec health`'s rows 4-5 (task spec `health-code`): what the
//! code says about the spec, read from the code index the call refreshes,
//! as `spec refs` reads it — one handle: the spec index refreshed
//! ([`indexed_in`]), the code index refreshed ([`refresh_code`],
//! [`CodeRefresh::require_complete`]), its stored files, markers and
//! mentions read at once (`code_refs()`); each stored ID judged once
//! ([`Corpus::judge`]). No walk of its own, no symbol or role row, no lock,
//! no git; the queue opened only as it is ([`SqliteQueue::open_existing`],
//! no schema step). Visibility, never a gate. The handle is kept open for
//! row 6 ([`CodeShare`], task spec `health-drift`): the call refreshes
//! each index once.
//!
//! - **Row 4, `assumes`**: per stored marker, whatever it binds: (a) an ID
//!   of the queue's form, any relation, judged against its item (`get` by
//!   the ID as written: `pending`, `as_assumed`, `otherwise`, `applied`,
//!   `rejected`, `unknown`); (b) an `@assumes` whose ID names nodes, one row
//!   per holder: `archived` (a Tier 3 document), else `in_question` (an
//!   `open` or `approved` item targets it), else unlisted. By the ID as
//!   written, path, line, column, holder.
//! - **Row 5, `unbound`**: per node a `[[health.bound]]` rule selects
//!   (core's [`bound_candidates`]), bound when a stored marker of one of
//!   the rule's relations maps ([`maps`], `spec unmapped`'s predicate) and
//!   names it; never by containment, never by a mention. Unbound when a
//!   selecting rule finds it so, listed with the markers that name it
//!   without binding and why (`homoglyph`, `unanchored`, `cannot_verify`,
//!   `relation`, the first that applies). By path, line.
//! - **Failures**: no `[code]`: both rows `off`, nothing opened; any other
//!   step that fails (a bad `[code]`, the index, a refresh, a gap of the
//!   code walk, a word the build does not know): both `cannot`, its CLI
//!   line less `spec: ` the cause; the queue's failures fail row 4 alone, a
//!   queue below this build's schema too (named; no step runs); no rule:
//!   row 5 `not_configured`; a bad `[health]`: row 5 `cannot`. Never an
//!   exit code: row 3 decides it.
//! - **Text**: rows 4-5's lines through [`escape_controls`]; a row 4 tail
//!   shows the choice and record for `as_assumed` and `otherwise` alone.
//! - **Notes**: the refreshes' notes and warnings, then a `<row>: <cause>`
//!   note per cause of a row that cannot be measured: stderr only.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Serialize, Serializer};
use serde_json::Value;
use specengine_code::Relation;
use specengine_core::check::{BoundCandidate, CheckInput, bound_candidates, is_tier3_file};
use specengine_core::proposal::prefix_clash;
use specengine_core::{BoundRule, CodeConfig, HealthConfig};
use specengine_model::grammar::split_superseded_by;
use specengine_store::{
    Choice, CodeRefs, KindFamily, Proposal, ProposalFilter, ProposalQueue as _, ProposalStatus,
    QUEUE_SCHEMA_VERSION, SqliteIndex, SqliteQueue, StoredBinding, StoredMarker,
};

use crate::code_index::{CodeRefresh, RON_AMBIGUOUS, known_frameworks, refresh_code};
use crate::corpus::indexed_in;
use crate::location::{OpenIndex, open_index};
use crate::project::{ProjectRoot, config_error};
use crate::proposals::{queue_below_schema, queue_cannot};
use crate::refs::{
    CANNOT_VERIFY, CodeCount, Corpus, FILE, ITEM, IdState, Judged, MODULE, NodeKey, RON,
    UNANCHORED, marker_words, no_code_table, stored_count, unknown_word,
};
use crate::symbols::{Place, maps, place};
use crate::verify::{CAUSE_DATA, unprefixed};
use crate::verify_rows::{CAUSE_CONFIG, CAUSE_WALK, RunCause};
use crate::{CliError, Env, Message, escape_controls, one_line, store_error};

/// Row 4's name, in its lines and notes.
const ASSUMES: &str = "assumes";
/// Row 5's name.
const UNBOUND: &str = "unbound";
/// Row 5 without a rule.
const NO_RULE: &str =
    "no [[health.bound]] rule: which nodes must be bound is the project's to declare";
/// A non-binding marker whose relation no failing rule lists.
const RELATION: &str = "relation";

/// How a row fared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Measured,
    /// No `[code]`: the code layer is off.
    Off,
    /// Row 5 without a `[[health.bound]]` rule.
    NotConfigured,
    Cannot,
}

impl RowState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Off => "off",
            Self::NotConfigured => "not_configured",
            Self::Cannot => "cannot",
        }
    }

    /// As a text line names it.
    const fn shown(self) -> &'static str {
        match self {
            Self::NotConfigured => "not configured",
            other => other.as_str(),
        }
    }
}

impl Serialize for RowState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One of rows 4-5: its state, the causes of any state but `measured`, and
/// what it counted when measured (never a count otherwise).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodeRow<C, R> {
    pub state: RowState,
    pub causes: Vec<String>,
    pub counts: Option<C>,
    pub rows: Option<Vec<R>>,
}

impl<C, R> CodeRow<C, R> {
    fn measured(counts: C, rows: Vec<R>) -> Self {
        Self {
            state: RowState::Measured,
            causes: Vec::new(),
            counts: Some(counts),
            rows: Some(rows),
        }
    }

    fn not_measured(state: RowState, causes: Vec<String>) -> Self {
        Self {
            state,
            causes,
            counts: None,
            rows: None,
        }
    }
}

/// How a marker's assumption fared (row 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssumedState {
    /// (a) `open`, or `approved` of a kind that does not decide.
    Pending,
    /// (a) decided on the working answer, or on the recommended option.
    AsAssumed,
    /// (a) decided on another answer or option.
    Otherwise,
    /// (a) `applied`, a kind that does not decide.
    Applied,
    /// (a) `rejected`.
    Rejected,
    /// (a) no item of that ID.
    Unknown,
    /// (b) the node's document is Tier 3.
    Archived,
    /// (b) an `open` or `approved` item targets the node.
    InQuestion,
}

impl AssumedState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::AsAssumed => "as_assumed",
            Self::Otherwise => "otherwise",
            Self::Applied => "applied",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
            Self::Archived => "archived",
            Self::InQuestion => "in_question",
        }
    }
}

impl Serialize for AssumedState {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Row 4's counts: the markers judged, the rows listed, one per state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct AssumesCounts {
    pub markers: usize,
    pub rows: usize,
    pub pending: usize,
    pub as_assumed: usize,
    pub otherwise: usize,
    pub applied: usize,
    pub rejected: usize,
    pub unknown: usize,
    pub archived: usize,
    pub in_question: usize,
}

/// One row 4 entry: a marker (and, for (b), one holder of its ID).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssumesRow {
    /// The ID as written.
    pub id: String,
    pub state: AssumedState,
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub relation: &'static str,
    /// (a)'s item; `None` for `unknown` and (b).
    pub proposal: Option<AssumedProposal>,
    /// (b)'s node; `None` for (a).
    pub node: Option<AssumedNode>,
}

/// The queue item an (a) marker names, in the model's words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssumedProposal {
    pub kind: &'static str,
    pub status: &'static str,
    /// The decision record's choice, its stored one-key JSON.
    pub choice: Option<Value>,
    pub record_id: Option<String>,
    pub record_path: Option<String>,
    /// A rejection's reason.
    pub reason: Option<String>,
}

/// The node a (b) marker assumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssumedNode {
    /// Canonical.
    pub id: String,
    pub path: String,
    pub line: usize,
    /// Its document's `status:` as written.
    pub status: Option<String>,
    /// `X` of `superseded-by X`.
    pub successor: Option<String>,
    /// The `open` and `approved` items targeting it, by ID number.
    pub proposals: Vec<String>,
}

/// Row 5's counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct UnboundCounts {
    pub rules: usize,
    pub selected: usize,
    pub bound: usize,
    pub unbound: usize,
}

/// One unbound node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnboundRow {
    pub id: String,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    /// The 1-based line its span starts on.
    pub line: usize,
    /// The rules finding it unbound, 1-based, in config order.
    pub rules: Vec<usize>,
    /// The mentions naming it.
    pub mentions: usize,
    /// The markers naming it without binding it, by path, line, column.
    pub markers: Vec<UnboundMarker>,
}

/// A marker naming an unbound node (its fix's node, for a look-alike).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnboundMarker {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub relation: &'static str,
    pub id_state: &'static str,
    /// `spec refs`' binding word.
    pub binding: &'static str,
    /// Why it binds nothing here.
    pub reason: &'static str,
}

/// Rows 4-5 as one call measured them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeRows {
    /// `spec refs`' count of the stored code files; `None` unless the rows
    /// read the index.
    pub code: Option<CodeCount>,
    pub assumes: CodeRow<AssumesCounts, AssumesRow>,
    pub unbound: CodeRow<UnboundCounts, UnboundRow>,
    /// The refreshes' notes and warnings, then a note per cause of a row
    /// that cannot be measured: stderr only.
    pub messages: Vec<Message>,
}

/// What rows 4-5 leave row 6 (task spec `health-drift`): the call's one
/// handle, or why there is none.
pub(crate) enum CodeShare {
    /// No `[code]`: nothing was opened; the rows' `off` cause.
    Off(String),
    /// `[code]` does not read: nothing was opened (plain verify's own read
    /// of it names the cause).
    Unread,
    /// A step of the handle failed (the data directory, either refresh,
    /// an unknown `[code] frameworks` name, the code walk's gap): rows 4-5's
    /// cause, as plain verify orders it.
    Failed(RunCause),
    /// The handle, both indexes refreshed and the code walk complete.
    Open(SharedIndex),
}

/// The handle rows 4-5 read, kept open, and the spec corpus it gave
/// (every file read again).
pub(crate) struct SharedIndex {
    pub index: SqliteIndex,
    pub input: CheckInput,
}

/// Rows 4-5 of `project`, whose config reads as `text` (row 3's bytes):
/// never an error, a step that fails is its rows' cause. With the handle
/// they read, for row 6.
pub(crate) fn code_rows(env: &Env, project: &ProjectRoot, text: &str) -> (CodeRows, CodeShare) {
    let label = project.config_label.as_str();
    let code = match CodeConfig::from_toml(text) {
        Ok(Some(code)) => Ok(code),
        Ok(None) => {
            let cause = cause_of(&no_code_table(label));
            let rows = CodeRows {
                code: None,
                assumes: CodeRow::not_measured(RowState::Off, vec![cause.clone()]),
                unbound: CodeRow::not_measured(RowState::Off, vec![cause.clone()]),
                messages: Vec::new(),
            };
            return (rows, CodeShare::Off(cause));
        }
        Err(error) => Err(cause_of(&config_error(label, &error))),
    };
    let relations: Vec<&str> = Relation::ALL.into_iter().map(Relation::as_str).collect();
    let defaults: Vec<&str> = Relation::ALL
        .into_iter()
        .filter(|relation| *relation != Relation::Assumes)
        .map(Relation::as_str)
        .collect();
    let rules = HealthConfig::from_toml(text, &project.config.scheme, &relations, &defaults)
        .map_err(|error| cause_of(&config_error(label, &error)));

    let mut share = CodeShare::Unread;
    let measured = code.and_then(|code| match open_code(env, project, &code) {
        Ok(opened) => {
            // The stored rows' read and what follows fail rows 4-5 alone:
            // row 6 reads its own selection.
            let measured = read_stored(&opened)
                .and_then(|read| measure(project, &read, rules.as_ref()))
                .map_err(|error| cause_of(&error));
            share = CodeShare::Open(SharedIndex {
                index: opened.index,
                input: opened.input,
            });
            measured
        }
        Err(cause) => {
            let text = cause.message.clone();
            share = CodeShare::Failed(cause);
            Err(text)
        }
    });
    let mut rows = match measured {
        Ok(rows) => rows,
        Err(cause) => CodeRows {
            code: None,
            assumes: CodeRow::not_measured(RowState::Cannot, vec![cause.clone()]),
            unbound: match &rules {
                Ok(config) if config.bound.is_empty() => not_configured(),
                Ok(_) => CodeRow::not_measured(RowState::Cannot, vec![cause]),
                Err(rule) => CodeRow::not_measured(RowState::Cannot, vec![rule.clone(), cause]),
            },
            messages: Vec::new(),
        },
    };
    let mut notes = cannot_notes(ASSUMES, &rows.assumes);
    notes.extend(cannot_notes(UNBOUND, &rows.unbound));
    rows.messages.extend(notes);
    (rows, share)
}

/// A `<row>: <cause>` note per cause of a row that cannot be measured.
fn cannot_notes<C, R>(name: &str, row: &CodeRow<C, R>) -> Vec<Message> {
    if row.state != RowState::Cannot {
        return Vec::new();
    }
    row.causes
        .iter()
        .map(|cause| Message::Note(format!("{name}: {cause}")))
        .collect()
}

/// Row 5 without a rule.
fn not_configured() -> CodeRow<UnboundCounts, UnboundRow> {
    CodeRow::not_measured(RowState::NotConfigured, vec![NO_RULE.to_owned()])
}

/// A failing step's CLI line less `spec: `.
fn cause_of(error: &CliError) -> String {
    error
        .message
        .strip_prefix("spec: ")
        .unwrap_or(&error.message)
        .to_owned()
}

/// The call's one handle, kept open for row 6: both indexes refreshed, the
/// code walk complete.
struct Opened {
    index: SqliteIndex,
    /// The spec index's input, each file read again (the judge's).
    input: CheckInput,
    db: PathBuf,
    slug: String,
    /// The refreshes' notes.
    messages: Vec<Message>,
    /// The names and files the code refresh skipped for not being UTF-8.
    skipped: usize,
}

/// What rows 4-5 read of the handle: its stored files, markers and
/// mentions, and their count.
struct CodeRead<'o> {
    opened: &'o Opened,
    stored: CodeRefs,
    code: CodeCount,
}

/// One database handle, as `spec refs` opens it: the spec index refreshed,
/// then the code index ([`CodeRefresh::require_complete`]: a gap fails). A
/// failing step's cause in plain verify's order (row 6 lists it as plain
/// would): `[code] frameworks` (1), the gap (3), any other (8).
fn open_code(env: &Env, project: &ProjectRoot, code: &CodeConfig) -> Result<Opened, RunCause> {
    let data = |error: CliError| RunCause::new(CAUSE_DATA, unprefixed(&error));
    let OpenIndex {
        mut index,
        db,
        slug,
    } = open_index(env, project).map_err(data)?;
    let mut messages = Vec::new();
    let input = indexed_in(&mut index, project, &mut messages, true).map_err(data)?;
    // `refresh_code` refuses an unknown name first: (1), not (8).
    known_frameworks(code, &project.config_label)
        .map_err(|error| RunCause::new(CAUSE_CONFIG, unprefixed(&error)))?;
    let refreshed: CodeRefresh = refresh_code(
        &mut index,
        &project.root,
        code,
        &project.config_label,
        &project.config.scheme,
        false,
    )
    .map_err(data)?;
    refreshed
        .require_complete()
        .map_err(|error| RunCause::new(CAUSE_WALK, unprefixed(&error)))?;
    messages.extend(refreshed.messages.iter().cloned());
    Ok(Opened {
        index,
        input,
        db,
        slug,
        messages,
        skipped: refreshed.report.skipped,
    })
}

/// The handle's stored rows, read at once (`code_refs()`).
fn read_stored(opened: &Opened) -> Result<CodeRead<'_>, CliError> {
    let stored = opened
        .index
        .code_refs()
        .map_err(store_error)?
        .unwrap_or_default();
    Ok(CodeRead {
        code: stored_count(&stored, opened.skipped),
        stored,
        opened,
    })
}

/// A stored marker with its relation, its judged ID, where it maps and
/// its binding as `spec refs` names it.
struct Marked<'s> {
    marker: &'s StoredMarker,
    relation: Relation,
    judged: Judged,
    /// [`maps`]' answer: `Some` when it binds the nodes its ID names.
    mapped: Option<Place<'s>>,
    binding: &'static str,
}

/// Rows 4-5 over what was read: every stored word checked first (one this
/// build does not write fails both rows), each stored ID judged once.
fn measure(
    project: &ProjectRoot,
    read: &CodeRead<'_>,
    rules: Result<&HealthConfig, &String>,
) -> Result<CodeRows, CliError> {
    let mut corpus = Corpus::new(
        &read.opened.input,
        &project.config.scheme,
        &project.config.paths,
    );
    let mut marked = Vec::with_capacity(read.stored.markers.len());
    for marker in &read.stored.markers {
        let relation = marker_words(marker)?.relation;
        let judged = corpus.judge(&marker.id);
        let mapped = maps(marker, &judged)?;
        let binding = binding_word(&marker.binding)?;
        marked.push(Marked {
            marker,
            relation,
            judged,
            mapped,
            binding,
        });
    }
    let mut mentions: BTreeMap<NodeKey, usize> = BTreeMap::new();
    for mention in &read.stored.mentions {
        place(&mention.binding)?;
        for key in corpus.judge(&mention.written).nodes {
            *mentions.entry(key).or_default() += 1;
        }
    }

    let assumes = match assumes(
        project,
        &read.opened.db,
        &read.opened.slug,
        &corpus,
        &marked,
    ) {
        Ok((counts, rows)) => CodeRow::measured(counts, rows),
        Err(cause) => CodeRow::not_measured(RowState::Cannot, vec![cause]),
    };
    let unbound = match rules {
        Ok(config) if config.bound.is_empty() => not_configured(),
        Ok(config) => {
            let (counts, rows) = unbound(&config.bound, &corpus, &marked, &mentions);
            CodeRow::measured(counts, rows)
        }
        Err(cause) => CodeRow::not_measured(RowState::Cannot, vec![cause.clone()]),
    };
    Ok(CodeRows {
        code: Some(read.code),
        assumes,
        unbound,
        messages: read.opened.messages.clone(),
    })
}

/// A stored binding as `spec refs` names it (a RON path's ambiguity
/// dropped: `ron`); a word this build does not write is an error.
fn binding_word(binding: &StoredBinding) -> Result<&'static str, CliError> {
    Ok(match binding.binding.as_str() {
        ITEM => ITEM,
        MODULE => MODULE,
        RON | RON_AMBIGUOUS => RON,
        UNANCHORED => UNANCHORED,
        CANNOT_VERIFY => CANNOT_VERIFY,
        FILE => FILE,
        other => return Err(unknown_word("binding", other)),
    })
}

// ------------------------------------------------------------------ row 4

/// Row 4: (a) the markers whose ID is of the queue's form, (b) the
/// `@assumes` whose ID names nodes; the queue as it is. `Err`: the queue's
/// cause, the row whole.
fn assumes(
    project: &ProjectRoot,
    db: &Path,
    slug: &str,
    corpus: &Corpus<'_>,
    marked: &[Marked<'_>],
) -> Result<(AssumesCounts, Vec<AssumesRow>), String> {
    if let Some(clash) = prefix_clash(&project.config.scheme) {
        return Err(format!("{}: {clash}", project.config_label));
    }
    let queued: Vec<&Marked<'_>> = marked
        .iter()
        .filter(|marked| marked.judged.state == IdState::Queue)
        .collect();
    let assumed: Vec<&Marked<'_>> = marked
        .iter()
        .filter(|marked| marked.relation == Relation::Assumes && marked.judged.state.names_nodes())
        .collect();
    let mut counts = AssumesCounts {
        markers: queued.len() + assumed.len(),
        ..AssumesCounts::default()
    };
    if counts.markers == 0 {
        return Ok((counts, Vec::new()));
    }
    let queue_failed = |error| cause_of(&queue_cannot(error));
    let queue = SqliteQueue::open_existing(db, slug).map_err(queue_failed)?;
    // A queue below this build's schema reads no current column, and Health
    // runs no step: named, row 4 whole (0: an index-only DB, empty).
    if let Some(queue) = &queue {
        let schema = queue.schema_version().map_err(queue_failed)?;
        if (1..QUEUE_SCHEMA_VERSION).contains(&schema) {
            return Err(queue_below_schema(schema));
        }
    }

    // (b): per canonical ID, the `open` and `approved` items targeting it.
    let mut under: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if let Some(queue) = queue.as_ref().filter(|_| !assumed.is_empty()) {
        let filter = ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Open, ProposalStatus::Approved],
        };
        let list = queue.list_readable(&filter).map_err(queue_failed)?;
        if let Some(row) = list.unreadable.first() {
            return Err(row.to_string());
        }
        for proposal in &list.proposals {
            for target in proposal.target_ids() {
                let ids = under.entry(target).or_default();
                if !ids.contains(&proposal.id) {
                    ids.push(proposal.id.clone());
                }
            }
        }
    }

    let mut rows = Vec::new();
    for marked in queued {
        let item = match &queue {
            Some(queue) => queue.get(&marked.marker.id).map_err(queue_failed)?,
            None => None,
        };
        let (state, proposal) = match &item {
            Some(item) => (decided(item), Some(assumed_proposal(item))),
            None => (AssumedState::Unknown, None),
        };
        rows.push(assumes_row(marked, state, proposal, None));
    }
    for marked in assumed {
        for &key in &marked.judged.nodes {
            let file = corpus.files[key.file];
            let Some(parsed) = &file.parsed else {
                continue;
            };
            let id = corpus.node_name(key).to_owned();
            let proposals = under.get(&id).cloned().unwrap_or_default();
            let state = if is_tier3_file(parsed) {
                AssumedState::Archived
            } else if !proposals.is_empty() {
                AssumedState::InQuestion
            } else {
                continue;
            };
            let status = parsed
                .document()
                .and_then(|document| document.fields.as_ref())
                .and_then(|fields| fields.status.clone());
            let successor = status
                .as_deref()
                .and_then(split_superseded_by)
                .map(|(_, successor)| successor.to_owned());
            let node = AssumedNode {
                id,
                path: file.path.clone(),
                line: corpus.node_line(key),
                status,
                successor,
                proposals,
            };
            rows.push(assumes_row(marked, state, None, Some(node)));
        }
    }
    fn holder(row: &AssumesRow) -> Option<(&str, usize)> {
        row.node
            .as_ref()
            .map(|node| (node.path.as_str(), node.line))
    }
    rows.sort_by(|a, b| {
        (&a.id, &a.path, a.line, a.column, holder(a)).cmp(&(
            &b.id,
            &b.path,
            b.line,
            b.column,
            holder(b),
        ))
    });
    counts.rows = rows.len();
    for row in &rows {
        let count = match row.state {
            AssumedState::Pending => &mut counts.pending,
            AssumedState::AsAssumed => &mut counts.as_assumed,
            AssumedState::Otherwise => &mut counts.otherwise,
            AssumedState::Applied => &mut counts.applied,
            AssumedState::Rejected => &mut counts.rejected,
            AssumedState::Unknown => &mut counts.unknown,
            AssumedState::Archived => &mut counts.archived,
            AssumedState::InQuestion => &mut counts.in_question,
        };
        *count += 1;
    }
    Ok((counts, rows))
}

fn assumes_row(
    marked: &Marked<'_>,
    state: AssumedState,
    proposal: Option<AssumedProposal>,
    node: Option<AssumedNode>,
) -> AssumesRow {
    AssumesRow {
        id: marked.marker.id.clone(),
        state,
        path: marked.marker.path.clone(),
        line: marked.marker.line as usize,
        column: marked.marker.column as usize,
        relation: marked.relation.as_str(),
        proposal,
        node,
    }
}

/// The row 4 judge (pure: an item in, a state out), in the model's words:
/// its status, whether its kind decides, its record's choice against its
/// recommendation (indices, never labels).
fn decided(item: &Proposal) -> AssumedState {
    let decides = item.kind.family() == KindFamily::Decides;
    match item.status {
        ProposalStatus::Open => AssumedState::Pending,
        ProposalStatus::Rejected => AssumedState::Rejected,
        ProposalStatus::Approved | ProposalStatus::Applied if decides => {
            let recommended = item
                .intake
                .as_ref()
                .and_then(|intake| intake.recommendation);
            match item.record.as_ref().map(|record| &record.choice) {
                Some(Choice::WorkingAnswer) => AssumedState::AsAssumed,
                Some(Choice::Option(index)) if Some(*index) == recommended => {
                    AssumedState::AsAssumed
                }
                _ => AssumedState::Otherwise,
            }
        }
        ProposalStatus::Approved => AssumedState::Pending,
        ProposalStatus::Applied => AssumedState::Applied,
    }
}

/// An item as row 4 shows it.
fn assumed_proposal(item: &Proposal) -> AssumedProposal {
    let record = item.record.as_ref();
    AssumedProposal {
        kind: item.kind.as_str(),
        status: item.status.as_str(),
        choice: record.and_then(|record| serde_json::to_value(&record.choice).ok()),
        record_id: record.map(|record| record.id.clone()),
        record_path: record.map(|record| record.path.clone()),
        reason: (item.status == ProposalStatus::Rejected)
            .then(|| item.decision_note.clone())
            .flatten(),
    }
}

// ------------------------------------------------------------------ row 5

/// Row 5 over the selected nodes: bound for a rule when a marker of one of
/// its relations maps and names the node; unbound when a selecting rule
/// finds it so. Rows by path, line; markers by path, line, column.
fn unbound(
    rules: &[BoundRule],
    corpus: &Corpus<'_>,
    marked: &[Marked<'_>],
    mentions: &BTreeMap<NodeKey, usize>,
) -> (UnboundCounts, Vec<UnboundRow>) {
    let candidates: Vec<BoundCandidate> = bound_candidates(&corpus.files, rules);
    // Per node, the relations of the markers binding it.
    let mut binding: BTreeMap<NodeKey, BTreeSet<&str>> = BTreeMap::new();
    for marked in marked.iter().filter(|marked| marked.mapped.is_some()) {
        for &key in &marked.judged.nodes {
            binding
                .entry(key)
                .or_default()
                .insert(marked.relation.as_str());
        }
    }
    let mut rows: Vec<(NodeKey, UnboundRow)> = Vec::new();
    for candidate in &candidates {
        let key = NodeKey {
            file: candidate.file,
            ord: candidate.ord,
        };
        let relations = binding.get(&key);
        let failed: Vec<usize> = candidate
            .rules
            .iter()
            .copied()
            .filter(|&at| {
                !relations.is_some_and(|bound| {
                    rules[at]
                        .relations
                        .iter()
                        .any(|relation| bound.contains(relation.as_str()))
                })
            })
            .collect();
        if failed.is_empty() {
            continue;
        }
        let file = corpus.files[key.file];
        let node = file
            .parsed
            .as_ref()
            .and_then(|parsed| parsed.nodes.get(key.ord));
        rows.push((
            key,
            UnboundRow {
                id: corpus.node_name(key).to_owned(),
                kind: node.and_then(|node| node.kind.clone()),
                title: node.and_then(|node| node.title.clone()),
                path: file.path.clone(),
                line: corpus.node_line(key),
                rules: failed.iter().map(|at| at + 1).collect(),
                mentions: mentions.get(&key).copied().unwrap_or_default(),
                markers: Vec::new(),
            },
        ));
    }
    let at: BTreeMap<NodeKey, usize> = rows
        .iter()
        .enumerate()
        .map(|(index, (key, _))| (*key, index))
        .collect();
    for marked in marked {
        for key in &marked.judged.nodes {
            let Some(&index) = at.get(key) else {
                continue;
            };
            rows[index].1.markers.push(UnboundMarker {
                path: marked.marker.path.clone(),
                line: marked.marker.line as usize,
                column: marked.marker.column as usize,
                relation: marked.relation.as_str(),
                id_state: marked.judged.state.as_str(),
                binding: marked.binding,
                reason: reason(marked),
            });
        }
    }
    let mut rows: Vec<UnboundRow> = rows.into_iter().map(|(_, row)| row).collect();
    for row in &mut rows {
        row.markers
            .sort_by(|a, b| (&a.path, a.line, a.column).cmp(&(&b.path, b.line, b.column)));
    }
    rows.sort_by(|a, b| (&a.path, a.line, &a.id).cmp(&(&b.path, b.line, &b.id)));
    let counts = UnboundCounts {
        rules: rules.len(),
        selected: candidates.len(),
        bound: candidates.len() - rows.len(),
        unbound: rows.len(),
    };
    (counts, rows)
}

/// Why a marker naming an unbound node binds nothing there, the first that
/// applies: a look-alike ID, a binding to nothing, a broken item's, else a
/// relation no failing rule lists.
fn reason(marked: &Marked<'_>) -> &'static str {
    if marked.judged.state == IdState::Homoglyph {
        IdState::Homoglyph.as_str()
    } else if marked.binding == UNANCHORED {
        UNANCHORED
    } else if marked.binding == CANNOT_VERIFY {
        CANNOT_VERIFY
    } else {
        RELATION
    }
}

// ------------------------------------------------------------------ text

/// Rows 4-5's rows, before the summaries: row 4's, then row 5's, each
/// with its markers. Every line through [`escape_controls`] (row 3's
/// bytes are its own).
pub(crate) fn row_lines(rows: &CodeRows, out: &mut String) {
    for row in rows.assumes.rows.iter().flatten() {
        out.push_str(&escape_controls(&assumes_line(row)));
        out.push('\n');
    }
    for row in rows.unbound.rows.iter().flatten() {
        out.push_str(&escape_controls(&format!(
            "{UNBOUND} | {} | {} | {} | {}:{} | rules {} | mentions {}\n",
            one_line(&row.id),
            row.kind.as_deref().map_or_else(|| "-".to_owned(), one_line),
            row.title
                .as_deref()
                .map_or_else(|| "-".to_owned(), one_line),
            one_line(&row.path),
            row.line,
            row.rules
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            row.mentions
        )));
        for marker in &row.markers {
            out.push_str(&escape_controls(&format!(
                "  {} | {}:{}:{} | {} | {} | {}\n",
                marker.relation,
                one_line(&marker.path),
                marker.line,
                marker.column,
                marker.id_state,
                marker.binding,
                marker.reason
            )));
        }
    }
}

/// `assumes | <state> | <id> | <path>:<line>:<column> | <relation>[ |
/// <tail>]`.
fn assumes_line(row: &AssumesRow) -> String {
    let mut line = format!(
        "{ASSUMES} | {} | {} | {}:{}:{} | {}",
        row.state.as_str(),
        one_line(&row.id),
        one_line(&row.path),
        row.line,
        row.column,
        row.relation
    );
    if let Some(proposal) = &row.proposal {
        line.push_str(&format!(" | {} {}", proposal.kind, proposal.status));
        // A decided state alone shows the record: an item reopened after a
        // failed apply keeps its choice and record (JSON shows them).
        let decided = matches!(row.state, AssumedState::AsAssumed | AssumedState::Otherwise);
        if let Some(choice) = proposal.choice.as_ref().filter(|_| decided) {
            line.push_str(&format!(" {}", one_line(&choice.to_string())));
        }
        if let Some(reason) = &proposal.reason {
            line.push_str(&format!(": {}", one_line(reason)));
        }
        if let (true, Some(id), Some(path)) = (decided, &proposal.record_id, &proposal.record_path)
        {
            line.push_str(&format!(" | record {} {}", one_line(id), one_line(path)));
        }
    }
    if let Some(node) = &row.node {
        line.push_str(&format!(
            " | {} | {}:{}",
            one_line(&node.id),
            one_line(&node.path),
            node.line
        ));
        match row.state {
            AssumedState::InQuestion => {
                line.push_str(&format!(" | under {}", node.proposals.join(", ")));
            }
            _ => {
                if let Some(status) = &node.status {
                    line.push_str(&format!(" | status {}", one_line(status)));
                }
                if let Some(successor) = &node.successor {
                    line.push_str(&format!(" | successor {}", one_line(successor)));
                }
            }
        }
    }
    line
}

/// Row 4's and row 5's summary (or a `<row>: <state>: <cause>` line per
/// cause); `code:` is [`code_line`]'s, after row 6's summary.
pub(crate) fn summary_lines(rows: &CodeRows, out: &mut String) {
    match &rows.assumes.counts {
        Some(counts) if rows.assumes.state == RowState::Measured => {
            out.push_str(&format!(
                "{ASSUMES}: {} rows of {} markers: {} pending, {} as_assumed, {} otherwise, \
                 {} applied, {} rejected, {} unknown, {} archived, {} in_question\n",
                counts.rows,
                counts.markers,
                counts.pending,
                counts.as_assumed,
                counts.otherwise,
                counts.applied,
                counts.rejected,
                counts.unknown,
                counts.archived,
                counts.in_question
            ));
        }
        _ => not_measured_lines(ASSUMES, &rows.assumes, out),
    }
    match &rows.unbound.counts {
        Some(counts) if rows.unbound.state == RowState::Measured => {
            out.push_str(&format!(
                "{UNBOUND}: {} of {} selected nodes, {} rules\n",
                counts.unbound, counts.selected, counts.rules
            ));
        }
        _ => not_measured_lines(UNBOUND, &rows.unbound, out),
    }
}

/// `code:` when the index was read.
pub(crate) fn code_line(rows: &CodeRows, out: &mut String) {
    if let Some(code) = &rows.code {
        out.push_str(&format!(
            "code: {} files (.rs {}, .ron {}), skipped {}\n",
            code.files, code.rs, code.ron, code.skipped
        ));
    }
}

/// `<row>: <state>: <cause>`, one per cause, through [`escape_controls`].
fn not_measured_lines<C, R>(name: &str, row: &CodeRow<C, R>, out: &mut String) {
    for cause in &row.causes {
        out.push_str(&escape_controls(&format!(
            "{name}: {}: {}\n",
            row.state.shown(),
            one_line(cause)
        )));
    }
}
