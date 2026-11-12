//! `spec show --history` (task spec `node-history`, NH1 to NH10): each
//! shown node's changes, newest first, built per read from git and the
//! queue and never stored; it feeds no rollup, mark or gate: nothing is
//! blocked by it.
//!
//! Git: once per read, the handle in the root, `rev-parse
//! --show-toplevel` (not a repository), `rev-parse
//! --is-shallow-repository` (a shallow clone), `HEAD` (unborn: no commit
//! entry, no note) and the common dir; then per holder one `log --follow`
//! of at most [`HISTORY_SCAN_MAX`] + 1 commits (the store's
//! [`WorktreeGit::file_log`]) and one `cat-file --batch` session, each blob
//! OID asked once, opened at the first blob asked. The commits are judged
//! newest first by core's [`judge`]: listed only when the node's span bytes
//! differ from the first parent's; the scan stops at the node's creation,
//! at a version that holds no such node, or at the file's first commit.
//! Every git command runs with `GIT_NO_LAZY_FETCH=1`.
//!
//! The queue, once per read, opened as it is (no schema step, nothing
//! written): the accepted cards targeting the node or a section inside it,
//! and this repository's applied questions, discrepancies and decisions
//! naming them; an item a listed commit's `Proposal:` names is not listed
//! again (NH8).
//!
//! A cause never fails the read (NH5): a holder not read gives one
//! `note: history: <path>: not read: <cause>` line and its nodes'
//! `history` `None`; a queue that cannot be read gives one `note: history:
//! the queue not read: <why>; commits only`.

use std::collections::{BTreeMap, BTreeSet};
use std::panic::{self, AssertUnwindSafe};
use std::path::PathBuf;

use serde::Serialize;
use specengine_core::history::{
    AppliedCommit, CommitChange, HISTORY_MAX, HISTORY_SCAN_MAX, HistoryKey, Judged, NodeVersion,
    applied_trailers, history_text, identity_name, judge, node_version, proposer,
};
use specengine_core::proposal::utc_timestamp;
use specengine_model::{Card, IdScheme};
use specengine_store::{
    BatchBlob, BlobBatch, GitEnv, GitError, LoggedCommit, Proposal, ProposalFilter, ProposalKind,
    ProposalQueue as _, ProposalStatus, QUEUE_SCHEMA_VERSION, SqliteQueue, WorktreeGit,
    same_repository,
};

use crate::location::db_path;
use crate::project::ProjectRoot;
use crate::proposals::queue_below_schema;
use crate::show::ShownNode;
use crate::{Env, one_line};

/// A shown node's history (task spec `node-history` NH1): its entries,
/// newest first, at most [`HISTORY_MAX`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownHistory {
    pub entries: Vec<HistoryEntry>,
    /// More existed than listed, or the scan stopped short of the node's
    /// creation (NH9).
    pub truncated: bool,
}

/// One change of a node: a commit, an accepted card or a decided item; the
/// 15 keys of every source, in this order (NH6, NH12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryEntry {
    /// `commit`, `card` or `item`.
    pub source: &'static str,
    /// UTC, `YYYY-MM-DDTHH:MM:SSZ`: a commit's committer time, a card's
    /// `acceptance.at`, an item's `decided_at`.
    pub at: String,
    /// A commit's full OID, a card's or an item's ID.
    pub id: String,
    /// A commit's `raised`, `editorial`, `applied` or `hand`; a card's
    /// `accepted`; an item's kind.
    pub change: &'static str,
    /// A commit whose first parent holds no such node, or a root commit.
    pub created: bool,
    /// A commit's revisions after and before (`rev_before` `None` when
    /// created); `None` elsewhere.
    pub rev: Option<u32>,
    pub rev_before: Option<u32>,
    /// As written: `Decided-by`, else the author `Name <email>`; a card's
    /// `acceptance.by`; an item's `decided_by`.
    pub by: String,
    /// [`identity_name`] of `by`.
    pub by_name: String,
    /// An apply commit's trailers as written and [`proposer`] of
    /// `proposed_by`; `None` elsewhere.
    pub proposal: Option<String>,
    pub proposed_by: Option<String>,
    pub proposer: Option<String>,
    /// An item's decision record.
    pub record: Option<String>,
    /// A hand commit's subject, a card's title: one line, cut at 300
    /// characters.
    pub title: Option<String>,
    /// An apply commit's rationale: one line, cut at 300 characters.
    pub rationale: Option<String>,
}

