//! `spec lock diff REF [--json]` (task spec `lock-diff`; canon
//! `spec-lock-diff`): beside each drifted binding of REF's holders that the
//! lock holds, what moved since its entry, each side against the working
//! tree, as git's hunks. A read: no terminal, no question, no verdict.
//!
//! In order: discovery (exit 2, as every read); REF trimmed, classified and
//! resolved as `spec show` resolves it over the refreshed spec index (its
//! exit-1 cases exit 1 here, `reason` its words; its refusals exit 2; a
//! written `@rev` its note; several holders its warning), the holders by
//! (path, position); then the code layer as a read's
//! ([`with_judged_set`]: `spec verify --node`'s judge over the holders, the
//! lock parsed with this build's recipes and cut to them): any run cause
//! leaves every holder's `rows` null with one `bindings: <cause>` note,
//! exit 0. Kept: a row with a lock entry whose state raises drift (core
//! [`raises`]), in verify's order, under each holder whose set holds it.
//!
//! - **Spec side**, when the node moved (`node_gone`, or its (R, H) is not
//!   the entry's (`rev`, `spec_hash`)): the file holding the node now (a
//!   gone node's, the node its old ID resolves to as an alias, else the
//!   holder's) read at the entry's `commit` (`cat-file`, the path made
//!   top-relative), parsed under the working tree's config, the node whose
//!   canonical cited ID is the entry's, cut as `spec show` cuts it and
//!   carried through its last line's LF; against the node's span so cut
//!   in the bytes this read parsed (a gone node with no alias: empty).
//! - **Code side**, when the code moved (`file_gone`, `symbol_gone`,
//!   `marker_gone`, or verify's "Code changed"): the entry's `file_blob`
//!   by OID in the judge's session (never fetched), the entry's item
//!   located in it as `spec lock rebase` locates it ([`BlobUnits`]), its
//!   whole lines; against the row's item located so in the bytes the judge
//!   read (`file_gone`, `symbol_gone`: empty). A `module` or `ron` binding,
//!   or an item not located: the whole files, with a note.
//! - **Hunks**: `git diff --no-index` over two scratch files in the data
//!   directory (store `WorktreeGit::diff_hunks`), removed before it
//!   returns; `""` when equal. A side that moved but cannot be read or
//!   diffed: its `diff` null and one note.
//!
//! `[verify]` is never read; nothing depends on a row, a side or a note.
//! Git, read-only: `rev-parse`, `hash-object` without `-w`, `cat-file` and
//! `diff --no-index`. Nothing is written but the data directory (the
//! indexes refreshed, the scratch files). Output is uncut.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use specengine_code::{FileAnalysis, RustBinding, RustParser, bind_file};
use specengine_core::check::canonical_id;
use specengine_core::drift::raises;
use specengine_core::lock::{EntryKey, LockKind, Verified};
use specengine_core::verify::{
    CODE_CHANGED, FILE_GONE, LEVELS_CHANGED, MARKER_GONE, NODE_GONE, SYMBOL_GONE, State,
};
use specengine_store::{GitEnv, GitError, ObjectBlob, VerifyGit, WorktreeGit};

use crate::bindings::{SetJudged, with_judged_set};
use crate::code_walk::CodeTree;
use crate::corpus::indexed_in;
use crate::location::{open_index, prepared_data_dir};
use crate::lock_rebase::{BlobUnits, NodeNames, Unit};
use crate::project::{ProjectRoot, discover_with_text};
use crate::proposals::escaped_error;
use crate::refs::{Corpus, NodeKey};
use crate::show::{SHOW_REV_IGNORED, classify};
use crate::verify::{VerifyRow, keyed_binding, row_block, shown as shown_row};
use crate::verify_rows::{Anchor, Row};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// `spec lock diff` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockDiffRequest {
    /// `REF` as given.
    pub reference: String,
    /// Git's environment: the judge's and the sides' reads.
    pub git: GitEnv,
}

/// What `spec lock diff` found: REF's holders with their kept rows, or the
/// reason there is none (exit 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockDiffOutcome {
    /// `REF` as given.
    pub reference: String,
    /// `spec show`'s reason when REF names nothing; else `None`.
    pub reason: Option<String>,
    /// `note:` and `warning:` lines, in the order they arose.
    pub messages: Vec<Message>,
    /// The holders, by (path, position); empty on exit 1.
    pub nodes: Vec<LockDiffNode>,
}

