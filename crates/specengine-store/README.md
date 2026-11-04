---
class: canon
tier: 1
scope: [crates/specengine-store]
owner: owner
reviewed: 2026-11-04
---

# specengine-store -- the spec index

A rebuildable SQLite + FTS5 projection of `specengine_core::parse` and of core's edges (`spec-index-edges.md`) over one worktree, updated incrementally. The hard property: **after any sequence of edits the incremental index equals a fresh rebuild, row for row** (equal canonical dumps). The DB, outside the repository (ADR-0001, ADR-0003), also holds the code index (`symbol-index.md`) and the queue (not derived). A default member on `-model`, `-core`, `rusqlite` (FTS5), `blake3`, `serde_json` (pins: `stack.md`). Callers: the CLI (a freshness step per read), eval.

## API

No `rusqlite` type in a public signature (`architecture.md#distribution`).

- `trait Source {root, paths, list -> Listing {paths, missing_roots, skipped_names, unreadable_dirs}, probe, probe_paths -> [Probed], read, stat -> StatKey, is_dir}`: `WorkingTree::new(root, &Paths)` canonicalises the root; `GitIndex::open(root, &Paths, &GitEnv)` (checks only): staged regular blobs by the walk rules, read up front.
- `SqliteIndex::open(db, project, root)`: a handle on a worktree `(project, canonical root)`, several per DB. Inherent: `root()`, `project()`, `settings() -> DbSettings`, `check_fts()`, `dump()`, `dump_worktree()`, `stored_edges()` (tests), `snapshot()` -> `IndexSnapshot` (`spec-read-subset.md`), `trusted_notes`, `node_norms()` (`spec-trace.md`). `Send`, not `Sync`.
- `trait IndexWriter {update, update_paths, rebuild}` -> `UpdateReport {walked, parsed, unchanged, removed, unreadable, reparsed_all, resolved, resolved_all, missing_roots, skipped_names, unreadable_dirs, hashed, committed}`. `update_paths` (for the watcher) probes the named paths, or walks (`spec-index-edges.md` "`[paths]` fingerprint and probes").
- `trait SpecIndex {files, file -> IndexedFile {parsed?, blake3?, size, read_error?}, lookup_id -> [IdHit {path, ord, node}], search(&SearchQuery {text, kinds, limit, archive}) -> SearchResults {hits: [SearchHit {path, ord, id?, kind?, title?, line, tier3, snippet, snippet_parts}], short_query, tier3_left_out}, indexed_input -> CheckInput}`; `indexed_input`: the stored worktree, one snapshot, empty `bytes`.
- `spec check`, no database (`spec-check-{cli,git}.md`): `load_config(&NamedBytes {name, bytes})` -> `(ProjectConfig, CheckConfig)`; `load_check` (+ the baseline) -> `CheckSetup` (`.team`: `hub-team.md`), else a `cannot-check` `Box<Report>` naming only `name`; `default_baseline(root)`; `check_input(&dyn Source, &IdScheme)` (a parser panic -> a read error), `check_source`, `check_tree`, `today_utc()`. Wrappers: `check_worktree(root, config, baseline?, today)`; `check_staged_with_notes(root, GivenFile?, GivenFile?, &GitEnv, today) -> StagedCheck {report, notes}`, `check_changed_with_notes` (against `HEAD`: `spec-check-git.md`, `rev-not-raised` inside), `check_staged` (the report); `bump_{staged,changed}` -> `BumpCheck {report, rows}`. `GitEnv::new(cwd, vars)`.
- `StoreError {DbInsideWorktree, DbDirMissing, NotIndexed, RootMismatch, Busy, Io {path, source}, Sqlite(String)}`; `INDEX_FORMAT = 17`; `SEARCH_LIMIT_{MIN,MAX,DEFAULT}` 1, 200, 20; `MIN_TERM_CHARS = 3`.
- Code index (`symbol-{index,reads}.md` seams): `CodeAnalyzer`, `{update,clear}_code`, `stored_names`, `code_{refs,symbols,snapshot{,_with_roles,_with_layer_c},roles,selection}`, `{lookup,file}_symbols`, `LookupWithFiles` (`spec-impact.md`); `StoredRole`, `RoleCandidate`, `CodeSnapshot.roles` (`code-roles.md`); `CodeSelection {refs, files}`, `SelectedFile {path, names, holds, symbols}` (`spec-verify-scope.md`); layer C: `replace_layer_c`, `layer_c_{stamp,snapshot}`, `code_tree_rows` (`layer-c{,-reads}.md`).
- `b3_hash(&[u8]) -> String`, streaming `B3Stream {new, update, finish}` (`node-revision.md` "Where it appears"): `bundle_hash`, `span_hash`, `patch_hash`, `norm_hash`.
- Proposals (`{proposal-*,queue-backup,agent-intake,decision-record,decision-staging,spec-ids}.md`, schemas `proposal-queue.md` "Store"; tasks `tasks.md`, cards `{cards,spec-trace}.md`, the gate `gate.md`): `ProposalQueue`, `SqliteQueue::open(db, project)`, ops named in those canons, `stage_as`, `unstage_as`, `record_verify_tests` (`spec-verify-tests.md`); `events_after(after, limit) -> EventsAfter {events: [TailEvent], last_seq, full}` (`user_version` checked); `WorktreeGit::new(dir, &GitEnv)` (`git.rs`, the check's): methods in those canons, `spec-lock.md`, `spec-verify-tests.md` (`status_all`) and `proposal-outputs.md` (`is_skip_worktree`, `local_blob_at`, `disk_name` -> `DiskName`; `outputs.rs` `committed_tree`); `floor.rs` `read_floor`, `id_side`, `store_floor` (`queue-backup.md`); children's environment, deadlines: `hub-git.md`, `gate.md`; `STDERR_MAX` (4096) public; `sync.rs` `SyncGit` (`changed` -> `ChangedPaths`), the only `fetch` and `push`, and `sparse.rs` `sparse_set` (`hub-git.md`); `spec verify`'s `VerifyGit` (`spec-verify.md` "Base"; `list_revision` (`impact --since`): `spec-impact.md` "Since"); the file writes, `same_repository`, `introduced_findings`: those canons.

## Rows

Keyed `(worktree, path, ord)`, `ord` = position in `ParsedFile.{nodes, links, anchors, diagnostics}` (node 0: the document). No key on `id`: absent, repeated, feature-scoped IDs stored as parsed (uniqueness: `spec check`); repeated citations and `R-12@3`/`R-12@4` are separate rows; a mention without `src` stays. `dst`, `parent`, aliases, anchors as written. NULL `parent_id` = the document contains the node. Query columns + the model value as JSON: `file(path)` rebuilds the `ParsedFile`.

Tables (columns: `schema.rs`), `STRICT`, no `CHECK`: `index_meta`; `worktrees` (+ `scheme_fp`, `paths_fp`), unique `(project, root)`; `files`, unique `(wt, path)`; `file_stats`, `walk_notes`; per file, keyed `(file_id, ord)`, cascading: `nodes`, `links`, `anchors` (`slug`, `attr`, `html`), `diagnostics`; `aliases`; lookup columns indexed; edge tables; the code tables (`symbol-index.md` "Tables"), layer C's (`layer-c.md`). `nodes_fts`, `own_text`: `spec-cli-read.md` "Search index".

**Canonical dump** (`dump.rs`): every table but those two (`pragma_table_xinfo` on `main`), surrogate keys replaced by `(project, root)`, `path`, `(path, ord)`, one sorted line per row `<table>\t<JSON array>`, plus `fts5vocab`.

## Cache key and format stamp

`update` reads a file only when its stat key moved or is racy (`spec-read-freshness.md`). Re-parse iff `(path, BLAKE3(bytes))` differs from the row, or the worktree's fingerprint (BLAKE3 of the JSON of `IdScheme::prefixes()`; an `[ids]` comment or another table leaves it) or the DB's stamp changed; `paths_fp` re-resolves edges only. Stamp = `INDEX_FORMAT` in `index_meta` (`user_version` is the queue's own schema step). When it differs, the first write recreates the tables in its transaction (other worktrees: `NotIndexed` until updated), dropping only the index's objects (`schema.rs` lists that never remove one): the queue's tables survive. `tests/format_history.txt` pins each stamp's dump (`symbol-index.md` "Analyzer stamp").

## Walk

`[paths]`, the matcher, core's `WalkScope`: core README. Regular files ending exactly in `.md` under the roots, minus `exclude`; below a root, symlinks and `.`-names skipped (a dot-name root is kept), `probe`, `read` refuse any symlink component; no `.gitignore`; non-UTF-8 names skipped and counted. A root's components match by exact name: NFC over an NFD directory, or a symlink on its path -> `missing_roots`. Paths: root-relative, `/`, as the OS lists them, byte-sorted, repeats dropped. An unlistable directory, also one on the way to a root (`""`: the root), -> `unreadable_dirs`, never `missing_roots`; its files absent.

## Writes, reads, search

- Parse outside any transaction against a snapshot of stored hashes; one `Immediate` transaction re-checks and applies (a file whose row moved is re-parsed in place); the next update catches a concurrent edit; a no-op opens none. A rebuild swaps the worktree's rows in one transaction, never seen empty. A read: one deferred transaction.
- A changed file's rows are deleted, re-inserted, never `INSERT OR REPLACE` on `nodes` (no `nodes_fts_delete` fires: a malformed FTS; on `files` safe only by the cascade).
- Nothing is fatal (ADR-0012): a non-UTF-8 file, broken front-matter or YAML keeps its row and diagnostics; an unreadable file or a caught parser panic -> `blake3` NULL, `read_error`, `size` 0, re-read every update.
- Search, the query and its order: `spec-cli-read.md` "Search index".

## Connection, location, concurrency

At creation, before the first table: `journal_mode=WAL`, `auto_vacuum=INCREMENTAL`; per connection `busy_timeout=5000`, `foreign_keys=ON`, `synchronous=NORMAL` (the queue's `FULL`), `journal_size_limit` 64 MiB, `trusted_schema=OFF` (FTS5 is innocuous), `recursive_triggers=OFF`. `open` gives `DbInsideWorktree`, creating nothing, when the DB's canonical parent or file is inside the worktree; no parent -> `DbDirMissing`. Until the daemon is the sole writer (05 s1 principle 6), handles in any process write directly: WAL, `Immediate`, `busy_timeout` (`BUSY`/`LOCKED` -> `Busy`), no lock file. Index updates are derived, not `events`.

## Open owner questions

Working answer -> the other's cost.

- Q1 the pins above, transitive ones pending (04 s6) -> `build_graph.rs` changes.
- Q2 `trigram case_sensitive 0` (`remove_diacritics 1` if `fts.rs` stays green), exact IDs outside FTS -> `unicode61`: prefix, substring, stem tests rewritten; a stemmer by `[project] language`: an ADR on `#universal`.
- Q3 role keys + `roots` + `exclude` -> role keys only: spec-b rewritten, READMEs outside `docs/` unreachable (08 s4.1); `roots` only: role keys leave the walk.

## Open minors

- -> the watcher: `update_paths` silently drops rows under an unreadable directory.
- Accepted: a pre-existing non-WAL DB with other tables keeps its rollback journal; a broken escalation invariant or JSON encode -> `Sqlite`; races (`read` checks components, then reads; `resolve` names the parent when an entry's type cannot be read).
- `GitIndex` holds each blob twice (`read` returns owned bytes); untested: `RootGone` by a spawn race, a file-system boundary.