/// The cause of a blob a partial clone lacks (NH5).
const BLOB_MISSING: &str = "a blob is missing (a partial clone?)";

/// What `--history` reads beside the shown nodes.
pub(crate) struct HistoryRead<'a> {
    pub env: &'a Env,
    pub project: &'a ProjectRoot,
    pub git: &'a GitEnv,
    /// REF is a root-relative path: each node is its whole file.
    pub whole_file: bool,
}

/// Each shown node's history; the notes, in order: one per holder not
/// read, then the queue's.
pub(crate) fn add_history(read: &HistoryRead<'_>, nodes: &mut [ShownNode]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for node in nodes.iter() {
        if !paths.contains(&node.path) {
            paths.push(node.path.clone());
        }
    }
    let repo = match Repository::open(&read.project.root, read.git) {
        Ok(repo) => repo,
        Err(cause) => return paths.iter().map(|path| not_read(path, &cause)).collect(),
    };
    let (queue, queue_why) = match read_queue(read, &repo.common_dir) {
        Ok(queue) => (queue, None),
        Err(why) => (QueueFacts::default(), Some(why)),
    };
    let scheme = &read.project.config.scheme;
    let mut notes = Vec::new();
    for path in &paths {
        let held: Vec<usize> = (0..nodes.len())
            .filter(|&at| nodes[at].path == *path)
            .collect();
        let keys: Vec<HistoryKey<'_>> = held
            .iter()
            .map(|&at| match (&nodes[at].id, read.whole_file) {
                (Some(id), false) => HistoryKey::Id(id),
                _ => HistoryKey::File,
            })
            .collect();
        let ids: Vec<Option<&str>> = held.iter().map(|&at| nodes[at].id.as_deref()).collect();
        let scans = match scan_path(&repo, scheme, path, &keys, &ids) {
            Ok(scans) => scans,
            Err(cause) => {
                notes.push(not_read(path, &cause));
                continue;
            }
        };
        let histories: Vec<ShownHistory> = held
            .iter()
            .zip(scans)
            .map(|(&at, scan)| node_history(&nodes[at], scan, &queue))
            .collect();
        for (&at, history) in held.iter().zip(histories) {
            nodes[at].history = Some(history);
        }
    }
    if let Some(why) = queue_why
        && nodes.iter().any(|node| node.history.is_some())
    {
        notes.push(format!("history: the queue not read: {why}; commits only"));
    }
    notes
}

/// `history: <path>: not read: <cause>`.
fn not_read(path: &str, cause: &str) -> String {
    format!("history: {path}: not read: {cause}")
}

/// A git failure's cause, one line: git missing, else git's first stderr
/// line, else the failure (`spec catalog`'s too).
pub(crate) fn git_cause(error: &GitError) -> String {
    match error {
        GitError::NotRunnable(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "git missing".to_owned()
        }
        GitError::Failed { stderr, .. } => stderr
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map_or_else(|| one_line(&error.to_string()), str::to_owned),
        _ => one_line(&error.to_string()),
    }
}

/// The repository the root lies in, read once per read.
struct Repository {
    git: WorktreeGit,
    /// `HEAD`'s commit; `None` when unborn.
    head: Option<String>,
    /// The canonical git common dir: which queue items are this
    /// repository's.
    common_dir: PathBuf,
}

impl Repository {
    /// The root's repository; the cause when its history cannot be read:
    /// git missing, not a repository, a shallow clone, a git failure.
    fn open(root: &std::path::Path, env: &GitEnv) -> Result<Self, String> {
        let cause = |error: GitError| git_cause(&error);
        let git = WorktreeGit::new(root, env).map_err(cause)?;
        if git.top_if_repository().map_err(cause)?.is_none() {
            return Err("not a repository".to_owned());
        }
        if git.is_shallow().map_err(cause)? {
            return Err("a shallow clone".to_owned());
        }
        let head = git.head().map_err(cause)?;
        let common_dir = git.common_dir().map_err(cause)?;
        Ok(Self {
            git,
            head,
            common_dir,
        })
    }
}