/// One holder of REF and its kept rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockDiffNode {
    /// Its canonical cited ID; `None` for a document without one.
    pub id: Option<String>,
    /// Root-relative.
    pub path: String,
    /// The 1-based line its span starts on.
    pub line: usize,
    /// Its kept rows, in verify's order; `None`: the code layer was not
    /// judged (a run cause, the read's `bindings:` note).
    pub rows: Option<Vec<LockDiffRow>>,
}

/// A kept row: verify's row and its two sides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockDiffRow {
    /// `spec verify --node REF --json`'s row of the same key.
    pub row: VerifyRow,
    /// `None` when the node did not move.
    pub spec: Option<LockDiffSpecSide>,
    /// `None` when the code did not move.
    pub code: Option<LockDiffCodeSide>,
}

/// The node's text at the entry's commit against the working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockDiffSpecSide {
    /// The entry's `commit`, full.
    pub commit: String,
    /// The root-relative file holding the node now, read at `commit`.
    pub path: String,
    /// Git's hunks from the first `@@`; `""` equal; `None` not shown (a
    /// note says why).
    pub diff: Option<String>,
}

/// The code at the entry's blob against the working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LockDiffCodeSide {
    /// The entry's `file_blob`, full.
    pub blob: String,
    /// The entry's `file`.
    pub file: String,
    /// Git's hunks from the first `@@`; `""` equal; `None` not shown (a
    /// note says why).
    pub diff: Option<String>,
}

impl LockDiffOutcome {
    /// 0 answered, whatever the rows, sides and notes; 1 REF names nothing.
    pub fn exit(&self) -> Exit {
        if self.reason.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// The `note:` lines' texts, in order.
    fn notes(&self) -> Vec<String> {
        self.messages
            .iter()
            .filter_map(|message| match message {
                Message::Note(text) => Some(one_line(text)),
                Message::Warning(_) => None,
            })
            .collect()
    }
}

impl Serialize for LockDiffOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        /// The document: every key present, uncut.
        #[derive(Serialize)]
        struct Document<'a> {
            #[serde(rename = "ref")]
            reference: &'a str,
            reason: Option<&'a str>,
            notes: Vec<String>,
            nodes: &'a [LockDiffNode],
        }
        Document {
            reference: &self.reference,
            reason: self.reason.as_deref(),
            notes: self.notes(),
            nodes: &self.nodes,
        }
        .serialize(serializer)
    }
}

/// `spec lock diff REF`: see the module documentation.
pub fn lock_diff(
    env: &Env,
    globals: &Globals,
    request: &LockDiffRequest,
) -> Result<LockDiffOutcome, CliError> {
    let (project, text) = discover_with_text(env, globals)?;
    project.slug()?;
    let written = request.reference.trim();
    let scheme = &project.config.scheme;
    let mut messages = Vec::new();
    let target =
        match classify(written, scheme, &mut messages, SHOW_REV_IGNORED).map_err(escaped_error)? {
            Ok(target) => target,
            Err(reason) => return Ok(not_found(request, &reason, messages)),
        };
    let mut open = open_index(env, &project).map_err(escaped_error)?;
    let scratch = prepared_data_dir(env, &project).map_err(escaped_error)?;
    let input =
        indexed_in(&mut open.index, &project, &mut messages, true).map_err(escaped_error)?;
    let mut corpus = Corpus::new(&input, scheme, &project.config.paths);
    let holders: Vec<NodeKey> = match corpus
        .target_nodes(&target, written)
        .map_err(escaped_error)?
    {
        Ok(nodes) => nodes
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        Err(reason) => return Ok(not_found(request, &reason, messages)),
    };
    if holders.len() > 1 {
        let cited: Vec<String> = holders.iter().map(|&key| corpus.citation(key)).collect();
        messages.push(Message::Warning(format!(
            "`{written}` has {} holders, all shown: {}",
            holders.len(),
            cited.join(", ")
        )));
    }
    let mut nodes: Vec<LockDiffNode> = holders
        .iter()
        .map(|&key| LockDiffNode {
            id: crate::node_set::canonical_of(&corpus, scheme, key),
            path: corpus.resolver.paths()[key.file].to_owned(),
            line: corpus.node_line(key),
            rows: None,
        })
        .collect();

    // The code layer: the holders' rows judged, the code sides' blobs read
    // and their places now located, in the judge's session.
    let kept = with_judged_set(
        &project,
        &text,
        &request.git,
        &mut open.index,
        &input,
        &holders,
        |judged, git| kept_rows(judged, git, &holders, &project, &mut corpus),
    );
    let kept = match kept {
        Ok(kept) => kept,
        Err(note) => {
            messages.push(Message::Note(note));
            return Ok(answered(request, messages, nodes));
        }
    };

    // Each kept row's sides once, its notes at its first place in output.
    let mut sides = Sides {
        project: &project,
        corpus,
        scratch,
        git_env: &request.git,
        blobs: kept.blobs,
        texts: kept.texts,
        at_root: None,
        differ: None,
        parser: None,
        specs: BTreeMap::new(),
        spec_diffs: BTreeMap::new(),
        bound: BTreeMap::new(),
    };
    let mut made: Vec<Option<LockDiffRow>> = vec![None; kept.rows.len()];
    for (node, of) in nodes.iter_mut().zip(&kept.by_holder) {
        let mut rows = Vec::with_capacity(of.len());
        for &at in of {
            if made[at].is_none() {
                let mut notes = Vec::new();
                made[at] = Some(sides.row(&kept.rows[at], node, &mut notes));
                messages.extend(notes.into_iter().map(Message::Note));
            }
            rows.extend(made[at].clone());
        }
        node.rows = Some(rows);
    }
    Ok(answered(request, messages, nodes))
}

