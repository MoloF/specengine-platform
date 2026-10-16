---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 6a-2 (WA-1..WA-8, WA-11..WA-15, WA-21, WA-22; Q1-Q27), split 2026-10-13 and 2026-10-15; 05 s3.3, s5.1, s5.2; 07 s2, s5; 08 s2, AC-10; Rust 1 iteration, review accepted, tests 1 round; TE-8 the owner's
shipped: 2026-10-16
adrs: []
---

# symbol-index: stored code rows, `refs` from them, AC-10

## Why

Every code read re-parsed the whole tree; nothing cheaply said which item sits at `path:line`, which node binds it, where code has no spec, or its signature (05 s5.1): agents read files, and a task's cost grew with the project.

Slice 6 is split (Q1, Q12, Q23): 6a-1 `symbol-discriminator.md` (names, signatures); **6a-2** (this): every code table, `INDEX_FORMAT` 9, the incremental refresh equal to a rebuild, `spec index`, `refs` from the index, the stored names 6b-2 reads, AC-10's code measurement; 6a-3 `symbol-reads.md` (lookup tiers, `spec symbols`, `spec unmapped`); 6b-1 `code-tools.md`, 6b-2 `verify-node.md`; 6c `code-index-speed` if AC-10 fails. Listed after 5a-2 and 6a-1, before 5b (08 s2; Q11: every item's level digests stored before 5b).

No ADR: ADR-0001, -0003, -0008, -0016, -0020 and -0046..-0050 applied (`architecture-code.md#sync-in-reads`, `#health`: health never reads this index).

## Description and interactions

Canon: `docs/canon/symbol-index.md` (all of it); `code-bindings.md` (opening, "The walk", "`spec refs`", "Guarantees", "Known limits"); `code-identity.md` (opening, "Open"); `symbol-names.md` ("Discriminator", "Ambiguity", "Signature", "Frozen names", "Known limits"); the store, CLI, eval, code and ra READMEs; 05 s3.3, s5.1, s5.2, 07 s2, s5, 08 AC-10 pointers.

- **Freshness**: only `spec index` and `spec refs` refresh the code index (canon opening; RD-9).
- **Crates**, 6b's partial tree (Q17): canon "Crates and seams".
- **Oracle**: `refs.rs` `collect_rust`, `collect_ron` over a walked tree, RD-8's (9a lifts them).

## Data

### Tables

`docs/canon/symbol-index.md` "Tables" (WA-1, WA-2, WA-7, WA-8; Q11, Q14, Q23).

### Cache key and cross-file pass

