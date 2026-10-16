//! The code index (`docs/canon/symbol-index.md` "Tables",
//! "Cache key and cross-file pass"): per-file rows a [`CodeAnalyzer`]
//! makes, stored beside the spec index of the same worktree, row for row
//! equal to a fresh rebuild. The store compares, applies and runs the
//! cross-file pass; what a row says about the code is the analyzer's (the
//! store knows no language: it stores strings and compares them).
//!
//! - **Cache key**: a file is re-derived when its BLAKE3, its package
//!   directory or that package's `targets_fp` (BLAKE3 of its target table)
//!   changed, or the worktree's `code_fp` did: BLAKE3 of the analyzer's
//!   [`CodeAnalyzer::fingerprint`] and the `[ids]` fingerprint of the spec
//!   writer, re-checked inside the write transaction. Never mtime.
//! - **Pass**: in the same `Immediate` transaction, `declared_by` of every
//!   path a re-derived or removed file declared or declares (and of every
//!   re-derived file), `twin_files` of every undiscriminated qpath such a
//!   file rendered or renders, read through the `qpath` index
//!   ([`TWIN_SQL`]), never by splitting a name.
//! - **Reads** compute an item's ambiguity from stored facts, the first
//!   that applies: `path_attribute` (a file declares it and its role is not
//!   [`ROLE_SHARED`]), `unrooted` (its role is [`ROLE_UNROOTED`]),
//!   `duplicate` (`twin_files` > 0, items only).

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{OptionalExtension, Statement, Transaction, TransactionBehavior};
use serde::Serialize;
use specengine_model::IdScheme;

use crate::error::{Db, StoreError};
use crate::index::SqliteIndex;
use crate::read::{with_worktree, worktree_id};
use crate::rows::hash_bytes;
use crate::symbols::{SymbolHit, file_symbols_in, lookup_in};
use crate::{b3_hash, schema, write};

/// The columns [`stored_symbol`] reads, `s` a `symbols` row and `f` its
/// `code_files` row: a literal, for `concat!`.
macro_rules! symbol_columns {
    () => {
        "f.path, s.ord, s.item_kind, s.name, s.owner, s.qpath, s.discriminator,
         s.parent_ord, s.start_line, s.end_line, s.start_byte, s.end_byte, s.signature,
         s.level_state, s.layer, s.twin_files, f.declared_by IS NOT NULL, f.role"
    };
}
pub(crate) use symbol_columns;

/// A file's role whose unit several crates may include: named by its
/// location, never `path_attribute`.
pub const ROLE_SHARED: &str = "shared";
/// A file's role outside every unit: its module path is unknown.
pub const ROLE_UNROOTED: &str = "unrooted";

/// The ambiguity of a file declared by a `#[path]` attribute.
pub const AMBIGUITY_PATH_ATTRIBUTE: &str = "path_attribute";
/// The ambiguity of a file in no unit.
pub const AMBIGUITY_UNROOTED: &str = "unrooted";
/// The ambiguity of an item another file renders too.
pub const AMBIGUITY_DUPLICATE: &str = "duplicate";

/// What joins an undiscriminated qpath and a twin's discriminator.
const DISCRIMINATOR_SEPARATOR: &str = " #";
/// The byte after `#`: `[u #, u $)` is every qpath starting with `u #`.
const SEPARATOR_END: &str = " $";

/// The statements the pass runs to find every symbol of the worktree
/// rendering one undiscriminated qpath `u`: one without a discriminator
/// whose qpath is `u` (`?1` `u`, `?2` the worktree), and twins, whose
/// qpath is `u` ‖ ` #` ‖ their discriminator, read as the range
/// [`u #`, `u $`) of the `qpath` index (`?1`, `?2`) filtered by the
/// discriminator column (`?3` `u`, `?4` the worktree): a file or directory
/// name may hold ` #`, so a qpath is never split. `CROSS JOIN` keeps the
/// index search outermost.
pub(crate) const TWIN_SQL: &[&str] = &[
    "SELECT s.file, s.ord, s.twin_files
     FROM symbols s CROSS JOIN code_files f ON f.file_id = s.file
     WHERE s.qpath = ?1 AND s.discriminator IS NULL AND f.wt = ?2",
    "SELECT s.file, s.ord, s.twin_files
     FROM symbols s CROSS JOIN code_files f ON f.file_id = s.file
     WHERE s.qpath >= ?1 AND s.qpath < ?2 AND s.discriminator IS NOT NULL
       AND s.qpath = ?3 || ' #' || s.discriminator AND f.wt = ?4",
];

/// The declaring files of one path: the smallest path of the worktree's
/// files holding a declaration of it (`?1` the path, `?2` the worktree).
const DECLARED_BY_SQL: &str = "SELECT min(f.path)
     FROM code_decls d CROSS JOIN code_files f ON f.file_id = d.file
     WHERE d.target = ?1 AND f.wt = ?2";

/// The code tree a refresh indexes, as its analyzer gives it: listing,
/// packages, per-file rows. Plain rows only: the store depends on no code
/// library (`docs/canon/symbol-index.md` "Crates and seams").
pub trait CodeAnalyzer {
    /// The files of the code walk, read: each listed file once, by path.
    fn list(&self) -> Result<CodeListing, StoreError>;
    /// The packages the listed files belong to, each with its target table.
    fn packages(&self, listing: &CodeListing) -> Result<Vec<CodePackageRow>, StoreError>;
    /// The analyzer's stamp: every rule and setting a per-file row depends
    /// on but the bytes, the path, the package table and the `[ids]`
    /// scheme (which the store adds).
    fn fingerprint(&self) -> Vec<u8>;
    /// The rows of one listed file, `package` its package's row (none: the
    /// file is in no package). A pure function of the file's bytes, path,
    /// package table, the fingerprint and the scheme; never an error (a
    /// file that cannot be read or parsed is a row with a `read_error`).
    fn analyze(&self, file: &ListedCodeFile, package: Option<&CodePackageRow>) -> CodeFileRows;
}

/// What the code walk listed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeListing {
    /// Root-relative `/` paths, unique.
    pub files: Vec<ListedCodeFile>,
    /// Directories that could not be listed (`""`: the root), root-relative:
    /// a gap of the walk, passed to [`CodeReport::gaps`].
    pub gaps: Vec<String>,
    /// Names skipped for not being UTF-8.
    pub skipped: usize,
}

