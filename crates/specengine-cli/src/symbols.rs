//! `spec symbols` (canon `symbol-reads`, "Lookup", "spec symbols"):
//! the code index's targets for one of four forms, each with its direct
//! markers — a QUERY looked up in tiers (`exact`, `suffix`, `name`: the
//! store's B-tree searches, the name key from `specengine-code`'s
//! `query_name`), the items, modules and RON paths bound to the nodes a
//! REF names (`--node`), a file's items in source order (`--file`), the
//! items covering a line, shortest span first (`--at`). Read-only: the
//! refreshes write the data directory, nothing else is written.
//!
//! - **Order of work**, as `spec refs`: the form, the config (`[code]`,
//!   else exit 2), `--node`'s REF (every form `spec show` takes; naming
//!   nothing: an outcome with its reason, exit 1), the spec index, the code
//!   index's refresh (a gap of the walk: exit 2), a PATH the walk reads
//!   (else exit 2), the stored rows; each stored ID judged once
//!   ([`Corpus::judge`]).
//! - **Order of rows**: a QUERY's by tier, qpath, path, start line; a
//!   node's by path, line; a file's by `ord`; a line's by span. Never by
//!   storage order. Targets are printed while the text stays within
//!   [`OUTPUT_CAP_CHARS`], then `[truncated: <k> symbols not shown]`; the
//!   JSON holds exactly the printed targets.
//! - **One snapshot**: the stored markers and the symbols the form reads
//!   come from one read transaction ([`SqliteIndex::code_snapshot`]), so
//!   a refresh by another process between them cannot pair one state's
//!   markers with another's symbols.
//! - **Roles** (canon `code-roles`, "Links"; with `[code]
//!   frameworks` listed): the snapshot also reads the role rows beside its
//!   symbols ([`SqliteIndex::code_snapshot_with_roles`]); each item shows
//!   the rows sitting on it and the rows whose subject it is `linked` or
//!   `ambiguous` for ([`crate::roles::state`], computed per read), one
//!   line each after `sig` (counted in the cap with its target) and JSON
//!   `roles` after `bindings`; a module or a RON target `[]`. An
//!   `unlinked` row and a row on nothing show nowhere; a role changes no
//!   binding, count or exit. Without the list every target's `roles` is
//!   `null` and the text is as before.
//! - **Layer C** (canon `layer-c`, "Join"; only `spec resolve` makes a
//!   snapshot, a read never spawns or refreshes it): while one is stored,
//!   the same transaction reads it ([`SqliteIndex::code_snapshot_with_layer_c`]:
//!   its row, the code tree, the files the form's targets come from), its
//!   freshness computed from that read ([`LayerCView`]); each item shows a
//!   `c` line after `sig`, before its roles (state, shown moniker,
//!   agreement, `stale`; JSON `c_state`, `moniker`, `agrees` after `roles`),
//!   the summary a `layer c:` line after it (JSON `layer_c`), and a QUERY
//!   meets the shown monikers in a tier after `exact` (the raw moniker
//!   searched by the store, the tiers merged here: the store's lookup,
//!   shared with `spec impact` and role links, keeps its three). Without a
//!   snapshot the keys are `null`, the text as before and no moniker is
//!   searched; a snapshot holding a state this build does not know is
//!   shown as none, with one warning, the exit unchanged.
//! - **For the bridges**: [`symbols`] with [`SymbolsRequest`] (the form an
//!   enum, `--at` raw: the one `PATH:LINE` reader is here) and the limit
//!   bounds [`SYMBOLS_LIMIT_MIN`], [`SYMBOLS_LIMIT_MAX`],
//!   [`SYMBOLS_LIMIT_DEFAULT`] with their usage error
//!   [`symbols_limit_error`], so a caller needs no clap.
//!
//! [`SqliteIndex::code_snapshot`]: specengine_store::SqliteIndex::code_snapshot
//! [`SqliteIndex::code_snapshot_with_roles`]: specengine_store::SqliteIndex::code_snapshot_with_roles
//! [`SqliteIndex::code_snapshot_with_layer_c`]: specengine_store::SqliteIndex::code_snapshot_with_layer_c

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_code::layer_c::raw_moniker;
use specengine_code::{frameworks, query_name};
use specengine_core::Revision;
use specengine_store::{
    CodeFileState, CodeRefs, CodeSnapshot, LayerCItem, SnapshotSymbols, StoredBinding,
    StoredMarker, StoredRole, StoredSymbol, SymbolHit, SymbolTier,
};

use crate::cap::OUTPUT_CAP_CHARS;
use crate::code_index::{LANG_RON, LANG_RUST, LAYER, RON_AMBIGUOUS, refresh_code};
use crate::corpus::indexed_in;
use crate::layer_c::{ItemReading, LayerCState, LayerCView};
use crate::location::open_index;
use crate::project::discover_with_text;
use crate::refs::{
    CANNOT_VERIFY, CodeCount, Corpus, FILE, ITEM, Judged, MODULE, MarkerWords, NodeKey, RON,
    UNANCHORED, code_config, known_ambiguity, levels_text, marker_words, rev_state_of,
    stored_count, unknown_word,
};
use crate::roles::{RoleState, state};
use crate::search::notes;
use crate::show::classify;
use crate::{CliError, Env, Globals, Message, one_line, store_error};

/// `--limit`'s bounds and default (QUERY only).
pub const SYMBOLS_LIMIT_MIN: usize = 1;
pub const SYMBOLS_LIMIT_MAX: usize = 200;
pub const SYMBOLS_LIMIT_DEFAULT: usize = 20;

/// `--limit`'s usage error (exit 2): `limit` as given, outside
/// [`SYMBOLS_LIMIT_MIN`]..=[`SYMBOLS_LIMIT_MAX`]. A bridge whose limit
/// can be below zero answers it with this text too, so no caller words
/// it twice.
pub fn symbols_limit_error(limit: impl std::fmt::Display) -> CliError {
    CliError::spec(format!(
        "--limit {limit}: the limit is {SYMBOLS_LIMIT_MIN} to {SYMBOLS_LIMIT_MAX}"
    ))
}

/// Why a written `@rev` of `--node`'s REF is ignored (the note's end).
const REV_IGNORED: &str = "`spec symbols --node` lists every binding whatever its revision";

