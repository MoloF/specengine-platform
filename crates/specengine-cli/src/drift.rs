//! `spec propose drift [REF…] [--author-role R] [--author-model M] [--run
//! ID] [--task T]` (task spec `drift-queue`, "spec propose drift",
//! "Triggers", "The item", "Dedup", "Output"): the drift of the
//! commit `HEAD` names, judged as `spec verify` judges a commit, queued as
//! one `drift` item per node whose lock pair no longer holds, its evidence
//! computed here and never typed. Only the queue and the data directory
//! (the index, when a REF needs the working tree's corpus) are written;
//! nothing under the root, no commit; nothing is blocked.
//!
//! In order, the first refusal answering:
//!
//! 1. the project and its queue (as every queue command: exit 2), the
//!    author's grammar (exit 2), at most [`NODE_IDS_MAX`] REFs (exit 2);
//! 2. the place (canon `proposal-queue`, "Creation" 5): a worktree, `HEAD`
//!    born and on a branch (exit 2); `base_commit` the `HEAD` read, the one
//!    commit judged, never `HEAD` read again;
//! 3. `specengine.toml` tracked and clean (exit 1 naming its state:
//!    commit it first); `[code]` of the commit's config (none: exit 2);
//! 4. `--task T`, as every proposal's (exit 1);
//! 5. the commit's side alone ([`read_commit`]: no working-tree file is
//!    read, an uncommitted edit is never judged; its causes exit 2);
//! 6. each REF resolved as `spec show` resolves it over the commit's
//!    documents (a written `@rev` dropped, a note): one node with an ID, or
//!    a gone ID with entries in the commit's lock; several nodes, none, a
//!    node only the working tree has (`commit it first`): exit 1; a
//!    look-alike: exit 2. No REF: a sweep of every node;
//! 7. the rows judged ([`judge_commit`]), per node (canonical, or the
//!    gone ID): `code_ahead`, `spec_ahead`, `conflict`, `broken_binding`
//!    raise an item, `unlocked` only for a named node (core's
//!    [`raises`]); a named node's `unlocked key_problem` rows a note, a
//!    named node raising nothing `no drift at HEAD`;
//! 8. the items stored in one transaction, deduplicated by the queue
//!    ([`ProposalQueue::create_drift`]): an open or approved item of the
//!    repository, branch and node is a hit, a rejected one with equal
//!    evidence settled; notes on an item whose evidence an applied one had
//!    (its lock commit undone) and on each open item of this repository and
//!    branch whose node now reads all `ok`.
//!
//! The answer (exit 0 whatever was stored): text by node bytes, `created
//! PR | node | severity | summary`, `queued PR | node | status | …`,
//! `settled PR | node | rejected | same evidence`, then `propose drift: <n>
//! created, <m> hit(s); <r> rows of <k> nodes judged at <commit> on
//! <branch>`; JSON the drift document ([`DriftDocument`], MCP
//! `report_drift`'s `structuredContent`).

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use specengine_core::check::canonical_id;
use specengine_core::drift::{
    DRIFT_ROWS_MAX, DriftEvidence, DriftLock, DriftMarker, DriftRow, HashedEntry, HashedRow,
    drift_hash_input, drift_summary, raises,
};
use specengine_core::intake::{IntakeSeverity, NODE_IDS_MAX};
use specengine_core::lock::LOCK_FILE;
use specengine_core::proposal::Author;
use specengine_core::verify::{Class, KEY_PROBLEM, RowBinding, State};
use specengine_store::{
    DriftResult, GitEnv, KindFamily, NewDrift, Place, ProposalFilter, ProposalQueue as _,
    ProposalStatus, QueueError, VerifyGit, WorktreeGit, b3_hash, same_repository,
};

use crate::corpus::indexed;
use crate::lock::{NOT_TRACKED, canonical_ids, state_of, without_rev};
use crate::project::CONFIG_FILE;
use crate::proposals::{QueueContext, checked_now, escaped_error, open_context, queue_cannot};
use crate::refs::Corpus;
use crate::show::classify;
use crate::task::bound_task;
use crate::verify::{VerifyRow, shown};
use crate::verify_rows::{CommitSide, Row, RunCause, commit_config, judge_commit, read_commit};
use crate::{CliError, Env, Exit, Globals, Message, escape_controls, one_line};

