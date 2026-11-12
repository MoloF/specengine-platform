//! `spec catalog --kind K… [--archive]` (task spec `direction-catalog`,
//! DC-1 to DC-9): a kinds direction as a table of its nodes, each with its
//! version, work state, open items, tasks and last change, computed per
//! read and never stored. It moves, refuses and gates nothing.
//!
//! One call, four reads (DC-8): the tree's lines for those kinds with no
//! root, over the index it refreshes ([`direction_lines`], DC-1); the
//! trace's reading of those nodes, built once as `spec trace` builds it
//! ([`node_work`], DC-2); the queue's open proposals once, this
//! repository's (DC-3); git once (DC-4): `rev-parse` for the repository
//! and `HEAD`, then, for the files no cache holds at that `HEAD`, the
//! store's [`WorktreeGit::last_commits_at`]. A [`LastChangeCache`] (the
//! daemon's) keeps each file's answer per (git common dir, `HEAD`): a
//! second read at one `HEAD` runs `rev-parse` alone of git. The CLI keeps
//! none.
//!
//! ```text
//! catalog <kinds joined ", "> | rows <n>[ [archive]]
//! <id> | version <rev or -> | <state> | open items <n> | tasks <t>, open <o> | <at>[ (file)] | <by> | <proposal or hand> | <title>
//! [truncated: <k> of <n> rows not shown]
//! ```
//!
//! A row with no last change prints `- | - | -` for its three. Every value
//! is on one line, every line's control characters escaped; the JSON holds
//! the values raw. The header prints, then the rows, each whole, while
//! they fit in [`crate::OUTPUT_CAP_CHARS`] (the first always); the JSON holds the
//! rows the text prints ([`catalog_with_view`] with the daemon's
//! [`View::Browser`]: nothing cut, `truncated` false, `omitted` `null`).
//!
//! The notes go to stderr as `note:` lines and into the JSON's `notes`,
//! sorted, each once; the tree's warnings follow them on stderr alone.
//! None changes the exit. A git problem hiding a shown row's last change
//! is a note counting the files it hides among the rows shown; a queue
//! with no queue step yet a note when a row is shown. Exit 2: no `--kind`
//! (nothing read), no project, an index or a queue that cannot be read (a
//! newer build's).

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;
use specengine_core::catalog::{
    CommitFacts, LAST_CHANGE_SCAN_MAX, LastChange, OpenItem, last_change, open_items_by_node,
};
use specengine_core::check::{NodeAt, SpecGraph};
use specengine_core::revision;
use specengine_core::trace::Rollup;
use specengine_store::{
    GitEnv, LastCommits, LoggedCommit, Proposal, ProposalFilter, ProposalQueue as _,
    ProposalStatus, SqliteQueue, WorktreeGit, same_repository,
};

use crate::cap::{View, lines_within};
use crate::corpus::indexed_in;
use crate::history::git_cause;
use crate::location::{db_path, open_index};
use crate::project::{ProjectRoot, discover};
use crate::proposals::queue_cannot;
use crate::subset::seams::{self, Event as Seam};
use crate::trace::{NodeWork, node_work, queue_read};
use crate::tree::{TreeNode, direction_lines};
use crate::{CliError, Env, Globals, Message, escape_controls, one_line, process_git, store_error};

/// The `HEAD`s a [`LastChangeCache`] keeps at most, the oldest dropped.
pub const LAST_CHANGE_HEADS: usize = 4;

/// The refusal of a call with no `--kind` (exit 2, nothing read).
const NO_KIND: &str = "spec catalog: give at least one `--kind`";

/// The note of a queue no queue step has run on.
const NO_QUEUE: &str = "catalog: no queue yet: no open item or task read";

/// What a text line shows for no value.
const ABSENT: &str = "-";

/// The cause of a root git places outside the worktree it names.
const ROOT_OUTSIDE: &str = "the project root is not in its git worktree";

/// The text's word for a commit with no `Proposal:` trailer.
const HAND: &str = "hand";

/// `spec catalog` options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CatalogRequest {
    /// `--kind`: the direction's kinds, at least one (an empty one names no
    /// node, as `spec tree --kind ""` reads it).
    pub kinds: Vec<String>,
    /// `--archive`: Tier 3 files too.
    pub archive: bool,
}

