---
class: canon
tier: 1
scope: [crates/specengine-store]
owner: owner
reviewed: 2026-09-30
---

# specengine-store — the spec index

A rebuildable SQLite + FTS5 projection of `specengine_core::parse` over one worktree, updated incrementally. The hard property: **after any sequence of edits the incremental index equals a fresh rebuild, row for row** (equal canonical dumps). Derived data outside the repository (ADR-0001, ADR-0003). A default member on `-model`, `-core`, `rusqlite =0.40.2` (`bundled`: SQLite 3.53.2, FTS5; never `sqlx`: `links = "sqlite3"`), `blake3`, `serde_json` (`float_roundtrip` workspace-wide: bit-exact floats). Callers: tests, `specengine-eval index`, `check`; the CLI and MCP will own freshness (`spec check` parses afresh).

## API

No `rusqlite` type in a public signature (`docs/canon/architecture.md#distribution`).

- `trait Source {root, list -> Listing {paths, missing_roots, skipped_names, unreadable_dirs}, probe, read, is_dir}` (`is_dir` provided: `false`) lets `spec check --staged` feed staged blobs. `WorkingTree::new(root, &Paths)` canonicalises the root.
- `SqliteIndex::open(db, project, root)`: a handle on one worktree `(project, canonical root)`; a DB holds several. Inherent: `root()`, `project()`, `settings() -> DbSettings` (PRAGMAs, `fts5`, `sqlite_version`), `check_fts()`, `dump()`, `dump_worktree()`. `Send`, not `Sync`.
- `trait IndexWriter {update, update_paths, rebuild}` → `UpdateReport {walked, parsed, unchanged, removed, unreadable, reparsed_all, missing_roots, skipped_names, unreadable_dirs}`. `update` walks all; `update_paths` (the Phase 2 watcher) probes each named path by the walk rules (unlisted → deleted; a directory gone from disk re-probes its stored files); an existing directory (`is_dir`) or a non-clean path (`""`, `docs/`, `./x.md`, absolute) walks, as do no prior index and a stamp or fingerprint change; a `[paths]` change needs `update`. `rebuild` = `spec index --full`.
- `trait SpecIndex {files, file -> IndexedFile {parsed?, blake3?, size, read_error?}, lookup_id -> [IdHit {path, ord, node}], search(&SearchQuery {text, kinds, limit}) -> SearchResults {hits: [SearchHit {path, ord, id?, kind?, title?, snippet}], short_query}}`; `lookup_id`: every node with exactly that `id`.
- `spec check`, no database (`docs/canon/spec-check.md`): `check_input(&dyn Source, &IdScheme) -> CheckInput` (a parser panic → a read error), `check_worktree(root, config, baseline?, today) -> Report` (all tables from `config`; no baseline passed → `BASELINE_FILE` `.spec-debt.toml` at the root if present; no root, a bad config or baseline → `cannot-check`, unwalked), `today_utc()`.
- `StoreError {DbInsideWorktree, DbDirMissing, NotIndexed, RootMismatch, Busy, Io {path, source}, Sqlite(String)}`; `INDEX_FORMAT = 5`; `SEARCH_LIMIT_{MIN,MAX,DEFAULT}` 1, 200, 20; `MIN_TERM_CHARS = 3`.

## Rows

Keyed `(worktree, path, ord)`, `ord` = position in `ParsedFile.{nodes, links, anchors, diagnostics}` (node 0: the document). No key on `id`: absent, repeated, feature-scoped IDs stored as parsed (uniqueness: `spec check`); repeated citations and `R-12@3`/`R-12@4` are separate rows; a mention without `src` stays. `dst`, `parent`, aliases, anchors as written (a resolved row would go stale). NULL `parent_id` = the document contains the node. Query columns + the model value as JSON: `file(path)` rebuilds the `ParsedFile`.

Tables (`schema.rs`), `STRICT`, no `CHECK`: `index_meta` (`format`); `worktrees (wt, project, root, scheme_fp)`, unique `(project, root)`; `files (file_id, wt, path, blake3?, size, read_error?, shell?)`, unique `(wt, path)`; per file, keyed `(file_id, ord)`, cascading: `nodes (node_id, id?, kind?, title?, parent_id?, own_text, node)`, `links (src?, type, dst_id?, dst_path?, link)`, `anchors (name, anchor)` (`slug`, `attr`, `html`), `diagnostics (code, diagnostic)`; `aliases (node_id, ord, alias)`; lookup columns indexed. `nodes_fts`: FTS5 over `id, title, own_text`, external content `nodes`, rowid `node_id` (never ordered by), `tokenize='trigram case_sensitive 0'`, kept by insert, delete, update triggers. `own_text` = the body minus the ID sections inside it, joined by `\n`.

**Canonical dump** (`dump.rs`): every table in column order, `wt`, `file_id`, `node_id` replaced by `(project, root)`, `path`, `(path, ord)`, one sorted line per row `<table>\t<JSON array>`, plus the FTS5 vocabulary (`fts5vocab` `row`; `instance` over its nodes for `dump_worktree`).

## Cache key and format stamp