/// One node's commit entries, as the scan left them.
#[derive(Default)]
struct NodeScan {
    entries: Vec<HistoryEntry>,
    /// Stopped: created, or a version holding no such node.
    done: bool,
    truncated: bool,
    /// The committer time of the last commit judged, when the scan
    /// stopped short: queue entries older than it are dropped.
    oldest: Option<i64>,
}

/// The nodes of one holder judged over one log and one blob session: each
/// blob asked once, parsed once, its versions of every node kept by OID.
struct PathScan<'a> {
    git: &'a WorktreeGit,
    scheme: &'a IdScheme,
    /// The holder's root-relative path: names each parse.
    path: &'a str,
    keys: &'a [HistoryKey<'a>],
    batch: Option<BlobBatch>,
    /// Per blob OID, each node's version in it (`None`: no such node).
    versions: BTreeMap<String, Vec<Option<NodeVersion>>>,
}

impl PathScan<'_> {
    /// Reads the blob `blob` (a commit's `<commit>:<path>`, its parent's
    /// `<commit>^:<old path>`) unless read already: each OID asked once.
    fn read(&mut self, blob: &str) -> Result<(), String> {
        if self.versions.contains_key(blob) {
            return Ok(());
        }
        let batch = match &mut self.batch {
            Some(batch) => batch,
            None => self
                .batch
                .insert(self.git.blob_batch().map_err(|error| git_cause(&error))?),
        };
        let found = match batch.blob(blob).map_err(|error| git_cause(&error))? {
            BatchBlob::Bytes(bytes) => {
                let (path, scheme) = (self.path, self.scheme);
                let parsed = panic::catch_unwind(AssertUnwindSafe(|| {
                    specengine_core::parse(path, &bytes, scheme)
                }))
                .map_err(|_| format!("the spec parser failed on its blob {}", short_oid(blob)))?;
                self.keys
                    .iter()
                    .map(|key| node_version(&parsed, &bytes, *key))
                    .collect()
            }
            BatchBlob::Missing => return Err(BLOB_MISSING.to_owned()),
            // Not a file's blob: no node in it.
            BatchBlob::Foreign => vec![None; self.keys.len()],
        };
        self.versions.insert(blob.to_owned(), found);
        Ok(())
    }

    /// Node `at`'s version in the blob `blob`, read already.
    fn version(&self, blob: &str, at: usize) -> Option<&NodeVersion> {
        self.versions
            .get(blob)
            .and_then(|versions| versions.get(at))
            .and_then(Option::as_ref)
    }
}