/// One node of the direction: the tree's line's keys as `spec tree` serves
/// them, but `rev` its revision's number (none written is 1, an unreadable
/// one `None`: "Version N", D3), then its work and its last change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRow {
    pub id: Option<String>,
    pub kind: Option<String>,
    pub title: Option<String>,
    pub path: String,
    pub line: usize,
    /// Core's `revision(..).number()`.
    pub rev: Option<u32>,
    pub status: Option<String>,
    pub archived: bool,
    /// The trace's rollup of this node.
    pub state: Rollup,
    /// This repository's open proposals naming its ID.
    pub open_items: u64,
    /// Its cards, cancelled ones never read, and of them the open.
    pub tasks: usize,
    pub tasks_open: usize,
    /// Its file's newest commit on `HEAD`'s first-parent line; `None` when
    /// none is read.
    pub last_change: Option<LastChange>,
}

/// What `spec catalog` read and how much of it prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogOutcome {
    /// As asked.
    pub kinds: Vec<String>,
    pub archive: bool,
    /// Sorted, each once.
    pub notes: Vec<String>,
    /// The tree's, in the order met, each once: stderr alone.
    pub warnings: Vec<String>,
    /// Every row, in the tree's order.
    pub rows: Vec<CatalogRow>,
    /// How many rows print, the first ones.
    pub shown: usize,
}

impl CatalogOutcome {
    /// A row does not print.
    pub fn truncated(&self) -> bool {
        self.shown < self.rows.len()
    }

    /// The rows that print.
    fn shown_rows(&self) -> &[CatalogRow] {
        &self.rows[..self.shown.min(self.rows.len())]
    }

    /// One `note:` line per note, then one `warning:` line per warning,
    /// escaped.
    pub(crate) fn stderr_lines(&self) -> Vec<String> {
        let notes = self
            .notes
            .iter()
            .map(|note| Message::Note(note.clone()).line());
        let warnings = self
            .warnings
            .iter()
            .map(|warning| Message::Warning(warning.clone()).line());
        notes
            .chain(warnings)
            .map(|line| escape_controls(&line))
            .collect()
    }
}

/// The JSON document: every key present, in this order.
#[derive(Serialize)]
struct CatalogJson<'a> {
    kinds: &'a [String],
    archive: bool,
    notes: &'a [String],
    rows: Vec<RowJson<'a>>,
    truncated: bool,
    /// The rows cut; `null` uncut.
    omitted: Option<usize>,
}

/// A row's 13 keys, in this order.
#[derive(Serialize)]
struct RowJson<'a> {
    id: Option<&'a str>,
    kind: Option<&'a str>,
    title: Option<&'a str>,
    path: &'a str,
    line: usize,
    rev: Option<u32>,
    status: Option<&'a str>,
    archived: bool,
    state: Rollup,
    open_items: u64,
    tasks: usize,
    tasks_open: usize,
    last_change: Option<&'a LastChange>,
}

/// The same document as [`crate::render_json`]: every key present, absent =
/// `null`.
impl Serialize for CatalogOutcome {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let truncated = self.truncated();
        CatalogJson {
            kinds: &self.kinds,
            archive: self.archive,
            notes: &self.notes,
            rows: self
                .shown_rows()
                .iter()
                .map(|row| RowJson {
                    id: row.id.as_deref(),
                    kind: row.kind.as_deref(),
                    title: row.title.as_deref(),
                    path: &row.path,
                    line: row.line,
                    rev: row.rev,
                    status: row.status.as_deref(),
                    archived: row.archived,
                    state: row.state,
                    open_items: row.open_items,
                    tasks: row.tasks,
                    tasks_open: row.tasks_open,
                    last_change: row.last_change.as_ref(),
                })
                .collect(),
            truncated,
            omitted: truncated.then(|| self.rows.len() - self.shown),
        }
        .serialize(serializer)
    }
}

// ------------------------------------------------------------- the cache

/// Each file's last change per (git common dir, `HEAD` OID), at most
/// [`LAST_CHANGE_HEADS`] heads, the oldest dropped (DC-8): a path's newest
/// commit at one `HEAD` never changes, so a read walks only for the paths
/// its `HEAD` has not seen. A path a shallow clone's history did not reach
/// is never kept: a deepening fetch changes that answer under the same
/// `HEAD`. Paths are relative to the worktree's top, so every project and
/// worktree of one repository shares them. The daemon holds one; the CLI
/// passes none.
#[derive(Debug, Default)]
pub struct LastChangeCache {
    heads: Mutex<VecDeque<CachedHead>>,
}

