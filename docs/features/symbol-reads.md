---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 slice 6a-3, split from symbol-index 2026-10-15 (its Q23, decided under the owner's pre-approval, review open; WA-8, WA-11..WA-13; Q4, Q7, Q18, Q27); 05 s5.1 (:158); 07 s1.2; 08 s2
adrs: []
---

# symbol-reads: symbol lookup, `spec symbols`, `spec unmapped`

## Why

6a-2 (`symbol-index.md`) stores every item, marker and mention beside the md index, but no command asks it which item sits at a name or `path:line`, what binds a node, or where code has no spec (05 s5.1; :158's suffix index): agents still read files. This slice answers the three on the CLI, the ground 6b mirrors in MCP (`code-tools.md`) and 7b's `--symbol` reuses (`impact.md`).

Split from 6a-2 (its Q23) so the reads neither wait on nor share iterations with its seeded incremental-equals-rebuild property test (its TE-2). Order (08 s2): after 6a-2, before 6b; 7a needs only 6a-2. No ADR: 6a-2's apply (ADR-0008, ADR-0020; ADR-0046: `architecture-code.md#sync-in-reads` already lists `symbols`, `unmapped`; ADR-0047: `#health`'s mapping rule reused, health never reads this index).

## Description and interactions

- **Who sees it**: the owner and agents through Bash: `spec symbols` (a query, a node, a file or a line) and `spec unmapped` (coverage per directory and file); 6b mirrors both (`code-tools.md` `find_symbols`, `unmapped`); 7b's `--symbol` takes the lookup's best tier and `unmapped`'s containment (`impact.md`).
- **Freshness**: each refreshes the md index, then the code index, as `refs` (`symbol-index.md` "Description and interactions", **Freshness**); a walk gap exits 2 (`symbol-index.md` "spec refs and spec index").
- **Crates**: store, the tier lookups and the containment as queries over 6a-2's tables, comparing strings only; `specengine-code` `naming.rs`, `query_name` (Q27); CLI, the two commands (new modules join `code_refs_walk.rs` `NEW_MODULES`), `Corpus::judge` once per stored ID, `OUTPUT_CAP_CHARS`. No table, column, `INDEX_FORMAT` or `CODE_INDEX_FORMAT` change (6a-2 creates every table: its Q23); no new dependency; MCP, HTTP, UI untouched (6b, Phase 4).

## Data

### Lookup

WA-8. Over 6a-2's `symbol_suffixes` rows (`symbol-index.md` "Tables", **Suffixes**) and its `qpath`, `name` indexes. The query (positionals joined by a space) matches in tiers, each a B-tree lookup (SW refinement): `exact`, `qpath` = query; `suffix`, a `suffix` = query; `name`, `name` = `query_name(query)` (after the last `::` outside `<>` `()` `[]`, a segment starting `impl ` to the end, ` #<value>` dropped). Once per symbol at its best tier; then qpath bytes, path bytes, start line, never rowid. Shells read an unquoted `#2` as a comment: quote it. `query_name` lives in `specengine-code` (`naming.rs`), the one reader of `::` in a query; the store compares strings only (Q27, "Rules and edge cases").

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

## Rules and edge cases

- WHEN `symbols` or `unmapped` run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical; no order follows rowid.
- WHEN a marker's ID is dangling, a look-alike, a queue ID or another project's, it SHALL map nothing, at any containment depth (Q18); a mention SHALL never map (Q7).
- Only the data directory is written, through 6a-2's refresh.
- Genre (`docs/canon/architecture.md#universal`): `query_name` in `specengine-code` is the only reader of `::` in a query (Q27); no fixture prefix, kind, `::` split, stack word, Cargo literal, `tests` or `src` literal in the store, core or the new CLI modules (`code_refs_walk.rs` `NEW_MODULES`).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- lookup (Q27). M: string split -> `trait_owner_with_a_path_is_one_segment`; a scan -> `symbol_lookup_uses_the_suffix_index` (`EXPLAIN QUERY PLAN`), `symbol_lookup_cost_is_sublinear` (10k vs 100k, best of three); rowid order -> `lookup_order_is_independent_of_insertion_order`.
- [ ] RD-2 -- `symbols`. M: two forms -> `symbols_takes_exactly_one_form`; outermost first -> `at_line_lists_innermost_first`; exit 0 without `[code]` -> `symbols_without_code_table_is_exit_2`; a key dropped (`levels`, `note` too) -> `symbols_json_every_key_present`.
- [ ] RD-3 -- `unmapped`. M: no containment -> `module_marker_maps_its_items`; mentions map -> `mentions_never_map`; percents averaged -> `unmapped_counts_items`; any `id_state` maps -> `dangling_marker_never_maps`.
- [ ] RD-4 -- M: rowid order -> `symbols_and_unmapped_are_byte_identical_across_roots_and_order`.

test-engineer:

- [ ] TE-1 -- over 6a-2's `fixtures/code-index/` as it stands (a new file there moves its pinned dump), `expected.json` gains each `symbols` form (QUERY, `--node`, `--file`, `--at`), each tier (`exact`, `suffix`, `name`; a quoted `#2`) and `unmapped`'s rows (a directory, a file, `(total)`, a file PATH's unmapped items, `-` for no unit).
- [ ] TE-2 -- determinism (two roots, cwd, reverse creation, CRLF): `symbols`, `unmapped`.
- [ ] TE-3 -- genre: on `spec-c` `symbols`, `unmapped` exit 2 naming `[code]`, stdout empty; the new modules in the "Rules" scan.
- [ ] TE-4 -- pilots A and B, manual, run by the owner under 6a-2's TE-8 conditions: `unmapped` per root (units, mapped, mentioned), counts only, recorded in "Implementation"; read-only proof as there.

spec-writer:

- [x] SW-1 -- this spec (split from `symbol-index.md` 2026-10-15, Q23); 08 s2 lists 6a-3; `code-tools.md`, `impact.md` pointers follow.
- [ ] SW-2 -- at shipping: "Lookup", "spec symbols", "spec unmapped" appended to 6a-2's `docs/canon/symbol-index.md` if it stays ≤ 12 288 B, else a Tier 2 file of their own (`docs/index-canon.md` re-measured); the CLI README's command lines (pointers, room first); `code-tools.md`, `impact.md` pointers moved to the canon; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

The tables, the refresh, `spec index`, `refs` from the index, names for 6b, AC-10 (6a-2 `symbol-index.md`); MCP `find_symbols`, `unmapped` (6b `code-tools.md`); 7b's `--symbol` (`impact.md`); `--unbound` (Q4); fuzzy search, references, call graphs; HTTP, UI (Q8).

## Open

| Q | Answer (else: the other answer's cost) |
|---|---|
| 4 `--unbound` | none; `spec unmapped FILE` |
| 7 coverage | containment; a RON file one unit; mentions apart |
| 18 `unmapped` | only resolved, alias or ambiguous-holder markers count as mapping, as health row 5 (else: a gone-node marker covers) |
| 27 `query_name` | in `specengine-code` `naming.rs`, the one `::` reader (else: a `::` split in the store or CLI, against "Rules") |

Q-numbers are shared with `symbol-index.md` (its Q23: this split) and `symbol-discriminator.md`. Q4, Q7 decided 2026-10-10 and Q18 2026-10-13, as rows of `symbol-index.md`; Q27 decided 2026-10-15 (the refresh against shipped 4b-1, 4c-1 and the 5a drafts); all under the owner's pre-approval, review open. WA-n: the 2026-10-10 analysis' working answers (`symbol-index.md`); "SW refinement": the spec-writer's.

## Implementation

Not started. Split from `symbol-index.md` (2026-10-15, Q23): its RD-5, RD-6, RD-7 -> RD-1, RD-2, RD-3; RD-10's `symbols_and_unmapped_are_byte_identical_across_roots_and_order` -> RD-4; TE-1's forms, tiers and unmapped rows -> TE-1; TE-4's and TE-6's `symbols`, `unmapped` parts -> TE-2, TE-3; TE-8's `unmapped` -> TE-4. Its "Lookup" was cut in two: the suffix rows' derivation stays with 6a-2's tables, the tiers came here. To record: modules, TE-4's figures, deviations with reasons.