/// Why a written `@rev` is ignored (the note's end).
const REV_IGNORED: &str = "`spec propose drift` judges the node at HEAD";

/// `spec propose drift` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftRequest {
    /// `REF…` as given, 0 to [`NODE_IDS_MAX`]; none: a sweep.
    pub references: Vec<String>,
    /// `--author-role`, `--author-model`, `--run`: any given → an agent.
    pub author_role: Option<String>,
    pub author_model: Option<String>,
    pub run: Option<String>,
    /// The injected clock: `YYYY-MM-DDTHH:MM:SSZ`.
    pub now: String,
    /// The caller's environment; git runs without its local `GIT_*`
    /// variables.
    pub git: GitEnv,
}

/// The rows and nodes judged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DriftJudged {
    /// The rows of the nodes judged, `ok` ones too.
    pub rows: usize,
    pub nodes: usize,
}

/// An item stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftCreated {
    pub id: String,
    pub node: String,
    /// `high` or `normal`.
    pub severity: String,
    pub summary: String,
}

/// An item the dedup found instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftHit {
    pub id: String,
    pub node: String,
    /// `open`, `approved` (queued) or `rejected` (settled).
    pub status: String,
    /// Its evidence equals the evidence judged now.
    pub same: bool,
}

/// The drift document: every key, absent `null`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct DriftDocument {
    /// The commit judged; `null` before the place was read.
    pub commit: Option<String>,
    pub branch: Option<String>,
    /// `null` when refused before judging.
    pub judged: Option<DriftJudged>,
    /// By node bytes.
    pub created: Vec<DriftCreated>,
    /// By node bytes.
    pub hits: Vec<DriftHit>,
    /// What the reader should know, a refusal's reason last; each one
    /// line.
    pub notes: Vec<String>,
}

/// What `spec propose drift` answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriftOutcome {
    pub document: DriftDocument,
    /// Why it was refused (exit 1); also the document's last note.
    pub refusal: Option<String>,
    pub messages: Vec<Message>,
    /// The text's item lines, by node bytes.
    lines: Vec<String>,
}

impl DriftOutcome {
    /// Exit 0 whether stored or not; 1 when refused.
    pub fn exit(&self) -> Exit {
        if self.refusal.is_some() {
            Exit::NotFound
        } else {
            Exit::Answered
        }
    }

    /// Refused: the document's notes every `note:` of `messages`, then the
    /// reason.
    fn refused(mut document: DriftDocument, reason: &str, messages: Vec<Message>) -> Self {
        let reason = one_line(reason);
        document.notes = notes_of(&messages);
        document.notes.push(reason.clone());
        Self {
            document,
            refusal: Some(reason),
            messages,
            lines: Vec::new(),
        }
    }
}

/// Every `note:` of `messages`, in order, each on one line: the document's
/// notes, so the JSON and stderr name the same.
fn notes_of(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Note(text) => Some(one_line(text)),
            Message::Warning(_) => None,
        })
        .collect()
}

impl Serialize for DriftOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.document.serialize(serializer)
    }
}

/// `spec propose drift`: the drift of the commit `HEAD` names queued, one
/// item per drifting node, bound to the task `task` when given.
pub fn propose_drift(
    env: &Env,
    globals: &Globals,
    request: &DriftRequest,
    task: Option<&str>,
) -> Result<DriftOutcome, CliError> {
    run_drift(env, globals, request, task).map_err(escaped_error)
}

/// What a REF named at the commit.
struct Named {
    /// The canonical ID, or a gone ID's bytes.
    id: String,
}

