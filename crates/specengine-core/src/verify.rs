//! The pure half of `spec verify`
//! (`docs/canon/spec-verify.md` "The `[verify]` table", "States",
//! "Verdict and exit"): the `[verify]` table read apart from the reads,
//! the row key, the facts one row is judged by, the judge, the classes and
//! the verdict (ADR-0042,
//! `docs/canon/architecture-code.md#verify`). Pure: facts in, a state out;
//! nothing here reads a file, a git object or the index, and the judge
//! never knows where its facts came from (a fresh walk and cascade, an
//! index's OIDs, an indexed hash), so every caller gets the same state for
//! the same facts.
//!
//! - **Table** ([`VerifyConfig::from_toml`]): closed, `mode` only
//!   (`observe`, the default; `enforce-introduced`; `enforce`); `tests`
//!   and `timeout` named as not built; a wrong type or value refused at
//!   its line. `ProjectConfig` only name-checks `verify`.
//! - **Key** ([`RowKey`]): a lock key ([`EntryKey`]) or a keyless
//!   binding's place, ordered as `EntryKey`, a keyless binding (no kind,
//!   symbol or item kind) first.
//! - **Judge** ([`judge_row`]): the first state of "States" that applies,
//!   with its category, detail and how the code was compared.
//! - **Verdict** ([`verdict`]): `cannot-verify` on any `cannot_verify`
//!   row or run cause, in every mode; `blocked` on a Fix row (under
//!   `enforce-introduced` one not Pre-existing); Check rows never block.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::check::Mode;
use crate::lock::{EntryKey, LockKind, LockRelation, Verified, key_problem};
use crate::project_toml::ProjectError;

/// The keys `[verify]` takes.
const KEYS: [&str; 1] = ["mode"];

/// `[verify]` keys of later slices (`tests`, `timeout`: slice 8a), named as
/// such when written.
const NOT_BUILT_KEYS: [&str; 2] = ["tests", "timeout"];

/// The `[verify]` table, checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifyConfig {
    /// `observe` when not written.
    pub mode: Mode,
}

impl Default for VerifyConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Observe,
        }
    }
}

impl VerifyConfig {
    /// Reads the `[verify]` table of `specengine.toml`; none: the default.
    pub fn from_toml(text: &str) -> Result<Self, ProjectError> {
        verify_from_toml(text)
    }
}

/// [`VerifyConfig::from_toml`] as a function.
pub fn verify_from_toml(text: &str) -> Result<VerifyConfig, ProjectError> {
    let error_at = |span: Option<Range<usize>>, message: String| ProjectError {
        line: span.map(|span| line_of(text, span.start)),
        message,
    };
    let raw: RawVerifyFile = toml::from_str(text)
        .map_err(|error| error_at(error.span(), error.message().trim().to_owned()))?;
    let Some(table) = raw.verify else {
        return Ok(VerifyConfig::default());
    };
    let mut config = VerifyConfig::default();
    for (key, value) in table.get_ref() {
        let span = Some(value.span());
        match key.as_str() {
            "mode" => {
                let toml::Value::String(written) = value.get_ref() else {
                    return Err(error_at(
                        span,
                        format!(
                            "`[verify] mode` is a {}, not \"observe\", \"enforce-introduced\" or \
                             \"enforce\"",
                            value.get_ref().type_str()
                        ),
                    ));
                };
                config.mode = Mode::ALL
                    .into_iter()
                    .find(|known| known.as_str() == written)
                    .ok_or_else(|| {
                        error_at(
                            span.clone(),
                            format!(
                                "`[verify] mode` `{written}` is not \"observe\", \
                                 \"enforce-introduced\" or \"enforce\""
                            ),
                        )
                    })?;
            }
            other if NOT_BUILT_KEYS.contains(&other) => {
                return Err(error_at(
                    span,
                    format!(
                        "`[verify] {other}` is not built: `[verify]` takes `{}` only",
                        KEYS.join("`, `")
                    ),
                ));
            }
            other => {
                return Err(error_at(
                    span,
                    format!(
                        "`[verify]` has no key `{other}`: it takes `{}` only",
                        KEYS.join("`, `")
                    ),
                ));
            }
        }
    }
    Ok(config)
}

/// Only `[verify]`, its keys read here.
#[derive(Deserialize)]
struct RawVerifyFile {
    #[serde(default)]
    verify: Option<Spanned<BTreeMap<String, Spanned<toml::Value>>>>,
}