/// The words of `form`.
const FORM_QUERY: &str = "query";
const FORM_NODE: &str = "node";
const FORM_FILE: &str = "file";
const FORM_AT: &str = "at";

/// The one form of a `spec symbols` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolsForm {
    /// QUERY: its words as given, joined by a space; `--limit` (`None`:
    /// [`SYMBOLS_LIMIT_DEFAULT`]).
    Query {
        terms: Vec<String>,
        limit: Option<usize>,
    },
    /// `--node REF`: any form `spec show` takes.
    Node(String),
    /// `--file PATH`: a code file the walk reads, root-relative.
    File(String),
    /// `--at PATH:LINE`, raw: read here.
    At(String),
}

/// `spec symbols` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolsRequest {
    pub form: SymbolsForm,
}

/// What `spec symbols` found: the targets of the form, or the reason
/// `--node`'s REF names nothing (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolsOutcome {
    /// `query`, `node`, `file` or `at`.
    pub form: &'static str,
    /// QUERY's words joined by a space.
    pub query: Option<String>,
    /// `--node`'s REF as given.
    pub node: Option<String>,
    /// `--file`'s or `--at`'s PATH.
    pub path: Option<String>,
    /// `--at`'s LINE.
    pub line: Option<usize>,
    /// Why REF names nothing; `None` when answered.
    pub reason: Option<String>,
    pub messages: Vec<Message>,
    /// In order, within the limit; the cap is applied when rendering.
    pub symbols: Vec<SymbolTarget>,
    /// Every target the form names, the limit's cut included.
    pub total: usize,
    /// The code tree read; `None` when REF names nothing.
    pub code: Option<CodeCount>,
    /// The layer C snapshot shown; `None` without one, when REF names
    /// nothing, and when it holds a state this build does not know (a
    /// warning then says so).
    pub layer_c: Option<SymbolsLayerC>,
}

/// One target: an item, a file's module or a RON value path, every key
/// present (what the text does not print for a module or a RON path is
/// `null`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolTarget {
    /// `item`, `module` or `ron`.
    pub target: &'static str,
    /// An item's or a module's `qpath`; a RON path as `<path>#<field path>`.
    pub qpath: String,
    /// An item's bare last segment.
    pub name: Option<String>,
    /// An in-file twin's discriminator.
    pub discriminator: Option<String>,
    pub item_kind: Option<String>,
    pub path: String,
    /// An item's first line (attributes excluded); a RON path's first
    /// marker's line.
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    /// An item's display signature; `None` when it has none.
    pub signature: Option<String>,
    /// `path_attribute`, `unrooted` or `duplicate`: shown, not resolved.
    pub ambiguity: Option<&'static str>,
    pub layer: Option<String>,
    /// `exact`, `suffix` or `name`: QUERY only.
    pub tier: Option<&'static str>,
    /// The mentions bound to the item itself.
    pub mentions: Option<usize>,
    /// Its direct markers, by line and column.
    pub bindings: Vec<SymbolBinding>,
    /// The role rows shown on an item, by layer (`b` first), word, path,
    /// line, source, `ord`; `[]` on a module or a RON target; `None`
    /// without `[code] frameworks`.
    pub roles: Option<Vec<SymbolRole>>,
    /// An item's layer C state while a snapshot is stored: the joined
    /// row's (`moniker`, `local`, `none`, `unresolved`, `not_loaded`),
    /// `unmatched` or `unseen`; `None` without a snapshot and on a module
    /// or a RON target.
    pub c_state: Option<&'static str>,
    /// The shown moniker (`<raw> #<value>` for a collision): a `moniker`
    /// row's only.
    pub moniker: Option<String>,
    /// Whether the moniker agrees with layer A's crate and modules;
    /// `None` where neither is judged.
    pub agrees: Option<bool>,
}

/// One role row shown on an item: on it, or naming it (`linked`, each
/// candidate of an `ambiguous` row). Every key present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolRole {
    /// The row's word (the code crate's data).
    pub role: String,
    /// The layer whose rules made the row.
    pub layer: &'static str,
    /// The row's own file, as stored.
    pub path: String,
    /// The row's own line.
    pub line: usize,
    pub source: String,
    /// As stored; `None` on a row with no form.
    pub form: Option<String>,
    pub origin: String,
    /// The group stored with the row (`group_text`).
    pub group: Option<String>,
    /// `linked`, `ambiguous`, or the row's certainty
    /// ([`RoleState::word`]).
    pub state: &'static str,
    /// The stored category (an uncertain row's).
    pub detail: Option<String>,
    /// An `ambiguous` row's count of candidates; else `None`.
    pub candidates: Option<usize>,
    /// The row's `ord` in its file: the order's last key, not printed.
    #[serde(skip)]
    ord: u32,
}

impl SymbolRole {
    /// The order of the rows on one target: layer (`b` first), word, path
    /// bytes, line, source, `ord`; never storage order.
    fn order(&self) -> (Reverse<&str>, &str, &[u8], usize, &str, u32) {
        (
            Reverse(self.layer),
            self.role.as_str(),
            self.path.as_bytes(),
            self.line,
            self.source.as_str(),
            self.ord,
        )
    }
}

/// One direct marker of a target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolBinding {
    pub relation: &'static str,
    /// 1-based, of the marker's `@`.
    pub line: usize,
    /// 1-based, in bytes.
    pub column: usize,
    /// The ID and the revision as written.
    pub token: String,
    /// The ID as written.
    pub id: String,
    /// `@N`: 1 to 9 digits ending the token, else `None`.
    pub rev: Option<u32>,
    pub id_state: &'static str,
    /// The Latin form of a look-alike ID.
    pub fix: Option<String>,
    pub rev_state: &'static str,
    /// The levels the marker depends on; `None` when its list is invalid.
    pub levels: Option<Vec<&'static str>>,
    /// `default`, `declared` or the invalid list's category.
    pub levels_state: &'static str,
    pub note: Option<String>,
    /// The IDs of the nodes the marker is listed under (as `spec refs`
    /// lists it), `, `-joined; `None` for none.
    pub node: Option<String>,
}