fn run_drift(
    env: &Env,
    globals: &Globals,
    request: &DriftRequest,
    task: Option<&str>,
) -> Result<DriftOutcome, CliError> {
    let now = checked_now(&request.now)?;
    let author = Author::new(
        request.author_role.clone(),
        request.author_model.clone(),
        request.run.clone(),
    )
    .map_err(CliError::spec)?;
    if request.references.len() > NODE_IDS_MAX {
        return Err(CliError::spec(format!(
            "{} REFs: name at most {NODE_IDS_MAX} nodes, or none for every drifting node",
            request.references.len()
        )));
    }
    let mut context = open_context(env, globals, &request.git)?;
    let mut messages = Vec::new();

    // 2. The place: `base_commit` the commit judged.
    let place = context.git.place(&context.project.root).map_err(|error| {
        CliError::spec(format!(
            "the project root {} cannot be bound to a proposal: {error}",
            context.project.root.display()
        ))
    })?;
    let mut document = DriftDocument {
        commit: Some(place.base_commit.clone()),
        branch: Some(place.branch.clone()),
        ..DriftDocument::default()
    };

    // 3. The config tracked and clean; `[code]` of the commit's.
    if let Some(state) = config_state(&place, &request.git)? {
        let reason = format!(
            "`{CONFIG_FILE}` is {state}: drift is judged under the config committed at HEAD; \
             commit it first; nothing queued"
        );
        return Ok(DriftOutcome::refused(document, &reason, messages));
    }
    let mut git = VerifyGit::open(&context.project.root, &request.git)
        .map_err(|cause| CliError::spec(cause.message))?;
    let config = match commit_config(&mut git, &place.base_commit) {
        Ok(config) => config,
        Err(cause) => return Err(run_cause(cause, git)),
    };

    // 4. The task, checked before the commit's side is read.
    let task = match task {
        Some(written) => match bound_task(&context, written, &place)? {
            Ok(id) => Some(id),
            Err(reason) => {
                let _ = git.finish();
                return Ok(DriftOutcome::refused(document, &reason, messages));
            }
        },
        None => None,
    };

    // 5. The commit's side alone.
    let side = match read_commit(&context.project, &mut git, config) {
        Ok(side) => side,
        Err(cause) => return Err(run_cause(cause, git)),
    };
    if let Err(cause) = git.finish() {
        return Err(CliError::spec(cause.message));
    }

    // 6. The REFs over the commit's documents.
    let mut named: Vec<Named> = Vec::new();
    for written in &request.references {
        match named_at(env, &context, &side, written.trim(), &mut messages)? {
            Ok(found) => {
                if !named.iter().any(|seen| seen.id == found.id) {
                    named.push(found);
                }
            }
            Err(reason) => return Ok(DriftOutcome::refused(document, &reason, messages)),
        }
    }

    // 7. The rows, per node.
    let judged =
        judge_commit(&side, &context.project, None).map_err(|cause| run_cause_text(&cause))?;
    let mut by_node: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    for row in &judged.rows {
        by_node.entry(row.key.node.as_str()).or_default().push(row);
    }
    let sweep = named.is_empty();
    // A sweep's nodes: those a row resolves, or a lock entry names (a gone
    // ID's); an unresolved marker's token is no node.
    let judged_nodes: Vec<&str> = if sweep {
        by_node
            .iter()
            .filter(|(_, rows)| rows.iter().any(|row| row.resolved || row.entry.is_some()))
            .map(|(node, _)| *node)
            .collect()
    } else {
        let mut ids: Vec<&str> = named.iter().map(|found| found.id.as_str()).collect();
        ids.sort_unstable();
        ids
    };
    let mut notes = Vec::new();
    let mut items: Vec<NewDrift> = Vec::new();
    let mut rows_judged = 0;
    for node in &judged_nodes {
        let rows = by_node.get(node).map_or(&[][..], Vec::as_slice);
        rows_judged += rows.len();
        let findings: Vec<&Row> = rows
            .iter()
            .copied()
            .filter(|row| row.judgement.state != State::Ok)
            .collect();
        if !sweep {
            let key_problems = findings
                .iter()
                .filter(|row| {
                    row.judgement.state == State::Unlocked
                        && row.judgement.detail.as_deref() == Some(KEY_PROBLEM)
                })
                .count();
            if key_problems > 0 {
                notes.push(format!(
                    "{node}: {key_problems} binding(s) cannot be locked (key_problem); spec \
                     verify names each"
                ));
            }
        }
        let raised = findings
            .iter()
            .any(|row| raises(row.judgement.state, row.judgement.detail.as_deref(), !sweep));
        if !raised {
            if !sweep {
                notes.push(format!("{node}: no drift at HEAD"));
            }
            continue;
        }
        items.push(new_drift(
            node,
            &findings,
            &place,
            &author,
            task.as_deref(),
        )?);
    }
    document.judged = Some(DriftJudged {
        rows: rows_judged,
        nodes: judged_nodes.len(),
    });

    // 8. The insert, deduplicated by the queue.
    let results = match context.queue.create_drift(&items, now) {
        Ok(results) => results,
        Err(QueueError::TaskRefused { reason, .. }) => {
            messages.extend(notes.into_iter().map(Message::Note));
            return Ok(DriftOutcome::refused(
                document,
                &format!("--task: {reason}"),
                messages,
            ));
        }
        Err(error) => return Err(queue_cannot(error)),
    };
    let mut lines = Vec::new();
    for (item, result) in items.iter().zip(results) {
        let node = item.target_id.clone();
        match result {
            DriftResult::Created { proposal, reverted } => {
                lines.push(format!(
                    "created {} | {node} | {} | {}",
                    proposal.id,
                    item.severity.as_str(),
                    item.summary
                ));
                if let Some((applied, commit)) = reverted {
                    notes.push(format!(
                        "{}: {node} drifts as when {applied} was applied by {}; that lock is not \
                         in effect at the commit judged",
                        proposal.id,
                        commit.as_deref().unwrap_or("its commit")
                    ));
                }
                document.created.push(DriftCreated {
                    id: proposal.id.clone(),
                    node,
                    severity: item.severity.as_str().to_owned(),
                    summary: item.summary.clone(),
                });
            }
            DriftResult::Hit { id, status, same } => {
                let (word, evidence) = match (status, same) {
                    (ProposalStatus::Rejected, _) => ("settled", "same evidence"),
                    (_, true) => ("queued", "same evidence"),
                    (_, false) => ("queued", "evidence changed since it was raised"),
                };
                lines.push(format!("{word} {id} | {node} | {status} | {evidence}"));
                document.hits.push(DriftHit {
                    id,
                    node,
                    status: status.as_str().to_owned(),
                    same,
                });
            }
        }
    }
    notes.extend(ok_now(
        &context,
        &place,
        &judged_nodes,
        &by_node,
        &document,
    )?);
    // The JSON's notes are every `note:` line, in order: a REF's ignored
    // `@rev` first.
    messages.extend(notes.into_iter().map(Message::Note));
    document.notes = notes_of(&messages);
    Ok(DriftOutcome {
        document,
        refusal: None,
        messages,
        lines,
    })
}