fn line_of(text: &str, offset: usize) -> usize {
    let end = offset.min(text.len());
    text.as_bytes()[..end]
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

// ------------------------------------------------------------------- key

/// What a row's key binds: a lock key's binding, or none (a keyless
/// marker: `unanchored` or `cannot_verify`, with its category).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RowBinding {
    Keyed {
        kind: LockKind,
        symbol: String,
        item_kind: Option<String>,
    },
    Keyless {
        /// `unanchored` or `cannot_verify`.
        state: String,
        category: String,
    },
}

/// One row's key: a lock key, or a keyless binding's node, relation, file,
/// state and category.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowKey {
    /// The canonical cited ID; an unresolved marker's token.
    pub node: String,
    pub relation: LockRelation,
    pub file: String,
    pub binding: RowBinding,
}

impl RowKey {
    /// The row of a lock key.
    pub fn of_entry(key: &EntryKey) -> Self {
        Self {
            node: key.node.clone(),
            relation: key.relation,
            file: key.file.clone(),
            binding: RowBinding::Keyed {
                kind: key.kind,
                symbol: key.symbol.clone(),
                item_kind: key.item_kind.clone(),
            },
        }
    }

    /// The lock key; `None` for a keyless binding.
    pub fn entry_key(&self) -> Option<EntryKey> {
        match &self.binding {
            RowBinding::Keyed {
                kind,
                symbol,
                item_kind,
            } => Some(EntryKey {
                node: self.node.clone(),
                relation: self.relation,
                file: self.file.clone(),
                kind: *kind,
                symbol: symbol.clone(),
                item_kind: item_kind.clone(),
            }),
            RowBinding::Keyless { .. } => None,
        }
    }
}

impl Ord for RowKey {
    /// [`EntryKey`]'s order; a keyless binding (no kind) before every kind,
    /// keyless ones by state and category bytes.
    fn cmp(&self, other: &Self) -> Ordering {
        self.node
            .as_bytes()
            .cmp(other.node.as_bytes())
            .then_with(|| self.relation.cmp(&other.relation))
            .then_with(|| self.file.as_bytes().cmp(other.file.as_bytes()))
            .then_with(|| match (&self.binding, &other.binding) {
                (
                    RowBinding::Keyless { state, category },
                    RowBinding::Keyless {
                        state: other_state,
                        category: other_category,
                    },
                ) => state
                    .as_bytes()
                    .cmp(other_state.as_bytes())
                    .then_with(|| category.as_bytes().cmp(other_category.as_bytes())),
                (RowBinding::Keyless { .. }, RowBinding::Keyed { .. }) => Ordering::Less,
                (RowBinding::Keyed { .. }, RowBinding::Keyless { .. }) => Ordering::Greater,
                (
                    RowBinding::Keyed {
                        kind,
                        symbol,
                        item_kind,
                    },
                    RowBinding::Keyed {
                        kind: other_kind,
                        symbol: other_symbol,
                        item_kind: other_item_kind,
                    },
                ) => kind
                    .as_str()
                    .cmp(other_kind.as_str())
                    .then_with(|| symbol.as_bytes().cmp(other_symbol.as_bytes()))
                    .then_with(|| {
                        item_kind
                            .as_deref()
                            .map(str::as_bytes)
                            .cmp(&other_item_kind.as_deref().map(str::as_bytes))
                    }),
            })
    }
}

impl PartialOrd for RowKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// ----------------------------------------------------------------- facts

/// The node a row names, as the spec reads now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFact {
    /// One node: its revision R (`None`: unreadable) and its `norm_hash` H
    /// (`None`: null).
    Found {
        rev: Option<u32>,
        hash: Option<String>,
    },
    /// The ID names several nodes.
    Ambiguous,
    /// A marker's ID is a look-alike or of mixed script.
    Homoglyph,
    /// A marker's ID names no node.
    Dangling,
    /// A lock entry's `node` is no node's canonical ID (deleted, renamed,
    /// an alias now).
    Gone,
}

/// A marker's `@N`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerRev {
    /// No `@` after the ID.
    Unversioned,
    Written(u32),
    /// `@` not followed by 1-9 digits ending the token.
    Bad,
}

/// One marker giving the row's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarkerFact {
    /// 1-based, the line of its `@`.
    pub line: usize,
    pub rev: MarkerRev,
    /// Its level list is malformed.
    pub bad_levels: bool,
}