/// One listed file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListedCodeFile {
    pub path: String,
    /// The bytes; `None` when they could not be read.
    pub bytes: Option<Vec<u8>>,
    /// Why the bytes could not be read.
    pub read_error: Option<String>,
    /// The directory of its package (relative to the root, `""` the root);
    /// `None` outside every package. Part of the cache key.
    pub package: Option<String>,
}

/// One package of the code tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodePackageRow {
    /// Relative to the root, `""` the root.
    pub dir: String,
    /// `b3:` and the BLAKE3 of its manifest; `None` when unread.
    pub manifest_b3: Option<String>,
    /// Where the target table came from.
    pub source: String,
    /// The target table as JSON: its BLAKE3 with `source` is the package's
    /// `targets_fp`.
    pub targets: String,
}

/// The rows of one file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeFileRows {
    pub lang: String,
    /// The bytes are UTF-8 (`false`: stored, nothing read from them).
    pub utf8: bool,
    /// Why the file could not be read or analysed (no rows below then).
    pub read_error: Option<String>,
    /// What the file is in its package ([`ROLE_SHARED`], [`ROLE_UNROOTED`],
    /// or another role); `None` when the language has no roles.
    pub role: Option<String>,
    /// The file's module qpath.
    pub module: Option<String>,
    pub orphan_errors: u32,
    /// By `ord`.
    pub symbols: Vec<CodeSymbolRow>,
    pub decls: Vec<CodeDeclRow>,
    pub markers: Vec<CodeMarkerRow>,
    pub mentions: Vec<CodeMentionRow>,
}

/// One item of a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeSymbolRow {
    pub ord: u32,
    pub item_kind: String,
    /// The bare last segment, without a discriminator.
    pub name: String,
    pub owner: Option<String>,
    /// Rendered, a twin's discriminator in it (`<u> #<discriminator>`).
    pub qpath: String,
    pub discriminator: Option<String>,
    pub parent_ord: Option<u32>,
    pub start_line: u32,
    pub end_line: u32,
    pub start_byte: u32,
    pub end_byte: u32,
    pub signature: Option<String>,
    pub level_state: String,
    /// JSON list of the categories (`[]` when hashed).
    pub level_categories: String,
    pub sig_hash: Option<String>,
    pub body_hash: Option<String>,
    pub deps_hash: Option<String>,
    pub skeleton_hash: Option<String>,
    pub quote_hash: Option<String>,
    pub body_bytes: Option<u32>,
    pub layer: String,
    /// The qpath's last d structured segments joined, for d = 1..: the
    /// suffix rows of depth d (the analyzer's segments, never a split).
    pub suffixes: Vec<String>,
}

/// One `#[path]` declaration of a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeDeclRow {
    pub ord: u32,
    /// The declared module's file, root-relative.
    pub target: String,
    pub line: u32,
}

/// One marker of a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeMarkerRow {
    pub ord: u32,
    pub relation: String,
    pub token: String,
    pub id: String,
    pub rev: Option<u32>,
    /// JSON list of the effective levels; `None` when the list is invalid.
    pub levels: Option<String>,
    pub levels_state: String,
    pub note: Option<String>,
    pub line: u32,
    pub column: u32,
    /// What it binds.
    pub binding: String,
    /// The bound symbol's `ord`, when it binds one.
    pub symbol_ord: Option<u32>,
    /// The binding's own text: a module qpath, a value path, a category.
    pub anchor: Option<String>,
}

/// One comment mention of an ID in a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeMentionRow {
    pub ord: u32,
    pub written: String,
    pub line: u32,
    pub column: u32,
    pub binding: String,
    pub symbol_ord: Option<u32>,
    pub anchor: Option<String>,
}

/// What a code refresh did; serialized as `spec index`'s JSON `code`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CodeReport {
    /// Files listed.
    pub walked: usize,
    /// Files whose rows were written from a fresh analysis.
    pub parsed: usize,
    /// Files whose rows were kept: the same cache key.
    pub unchanged: usize,
    /// Stored files no longer listed, whose rows were deleted.
    pub removed: usize,
    /// Files written with a `read_error`.
    pub unreadable: usize,
    /// Names skipped for not being UTF-8, and listed files whose bytes are
    /// not UTF-8.
    pub skipped: usize,
    /// The worktree's rows after the refresh.
    pub symbols: usize,
    pub markers: usize,
    pub mentions: usize,
    /// Every file was re-derived: `full`, a `code_fp` change, or the index
    /// recreated by this handle's spec update.
    pub rederived_all: bool,
    /// The listing's gaps ([`CodeListing::gaps`]).
    #[serde(skip)]
    pub gaps: Vec<String>,
    /// The listed files whose bytes are not UTF-8, by path.
    #[serde(skip)]
    pub not_utf8: Vec<String>,
    /// The stored files with a `read_error`, by path: (path, error).
    #[serde(skip)]
    pub read_errors: Vec<(String, String)>,
}

/// A rendered name and why it is not trusted, if it is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredName {
    pub qpath: String,
    /// [`AMBIGUITY_PATH_ATTRIBUTE`], [`AMBIGUITY_UNROOTED`] or
    /// [`AMBIGUITY_DUPLICATE`].
    pub ambiguity: Option<String>,
    pub discriminator: Option<String>,
}

/// The names one file holds
/// (`docs/canon/symbol-index.md` "Crates and seams", 6b's partial tree):
/// its module and every item by `ord`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredNames {
    pub module: StoredName,
    pub items: Vec<StoredName>,
}

/// One stored file as a read sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeFileState {
    pub path: String,
    pub lang: String,
    pub utf8: bool,
    pub read_error: Option<String>,
    /// Its module and the module's ambiguity (never `duplicate`).
    pub module: Option<StoredName>,
}

/// The symbol a marker or a mention is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundSymbol {
    /// Its `ord` in the marker's or mention's own file.
    pub ord: u32,
    pub qpath: String,
    pub item_kind: String,
    pub start_line: u32,
    pub ambiguity: Option<String>,
}

/// What a stored marker or mention binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBinding {
    pub binding: String,
    pub anchor: Option<String>,
    /// The symbol at `symbol_ord`, when one is named.
    pub symbol: Option<BoundSymbol>,
}