/// `specengine.toml`'s state when it is not tracked and clean
/// (`modified`, `staged`, `untracked`, `deleted`, `not tracked`), as `spec
/// lock accept` names a dirty input; `None`: committed as it is.
fn config_state(place: &Place, git_env: &GitEnv) -> Result<Option<&'static str>, CliError> {
    let git = WorktreeGit::new(&place.worktree, git_env)
        .map_err(|error| CliError::spec(format!("cannot run git: {error}")))?;
    let top = if place.root_rel.is_empty() {
        CONFIG_FILE.to_owned()
    } else {
        format!("{}/{CONFIG_FILE}", place.root_rel)
    };
    let failed = |error: specengine_store::GitError| CliError::spec(format!("git: {error}"));
    let entries = git.status_entries(&[top.as_str()], true).map_err(failed)?;
    if let Some(entry) = entries.iter().find(|entry| entry.path == top) {
        return Ok(Some(state_of(entry)));
    }
    if git
        .blob_oid(&place.base_commit, &top)
        .map_err(failed)?
        .is_none()
    {
        return Ok(Some(NOT_TRACKED));
    }
    Ok(None)
}

/// A run cause at the commit: exit 2, its message; the session ended.
fn run_cause(cause: RunCause, git: VerifyGit) -> CliError {
    let _ = git.finish();
    run_cause_text(&cause)
}

fn run_cause_text(cause: &RunCause) -> CliError {
    CliError::spec(format!("cannot judge the drift: {}", cause.message))
}