/// How a bound binding's code compares with its lock entry, as its
/// producer found it (the cascade, an index's OIDs, an indexed hash): the
/// judge never compares hashes itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparison {
    /// Equal to the entry, by the file's blob or by the item's hash.
    Equal(Compared),
    /// Changed since the entry, by the blob (`module`, `ron`) or by the
    /// item's hash after the blob differed.
    Changed(Compared),
    /// The item's hash refused: its first error category, snake case.
    CannotVerify(String),
    /// Not compared: no entry to compare with, or no hash made.
    NotHashed,
}

/// Why no marker gives a lock entry's key now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unbound {
    /// The file is absent.
    FileGone,
    /// The file is there but cannot be judged: `unreadable`, `not_utf8`,
    /// `outside_code`.
    Unreadable(String),
    /// The symbol is absent, no orphan parse error in the file.
    SymbolGone,
    /// The symbol is absent from a file with an orphan parse error.
    HiddenByError,
    /// The symbol names an item with a parse error, and a keyless
    /// `cannot_verify` marker of the entry's node and relation lies in the
    /// file: the item's first error category.
    ItemError(String),
    /// The symbol is there; no marker of the node binds it.
    MarkerGone,
}

/// The code a row names, now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeFact {
    /// Bound by the row's markers, compared with the row's entry.
    Bound(Comparison),
    /// Bound, but the binding is ambiguous: `path_attribute`, `unrooted`,
    /// `duplicate`.
    Ambiguous(String),
    /// A keyless marker binding nothing: its category.
    Unanchored(String),
    /// A keyless marker on an item with a parse error (its first error
    /// category), or in a RON container nested too deep.
    ItemError(String),
    /// No marker gives the key: why.
    Unbound(Unbound),
}

/// Everything one row is judged by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowFacts {
    pub key: RowKey,
    pub node: NodeFact,
    /// The row's lock entry E.
    pub entry: Option<Verified>,
    /// The markers giving the key, in file order; none for an entry alone.
    pub markers: Vec<MarkerFact>,
    pub code: CodeFact,
}

// ----------------------------------------------------------------- judge

/// A row's state, in the order "States" tries them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum State {
    CannotVerify,
    Predated,
    BadMarker,
    BrokenBinding,
    Unlocked,
    SpecAhead,
    CodeAhead,
    Conflict,
    Ok,
}

impl State {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CannotVerify => "cannot_verify",
            Self::Predated => "predated",
            Self::BadMarker => "bad_marker",
            Self::BrokenBinding => "broken_binding",
            Self::Unlocked => "unlocked",
            Self::SpecAhead => "spec_ahead",
            Self::CodeAhead => "code_ahead",
            Self::Conflict => "conflict",
            Self::Ok => "ok",
        }
    }

    /// The state's class; `None` for `ok`.
    pub const fn class(self) -> Option<Class> {
        match self {
            Self::CannotVerify => Some(Class::Cannot),
            Self::Predated | Self::BadMarker | Self::BrokenBinding | Self::Conflict => {
                Some(Class::Fix)
            }
            Self::Unlocked | Self::SpecAhead | Self::CodeAhead => Some(Class::Check),
            Self::Ok => None,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A finding's class: Fix (broken by the change), Check (a human
/// re-reads), Cannot (could not be verified).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    Fix,
    Check,
    Cannot,
}

impl Class {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fix => "fix",
            Self::Check => "check",
            Self::Cannot => "cannot",
        }
    }
}

/// How the code side was compared with the entry: by the file's blob
/// alone, or by the item's hash after the blob differed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Compared {
    Blob,
    Ast,
}

impl Compared {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Ast => "ast",
        }
    }
}

/// A row's judgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Judgement {
    pub state: State,
    pub category: Option<String>,
    pub detail: Option<String>,
    /// `None`: no entry, or the state was decided before the code was
    /// compared.
    pub compared: Option<Compared>,
}

impl Judgement {
    fn new(state: State, category: Option<&str>, detail: Option<&str>) -> Self {
        Self {
            state,
            category: category.map(str::to_owned),
            detail: detail.map(str::to_owned),
            compared: None,
        }
    }
}