/// Exit 1: `spec show`'s reason, on one line.
fn not_found(request: &LockDiffRequest, reason: &str, messages: Vec<Message>) -> LockDiffOutcome {
    LockDiffOutcome {
        reference: request.reference.clone(),
        reason: Some(one_line(reason)),
        messages,
        nodes: Vec::new(),
    }
}

/// Exit 0.
fn answered(
    request: &LockDiffRequest,
    messages: Vec<Message>,
    nodes: Vec<LockDiffNode>,
) -> LockDiffOutcome {
    LockDiffOutcome {
        reference: request.reference.clone(),
        reason: None,
        messages,
        nodes,
    }
}

// ------------------------------------------------------------ the judge's facts

/// What the judge's session gave: the kept rows, each once, and per holder
/// (in order) the positions of its own; the code sides' blobs by OID
/// (`None`: the session could not read them); the code sides' files now,
/// as the judge read them, each once by path.
struct Kept {
    rows: Vec<KeptRow>,
    by_holder: Vec<Vec<usize>>,
    blobs: Option<BTreeMap<String, ObjectBlob>>,
    texts: BTreeMap<String, String>,
}

/// A kept row, the set nodes it belongs to, and, with a code side, its
/// code now.
struct KeptRow {
    row: Row,
    set: BTreeSet<NodeKey>,
    now: Option<CodeNow>,
}

/// A code side's working-tree half, as the judge read it (its file's text:
/// [`Kept::texts`], absent when the file is not in the judged tree).
struct CodeNow {
    /// An `item` row's item located in its file now: its byte range.
    item: Option<Range<usize>>,
}

/// The rows kept (see the module documentation), the code sides' blobs
/// read by OID in the open session, each code side's place now located in
/// the judged tree (each file's text taken and its units read once).
fn kept_rows(
    judged: &SetJudged,
    git: &mut VerifyGit,
    holders: &[NodeKey],
    project: &ProjectRoot,
    corpus: &mut Corpus<'_>,
) -> Kept {
    let mut rows = Vec::new();
    let mut by_holder = vec![Vec::new(); holders.len()];
    let mut now = NowFiles {
        tree: &judged.tree,
        texts: BTreeMap::new(),
        units: BTreeMap::new(),
    };
    for (row, set) in judged.judged.rows.iter().zip(&judged.nodes) {
        let judgement = &row.judgement;
        if row.entry.is_none() || !raises(judgement.state, judgement.detail.as_deref(), false) {
            continue;
        }
        let at = rows.len();
        for (holder, own) in holders.iter().zip(by_holder.iter_mut()) {
            if set.contains(holder) {
                own.push(at);
            }
        }
        let code = code_moved(row).then(|| now.code_now(row, project, corpus));
        rows.push(KeptRow {
            row: row.clone(),
            set: set.clone(),
            now: code,
        });
    }
    let oids: BTreeSet<&str> = rows
        .iter()
        .filter(|kept| kept.now.is_some())
        .filter_map(|kept| kept.row.entry.as_ref())
        .map(|entry| entry.file_blob.as_str())
        .collect();
    let blobs = if oids.is_empty() {
        Some(BTreeMap::new())
    } else {
        git.read_blobs(oids).ok()
    };
    Kept {
        rows,
        by_holder,
        blobs,
        texts: now.texts,
    }
}

/// The judged tree's files as the code sides read them: each text taken
/// once by path, each Rust file's units read once (`None`: the naming
/// failed).
struct NowFiles<'t> {
    tree: &'t CodeTree,
    texts: BTreeMap<String, String>,
    units: BTreeMap<String, Option<BlobUnits>>,
}

