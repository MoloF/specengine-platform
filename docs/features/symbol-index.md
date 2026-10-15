---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 6a (WA-1..WA-8, WA-11..WA-15, WA-21, WA-22; Q1-Q10), refreshed and split 2026-10-13 (6a-2; Q11-Q18), refreshed 2026-10-15 against shipped 4b-1, 4c-1 and the drafts 4b-2, 4c-2, 5a-1, 5a-1b, 5a-2 (Q21, Q23-Q27); 05 s3.3, s5.1, s5.2; 07 s2, s5; 08 s2, AC-10
adrs: []
---

# symbol-index: stored code rows, `refs` from them, AC-10

## Why

Every code read re-parses the tree (`docs/canon/code-bindings.md` "The walk", "Known limits"); nothing cheaply says which item sits at `path:line`, which node binds it, where code has no spec, or a signature (05 s5.1): agents read files, and a task's cost grows with the project.

Slice 6 is split (Q1): 6a, 6b (6b-1 `code-tools`, 6b-2 `verify-node`: `code-tools.md` Q28), 6c `code-index-speed`; 6a again (Q12): 6a-1 `symbol-discriminator.md` (shipped 2026-10-16: the discriminator, the signature entry, canon `symbol-names.md`) and **6a-2** (this): slice 1's per-file results and 5a-1's level digests stored beside the md index, row for row equal to a fresh rebuild, serving `refs`, 6a-3's reads, 7a and AC-10's code half; 6a-2 again (Q23): this, every table, the refresh, `spec index`, `refs` from the index, names for 6b, AC-10; **6a-3** `symbol-reads.md`, lookup tiers, `spec symbols`, `spec unmapped`. On shipped slices 1-4c-1; after 5a-1 (`item_levels`, `LevelState`, `level_header()`), 5a-2 (its code; its pilot `cost` plans 6c whenever it comes, never a gate: `hash-levels-eval.md` Q4) and 6a-1 (the discriminator, `item_signatures`, `name_files`' halves: its Q21); listed before 5b (08 s2), though neither needs the other (Q11's reasons are 6a-1's). "6a" in other drafts names all its parts.

No ADR: ADR-0001, -0003, -0008, -0016, -0020 and -0046..-0050 applied (`architecture-code.md#sync-in-reads`: these four reads walk code; `#health`: never this index; C joins rows by file and start byte; digests move only with `LEVEL_RECIPE`, in `code_fp`). Only Q2's cargo runner would need one.

## Description and interactions

- **Freshness** (WA-6): `refs`, `index` (and 6a-3's `symbols`, `unmapped`) refresh the md index, then the code index, a call of its own, never inside CLI `refresh.rs` `refresh` or `corpus.rs` `indexed`, which `verify` (`--changed`; 4b-2's `--staged`), `lock accept`, 4c-1's `spec propose drift` and a drift item's approve, `show` and 9a/9b's Health rows use, keeping their fresh `read_tree` or a commit's blobs (`#health`; `spec-verify.md` "Cascade", "Base": `read_commit`, `tree_of_blobs` over `CodeTree::empty()`): the index holds the working tree only. 4b-2's `spec bump` reads no code. `show` without `--bindings`, `search`, `graph` never walk code. No MCP tool or daemon route refreshes code here: 6b-1, 6b-2, 7a add theirs (6b-2's `verify --node` the one verify that refreshes: `verify-node.md`).
- **Crates** (WA-5, WA-22): the store owns compare, apply and the cross-file pass behind its trait `CodeAnalyzer` (list, packages, fingerprint, analyze; plain rows), implemented in the CLI library over `code_walk.rs` and `specengine-code`, which the store may not depend on (`build_graph.rs` `core_and_store_have_no_tree_sitter`); the eval via its CLI edge, MCP and the daemon via the CLI. No `rusqlite` type public; no new dependency (6c aside).
- **Seams** (built; SW refinement): one listing, `code_walk.rs` `list`/`walk`, for `read_tree` and the refresh; one parse per re-derived `.rs` file for `bind_tree`, `name_files`' per-file half (shipped by 6a-1, its Q21), `item_levels`, `item_signatures` (each one collect walk over that tree), later 10a's detector; `comment_anchors` once per `.ron` file. The store pass recomputes `name_files`' tree-wide halves (`tree_passes`) over stored rows as set operations: `path_targets` as `declared_by`, `duplicate` as `twin_files`, which counts every file, unrooted and `#[path]` targets included, as `mark_duplicates` does (`docs/canon/symbol-names.md` "Ambiguity"). `name_file`'s result depends on the package table (`role`, `shared`, the module), so its cache key includes it (`targets_fp`: "Cache key and cross-file pass"). `code_fp`'s `[ids]` part is the md writer's `fingerprint` (store `write.rs`), re-checked in the transaction as there. Reused: `Corpus::judge` (`refs.rs`) once per stored ID, `OUTPUT_CAP_CHARS`.
- **Oracle**: `refs.rs` `collect_rust`/`collect_ron` over a `CodeTree` stay as RD-8's (9a lifts them).
- **6b-2's partial tree** (Q17): the store answers per file its module qpath and each item's qpath and ambiguity, so `verify --node` (`verify-node.md`) walks no tree; it calls the refresh as one named CLI-library function (working name `refresh_code`).

