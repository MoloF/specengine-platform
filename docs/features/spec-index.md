---
class: spec
status: shipped
scope: [crates/specengine-store]
ref: 08-roadmap.md Phase 1, second bullet
shipped: 2026-09-29
adrs: []
---

# Spec index: SQLite + FTS5, incremental

## Why

The context cost of a task must not grow with the project (`CLAUDE.md`): agents get `search`, `get_node`, `get_tree` from an index, not grep (07 §1.2). The parser reads one file at a time; without an index every remaining Phase 1 piece (`spec check`, CLI, MCP stdio, pilot W) re-parses the corpus per call. The index is derived data (ADR-0001, ADR-0003), a rebuildable projection of `specengine_core::parse` over one worktree with one hard property: **after any sequence of edits the incremental index equals a fresh rebuild, row for row.** Hence the key is exactly the inputs of `parse`, nothing is resolved, and keys hold ID-less documents, duplicate and scoped IDs, broken files.

## Acceptance criteria

Tests in `crates/specengine-store/tests/` unless named; scratch corpora in temp directories, never in `fixtures/`; each named mutation turns its test red.

- [x] AC-01 `specengine-eval` `tests/build_graph.rs`: eight default members; `-model`/`-core` normal graphs without `rusqlite`, `libsqlite3-sys`, `specengine-store`; no `sqlx`; `-store` on no `specengine-{code,import,mcp,eval,ra}`. Mutation: `rusqlite` in `-core`, or `-store` not default.
- [x] AC-02 `build_graph.rs`: `rusqlite =0.40.2` + `bundled` in `[workspace.dependencies]`; one version each of `rusqlite`, `libsqlite3-sys`, `blake3`. Mutation: `"0.40"`.
- [x] AC-03 `schema.rs`: a reopened connection reads every PRAGMA back (`wal`, 2, 5000, 1, 1, 67108864, 0, 0) and `ENABLE_FTS5` compiled in. Mutation: `auto_vacuum` after the first table → 0.
- [x] AC-04 `location.rs`: a DB inside the worktree — direct, via `..`, via a symlinked directory — gives `DbInsideWorktree`, no file; `git status --porcelain -- fixtures/` empty. Mutation: no guard.
- [x] AC-05 `projection.rs`: every file of spec-a and spec-b: `file(path)` equals `parse(path, bytes, scheme)`, BLAKE3, size; a scratch copy adds an ID-less document with a top-level section and a mention outside ID sections. Mutation: skip nodes without `id` or links without `src`.
- [x] AC-06 `projection.rs`: an ID defined in two files and twice in a third: `Ok`, `lookup_id` gives all four by `(path, ord)`. Mutation: `UNIQUE` on the ID.
- [x] AC-07 `incremental.rs`, scratch spec-a, in turn: edit a section, add, delete, rename keeping bytes, touch mtime, add then remove an `[ids]` prefix, drop a root; after each step the dump equals a fresh rebuild's. Mutations: keep a deleted file's rows; no fingerprint in the key (at `[ids]`); rows keyed by hash without path (at the rename).
- [x] AC-08 `incremental.rs`: `parsed` 0 after no change, an mtime touch, a `[budgets]` edit, an `[ids]` comment; 1 after one edit; `update_paths` of it gives the walk's dump. Mutations: skip the BLAKE3 comparison; fingerprint the `specengine.toml` bytes.
- [x] AC-09 `format.rs`: a rewritten stored stamp → the next update re-parses all (`reparsed_all`), dump equals a rebuild; `format_history.txt` ends with the current `(INDEX_FORMAT, dump hash)`, no earlier line has its number with another hash. Mutations: ignore the stamp; drop `classes` from the serialised `node` with no new line (red because the spec-a/spec-b fixtures carry a heading class; a dropped field the fixtures never exercise would not change the dump).
- [x] AC-10 `fts.rs`: a word removed by an edit, or only in a deleted file, has no hit; after the AC-07 script FTS5 `integrity-check` with rank 1 passes. Mutations: no delete trigger; `INSERT OR REPLACE` on `nodes` in place by `(file_id, ord)`, keeping the file row → `integrity-check … database disk image is malformed`, ties differ. (REPLACE on `files` is not a mutation: with `foreign_keys=ON` it cascades into ordinary `nodes` deletes, which fire `nodes_fts_delete`.)
- [x] AC-11 `fts.rs`: a word of `fixtures/spec-b/docs/spec/cli.md` line 23 found nowhere else → one hit, `FLAG-DRY-RUN`. Mutation: index whole spans → 3 hits.
- [x] AC-12 `fts.rs`, queries from that file or as `\u{…}`: a capitalised Cyrillic word found by its lower-case form; the stem `\u{043a}\u{043e}\u{043f}\u{0438}` hits `MOD-CLI` (line 13) and `CMD-SYNC` (line 18). Mutations: `tokenize='ascii'`; `unicode61` (stem misses).
- [x] AC-13 `fts.rs`: `R-12`, `RULE-STAM`, `stamina::regen`, queries with `"`, `(`, `*`, `NOT`, and a two-character one return `Ok`; `RULE-STAM` finds `RULE-STAM-REGEN`; the short one sets `short_query`. Mutation: raw query to `MATCH`.
- [x] AC-14 `fts.rs`: two nodes of identical text in different files tie on rank; hits are identical for a fresh index, the AC-07 path and a reversed walk. Mutation: `ORDER BY rank` alone.
- [x] AC-15 `broken.rs`: a non-UTF-8 file, an unclosed front-matter, a YAML error, a mode-000 file: `Ok`, all other nodes present, each broken file with its row, BLAKE3 (NULL + `read_error` if unreadable), diagnostics. Mutation: `?` on an error diagnostic.
- [x] AC-16 `walk.rs`: spec-a by the defaults, spec-b by `roots`; outside the roots, excluded, dot-directory, dot-file, symlink, non-`.md`: not indexed; a single-file root: indexed; an unknown key, a `..` or absolute root → an error with its line. Mutation: walk the whole root.
- [x] AC-17 `concurrency.rs` (≤ 60 s), a handle per thread: two writers × 20 edit-and-update rounds, a rebuilder, a reader looping `search` and `files()`; no error reaches a caller; `files()` never shrinks after the first index; after the join one update gives a rebuild's dump. Mutations: `Deferred` writes; a rebuild committing its delete separately.
- [x] AC-18 `worktrees.rs`: two scratch copies in one DB; updating or rebuilding one leaves the other's dump unchanged. Mutation: delete by project only.
- [x] AC-19 `api.rs`: no `pub` item in the store's `src` names `rusqlite`; tests use only `SpecIndex`/`IndexWriter`. Mutation: a public `&rusqlite::Connection` accessor.
- [x] AC-20 `genre.rs`: spec-a and spec-b pass the same tests; no prefix, kind or fixture path literal in `src`. Mutation: special-case `"RULE-"`.
- [x] AC-21 `specengine-eval` `tests/index_cli.rs`: one envelope, `one_file_parsed` 1, no path or ID; `--out` under the corpus, or an unknown `[paths]` key (`file:line: message`), exits 2 with nothing under `--out`; `git status -- fixtures/` clean. Fixture runs met. Pilot runs are owner-run `#[ignore]` tests, skipped until the out-of-repo schemes get `[paths]`; 08 AC-10 (`full_ms` ≤ 10 000, `one_file_ms` ≤ 200) awaits them.
- [x] AC-22 `cargo nextest run -p specengine-model -p specengine-core` green; parser AC-02 (no `std::fs`) holds.
- [x] AC-23 Docs check green; worst W 118 068 B ≤ 118 105 B at shipping; `anonymity.rs` green.