/// The categories and details the judge gives.
pub const ITEM_ERROR: &str = "item_error";
pub const AMBIGUOUS_SYMBOL: &str = "ambiguous_symbol";
pub const AMBIGUOUS_ID: &str = "ambiguous_id";
pub const REV_UNKNOWN: &str = "rev_unknown";
pub const HASH_NULL: &str = "hash_null";
pub const UNREADABLE: &str = "unreadable";
pub const HIDDEN_BY_ERROR: &str = "hidden_by_error";
pub const DANGLING: &str = "dangling";
pub const HOMOGLYPH: &str = "homoglyph";
pub const BAD_REV: &str = "bad_rev";
pub const UNANCHORED: &str = "unanchored";
pub const BAD_LEVELS: &str = "bad_levels";
pub const NODE_GONE: &str = "node_gone";
pub const FILE_GONE: &str = "file_gone";
pub const SYMBOL_GONE: &str = "symbol_gone";
pub const MARKER_GONE: &str = "marker_gone";
pub const KEY_PROBLEM: &str = "key_problem";
pub const MARKER_BEHIND: &str = "marker_behind";
pub const SPEC_CHANGED: &str = "spec_changed";
pub const SAME_REVISION: &str = "same_revision";
pub const CODE_CHANGED: &str = "code_changed";
pub const RAISED: &str = "raised";

/// The row's state: the first of "States" that applies.
pub fn judge_row(facts: &RowFacts) -> Judgement {
    let found = match &facts.node {
        NodeFact::Found { rev, hash } => Some((*rev, hash.as_deref())),
        _ => None,
    };

    // cannot_verify
    if let CodeFact::ItemError(category) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }
    if facts.entry.is_some()
        && let CodeFact::Unbound(Unbound::ItemError(category)) = &facts.code
    {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }
    if let CodeFact::Ambiguous(reason) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(AMBIGUOUS_SYMBOL), Some(reason));
    }
    if facts.node == NodeFact::Ambiguous {
        return Judgement::new(State::CannotVerify, Some(AMBIGUOUS_ID), None);
    }
    if let Some((rev, hash)) = found {
        if rev.is_none() {
            return Judgement::new(State::CannotVerify, Some(REV_UNKNOWN), None);
        }
        if hash.is_none() {
            return Judgement::new(State::CannotVerify, Some(HASH_NULL), None);
        }
    }
    if let CodeFact::Unbound(Unbound::Unreadable(detail)) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(UNREADABLE), Some(detail));
    }
    if facts.entry.is_some() && facts.code == CodeFact::Unbound(Unbound::HiddenByError) {
        return Judgement::new(State::CannotVerify, Some(HIDDEN_BY_ERROR), None);
    }
    if let CodeFact::Bound(Comparison::CannotVerify(category)) = &facts.code {
        return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), Some(category));
    }

    // predated
    let rev = found.and_then(|(rev, _)| rev);
    let written = || {
        facts.markers.iter().filter_map(|marker| match marker.rev {
            MarkerRev::Written(n) => Some(n),
            MarkerRev::Unversioned | MarkerRev::Bad => None,
        })
    };
    if let Some(rev) = rev
        && written().any(|n| n > rev)
    {
        return Judgement::new(State::Predated, None, None);
    }

    // bad_marker
    match facts.node {
        NodeFact::Dangling => return Judgement::new(State::BadMarker, Some(DANGLING), None),
        NodeFact::Homoglyph => return Judgement::new(State::BadMarker, Some(HOMOGLYPH), None),
        _ => {}
    }
    if facts
        .markers
        .iter()
        .any(|marker| marker.rev == MarkerRev::Bad)
    {
        return Judgement::new(State::BadMarker, Some(BAD_REV), None);
    }
    if let CodeFact::Unanchored(category) = &facts.code {
        return Judgement::new(State::BadMarker, Some(UNANCHORED), Some(category));
    }
    if facts.markers.iter().any(|marker| marker.bad_levels) {
        return Judgement::new(State::BadMarker, Some(BAD_LEVELS), None);
    }

    // broken_binding
    if facts.entry.is_some() {
        if facts.node == NodeFact::Gone {
            return Judgement::new(State::BrokenBinding, Some(NODE_GONE), None);
        }
        let gone = match &facts.code {
            CodeFact::Unbound(Unbound::FileGone) => Some(FILE_GONE),
            CodeFact::Unbound(Unbound::SymbolGone) => Some(SYMBOL_GONE),
            CodeFact::Unbound(Unbound::MarkerGone) => Some(MARKER_GONE),
            _ => None,
        };
        if let Some(category) = gone {
            return Judgement::new(State::BrokenBinding, Some(category), None);
        }
    }

    // unlocked
    if facts
        .key
        .entry_key()
        .as_ref()
        .and_then(key_problem)
        .is_some()
    {
        return Judgement::new(State::Unlocked, None, Some(KEY_PROBLEM));
    }
    let lagging = rev.is_some_and(|rev| written().any(|n| n < rev));
    let Some(entry) = &facts.entry else {
        return if lagging {
            Judgement::new(State::SpecAhead, Some(MARKER_BEHIND), None)
        } else {
            Judgement::new(State::Unlocked, None, None)
        };
    };

    // The comparison: an entry, its node found, its binding bound.
    let (Some(rev), Some((_, Some(hash)))) = (rev, found) else {
        // Not reached: every other node or code fact is decided above.
        return Judgement::new(State::CannotVerify, None, None);
    };
    let (code_changed, compared) = match &facts.code {
        CodeFact::Bound(Comparison::Equal(by)) => (false, *by),
        CodeFact::Bound(Comparison::Changed(by)) => (true, *by),
        // An entry whose code was never compared is never `ok`.
        _ => return Judgement::new(State::CannotVerify, Some(ITEM_ERROR), None),
    };
    let spec_changed = (rev, hash) != (entry.rev, entry.spec_hash.as_str());
    let raised = rev > entry.rev && written().any(|n| n == rev);
    let mut judged = if lagging && !code_changed {
        Judgement::new(State::SpecAhead, Some(MARKER_BEHIND), None)
    } else if spec_changed && !raised && !code_changed {
        let category = if rev == entry.rev {
            SAME_REVISION
        } else {
            SPEC_CHANGED
        };
        Judgement::new(State::SpecAhead, Some(category), None)
    } else if code_changed && !spec_changed {
        Judgement::new(State::CodeAhead, Some(CODE_CHANGED), None)
    } else if spec_changed && raised {
        Judgement::new(State::CodeAhead, Some(RAISED), None)
    } else if spec_changed && code_changed {
        Judgement::new(State::Conflict, None, None)
    } else {
        Judgement::new(State::Ok, None, None)
    };
    judged.compared = Some(compared);
    judged
}