/// One stored marker with its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMarker {
    pub path: String,
    pub relation: String,
    pub token: String,
    pub id: String,
    pub rev: Option<u32>,
    pub levels: Option<String>,
    pub levels_state: String,
    pub note: Option<String>,
    pub line: u32,
    pub column: u32,
    pub binding: StoredBinding,
}

/// One stored mention with its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredMention {
    pub path: String,
    pub written: String,
    pub line: u32,
    pub column: u32,
    pub binding: StoredBinding,
}

/// Everything `spec refs` reads of the code index: every file, marker and
/// mention of the worktree, by (path, ord).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeRefs {
    pub files: Vec<CodeFileState>,
    pub markers: Vec<StoredMarker>,
    pub mentions: Vec<StoredMention>,
}

/// One stored symbol with its file and its ambiguity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSymbol {
    pub path: String,
    pub ord: u32,
    pub item_kind: String,
    pub name: String,
    pub owner: Option<String>,
    pub qpath: String,
    pub discriminator: Option<String>,
    pub parent_ord: Option<u32>,
    pub start_line: u32,
    pub end_line: u32,
    /// Its byte range in its file as indexed.
    pub start_byte: u32,
    pub end_byte: u32,
    pub signature: Option<String>,
    pub level_state: String,
    /// The layer whose rules made the row.
    pub layer: String,
    pub twin_files: u32,
    pub ambiguity: Option<String>,
}

impl SqliteIndex {
    /// Brings the worktree's code rows up to date with `analyzer`'s tree;
    /// `full` re-derives every file. Analyses run before the write
    /// transaction; one `Immediate` transaction re-checks the cache keys,
    /// applies and runs the cross-file pass. The worktree must be indexed
    /// (its spec rows of the current format): [`StoreError::NotIndexed`]
    /// otherwise. Queue tables are never touched.
    pub fn update_code(
        &mut self,
        analyzer: &dyn CodeAnalyzer,
        scheme: &IdScheme,
        full: bool,
    ) -> Result<CodeReport, StoreError> {
        update(self, analyzer, scheme, full)
    }

    /// Deletes the worktree's code rows (no `[code]`); `true` when there
    /// were any. A worktree not indexed has none.
    pub fn clear_code(&mut self) -> Result<bool, StoreError> {
        let tx = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate).db()?;
        if !schema::format_is_current(&tx)? {
            return Ok(false);
        }
        let Some(wt) = worktree_id(&tx, self)? else {
            return Ok(false);
        };
        let deleted = tx
            .execute("DELETE FROM code_worktrees WHERE wt = ?1", [wt])
            .db()?;
        tx.commit().db()?;
        Ok(deleted > 0)
    }

    /// The stored names of one file: its module and each item's qpath and
    /// ambiguity, by `ord`; `Ok(None)` when the path is not stored or holds
    /// no module (a file of another language, unreadable or not UTF-8).
    /// Reads no code: a file edited since keeps its stored names.
    pub fn stored_names(&self, path: &str) -> Result<Option<StoredNames>, StoreError> {
        with_worktree(self, |tx, wt| {
            let file = tx
                .query_row(
                    "SELECT file_id, module, role, declared_by IS NOT NULL
                     FROM code_files WHERE wt = ?1 AND path = ?2",
                    (wt, path),
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, bool>(3)?,
                        ))
                    },
                )
                .optional()
                .db()?;
            let Some((file_id, Some(module), role, declared)) = file else {
                return Ok(None);
            };
            let facts = FileFacts {
                declared,
                role: role.as_deref(),
            };
            let mut statement = tx
                .prepare(
                    "SELECT qpath, discriminator, twin_files FROM symbols
                     WHERE file = ?1 ORDER BY ord",
                )
                .db()?;
            let items = statement
                .query_map([file_id], |row| {
                    let twin_files: i64 = row.get(2)?;
                    Ok(StoredName {
                        qpath: row.get(0)?,
                        ambiguity: facts.item_ambiguity(twin_files).map(str::to_owned),
                        discriminator: row.get(1)?,
                    })
                })
                .db()?
                .collect::<Result<Vec<_>, _>>()
                .db()?;
            Ok(Some(StoredNames {
                module: StoredName {
                    qpath: module,
                    ambiguity: facts.module_ambiguity().map(str::to_owned),
                    discriminator: None,
                },
                items,
            }))
        })
    }

    /// Every stored file, marker and mention of the worktree; `Ok(None)`
    /// when no code is indexed.
    pub fn code_refs(&self) -> Result<Option<CodeRefs>, StoreError> {
        with_worktree(self, |tx, wt| {
            if stored_code_fp(tx, wt)?.is_none() {
                return Ok(None);
            }
            Ok(Some(refs_in(tx, wt)?))
        })
    }

    /// Every stored symbol of the worktree with its file and ambiguity, by
    /// (path, ord); `Ok(None)` when no code is indexed.
    pub fn code_symbols(&self) -> Result<Option<Vec<StoredSymbol>>, StoreError> {
        with_worktree(self, |tx, wt| {
            if stored_code_fp(tx, wt)?.is_none() {
                return Ok(None);
            }
            Ok(Some(all_symbols(tx, wt)?))
        })
    }

    /// One read snapshot of the code index: [`Self::code_refs`]' files,
    /// markers and mentions and the symbols `symbols` names, read in one
    /// transaction, so a refresh by another connection between two reads
    /// never pairs one state's markers with another's symbols (a read's
    /// one snapshot: `docs/canon/symbol-reads.md` "Crates and seams");
    /// `Ok(None)` when no code is indexed.
    pub fn code_snapshot(
        &self,
        symbols: SnapshotSymbols<'_>,
    ) -> Result<Option<CodeSnapshot>, StoreError> {
        with_worktree(self, |tx, wt| {
            if stored_code_fp(tx, wt)?.is_none() {
                return Ok(None);
            }
            let mut snapshot = CodeSnapshot {
                refs: refs_in(tx, wt)?,
                hits: Vec::new(),
                symbols: Vec::new(),
            };
            match symbols {
                SnapshotSymbols::Lookup { query, name } => {
                    snapshot.hits = lookup_in(tx, wt, query, name)?;
                }
                SnapshotSymbols::File(path) => snapshot.symbols = file_symbols_in(tx, wt, path)?,
                SnapshotSymbols::Bound => snapshot.symbols = bound_symbols(tx, wt)?,
                SnapshotSymbols::All => snapshot.symbols = all_symbols(tx, wt)?,
            }
            Ok(Some(snapshot))
        })
    }
}

