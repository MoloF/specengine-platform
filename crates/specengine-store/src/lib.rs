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
//!   ([`specengine_core::Paths`]);
//! - [`SqliteIndex::open`] binds a handle to one worktree `(project,
//!   canonical root)` of a DB outside it; a DB holds several worktrees;
//! - [`IndexWriter`]: [`IndexWriter::update`] (full walk),
//!   [`IndexWriter::update_paths`] (named paths, for the Phase 2 watcher),
//!   [`IndexWriter::rebuild`] (`spec index --full`), each giving an
//!   [`UpdateReport`];
//! - [`SpecIndex`]: [`SpecIndex::files`], [`SpecIndex::file`],
//!   [`SpecIndex::lookup_id`], [`SpecIndex::search`];
//! - [`check_input`], [`check_worktree`]: `spec check` over a fresh parse
//!   of a [`Source`], no database (docs/features/spec-check.md).
//!
//! No `rusqlite` type appears in a public signature
//! (`docs/canon/architecture.md#distribution`); the Phase 2 daemon can take
//! the traits over as the sole writer (05 §1 principle 6). Until then any
//! number of handles, in any processes, may write: WAL, `Immediate` write
//! transactions and `busy_timeout`, no lock file and no global state.

mod check;
mod dump;
mod error;
mod glob;
mod index;
mod read;
mod rows;
mod schema;
mod search;
mod source;
mod write;

use serde::Serialize;
use specengine_model::{IdScheme, Node, ParsedFile};

pub use check::{BASELINE_FILE, check_input, check_worktree, today_utc};
pub use error::StoreError;
pub use index::{DbSettings, SqliteIndex};
pub use source::{Listing, Source, WorkingTree};

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
pub const INDEX_FORMAT: u32 = 3;

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
    /// Directories below a root that could not be listed (their files are
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
}

impl SearchQuery {
    /// `text` over any kind, up to [`SEARCH_LIMIT_DEFAULT`] hits.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kinds: Vec::new(),
            limit: SEARCH_LIMIT_DEFAULT,
        }
    }
}

/// One node found by [`SpecIndex::search`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchHit {
    pub path: String,
    pub ord: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The best-matching column's text around the match, matches in `**`,
    /// cuts marked `…`.
    pub snippet: String,
}

/// The hits of a search, best first: bm25 (weights `id` 10, `title` 5,
/// text 1), then path, then position; never by storage order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SearchResults {
    pub hits: Vec<SearchHit>,
    /// No term of at least [`MIN_TERM_CHARS`] characters was left, so
    /// nothing was searched.
    pub short_query: bool,
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
