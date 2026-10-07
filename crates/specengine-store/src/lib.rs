//! The spec index of SpecEngine (docs/features/spec-index.md): a
//! rebuildable SQLite + FTS5 projection of `specengine_core::parse` over one
//! worktree, updated incrementally, with one hard property: **after any
//! sequence of edits the incremental index equals a fresh rebuild, row for
//! row.**
//!
//! The index is derived data (ADR-0001, ADR-0003): it lives outside the
//! repository, is keyed by exactly the inputs of `parse` (path, BLAKE3 of the
//! bytes, the `[ids]` fingerprint, the [`INDEX_FORMAT`] stamp) and resolves
//! nothing, so ID-less documents, duplicate and scoped IDs and broken files
//! are stored as parsed. A broken file never fails an update (ADR-0012).
//!
//! - [`Source`] lists, probes and reads a worktree's spec files;
//!   [`WorkingTree`] walks the working tree by `[paths]`
//!   ([`specengine_core::Paths`]), [`GitIndex`] the git index by the same
//!   rules (checks only), its git run with an explicit [`GitEnv`];
//! - [`SqliteIndex::open`] binds a handle to one worktree `(project,
//!   canonical root)` of a DB outside it; a DB holds several worktrees;
//! - [`IndexWriter`]: [`IndexWriter::update`] (full walk),
//!   [`IndexWriter::update_paths`] (named paths, for the Phase 2 watcher),
//!   [`IndexWriter::rebuild`] (`spec index --full`), each giving an
//!   [`UpdateReport`];
//! - [`SpecIndex`]: [`SpecIndex::files`], [`SpecIndex::file`],
//!   [`SpecIndex::lookup_id`], [`SpecIndex::search`] (Tier 3 files left out
//!   unless [`SearchQuery::archive`]; each hit's snippet as text and as its
//!   structure, [`Snippet`]), [`SpecIndex::indexed_input`] (the
//!   index-fed [`CheckInput`] of `spec show`);
//! - [`load_config`], [`load_check`]: the config and the baseline from
//!   their bytes ([`NamedBytes`]), validated into a [`CheckSetup`] or a
//!   `cannot-check` report; [`check_input`], [`check_source`],
//!   [`check_tree`], [`check_worktree`]: `spec check` over a fresh parse of
//!   a [`Source`], no database (docs/features/spec-check.md);
//!   [`check_staged`]: over the git index, config and baseline from it
//!   (docs/features/spec-cli-staged.md), judged against `HEAD`, its base;
//!   [`check_staged_with_notes`] takes the given files' places
//!   ([`GivenFile`]) and returns the notes too ([`StagedCheck`])
//!   (docs/features/spec-cli-introduced.md); [`check_changed_with_notes`]:
//!   over the working tree, config and baseline from disk, judged against
//!   `HEAD` the same way (docs/features/spec-cli-changed.md);
//! - [`b3_hash`]: the `b3:` content address of a byte string, the BLAKE3
//!   of the index's cache key in the `spec.lock` form (05 §3.5), and
//!   `spec bundle`'s `bundle_hash` (docs/features/spec-cli-bundle.md).
//!
//! - the proposal queue (docs/features/proposal-apply.md): [`SqliteQueue`]
//!   ([`ProposalQueue`]) keeps the operational tables `proposals` and
//!   `events` in the same DB, made by their own schema steps on
//!   `user_version` and never dropped by the index, and gives them raw for
//!   their backup ([`StoredQueue`], `docs/canon/queue-backup.md` "Store");
//!   a live tail reads a project's events after a `seq` in one short read
//!   transaction ([`SqliteQueue::events_after`], the daemon's);
//!   [`WorktreeGit`] is the
//!   write side of one recorded worktree's git (its place, the dirty check,
//!   `merge-file`, `commit --only`, the trailer lookup), every `GIT_*`
//!   local variable dropped; [`replace_file`] the atomic write;
//!   [`update_file`], [`span_hash`], [`introduced_findings`] the update of
//!   one span over core's pure half; a question's or a discrepancy's
//!   decision record (`docs/canon/decision-record.md` "ID", "Steps"): its ID
//!   issued under the queue's write lock ([`RecordSeries`],
//!   [`RecordApproval`]), the new file [`create_file`] makes without ever
//!   replacing one (a create's new file too); a create's new IDs held while
//!   it is live ([`Reservation`], [`QueueError::Reserved`]).
//!
//! No `rusqlite` type appears in a public signature
//! (`docs/canon/architecture.md#distribution`); the Phase 2 daemon can take
//! the traits over as the sole writer (05 §1 principle 6). Until then any
//! number of handles, in any processes, may write: WAL, `Immediate` write
//! transactions and `busy_timeout`, no lock file and no global state.

