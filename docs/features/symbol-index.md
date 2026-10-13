---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 6a (WA-1..WA-8, WA-11..WA-15, WA-21, WA-22; Q1-Q10), refreshed and split 2026-10-13 (6a-2; Q11-Q18); 05 s3.3, s5.1, s5.2; 07 s2, s5; 08 s2, AC-10
adrs: []
---

# symbol-index: stored code rows, symbol lookup, coverage

## Why

Every code read re-parses the tree (`docs/canon/code-bindings.md` "The walk", "Known limits"); nothing cheaply says which item sits at `path:line`, which node binds it, where code has no spec, or a signature (05 s5.1): agents read files, and a task's cost grows with the project.

Slice 6 is split (Q1): 6a, 6b `code-tools`, 6c `code-index-speed`; 6a again (Q12): 6a-1 `symbol-discriminator.md` (the discriminator, the signature entry) and **6a-2** (this): slice 1's per-file results and 5a-1's level digests stored beside the md index, row for row equal to a fresh rebuild, serving `spec symbols`, `unmapped`, `refs` and AC-10's code half. On shipped slices 1-4a; after 5a-1 (`bind_tree`, `item_levels`), 5a-2 (`cost.levels_ms_per_kloc`) and 6a-1; before 5b (Q11). "6a" in other drafts names both halves.

No ADR: ADR-0001, -0003, -0008, -0016, -0020 and -0046..-0050 applied (`architecture-code.md#sync-in-reads`: these four reads walk code; `#health`: never this index; C joins rows by file and start byte; digests move only with `LEVEL_RECIPE`, in `code_fp`). Only Q2's cargo runner would need one.

## Description and interactions

- **Freshness** (WA-6): `refs`, `symbols`, `unmapped`, `index` refresh the md index, then the code index, a call of its own, never inside CLI `refresh.rs` `refresh` or `corpus.rs` `indexed`, which `verify`, `lock accept`, `spec health`, `show` use, keeping their fresh `read_tree` (`#health`, `spec-verify.md` "Cascade"; `tree_of_blobs` too: the index holds the working tree only). `show` without `--bindings`, `search`, `graph` never walk code. No MCP tool or daemon route refreshes code here: 6b, 7a add theirs.
- **Crates** (WA-5, WA-22): the store owns compare, apply and the cross-file pass behind its trait `CodeAnalyzer` (list, packages, fingerprint, analyze; plain rows), implemented in the CLI library over `code_walk.rs` and `specengine-code`, which the store may not depend on (`build_graph.rs` `core_and_store_have_no_tree_sitter`); the eval via its CLI edge, MCP and the daemon via the CLI. No `rusqlite` type public; no new dependency (6c aside).
- **Seams** (built; SW refinement): one listing, `code_walk.rs` `list`/`walk`, for `read_tree` and the refresh; one parse per re-derived file for `bind_tree`, `comment_anchors`, `name_files`' per-file half (6a-1's discriminator), `item_levels`, `item_signatures`, later 10a's detector. The store pass recomputes `name_files`' tree-wide halves over stored rows as set operations: `path_targets` as `declared_by`, `duplicate` as `twin_files`. `code_fp`'s `[ids]` part is the md writer's `fingerprint` (store `write.rs`), re-checked in the transaction as there. Reused: `Corpus::judge` (`refs.rs`) once per stored ID, `OUTPUT_CAP_CHARS`.
- **Oracle**: `refs.rs` `collect_rust`/`collect_ron` over a `CodeTree` stay as RD-8's (9a lifts them).
- **6b's partial tree** (Q17): the store answers per file its module qpath and each item's qpath and ambiguity, so `verify --node` (`code-tools.md`) walks no tree.

## Data

### Tables

WA-1, WA-2, WA-7; `code_decls`, `code_worktrees`, `role`, the pass's facts: SW refinement. Per-slug DB, keyed by worktree; created with the schema in every DB; in `FORMAT_TABLES` children first, before `worktrees` (store `schema.rs`); per-file rows `(file, ord)`, cascading; IDs, revs as written.