/// What `written` names at the commit: one node with an ID (its canonical
/// ID), or a gone ID with entries in the commit's lock; `Ok(Err(reason))`
/// refuses (exit 1): several nodes, none, a node without an ID, a node
/// only the working tree has.
fn named_at(
    env: &Env,
    context: &QueueContext,
    side: &CommitSide,
    written: &str,
    messages: &mut Vec<Message>,
) -> Result<Result<Named, String>, CliError> {
    let project = &context.project;
    let scheme = &project.config.scheme;
    let target = classify(written, scheme, messages, REV_IGNORED)?;
    let corpus = Corpus::new(side.input(), scheme, &project.config.paths);
    let nodes = match &target {
        Ok(target) => corpus.target_nodes(target, written)?,
        Err(reason) => Err(reason.clone()),
    };
    match nodes {
        Ok(nodes) if nodes.len() > 1 => Ok(Err(format!(
            "`{written}` names {} nodes at HEAD: name one (`slug/ID`, `ID#SECTION` or its file)",
            nodes.len()
        ))),
        Ok(nodes) => {
            let Some(key) = nodes.first() else {
                return Ok(Err(format!("`{written}` names no node at HEAD")));
            };
            let path = corpus.resolver.paths()[key.file];
            let id = corpus.files[key.file]
                .parsed
                .as_ref()
                .and_then(|parsed| parsed.nodes.get(key.ord))
                .and_then(|node| node.id.clone());
            Ok(match id {
                Some(id) => Ok(Named {
                    id: canonical_id(&corpus.resolver, scheme, path, &id, None),
                }),
                None => Err(format!(
                    "`{written}`: the node has no ID, and spec.lock records nodes by ID"
                )),
            })
        }
        Err(reason) => {
            // A gone ID: its entries in the commit's lock, no node of it at
            // the commit.
            let gone = without_rev(written);
            if side.lock.entries_of(gone).next().is_some()
                && !canonical_ids(side.input(), scheme, project).contains(gone)
            {
                return Ok(Ok(Named {
                    id: gone.to_owned(),
                }));
            }
            // Only the working tree has it: commit it first.
            if let Ok(target) = &target {
                let mut quiet = Vec::new();
                let input = indexed(env, project, &mut quiet, false)?;
                let current = Corpus::new(&input, scheme, &project.config.paths);
                if current
                    .target_nodes(target, written)?
                    .is_ok_and(|nodes| !nodes.is_empty())
                {
                    return Ok(Err(format!(
                        "`{written}` names no node at HEAD: commit it first"
                    )));
                }
            }
            Ok(Err(reason))
        }
    }
}

/// One node's item: its non-`ok` rows (`findings`, key order) as stored
/// (the first [`DRIFT_ROWS_MAX`], `pre_existing` `true`) and as hashed
/// (all), its severity and summary.
fn new_drift(
    node: &str,
    findings: &[&Row],
    place: &Place,
    author: &Author,
    task: Option<&str>,
) -> Result<NewDrift, CliError> {
    let stored: Vec<DriftRow> = findings
        .iter()
        .take(DRIFT_ROWS_MAX)
        .map(|row| {
            let mut shown = shown(row);
            shown.pre_existing = true;
            drift_row(&shown)
        })
        .collect();
    let evidence = DriftEvidence {
        rows: stored,
        more: (findings.len().saturating_sub(DRIFT_ROWS_MAX)) as u64,
    };
    let evidence = serde_json::to_string(&evidence)
        .map_err(|error| CliError::spec(format!("cannot encode the drift rows: {error}")))?;
    let hashed: Vec<HashedRow<'_>> = findings.iter().map(|row| hashed_row(row)).collect();
    let hash = b3_hash(drift_hash_input(&hashed).as_bytes());
    let severity = if findings
        .iter()
        .any(|row| row.judgement.state.class() == Some(Class::Fix))
    {
        IntakeSeverity::High
    } else {
        IntakeSeverity::Normal
    };
    let summary = drift_summary(findings.iter().map(|row| {
        (
            row.judgement.state.as_str(),
            row.judgement.category.as_deref(),
        )
    }));
    Ok(NewDrift {
        target_id: node.to_owned(),
        target_path: LOCK_FILE.to_owned(),
        place: place.clone(),
        author: author.clone(),
        task_id: task.map(str::to_owned),
        severity,
        summary,
        evidence,
        hash,
    })
}