mod base;
mod check;
mod dump;
mod error;
mod git;
mod index;
mod queue;
mod read;
mod rows;
mod schema;
mod search;
mod source;
mod update;
mod worktree;
mod write;

use serde::Serialize;
use specengine_core::check::CheckInput;
use specengine_model::{IdScheme, Node, ParsedFile};

pub use check::{
    BASELINE_FILE, CONFIG_FILE, CheckSetup, GivenFile, NamedBytes, StagedCheck,
    check_changed_with_notes, check_input, check_source, check_staged, check_staged_with_notes,
    check_tree, check_worktree, default_baseline, load_check, load_config, today_utc,
};
pub use error::StoreError;
pub use git::GitEnv;
pub use index::{DbSettings, SqliteIndex};
pub use queue::{
    APPLY_VERIFY_STEP, ApplyFailure, Choice, Decision, DecisionRecord, EVENT_APPLIED,
    EVENT_APPLY_FAILED, EVENT_APPROVED, EVENT_COLUMNS, EVENT_CREATED, EVENT_REJECTED,
    EVENT_TASK_APPROVED, EVENT_TASK_CANCELLED, EVENT_TASK_CHANGES_REQUESTED, EVENT_TASK_CLAIMED,
    EVENT_TASK_COMPLETED, EVENT_TASK_CREATED, EVENT_TASK_PLANNED, EVENT_TASK_REFRESHED,
    EVENT_TASK_RUN_REPORTED, Event, EventsAfter, Intake, IntakeResult, NewIntake, NewProposal,
    NewTask, PROPOSAL_COLUMNS, Place, Proposal, ProposalFilter, ProposalFinding, ProposalKind,
    ProposalList, ProposalQueue, ProposalStatus, QUEUE_SCHEMA_VERSION, QueueCounts, QueueError,
    QueueMatch, RUN_COLUMNS, RecordApproval, RecordSeries, RefreshEntry, Reservation, Restore, Run,
    Seen, SnapshotEntry, SqliteQueue, StoredEvent, StoredNote, StoredProposal, StoredQueue,
    StoredRun, StoredTask, TASK_COLUMNS, TailEvent, Task, TaskChange, TaskList, TaskRefresh,
    TaskSeen, TaskSnapshot, UnreadableRow, UnreadableTask, binding_problem, claimed_elsewhere,
    patch_hash, proposal_columns, task_event,
};
pub use source::{GitIndex, Listing, Source, WorkingTree};
pub use update::{UpdateError, introduced_findings, span_hash, update_file};
pub use worktree::{
    ChangedPath, CreateFileError, CreatedFile, GitError, ListedWorktree, Merge, Operation,
    PlaceError, WorktreeGit, create_file, replace_file, same_repository,
};