// --------------------------------------------------------------- verdict

/// What a run says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// Every row `ok`.
    Clean,
    /// Findings, none blocking.
    Observed,
    /// A Fix row the mode stops on.
    Blocked,
    /// A `cannot_verify` row or a run cause.
    CannotVerify,
}

impl Verdict {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Clean => "clean",
            Self::Observed => "observed",
            Self::Blocked => "blocked",
            Self::CannotVerify => "cannot-verify",
        }
    }
}

/// One judged row as the verdict reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finding {
    pub state: State,
    /// `HEAD` gives the same key, state and category.
    pub pre_existing: bool,
}

/// The verdict of a run in `mode` (the mode judged: without a base,
/// `enforce-introduced` is judged as `enforce` by the caller): a run cause
/// or a `cannot_verify` row wins in every mode; `enforce` blocks on any
/// Fix row, `enforce-introduced` on one not Pre-existing, `observe` never.
pub fn verdict(mode: Mode, run_causes: bool, rows: impl IntoIterator<Item = Finding>) -> Verdict {
    if run_causes {
        return Verdict::CannotVerify;
    }
    let mut verdict = Verdict::Clean;
    for row in rows {
        let blocks = match row.state.class() {
            Some(Class::Cannot) => return Verdict::CannotVerify,
            Some(Class::Fix) => match mode {
                Mode::Observe => false,
                Mode::EnforceIntroduced => !row.pre_existing,
                Mode::Enforce => true,
            },
            Some(Class::Check) => false,
            None => continue,
        };
        verdict = verdict.max(if blocks {
            Verdict::Blocked
        } else {
            Verdict::Observed
        });
    }
    verdict
}

/// Whether a row judged now is Pre-existing: not `ok`, and `head` (the
/// base's judgements by key) gives the same state and category.
pub fn is_pre_existing(
    key: &RowKey,
    judged: &Judgement,
    head: &BTreeMap<RowKey, Judgement>,
) -> bool {
    judged.state != State::Ok
        && head.get(key).is_some_and(|at_head| {
            at_head.state == judged.state && at_head.category == judged.category
        })
}