/// `spec symbols`: reads the form, the config, `--node`'s REF, refreshes
/// both indexes and answers from the code index.
pub fn symbols(
    env: &Env,
    globals: &Globals,
    request: &SymbolsRequest,
) -> Result<SymbolsOutcome, CliError> {
    let form = Form::read(&request.form)?;
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let code = code_config(&project, &text)?;
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let mut outcome = form.outcome();

    let target = match &form {
        Form::Node(written) => {
            match classify(written.trim(), scheme, &mut messages, REV_IGNORED)? {
                Ok(target) => Some(target),
                Err(reason) => return Ok(outcome.not_found(reason, messages)),
            }
        }
        _ => None,
    };
    // One database handle for both refreshes.
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, false)?;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let nodes = match (&form, &target) {
        (Form::Node(written), Some(target)) => {
            let written = written.trim();
            match corpus.target_nodes(target, written)? {
                Ok(nodes) => {
                    if nodes.len() > 1 {
                        let cited: Vec<String> =
                            nodes.iter().map(|&key| corpus.citation(key)).collect();
                        messages.push(Message::Warning(format!(
                            "`{written}` has {} holders, all shown: {}",
                            nodes.len(),
                            cited.join(", ")
                        )));
                    }
                    nodes
                }
                Err(reason) => return Ok(outcome.not_found(reason, messages)),
            }
        }
        _ => Vec::new(),
    };

    let refreshed = refresh_code(
        &mut open.index,
        &project.root,
        &code,
        &project.config_label,
        scheme,
        false,
    )?;
    refreshed.require_complete()?;
    messages.extend(refreshed.messages.iter().cloned());
    // One snapshot: the markers and the symbols they bind from one read
    // transaction, whatever another connection writes meanwhile.
    let wanted = match &form {
        Form::Query { query, .. } => SnapshotSymbols::Lookup {
            query,
            name: query_name(query),
        },
        Form::File(path) | Form::At { path, .. } => SnapshotSymbols::File(path),
        Form::Node(_) => SnapshotSymbols::Bound,
    };
    // With frameworks listed, the role rows beside the symbols too, and
    // the layer C snapshot as this read joins it, in the same transaction:
    // a QUERY's raw moniker searched, the files its targets may come from
    // picked (never a spawn: only `spec resolve` makes a snapshot).
    let with_roles = !code.frameworks.is_empty();
    let kinds = with_roles.then(|| frameworks::item_kinds(&code.frameworks));
    let query = match &form {
        Form::Query { query, .. } => Some(query.as_str()),
        _ => None,
    };
    let node_set: BTreeSet<NodeKey> = nodes.into_iter().collect();
    let pick = |snapshot: &CodeSnapshot, found: &[LayerCItem]| -> BTreeSet<String> {
        match &form {
            // The merge only moves a hit up (to the moniker tier), so the
            // shown targets lie in the found items' files and the first
            // `limit` hits' files.
            Form::Query { limit, .. } => snapshot
                .hits
                .iter()
                .take(*limit)
                .map(|hit| hit.symbol.path.clone())
                .chain(found.iter().map(|item| item.path.clone()))
                .collect(),
            Form::File(path) | Form::At { path, .. } => BTreeSet::from([path.clone()]),
            Form::Node(_) => node_paths(&node_set, &snapshot.refs, &mut corpus),
        }
    };
    let snapshot =
        open.index
            .code_snapshot_with_layer_c(wanted, kinds.as_ref(), query.map(raw_moniker), pick);
    let CodeSnapshot {
        refs: stored,
        hits,
        symbols,
        roles,
        layer_c,
    } = snapshot.map_err(store_error)?.unwrap_or_default();
    // A stored state this build does not know: the snapshot is not shown,
    // one warning says why; nothing else changes.
    let layer_c = layer_c.and_then(|read| match LayerCView::of(read, &project.root, query) {
        Ok(view) => Some(view),
        Err(unknown) => {
            messages.push(Message::Warning(unknown.warning()));
            None
        }
    });
    let marks = Marks::new(&stored)?
        .with_roles(with_roles.then_some(roles.as_slice()))
        .with_layer_c(layer_c.as_ref());

    let (targets, total) = match &form {
        Form::Query { limit, .. } => {
            let found = layer_c
                .as_ref()
                .map_or(&[][..], |view| view.found.as_slice());
            let merged = merged_hits(hits, found);
            let total = merged.len();
            let targets = merged
                .into_iter()
                .take(*limit)
                .map(|(tier, symbol)| item_target(symbol, Some(tier), &marks, &mut corpus))
                .collect::<Result<Vec<_>, _>>()?;
            (targets, total)
        }
        Form::File(path) => {
            walked(&stored, path)?;
            let targets = symbols
                .into_iter()
                .map(|symbol| item_target(symbol, None, &marks, &mut corpus))
                .collect::<Result<Vec<_>, _>>()?;
            let total = targets.len();
            (targets, total)
        }
        Form::At { path, line } => {
            walked(&stored, path)?;
            let mut covering: Vec<StoredSymbol> = symbols
                .into_iter()
                .filter(|symbol| {
                    (symbol.start_line as usize) <= *line && *line <= symbol.end_line as usize
                })
                .collect();
            // Innermost first: an inner item spans fewer lines, else fewer
            // bytes, else (the same range) comes later in pre-order.
            covering.sort_by_key(|symbol| {
                (
                    symbol.end_line.saturating_sub(symbol.start_line),
                    symbol.end_byte.saturating_sub(symbol.start_byte),
                    std::cmp::Reverse(symbol.ord),
                )
            });
            let targets = covering
                .into_iter()
                .map(|symbol| item_target(symbol, None, &marks, &mut corpus))
                .collect::<Result<Vec<_>, _>>()?;
            let total = targets.len();
            (targets, total)
        }
        Form::Node(_) => {
            let targets = node_targets(&node_set, &stored, &marks, &symbols, &mut corpus)?;
            let total = targets.len();
            (targets, total)
        }
    };
    outcome.messages = messages;
    outcome.symbols = targets;
    outcome.total = total;
    outcome.code = Some(stored_count(&stored, refreshed.report.skipped));
    outcome.layer_c = layer_c.map(|view| SymbolsLayerC::of(&view));
    Ok(outcome)
}