/// The format stamp stored in `index_meta` (`('format', '2')`). Any change
/// of what a fresh index stores for the same corpus — schema, row
/// projection, JSON of a model value — needs a new number, and a new line
/// `<INDEX_FORMAT> <dump hash>` in `tests/format_history.txt`. A DB with
/// another stamp is recreated by its first write; until then its worktrees
/// read [`StoreError::NotIndexed`].
///
/// 2: anchors carry an origin (`slug`, `attr`, `html`) and a span, and every
/// heading slug and HTML anchor is stored (docs/features/spec-check.md).
///
/// 3: alias matches emit no `homoglyph`; non-finite floats are strings; map
/// keys are unique; `<!-->`, `<!--->` close a comment.
///
/// 4: the fixtures define feature-scoped criteria as `{#ID}` sections of
/// their feature documents (ADR-0026); the store code is unchanged
/// (docs/features/spec-check-scopes.md).
///
/// 5: local Markdown link destinations and reference definitions are
/// `mentions` links with a path target (`dst_path`), and the fixtures gained
/// a `link_base` link; no schema change (docs/features/spec-check-links.md).
///
/// 6: `files.tier3` (core's `check::is_tier3_file`, the archive filter of
/// [`SearchQuery::archive`]) and `nodes.line` (the 1-based line of the
/// node's span start), both pure functions of the file's bytes
/// (docs/features/spec-cli.md).
///
/// 7: the token weights of core's `tokens_est` were recalibrated, so
/// `nodes.tokens_est` changes; no schema change
/// (docs/features/token-calibration.md).
pub const INDEX_FORMAT: u32 = 7;

/// `b3:` and the lower-case hex BLAKE3 of `bytes` (64 digits): the
/// `spec.lock` form of a content hash (05 §3.5). Nothing is stored.
pub fn b3_hash(bytes: &[u8]) -> String {
    format!("b3:{}", rows::hash_bytes(bytes))
}

/// Smallest and largest [`SearchQuery::limit`]; a limit outside is clamped.
pub const SEARCH_LIMIT_MIN: usize = 1;
pub const SEARCH_LIMIT_MAX: usize = 200;
/// The limit of [`SearchQuery::new`].
pub const SEARCH_LIMIT_DEFAULT: usize = 20;
/// Terms shorter than this many characters are dropped from a search (the
/// trigram tokenizer cannot match them).
pub const MIN_TERM_CHARS: usize = 3;

/// What an update did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpdateReport {
    /// Files listed by the walk (`update_paths`: named paths that are listed).
    pub walked: usize,
    /// Files whose rows were written from a fresh parse.
    pub parsed: usize,
    /// Files whose stored rows were kept: same path, same BLAKE3.
    pub unchanged: usize,
    /// Stored files no longer listed, whose rows were deleted.
    pub removed: usize,
    /// Files stored with no hash and a `read_error` (unreadable, or the
    /// parser panicked on them); retried by every update.
    pub unreadable: usize,
    /// Every file was re-parsed: the format stamp or the `[ids]`
    /// fingerprint changed, or this was a rebuild.
    pub reparsed_all: bool,
    /// Configured roots that name no directory and no `.md` file.
    pub missing_roots: Vec<String>,
    /// Directory and `.md` names skipped because they are not UTF-8.
    pub skipped_names: usize,
    /// Directories below a root, or on the way to one (`""`: the root
    /// itself), that could not be listed, root-relative (their files are
    /// treated as absent).
    pub unreadable_dirs: Vec<String>,
}

/// One stored file.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedFile {
    /// The parse, rebuilt from the rows; `None` when the bytes were not read.
    pub parsed: Option<ParsedFile>,
    /// BLAKE3 of the bytes, lower-case hex; `None` when they were not read.
    pub blake3: Option<String>,
    /// Bytes; 0 when they were not read.
    pub size: u64,
    /// Why the bytes were not read (or not parsed).
    pub read_error: Option<String>,
}

/// A node whose `id` is exactly the one asked for.
#[derive(Debug, Clone, PartialEq)]
pub struct IdHit {
    /// Root-relative path of the file.
    pub path: String,
    /// Position in the file's `nodes` (0: the document).
    pub ord: usize,
    pub node: Node,
}