/// One `HEAD`'s answers.
#[derive(Debug)]
struct CachedHead {
    common_dir: PathBuf,
    head: String,
    paths: BTreeMap<String, Known>,
}

impl LastChangeCache {
    /// The answers held for `wanted` at (`common_dir`, `head`); the paths
    /// held none for are the rest.
    fn take(
        &self,
        common_dir: &Path,
        head: &str,
        wanted: &BTreeSet<String>,
    ) -> BTreeMap<String, Known> {
        let heads = self.heads.lock().unwrap_or_else(PoisonError::into_inner);
        heads
            .iter()
            .find(|cached| cached.head == head && cached.common_dir == common_dir)
            .map(|cached| {
                wanted
                    .iter()
                    .filter_map(|path| Some((path.clone(), cached.paths.get(path)?.clone())))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Keeps `read` at (`common_dir`, `head`), but what a shallow clone's
    /// history did not reach; a new head drops the oldest past
    /// [`LAST_CHANGE_HEADS`].
    fn keep(&self, common_dir: &Path, head: &str, read: &BTreeMap<String, Known>) {
        let kept = read
            .iter()
            .filter(|(_, known)| **known != Known::Unreached(Reach::Shallow))
            .map(|(path, known)| (path.clone(), known.clone()));
        let mut heads = self.heads.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(cached) = heads
            .iter_mut()
            .find(|cached| cached.head == head && cached.common_dir == common_dir)
        {
            cached.paths.extend(kept);
            return;
        }
        heads.push_back(CachedHead {
            common_dir: common_dir.to_path_buf(),
            head: head.to_owned(),
            paths: kept.collect(),
        });
        while heads.len() > LAST_CHANGE_HEADS {
            heads.pop_front();
        }
    }
}

/// What one `HEAD` says of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Known {
    /// Its newest commit, read as a document row's change.
    Change(LastChange),
    /// `HEAD` does not hold it.
    Absent,
    /// The walk did not reach its change.
    Unreached(Reach),
}

/// Why a walk did not reach a file's change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// The walk stopped at [`LAST_CHANGE_SCAN_MAX`] with commits left.
    Cap,
    /// The history ended first: a shallow clone's.
    Shallow,
}

// ------------------------------------------------------------- the reads

/// `spec catalog`, cut at the output cap; no cache.
pub fn catalog(
    env: &Env,
    globals: &Globals,
    request: &CatalogRequest,
) -> Result<CatalogOutcome, CliError> {
    catalog_with_view(env, globals, request, View::Capped, None)
}

/// [`catalog`], its rows bounded by `view` (the daemon's [`View::Browser`]:
/// every row, `truncated` false, `omitted` `null`), the last changes taken
/// from `cache` where it holds them at this `HEAD` and kept there.
pub fn catalog_with_view(
    env: &Env,
    globals: &Globals,
    request: &CatalogRequest,
    view: View,
    cache: Option<&LastChangeCache>,
) -> Result<CatalogOutcome, CliError> {
    if request.kinds.is_empty() {
        return Err(CliError::cannot(NO_KIND));
    }
    let project = discover(env, globals)?;
    let slug = project.slug()?.to_owned();
    let mut messages = Vec::new();
    let mut open = open_index(env, &project)?;
    let input = indexed_in(&mut open.index, &project, &mut messages, true)?;
    let graph = SpecGraph::new(&input, &project.config.scheme, &project.config.paths);
    let lines = direction_lines(
        &graph,
        &project.config.paths.spec,
        &request.kinds,
        request.archive,
        &mut messages,
    );
    let nodes: Vec<NodeAt> = lines.iter().map(|&(at, _)| at).collect();

    seams::note(Seam::CatalogTrace);
    let read = queue_read(env, &slug)?;
    let stored = open.index.node_norms().map_err(store_error)?;
    let work = node_work(&graph, &read, &stored, &nodes, &mut messages);

    let paths: BTreeSet<String> = lines.iter().map(|(_, line)| line.path.clone()).collect();
    let git = if paths.is_empty() {
        GitRead::default()
    } else {
        git_read(&project, &process_git(env), &paths, cache)
    };
    let items = open_items(env, &slug, git.common_dir.as_deref())?;

    let rows: Vec<CatalogRow> = lines
        .into_iter()
        .zip(work)
        .map(|((at, line), work)| {
            let rev = graph
                .file(at.file)
                .and_then(|file| file.parsed.as_ref())
                .and_then(|parsed| revision(parsed, at.ord).number());
            row_of(line, at, rev, work, &items.counts, &git)
        })
        .collect();
    let shown = match view {
        View::Capped => fitting(request, &rows),
        View::Browser => rows.len(),
    };
    let mut notes: BTreeSet<String> = BTreeSet::new();
    let mut warnings: Vec<String> = Vec::new();
    for message in messages {
        match message {
            Message::Note(text) => {
                notes.insert(one_line(&text));
            }
            Message::Warning(text) => {
                let text = one_line(&text);
                if !warnings.contains(&text) {
                    warnings.push(text);
                }
            }
        }
    }
    if shown > 0 {
        notes.extend(git.notes(&rows[..shown]));
        notes.extend(items.notes);
    }
    Ok(CatalogOutcome {
        kinds: request.kinds.clone(),
        archive: request.archive,
        notes: notes.into_iter().collect(),
        warnings,
        rows,
        shown,
    })
}

/// A tree line joined with its node's work, open items and last change.
fn row_of(
    line: TreeNode,
    at: NodeAt,
    rev: Option<u32>,
    work: NodeWork,
    counts: &BTreeMap<String, u64>,
    git: &GitRead,
) -> CatalogRow {
    let open_items = line
        .id
        .as_ref()
        .and_then(|id| counts.get(id))
        .copied()
        .unwrap_or(0);
    let last_change = match git.known.get(&line.path) {
        Some(Known::Change(change)) => Some(LastChange {
            // A section's change is its file's (W1).
            of_file: at.ord != 0,
            ..change.clone()
        }),
        _ => None,
    };
    CatalogRow {
        id: line.id,
        kind: line.kind,
        title: line.title,
        path: line.path,
        line: line.line,
        rev,
        status: line.status,
        archived: line.archived,
        state: work.rollup,
        open_items,
        tasks: work.cards,
        tasks_open: work.open,
        last_change,
    }
}

/// The open items of this repository naming each node, read once.
#[derive(Default)]
struct Items {
    counts: BTreeMap<String, u64>,
    /// No queue yet; a row that does not read.
    notes: Vec<String>,
}

/// This repository's `open` proposals, staged ones included, counted per
/// node they name (DC-3): one list of the queue as it is, no schema step.
/// With no common dir read (git not read), every repository's count: an
/// item is never hidden for a failure of git. A queue that cannot be read
/// is the caller's exit 2.
fn open_items(env: &Env, slug: &str, common_dir: Option<&Path>) -> Result<Items, CliError> {
    let db = db_path(env, slug)?;
    let mut items = Items::default();
    let queue = match SqliteQueue::open_existing(&db, slug).map_err(queue_cannot)? {
        Some(queue) if queue.schema_version().map_err(queue_cannot)? > 0 => queue,
        _ => {
            items.notes.push(NO_QUEUE.to_owned());
            return Ok(items);
        }
    };
    seams::note(Seam::CatalogItems);
    let listed = queue
        .list_readable(&ProposalFilter {
            git_common_dir: None,
            statuses: vec![ProposalStatus::Open],
        })
        .map_err(queue_cannot)?;
    items.notes.extend(listed.unreadable.iter().map(|row| {
        one_line(&format!(
            "catalog: {}: its row does not read ({}: {}); not counted",
            row.id, row.column, row.reason
        ))
    }));
    let ours = |item: &&Proposal| {
        common_dir.is_none_or(|dir| same_repository(&item.place.git_common_dir, dir))
    };
    let named: Vec<OpenItem<'_>> = listed
        .proposals
        .iter()
        .filter(ours)
        .map(|item| OpenItem {
            target_id: Some(item.target_id.as_str()),
            node_ids: item
                .intake
                .as_ref()
                .map_or(&[][..], |intake| &intake.target_ids),
        })
        .collect();
    items.counts = open_items_by_node(&named);
    Ok(items)
}

// ------------------------------------------------------------------ git

/// What git said of the rows' files.
#[derive(Default)]
struct GitRead {
    /// The repository's canonical common dir; `None` when git was not read.
    common_dir: Option<PathBuf>,
    /// By root-relative path.
    known: BTreeMap<String, Known>,
    /// Why no last change was read at all.
    unread: Option<String>,
}

impl GitRead {
    /// Git not read, for `cause`.
    fn unread(cause: String) -> Self {
        Self {
            unread: Some(cause),
            ..Self::default()
        }
    }