## Implementation

One iteration; `cargo nextest run --workspace` 389 passed, 13 skipped; clippy, fmt, docs check clean; review accepted (no blocker or major). Built: `specengine-store` (`source` walk + `glob`, `schema` + format stamp, `rows` projection, `write` plan/parse/apply, `read`, `search`, `dump`, `index` handle, `error`), `specengine-core` `paths_toml` (`[paths]`), `specengine-eval index`; `rusqlite =0.40.2` in `[workspace.dependencies]`. Verified at adoption: `libsqlite3-sys` 0.38.2 builds SQLite 3.53.2 with `-DSQLITE_ENABLE_FTS5`; trigram `case_sensitive 0|1`, `remove_diacritics 0|1|2` (only with 0); FTS5 is `SQLITE_VTAB_INNOCUOUS`, so its triggers run under `trusted_schema=OFF`; `fts5vocab` is DIRECTONLY, so the dump creates it at top level in the temp schema; the FTS table is the search's outer loop (`EXPLAIN QUERY PLAN`).

Deviations, reviewed as acceptable and folded into `crates/specengine-store/README.md` as current behaviour: `serde_json` `float_roundtrip`; `unreadable_dirs` (files in an unlistable directory are absent); `update_paths` escalation rules; `short_query` only when no term is left, `limit` clamped, default 20; recreation drops only the index's objects; a caught parser panic stored like an unreadable file, `size` 0; the inherent `SqliteIndex` methods; the exact-name root lookup, symlinked roots missing, dot-names in a configured root allowed, repeats dropped; `specengine-eval index` extras (`unreadable`, `missing_roots`, `noop_parsed`, `one_path_ms`, `reports.json`).

Open, beyond the store README's owner questions Q1–Q7 and minors:

- The two review minors are for the owner: `float_roundtrip` onto the workspace entry, and `update_paths` escalating for a directory or non-clean path (with its first caller).
- `docs/canon/architecture.md#distribution` says changes go through the `events` log; the index reads it as operational changes, index updates being derived data. The Phase 2 ADR introducing `events` should sharpen the canon wording.
- The `canon:` anchors of this repository are `<a id>` tags the parser never emits, so running `xtask docs check` 3 over the index needs a parser change.
- For `spec check`: a `parent` written as an alias resolves only through stored aliases (`ParentRef` drops `alias_of`); tokens vs bytes (R7) stays with the ADR due before it.
- Untested: a caught parser panic (no known panicking input); non-UTF-8 file names (APFS refuses them). Not planned: `.gitignore`, non-Markdown sources, merging the xtask, census and `eval parse` walks, sharing parses across worktrees.