/// A full-text query over the handle's worktree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Whitespace-separated terms, ANDed; each is matched as a substring,
    /// case-folded; FTS5 syntax is never interpreted.
    pub text: String,
    /// Node kinds to keep; empty: any.
    pub kinds: Vec<String>,
    /// Hits at most, clamped to [`SEARCH_LIMIT_MIN`]..=[`SEARCH_LIMIT_MAX`].
    pub limit: usize,
    /// Keep the nodes of Tier 3 files (`files.tier3`); `false` drops them in
    /// the query, before the limit, and counts them in
    /// [`SearchResults::tier3_left_out`].
    pub archive: bool,
}

impl SearchQuery {
    /// `text` over any kind, up to [`SEARCH_LIMIT_DEFAULT`] hits, Tier 3
    /// files left out.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kinds: Vec::new(),
            limit: SEARCH_LIMIT_DEFAULT,
            archive: false,
        }
    }
}

/// One node found by [`SpecIndex::search`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub path: String,
    pub ord: usize,
    /// The 1-based line of the node's span start.
    pub line: usize,
    /// The node's file is Tier 3 (`files.tier3`).
    pub tier3: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The best-matching column's text around the match, matches in `**`,
    /// cuts marked `…`: [`Self::snippet_parts`] rendered
    /// ([`Snippet::marked`]), as FTS5 marked it before the structure.
    pub snippet: String,
    /// The same snippet as structure: a corpus `**` is text, never a hit.
    #[serde(skip)]
    pub snippet_parts: Snippet,
}

/// A hit's snippet as structure (task spec `daemon-read`, "Data"): the
/// text in order, each run marked as a match or not, and whether the text
/// is cut before or after it. Read from the one FTS5 `snippet()`, which
/// marks a match's start [`SNIPPET_HIT_START`], its end
/// [`SNIPPET_HIT_END`] and a cut [`SNIPPET_CUT`] (a corpus holding these
/// control characters may fake a match or a cut, a known limit).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Snippet {
    pub segments: Vec<SnippetSegment>,
    pub cut_start: bool,
    pub cut_end: bool,
}

/// A run of a [`Snippet`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnippetSegment {
    pub text: String,
    /// A matched term.
    pub hit: bool,
}

/// FTS5 `snippet()`'s mark before a match (U+0002).
pub const SNIPPET_HIT_START: char = '\u{2}';
/// FTS5 `snippet()`'s mark after a match (U+0003).
pub const SNIPPET_HIT_END: char = '\u{3}';
/// FTS5 `snippet()`'s mark of a cut, at the text's start or end (U+0004).
pub const SNIPPET_CUT: char = '\u{4}';

impl Snippet {
    /// The structure of `snippet()`'s text: a leading and a trailing
    /// [`SNIPPET_CUT`] are the cuts; [`SNIPPET_HIT_START`] opens a match
    /// and [`SNIPPET_HIT_END`] closes it; any other of these characters,
    /// and a match never closed, are text as written.
    pub fn from_marks(marked: &str) -> Self {
        let mut body = marked;
        let cut_start = body.starts_with(SNIPPET_CUT);
        if cut_start {
            body = &body[SNIPPET_CUT.len_utf8()..];
        }
        let cut_end = body.ends_with(SNIPPET_CUT);
        if cut_end {
            body = &body[..body.len() - SNIPPET_CUT.len_utf8()];
        }
        let mut snippet = Self {
            segments: Vec::new(),
            cut_start,
            cut_end,
        };
        let mut text = String::new();
        let mut in_hit = false;
        for c in body.chars() {
            if c == SNIPPET_HIT_START && !in_hit {
                snippet.push(std::mem::take(&mut text), false);
                in_hit = true;
            } else if c == SNIPPET_HIT_END && in_hit {
                snippet.push(std::mem::take(&mut text), true);
                in_hit = false;
            } else {
                text.push(c);
            }
        }
        if in_hit {
            // Opened, never closed: the mark was the corpus's own.
            text.insert(0, SNIPPET_HIT_START);
        }
        snippet.push(text, false);
        snippet
    }