impl NowFiles<'_> {
    /// A code side's half now: the row's file as the judge read it, and
    /// an `item` row's item located in it as the blob locator locates one.
    fn code_now(&mut self, row: &Row, project: &ProjectRoot, corpus: &mut Corpus<'_>) -> CodeNow {
        let file = row.key.file.as_str();
        let tree = self.tree;
        if let Some(rust) = tree.rust.iter().find(|rust| rust.path == file) {
            self.texts
                .entry(file.to_owned())
                .or_insert_with(|| rust.text.clone());
            let item = match row.key.entry_key() {
                Some(key) if key.kind == LockKind::Item && !gone_now(row) => {
                    let units = self
                        .units
                        .entry(file.to_owned())
                        .or_insert_with(|| bound_units(&rust.binding, file, &rust.text));
                    units.as_ref().and_then(|units| {
                        let mut names = NodeNames::new(corpus, project);
                        located_item(units, &rust.binding.analysis, &key, &mut names)
                    })
                }
                _ => None,
            };
            return CodeNow { item };
        }
        if let Some(ron) = tree.ron.iter().find(|ron| ron.path == file) {
            self.texts
                .entry(file.to_owned())
                .or_insert_with(|| ron.text.clone());
        }
        CodeNow { item: None }
    }
}

/// The units of `binding`, the comment binding of `text` at `file`, as the
/// blob locator reads them; `None` when the naming panics.
fn bound_units(binding: &RustBinding, file: &str, text: &str) -> Option<BlobUnits> {
    panic::catch_unwind(AssertUnwindSafe(|| BlobUnits::of(binding, file, text))).ok()
}

/// `key` located among `units` (of the file `analysis` is of) as `spec
/// lock rebase` locates an entry: an item's byte range; `None` not located
/// (none, several, a module, a panic).
fn located_item(
    units: &BlobUnits,
    analysis: &FileAnalysis,
    key: &EntryKey,
    names: &mut NodeNames<'_, '_>,
) -> Option<Range<usize>> {
    panic::catch_unwind(AssertUnwindSafe(|| {
        match units.locate(analysis, key, names) {
            Ok(Unit::Item(index)) => analysis
                .items
                .get(index)
                .map(|item| item.byte_range.clone()),
            Ok(Unit::Module(_)) | Err(_) => None,
        }
    }))
    .ok()
    .flatten()
}

// ------------------------------------------------------------ the sides

/// Whether the row's node moved since its entry: `node_gone`, or its (R,
/// H) is not the entry's (`rev`, `spec_hash`).
fn spec_moved(row: &Row, entry: &Verified) -> bool {
    let judgement = &row.judgement;
    if judgement.state == State::BrokenBinding && judgement.category.as_deref() == Some(NODE_GONE) {
        return true;
    }
    (row.node_rev, row.norm_hash.as_deref()) != (Some(entry.rev), Some(entry.spec_hash.as_str()))
}

/// Whether the row's code moved since its entry: `file_gone`,
/// `symbol_gone`, `marker_gone`, or verify's "Code changed" (a `conflict`;
/// `code_ahead` `code_changed` or `levels_changed`; `raised` with a digest
/// changed or L_lock not L_mark).
fn code_moved(row: &Row) -> bool {
    let judgement = &row.judgement;
    let category = judgement.category.as_deref();
    match judgement.state {
        State::Conflict => true,
        State::CodeAhead => {
            matches!(category, Some(CODE_CHANGED | LEVELS_CHANGED))
                || row.levels.as_ref().is_some_and(|levels| {
                    levels
                        .changed
                        .as_ref()
                        .is_some_and(|changed| !changed.is_empty())
                        || levels.lock != levels.markers
                })
        }
        State::BrokenBinding => matches!(category, Some(FILE_GONE | SYMBOL_GONE | MARKER_GONE)),
        _ => false,
    }
}

/// The row's code is gone now: `file_gone`, `symbol_gone` (its new text is
/// empty).
fn gone_now(row: &Row) -> bool {
    row.judgement.state == State::BrokenBinding
        && matches!(
            row.judgement.category.as_deref(),
            Some(FILE_GONE | SYMBOL_GONE)
        )
}

/// The entry's key: a moved row's old place, else the row's own.
fn entry_key(row: &Row) -> Option<EntryKey> {
    match &row.anchor {
        Some(Anchor::Moved(from)) => Some(from.clone()),
        _ => row.key.entry_key(),
    }
}

/// `<node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]`: the
/// row's key as verify's text prints it.
fn key_text(row: &VerifyRow) -> String {
    let binding = keyed_binding(row.kind, row.symbol.as_deref(), row.item_kind.as_deref())
        .unwrap_or_default();
    format!(
        "{} | {} | {} | {binding}",
        one_line(&row.node),
        one_line(row.relation),
        one_line(&row.file)
    )
}