/// A QUERY's tiers, the best first: the store's lookup tiers
/// ([`SymbolTier`]) with the moniker tier after `exact`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QueryTier {
    Exact,
    /// The item's shown layer C moniker is the query.
    Moniker,
    Suffix,
    Name,
}

/// The moniker tier's word.
const TIER_MONIKER: &str = "moniker";

impl QueryTier {
    fn of(tier: SymbolTier) -> Self {
        match tier {
            SymbolTier::Exact => Self::Exact,
            SymbolTier::Suffix => Self::Suffix,
            SymbolTier::Name => Self::Name,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => SymbolTier::Exact.as_str(),
            Self::Moniker => TIER_MONIKER,
            Self::Suffix => SymbolTier::Suffix.as_str(),
            Self::Name => SymbolTier::Name.as_str(),
        }
    }
}

/// The lookup's hits and the moniker tier's items merged: each item once,
/// at its best tier, by tier, qpath, path, start line, `ord` (the lookup's
/// own order within a tier). Without a moniker tier item: the hits as the
/// lookup ordered them.
fn merged_hits(hits: Vec<SymbolHit>, found: &[StoredSymbol]) -> Vec<(QueryTier, StoredSymbol)> {
    let mut best: BTreeMap<(String, u32), (QueryTier, StoredSymbol)> = BTreeMap::new();
    for hit in hits {
        let key = (hit.symbol.path.clone(), hit.symbol.ord);
        best.insert(key, (QueryTier::of(hit.tier), hit.symbol));
    }
    for symbol in found {
        let key = (symbol.path.clone(), symbol.ord);
        let entry = best
            .entry(key)
            .or_insert_with(|| (QueryTier::Moniker, symbol.clone()));
        entry.0 = entry.0.min(QueryTier::Moniker);
    }
    let mut merged: Vec<(QueryTier, StoredSymbol)> = best.into_values().collect();
    merged.sort_by(|a, b| merged_order(a).cmp(&merged_order(b)));
    merged
}

/// A merged hit's place: tier, qpath, path, start line, `ord`.
fn merged_order((tier, symbol): &(QueryTier, StoredSymbol)) -> (QueryTier, &str, &str, u32, u32) {
    (
        *tier,
        symbol.qpath.as_str(),
        symbol.path.as_str(),
        symbol.start_line,
        symbol.ord,
    )
}

/// The files of the items a marker listed under one of `nodes` binds (as
/// [`node_targets`] reads them): the files `--node` joins with layer C.
fn node_paths(
    nodes: &BTreeSet<NodeKey>,
    stored: &CodeRefs,
    corpus: &mut Corpus<'_>,
) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    for marker in &stored.markers {
        if !matches!(place(&marker.binding), Ok(Place::Item(_))) || paths.contains(&marker.path) {
            continue;
        }
        let judged = corpus.judge(&marker.id);
        if judged.nodes.iter().any(|key| nodes.contains(key)) {
            paths.insert(marker.path.clone());
        }
    }
    paths
}

/// The layer C snapshot a read showed: its freshness and the run's row
/// (canon `layer-c`, "Tables"), every key present.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolsLayerC {
    /// `fresh` or `stale`.
    pub state: &'static str,
    pub protocol: u32,
    pub ra_ap: String,
    pub host: String,
    /// The run's workspaces' distinct known toolchains, byte-sorted.
    pub toolchains: Vec<String>,
    pub degraded: bool,
    pub workspaces: u32,
    pub files: u64,
    pub items: u64,
}

impl SymbolsLayerC {
    fn of(view: &LayerCView) -> Self {
        let head = &view.head;
        Self {
            state: view.state.as_str(),
            protocol: head.stamp.protocol,
            ra_ap: head.stamp.ra_ap.clone(),
            host: head.stamp.host.clone(),
            toolchains: head.toolchains.clone(),
            degraded: head.degraded,
            workspaces: head.workspaces,
            files: head.files,
            items: head.items,
        }
    }
}

/// The request read: QUERY joined and its limit checked, `--at` split.
enum Form {
    Query { query: String, limit: usize },
    Node(String),
    File(String),
    At { path: String, line: usize },
}

impl Form {
    fn read(form: &SymbolsForm) -> Result<Self, CliError> {
        Ok(match form {
            SymbolsForm::Query { terms, limit } => {
                if terms.is_empty() {
                    return Err(CliError::spec(
                        "give a QUERY, --node REF, --file PATH or --at PATH:LINE",
                    ));
                }
                let limit = match *limit {
                    None => SYMBOLS_LIMIT_DEFAULT,
                    Some(limit) if (SYMBOLS_LIMIT_MIN..=SYMBOLS_LIMIT_MAX).contains(&limit) => {
                        limit
                    }
                    Some(limit) => return Err(symbols_limit_error(limit)),
                };
                Self::Query {
                    query: terms.join(" "),
                    limit,
                }
            }
            SymbolsForm::Node(reference) => Self::Node(reference.clone()),
            SymbolsForm::File(path) => Self::File(path.clone()),
            SymbolsForm::At(raw) => {
                let (path, line) = path_line(raw)?;
                Self::At { path, line }
            }
        })
    }

    /// The outcome echoing the request, nothing found yet.
    fn outcome(&self) -> SymbolsOutcome {
        let mut outcome = SymbolsOutcome {
            form: FORM_QUERY,
            query: None,
            node: None,
            path: None,
            line: None,
            reason: None,
            messages: Vec::new(),
            symbols: Vec::new(),
            total: 0,
            code: None,
            layer_c: None,
        };
        match self {
            Self::Query { query, .. } => outcome.query = Some(query.clone()),
            Self::Node(reference) => {
                outcome.form = FORM_NODE;
                outcome.node = Some(reference.clone());
            }
            Self::File(path) => {
                outcome.form = FORM_FILE;
                outcome.path = Some(path.clone());
            }
            Self::At { path, line } => {
                outcome.form = FORM_AT;
                outcome.path = Some(path.clone());
                outcome.line = Some(*line);
            }
        }
        outcome
    }
}

impl SymbolsOutcome {
    /// Exit 1: the reason on one line, the document kept.
    fn not_found(mut self, reason: String, messages: Vec<Message>) -> Self {
        self.reason = Some(one_line(&reason));
        self.messages = messages;
        self
    }
}