    /// The snippet as one text: a cut as `…`, a match between `**`.
    pub fn marked(&self) -> String {
        let mut out = String::new();
        if self.cut_start {
            out.push('…');
        }
        for segment in &self.segments {
            if segment.hit {
                out.push_str("**");
                out.push_str(&segment.text);
                out.push_str("**");
            } else {
                out.push_str(&segment.text);
            }
        }
        if self.cut_end {
            out.push('…');
        }
        out
    }

    /// No text and no cut.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty() && !self.cut_start && !self.cut_end
    }

    /// Appends a run: a match always, other text when not empty, joined to
    /// the run before it when that is other text too.
    fn push(&mut self, text: String, hit: bool) {
        if !hit && text.is_empty() {
            return;
        }
        if !hit
            && let Some(last) = self.segments.last_mut()
            && !last.hit
        {
            last.text.push_str(&text);
            return;
        }
        self.segments.push(SnippetSegment { text, hit });
    }
}

/// The hits of a search, best first: bm25 (weights `id` 10, `title` 5,
/// text 1), then path, then position; never by storage order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// No term of at least [`MIN_TERM_CHARS`] characters was left, so
    /// nothing was searched.
    pub short_query: bool,
    /// Matches of Tier 3 files the query dropped (all of them, not only
    /// those within the limit); 0 with [`SearchQuery::archive`].
    pub tier3_left_out: u32,
}

/// Reading the index of the handle's worktree. Every call reads one
/// consistent snapshot; a worktree without rows of the current format gives
/// [`StoreError::NotIndexed`].
pub trait SpecIndex {
    /// Every stored path, byte-sorted.
    fn files(&self) -> Result<Vec<String>, StoreError>;
    /// One stored file; `Ok(None)` when the path is not stored.
    fn file(&self, path: &str) -> Result<Option<IndexedFile>, StoreError>;
    /// Every node whose `id` is exactly `id`, by `(path, ord)`; repeated and
    /// feature-scoped IDs give several.
    fn lookup_id(&self, id: &str) -> Result<Vec<IdHit>, StoreError>;
    /// Full-text search (trigram substrings; exact IDs: [`Self::lookup_id`]).
    fn search(&self, query: &SearchQuery) -> Result<SearchResults, StoreError>;
    /// The stored worktree as the check's input, from one snapshot: every
    /// file in path order with its stored `size`, parse and `read_error`;
    /// `bytes` empty, no walk `problems` (the index keeps none).
    fn indexed_input(&self) -> Result<CheckInput, StoreError>;
}

/// Writing the index of the handle's worktree. Parsing runs before the
/// write transaction; one `Immediate` transaction re-checks the stored
/// hashes and applies. `source` must walk the handle's worktree
/// ([`StoreError::RootMismatch`] otherwise).
pub trait IndexWriter {
    /// Walks everything: re-parses the files whose `(path, BLAKE3)` changed
    /// (all of them after a stamp or fingerprint change), deletes the rows of
    /// files no longer listed.
    fn update(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
    ) -> Result<UpdateReport, StoreError>;
    /// Probes each named path by the walk rules: listed → as `update`;
    /// unlisted → its rows (and those of stored files under it, for a
    /// directory gone from disk) are deleted. Escalates to [`Self::update`]
    /// when the worktree is not indexed yet, the stamp or the fingerprint
    /// changed, or a named path is not clean root-relative (`""`, `docs/`,
    /// `./x.md`, absolute) or is a directory by [`Source::is_dir`] (new or
    /// stored: a rename). A `[paths]` change needs [`Self::update`]: the
    /// named paths alone cannot show what the new walk admits or drops.
    fn update_paths(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
        paths: &[&str],
    ) -> Result<UpdateReport, StoreError>;
    /// Re-parses everything and replaces the worktree's rows in one
    /// transaction; never seen empty.
    fn rebuild(
        &mut self,
        source: &dyn Source,
        scheme: &IdScheme,
    ) -> Result<UpdateReport, StoreError>;
}