`docs/canon/symbol-index.md` "Cache key and cross-file pass" (WA-3, WA-4; 6a-1's S4).

### Analyzer stamp

`docs/canon/symbol-index.md` "Analyzer stamp" (Q16, Q24).

### spec refs and spec index

`docs/canon/symbol-index.md` "spec refs and spec index" (WA-6, WA-14, Q15): a bad `[code]` prints `<config>:<line>: …` after the md index, exit 2 (D5).

### AC-10 measurement

`docs/canon/symbol-index.md` "AC-10 measurement" (WA-21, Q10, Q13, Q25, Q26).

## Rules and edge cases

- WHEN any sequence of file, manifest, `[code]`, `[ids]` or stamp changes is indexed, the dump SHALL equal a fresh rebuild and `refs` from the index SHALL equal `refs` from the walk; WHEN reads run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical.
- Only the data directory is written; pilots `#[ignore]`, `--timeout`, a scratch HOME, unnamed.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax stays in `specengine-code`; no fixture prefix, kind, `::` split, stack word or Cargo literal in the store, core or new CLI modules, which join `code_refs_walk.rs` `NEW_MODULES` (`STACK_WORDS` holds `system`).

## Acceptance criteria

rust-developer:

- [x] RD-1 -- tables. M: created lazily -> `index_without_code_creates_code_tables` (spec-a); absent from `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [x] RD-2 -- cache key. M (each ignored): manifest -> `manifest_edit_renames_qpaths_as_a_rebuild`; package directory -> `new_manifest_between_reparents_files`; `[ids]` -> `ids_change_rescans_mentions`; `CODE_INDEX_FORMAT` -> `analyzer_format_change_rederives_every_code_file`; mtime -> `same_size_same_mtime_edit_is_reindexed`; `exclude` -> `code_exclude_change_drops_rows`. M: a signature rule changed, the stamp kept -> `code_format_history_pins_the_code_index_dump` (Q24).
- [x] RD-3 -- the pass. M: per-file ambiguity -> `removing_a_twin_in_another_file_clears_duplicate`; `#[path]` ignored -> `path_attribute_follows_the_declaring_file`; a scan for cross-file twins -> `twin_pass_reads_the_qpath_index` (`EXPLAIN QUERY PLAN`).
- [x] RD-4 -- injection. M: `specengine-code` in the store -> `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`; tree-sitter -> `core_and_store_have_no_tree_sitter`; rowid dumped -> `code_dump_is_independent_of_insertion_order`.
- RD-5 -- lookup: moved to `symbol-reads.md` RD-1 (Q23).
- RD-6 -- `symbols`: moved to `symbol-reads.md` RD-2 (Q23).
- RD-7 -- `unmapped`: moved to `symbol-reads.md` RD-3 (Q23).
- [x] RD-8 -- `refs`. M: unrefreshed rows -> `refs_after_edit_equals_a_fresh_walk`; index ≠ walk -> `refs_from_index_equals_refs_from_the_walk`; a partial list -> `code_walk_gap_is_exit_2` stays green.
- [x] RD-9 -- freshness. M: `show`, `search`, `graph` refresh code -> `show_search_graph_never_refresh_code`; verify, `verify --staged`, `propose drift` or `lock accept` refreshes it -> `verify_leaves_the_code_index_untouched` over the four.
- [x] RD-10 -- M: a cache under the root -> `index_leaves_the_worktree_untouched` (`index`, `refs`); its `symbols`, `unmapped` half: `symbol-reads.md` RD-4 (Q23).
- [x] RD-11 -- Q2 refused: `crates` joins `code_table_unknown_key_is_refused`'s cases (refused today by core `code_toml.rs` `NOT_BUILT_KEYS`: no code change), its `show`, `check` exit 0 kept. M: the key accepted.
- [x] RD-12 -- harness. M: no edit before the one-file timing -> `one_file_parsed` 1; vacuous timings -> `items` = stored symbols; the first file edited -> the largest `.rs` is; Q25's fields: a cargo stub answering another table -> `roles_differ_counts_a_changed_role` (as `ast_hash_units.rs`), `differ` `null` with no answer; `twins`, `signatures` off -> `fixtures/code-index/expected.json`'s counts.
- [x] RD-13 -- seams. M: a second listing -> `index_and_walk_list_alike` (over `code_refs_walk.rs`'s cases); a second parse -> `one_parse_per_rederived_file`; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`, which gains the analyzer (it hashes through `item_levels(`) (the pin names per file the entry it must call: `hash_bound_items(` for `lock.rs`, `item_levels(` for the analyzer; 5b's files: `lock-levels.md` Q16).
- [x] RD-14 -- a bad `[code]` (Q15). M: the md index held back -> `bad_code_table_still_writes_the_md_index` (exit 2, cause named, md rows fresh).
- [x] RD-15 -- names for 6b-2 (Q17). M: names from a tree walk -> `stored_names_equal_the_walks_for_one_file`.

test-engineer:

- [x] TE-1 -- a new `fixtures/code-index/` (own `specengine.toml`, `expected.json`): `impl a::b::T for X`, cfg and ordinal twins, a nested inline mod with `//!`, `--at` lines in bodies, a bin and a test target, a `.ron` file, a broken item; `expected.json` covers each discriminated row, suffix rows, signature, level state (each `symbols` form, tier, unmapped row: `symbol-reads.md` TE-1). `fixtures/code-a` stays as 6a-1 leaves it (slices 1-4c-1's goldens); RD-8 reads it too.
- [x] TE-2 -- seeded random file adds, removes, renames, edits, `Cargo.toml` (one between a file and its package too), roots/exclude, `[ids]`, stamp, CRLF, twins inserted, removed and `cfg`-edited (rule 1 <-> rule 2); after each step dump = rebuild and RD-8's refs = the walk's; each RD-2, RD-3 mutation red; bounded in steps and time.
- [x] TE-3 -- the store's `9 <hash>` (spec-a, spec-b, the stub), the CLI's `1 <hash>` (code-index); a proposal, task, event, stage survive a stamp change and `--full`. Pins moved: `bundle_config.rs` `ac16_index_format_is_eight` (9, nine lines), store `format.rs`'s format-8 pin, the store README's format line, the `mcp_index.rs` header.
- [x] TE-4 -- determinism (two roots, cwd, reverse creation, CRLF): `refs` (`symbols`, `unmapped`: `symbol-reads.md` TE-2).
- [x] TE-5 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- [x] TE-6 -- genre: on `spec-c` `refs` exits 2 naming `[code]`; `index` exits 0, no `code` line, JSON `code` `null`; the "Rules" scan (`symbols`, `unmapped`: `symbol-reads.md` TE-3).
- [x] TE-7 -- build graph: store, core unchanged; `specengine-code` keeps three dependencies; CLI gains nothing outside 6c; MCP `TOOLS` 15.
- [ ] TE-8 -- pilots A and B, manual, run by the owner, pending (`#[ignore]`, release and dev, foreground, `--timeout`, a scratch HOME, the child given `PATH`, the label's variables and `SPECENGINE_CARGO` as an absolute toolchain path, `rustup which cargo`: rustup's proxy finds no toolchain under a scratch HOME, and `roles` would compare layout with layout). Recorded in "Implementation", counts only: AC-10's figures; `twins`; `signatures`; mentions per root (`spec index`'s `code` line; `unmapped`: `symbol-reads.md` TE-4); `roles` (Q2's outcome, counted only with `roles.metadata` > 0); read-only proof (`git --no-optional-locks status`, a (path, kind, size, mtime) listing, before and after).

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13; 08 s2 lists 6a-1, 6a-2, 6b, (6c); Q11's neighbours amended (`lock-levels`, `bundle-bindings`, `code-tools`, `anchor-states`, `hash-levels`, `impact`); split again 2026-10-15 (Q23): `symbol-reads.md`, 08 s2 lists 6a-3, `code-tools`, `impact` pointers follow.
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-index.md` (≤ 12 288 B; with Q23, 6a-3 appends "Lookup", "spec symbols", "spec unmapped" if it holds, else its own file); `docs/index-canon.md` re-measured (`docs/README.md` "Generated"); store, CLI, eval READMEs (room first, each new line a pointer) and the code README (after 5a-1's move); `code-bindings.md` opening, "The walk", "Guarantees" (`INDEX_FORMAT` unchanged), "Known limits" (all say nothing is stored); `code-identity.md` opening ("Phase 3 stores it") and "Open" (`symbols` and `markers` rows); 05 s3.3 (`ast_hash`), s5.1 (:158), s5.2, 07 s2, s5 pointers; AC-10 in 08; `CLAUDE.md` State; `stack.md` with 6c only.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

Bundle layers, read `sync` (7a); `delta.symbols`, `get_impact --since` (7b; Q6); the Bevy layer (10a `bevy-roles.md`, 10b `bevy-dump.md`: role rows beside these, `layer` stays `a`); layers B, C; module and RON digests (Q14); `verify` on the index; MCP and daemon refresh; HTTP, UI (Q8); a watcher; fuzzy search, references, call graphs; pruning.

- **6a-3 `symbol-reads`** (Q23): lookup tiers, `spec symbols`, `spec unmapped`, over these tables.
- **6b** (`code-tools.md` Q28): 6b-1 MCP `find_symbols`, `refs`, `unmapped`; 6b-2 `spec verify --node` on Q17's read and this refresh, `check_binding`.
- **6c `code-index-speed`** (if AC-10 fails): `rayon =1.12.0` in the CLI, rows ordered by path before insert.
- **`[code] crates`** (WA-15, Q2): TE-8's `roles` compares each pilot file's role under `layout_targets` and `cargo metadata`. All equal: `crates` stays refused as "not built" (core `code_toml.rs` `NOT_BUILT_KEYS`). A difference makes its own slice: `crates = "layout" | "cargo-metadata"` (in `code_fp`), `code-targets.md` "Target table"'s runner in one CLI module, cached by manifest BLAKE3; a cargo failure exits 2, never a layout fallback; reads spawning cargo need a decision (`architecture-code.md#code-identity`).

## Open

Decided under the owner's pre-approval: Q1 slice 6's split; Q2 `crates` built only if TE-8's layout and metadata roles differ, as its own slice; Q3 a discriminator in the qpath (6a-1); Q5 `check_binding` is `spec verify --node` (6b-2); Q6 `delta.symbols` in 7b; Q8 HTTP in Phase 4; Q9 a signature without attributes or fields (6a-1); Q10, Q13 AC-10 judged in release as 08 states it, plus `noop_ms` ≤ 200 for 7a, `full_ms` recorded; Q11 6a after 5a-2, before 5b, every item's `LevelState`, categories and level digests stored (5b's key names), no `ast_hash`; Q12, Q23 the 6a-1/6a-2/6a-3 split; Q14 no module or RON digest; Q15 a bad `[code]` writes the md index, then exits 2; Q16 a CLI `code_format_history.txt`; Q17 per-file stored names for 6b-2; Q21 `name_files`' halves in 6a-1; Q24 a new `fixtures/code-index/`, `code-a` untouched; Q25 TE-8's `twins`, `signatures`, `roles`, cargo by absolute path, `roles` counted only with metadata answers; Q26 the 6c trigger one-sided (5a-2's `levels_ms_per_kloc` above 6.67 plans it, else TE-8 decides). Q4, Q7, Q18, Q27 live in `symbol-reads.md`. D5, D6 owner pre-approved 2026-10-16. WA-n: the 2026-10-10 analysis' working answers.

## Implementation

| Module | What it does |
|---|---|
| store `code.rs` | `CodeAnalyzer` and its rows; `update_code` (snapshot plan, `Immediate` re-check, apply, pass), `clear_code`, `stored_names`, `code_refs`, `code_symbols`; `TWIN_SQL` |
| store `schema.rs`, `dump.rs`, `index.rs`, `lib.rs`, `write.rs` | eight tables, four indexes, `FORMAT_TABLES`; path-keyed dump; `reparsed_all` for `rederived_all`; `INDEX_FORMAT` 9; `fingerprint` `pub(crate)` |
| CLI `code_index.rs` | `CodeFiles`, `Analyzer`, `refresh_code`, `CodeRefresh`, `listed_code_files` |
| CLI `code_walk.rs` | `list` shared; `package_tables`, `disk_packages` split from `name_tree` |
| CLI `refresh.rs`, `lib.rs` | `index`'s code line, JSON `code`, `code_error` (exit 2), the gap warning; exports |
| CLI `refs.rs` | `refs` from the index (`collect_stored`); the walk kept as the oracle |
| code `lib.rs`, `naming.rs`, `qpath.rs`, `items.rs`, `bind.rs` | `CODE_INDEX_FORMAT` 1; `indexed_name_file`, `file_place`; `QPath::segments`, `suffixes`; 6a-1's six review nits |
| eval `code_index.rs`, `main.rs`, `harness.rs`, `ast_hash/` | `code-index`; `PILOT_CODE_ROOTS`; `targets`, `package_name` reused |

Deviations, accepted in review 1: D1 `ListedCodeFile.package` (the key's package, known before analysis); D2 `CodeFileRows` has `read_error`, an optional `role`, no `package`; D3, D4 the role and binding words, `anchor` (canon "Tables"); D5 (owner) a bad `[code]` prints `<config>:<line>: …` without `spec: `, the CLI's config-error form, as `refs`; D6 (owner) `index` warns on a gap, exit 0; D7 `"column"` quoted; D8 extra reads `code_refs`, `code_symbols`, `clear_code`; D9 `config.rs`' data example adapted to Q15. `index --json` with a bad `[code]` prints its document, `code` `null` (CLI README). Readings R1-R13 (`rederived_all`, counts, caching, gap rule, eval): the canon.

Tests, 1 round: 2281/2283 (`import_cli`, `layout_cli` red only until `fixtures/code-index/` is committed); mutations 10/10 killed (m1-m8; spares a rowid in the dump, the first file edited); RD-9's verify-side mutation not run (red by reasoning); history lines store `9 8c4c50d2…`, CLI `1 3eac6715…`. TE-8, the owner's pilot run (AC-10's figures, `twins`, `signatures`, mentions per root, Q2's `roles`), pending: it decides Q2 and 6c. Review 1's nits: done in 6a-3 (`symbol-reads.md`).