/// The sides' reads: the corpus read now, the scratch directory, git's
/// environment, the code sides' blobs and files now, and what is made on
/// first use and kept.
struct Sides<'a, 'c> {
    project: &'a ProjectRoot,
    corpus: Corpus<'c>,
    scratch: PathBuf,
    git_env: &'a GitEnv,
    blobs: Option<BTreeMap<String, ObjectBlob>>,
    /// The code sides' files now, by path ([`Kept::texts`]).
    texts: BTreeMap<String, String>,
    /// Git at the root (`GIT_NO_LAZY_FETCH=1`: an object a partial clone
    /// lacks is never fetched) and the root's place below the worktree's
    /// top.
    at_root: Option<Result<(WorktreeGit, String), String>>,
    /// Git in the scratch directory: the hunks; `Err` the note's why.
    differ: Option<Result<WorktreeGit, String>>,
    parser: Option<Option<RustParser>>,
    /// A spec file's bytes at a commit, by (commit, root-relative path);
    /// `None` cannot be read.
    specs: BTreeMap<(String, String), Option<Vec<u8>>>,
    /// A spec side's diff, by (commit, path, the entry's node, the node
    /// now): the rows of one node share it; `Err` the note's why.
    spec_diffs: SpecDiffs,
    /// A blob bound and its units, by (OID, file); `None`: no tree or the
    /// naming failed.
    bound: BTreeMap<(String, String), Option<(RustBinding, BlobUnits)>>,
}

/// The spec sides' diffs made, by (commit, path, node, node now).
type SpecDiffs = BTreeMap<(String, String, String, Option<NodeKey>), Result<String, String>>;

impl Sides<'_, '_> {
    /// One kept row as shown under `holder`, its notes added to `notes`.
    fn row(
        &mut self,
        kept: &KeptRow,
        holder: &LockDiffNode,
        notes: &mut Vec<String>,
    ) -> LockDiffRow {
        let row = shown_row(&kept.row);
        let key = key_text(&row);
        let entry_key = entry_key(&kept.row);
        let spec = match (&kept.row.entry, &entry_key) {
            (Some(entry), Some(entry_key)) if spec_moved(&kept.row, entry) => {
                Some(self.spec_side(kept, entry, entry_key, holder, &key, notes))
            }
            _ => None,
        };
        let code = match (&kept.row.entry, &entry_key, &kept.now) {
            (Some(entry), Some(entry_key), Some(now)) => {
                Some(self.code_side(&kept.row, entry, entry_key, now, &key, notes))
            }
            _ => None,
        };
        LockDiffRow { row, spec, code }
    }

    /// The spec side (see the module documentation).
    fn spec_side(
        &mut self,
        kept: &KeptRow,
        entry: &Verified,
        entry_key: &EntryKey,
        holder: &LockDiffNode,
        key: &str,
        notes: &mut Vec<String>,
    ) -> LockDiffSpecSide {
        // The node now: the one its ID names (a set node first), a gone
        // node's alias; none: the holder's file, empty.
        let judged = self.corpus.judge(&entry_key.node);
        let now = judged
            .nodes
            .iter()
            .copied()
            .find(|node| kept.set.contains(node))
            .or_else(|| judged.nodes.first().copied());
        let path = match now {
            Some(node) => self.corpus.resolver.paths()[node.file].to_owned(),
            None => holder.path.clone(),
        };
        let cache = (
            entry.commit.clone(),
            path.clone(),
            entry_key.node.clone(),
            now,
        );
        let made = match self.spec_diffs.get(&cache) {
            Some(made) => made.clone(),
            None => {
                let made = self.spec_diff(&entry.commit, &path, &entry_key.node, now);
                self.spec_diffs.insert(cache, made.clone());
                made
            }
        };
        let mut side = LockDiffSpecSide {
            commit: entry.commit.clone(),
            path,
            diff: None,
        };
        match made {
            Ok(diff) => side.diff = Some(diff),
            Err(why) => notes.push(format!("lock diff: {key}: no spec diff: {why}")),
        }
        side
    }

    /// The node `node`'s span in `path` at `commit` against the node now's
    /// span (none: empty); `Err` why not (N1, N3, N2, N7).
    fn spec_diff(
        &mut self,
        commit: &str,
        path: &str,
        node: &str,
        now: Option<NodeKey>,
    ) -> Result<String, String> {
        let new = now.map(|now| self.span_now(now)).unwrap_or_default();
        let bytes = self
            .spec_at(commit, path)
            .ok_or_else(|| format!("{path} cannot be read at {commit}"))?;
        if std::str::from_utf8(&bytes).is_err() {
            return Err(format!("{path} at {commit} is not UTF-8"));
        }
        let old = self
            .span_at(path, &bytes, node)
            .ok_or_else(|| format!("{path} at {commit} holds no {node}"))?;
        self.hunks(&old, &new)
    }