## Data

### Tables

WA-1, WA-2, WA-7; `code_decls`, `code_worktrees`, `role`, the pass's facts: SW refinement. Per-slug DB, keyed by worktree; created with the schema in every DB; in `FORMAT_TABLES` children first, inserted before `worktrees` (store `schema.rs`, whose "append-only" doc becomes "never removes one"); per-file rows `(file, ord)`, cascading; IDs, revs as written.

`INDEX_FORMAT` 9 (8 today, store `lib.rs`; 6a-2's alone: 4b-1, 4c-1 bumped `QUEUE_SCHEMA_VERSION` to 8, 4b-2 and 4c-2 keep both, 5a-1, 5a-1b, 5a-2 and 6a-1 touch no store; every table is created here, 6a-3 adds none (Q23); 10a takes 10); `format_history.txt` `9 <hash>` over `spec-a`, `spec-b` plus a stub analyzer's rows (the store has no dev-dependency on `specengine-code`); `fixtures/code-index`'s rows (Q24): "Analyzer stamp". `dump.rs` maps code keys to `path`, `(path, ord)`.

| Table | Key | Columns |
|---|---|---|
| `code_worktrees` | `wt` | `code_fp`; no row: no code indexed |
| `code_files` | `(wt, path)` | `lang`, `blake3`, `size`, `read_error`, `utf8`, `package` (its directory), `role`, `module` (qpath), `orphan_errors`, `targets_fp`; pass: `declared_by` |
| `code_packages` | `(wt, dir)` | `manifest_b3`, `source`, `targets` (JSON) |
| `symbols` | `(file, ord)`; `qpath`, `name` indexed | `item_kind`, `name` (bare last segment), `owner`, `qpath` (discriminator in), `discriminator`, `parent_ord`, `start_line`, `end_line`, `start_byte`, `end_byte`, `signature`, `level_state`, `level_categories`, `sig_hash`, `body_hash`, `deps_hash`, `skeleton_hash`, `quote_hash`, `body_bytes`, `layer`; pass: `twin_files` |
| `symbol_suffixes` | `(file, ord, depth)`; `suffix` indexed | `suffix` |
| `code_decls` | `(file, ord)` | `target` (a `#[path]` module's path), `line` |
| `code_markers` | `(file, ord)` | `relation`, `token`, `id`, `rev`, `levels`, `levels_state`, `note`, `line`, `column`, `binding`, `symbol_ord`, `anchor` (module qpath, RON path, category) |
| `code_mentions` | `(file, ord)` | `written`, `line`, `column`, `binding`, `symbol_ord`, `anchor` |

```
symbols          (counter.rs, n) impl_item | name "impl Tally" | qpath "crates/tally::counter::impl Tally #2" | discriminator "2" | 40-45 | signature "impl Tally" | hashed [] | sig_hash "b3:41aa…" … body_bytes 37
symbol_suffixes  (counter.rs, n) 1 "impl Tally #2" · 2 "counter::impl Tally #2" · 3 "crates/tally::counter::impl Tally #2"
```

- **Start, end**: the item node's, attributes excluded, as refs' `line` (`code-bindings.md` "Rust binding"); 11a joins on `(path, start_byte)`.
- **Levels** (Q11), every item, bound or not (7b compares all: `impact.md` "Since"; 5c's reads pair by fingerprints): `level_state` 5a-1's `LevelState` (`hash-levels.md` "Entry points"), `hashed` or `cannot_verify`, `level_categories` its `ErrorCategory` names (JSON, `[]` when hashed); digests `b3:` + 64 hex as 5b's keys (`lock-levels.md` "Format 2"), NULL when `cannot_verify`. No `ast_hash`; no module or RON digest (Q14).
- **Suffixes** (WA-8): `symbol_suffixes` rows come from the structured `QPath` segments (package, unit, module…, owner, name + discriminator), never string splitting (`<T as a::Tr>` is one segment): depth d = the last d joined by `::`; 6a-3's tiers read them (`symbol-reads.md` "Lookup").
- **Per read**: `id_state`, `rev_state`, coverage (6a-3), ambiguity (`path_attribute` if `declared_by` and the role is not shared, else `unrooted` by role, else `duplicate` if `twin_files` > 0). RON values are not symbols.
- **Twins across files**: the pass reads an undiscriminated qpath `u` as `qpath = u` (`discriminator` NULL) or `qpath` in [`u #`, `u $`) over the `qpath` index filtered by `discriminator IS NOT NULL AND qpath = u ‖ ' #' ‖ discriminator`, never a scan (RD-3), never by splitting a qpath: file and directory names can hold ` #` (`symbol-names.md` "Discriminator").

`layer` (`layer-c.md`, ADR-0049): the layer whose rules made the row and its qpath, `a` on every row; B (10b's role rows) and C (11a's `code_monikers`) annotate from their own tables, never changing it.

### Cache key and cross-file pass

WA-3, WA-4. A code file is re-derived when its `(path, BLAKE3)` changed; its package directory (a new `Cargo.toml` between the file and its package re-parents it: `code-bindings.md` "The walk"); that package's `targets_fp` (BLAKE3 of the target table: a manifest edit or a new `src/bin/x.rs` renames qpaths); or the worktree's `code_fp` in `code_worktrees`: BLAKE3 of `CODE_INDEX_FORMAT` (`specengine-code`, bumped with any rule here, in 6a-1 or slice 1), `level_header()` (5a-1), the parsed `[code]`, the `[ids]` fingerprint. Not mtime, not the blob OID.

Per-file rows are pure functions of (bytes, path, package table, `code_fp`). In the same Immediate transaction one pass rewrites `twin_files` per undiscriminated qpath and `declared_by` per path that a re-derived or removed file held or holds. No `[code]`: the worktree's code rows go. A stamp change or `--full` re-derives all; queue tables stay.

### Analyzer stamp

Q16. `CODE_INDEX_FORMAT` starts at 1. `crates/specengine-cli/tests/code_format_history.txt` holds `<CODE_INDEX_FORMAT> <code-index dump hash>` lines (`fixtures/code-index`, Q24), as the store's `format_history.txt`: a changed dump with the stamp kept is red.

### spec refs and spec index

WA-14, WA-6. `refs` reads the refreshed index, byte-identical to the walk path (6a-1's names in both). `spec index` prints `indexed <slug>: walked …, parsed …, unchanged …, removed …, unreadable …` then `db <path>` (CLI `refresh.rs`); 6a-2 adds before `db` `code <slug>: walked 41, parsed 41, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 230, markers 31, mentions 9` (`, rederived all`), JSON `code` (or `null`). A bad `[code]` (Q15): the md index is written and printed, then exit 2, `spec: <config>:<line>: …` naming the code cause; no code row changes. Store lenient (unreadable or no tree: a `read_error` row; non-UTF-8: `utf8` 0, a `note:`), `index` exits 0; reads exit 2 on any gap (`spec: the code walk is incomplete: <gap>`).

### AC-10 measurement

WA-21, Q10, Q13, Q25. `specengine-eval code-index` over a scratch copy of the `[code]` walk's files and manifests (`[code]` from the scheme; a pilot's roots from `SPECENGINE_CODE_ROOTS_A`/`_B`, as `code_refs_pilot.rs`): `kloc` (`.rs` newlines / 1000, 5a-2's `cost` alike), `rs_files`, `ron_files`, `bytes`, `items`, `hash_ms` (parse, levels, signatures; no DB), `full_ms`, `noop_ms` + `noop_parsed`, `one_file_ms` + `one_file_parsed` (a line appended to the largest `.rs`; the md harness takes the first: eval `index.rs`), `db_bytes`, `refs_ms` (information); from the index: `twins.{cfg, ordinal}` (in-file groups and items by rule), `twins.cross_file` (`duplicate` items), `signatures.{items, cut}`; `roles.{files, metadata, differ}`: each Rust file's `FileRole` under `layout_targets` against `cargo metadata`'s table (`ast_hash/targets.rs` `tables`), `differ` `null` when no package answered.

Judged in release (dev recorded), as 08 AC-10 states: `hash_ms` × 300 / `kloc` ≤ 2 000; `one_file_ms` ≤ 200; plus `noop_ms` ≤ 200, 7a's gate (`bundle-bindings.md` "Dependencies and ordering"); `full_ms` recorded only. 6c (Q26): 5a-2's pilot `cost.levels_ms_per_kloc` above 6.67 (2 000 ms / 300 kLOC; parse excluded, so a lower bound of `hash_ms`'s rate) plans it at once, at or below it decides nothing; then any failed figure here. 7a waits on `noop_ms` alone (else 6c first: `bundle-bindings.md` "Dependencies and ordering").

## Rules and edge cases

- WHEN any sequence of file, manifest, `[code]`, `[ids]` or stamp changes is indexed, the dump SHALL equal a fresh rebuild and `refs` from the index SHALL equal `refs` from the walk; WHEN reads run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical.
- Only the data directory is written; pilots `#[ignore]`, `--timeout`, a scratch HOME, unnamed.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax stays in `specengine-code`; no fixture prefix, kind, `::` split, stack word or Cargo literal in the store, core or new CLI modules, which join `code_refs_walk.rs` `NEW_MODULES` (`STACK_WORDS` holds `system`).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- tables. M: created lazily -> `index_without_code_creates_code_tables` (spec-a); absent from `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [ ] RD-2 -- cache key. M (each ignored): manifest -> `manifest_edit_renames_qpaths_as_a_rebuild`; package directory -> `new_manifest_between_reparents_files`; `[ids]` -> `ids_change_rescans_mentions`; `CODE_INDEX_FORMAT` -> `analyzer_format_change_rederives_every_code_file`; mtime -> `same_size_same_mtime_edit_is_reindexed`; `exclude` -> `code_exclude_change_drops_rows`. M: a signature rule changed, the stamp kept -> `code_format_history_pins_the_code_index_dump` (Q24).
- [ ] RD-3 -- the pass. M: per-file ambiguity -> `removing_a_twin_in_another_file_clears_duplicate`; `#[path]` ignored -> `path_attribute_follows_the_declaring_file`; a scan for cross-file twins -> `twin_pass_reads_the_qpath_index` (`EXPLAIN QUERY PLAN`).
- [ ] RD-4 -- injection. M: `specengine-code` in the store -> `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`; tree-sitter -> `core_and_store_have_no_tree_sitter`; rowid dumped -> `code_dump_is_independent_of_insertion_order`.
- RD-5 -- lookup: moved to `symbol-reads.md` RD-1 (Q23).
- RD-6 -- `symbols`: moved to `symbol-reads.md` RD-2 (Q23).
- RD-7 -- `unmapped`: moved to `symbol-reads.md` RD-3 (Q23).
- [ ] RD-8 -- `refs`. M: unrefreshed rows -> `refs_after_edit_equals_a_fresh_walk`; index ≠ walk -> `refs_from_index_equals_refs_from_the_walk`; a partial list -> `code_walk_gap_is_exit_2` stays green.
- [ ] RD-9 -- freshness. M: `show`, `search`, `graph` refresh code -> `show_search_graph_never_refresh_code`; verify, `verify --staged`, `propose drift` or `lock accept` refreshes it -> `verify_leaves_the_code_index_untouched` over the four.
- [ ] RD-10 -- M: a cache under the root -> `index_leaves_the_worktree_untouched` (`index`, `refs`); its `symbols`, `unmapped` half: `symbol-reads.md` RD-4 (Q23).
- [ ] RD-11 -- Q2 refused: `crates` joins `code_table_unknown_key_is_refused`'s cases (refused today by core `code_toml.rs` `NOT_BUILT_KEYS`: no code change), its `show`, `check` exit 0 kept. M: the key accepted.
- [ ] RD-12 -- harness. M: no edit before the one-file timing -> `one_file_parsed` 1; vacuous timings -> `items` = stored symbols; the first file edited -> the largest `.rs` is; Q25's fields: a cargo stub answering another table -> `roles_differ_counts_a_changed_role` (as `ast_hash_units.rs`), `differ` `null` with no answer; `twins`, `signatures` off -> `fixtures/code-index/expected.json`'s counts.
- [ ] RD-13 -- seams. M: a second listing -> `index_and_walk_list_alike` (over `code_refs_walk.rs`'s cases); a second parse -> `one_parse_per_rederived_file`; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`, which gains the analyzer (it hashes through `item_levels(`) (the pin names per file the entry it must call: `hash_bound_items(` for `lock.rs`, `item_levels(` for the analyzer; 5b's files: `lock-levels.md` Q16).
- [ ] RD-14 -- a bad `[code]` (Q15). M: the md index held back -> `bad_code_table_still_writes_the_md_index` (exit 2, cause named, md rows fresh).
- [ ] RD-15 -- names for 6b-2 (Q17). M: names from a tree walk -> `stored_names_equal_the_walks_for_one_file`.

test-engineer:

- [ ] TE-1 -- a new `fixtures/code-index/` (own `specengine.toml`, `expected.json`): `impl a::b::T for X`, cfg and ordinal twins, a nested inline mod with `//!`, `--at` lines in bodies, a bin and a test target, a `.ron` file, a broken item; `expected.json` covers each discriminated row, suffix rows, signature, level state (each `symbols` form, tier, unmapped row: `symbol-reads.md` TE-1). `fixtures/code-a` stays as 6a-1 leaves it (slices 1-4c-1's goldens); RD-8 reads it too.
- [ ] TE-2 -- seeded random file adds, removes, renames, edits, `Cargo.toml` (one between a file and its package too), roots/exclude, `[ids]`, stamp, CRLF, twins inserted, removed and `cfg`-edited (rule 1 <-> rule 2); after each step dump = rebuild and RD-8's refs = the walk's; each RD-2, RD-3 mutation red; bounded in steps and time.
- [ ] TE-3 -- the store's `9 <hash>` (spec-a, spec-b, the stub), the CLI's `1 <hash>` (code-index); a proposal, task, event, stage survive a stamp change and `--full`. Pins moved: `bundle_config.rs` `ac16_index_format_is_eight` (9, nine lines), store `format.rs`'s format-8 pin, the store README's format line, the `mcp_index.rs` header (says 7).
- [ ] TE-4 -- determinism (two roots, cwd, reverse creation, CRLF): `refs` (`symbols`, `unmapped`: `symbol-reads.md` TE-2).
- [ ] TE-5 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- [ ] TE-6 -- genre: on `spec-c` `refs` exits 2 naming `[code]`; `index` exits 0, no `code` line, JSON `code` `null`; the "Rules" scan (`symbols`, `unmapped`: `symbol-reads.md` TE-3).
- [ ] TE-7 -- build graph: store, core unchanged; `specengine-code` keeps three dependencies; CLI gains nothing outside 6c; MCP `TOOLS` 15.
- [ ] TE-8 -- pilots A and B, manual, run by the owner (`#[ignore]`, release and dev, foreground, `--timeout`, a scratch HOME, the child given `PATH`, the label's variables and `SPECENGINE_CARGO` as an absolute toolchain path, `rustup which cargo`: rustup's proxy finds no toolchain under a scratch HOME, and `roles` would compare layout with layout). Recorded in "Implementation", counts only: AC-10's figures; `twins`; `signatures`; mentions per root (`spec index`'s `code` line; `unmapped`: `symbol-reads.md` TE-4); `roles` (Q2's outcome, counted only with `roles.metadata` > 0); read-only proof (`git --no-optional-locks status`, a (path, kind, size, mtime) listing, before and after).

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13; 08 s2 lists 6a-1, 6a-2, 6b, (6c); Q11's neighbours amended (`lock-levels`, `bundle-bindings`, `code-tools`, `anchor-states`, `hash-levels`, `impact`); split again 2026-10-15 (Q23): `symbol-reads.md`, 08 s2 lists 6a-3, `code-tools`, `impact` pointers follow.
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-index.md` (≤ 12 288 B; with Q23, 6a-3 appends "Lookup", "spec symbols", "spec unmapped" if it holds, else its own file); `docs/index-canon.md` re-measured (7 398 B on 2026-10-15, + ≈ 200 B; over 10 240 B the shard splits: `docs/README.md` "Generated"); store, CLI, eval READMEs (10 128, 8 441, 10 195 B on 2026-10-15, before 4c-2's lines: room first where short, each new line a pointer) and the code README (after 5a-1's move); `code-bindings.md` opening, "The walk", "Guarantees" (`INDEX_FORMAT` unchanged), "Known limits" (all say nothing is stored); `code-identity.md` opening ("Phase 3 stores it") and "Open" (`symbols` and `markers` rows); 05 s3.3 (`ast_hash`), s5.1 (:158), s5.2, 07 s2, s5 pointers; AC-10 in 08; `CLAUDE.md` State; `stack.md` with 6c only.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Incremental ≠ rebuild** via cross-file facts, a re-parenting manifest, `targets_fp`, `[ids]`-driven mentions, an unbumped `CODE_INDEX_FORMAT` (invisible to TE-2's single build; Q16 catches it).
- **Divergence**: ambiguity is computed in `naming.rs`, the store pass, the eval; drifting, reads and verify disagree (ADR-0046); RD-8, TE-2 guard the first two. The eval's count differs by design (undiscriminated: `docs/canon/symbol-names.md` "Ambiguity").
- The stored digests feed 7a's read `sync` (`bundle-bindings.md`); `verify` and the lock writers still hash afresh (05 s5.2).
- **AC-10**: levels cost 3-4 walks per item (`hash-levels.md` "Risks") against 6.67 ms per kLOC; 05 s5.2's 112 MB/s was rayon on 14 cores: 6c is likely. Each refresh BLAKE3s every code file; DB growth unpruned; pilots hold no marker. The 6c trigger is one-sided (Q26); 7a needs only `noop_ms`.
- **Carry-overs**: 4b-1 and 4c-1 shipped (4c-1 added `CodeTree::empty()` and the one-commit seam in `verify_rows.rs`); 4b-2 (in progress: core `check/`, `raise.rs`; store `base.rs`, `check.rs`, `git.rs`, `verify_git.rs`, `lib.rs`; CLI `main.rs`, `lib.rs`, `check.rs`, `create.rs`, `drift.rs`, `interpret.rs`, `lock.rs`, `propose.rs`, `verify*.rs`, `bump.rs`) and 4c-2 (store `queue.rs`; CLI `main.rs`, `apply.rs`, `create.rs`, `decide.rs`) edit files 6a-2 edits (`main.rs`, `lib.rs`, store `lib.rs`): 6a-2 starts on the commit after both ship. New comments cite the canon written at shipping (`doc_pointers.rs` pins quoted headings). From 6a-1's review 1 (nits, `specengine-code`): `tree_passes` asserts `files.len() == named.len()` (`debug_assert_eq!`) or states its panic contract, its `zip` truncating silently today; an indexed `name_file` that does not rebuild `TargetIndex` per call; `items.rs`' `record.cfg.clone()` (and `make_record`'s `scope.cfg` clone) moved into the mod, impl and trait arms that use it; comments skipped when picking an attribute's path (`#[/*c*/cfg(a)]`, `path_attribute` alike: a name change for such twins, before `CODE_INDEX_FORMAT`'s first value); `trim_end` before `…` in `bind.rs` `cap_signature`; the comments citing `symbol-discriminator.md` "Discriminator", "Signature" (`items.rs`, `naming.rs`, `bind.rs`, `lib.rs`) re-pointed to `docs/canon/symbol-names.md`.

## Out of scope

Bundle layers, read `sync` (7a); `delta.symbols`, `get_impact --since` (7b; Q6); the Bevy layer (10a `bevy-roles.md`, 10b `bevy-dump.md`: role rows beside these, `layer` stays `a`); layers B, C; module and RON digests (Q14); `verify` on the index; MCP and daemon refresh; HTTP, UI (Q8); a watcher; fuzzy search, references, call graphs; pruning.

- **6a-3 `symbol-reads`** (Q23): lookup tiers, `spec symbols`, `spec unmapped`, over this slice's tables.
- **6b**, split 2026-10-16 (`code-tools.md` Q28): 6b-1 `code-tools.md`, MCP `find_symbols`, `refs`, `unmapped`, `TOOLS` 18, `read-spec`, after 6a-3; 6b-2 `verify-node.md`, `spec verify --node` (on Q17's read and this slice's refresh), `check_binding`, `TOOLS` 19, `implement`, after 5c. MCP `INSTRUCTIONS` has 5 B under its 2 048 B probes assert, which 7a keeps too (`bundle-bindings.md` RD-11): 6b-1 rewords the text (80 B left), 6b-2 adds a clause (34 B), never a line per tool.
- **6c `code-index-speed`** (if AC-10 fails): `rayon =1.12.0` in the CLI (`specengine-code`'s three dependencies are pinned: `build_graph.rs`), rows ordered by path before insert.
- **`[code] crates`** (WA-15, Q2): TE-8's `roles` (Q25) compares each pilot file's role under `layout_targets` and `cargo metadata`. All equal: `crates` stays refused as "not built" (core `code_toml.rs` `NOT_BUILT_KEYS`; `project_toml.rs` only name-checks `code`), and no planned slice builds it (`layer-c.md` Q11). A difference, recorded in "Implementation", makes its own slice (no file yet): `crates = "layout" | "cargo-metadata"` (default `layout`, in `code_fp`), `code-targets.md` "Target table"'s hardened runner in one CLI module holding every Cargo literal, cached by manifest BLAKE3; a cargo failure exits 2, never a layout fallback. Reads spawning cargo need a decision: only `spec resolve` runs `cargo metadata` (`architecture-code.md#code-identity`).

## Open

| Q | Answer (else: the other answer's cost) |
|---|---|
| 1 slice 6 | 6a (6a-1, 6a-2: Q12), 6b (6b-1 `code-tools`, 6b-2 `verify-node`: `code-tools.md` Q28), 6c `code-index-speed` if AC-10 fails |
| 2 `crates` | built only if pilot layout and metadata roles differ, as its own slice |
| 3 equal qpaths | a discriminator in the qpath (6a-1) |
| 5 `check_binding` | `spec verify --node REF [--symbol QPATH]` (6b-2 `verify-node.md`) |
| 6 `delta.symbols` | slice 7b `impact` |
| 8 HTTP route | Phase 4 |
| 9 signature | no attributes, no fields (6a-1) |
| 10 AC-10 | judged in release |
| 11 order | 6a after 5a-2 and before 5b; 7a after 5b. 6a stores, for every item, 5a-1's `LevelState` as `level_state` with its categories, `sig_hash`, `body_hash`, `deps_hash`, `skeleton_hash`, `quote_hash`, `body_bytes` (5b's key names); no `ast_hash` column (else: 5b, 5c goldens rewritten, ordinal shifts untested in 5c, lookup two slices later) |
| 12 split | 6a-1 `symbol-discriminator.md`: the discriminator and the display signature entry in `specengine-code`, with the goldens and canon of slices 1, 3, 4a; 6a-2 (this): store tables, refresh, `index`/`refs`/`symbols`/`unmapped`, AC-10; `crates` its own slice only if TE-8 shows layout and metadata differ (else: one L slice, out of iterations) |
| 13 AC-10 | judge `hash_ms` and `one_file_ms` as 08 states them; add `noop_ms` ≤ 200 for 7a; record `full_ms`; decide 6c from 5a-2's figure first (else: a gate stricter than 08, 7a's undefined) |
| 14 module and RON digests | not stored; reads parse the bound files whose blob differs (else: a whole-file walk per file per refresh) |
| 15 bad `[code]` in `spec index` | write the md index, then exit 2 naming the code cause (else: the md index held hostage) |
| 16 analyzer stamp | a CLI `code_format_history.txt`: `<CODE_INDEX_FORMAT> <code-a dump hash>` (else: unbumped rules, stale rows) |
| 17 6b-2's partial tree | 6a-2 exposes per-file stored names (qpath, module, ambiguity) (else: `verify --node` walks the tree) |
| 21 `name_files`' halves | 6a-1 ships them (`symbol-discriminator.md` Q21) (else: `naming.rs` is rewritten twice and this slice grows) |
| 23 split (owner) | 6a-2 (this): every table, `INDEX_FORMAT` 9, the refresh, `index`, `refs` from the index, names for 6b, AC-10; 6a-3 `symbol-reads.md`: lookup tiers, `spec symbols`, `spec unmapped`; 6b after 6a-3, 7a after 6a-2 (else: one L slice with a seeded property test and two commands; a third iteration likely) |
| 24 fixture | a new `fixtures/code-index/`; `code-a` stays as 6a-1 leaves it (else: every code-a golden of slices 1-4c-1 and 6a-1 is re-pinned here) |
| 25 TE-8's figures (owner, critical for Q2) | `code-index` fields `twins`, `signatures`, `roles`; cargo by absolute path; `roles` counts only with metadata answers (else: Q2 is closed on layout compared with layout, and the twin and signature figures are never counted) |
| 26 the 6c trigger | one-sided: 5a-2's pilot `levels_ms_per_kloc` above 6.67 plans 6c, otherwise TE-8 decides; one `kloc` definition (else: a `parse_ms_per_kloc` added to 5a-2's `cost`) |

Q1-Q10 (2026-10-10), Q11-Q18 (2026-10-13, the refresh's Q1-Q8) decided under the owner's pre-approval, review open; Q-numbers shared with `symbol-discriminator.md` and `symbol-reads.md`, where Q4, Q7, Q18 and Q27 (the reads) now live; Q16's dump is `fixtures/code-index`'s since Q24. WA-n: the 2026-10-10 analysis' working answers. Q21, Q23-Q27 (2026-10-15, the refresh against shipped 4b-1, 4c-1 and the 5a drafts) decided under the owner's pre-approval, review open; Q23 and Q25 flagged.

## Implementation

Not started. The split (2026-10-13, Q12): RD-1..5, TE-1..8, SW-1..3 kept; old RD-6, RD-7 -> `symbol-discriminator.md` RD-1, RD-2; old RD-8..12 -> RD-6..10; old RD-13 -> RD-11 (its built half: the later `crates` slice); old RD-14 -> RD-12; RD-13..15 new; old TE-1's twin, signature forms -> `symbol-discriminator.md` TE-1, TE-2. Refreshed 2026-10-15, no renumbering: RD-3, RD-9, RD-12, RD-13 gain tests; RD-11 a case, not a test; TE-1 a new fixture; TE-2 twin edits; TE-6, TE-8 rewritten. Split (Q23): RD-5, RD-6, RD-7, RD-10's `symbols`/`unmapped` half, TE-1's forms, tiers and unmapped rows, TE-4's and TE-6's `symbols`/`unmapped` parts, TE-8's `unmapped` -> `symbol-reads.md`; RD-5..RD-7 kept as pointers; RD-10's cache test renamed `index_leaves_the_worktree_untouched`, RD-2's history pin `code_format_history_pins_the_code_index_dump` (Q24). To record: modules, TE-8's figures, Q2's outcome, deviations with reasons.