/// `PATH:LINE`, LINE from 1: split at the last `:`; else exit 2.
fn path_line(raw: &str) -> Result<(String, usize), CliError> {
    raw.rsplit_once(':')
        .filter(|(path, line)| {
            !path.is_empty() && !line.is_empty() && line.bytes().all(|b| b.is_ascii_digit())
        })
        .and_then(|(path, line)| {
            let line: usize = line.parse().ok()?;
            (line >= 1).then(|| (path.to_owned(), line))
        })
        .ok_or_else(|| {
            CliError::spec(format!(
                "`{raw}` is no PATH:LINE (a root-relative code file, a line from 1)"
            ))
        })
}

/// A stored Rust or RON file read whole and UTF-8: a file the walk reads.
pub(crate) fn is_read(file: &CodeFileState) -> bool {
    file.utf8 && file.read_error.is_none() && (file.lang == LANG_RUST || file.lang == LANG_RON)
}

/// The file at `path` if the walk reads it, else exit 2.
fn walked<'s>(stored: &'s CodeRefs, path: &str) -> Result<&'s CodeFileState, CliError> {
    stored
        .files
        .binary_search_by(|file| file.path.as_str().cmp(path))
        .ok()
        .and_then(|at| stored.files.get(at))
        .filter(|file| is_read(file))
        .ok_or_else(|| {
            CliError::spec(format!(
                "`{}` is no code file the `[code]` walk reads (a root-relative `.rs` or `.ron` file)",
                one_line(path)
            ))
        })
}

/// Where a stored marker or mention is bound, as the reads place it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place<'s> {
    /// An item (`item`), or an inline mod's module (`module` with its
    /// item): the item, by `ord`.
    Item(u32),
    /// The file's module (`module` without an item).
    FileModule,
    /// A RON value path (`ron`, `ron_ambiguous`): its field path.
    Ron(&'s str),
    /// The file itself (a mention's `file`).
    File,
    /// Nothing (`unanchored`, `cannot_verify`).
    Nothing,
}

/// The place of a stored binding; a word this build does not write is an
/// error (`spec index --full`).
pub(crate) fn place(binding: &StoredBinding) -> Result<Place<'_>, CliError> {
    match binding.binding.as_str() {
        ITEM => binding
            .symbol
            .as_ref()
            .map(|symbol| Place::Item(symbol.ord))
            .ok_or_else(|| unknown_word("binding without its item", ITEM)),
        MODULE => Ok(binding
            .symbol
            .as_ref()
            .map_or(Place::FileModule, |symbol| Place::Item(symbol.ord))),
        RON | RON_AMBIGUOUS => Ok(Place::Ron(binding.anchor.as_deref().unwrap_or_default())),
        FILE => Ok(Place::File),
        UNANCHORED | CANNOT_VERIFY => Ok(Place::Nothing),
        other => Err(unknown_word("binding", other)),
    }
}

/// Where a stored marker maps, the one predicate `spec unmapped` and `spec
/// health`'s row 5 share (task spec `health-code`, "The shared
/// predicate"): its [`place`] first (a word this build does not write is
/// an error whatever the ID), then that place when the judged ID names its
/// nodes itself (`IdState::names_nodes`: resolved, an alias or ambiguous)
/// and the place is something; else `None` (a dangling, look-alike, queue
/// or other-project ID; an `unanchored` or `cannot_verify` binding).
pub(crate) fn maps<'s>(
    marker: &'s StoredMarker,
    judged: &Judged,
) -> Result<Option<Place<'s>>, CliError> {
    let place = place(&marker.binding)?;
    Ok((judged.state.names_nodes() && place != Place::Nothing).then_some(place))
}