/// A shown row as stored: `spec verify --json`'s row, key for key.
fn drift_row(row: &VerifyRow) -> DriftRow {
    DriftRow {
        node: row.node.clone(),
        relation: row.relation.to_owned(),
        file: row.file.clone(),
        kind: row.kind.map(str::to_owned),
        symbol: row.symbol.clone(),
        item_kind: row.item_kind.clone(),
        state: row.state.as_str().to_owned(),
        category: row.category.clone(),
        detail: row.detail.clone(),
        class: row.class.map(str::to_owned),
        pre_existing: row.pre_existing,
        markers: row
            .markers
            .iter()
            .map(|marker| DriftMarker {
                line: marker.line,
                rev: marker.rev,
            })
            .collect(),
        node_rev: row.node_rev,
        lock: row.lock.as_ref().map(|lock| DriftLock {
            rev: lock.rev,
            commit: lock.commit.clone(),
            verified_at: lock.verified_at.clone(),
        }),
        compared: row.compared.map(str::to_owned),
    }
}

/// A judged row as its item's hash covers it (no line, no time, no
/// commit).
fn hashed_row(row: &Row) -> HashedRow<'_> {
    let (kind, symbol, item_kind) = match &row.key.binding {
        RowBinding::Keyed {
            kind,
            symbol,
            item_kind,
        } => (
            Some(kind.as_str()),
            Some(symbol.as_str()),
            item_kind.as_deref(),
        ),
        RowBinding::Keyless { .. } => (None, None, None),
    };
    HashedRow {
        node: &row.key.node,
        relation: row.key.relation.as_str(),
        file: &row.key.file,
        kind,
        symbol,
        item_kind,
        state: row.judgement.state,
        category: row.judgement.category.as_deref(),
        detail: row.judgement.detail.as_deref(),
        marker_revs: row.markers.iter().map(|marker| marker.rev).collect(),
        entry: row.entry.as_ref().map(|entry| HashedEntry {
            rev: entry.rev,
            spec_hash: &entry.spec_hash,
            code: entry.ast_hash.as_deref(),
            file_blob: &entry.file_blob,
        }),
        rev: row.node_rev,
        norm_hash: row.norm_hash.as_deref(),
        code: row.code.as_deref(),
    }
}

/// The notes on each `open` drift item of this repository and branch whose
/// node was judged now and read all `ok` (none of its rows a finding), by
/// ID number, the items this raise found left out.
fn ok_now(
    context: &QueueContext,
    place: &Place,
    judged_nodes: &[&str],
    by_node: &BTreeMap<&str, Vec<&Row>>,
    document: &DriftDocument,
) -> Result<Vec<String>, CliError> {
    let listed = context
        .queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Open],
        })
        .map_err(queue_cannot)?;
    let touched: BTreeSet<&str> = document
        .created
        .iter()
        .map(|created| created.id.as_str())
        .chain(document.hits.iter().map(|hit| hit.id.as_str()))
        .collect();
    let judged: BTreeSet<&str> = judged_nodes.iter().copied().collect();
    let mut notes = Vec::new();
    for proposal in &listed.proposals {
        if proposal.kind.family() != KindFamily::Locks
            || proposal.place.branch != place.branch
            || !same_repository(&proposal.place.git_common_dir, &context.common_dir)
            || touched.contains(proposal.id.as_str())
            || !judged.contains(proposal.target_id.as_str())
        {
            continue;
        }
        let all_ok = by_node
            .get(proposal.target_id.as_str())
            .is_none_or(|rows| rows.iter().all(|row| row.judgement.state == State::Ok));
        if all_ok {
            notes.push(format!(
                "{id}: {} reads ok at HEAD now; spec reject {id} --reason … settles it",
                proposal.target_id,
                id = proposal.id
            ));
        }
    }
    Ok(notes)
}

/// The text: each item's line by node bytes, then the summary line; a
/// refusal prints nothing (stderr says why). Control characters escaped.
pub(crate) fn render_text(outcome: &DriftOutcome) -> String {
    if outcome.refusal.is_some() {
        return String::new();
    }
    // Built in item order: node bytes.
    let mut out = String::new();
    for line in &outcome.lines {
        out.push_str(&one_line(line));
        out.push('\n');
    }
    let document = &outcome.document;
    let judged = document.judged.unwrap_or(DriftJudged { rows: 0, nodes: 0 });
    out.push_str(&format!(
        "propose drift: {} created, {} hit(s); {} rows of {} nodes judged at {} on {}\n",
        document.created.len(),
        document.hits.len(),
        judged.rows,
        judged.nodes,
        document.commit.as_deref().unwrap_or_default(),
        document.branch.as_deref().unwrap_or_default()
    ));
    escape_controls(&out)
}