    /// The notes of what hid a shown row's last change, the files counted
    /// once each.
    fn notes(&self, rows: &[CatalogRow]) -> Vec<String> {
        if let Some(cause) = &self.unread {
            return vec![one_line(&format!("catalog: last change not read: {cause}"))];
        }
        let files = |wanted: Reach| {
            rows.iter()
                .filter(|row| self.known.get(&row.path) == Some(&Known::Unreached(wanted)))
                .map(|row| row.path.as_str())
                .collect::<BTreeSet<&str>>()
                .len()
        };
        let mut notes = Vec::new();
        let shallow = files(Reach::Shallow);
        if shallow > 0 {
            notes.push(format!(
                "catalog: a shallow clone: {shallow} files its history does not reach read no \
                 last change"
            ));
        }
        let capped = files(Reach::Cap);
        if capped > 0 {
            notes.push(format!(
                "catalog: {capped} files not reached in the last {LAST_CHANGE_SCAN_MAX} commits \
                 read no last change"
            ));
        }
        notes
    }
}

/// The last change of each of `paths` (root-relative) at `HEAD`: the
/// repository and `HEAD` read by `rev-parse`, the paths `cache` holds at
/// that `HEAD` taken, one walk for the rest, kept there. A failure is the
/// note's cause and no last change; `HEAD` unborn holds no file.
fn git_read(
    project: &ProjectRoot,
    env: &GitEnv,
    paths: &BTreeSet<String>,
    cache: Option<&LastChangeCache>,
) -> GitRead {
    let cause = |error| git_cause(&error);
    let git = match WorktreeGit::new(&project.root, env) {
        Ok(git) => git,
        Err(error) => return GitRead::unread(cause(error)),
    };
    let top = match git.top_if_repository() {
        Ok(Some(top)) => top,
        Ok(None) => return GitRead::unread("not a repository".to_owned()),
        Err(error) => return GitRead::unread(cause(error)),
    };
    let common_dir = match git.common_dir() {
        Ok(dir) => dir,
        Err(error) => return GitRead::unread(cause(error)),
    };
    let head = match git.head() {
        Ok(head) => head,
        Err(error) => return GitRead::unread(cause(error)),
    };
    let mut read = GitRead {
        common_dir: Some(common_dir.clone()),
        ..GitRead::default()
    };
    let Some(head) = head else {
        read.known = paths
            .iter()
            .map(|path| (path.clone(), Known::Absent))
            .collect();
        return read;
    };
    let Some(prefix) = top_prefix(&project.root, &top) else {
        // Fixed words: no absolute path reaches the JSON.
        read.unread = Some(ROOT_OUTSIDE.to_owned());
        return read;
    };
    let by_top: BTreeMap<String, &String> = paths
        .iter()
        .map(|path| (format!("{prefix}{path}"), path))
        .collect();
    let wanted: BTreeSet<String> = by_top.keys().cloned().collect();
    let mut known = cache.map_or_else(BTreeMap::new, |cache| {
        cache.take(&common_dir, &head, &wanted)
    });
    let missing: BTreeSet<String> = wanted
        .iter()
        .filter(|path| !known.contains_key(*path))
        .cloned()
        .collect();
    if !missing.is_empty() {
        let listed: Vec<String> = missing.iter().cloned().collect();
        seams::note(Seam::CatalogWalk(&listed));
        let walked = match git.last_commits_at(&head, &missing, LAST_CHANGE_SCAN_MAX) {
            Ok(walked) => walked,
            Err(error) => {
                read.unread = Some(cause(error));
                return read;
            }
        };
        let fresh = known_of(walked);
        if let Some(cache) = cache {
            cache.keep(&common_dir, &head, &fresh);
        }
        known.extend(fresh);
    }
    read.known = known
        .into_iter()
        .filter_map(|(top_path, answer)| Some(((*by_top.get(&top_path)?).clone(), answer)))
        .collect();
    read
}

/// The store's answer, per top-relative path: a path not reached was cut
/// by the cap when the walk stopped at it with commits left, else by the
/// end of a shallow clone's history (the one other end short of a path).
fn known_of(walked: LastCommits) -> BTreeMap<String, Known> {
    let reach = if walked.capped {
        Reach::Cap
    } else {
        Reach::Shallow
    };
    let mut known: BTreeMap<String, Known> = walked
        .found
        .iter()
        .map(|(path, commit)| (path.clone(), Known::Change(change_of(commit))))
        .collect();
    known.extend(walked.absent.into_iter().map(|path| (path, Known::Absent)));
    known.extend(
        walked
            .unreached
            .into_iter()
            .map(|path| (path, Known::Unreached(reach))),
    );
    known
}

/// A logged commit as a document row's last change.
fn change_of(commit: &LoggedCommit) -> LastChange {
    last_change(
        &CommitFacts {
            oid: &commit.oid,
            author_name: &commit.author_name,
            author_email: &commit.author_email,
            committed_at: commit.committed_at,
            message: &commit.message,
        },
        false,
    )
}

/// The root's place under the worktree's top as a path prefix (`""` at
/// the top, else `<dir>/`); `None` when the root is not under it or a part
/// is not UTF-8. Both are canonical.
fn top_prefix(root: &Path, top: &Path) -> Option<String> {
    let inner = root.strip_prefix(top).ok()?;
    let mut prefix = String::new();
    for component in inner.components() {
        match component {
            Component::Normal(part) => {
                prefix.push_str(part.to_str()?);
                prefix.push('/');
            }
            _ => return None,
        }
    }
    Some(prefix)
}

// -------------------------------------------------------------- the text

/// The header line, no line end.
fn header(request_kinds: &[String], archive: bool, rows: usize) -> String {
    let kinds: Vec<String> = request_kinds.iter().map(|kind| one_line(kind)).collect();
    let mut line = format!("catalog {} | rows {rows}", kinds.join(", "));
    if archive {
        line.push_str(" [archive]");
    }
    escape_controls(&line)
}

/// A row's line, no line end, escaped.
fn row_line(row: &CatalogRow) -> String {
    let held =
        |text: Option<&String>| text.map_or_else(|| ABSENT.to_owned(), |text| one_line(text));
    let name = row.id.as_ref().unwrap_or(&row.path);
    let rev = row
        .rev
        .map_or_else(|| ABSENT.to_owned(), |rev| rev.to_string());
    let (at, by, proposal) = match &row.last_change {
        Some(change) => (
            if change.of_file {
                format!("{} (file)", one_line(&change.at))
            } else {
                one_line(&change.at)
            },
            one_line(&change.by),
            change
                .proposal
                .as_deref()
                .map_or_else(|| HAND.to_owned(), one_line),
        ),
        None => (ABSENT.to_owned(), ABSENT.to_owned(), ABSENT.to_owned()),
    };
    escape_controls(&format!(
        "{} | version {rev} | {} | open items {} | tasks {}, open {} | {at} | {by} | {proposal} | {}",
        one_line(name),
        row.state.as_str(),
        row.open_items,
        row.tasks,
        row.tasks_open,
        held(row.title.as_ref())
    ))
}

/// How many rows print within [`crate::OUTPUT_CAP_CHARS`] after the header, each
/// whole, the first always.
fn fitting(request: &CatalogRequest, rows: &[CatalogRow]) -> usize {
    let head = header(&request.kinds, request.archive, rows.len())
        .chars()
        .count()
        + 1;
    let costs = rows.iter().map(|row| row_line(row).chars().count() + 1);
    lines_within(costs, head, true).0
}

/// The header, the rows that print, then the tail when one does not.
pub(crate) fn render_text(outcome: &CatalogOutcome) -> String {
    let mut out = header(&outcome.kinds, outcome.archive, outcome.rows.len());
    out.push('\n');
    for row in outcome.shown_rows() {
        out.push_str(&row_line(row));
        out.push('\n');
    }
    if outcome.truncated() {
        out.push_str(&format!(
            "[truncated: {} of {} rows not shown]\n",
            outcome.rows.len() - outcome.shown,
            outcome.rows.len()
        ));
    }
    out
}