/// The stored markers by what they bind, and the mentions bound to each
/// item; each list in file order.
struct Marks<'s> {
    /// (path, ord) → the markers binding that item.
    items: BTreeMap<(&'s str, u32), Vec<&'s StoredMarker>>,
    /// path → the markers binding its file's module.
    modules: BTreeMap<&'s str, Vec<&'s StoredMarker>>,
    /// (path, field path) → the markers anchored to that RON value.
    ron: BTreeMap<(&'s str, &'s str), Vec<&'s StoredMarker>>,
    /// (path, ord) → the mentions bound to that item itself.
    mentions: BTreeMap<(&'s str, u32), usize>,
    /// (path, ord) → the role rows shown on that item, in
    /// [`SymbolRole::order`]; `None` without `[code] frameworks`.
    roles: Option<BTreeMap<(&'s str, u32), Vec<SymbolRole>>>,
    /// The layer C snapshot as the read joined it; `None` without one.
    layer_c: Option<&'s LayerCView>,
}

impl<'s> Marks<'s> {
    fn new(stored: &'s CodeRefs) -> Result<Self, CliError> {
        let mut marks = Self {
            items: BTreeMap::new(),
            modules: BTreeMap::new(),
            ron: BTreeMap::new(),
            mentions: BTreeMap::new(),
            roles: None,
            layer_c: None,
        };
        for marker in &stored.markers {
            let path = marker.path.as_str();
            match place(&marker.binding)? {
                Place::Item(ord) => marks.items.entry((path, ord)).or_default().push(marker),
                Place::FileModule => marks.modules.entry(path).or_default().push(marker),
                Place::Ron(field) => marks.ron.entry((path, field)).or_default().push(marker),
                Place::File | Place::Nothing => {}
            }
        }
        for mention in &stored.mentions {
            if let Place::Item(ord) = place(&mention.binding)? {
                *marks
                    .mentions
                    .entry((mention.path.as_str(), ord))
                    .or_default() += 1;
            }
        }
        Ok(marks)
    }

    /// The role rows of the snapshot (`None`: no frameworks listed) placed
    /// on the items they show on, each state computed now: a row without a
    /// subject on its `symbol_ord` (none: nowhere), a `linked` row on its
    /// candidate, an `ambiguous` row on each of its candidates, an
    /// `unlinked` row nowhere.
    fn with_roles(mut self, roles: Option<&'s [StoredRole]>) -> Self {
        self.roles = roles.map(|roles| {
            let mut shown: BTreeMap<(&'s str, u32), Vec<SymbolRole>> = BTreeMap::new();
            for role in roles {
                let found = state(role);
                let word = found.word();
                match &found {
                    RoleState::Certain | RoleState::Uncertain => {
                        if let Some(ord) = role.symbol_ord {
                            shown
                                .entry((role.path.as_str(), ord))
                                .or_default()
                                .push(symbol_role(role, word, None));
                        }
                    }
                    RoleState::Linked(candidate) => shown
                        .entry((candidate.symbol.path.as_str(), candidate.symbol.ord))
                        .or_default()
                        .push(symbol_role(role, word, None)),
                    RoleState::Ambiguous(candidates) => {
                        for candidate in candidates {
                            shown
                                .entry((candidate.symbol.path.as_str(), candidate.symbol.ord))
                                .or_default()
                                .push(symbol_role(role, word, Some(candidates.len())));
                        }
                    }
                    RoleState::Unlinked => {}
                }
            }
            for rows in shown.values_mut() {
                rows.sort_by(|a, b| a.order().cmp(&b.order()));
            }
            shown
        });
        self
    }

    /// The role rows shown on the item at (`path`, `ord`): `None` without
    /// frameworks listed.
    fn roles_on(&self, path: &str, ord: u32) -> Option<Vec<SymbolRole>> {
        self.roles
            .as_ref()
            .map(|shown| shown.get(&(path, ord)).cloned().unwrap_or_default())
    }

    /// A module's or a RON target's roles: `[]` with frameworks listed.
    fn no_roles(&self) -> Option<Vec<SymbolRole>> {
        self.roles.as_ref().map(|_| Vec::new())
    }

    /// The layer C snapshot the read joined (`None`: none stored).
    fn with_layer_c(mut self, view: Option<&'s LayerCView>) -> Self {
        self.layer_c = view;
        self
    }

    /// The layer C reading of the item at (`path`, `ord`): `None` without
    /// a snapshot.
    fn reading_on(&self, path: &str, ord: u32) -> Option<ItemReading> {
        self.layer_c.map(|view| view.reading(path, ord))
    }
}

/// One stored role row as a target shows it, `state` its word now,
/// `candidates` an `ambiguous` row's count.
fn symbol_role(role: &StoredRole, state: &'static str, candidates: Option<usize>) -> SymbolRole {
    SymbolRole {
        role: role.word.clone(),
        layer: LAYER,
        path: role.path.clone(),
        line: role.line as usize,
        source: role.source.clone(),
        form: role.form.clone(),
        origin: role.origin.clone(),
        group: role.group_text.clone(),
        state,
        detail: role.category.clone(),
        candidates,
        ord: role.ord,
    }
}

/// A target of `--node`, before it is read: what a marker binds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bound<'s> {
    Item(&'s str, u32),
    Module(&'s str),
    Ron(&'s str, &'s str),
}

/// The targets bound by a marker listed under one of `nodes` (as `spec
/// refs` lists it), each once, by path, line; an item found by binary
/// search in `bound_symbols`, the symbols the snapshot's markers are bound
/// to, by (path, ord).
fn node_targets(
    nodes: &BTreeSet<NodeKey>,
    stored: &CodeRefs,
    marks: &Marks<'_>,
    bound_symbols: &[StoredSymbol],
    corpus: &mut Corpus<'_>,
) -> Result<Vec<SymbolTarget>, CliError> {
    let mut bound: BTreeSet<Bound<'_>> = BTreeSet::new();
    for marker in &stored.markers {
        let judged = corpus.judge(&marker.id);
        if !judged.nodes.iter().any(|key| nodes.contains(key)) {
            continue;
        }
        let path = marker.path.as_str();
        match place(&marker.binding)? {
            Place::Item(ord) => bound.insert(Bound::Item(path, ord)),
            Place::FileModule => bound.insert(Bound::Module(path)),
            Place::Ron(field) => bound.insert(Bound::Ron(path, field)),
            Place::File | Place::Nothing => false,
        };
    }
    let mut targets = Vec::with_capacity(bound.len());
    for target in bound {
        targets.push(match target {
            Bound::Item(path, ord) => {
                let symbol = bound_symbols
                    .binary_search_by_key(&(path, ord), |symbol| (symbol.path.as_str(), symbol.ord))
                    .ok()
                    .and_then(|at| bound_symbols.get(at))
                    .cloned()
                    .ok_or_else(|| unknown_word("binding of a missing item", ITEM))?;
                item_target(symbol, None, marks, corpus)?
            }
            Bound::Module(path) => {
                let module = walked(stored, path)
                    .ok()
                    .and_then(|file| file.module.as_ref())
                    .ok_or_else(|| unknown_word("binding of a file without a name", MODULE))?;
                let markers = marks.modules.get(path).map_or(&[][..], Vec::as_slice);
                SymbolTarget {
                    bindings: bindings(markers, corpus)?,
                    roles: marks.no_roles(),
                    ..bare(MODULE, module.qpath.clone(), path, None)
                }
            }
            Bound::Ron(path, field) => {
                let markers = marks.ron.get(&(path, field)).map_or(&[][..], Vec::as_slice);
                let line = markers.iter().map(|marker| marker.line as usize).min();
                SymbolTarget {
                    bindings: bindings(markers, corpus)?,
                    roles: marks.no_roles(),
                    ..bare(RON, format!("{path}#{field}"), path, line)
                }
            }
        });
    }
    targets.sort_by(|a, b| node_order(a).cmp(&node_order(b)));
    Ok(targets)
}

/// A `--node` target's place: by path, then line (a file's module
/// first, a RON path at its first marker's line), then qpath.
fn node_order(target: &SymbolTarget) -> (&str, usize, bool, &str) {
    (
        target.path.as_str(),
        target.start_line.unwrap_or(0),
        target.target != MODULE,
        target.qpath.as_str(),
    )
}

/// A module's or a RON path's target: what the text prints, the rest
/// `None`.
fn bare(target: &'static str, qpath: String, path: &str, line: Option<usize>) -> SymbolTarget {
    SymbolTarget {
        target,
        qpath,
        name: None,
        discriminator: None,
        item_kind: None,
        path: path.to_owned(),
        start_line: line,
        end_line: None,
        signature: None,
        ambiguity: None,
        layer: None,
        tier: None,
        mentions: None,
        bindings: Vec::new(),
        roles: None,
        c_state: None,
        moniker: None,
        agrees: None,
    }
}

/// An item's target with its direct markers, mentions, roles and layer C
/// reading.
fn item_target(
    symbol: StoredSymbol,
    tier: Option<QueryTier>,
    marks: &Marks<'_>,
    corpus: &mut Corpus<'_>,
) -> Result<SymbolTarget, CliError> {
    let key = (symbol.path.as_str(), symbol.ord);
    let markers = marks.items.get(&key).map_or(&[][..], Vec::as_slice);
    let bindings = bindings(markers, corpus)?;
    let mentions = marks.mentions.get(&key).copied().unwrap_or(0);
    let roles = marks.roles_on(&symbol.path, symbol.ord);
    let reading = marks.reading_on(&symbol.path, symbol.ord);
    let ambiguity = symbol
        .ambiguity
        .as_deref()
        .map(known_ambiguity)
        .transpose()?;
    Ok(SymbolTarget {
        target: ITEM,
        qpath: symbol.qpath,
        name: Some(symbol.name),
        discriminator: symbol.discriminator,
        item_kind: Some(symbol.item_kind),
        path: symbol.path,
        start_line: Some(symbol.start_line as usize),
        end_line: Some(symbol.end_line as usize),
        signature: symbol.signature,
        ambiguity,
        layer: Some(symbol.layer),
        tier: tier.map(QueryTier::as_str),
        mentions: Some(mentions),
        bindings,
        roles,
        c_state: reading.as_ref().map(|reading| reading.state),
        agrees: reading.as_ref().and_then(|reading| reading.agrees),
        moniker: reading.and_then(|reading| reading.moniker),
    })
}

/// The binding rows of a target's direct markers, by line and column.
fn bindings(
    markers: &[&StoredMarker],
    corpus: &mut Corpus<'_>,
) -> Result<Vec<SymbolBinding>, CliError> {
    let mut rows = markers
        .iter()
        .map(|marker| binding(marker, corpus))
        .collect::<Result<Vec<_>, _>>()?;
    rows.sort_by_key(|row| (row.line, row.column));
    Ok(rows)
}

/// One marker's row: its ID judged (once per call), its revision against
/// the nodes it is listed under.
fn binding(marker: &StoredMarker, corpus: &mut Corpus<'_>) -> Result<SymbolBinding, CliError> {
    let MarkerWords {
        relation,
        levels,
        levels_state,
    } = marker_words(marker)?;
    let judged = corpus.judge(&marker.id);
    Ok(SymbolBinding {
        relation: relation.as_str(),
        line: marker.line as usize,
        column: marker.column as usize,
        token: marker.token.clone(),
        id: marker.id.clone(),
        rev: marker.rev,
        id_state: judged.state.as_str(),
        fix: judged.fix.clone(),
        rev_state: judged_rev_state(marker, &judged, corpus),
        levels,
        levels_state,
        note: marker.note.clone(),
        node: node_text(&judged, corpus),
    })
}

/// The marker's revision state over its nodes: theirs when they agree,
/// else (several holders of differing revisions) as against an unknown
/// revision; none: as against an unknown revision.
fn judged_rev_state(marker: &StoredMarker, judged: &Judged, corpus: &Corpus<'_>) -> &'static str {
    let state = |revision: Revision| {
        rev_state_of(
            &marker.id,
            marker.rev,
            &marker.token,
            judged.state,
            revision,
        )
    };
    let mut states: BTreeSet<&'static str> = judged
        .nodes
        .iter()
        .map(|&key| state(corpus.revision(key)))
        .collect();
    match (states.pop_first(), states.is_empty()) {
        (Some(one), true) => one,
        _ => state(Revision::Unreadable),
    }
}

/// The IDs of the nodes a marker is listed under, each once, `, `-joined.
fn node_text(judged: &Judged, corpus: &Corpus<'_>) -> Option<String> {
    let mut seen = BTreeSet::new();
    let names: Vec<&str> = judged
        .nodes
        .iter()
        .map(|&key| corpus.node_name(key))
        .filter(|name| seen.insert(*name))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

// ------------------------------------------------------------- rendering

/// A target's lines: its own, its signature, its layer C line (an item
/// while a snapshot is stored, `stale` its freshness) and its roles
/// (items), its bindings.
fn target_lines(target: &SymbolTarget, stale: bool) -> Vec<String> {
    let roles = target.roles.as_deref().unwrap_or_default();
    let mut lines = Vec::with_capacity(target.bindings.len() + roles.len() + 3);
    let qpath = one_line(&target.qpath);
    let path = one_line(&target.path);
    match target.target {
        ITEM => {
            let mut line = format!(
                "{qpath} | {} | {path}:{}-{} | layer {} | mentions {}",
                one_line(target.item_kind.as_deref().unwrap_or_default()),
                target.start_line.unwrap_or_default(),
                target.end_line.unwrap_or_default(),
                one_line(target.layer.as_deref().unwrap_or_default()),
                target.mentions.unwrap_or_default()
            );
            if let Some(reason) = target.ambiguity {
                line.push_str(&format!(" | ambiguous {reason}"));
            }
            if let Some(tier) = target.tier {
                line.push_str(&format!(" | {tier}"));
            }
            lines.push(line);
            lines.push(format!(
                "  sig {}",
                target
                    .signature
                    .as_deref()
                    .map_or_else(|| "-".to_owned(), one_line)
            ));
            lines.extend(c_line(target, stale));
            lines.extend(roles.iter().map(role_line));
        }
        RON => lines.push(format!(
            "{qpath} | {RON} | {path}:{}",
            target.start_line.unwrap_or_default()
        )),
        kind => lines.push(format!("{qpath} | {kind} | {path}")),
    }
    lines.extend(target.bindings.iter().map(binding_line));
    lines
}

/// The words of the `c` line's `agrees`.
const AGREES: &str = "agrees";
const DISAGREES: &str = "disagrees";

/// `  c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]`: an item's
/// layer C reading, `stale` while the snapshot is; none without a
/// snapshot. Every field through [`one_line`].
fn c_line(target: &SymbolTarget, stale: bool) -> Option<String> {
    let state = target.c_state?;
    let mut line = format!("  c {}", one_line(state));
    if let Some(moniker) = &target.moniker {
        line.push_str(" | ");
        line.push_str(&one_line(moniker));
    }
    match target.agrees {
        Some(true) => line.push_str(&format!(" | {AGREES}")),
        Some(false) => line.push_str(&format!(" | {DISAGREES}")),
        None => {}
    }
    if stale {
        line.push_str(&format!(" | {}", LayerCState::Stale.as_str()));
    }
    Some(line)
}

/// `  role <word> | <layer> | <path>:<line> | <source>[ <form>][ <origin>]
/// | <group or -> | <state>[ <detail>]`: the form unless the plain one
/// (a path), the origin unless ordinary code (the code crate's
/// [`frameworks::PATH_FORM`], [`frameworks::CODE_ORIGIN`]); the detail an
/// `ambiguous` row's count of candidates, else its category. Every stored
/// text through [`one_line`].
fn role_line(row: &SymbolRole) -> String {
    let mut source = one_line(&row.source);
    if let Some(form) = row
        .form
        .as_deref()
        .filter(|form| *form != frameworks::PATH_FORM)
    {
        source.push(' ');
        source.push_str(&one_line(form));
    }
    if row.origin != frameworks::CODE_ORIGIN {
        source.push(' ');
        source.push_str(&one_line(&row.origin));
    }
    let detail = match row.candidates {
        Some(count) => Some(count.to_string()),
        None => row.detail.as_deref().map(one_line),
    };
    let mut line = format!(
        "  role {} | {} | {}:{} | {source} | {} | {}",
        one_line(&row.role),
        row.layer,
        one_line(&row.path),
        row.line,
        row.group
            .as_deref()
            .map_or_else(|| "-".to_owned(), one_line),
        row.state
    );
    if let Some(detail) = detail {
        line.push(' ');
        line.push_str(&detail);
    }
    line
}

/// `  <relation> | <line>:<column> | <token> | <id_state>[ <fix>] |
/// <rev_state> | <levels> | <node or ->[ | <note>]`.
fn binding_line(row: &SymbolBinding) -> String {
    let fix = row
        .fix
        .as_deref()
        .map(|fix| format!(" {}", one_line(fix)))
        .unwrap_or_default();
    let mut line = format!(
        "  {} | {}:{} | {} | {}{fix} | {} | {} | {}",
        row.relation,
        row.line,
        row.column,
        one_line(&row.token),
        row.id_state,
        row.rev_state,
        levels_text(row.levels.as_deref(), row.levels_state),
        row.node.as_deref().map_or_else(|| "-".to_owned(), one_line)
    );
    if let Some(note) = &row.note {
        line.push_str(" | ");
        line.push_str(&one_line(note));
    }
    line
}

/// How many targets the text shows: whole targets while the text before
/// the tail stays within [`OUTPUT_CAP_CHARS`] characters (the first
/// always).
fn shown(outcome: &SymbolsOutcome) -> usize {
    let stale = outcome.stale();
    let mut used = 0usize;
    for (index, target) in outcome.symbols.iter().enumerate() {
        let size: usize = target_lines(target, stale)
            .iter()
            .map(|line| line.chars().count() + 1)
            .sum();
        if index > 0 && used + size > OUTPUT_CAP_CHARS {
            return index;
        }
        used += size;
    }
    outcome.symbols.len()
}

/// The text: the targets within the cap, the tail when cut, the summary;
/// nothing for exit 1.
pub(crate) fn render_text(outcome: &SymbolsOutcome) -> String {
    let Some(code) = outcome.code.filter(|_| outcome.reason.is_none()) else {
        return String::new();
    };
    let shown = shown(outcome);
    let stale = outcome.stale();
    let mut out = String::new();
    for target in &outcome.symbols[..shown] {
        for line in target_lines(target, stale) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    let omitted = outcome.symbols.len() - shown;
    if omitted > 0 {
        out.push_str(&format!("[truncated: {omitted} symbols not shown]\n"));
    }
    out.push_str(&format!(
        "symbols: {shown} of {}; code files {} (.rs {}, .ron {}), skipped {}\n",
        outcome.total, code.files, code.rs, code.ron, code.skipped
    ));
    if let Some(layer_c) = &outcome.layer_c {
        let ra_ap = one_line(&layer_c.ra_ap);
        if stale {
            out.push_str(&format!(
                "layer c: {} (ra_ap {ra_ap}); spec resolve refreshes it\n",
                layer_c.state
            ));
        } else {
            out.push_str(&format!("layer c: {} (ra_ap {ra_ap})\n", layer_c.state));
        }
    }
    out
}

impl SymbolsOutcome {
    /// Whether the shown layer C snapshot is stale.
    fn stale(&self) -> bool {
        self.layer_c
            .as_ref()
            .is_some_and(|layer_c| layer_c.state == LayerCState::Stale.as_str())
    }
}

/// The targets the cap left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Omitted {
    symbols: usize,
}

/// The JSON document: the text's targets, every key present.
#[derive(Serialize)]
struct SymbolsJson<'a> {
    form: &'a str,
    query: Option<&'a str>,
    node: Option<&'a str>,
    path: Option<&'a str>,
    line: Option<usize>,
    reason: Option<&'a str>,
    notes: Vec<String>,
    symbols: &'a [SymbolTarget],
    total: usize,
    omitted: Option<Omitted>,
    code: Option<CodeCount>,
    layer_c: Option<&'a SymbolsLayerC>,
}

impl Serialize for SymbolsOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let answered = self.reason.is_none() && self.code.is_some();
        let shown = if answered { shown(self) } else { 0 };
        SymbolsJson {
            form: self.form,
            query: self.query.as_deref(),
            node: self.node.as_deref(),
            path: self.path.as_deref(),
            line: self.line,
            reason: self.reason.as_deref(),
            notes: notes(&self.messages),
            symbols: &self.symbols[..shown],
            total: self.total,
            omitted: answered.then(|| Omitted {
                symbols: self.symbols.len() - shown,
            }),
            code: if answered { self.code } else { None },
            layer_c: self.layer_c.as_ref().filter(|_| answered),
        }
        .serialize(serializer)
    }
}