    /// The node's span in the bytes this read parsed, as `spec show` cuts
    /// it, through its last line's LF ([`through_lf`]).
    fn span_now(&self, node: NodeKey) -> Vec<u8> {
        let Some(file) = self.corpus.files.get(node.file) else {
            return Vec::new();
        };
        file.parsed
            .as_ref()
            .and_then(|parsed| parsed.nodes.get(node.ord))
            .and_then(|found| through_lf(&file.bytes, found.span.range()))
            .unwrap_or_default()
            .to_vec()
    }

    /// `path` at `commit` (`cat-file`, top-relative, never fetched); `None`
    /// when it cannot be read (the commit or the object gone, the file not
    /// in it, git failing).
    fn spec_at(&mut self, commit: &str, path: &str) -> Option<Vec<u8>> {
        let cache = (commit.to_owned(), path.to_owned());
        if let Some(read) = self.specs.get(&cache) {
            return read.clone();
        }
        let read = match self.at_root() {
            Ok((git, prefix)) => {
                let top = if prefix.is_empty() {
                    path.to_owned()
                } else {
                    format!("{prefix}/{path}")
                };
                git.blob_at(commit, &top).ok()
            }
            Err(_) => None,
        };
        self.specs.insert(cache, read.clone());
        read
    }

    /// The span of the node whose canonical cited ID is `node` in `bytes`,
    /// `path`'s parse under the working tree's config, as `spec show` cuts
    /// it, through its last line's LF ([`through_lf`]); `None`: no such
    /// node (or the parser failed).
    fn span_at(&self, path: &str, bytes: &[u8], node: &str) -> Option<Vec<u8>> {
        let scheme = &self.project.config.scheme;
        let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
            specengine_core::parse(path, bytes, scheme)
        }))
        .ok()?;
        let found = parsed.nodes.iter().find(|candidate| {
            candidate.id.as_deref().is_some_and(|id| {
                canonical_id(&self.corpus.resolver, scheme, path, id, None) == node
            })
        })?;
        through_lf(bytes, found.span.range()).map(<[u8]>::to_vec)
    }

    /// The code side (see the module documentation).
    fn code_side(
        &mut self,
        row: &Row,
        entry: &Verified,
        entry_key: &EntryKey,
        now: &CodeNow,
        key: &str,
        notes: &mut Vec<String>,
    ) -> LockDiffCodeSide {
        let blob = entry.file_blob.as_str();
        let mut side = LockDiffCodeSide {
            blob: blob.to_owned(),
            file: entry_key.file.clone(),
            diff: None,
        };
        let no_diff = |why: String| format!("lock diff: {key}: no code diff: {why}");
        let old = match self.blobs.as_ref().and_then(|blobs| blobs.get(blob)) {
            Some(ObjectBlob::Bytes(bytes)) => bytes.clone(),
            _ => {
                notes.push(no_diff(format!(
                    "blob {blob} of {} cannot be read",
                    side.file
                )));
                return side;
            }
        };
        let Ok(old) = String::from_utf8(old) else {
            notes.push(no_diff(format!(
                "blob {blob} of {} is not UTF-8",
                side.file
            )));
            return side;
        };
        let gone = gone_now(row);
        let text = self.texts.get(&row.key.file).cloned();
        let whole_new = if gone {
            ""
        } else {
            text.as_deref().unwrap_or_default()
        };
        let symbol = |key: &EntryKey| one_line(&key.symbol);
        let (old_text, new_text, why) = match entry_key.kind {
            LockKind::Module => (old.as_str(), whole_new, Some("a module binding".to_owned())),
            LockKind::Ron => (old.as_str(), whole_new, Some("a ron binding".to_owned())),
            LockKind::Item => match self.located_old(blob, &old, entry_key) {
                None => (
                    old.as_str(),
                    whole_new,
                    Some(format!("{} is not found in blob {blob}", symbol(entry_key))),
                ),
                Some(range) if gone => (whole_lines(&old, &range), "", None),
                Some(range) => match (&now.item, &text) {
                    (Some(item), Some(text)) => {
                        (whole_lines(&old, &range), whole_lines(text, item), None)
                    }
                    _ => {
                        let row_key = row.key.entry_key();
                        let shown_symbol = row_key.as_ref().map_or_else(String::new, symbol);
                        (
                            old.as_str(),
                            whole_new,
                            Some(format!(
                                "{shown_symbol} is not found in {} now",
                                one_line(&row.key.file)
                            )),
                        )
                    }
                },
            },
        };
        match self.hunks(old_text.as_bytes(), new_text.as_bytes()) {
            Ok(hunks) => {
                side.diff = Some(hunks);
                if let Some(why) = why {
                    notes.push(format!(
                        "lock diff: {key}: the code diff is the whole file: {why}"
                    ));
                }
            }
            Err(why) => notes.push(no_diff(why)),
        }
        side
    }

    /// The entry `key` located in the blob `oid` (`text`) as `spec lock
    /// rebase` locates it: its item's range; `None` not located. The blob
    /// is bound and its units read once per (OID, file).
    fn located_old(&mut self, oid: &str, text: &str, key: &EntryKey) -> Option<Range<usize>> {
        let cache = (oid.to_owned(), key.file.clone());
        if !self.bound.contains_key(&cache) {
            let parser = self
                .parser
                .get_or_insert_with(|| RustParser::new().ok())
                .as_mut();
            let bound = parser.and_then(|parser| {
                let binding =
                    panic::catch_unwind(AssertUnwindSafe(|| bind_file(parser, text))).ok()??;
                let units = bound_units(&binding, &key.file, text)?;
                Some((binding, units))
            });
            self.bound.insert(cache.clone(), bound);
        }
        let (binding, units) = self.bound.get(&cache)?.as_ref()?;
        let mut names = NodeNames::new(&mut self.corpus, self.project);
        located_item(units, &binding.analysis, key, &mut names)
    }

    /// Git at the root and the root's place below its worktree's top,
    /// made once; its children get `GIT_NO_LAZY_FETCH=1`, so `cat-file`
    /// never fetches an object a partial clone lacks (nothing is written
    /// under `.git/objects`).
    fn at_root(&mut self) -> Result<&(WorktreeGit, String), &String> {
        let project = self.project;
        let git_env = self.git_env;
        self.at_root
            .get_or_insert_with(|| {
                let env = git_env.clone().with_var(NO_LAZY_FETCH, "1");
                let git = WorktreeGit::new(&project.root, &env).map_err(|e| e.to_string())?;
                let prefix = root_prefix(&git, &project.root)?;
                Ok((git, prefix))
            })
            .as_ref()
    }

    /// Git's hunks from `old` to `new` over two scratch files (removed
    /// before it returns): every line from the first `@@`, `""` equal.
    /// `Err`, the note's why on one line, never a path: git failing; or
    /// bytes that differ with no hunk shown (a binary pair).
    fn hunks(&mut self, old: &[u8], new: &[u8]) -> Result<String, String> {
        let scratch = self.scratch.clone();
        let git_env = self.git_env;
        let differ = self
            .differ
            .get_or_insert_with(|| {
                WorktreeGit::new(&scratch, git_env).map_err(|error| diff_failure(&error, &scratch))
            })
            .as_ref()
            .map_err(Clone::clone)?;
        let hunks = differ
            .diff_hunks(&scratch, old, new)
            .map_err(|error| diff_failure(&error, &scratch))?;
        if hunks.is_empty() && old != new {
            return Err(NO_HUNK.to_owned());
        }
        Ok(String::from_utf8_lossy(&hunks).into_owned())
    }
}