Re-parse iff `(path, BLAKE3(bytes))` differs from the row, or the worktree's fingerprint (BLAKE3 of the JSON of `IdScheme::prefixes()`; an `[ids]` comment or another table leaves it) or the DB's stamp changed. Stamp = `INDEX_FORMAT` in `index_meta` (`user_version` is for Phase 2's `rusqlite_migration`). WHEN it differs, the first write recreates the tables in its transaction (other worktrees: `NotIndexed` until updated), dropping only the index's objects (append-only lists in `schema.rs`): Phase 2 tables survive. `tests/format_history.txt`: `<INDEX_FORMAT> <BLAKE3 of the spec-a + spec-b dump, root masked>`; a changed dump needs a new line and number (reasons: `INDEX_FORMAT`'s doc comment; 5: file links as `dst_path` rows, no schema change).

## Walk

`[paths]` and the matcher, core's `WalkScope` (also the link check's): core README. Regular files ending exactly in `.md` under the roots, minus `exclude`; below a root, symlinks and `.`-names skipped (a dot-name written in a root is kept), `probe`, `read` refuse any symlink component; no `.gitignore`; non-UTF-8 names skipped and counted. A root's components match by exact name: NFC over an NFD directory, or a symlink on its path → `missing_roots`. Paths: root-relative, `/`, as the OS lists them, byte-sorted, repeats dropped. An unlistable directory → `unreadable_dirs`, its files absent.

## Writes, reads, search

- Parse outside any transaction against a snapshot of stored hashes; one `Immediate` transaction re-checks and applies (a file whose row moved is re-parsed in place); the next update catches a concurrent edit. A rebuild swaps the worktree's rows in one transaction, never seen empty. A read = one deferred transaction.
- A changed file's rows are deleted and re-inserted: `INSERT OR REPLACE` on `nodes` fires no `nodes_fts_delete`, leaving the FTS malformed (on `files`, safe only through the `foreign_keys` cascade).
- Nothing is fatal (ADR-0012): a non-UTF-8 file, broken front-matter or YAML keeps its row and diagnostics; an unreadable file or a caught parser panic → `blake3` NULL, `read_error`, `size` 0, re-read every update.
- Search: whitespace splits the query; each term of ≥ 3 characters becomes a quoted FTS5 string (`"` doubled), never syntax; ANDed; shorter terms dropped, none left → no hits, `short_query`. Trigram matches substrings (`R-12` → also `R-123`; exact IDs: `lookup_id`), case-folded, no stemmer. The handle's worktree, `kinds` (empty = any), `ORDER BY bm25(nodes_fts, 10.0, 5.0, 1.0), path, ord`, `limit` clamped, a 64-token `snippet` marked `**`. No order depends on rowid; bm25 statistics are DB-wide, so ranks shift when another worktree changes.

## Connection, location, concurrency

At creation, before the first table: `journal_mode=WAL`, `auto_vacuum=INCREMENTAL`; per connection `busy_timeout=5000`, `foreign_keys=ON`, `synchronous=NORMAL`, `journal_size_limit=67108864`, `trusted_schema=OFF` (FTS5 is `SQLITE_VTAB_INNOCUOUS`), `recursive_triggers=OFF`. `open` gives `DbInsideWorktree`, creating nothing, when the DB's canonical parent or file is inside the worktree; no parent → `DbDirMissing`. Until the Phase 2 daemon is the sole writer (05 §1 principle 6), handles in any processes write directly: WAL, `Immediate`, `busy_timeout` (`BUSY`/`LOCKED` → `Busy`), no lock file or global state. Index updates are derived data, not `events`.

## Open owner questions

Working answer (the code) → what the other answer triggers.

- Q1 the pins above; transitive crates pending (04 §6). Other pins → `build_graph.rs` changes.
- Q2 `trigram case_sensitive 0` (+ `remove_diacritics 1` only if `fts.rs` stays green), exact IDs outside FTS → `unicode61`: prefix terms, substring and stem tests rewritten; a stemmer by `[project] language`: an ADR on `#universal`.
- Q3 role keys + `roots` + `exclude` → role keys only: spec-b rewritten, READMEs outside `docs/` unreachable (08 §4.1); `roots` only: role keys leave the walk.
- Q4 `norm_hash`, 08 AC-13, `spec bump`: own increment after the tracey reading (Q5, outside the pipeline), before Phase 2 → now: Q5 blocks it.
- Q6 a library + `specengine-eval index` → `spec index` now: `specengine-cli` comes with Q7.
- Q7 (CLI) `~/Library/Application Support/specengine/<slug>.db`, `[project] slug` required, written by `spec init`; one DB per project recording its root, refusing another; 05 §1 vs §8 DB name settled then → one DB per worktree.

## Open minors

- `entry_kind` re-reads a directory per component per probe (`is_dir`: twice per named clean path) → the Phase 2 watcher.
- A pre-existing non-WAL DB with other tables keeps its rollback journal → Phase 2.
- A broken escalation invariant or JSON encode → `Sqlite` (the `StoreError` taxonomy: the CLI increment).
- `read` checks its components, then reads: a race, as `probe` → `read` (accepted).

Tests: `tests/`, by the criteria of `docs/features/spec-index.md`, `spec-check.md` (`check_*.rs`); scratch corpora in temp dirs.
