---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 slice 6a-3, split from symbol-index 2026-10-15 (its Q23; WA-8, WA-11..WA-13; Q4, Q7, Q18, Q27, Q35); 05 s5.1 (:158); 07 s1.2; 08 s2; Rust 1 iteration, review accepted with minors, tests 1 round; TE-4 the owner's
shipped: 2026-10-16
adrs: []
---

# symbol-reads: symbol lookup, `spec symbols`, `spec unmapped`

## Why

6a-2 (`symbol-index.md`) stores every item, marker and mention beside the md index, but no command asked it which item sits at a name or `path:line`, what binds a node, or where code has no spec (05 s5.1; :158's suffix index): agents still read files. This slice answers the three on the CLI, the ground 6b-1 mirrors in MCP (`code-tools.md`) and 7b's `--symbol` reuses (`impact.md`).

Split from 6a-2 (its Q23) so the reads neither waited on nor shared iterations with its incremental-equals-rebuild property test. Order (08 s2): after 6a-2, right before 6b-1. No ADR: 6a-2's apply (ADR-0008, ADR-0020; ADR-0046: `architecture-code.md#sync-in-reads` lists `symbols`, `unmapped`; ADR-0047: `#health`'s mapping rule reused, health never reads this index).

## Description and interactions

Canon: `docs/canon/symbol-reads.md` (all of it); `symbol-index.md` opening, "Crates and seams", "Guarantees", "Known limits"; the CLI, store and code READMEs; `code-tools.md`, `impact.md`, 05 s5.1 pointers.

## Data

### Lookup

`docs/canon/symbol-reads.md` "Lookup" (WA-8, Q27).

### spec symbols

`docs/canon/symbol-reads.md` "spec symbols" (WA-11, Q4, Q35).

### spec unmapped

`docs/canon/symbol-reads.md` "spec unmapped" (WA-12, WA-13, Q7, Q18, Q35).

## Rules and edge cases

- WHEN `symbols` or `unmapped` run from two roots, cwds, creation orders or CRLF copies, output SHALL be byte-identical; no order follows rowid.
- WHEN a marker's ID is dangling, a look-alike, a queue ID or another project's, it SHALL map nothing, at any containment depth (Q18); a mention SHALL never map (Q7).
- Only the data directory is written; genre: canon "Crates and seams".

## Acceptance criteria

rust-developer:

- [x] RD-1 -- lookup (Q27). M: string split -> `trait_owner_with_a_path_is_one_segment`; a scan -> `symbol_lookup_uses_the_suffix_index` (`EXPLAIN QUERY PLAN`), `symbol_lookup_cost_is_sublinear` (10k vs 100k, best of three); rowid order -> `lookup_order_is_independent_of_insertion_order`.
- [x] RD-2 -- `symbols`. M: two forms -> `symbols_takes_exactly_one_form`; outermost first -> `at_line_lists_innermost_first`; exit 0 without `[code]` -> `symbols_without_code_table_is_exit_2`; a key dropped (`levels`, `note` too) -> `symbols_json_every_key_present`; a Q35 type changed (`percent` a string, `tier` set outside QUERY) -> `symbol_reads_json_types_are_pinned`; REF naming nothing as a `CliError` -> `symbols_node_naming_nothing_keeps_its_document`; X1, X2: only resolved or only the node's markers -> `node_lists_a_look_alike_target_and_every_direct_marker`.
- [x] RD-3 -- `unmapped`. M: no containment -> `module_marker_maps_its_items`; mentions map -> `mentions_never_map`; percents averaged -> `unmapped_counts_items`; any `id_state` maps -> `dangling_marker_never_maps`.
- [x] RD-4 -- M: rowid order -> `symbols_and_unmapped_are_byte_identical_across_roots_and_order`.

test-engineer:

- [x] TE-1 -- over 6a-2's `fixtures/code-index/`, `expected.json` gains each `symbols` form (QUERY, `--node`, `--file`, `--at`), each tier (`exact`, `suffix`, `name`; a quoted `#2`) and `unmapped`'s rows (a directory, a file, `(total)`, a file PATH's unmapped items, `-` for no unit).
- [x] TE-2 -- determinism (two roots, cwd, reverse creation, CRLF): `symbols`, `unmapped`.
- [x] TE-3 -- genre: on `spec-c` `symbols`, `unmapped` exit 2 naming `[code]`, stdout empty; the new modules in the "Rules" scan.
- [ ] TE-4 -- pilots A and B, manual, run by the owner under 6a-2's TE-8 conditions: `unmapped` per root (units, mapped, mentioned), counts only, recorded in "Implementation"; read-only proof as there.

spec-writer:

- [x] SW-1 -- this spec (split from `symbol-index.md` 2026-10-15, Q23); 08 s2 lists 6a-3; `code-tools.md`, `impact.md` pointers follow.
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/symbol-reads.md` (`symbol-index.md` had no room for three sections); `docs/index-canon.md` re-measured; `symbol-index.md` opening, "Crates and seams", "Guarantees", "Known limits"; the CLI, store, code READMEs; `code-tools.md` (pointers, "Carry-overs"), `impact.md`, `health-code.md` (X3), 05 s5.1, 08 s2; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

The tables, the refresh, `refs` (6a-2); MCP `find_symbols`, `unmapped` (6b-1); 7b's `--symbol`; `--unbound` (Q4); fuzzy search, references, call graphs; HTTP, UI (Q8).

## Open

Decided under the owner's pre-approval: Q4 no `--unbound` (`spec unmapped FILE`); Q7 containment, a RON file one unit, mentions apart; Q18 only resolved, alias or ambiguous-holder markers map, as health row 5; Q27 `query_name` in `specengine-code`, the one `::` reader; Q35 the JSON types and exports pinned before 6b-1. Review 1 (2026-10-16): X1 `--node` follows `refs`: a target whose only marker is a look-alike is listed with its `id_state`, `unmapped` never counts it; X2 `--node` lists every direct marker of a target; X3 a `cannot_verify` marker maps nothing (`health-code.md` row 5 aligns at its refresh); X4 one read snapshot per command, carried to 6b-1. Q-numbers shared with `symbol-index.md` and `code-tools.md`.

## Implementation

| Module | What it does |
|---|---|
| store `symbols.rs`, `lib.rs` | `SymbolTier`, `SymbolHit`, `LOOKUP_SQL`; `lookup_symbols`, `file_symbols` |
| store `code.rs` | `BoundSymbol.ord`, `StoredSymbol.{start_byte, end_byte, layer}`; a no-op refresh writes nothing |
| code `naming.rs`, `lib.rs`; `markers`, `bind`, `hash`, `qpath` | `query_name`; `placed_name_file` -> `PlacedNames`; five `ALL` consts |
| CLI `symbols.rs`, `unmapped.rs` | the two commands and their library types; `place`, `reached`, `Seeds` for 7b |
| CLI `main.rs`, `lib.rs` | the subcommands (an `ArgGroup` of the four forms), exports, `Outcome::{Symbols, Unmapped}` |
| CLI `refs.rs`, `corpus.rs`, `code_index.rs` | one DB handle (`indexed_in`); `STORED_WORDS` from the `ALL` consts; `placed_name_file` once per file; `dir_text` keeps `\` on Unix |
| store `schema.rs`, `dump.rs`, `index.rs`; eval `code_index.rs`, `main.rs`; CLI `refresh.rs` | comments re-pointed to canon `symbol-index.md` |

Deviation: `--limit` refuses the other forms with clap `conflicts_with_all`, because clap 4.6.7 did not enforce `requires = "query"` on the positional list (`--node X --limit 5` answered 0). The developer's readings R1-R14 and the tester's L1-L28 are in the canon; X1-X3 decided them where they differed (R2, R5 against L19; L13). 6a-2's carry-overs are done. Review 1's nits and X4 shipped with 6b-1 (`code-tools.md`). The plain `"Lookup"` citations in code keep this file's "Lookup" heading as a pointer until CLI `symbols.rs:1`, the last, is re-pointed (`lock-levels.md` "Carry-overs").

Tests, 1 round: 2313/2315 (`import_cli`, `layout_cli` red only until `fixtures/code-index/expected.json` is committed); 34 new, the 2 TE-4 pilot tests ignored; mutations M1-M8, M3b, M5b, M6b, S1, S2, X1 killed; "percents averaged" has no one-site mutation, pinned by `unmapped_counts_items`' golden (21.2 % against a 45.8 % mean). TE-4, the owner's pilot run, is pending.