/// The commit entries of the nodes `keys` of the holder `path` (`ids`:
/// each node's ID, which a `Revision:` names), in log order; the cause
/// when its history cannot be read.
fn scan_path(
    repo: &Repository,
    scheme: &IdScheme,
    path: &str,
    keys: &[HistoryKey<'_>],
    ids: &[Option<&str>],
) -> Result<Vec<NodeScan>, String> {
    let mut scans: Vec<NodeScan> = keys.iter().map(|_| NodeScan::default()).collect();
    if repo.head.is_none() {
        return Ok(scans);
    }
    let log = repo
        .git
        .file_log(path, HISTORY_SCAN_MAX + 1)
        .map_err(|error| git_cause(&error))?;
    let mut scan = PathScan {
        git: &repo.git,
        scheme,
        path,
        keys,
        batch: None,
        versions: BTreeMap::new(),
    };
    for (index, commit) in log.iter().enumerate() {
        if scans.iter().all(|scan| scan.done) {
            break;
        }
        if index == HISTORY_SCAN_MAX {
            for node in scans.iter_mut().filter(|scan| !scan.done) {
                node.truncated = true;
                node.oldest = Some(log[index - 1].committed_at);
            }
            break;
        }
        let Some(change) = &commit.change else {
            // The file is its first parent's: nothing changed.
            continue;
        };
        let Some(after) = &change.after else {
            // The commit deletes the file: no node of it lives on here.
            for node in &mut scans {
                node.done = true;
            }
            continue;
        };
        scan.read(after)?;
        let wanted = (0..keys.len()).any(|at| !scans[at].done && scan.version(after, at).is_some());
        if wanted && let Some(before) = &change.before {
            scan.read(before)?;
        }
        let applied = applied_trailers(&commit.message);
        for (at, node) in scans.iter_mut().enumerate() {
            if node.done {
                continue;
            }
            let before = change
                .before
                .as_ref()
                .and_then(|before| scan.version(before, at));
            match judge(scan.version(after, at), before) {
                Judged::Same => {}
                Judged::Absent => node.done = true,
                Judged::Changed {
                    created,
                    rev,
                    rev_before,
                } => {
                    node.entries.push(commit_entry(
                        commit,
                        applied.as_ref(),
                        ids[at],
                        created,
                        rev,
                        rev_before,
                    ));
                    node.done = created;
                }
            }
        }
    }
    if let Some(batch) = scan.batch {
        batch.finish().map_err(|error| git_cause(&error))?;
    }
    Ok(scans)
}

/// A listed commit's entry.
fn commit_entry(
    commit: &LoggedCommit,
    applied: Option<&AppliedCommit>,
    id: Option<&str>,
    created: bool,
    rev: Option<u32>,
    rev_before: Option<u32>,
) -> HistoryEntry {
    let by = applied
        .and_then(|applied| applied.decided_by.clone())
        .unwrap_or_else(|| format!("{} <{}>", commit.author_name, commit.author_email));
    let proposed_by = applied.and_then(|applied| applied.proposed_by.clone());
    let title = match applied {
        Some(_) => None,
        None => Some(subject(&commit.message))
            .filter(|subject| !subject.is_empty())
            .map(|subject| shown_text(&subject)),
    };
    HistoryEntry {
        source: "commit",
        at: utc_timestamp(commit.committed_at),
        id: commit.oid.clone(),
        change: CommitChange::of(applied, id).as_str(),
        created,
        rev,
        rev_before,
        by_name: identity_name(&by).to_owned(),
        by,
        proposal: applied.map(|applied| applied.proposal.clone()),
        proposer: proposed_by.as_deref().and_then(proposer),
        proposed_by,
        record: None,
        title,
        rationale: applied
            .and_then(|applied| applied.rationale.as_deref())
            .map(shown_text),
    }
}

/// A message's subject as git reads it (`%s`): its first paragraph, the
/// blank lines before it skipped, each line's trailing whitespace dropped,
/// the lines joined with one space.
fn subject(message: &str) -> String {
    message
        .lines()
        .map(str::trim_end)
        .skip_while(|line| line.is_empty())
        .take_while(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A title or a rationale as an entry holds it: one line, cut at 300
/// characters.
fn shown_text(text: &str) -> String {
    history_text(&one_line(text))
}

/// An OID's first 12 hex digits, as the text prints a commit.
pub(crate) fn short_oid(oid: &str) -> &str {
    oid.get(..12).unwrap_or(oid)
}

/// What the queue holds for every shown node, read once.
#[derive(Default)]
struct QueueFacts {
    /// The project's accepted cards.
    cards: Vec<Card>,
    /// This repository's applied questions, discrepancies and decisions.
    items: Vec<Proposal>,
}

/// The queue as it is, no schema step and nothing written: none, or one
/// with no queue step run yet, holds nothing; the reason it cannot be read.
fn read_queue(read: &HistoryRead<'_>, common_dir: &std::path::Path) -> Result<QueueFacts, String> {
    let slug = read
        .project
        .slug()
        .map_err(|error| one_line(&error.to_string()))?;
    let db = db_path(read.env, slug).map_err(|error| one_line(&error.to_string()))?;
    let text = |error: specengine_store::QueueError| one_line(&error.to_string());
    let Some(queue) = SqliteQueue::open_existing(&db, slug).map_err(text)? else {
        return Ok(QueueFacts::default());
    };
    let schema = queue.schema_version().map_err(text)?;
    if schema == 0 {
        return Ok(QueueFacts::default());
    }
    if schema < QUEUE_SCHEMA_VERSION {
        return Err(queue_below_schema(schema));
    }
    let cards = queue
        .list_existing_cards()
        .map_err(text)?
        .cards
        .into_iter()
        .filter(|card| card.acceptance.is_some())
        .collect();
    let items = queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Applied],
        })
        .map_err(text)?
        .proposals
        .into_iter()
        .filter(|item| {
            matches!(
                item.kind,
                ProposalKind::Question | ProposalKind::Discrepancy | ProposalKind::Decision
            ) && same_repository(&item.place.git_common_dir, common_dir)
        })
        .collect();
    Ok(QueueFacts { cards, items })
}