/// The variable that keeps git from fetching an object a partial clone
/// lacks.
const NO_LAZY_FETCH: &str = "GIT_NO_LAZY_FETCH";

/// N7's why when the diff could not run, never naming a path.
const DIFF_NOT_RUN: &str = "git diff --no-index could not run";

/// N7's why for bytes that differ that git shows no hunk for (a binary
/// pair: a NUL byte).
const NO_HUNK: &str = "git shows no hunk for bytes that differ";

/// N7's why of a failed diff, on one line and path-free: a scratch file
/// that could not be written or removed, or git's words naming one, is
/// [`DIFF_NOT_RUN`]; else `git diff --no-index failed: <error>`.
fn diff_failure(error: &GitError, scratch: &Path) -> String {
    let names_scratch = |text: &str| {
        text.contains("specengine-")
            || scratch
                .to_str()
                .is_some_and(|dir| !dir.is_empty() && text.contains(dir))
    };
    match error {
        GitError::Io { .. } => DIFF_NOT_RUN.to_owned(),
        GitError::Failed { stderr, .. } if names_scratch(stderr) => DIFF_NOT_RUN.to_owned(),
        other => format!(
            "git diff --no-index failed: {}",
            one_line(&other.to_string())
        ),
    }
}

/// The root's place below its worktree's top (`rev-parse --show-toplevel`),
/// `/`-joined; `""` at the top.
fn root_prefix(git: &WorktreeGit, root: &Path) -> Result<String, String> {
    let top = git.top().map_err(|error| error.to_string())?;
    let root = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let inner = root
        .strip_prefix(&top)
        .map_err(|_| format!("{} lies outside {}", root.display(), top.display()))?;
    let mut parts = Vec::new();
    for component in inner.components() {
        match component {
            Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| format!("{} is not UTF-8", root.display()))?,
            ),
            _ => return Err(format!("{} lies outside {}", root.display(), top.display())),
        }
    }
    Ok(parts.join("/"))
}