/// Which symbols a [`CodeSnapshot`] holds beside its refs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSymbols<'q> {
    /// [`SqliteIndex::lookup_symbols`]' hits of `query`, `name` its name
    /// key, in [`CodeSnapshot::hits`].
    Lookup { query: &'q str, name: &'q str },
    /// [`SqliteIndex::file_symbols`] of one path.
    File(&'q str),
    /// Each symbol a stored marker is bound to, once, by (path, ord).
    Bound,
    /// Every symbol, as [`SqliteIndex::code_symbols`].
    All,
}

/// What [`SqliteIndex::code_snapshot`] read in its one transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CodeSnapshot {
    /// Every stored file, marker and mention, as [`SqliteIndex::code_refs`].
    pub refs: CodeRefs,
    /// [`SnapshotSymbols::Lookup`]'s hits in the lookup's order; else none.
    pub hits: Vec<SymbolHit>,
    /// The other forms' symbols, by (path, ord); none for a lookup.
    pub symbols: Vec<StoredSymbol>,
}

/// Every stored file, marker and mention of the worktree inside the
/// caller's transaction.
fn refs_in(tx: &Transaction<'_>, wt: i64) -> Result<CodeRefs, StoreError> {
    Ok(CodeRefs {
        files: file_states(tx, wt)?,
        markers: stored_markers(tx, wt)?,
        mentions: stored_mentions(tx, wt)?,
    })
}

/// Every stored symbol of the worktree by (path, ord) inside the caller's
/// transaction.
fn all_symbols(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredSymbol>, StoreError> {
    let mut statement = tx
        .prepare(concat!(
            "SELECT ",
            symbol_columns!(),
            " FROM code_files f JOIN symbols s ON s.file = f.file_id
             WHERE f.wt = ?1 ORDER BY f.path, s.ord"
        ))
        .db()?;
    let symbols = statement
        .query_map([wt], stored_symbol)
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(symbols)
}

/// Each stored symbol a marker of the worktree is bound to, once, by
/// (path, ord), inside the caller's transaction: read from the markers,
/// never by scanning the symbols.
fn bound_symbols(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredSymbol>, StoreError> {
    let mut statement = tx
        .prepare(concat!(
            "SELECT DISTINCT ",
            symbol_columns!(),
            " FROM code_files f JOIN code_markers m ON m.file = f.file_id
             JOIN symbols s ON s.file = m.file AND s.ord = m.symbol_ord
             WHERE f.wt = ?1 ORDER BY f.path, s.ord"
        ))
        .db()?;
    let symbols = statement
        .query_map([wt], stored_symbol)
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(symbols)
}

/// A symbol with its file and its ambiguity from a row of
/// [`symbol_columns!`] (first).
pub(crate) fn stored_symbol(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredSymbol> {
    let twin_files: i64 = row.get(15)?;
    let role: Option<String> = row.get(17)?;
    let facts = FileFacts {
        declared: row.get(16)?,
        role: role.as_deref(),
    };
    Ok(StoredSymbol {
        path: row.get(0)?,
        ord: row.get(1)?,
        item_kind: row.get(2)?,
        name: row.get(3)?,
        owner: row.get(4)?,
        qpath: row.get(5)?,
        discriminator: row.get(6)?,
        parent_ord: row.get(7)?,
        start_line: row.get(8)?,
        end_line: row.get(9)?,
        start_byte: row.get(10)?,
        end_byte: row.get(11)?,
        signature: row.get(12)?,
        level_state: row.get(13)?,
        layer: row.get(14)?,
        twin_files: u32::try_from(twin_files).unwrap_or(u32::MAX),
        ambiguity: facts.item_ambiguity(twin_files).map(str::to_owned),
    })
}

/// What a file's row says about its names' ambiguity.
#[derive(Clone, Copy)]
struct FileFacts<'a> {
    /// A file of the worktree declares it with a `#[path]` attribute.
    declared: bool,
    role: Option<&'a str>,
}

impl FileFacts<'_> {
    fn module_ambiguity(self) -> Option<&'static str> {
        if self.declared && self.role != Some(ROLE_SHARED) {
            Some(AMBIGUITY_PATH_ATTRIBUTE)
        } else if self.role == Some(ROLE_UNROOTED) {
            Some(AMBIGUITY_UNROOTED)
        } else {
            None
        }
    }

    fn item_ambiguity(self, twin_files: i64) -> Option<&'static str> {
        self.module_ambiguity()
            .or_else(|| (twin_files > 0).then_some(AMBIGUITY_DUPLICATE))
    }
}

/// The stored `code_fp` of the worktree, if it has code rows.
fn stored_code_fp(tx: &Transaction<'_>, wt: i64) -> Result<Option<String>, StoreError> {
    tx.query_row(
        "SELECT code_fp FROM code_worktrees WHERE wt = ?1",
        [wt],
        |row| row.get(0),
    )
    .optional()
    .db()
}

