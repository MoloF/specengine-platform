---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 3 plan 2026-10-10, slice 6a (WA-1..WA-15, WA-21, WA-22; Q1-Q10); 05 s3.3, s5.1, s5.2; 07 s2, s5; 08 s2, AC-10
adrs: []
---

# symbol-index: stored code rows, symbol lookup, coverage

## Why

Every code read re-parses the tree (`code-refs.md` "The walk"); nothing cheaply says which item sits at `path:line`, which node binds it, where code has no spec, or a signature (05 s5.1): agents read files, and a task's cost grows with the project.

Slice 6 is split (Q1): **6a** (this) stores slice 1's per-file results beside the md index, row for row equal to a fresh rebuild, and serves `spec symbols`, `spec unmapped`, `spec refs` and AC-10's code half from them; after slice 1 ships, beside 5a. No ADR (ADR-0001, -0003, -0008, -0016, -0020 applied).

## Description and interactions

- **Freshness** (WA-6): `refs`, `symbols`, `unmapped`, `index` refresh the md index, then the code index, as 7a's reads printing code data do, in the CLI, MCP and the daemon (bundle, `show --bindings`, tree, `task show`, `impact`: `bundle-bindings.md`, ADR-0046); `show` without `--bindings`, `search`, `graph` never walk code; `verify`, `lock accept` and `spec health` (`health-code.md` WA-4, 2026-10-10) keep their fresh walk.
- **Crates** (WA-5, WA-22): the store owns walk → compare → apply → cross-file pass behind its trait `CodeAnalyzer` (extensions, packages, fingerprint, analyze; plain rows), implemented in the CLI library over `specengine-code`, reused by the eval; no `rusqlite` type public, no new store or core dependency.
- **Amended** (WA-9): slices 1, 3, 4a name and lock discriminated twins (`code-a` goldens follow); unique qpaths stay byte-identical.

## Data

### Tables

WA-1, WA-2, WA-7; `code_decls`, `role` and the pass's facts: SW refinement. Per-slug DB, keyed by worktree; created with the schema in every DB, `[code]` or not; appended to `FORMAT_TABLES`/`FORMAT_TRIGGERS`; per-file rows `(file, ord)`, cascading; IDs, revs as written. `INDEX_FORMAT` 9; `format_history.txt` `9 <hash>` over `spec-a`, `spec-b`, now `code-a`; `dump.rs` maps code keys to `path`, `(path, ord)`.