/// A spec span's bytes of `bytes`, its end carried through its last
/// line's LF (`spec show`'s cut stops before it), so a span ending
/// mid-file gives hunks without git's no-newline marker; a span that ends
/// the file without an LF is kept as it is. `None`: `range` is not in
/// `bytes`.
fn through_lf(bytes: &[u8], range: Range<usize>) -> Option<&[u8]> {
    let span = bytes.get(range.clone())?;
    if span.last() == Some(&b'\n') {
        return Some(span);
    }
    let end = bytes[range.end..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |at| range.end + at + 1);
    bytes.get(range.start..end)
}

/// The whole lines of `text` holding `range`: its first line's start
/// through its last line's LF (the text's end without one).
fn whole_lines<'t>(text: &'t str, range: &Range<usize>) -> &'t str {
    let bytes = text.as_bytes();
    let begin = range.start.min(bytes.len());
    let start = bytes[..begin]
        .iter()
        .rposition(|&byte| byte == b'\n')
        .map_or(0, |at| at + 1);
    let last = range.end.saturating_sub(1).max(begin).min(bytes.len());
    let end = bytes[last..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |at| last + at + 1);
    text.get(start..end).unwrap_or_default()
}

// ------------------------------------------------------------ text

/// The text report: per holder its line and its rows, each row's block
/// (verify's), its spec and code sides and their hunks as git gave them;
/// last, the summary. Exit 1: nothing.
pub(crate) fn render_text(outcome: &LockDiffOutcome) -> String {
    if outcome.reason.is_some() {
        return String::new();
    }
    let mut out = String::new();
    let (mut rows, mut specs, mut codes, mut hidden) = (0usize, 0usize, 0usize, 0usize);
    let mut judged = true;
    for node in &outcome.nodes {
        out.push_str(&format!(
            "{} | {}:{}\n",
            node.id.as_deref().map_or_else(|| "-".to_owned(), one_line),
            one_line(&node.path),
            node.line
        ));
        let Some(kept) = &node.rows else {
            judged = false;
            out.push_str("  not judged\n");
            continue;
        };
        if kept.is_empty() {
            out.push_str("  no drifted row with a lock entry\n");
        }
        for row in kept {
            rows += 1;
            out.push('\n');
            out.push_str(&row_block(&row.row));
            if let Some(spec) = &row.spec {
                specs += 1;
                hidden += usize::from(spec.diff.is_none());
                let head = format!(
                    "  spec {} at {}",
                    one_line(&spec.path),
                    one_line(&spec.commit)
                );
                push_side(&mut out, &head, spec.diff.as_deref());
            }
            if let Some(code) = &row.code {
                codes += 1;
                hidden += usize::from(code.diff.is_none());
                let head = format!(
                    "  code {} blob {}",
                    one_line(&code.file),
                    one_line(&code.blob)
                );
                push_side(&mut out, &head, code.diff.as_deref());
            }
        }
    }
    out.push('\n');
    let reference = one_line(outcome.reference.trim());
    if judged {
        out.push_str(&format!(
            "lock diff {reference}: rows {rows}, spec sides {specs}, code sides {codes}"
        ));
        if hidden > 0 {
            out.push_str(&format!(", not shown {hidden}"));
        }
        out.push('\n');
    } else {
        out.push_str(&format!("lock diff {reference}: not judged\n"));
    }
    escape_controls(&out)
}

/// A side's line, `: unchanged` (`""`) or `: not shown` (`None`) at its
/// end, then its hunks as git gave them.
fn push_side(out: &mut String, head: &str, diff: Option<&str>) {
    out.push_str(head);
    match diff {
        None => out.push_str(": not shown\n"),
        Some("") => out.push_str(": unchanged\n"),
        Some(hunks) => {
            out.push('\n');
            out.push_str(hunks);
            if !hunks.ends_with('\n') {
                out.push('\n');
            }
        }
    }
}