fn file_states(tx: &Transaction<'_>, wt: i64) -> Result<Vec<CodeFileState>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT path, lang, utf8, read_error, module, role, declared_by IS NOT NULL
             FROM code_files WHERE wt = ?1 ORDER BY path",
        )
        .db()?;
    let files = statement
        .query_map([wt], |row| {
            let role: Option<String> = row.get(5)?;
            let facts = FileFacts {
                declared: row.get(6)?,
                role: role.as_deref(),
            };
            let module: Option<String> = row.get(4)?;
            Ok(CodeFileState {
                path: row.get(0)?,
                lang: row.get(1)?,
                utf8: row.get(2)?,
                read_error: row.get(3)?,
                module: module.map(|qpath| StoredName {
                    qpath,
                    ambiguity: facts.module_ambiguity().map(str::to_owned),
                    discriminator: None,
                }),
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(files)
}

/// The binding of a marker or mention row: its columns from `first` on are
/// `binding`, `anchor`, then the bound symbol's `qpath`, `item_kind`,
/// `start_line`, `twin_files`, `ord` (NULL without one), then the file's
/// `declared_by IS NOT NULL` and `role`.
fn binding_at(row: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<StoredBinding> {
    let role: Option<String> = row.get(first + 8)?;
    let facts = FileFacts {
        declared: row.get(first + 7)?,
        role: role.as_deref(),
    };
    let qpath: Option<String> = row.get(first + 2)?;
    let symbol = match qpath {
        Some(qpath) => {
            let twin_files: i64 = row.get(first + 5)?;
            Some(BoundSymbol {
                ord: row.get(first + 6)?,
                qpath,
                item_kind: row.get(first + 3)?,
                start_line: row.get(first + 4)?,
                ambiguity: facts.item_ambiguity(twin_files).map(str::to_owned),
            })
        }
        None => None,
    };
    Ok(StoredBinding {
        binding: row.get(first)?,
        anchor: row.get(first + 1)?,
        symbol,
    })
}

fn stored_markers(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredMarker>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, m.relation, m.token, m.id, m.rev, m.levels, m.levels_state,
                    m.note, m.line, m.\"column\",
                    m.binding, m.anchor, s.qpath, s.item_kind, s.start_line, s.twin_files,
                    s.ord, f.declared_by IS NOT NULL, f.role
             FROM code_files f JOIN code_markers m ON m.file = f.file_id
             LEFT JOIN symbols s ON s.file = m.file AND s.ord = m.symbol_ord
             WHERE f.wt = ?1 ORDER BY f.path, m.ord",
        )
        .db()?;
    let markers = statement
        .query_map([wt], |row| {
            Ok(StoredMarker {
                path: row.get(0)?,
                relation: row.get(1)?,
                token: row.get(2)?,
                id: row.get(3)?,
                rev: row.get(4)?,
                levels: row.get(5)?,
                levels_state: row.get(6)?,
                note: row.get(7)?,
                line: row.get(8)?,
                column: row.get(9)?,
                binding: binding_at(row, 10)?,
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(markers)
}

fn stored_mentions(tx: &Transaction<'_>, wt: i64) -> Result<Vec<StoredMention>, StoreError> {
    let mut statement = tx
        .prepare(
            "SELECT f.path, m.written, m.line, m.\"column\",
                    m.binding, m.anchor, s.qpath, s.item_kind, s.start_line, s.twin_files,
                    s.ord, f.declared_by IS NOT NULL, f.role
             FROM code_files f JOIN code_mentions m ON m.file = f.file_id
             LEFT JOIN symbols s ON s.file = m.file AND s.ord = m.symbol_ord
             WHERE f.wt = ?1 ORDER BY f.path, m.ord",
        )
        .db()?;
    let mentions = statement
        .query_map([wt], |row| {
            Ok(StoredMention {
                path: row.get(0)?,
                written: row.get(1)?,
                line: row.get(2)?,
                column: row.get(3)?,
                binding: binding_at(row, 4)?,
            })
        })
        .db()?
        .collect::<Result<Vec<_>, _>>()
        .db()?;
    Ok(mentions)
}

// ------------------------------------------------------------------ write

/// The cache key a stored file was written under.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    blake3: Option<String>,
    package: Option<String>,
    targets_fp: Option<String>,
}

/// What to do with one listed file.
enum Plan {
    /// The stored row has this key.
    Unchanged(Key),
    /// Rows to write under this key, with the file's size.
    Write(Key, i64, Box<CodeFileRows>),
}

/// `b3:` BLAKE3 of the parts, each prefixed by its length (8 bytes,
/// little-endian), so no two lists of parts share a stream.
fn framed_hash(parts: &[&[u8]]) -> String {
    let mut stream = Vec::new();
    for part in parts {
        let length = u64::try_from(part.len()).unwrap_or(u64::MAX);
        stream.extend_from_slice(&length.to_le_bytes());
        stream.extend_from_slice(part);
    }
    b3_hash(&stream)
}

fn update(
    index: &mut SqliteIndex,
    analyzer: &dyn CodeAnalyzer,
    scheme: &IdScheme,
    full: bool,
) -> Result<CodeReport, StoreError> {
    let ids = write::fingerprint(scheme)?;
    let code_fp = framed_hash(&[&analyzer.fingerprint(), ids.as_bytes()]);
    let listing = analyzer.list()?;
    let packages = analyzer.packages(&listing)?;
    let targets: BTreeMap<&str, (&CodePackageRow, String)> = packages
        .iter()
        .map(|row| {
            let fp = framed_hash(&[row.source.as_bytes(), row.targets.as_bytes()]);
            (row.dir.as_str(), (row, fp))
        })
        .collect();
    let mut seen = BTreeSet::new();
    let files: Vec<&ListedCodeFile> = listing
        .files
        .iter()
        .filter(|file| seen.insert(file.path.as_str()))
        .collect();
    let key_of = |file: &ListedCodeFile| -> Key {
        let package = file.package.clone();
        let targets_fp = package
            .as_deref()
            .and_then(|dir| targets.get(dir))
            .map(|(_, fp)| fp.clone());
        Key {
            blake3: file.bytes.as_deref().map(hash_bytes),
            package,
            targets_fp,
        }
    };
    let analyze = |file: &ListedCodeFile| -> Box<CodeFileRows> {
        let package = file
            .package
            .as_deref()
            .and_then(|dir| targets.get(dir))
            .map(|(row, _)| *row);
        Box::new(analyzer.analyze(file, package))
    };
    let size = |file: &ListedCodeFile| -> i64 {
        file.bytes
            .as_ref()
            .map_or(0, |bytes| i64::try_from(bytes.len()).unwrap_or(i64::MAX))
    };

    // The plan, against a snapshot, analyses outside any write lock.
    let (snapshot_fp, snapshot) = {
        let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Deferred).db()?;
        if !schema::format_is_current(&tx)? {
            return Err(StoreError::NotIndexed);
        }
        let wt = worktree_id(&tx, index)?.ok_or(StoreError::NotIndexed)?;
        let fp = stored_code_fp(&tx, wt)?;
        let stored = stored_keys(&tx, wt)?;
        tx.commit().db()?;
        (fp, stored)
    };
    // A file's rows are kept only under the stamp they were written with.
    let reusable = |stored: Option<&str>| !full && stored == Some(code_fp.as_str());
    let mut plans = Vec::with_capacity(files.len());
    for file in &files {
        let key = key_of(file);
        let kept = reusable(snapshot_fp.as_deref())
            && key.blake3.is_some()
            && snapshot.get(&file.path).map(|(_, stored)| stored) == Some(&key);
        plans.push(if kept {
            Plan::Unchanged(key)
        } else {
            let rows = analyze(file);
            Plan::Write(key, size(file), rows)
        });
    }

    let tx = Transaction::new_unchecked(&index.conn, TransactionBehavior::Immediate).db()?;
    if !schema::format_is_current(&tx)? {
        return Err(StoreError::NotIndexed);
    }
    let wt = worktree_id(&tx, index)?.ok_or(StoreError::NotIndexed)?;
    // Re-checked here: another writer may have moved the stamp since the
    // snapshot.
    let current_fp = stored_code_fp(&tx, wt)?;
    let reuse = reusable(current_fp.as_deref());
    let rederived_all = full
        || current_fp.as_deref().is_some_and(|fp| fp != code_fp)
        || current_fp.is_none() && index.reparsed_all;
    // A no-op refresh writes nothing: the stamp and the package rows are
    // written only when they differ.
    match current_fp.as_deref() {
        Some(fp) if fp == code_fp => {}
        Some(_) => {
            tx.execute(
                "UPDATE code_worktrees SET code_fp = ?1 WHERE wt = ?2",
                (code_fp.as_str(), wt),
            )
            .db()?;
        }
        None => {
            tx.execute(
                "INSERT INTO code_worktrees (wt, code_fp) VALUES (?1, ?2)",
                (wt, code_fp.as_str()),
            )
            .db()?;
        }
    }
    write_packages(&tx, wt, &packages)?;

    let stored = stored_keys(&tx, wt)?;
    let mut writer = Writer::new(&tx)?;
    let mut touched = Touched::default();
    let mut report = CodeReport {
        walked: files.len(),
        rederived_all,
        gaps: listing.gaps.clone(),
        ..CodeReport::default()
    };
    let listed: BTreeSet<&str> = files.iter().map(|file| file.path.as_str()).collect();
    for (path, (file_id, _)) in &stored {
        if !listed.contains(path.as_str()) {
            writer.forget(*file_id, path, &mut touched)?;
            report.removed += 1;
        }
    }
    for (file, plan) in files.iter().zip(plans) {
        let row = stored.get(&file.path);
        let (key, size, rows) = match plan {
            Plan::Unchanged(key) if reuse && row.map(|(_, stored)| stored) == Some(&key) => {
                report.unchanged += 1;
                continue;
            }
            // The row moved since the snapshot, or everything must be
            // re-derived now: analyse in place.
            Plan::Unchanged(key) => (key, size(file), analyze(file)),
            Plan::Write(key, size, rows) => (key, size, rows),
        };
        if let Some((file_id, _)) = row {
            writer.forget(*file_id, &file.path, &mut touched)?;
        }
        writer.insert(wt, &file.path, &key, size, &rows, &mut touched)?;
        if rows.read_error.is_some() {
            report.unreadable += 1;
        } else {
            report.parsed += 1;
        }
    }
    drop(writer);

    pass(&tx, wt, &touched)?;
    report.symbols = count(&tx, "symbols", wt)?;
    report.markers = count(&tx, "code_markers", wt)?;
    report.mentions = count(&tx, "code_mentions", wt)?;
    report.not_utf8 = {
        let mut statement = tx
            .prepare("SELECT path FROM code_files WHERE wt = ?1 AND utf8 = 0 ORDER BY path")
            .db()?;
        statement
            .query_map([wt], |row| row.get::<_, String>(0))
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?
    };
    report.read_errors = {
        let mut statement = tx
            .prepare(
                "SELECT path, read_error FROM code_files
                 WHERE wt = ?1 AND read_error IS NOT NULL ORDER BY path",
            )
            .db()?;
        statement
            .query_map([wt], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?
    };
    report.skipped = listing.skipped + report.not_utf8.len();
    tx.commit().db()?;
    Ok(report)
}

/// The package rows of a refresh, by directory (a repeated directory's
/// first row kept): what `code_packages` holds after it.
fn package_rows(packages: &[CodePackageRow]) -> Vec<&CodePackageRow> {
    let mut by_dir: BTreeMap<&str, &CodePackageRow> = BTreeMap::new();
    for row in packages {
        by_dir.entry(row.dir.as_str()).or_insert(row);
    }
    by_dir.into_values().collect()
}

/// Rewrites the worktree's `code_packages` with `packages`, only when the
/// stored rows differ.
fn write_packages(
    tx: &Transaction<'_>,
    wt: i64,
    packages: &[CodePackageRow],
) -> Result<(), StoreError> {
    let rows = package_rows(packages);
    let stored = {
        let mut statement = tx
            .prepare(
                "SELECT dir, manifest_b3, source, targets FROM code_packages
                 WHERE wt = ?1 ORDER BY dir",
            )
            .db()?;
        statement
            .query_map([wt], |row| {
                Ok(CodePackageRow {
                    dir: row.get(0)?,
                    manifest_b3: row.get(1)?,
                    source: row.get(2)?,
                    targets: row.get(3)?,
                })
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?
    };
    if stored.len() == rows.len() && stored.iter().zip(&rows).all(|(old, new)| old == *new) {
        return Ok(());
    }
    tx.execute("DELETE FROM code_packages WHERE wt = ?1", [wt])
        .db()?;
    let mut insert = tx
        .prepare(
            "INSERT INTO code_packages (wt, dir, manifest_b3, source, targets)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )
        .db()?;
    for row in rows {
        insert
            .execute((
                wt,
                row.dir.as_str(),
                row.manifest_b3.as_deref(),
                row.source.as_str(),
                row.targets.as_str(),
            ))
            .db()?;
    }
    Ok(())
}

/// path → (file_id, key) of the worktree's stored code files.
fn stored_keys(tx: &Transaction<'_>, wt: i64) -> Result<BTreeMap<String, (i64, Key)>, StoreError> {
    let mut statement = tx
        .prepare("SELECT path, file_id, blake3, package, targets_fp FROM code_files WHERE wt = ?1")
        .db()?;
    let rows = statement
        .query_map([wt], |row| {
            Ok((
                row.get::<_, String>(0)?,
                (
                    row.get::<_, i64>(1)?,
                    Key {
                        blake3: row.get(2)?,
                        package: row.get(3)?,
                        targets_fp: row.get(4)?,
                    },
                ),
            ))
        })
        .db()?
        .collect::<Result<BTreeMap<_, _>, _>>()
        .db()?;
    Ok(rows)
}

/// The rows of `table` (a code table keyed by `file`) in the worktree.
fn count(tx: &Transaction<'_>, table: &str, wt: i64) -> Result<usize, StoreError> {
    let sql = format!(
        "SELECT count(*) FROM code_files f JOIN {} x ON x.file = f.file_id WHERE f.wt = ?1",
        schema::quoted(table)
    );
    let rows: i64 = tx.query_row(&sql, [wt], |row| row.get(0)).db()?;
    Ok(usize::try_from(rows).unwrap_or(usize::MAX))
}

/// What the pass must look at again.
#[derive(Default)]
struct Touched {
    /// Undiscriminated qpaths a removed or re-derived file rendered or
    /// renders.
    twins: BTreeSet<String>,
    /// Paths a removed or re-derived file declared or declares, and the
    /// re-derived files themselves.
    declared: BTreeSet<String>,
}

/// The qpath without its twin's discriminator: the stored qpath minus the
/// ` #<discriminator>` it ends with (never a split of the name).
fn undiscriminated<'q>(qpath: &'q str, discriminator: Option<&str>) -> &'q str {
    discriminator
        .and_then(|value| {
            qpath
                .strip_suffix(value)?
                .strip_suffix(DISCRIMINATOR_SEPARATOR)
        })
        .unwrap_or(qpath)
}

/// The prepared statements of one write transaction.
struct Writer<'t> {
    old_symbols: Statement<'t>,
    old_decls: Statement<'t>,
    delete_file: Statement<'t>,
    insert_file: Statement<'t>,
    insert_symbol: Statement<'t>,
    insert_suffix: Statement<'t>,
    insert_decl: Statement<'t>,
    insert_marker: Statement<'t>,
    insert_mention: Statement<'t>,
}

impl<'t> Writer<'t> {
    fn new(tx: &'t Transaction<'_>) -> Result<Self, StoreError> {
        Ok(Self {
            old_symbols: tx
                .prepare("SELECT qpath, discriminator FROM symbols WHERE file = ?1")
                .db()?,
            old_decls: tx
                .prepare("SELECT target FROM code_decls WHERE file = ?1")
                .db()?,
            delete_file: tx
                .prepare("DELETE FROM code_files WHERE file_id = ?1")
                .db()?,
            insert_file: tx
                .prepare(
                    "INSERT INTO code_files
                       (wt, path, lang, blake3, size, read_error, utf8, package, role, module,
                        orphan_errors, targets_fp, declared_by)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL)",
                )
                .db()?,
            insert_symbol: tx
                .prepare(
                    "INSERT INTO symbols
                       (file, ord, item_kind, name, owner, qpath, discriminator, parent_ord,
                        start_line, end_line, start_byte, end_byte, signature, level_state,
                        level_categories, sig_hash, body_hash, deps_hash, skeleton_hash,
                        quote_hash, body_bytes, layer, twin_files)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                             ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, 0)",
                )
                .db()?,
            insert_suffix: tx
                .prepare(
                    "INSERT INTO symbol_suffixes (file, ord, depth, suffix)
                     VALUES (?1, ?2, ?3, ?4)",
                )
                .db()?,
            insert_decl: tx
                .prepare("INSERT INTO code_decls (file, ord, target, line) VALUES (?1, ?2, ?3, ?4)")
                .db()?,
            insert_marker: tx
                .prepare(
                    "INSERT INTO code_markers
                       (file, ord, relation, token, id, rev, levels, levels_state, note, line,
                        \"column\", binding, symbol_ord, anchor)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                )
                .db()?,
            insert_mention: tx
                .prepare(
                    "INSERT INTO code_mentions
                       (file, ord, written, line, \"column\", binding, symbol_ord, anchor)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                )
                .db()?,
        })
    }

    /// Deletes a stored file's rows (by cascade), noting what the pass must
    /// look at again.
    fn forget(
        &mut self,
        file_id: i64,
        path: &str,
        touched: &mut Touched,
    ) -> Result<(), StoreError> {
        let names = self
            .old_symbols
            .query_map([file_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        for (qpath, discriminator) in &names {
            touched
                .twins
                .insert(undiscriminated(qpath, discriminator.as_deref()).to_owned());
        }
        let targets = self
            .old_decls
            .query_map([file_id], |row| row.get::<_, String>(0))
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        touched.declared.extend(targets);
        touched.declared.insert(path.to_owned());
        self.delete_file.execute([file_id]).db()?;
        Ok(())
    }

    fn insert(
        &mut self,
        wt: i64,
        path: &str,
        key: &Key,
        size: i64,
        rows: &CodeFileRows,
        touched: &mut Touched,
    ) -> Result<(), StoreError> {
        let file_id = self
            .insert_file
            .insert((
                wt,
                path,
                rows.lang.as_str(),
                key.blake3.as_deref(),
                size,
                rows.read_error.as_deref(),
                rows.utf8,
                key.package.as_deref(),
                rows.role.as_deref(),
                rows.module.as_deref(),
                rows.orphan_errors,
                key.targets_fp.as_deref(),
            ))
            .db()?;
        touched.declared.insert(path.to_owned());
        for symbol in &rows.symbols {
            self.insert_symbol
                .execute(rusqlite::params![
                    file_id,
                    symbol.ord,
                    symbol.item_kind,
                    symbol.name,
                    symbol.owner,
                    symbol.qpath,
                    symbol.discriminator,
                    symbol.parent_ord,
                    symbol.start_line,
                    symbol.end_line,
                    symbol.start_byte,
                    symbol.end_byte,
                    symbol.signature,
                    symbol.level_state,
                    symbol.level_categories,
                    symbol.sig_hash,
                    symbol.body_hash,
                    symbol.deps_hash,
                    symbol.skeleton_hash,
                    symbol.quote_hash,
                    symbol.body_bytes,
                    symbol.layer,
                ])
                .db()?;
            for (depth, suffix) in symbol.suffixes.iter().enumerate() {
                let depth = i64::try_from(depth).unwrap_or(i64::MAX - 1) + 1;
                self.insert_suffix
                    .execute((file_id, symbol.ord, depth, suffix.as_str()))
                    .db()?;
            }
            touched
                .twins
                .insert(undiscriminated(&symbol.qpath, symbol.discriminator.as_deref()).to_owned());
        }
        for decl in &rows.decls {
            self.insert_decl
                .execute((file_id, decl.ord, decl.target.as_str(), decl.line))
                .db()?;
            touched.declared.insert(decl.target.clone());
        }
        for marker in &rows.markers {
            self.insert_marker
                .execute(rusqlite::params![
                    file_id,
                    marker.ord,
                    marker.relation,
                    marker.token,
                    marker.id,
                    marker.rev,
                    marker.levels,
                    marker.levels_state,
                    marker.note,
                    marker.line,
                    marker.column,
                    marker.binding,
                    marker.symbol_ord,
                    marker.anchor,
                ])
                .db()?;
        }
        for mention in &rows.mentions {
            self.insert_mention
                .execute(rusqlite::params![
                    file_id,
                    mention.ord,
                    mention.written,
                    mention.line,
                    mention.column,
                    mention.binding,
                    mention.symbol_ord,
                    mention.anchor,
                ])
                .db()?;
        }
        Ok(())
    }
}

/// The cross-file pass over what the transaction touched: `declared_by`
/// of each touched path that is stored, `twin_files` of each symbol
/// rendering a touched undiscriminated qpath (the number of other files
/// rendering it, every file counted).
fn pass(tx: &Transaction<'_>, wt: i64, touched: &Touched) -> Result<(), StoreError> {
    let mut declared_by = tx.prepare(DECLARED_BY_SQL).db()?;
    let mut set_declared = tx
        .prepare(
            "UPDATE code_files SET declared_by = ?1
             WHERE wt = ?2 AND path = ?3 AND declared_by IS NOT ?1",
        )
        .db()?;
    for path in &touched.declared {
        let by: Option<String> = declared_by
            .query_row((path.as_str(), wt), |row| row.get(0))
            .db()?;
        set_declared
            .execute((by.as_deref(), wt, path.as_str()))
            .db()?;
    }

    let [plain_sql, twins_sql] = TWIN_SQL else {
        return Err(StoreError::Sqlite(
            "the twin pass needs its two statements".to_owned(),
        ));
    };
    let mut plain = tx.prepare(plain_sql).db()?;
    let mut twins = tx.prepare(twins_sql).db()?;
    let mut set_twins = tx
        .prepare("UPDATE symbols SET twin_files = ?1 WHERE file = ?2 AND ord = ?3")
        .db()?;
    let read = |row: &rusqlite::Row<'_>| -> rusqlite::Result<(i64, i64, i64)> {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    };
    for name in &touched.twins {
        let low = format!("{name}{DISCRIMINATOR_SEPARATOR}");
        let high = format!("{name}{SEPARATOR_END}");
        let mut rows = plain
            .query_map((name.as_str(), wt), read)
            .db()?
            .collect::<Result<Vec<_>, _>>()
            .db()?;
        rows.extend(
            twins
                .query_map((low.as_str(), high.as_str(), name.as_str(), wt), read)
                .db()?
                .collect::<Result<Vec<_>, _>>()
                .db()?,
        );
        let files: BTreeSet<i64> = rows.iter().map(|(file, _, _)| *file).collect();
        let others = i64::try_from(files.len())
            .unwrap_or(i64::MAX)
            .saturating_sub(1);
        for (file, ord, stored) in rows {
            if stored != others {
                set_twins.execute((others, file, ord)).db()?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    //! docs/features/symbol-index.md RD-3. M: a scan for cross-file twins.
    //! Every statement the pass runs to find the items rendering an
    //! undiscriminated qpath `u` (`qpath = u`, or a twin whose
    //! `discriminator` is set and whose `qpath` is `u` ‖ ` #` ‖ it: the
    //! 6a-1 review's S4, never a split of the name) is planned by SQLite as
    //! a search of the `qpath` index (`EXPLAIN QUERY PLAN`: a `SEARCH …
    //! USING … INDEX … (qpath…)` row, no `SCAN` of a table). Without
    //! `ANALYZE` statistics SQLite plans for large tables, so an empty
    //! database shows the plan a large one gets.

    use super::TWIN_SQL;
    use crate::index::SqliteIndex;

    /// A scratch directory, removed on drop.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .join(format!("specengine-store-twin-plan-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(path.join("wt")).expect("scratch");
            std::fs::create_dir_all(path.join("data")).expect("scratch");
            Self(std::fs::canonicalize(path).expect("canonical"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The plan's `detail` rows of `sql`, every parameter bound to a text.
    fn plan(index: &SqliteIndex, sql: &str) -> Vec<String> {
        let mut statement = index
            .conn
            .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
            .unwrap_or_else(|error| panic!("{sql}: {error}"));
        let count = statement.parameter_count();
        let values: Vec<String> = (0..count).map(|n| format!("q{n}")).collect();
        statement
            .query_map(rusqlite::params_from_iter(values.iter()), |row| {
                row.get::<_, String>(3)
            })
            .expect("plan")
            .collect::<Result<Vec<_>, _>>()
            .expect("plan rows")
    }

    #[test]
    fn twin_pass_reads_the_qpath_index() {
        let scratch = Scratch::new();
        let index = SqliteIndex::open(scratch.0.join("data/t.db"), "plan", scratch.0.join("wt"))
            .expect("open");
        assert!(!TWIN_SQL.is_empty(), "the pass reads twins somewhere");
        for sql in TWIN_SQL {
            let details = plan(&index, sql);
            assert!(
                details
                    .iter()
                    .any(|detail| detail.starts_with("SEARCH") && detail.contains("qpath")),
                "{sql}: no search of the qpath index: {details:?}"
            );
            assert!(
                !details
                    .iter()
                    .any(|detail| detail.starts_with("SCAN") && !detail.contains("CONSTANT ROW")),
                "{sql}: a scan: {details:?}"
            );
        }
    }
}