| Table | Key | Columns |
|---|---|---|
| `code_files` | `(wt, path)` | `lang`, `blake3`, `size`, `read_error`, `utf8`, `package`, `role`, `module`, `orphan_errors`, `targets_fp`; pass: `declared_by` |
| `code_packages` | `(wt, dir)` | `manifest_b3`, `source`, `targets` (JSON) |
| `symbols` | `(file, ord)`; `qpath`, `name` indexed | `item_kind`, `name` (bare last segment), `owner`, `qpath` (discriminator in), `discriminator`, `parent_ord`, start/end line and byte, `signature`, `hash_state`, `ast_hash`, `layer`; pass: `twin_files` |
| `symbol_suffixes` | `(file, ord, depth)`; `suffix` indexed | `suffix` |
| `code_decls` | `(file, ord)` | `target` (a `#[path]` module's path), `line` |
| `code_markers` | `(file, ord)` | `relation`, `token`, `id`, `rev`, `levels`, `levels_state`, `note`, `line`, `column`, `binding`, `symbol_ord`, `anchor` (module qpath, RON path, category) |
| `code_mentions` | `(file, ord)` | `written`, `line`, `column`, `binding`, `symbol_ord`, `anchor` |

```
symbols          (counter.rs, n) impl_item | name "impl Tally" | qpath "crates/tally::counter::impl Tally #2" | discriminator "2" | 40-45 | signature "impl Tally"
symbol_suffixes  (counter.rs, n) 1 "impl Tally #2" · 2 "counter::impl Tally #2" · 3 "crates/tally::counter::impl Tally #2"
```

Computed per read: `id_state`, `rev_state`, coverage, ambiguity (`path_attribute` if `declared_by` and the role is not shared, else `unrooted` by role, else `duplicate` if `twin_files` > 0). RON values are not symbols.

### Cache key and cross-file pass

WA-3, WA-4. A code file is re-derived when its `(path, BLAKE3)` changed, its package's `targets_fp` (BLAKE3 of the target table: a manifest edit or a new `src/bin/x.rs` renames qpaths), or the worktree's `code_fp`: BLAKE3 of `CODE_INDEX_FORMAT` (`specengine-code`, bumped with any rule here or in slice 1), `recipe_header()`, `[code]`, `IdScheme::prefixes()`; not mtime, not the blob OID. Per-file rows are pure functions of (bytes, path, package table, `code_fp`); in the same Immediate transaction one pass rewrites `twin_files` per qpath and `declared_by` per path that a re-derived or removed file held or holds. No `[code]`: the worktree's code rows go. A stamp change or `--full` re-derives all; queue tables stay.

### Discriminator

WA-9, Q3. A group: ≥ 2 items of one file with an equal qpath. Each member takes the first rule giving all members non-empty, distinct values (SW refinement): 1. `cfg(<p>)`, the predicates of its own and its enclosing impl's, trait's, inline mods' `#[cfg(..)]`, outermost first, whitespace removed, joined by `,`; 2. its 1-based ordinal in source order.

Rendered `<qpath> #<value>` (SW refinement): `crates/tally::counter::impl Tally #2`, `crates/a::sys::regen #cfg(not(test))`. No undiscriminated qpath produces this: a last segment never holds ` #` (an identifier has no space; an `impl` label's only spaces, `impl ` and ` as `, precede a type, `as` or a trait), and a segment inside the whitespace-free `<value>` ends in `)` or is digits, never a name. Cross-file groups stay `duplicate`. Ordinal shifts read `code_ahead` or `broken_binding`, never a silent `ok`.

### Signature

WA-10, Q9. From the first token after attributes and comments to the body (`hash-levels.md` "The body field", shared with 5a); no body: the item minus a final `;`; `macro_definition`: through its name. Comments dropped; whitespace runs one space, none after `(` `[` or before `)` `]` `,`; trimmed; over `SIGNATURE_CAP_BYTES` 512: cut at a char boundary, `…` within 512. `has_error`: NULL. `pub struct Tally`.

### Lookup

WA-8. Suffix rows from the structured `QPath` segments (package, unit, module…, owner, name + discriminator), never string splitting (`<T as a::Tr>` is one segment): depth d = the last d joined by `::`. The query (positionals joined by a space) matches in tiers, each a B-tree lookup (SW refinement): `exact`, `qpath` = query; `suffix`, a `suffix` = query; `name`, `name` = `query_name(query)` (after the last `::` outside `<>` `()` `[]`, a segment starting `impl ` to the end, ` #<value>` dropped). Once per symbol at its best tier; then qpath bytes, path bytes, start line, never rowid.

### spec symbols

WA-11, Q4. `spec symbols QUERY… [--limit N] | --node REF | --file PATH | --at PATH:LINE`: exactly one form; `--limit` 1..=200, default 20, QUERY only. `--node`: items, modules, RON paths bound to the nodes REF names (each stored ID resolved once per call), by path, line; `--file`: its items in source order; `--at`: items covering LINE, shortest span first.

```
crates/tally::counter::impl Tally #2 | impl_item | crates/tally/src/counter.rs:40-45 | layer a | mentions 0 | exact
  sig impl Tally
  implements | 45:6 | LAW-BOUND@3a | resolved | bad_rev | LAW-BOUND
symbols: 1 of 1; code files <f> (.rs <r>, .ron <o>), skipped <s>
```

- Target: `<qpath> | <item_kind> | <path>:<start>-<end> | layer <l> | mentions <n>[ | ambiguous <reason>][ | <tier>]`; module `<qpath> | module | <path>`, RON `<path>#<field path> | ron | <path>:<line>`, no `sig`; `sig -` when NULL. Bindings: its direct markers by line, `<relation> | <line>:<column> | <token> | <id_state>[ <fix>] | <rev_state> | <node or ->`.
- JSON, every key present: `{form, query, node, path, line, notes, symbols: [{target, qpath, name, discriminator, item_kind, path, start_line, end_line, signature, ambiguity, layer, tier, mentions, bindings: [{relation, line, column, token, id, rev, id_state, fix, rev_state, node}]}], total, omitted, code}`.
- Exit 0 answered, zero rows too; 1 `--node` REF names nothing; 2, stdout empty: usage, discovery, config, no `[code]` (`code-refs.md`'s text), PATH not a walked code file, a bad LINE, a walk gap. Cap `OUTPUT_CAP_CHARS`, then `[truncated: <k> symbols not shown]`.

### spec unmapped

WA-12, WA-13, Q7. Units: every symbol, each RON file. An item is mapped when an `item` or `module` marker binds it, an enclosing item or inline mod, or its file module, whatever the `id_state`; a RON file when a `ron` marker anchors in it. `mentioned`: units a mention reaches alike (a `file` mention: all its file's); never mapping. No path classified (no `tests`, `src` literal).

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

WA-14, WA-6. `refs` reads the refreshed index: byte-identical to slice 1's fresh walk but discriminated qpaths. `spec index` adds `code <slug>: walked 41, parsed 41, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 230, markers 31, mentions 9` (`, rederived all`), JSON `code` (or `null`); a bad `[code]`: exit 2 before any write. Store lenient (unreadable or no tree: a `read_error` row; non-UTF-8: `utf8` 0, a `note:`), `index` exits 0; reads exit 2 on any gap (`spec: the code walk is incomplete: <gap>`).

### [code] crates

WA-15, Q2. TE-8 compares each pilot file's role under `layout_targets` and `cargo metadata`. All equal: `crates` stays refused (`code_toml.rs`) until slice 11. A difference: `crates = "layout" | "cargo-metadata"` (default `layout`, in `code_fp`), `code-identity.md` "Target table"'s hardened runner in one CLI module holding every Cargo literal, cached by manifest BLAKE3; a cargo failure exits 2, never a silent layout fallback.

### AC-10 measurement

WA-21, Q10. `specengine-eval code-index` over a scratch copy of the `[code]` walk's files and manifests: `kloc` (`.rs` newlines / 1000), `rs_files`, `ron_files`, `bytes`, `items`, `hash_ms` (parse and hash, no DB), `full_ms`, `noop_ms` + `noop_parsed`, `one_file_ms` + `one_file_parsed` (a line appended to one `.rs`), `db_bytes`, `refs_ms` (information). Pass in release (dev recorded): `hash_ms`, `full_ms` × 300 / `kloc` ≤ 2 000; `one_file_ms` ≤ 200; else 6c.

## Rules and edge cases

- WHEN any sequence of file, manifest, `[code]`, `[ids]` or stamp changes is indexed, the dump SHALL equal a fresh rebuild; WHEN reads run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical.
- Only the data directory is written; pilots `#[ignore]`, `--timeout`, a scratch HOME, unnamed.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax stays in `specengine-code`; no fixture prefix, kind, `::` split or stack word in the store, core or new CLI modules; Cargo only in Q2's module.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- tables. M: created lazily -> `mcp_index.rs`; absent from `FORMAT_TABLES` -> `format_change_recreates_code_tables_and_keeps_queue_rows`.
- [ ] RD-2 -- cache key. M: manifest -> `manifest_edit_renames_qpaths_as_a_rebuild`; `[ids]` -> `ids_change_rescans_mentions`; `CODE_INDEX_FORMAT` -> `analyzer_format_change_rederives_every_code_file`; mtime -> `same_size_same_mtime_edit_is_reindexed`; `exclude` -> `code_exclude_change_drops_rows` (each ignored).
- [ ] RD-3 -- the pass. M: per-file ambiguity -> `removing_a_twin_in_another_file_clears_duplicate`; `#[path]` ignored -> `path_attribute_follows_the_declaring_file`.
- [ ] RD-4 -- injection. M: `specengine-code` in the store -> `store_depends_on_model_core_and_sqlite_but_no_measurement_crate`; tree-sitter -> `core_and_store_have_no_tree_sitter`; rowid dumped -> `code_dump_is_independent_of_insertion_order`.
- [ ] RD-5 -- lookup. M: string split -> `trait_owner_with_a_path_is_one_segment`; a scan -> `symbol_lookup_uses_the_suffix_index` (`EXPLAIN QUERY PLAN`), `symbol_lookup_cost_is_sublinear` (10k vs 100k, best of three); rowid order -> `lookup_order_is_independent_of_insertion_order`.
- [ ] RD-6 -- discriminator. M: unique qpath discriminated -> `unique_qpath_is_unchanged`; ordinal first -> `cfg_twins_are_named_by_cfg`; cross-file discriminated -> `cross_file_twins_stay_duplicate`; kept ambiguous -> `marked_adjacent_impl_is_lockable`.
- [ ] RD-7 -- signature. M: body kept -> `signature_stops_at_the_body`; comments -> `signature_drops_comments`; broken text -> `broken_item_has_no_signature`; byte cut -> `long_signature_is_cut_at_a_char_boundary`; CR -> `signature_is_equal_under_lf_and_crlf`.
- [ ] RD-8 -- `symbols`. M: two forms -> `symbols_takes_exactly_one_form`; outermost first -> `at_line_lists_innermost_first`; exit 0 without `[code]` -> `symbols_without_code_table_is_exit_2`; a key dropped -> `symbols_json_every_key_present`.
- [ ] RD-9 -- `unmapped`. M: no containment -> `module_marker_maps_its_items`; mentions map -> `mentions_never_map`; percents averaged -> `unmapped_counts_items`.
- [ ] RD-10 -- `refs`. M: unrefreshed rows -> `refs_after_edit_equals_a_fresh_walk`; a partial list -> `code_walk_gap_is_exit_2` stays green.
- [ ] RD-11 -- M: `show` without `--bindings` walks code -> `show_never_walks_code` (`--bindings` refreshes it: `bundle-bindings.md` RD-7).
- [ ] RD-12 -- M: a cache under the root -> `symbols_leaves_the_worktree_untouched`; rowid order -> `symbols_and_unmapped_are_byte_identical_across_roots_and_order`.
- [ ] RD-13 -- Q2 refused, M: key accepted -> `code_crates_key_is_refused_until_built`; built, M: layout fallback -> `cargo_failure_is_exit_2`; cwd the project -> `cargo_runs_from_root_dir`.
- [ ] RD-14 -- harness. M: no edit before the one-file timing -> `one_file_parsed` 1; vacuous timings -> `items` = stored symbols.

test-engineer:

- [ ] TE-1 -- `code-a` gains cfg twins (fn, impl), `impl a::b::T for X`, a nested inline mod with `//!`, `--at` lines in bodies, a bin and a test target; `expected.json` covers each form, tier, discriminator, signature, unmapped row.
- [ ] TE-2 -- seeded random file adds, removes, renames, edits, `Cargo.toml`, roots/exclude, `[ids]`, stamp, CRLF; after each step dump = rebuild; each RD-2, RD-3 mutation red.
- [ ] TE-3 -- `9 <hash>` with `code-a`; a proposal, task, event, stage survive a stamp change and `--full`.
- [ ] TE-4 -- determinism (two roots, cwd, reverse creation, CRLF): `symbols`, `unmapped`, `refs`.
- [ ] TE-5 -- read-only in a temp git repo: porcelain empty; code files' size, mtime, BLAKE3 kept.
- [ ] TE-6 -- genre: `spec-c` exits 2 naming `[code]`; the "Rules" scan.
- [ ] TE-7 -- build graph: store, core unchanged; CLI gains nothing outside 6c.
- [ ] TE-8 -- pilots, manual: WA-21's figures, release and dev; discriminated by cfg, ordinal, residual; signatures cut; unmapped, mentions; Q2's agreement; read-only proof.

spec-writer:

- [x] SW-1 -- this spec; 08 s2 lists 6a, 6b, (6c).
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-index.md`; store, code, CLI READMEs; `code-identity.md`; `code-refs`, `spec-lock`, `spec-verify` canons (discriminated bindings); 05 s3.3, s5.1-5.2, 07 s2, s5 pointers; AC-10 in 08; `CLAUDE.md` State; `stack.md` with 6c only.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

Incremental ≠ rebuild via cross-file facts, manifests, `[ids]`, an unbumped `CODE_INDEX_FORMAT` (TE-2). The stored `ast_hash` feeds 7a's read `sync` (equal bytes, equal hash: `bundle-bindings.md` "Description and interactions"); `verify` and the lock writers still hash afresh (05 s5.2). Each refresh hashes every code file; DB growth unpruned. Pilots hold no marker.

## Out of scope

bundle layers, read `sync` (7a `bundle-bindings`), `delta.symbols`, `get_impact --since` (7b `impact`; Q6); the Bevy layer (10); layers B, C; levels in the index; `verify` on the index; HTTP, UI (Q8); a watcher; fuzzy search, references, call graphs (LSP's); pruning.

- **6b `code-tools`** (after 4a): MCP `find_symbols`, `refs`, `unmapped`, `check_binding` (= `spec verify --node REF [--symbol QPATH]`, 4a's rows restricted), TOOLS 18, skills.
- **6c `code-index-speed`** (if AC-10 fails): `rayon =1.12.0`, rows ordered by path before insert.

## Open

| Q | Answer |
|---|---|
| 1 slice 6 | 6a, 6b `code-tools`, 6c `code-index-speed` if AC-10 fails |
| 2 `crates` | built only if pilot layout and metadata roles differ |
| 3 equal qpaths | a discriminator in the qpath |
| 4 `--unbound` | none; `spec unmapped FILE` |
| 5 `check_binding` | `spec verify --node REF [--symbol QPATH]` (6b) |
| 6 `delta.symbols` | slice 7b `impact` |
| 7 coverage | containment; a RON file one unit; mentions apart |
| 8 HTTP route | Phase 4 |
| 9 signature | no attributes, no fields |
| 10 AC-10 | judged in release |

Decided under the owner's pre-approval, review open; WA-n: the analysis' working answers (2026-10-10); SW refinement: the spec-writer's.

## Implementation

Not started. To record: modules, TE-8's figures, Q2's outcome, deviations with reasons.