/// A node's history: its commit entries and the queue's joined, ordered,
/// capped (NH8, NH9).
fn node_history(node: &ShownNode, scan: NodeScan, queue: &QueueFacts) -> ShownHistory {
    let NodeScan {
        mut entries,
        mut truncated,
        oldest,
        ..
    } = scan;
    // The node's ID and the IDs of the sections inside it; an ID-less
    // document by its path, as a card targets it.
    let mut ids: BTreeSet<&str> = node
        .sections
        .iter()
        .map(|section| section.id.as_str())
        .collect();
    match &node.id {
        Some(id) => ids.insert(id.as_str()),
        None => ids.insert(node.path.as_str()),
    };
    let listed: BTreeSet<String> = entries
        .iter()
        .filter_map(|entry| entry.proposal.clone())
        .collect();
    let oldest = oldest.map(utc_timestamp);
    let kept = |at: &str| oldest.as_deref().is_none_or(|oldest| at >= oldest);
    for card in &queue.cards {
        let Some(acceptance) = &card.acceptance else {
            continue;
        };
        if !card
            .targets
            .iter()
            .any(|target| ids.contains(target.as_str()))
            || !kept(&acceptance.at)
        {
            continue;
        }
        entries.push(HistoryEntry {
            source: "card",
            at: acceptance.at.clone(),
            id: card.id.clone(),
            change: "accepted",
            created: false,
            rev: None,
            rev_before: None,
            by: acceptance.by.clone(),
            by_name: identity_name(&acceptance.by).to_owned(),
            proposal: None,
            proposed_by: None,
            proposer: None,
            record: None,
            title: Some(shown_text(&card.title)),
            rationale: None,
        });
    }
    for item in &queue.items {
        let at = item.decided_at.as_deref().unwrap_or(&item.updated_at);
        if listed.contains(&item.id)
            || !item
                .target_ids()
                .iter()
                .any(|target| ids.contains(target.as_str()))
            || !kept(at)
        {
            continue;
        }
        let by = item.decided_by.clone().unwrap_or_default();
        entries.push(HistoryEntry {
            source: "item",
            at: at.to_owned(),
            id: item.id.clone(),
            change: item.kind.as_str(),
            created: false,
            rev: None,
            rev_before: None,
            by_name: identity_name(&by).to_owned(),
            by,
            proposal: None,
            proposed_by: None,
            proposer: None,
            record: item.record.as_ref().map(|record| record.id.clone()),
            title: None,
            rationale: None,
        });
    }
    // Newest first; on a tie commits (in log order: the sort is stable),
    // then cards, then items, each by ID.
    entries.sort_by(|a, b| {
        b.at.cmp(&a.at)
            .then_with(|| rank(a).cmp(&rank(b)))
            .then_with(|| match a.source {
                "commit" => std::cmp::Ordering::Equal,
                _ => (a.id.len(), &a.id).cmp(&(b.id.len(), &b.id)),
            })
    });
    if entries.len() > HISTORY_MAX {
        entries.truncate(HISTORY_MAX);
        truncated = true;
    }
    ShownHistory { entries, truncated }
}

/// An entry's place among entries of one time: commits, cards, items.
fn rank(entry: &HistoryEntry) -> u8 {
    match entry.source {
        "commit" => 0,
        "card" => 1,
        _ => 2,
    }
}