`INDEX_FORMAT` 9 (8 today; 6a-2's alone: 4b-1, 4c-1 bump `QUEUE_SCHEMA_VERSION`, 4b-2 keeps 8, 10a takes 10); `format_history.txt` `9 <hash>` over `spec-a`, `spec-b` plus a stub analyzer's rows (the store has no dev-dependency on `specengine-code`); `code-a`'s rows: "Analyzer stamp". `dump.rs` maps code keys to `path`, `(path, ord)`.

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
- **Per read**: `id_state`, `rev_state`, coverage, ambiguity (`path_attribute` if `declared_by` and the role is not shared, else `unrooted` by role, else `duplicate` if `twin_files` > 0). RON values are not symbols.

`layer` (`layer-c.md`, ADR-0049): the layer whose rules made the row and its qpath, `a` on every row; B (10b's role rows) and C (11a's `code_monikers`) annotate from their own tables, never changing it.

### Cache key and cross-file pass

WA-3, WA-4. A code file is re-derived when its `(path, BLAKE3)` changed; its package directory (a new `Cargo.toml` between the file and its package re-parents it: `code-bindings.md` "The walk"); that package's `targets_fp` (BLAKE3 of the target table: a manifest edit or a new `src/bin/x.rs` renames qpaths); or the worktree's `code_fp` in `code_worktrees`: BLAKE3 of `CODE_INDEX_FORMAT` (`specengine-code`, bumped with any rule here, in 6a-1 or slice 1), `level_header()` (5a-1), the parsed `[code]`, the `[ids]` fingerprint. Not mtime, not the blob OID.

Per-file rows are pure functions of (bytes, path, package table, `code_fp`). In the same Immediate transaction one pass rewrites `twin_files` per undiscriminated qpath and `declared_by` per path that a re-derived or removed file held or holds. No `[code]`: the worktree's code rows go. A stamp change or `--full` re-derives all; queue tables stay.

### Analyzer stamp

Q16. `CODE_INDEX_FORMAT` starts at 1. `crates/specengine-cli/tests/code_format_history.txt` holds `<CODE_INDEX_FORMAT> <code-a dump hash>` lines, as the store's `format_history.txt`: a changed code-a dump with the stamp kept is red.

### Lookup

WA-8. Suffix rows come from the structured `QPath` segments (package, unit, module…, owner, name + discriminator), never string splitting (`<T as a::Tr>` is one segment): depth d = the last d joined by `::`. The query (positionals joined by a space) matches in tiers, each a B-tree lookup (SW refinement): `exact`, `qpath` = query; `suffix`, a `suffix` = query; `name`, `name` = `query_name(query)` (after the last `::` outside `<>` `()` `[]`, a segment starting `impl ` to the end, ` #<value>` dropped). Once per symbol at its best tier; then qpath bytes, path bytes, start line, never rowid. Shells read an unquoted `#2` as a comment: quote it.

### spec symbols

WA-11, Q4. `spec symbols QUERY… [--limit N] | --node REF | --file PATH | --at PATH:LINE`: exactly one form; `--limit` 1..=200, default 20, QUERY only. `--node`: items, modules, RON paths bound to the nodes REF names (each stored ID resolved once per call), by path, line; `--file`: its items in source order; `--at`: items covering LINE, shortest span first.

```
crates/tally::counter::impl Tally #2 | impl_item | crates/tally/src/counter.rs:40-45 | layer a | mentions 0 | exact
  sig impl Tally
  implements | 45:6 | LAW-BOUND@3a | resolved | bad_rev | default | LAW-BOUND
symbols: 1 of 1; code files <f> (.rs <r>, .ron <o>), skipped <s>
```

- Target: `<qpath> | <item_kind> | <path>:<start>-<end> | layer <l> | mentions <n>[ | ambiguous <reason>][ | <tier>]`; module `<qpath> | module | <path>`, RON `<path>#<field path> | ron | <path>:<line>`, no `sig`; `sig -` when NULL. Bindings: its direct markers by line, `<relation> | <line>:<column> | <token> | <id_state>[ <fix>] | <rev_state> | <levels> | <node or ->[ | <note>]`, `levels`, `note` as refs prints them (`code-bindings.md` "`spec refs`").
- JSON, every key present: `{form, query, node, path, line, notes, symbols: [{target, qpath, name, discriminator, item_kind, path, start_line, end_line, signature, ambiguity, layer, tier, mentions, bindings: [{relation, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, node}]}], total, omitted, code}`.
- Exit 0 answered, zero rows too; 1 `--node` REF names nothing; 2, stdout empty: usage, discovery, config, no `[code]` (`code-bindings.md` "The `[code]` table"'s text), PATH not a walked code file, a bad LINE, a walk gap. Cap `OUTPUT_CAP_CHARS`, then `[truncated: <k> symbols not shown]`.

### spec unmapped

WA-12, WA-13, Q7, Q18. Units: every symbol, each RON file. An item is mapped when a marker binds it, an enclosing item or inline mod, or its file module, and that marker's ID is resolved, an alias or ambiguous (each holder), as health row 5 (`architecture-code.md#health`): dangling, look-alike, queue and other-project IDs never map. A RON file is mapped when such a `ron` marker anchors in it. One containment function, 7b's too (`impact.md` "Seeds and walk"). `mentioned`: units a mention reaches alike (a `file` mention: all its file's); never mapping. No path classified (no `tests`, `src` literal).

PATH: a root, a directory or code file under one (else exit 2); none: a row per root. Rows: each direct child directory (aggregated, `/`) and code file by path bytes, then `(total)`; a file PATH: its row, then its unmapped items. Percent: mapped‰ rounded half up, one decimal; `-` for no unit.

```
crates/tally/src/ | units 14 | mapped 6 | 42.9 % | mentioned 3
crates/tally/tests/ | units 2 | mapped 1 | 50.0 % | mentioned 0
crates/tally (total) | units 16 | mapped 7 | 43.8 % | mentioned 3
  crates/tally::counter::Size | enum_item | 60          (file PATH only)
unmapped: code files <f> (.rs <r>, .ron <o>), skipped <s>
```

JSON `{path, rows: [{path, dir, units, mapped, percent, mentioned}], total, unmapped: [{qpath, item_kind, path, line}], notes, omitted, code}`. Exits and cap as `symbols`, no exit 1.

### spec refs and spec index

WA-14, WA-6. `refs` reads the refreshed index, byte-identical to the walk path (6a-1's names in both). `spec index` prints `indexed <slug>: walked …, parsed …, unchanged …, removed …, unreadable …` then `db <path>` (CLI `refresh.rs`); 6a-2 adds before `db` `code <slug>: walked 41, parsed 41, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 230, markers 31, mentions 9` (`, rederived all`), JSON `code` (or `null`). A bad `[code]` (Q15): the md index is written and printed, then exit 2, `spec: <config>:<line>: …` naming the code cause; no code row changes. Store lenient (unreadable or no tree: a `read_error` row; non-UTF-8: `utf8` 0, a `note:`), `index` exits 0; reads exit 2 on any gap (`spec: the code walk is incomplete: <gap>`).

### AC-10 measurement

WA-21, Q10, Q13. `specengine-eval code-index` over a scratch copy of the `[code]` walk's files and manifests: `kloc` (`.rs` newlines / 1000), `rs_files`, `ron_files`, `bytes`, `items`, `hash_ms` (parse, levels, signatures; no DB), `full_ms`, `noop_ms` + `noop_parsed`, `one_file_ms` + `one_file_parsed` (a line appended to the largest `.rs`; the md harness takes the first: eval `index.rs`), `db_bytes`, `refs_ms` (information).

Judged in release (dev recorded), as 08 AC-10 states: `hash_ms` × 300 / `kloc` ≤ 2 000; `one_file_ms` ≤ 200; plus `noop_ms` ≤ 200, 7a's gate (`bundle-bindings.md` "Dependencies and ordering"); `full_ms` recorded only. 6c: 5a-2's `cost.levels_ms_per_kloc` above 6.7 (AC-10's budget, parse excluded) plans it at once; else any failed figure here.

## Rules and edge cases

- WHEN any sequence of file, manifest, `[code]`, `[ids]` or stamp changes is indexed, the dump SHALL equal a fresh rebuild and `refs` from the index SHALL equal `refs` from the walk; WHEN reads run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical.
- Only the data directory is written; pilots `#[ignore]`, `--timeout`, a scratch HOME, unnamed.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax stays in `specengine-code`; no fixture prefix, kind, `::` split, stack word or Cargo literal in the store, core or new CLI modules, which join `code_refs_walk.rs` `NEW_MODULES` (`STACK_WORDS` holds `system`).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- tables. M: created lazily -> `index_without_code_creates_code_tables` (spec-a); absent from `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [ ] RD-2 -- cache key. M (each ignored): manifest -> `manifest_edit_renames_qpaths_as_a_rebuild`; package directory -> `new_manifest_between_reparents_files`; `[ids]` -> `ids_change_rescans_mentions`; `CODE_INDEX_FORMAT` -> `analyzer_format_change_rederives_every_code_file`; mtime -> `same_size_same_mtime_edit_is_reindexed`; `exclude` -> `code_exclude_change_drops_rows`. M: a signature rule changed, the stamp kept -> `code_format_history_pins_the_code_a_dump`.
- [ ] RD-3 -- the pass. M: per-file ambiguity -> `removing_a_twin_in_another_file_clears_duplicate`; `#[path]` ignored -> `path_attribute_follows_the_declaring_file`.
- [ ] RD-4 -- injection. M: `specengine-code` in the store -> `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`; tree-sitter -> `core_and_store_have_no_tree_sitter`; rowid dumped -> `code_dump_is_independent_of_insertion_order`.
- [ ] RD-5 -- lookup. M: string split -> `trait_owner_with_a_path_is_one_segment`; a scan -> `symbol_lookup_uses_the_suffix_index` (`EXPLAIN QUERY PLAN`), `symbol_lookup_cost_is_sublinear` (10k vs 100k, best of three); rowid order -> `lookup_order_is_independent_of_insertion_order`.
- [ ] RD-6 -- `symbols`. M: two forms -> `symbols_takes_exactly_one_form`; outermost first -> `at_line_lists_innermost_first`; exit 0 without `[code]` -> `symbols_without_code_table_is_exit_2`; a key dropped (`levels`, `note` too) -> `symbols_json_every_key_present`.
- [ ] RD-7 -- `unmapped`. M: no containment -> `module_marker_maps_its_items`; mentions map -> `mentions_never_map`; percents averaged -> `unmapped_counts_items`; any `id_state` maps -> `dangling_marker_never_maps`.
- [ ] RD-8 -- `refs`. M: unrefreshed rows -> `refs_after_edit_equals_a_fresh_walk`; index ≠ walk -> `refs_from_index_equals_refs_from_the_walk`; a partial list -> `code_walk_gap_is_exit_2` stays green.
- [ ] RD-9 -- freshness. M: `show`, `search`, `graph` refresh code -> `show_search_graph_never_refresh_code`; verify refreshes it -> `verify_leaves_the_code_index_untouched`.
- [ ] RD-10 -- M: a cache under the root -> `symbols_leaves_the_worktree_untouched`; rowid order -> `symbols_and_unmapped_are_byte_identical_across_roots_and_order`.
- [ ] RD-11 -- Q2 refused. M: key accepted -> `code_crates_key_is_refused_until_built`; `crates` joins `code_table_unknown_key_is_refused` (its `show`, `check` exit 0 kept).
- [ ] RD-12 -- harness. M: no edit before the one-file timing -> `one_file_parsed` 1; vacuous timings -> `items` = stored symbols; the first file edited -> the largest `.rs` is.
- [ ] RD-13 -- seams. M: a second listing -> `index_and_walk_list_alike` (over `code_refs_walk.rs`'s cases); a second parse -> `one_parse_per_rederived_file`; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`, which gains the analyzer (it hashes through `item_levels(`).
- [ ] RD-14 -- a bad `[code]` (Q15). M: the md index held back -> `bad_code_table_still_writes_the_md_index` (exit 2, cause named, md rows fresh).
- [ ] RD-15 -- names for 6b (Q17). M: names from a tree walk -> `stored_names_equal_the_walks_for_one_file`.

test-engineer:

- [ ] TE-1 -- `code-a` gains `impl a::b::T for X`, a nested inline mod with `//!`, `--at` lines in bodies, a bin and a test target; `expected.json` covers each form, tier, discriminated row, signature, level state, unmapped row.
- [ ] TE-2 -- seeded random file adds, removes, renames, edits, `Cargo.toml` (one between a file and its package too), roots/exclude, `[ids]`, stamp, CRLF; after each step dump = rebuild and RD-8's refs = the walk's; each RD-2, RD-3 mutation red; bounded in steps and time.
- [ ] TE-3 -- the store's `9 <hash>` (spec-a, spec-b, the stub), the CLI's `1 <hash>` (code-a); a proposal, task, event, stage survive a stamp change and `--full`. Pins moved: `bundle_config.rs` `ac16_index_format_is_eight` (9, nine lines), store `format.rs`'s format-8 pin, the store README's format line, the `mcp_index.rs` header (says 7).
- [ ] TE-4 -- determinism (two roots, cwd, reverse creation, CRLF): `symbols`, `unmapped`, `refs`.
- [ ] TE-5 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- [ ] TE-6 -- genre: `spec-c` exits 2 naming `[code]`; the "Rules" scan.
- [ ] TE-7 -- build graph: store, core unchanged; `specengine-code` keeps three dependencies; CLI gains nothing outside 6c; MCP `TOOLS` 14.
- [ ] TE-8 -- pilots, manual: AC-10's figures, release and dev; twins by cfg, ordinal and residue, counted from the index; signatures cut; unmapped, mentions; Q2's agreement; read-only proof.

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13; 08 s2 lists 6a-1, 6a-2, 6b, (6c); Q11's neighbours amended (`lock-levels`, `bundle-bindings`, `code-tools`, `anchor-states`, `hash-levels`, `impact`).
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-index.md`; store, code, CLI READMEs (10 232, 10 199, 10 240 B: room first); `code-bindings.md` opening, "The walk", "Guarantees", "Known limits" (all say nothing is stored); 05 s3.3 (`ast_hash`), s5.2, 07 s2, s5 pointers; AC-10 in 08; `CLAUDE.md` State; `stack.md` with 6c only.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Incremental ≠ rebuild** via cross-file facts, a re-parenting manifest, `targets_fp`, `[ids]`-driven mentions, an unbumped `CODE_INDEX_FORMAT` (invisible to TE-2's single build; Q16 catches it).
- **Divergence**: ambiguity is computed in `naming.rs`, the store pass, the eval; drifting, reads and verify disagree (ADR-0046); RD-8, TE-2 guard the first two.
- The stored digests feed 7a's read `sync` (`bundle-bindings.md`); `verify` and the lock writers still hash afresh (05 s5.2).
- **AC-10**: levels cost 3-4 walks per item (`hash-levels.md` "Risks") against 6.7 ms per kLOC; 05 s5.2's 112 MB/s was rayon on 14 cores: 6c is likely. Each refresh BLAKE3s every code file; DB growth unpruned; pilots hold no marker.
- **Carry-overs**: 4b-1 edits `code_walk.rs`, `verify*.rs`, `lock.rs`, store `lib.rs`, the queue: 6a-2 starts on its commit. New comments cite the canon written at shipping (`doc_pointers.rs` pins quoted headings).

## Out of scope

Bundle layers, read `sync` (7a); `delta.symbols`, `get_impact --since` (7b; Q6); the Bevy layer (10a `bevy-roles.md`, 10b `bevy-dump.md`: role rows beside these, `layer` stays `a`); layers B, C; module and RON digests (Q14); `verify` on the index; MCP and daemon refresh; HTTP, UI (Q8); a watcher; fuzzy search, references, call graphs; pruning.

- **6b `code-tools`**: MCP `find_symbols`, `refs`, `unmapped`, `check_binding` (`spec verify --node`), `TOOLS` 18, skills.
- **6c `code-index-speed`** (if AC-10 fails): `rayon =1.12.0` in the CLI (`specengine-code`'s three dependencies are pinned: `build_graph.rs`), rows ordered by path before insert.
- **`[code] crates`** (WA-15, Q2): TE-8 compares each pilot file's role under `layout_targets` and `cargo metadata`. All equal: `crates` stays refused as "not built" (core `code_toml.rs` `NOT_BUILT_KEYS`; `project_toml.rs` only name-checks `code`), and no planned slice builds it (`layer-c.md` Q11). A difference, recorded in "Implementation", makes its own slice (no file yet): `crates = "layout" | "cargo-metadata"` (default `layout`, in `code_fp`), `code-identity.md` "Target table"'s hardened runner in one CLI module holding every Cargo literal, cached by manifest BLAKE3; a cargo failure exits 2, never a layout fallback. Reads spawning cargo need a decision: only `spec resolve` runs `cargo metadata` (`architecture-code.md#symbol-identity-and-digest`).

## Open

| Q | Answer (else: the other answer's cost) |
|---|---|
| 1 slice 6 | 6a (6a-1, 6a-2: Q12), 6b `code-tools`, 6c `code-index-speed` if AC-10 fails |
| 2 `crates` | built only if pilot layout and metadata roles differ, as its own slice |
| 3 equal qpaths | a discriminator in the qpath (6a-1) |
| 4 `--unbound` | none; `spec unmapped FILE` |
| 5 `check_binding` | `spec verify --node REF [--symbol QPATH]` (6b) |
| 6 `delta.symbols` | slice 7b `impact` |
| 7 coverage | containment; a RON file one unit; mentions apart |
| 8 HTTP route | Phase 4 |
| 9 signature | no attributes, no fields (6a-1) |
| 10 AC-10 | judged in release |
| 11 order | 6a after 5a-2 and before 5b; 7a after 5b. 6a stores, for every item, 5a-1's `LevelState` as `level_state` with its categories, `sig_hash`, `body_hash`, `deps_hash`, `skeleton_hash`, `quote_hash`, `body_bytes` (5b's key names); no `ast_hash` column (else: 5b, 5c goldens rewritten, ordinal shifts untested in 5c, lookup two slices later) |
| 12 split | 6a-1 `symbol-discriminator.md`: the discriminator and the display signature entry in `specengine-code`, with the goldens and canon of slices 1, 3, 4a; 6a-2 (this): store tables, refresh, `index`/`refs`/`symbols`/`unmapped`, AC-10; `crates` its own slice only if TE-8 shows layout and metadata differ (else: one L slice, out of iterations) |
| 13 AC-10 | judge `hash_ms` and `one_file_ms` as 08 states them; add `noop_ms` ≤ 200 for 7a; record `full_ms`; decide 6c from 5a-2's figure first (else: a gate stricter than 08, 7a's undefined) |
| 14 module and RON digests | not stored; reads parse the bound files whose blob differs (else: a whole-file walk per file per refresh) |
| 15 bad `[code]` in `spec index` | write the md index, then exit 2 naming the code cause (else: the md index held hostage) |
| 16 analyzer stamp | a CLI `code_format_history.txt`: `<CODE_INDEX_FORMAT> <code-a dump hash>` (else: unbumped rules, stale rows) |
| 17 6b's partial tree | 6a-2 exposes per-file stored names (qpath, module, ambiguity) (else: `verify --node` walks the tree) |
| 18 `unmapped` | only resolved, alias or ambiguous-holder markers count as mapping, as health row 5 (else: a gone-node marker covers) |

Q1-Q10 (2026-10-10), Q11-Q18 (2026-10-13, the refresh's Q1-Q8) decided under the owner's pre-approval, review open; Q-numbers shared with `symbol-discriminator.md`. WA-n: the 2026-10-10 analysis' working answers.

## Implementation

Not started. The split (2026-10-13, Q12): RD-1..5, TE-1..8, SW-1..3 kept; old RD-6, RD-7 -> `symbol-discriminator.md` RD-1, RD-2; old RD-8..12 -> RD-6..10; old RD-13 -> RD-11 (its built half: the later `crates` slice); old RD-14 -> RD-12; RD-13..15 new; old TE-1's twin, signature forms -> `symbol-discriminator.md` TE-1, TE-2. To record: modules, TE-8's figures, Q2's outcome, deviations with reasons.
