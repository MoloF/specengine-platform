---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-mcp, plugin]
ref: Phase 3 plan, slice 11a-3, split from 11a `layer-c.md` 2026-10-19, refreshed 2026-10-22 after the owner's TE-6 (Q1-Q5 owner pre-approved 2026-10-22, Q4 and L2 earlier; review open); 07 s1.2; 08 s2, AC-10; developer 2 iterations, review 2 accepted
shipped: 2026-10-22
adrs: [ADR-0049]
---

# layer-c-reads (11a-3): monikers in `spec symbols` and `find_symbols`

## Why

No read showed 11a-2's layer C snapshot. An agent asking about an item now sees, beside layer A's qpath, layer C's state and moniker, whether it agrees with layer A's crate and modules, and can query by it; `find_symbols` the same. Only the owner's `spec resolve` makes a snapshot; reads show it. TE-6 (`layer-c-resolve.md`) said build it. Display only, inside ADR-0049 (`architecture.md#code-identity`).

## Description and interactions

As built: `docs/canon/layer-c-reads.md`; the lookup and `spec symbols`: `symbol-reads.md`; the mirror: `mcp-code.md` "Mirrors"; plugin 0.20.0 (`plugin-files.md` "Changing it"). No other read, tool or route shows a moniker.

## Data

Canon `layer-c-reads.md` "Text", "Rows": the `  c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]` line after `sig`, the `layer c:` line, the seven `c_state` words.

### JSON

18 target keys (`c_state`, `moniker`, `agrees` after `roles`), 12 document keys (`layer_c` after `code`), null while absent and on exit 1: canon "JSON".

### Tier moniker

`exact`, `moniker`, `suffix`, `name`; five steps, the limit after the merge, a collision found only by its shown form: canon "Tier moniker".

## Rules and edge cases

Absent, stale, partial, an unknown stored state, determinism, limits (never a spawn, `layer_c_state()`, a write under the root or an exit change; `INDEX_FORMAT` 12): canon "States", the intro.

## Acceptance criteria

M: mutation -> the test turning red. Phase B, iteration 2: every criterion met; M1-M8 8 of 8 red, no survivor (iteration 1 too).

rust-developer:

- [x] RD-1 -- text, keys and tier as in "Data".
- [x] RD-2 -- CLI `join_rows` serves `resolve`'s `join_counts` and the reads.
- [x] RD-3 -- the layer C read inside the code snapshot's transaction; `Tree::of` reads the tree once.
- [x] RD-4 -- `SymbolsTier` four words, `CodeTier` three (`get_impact`'s schema 0.19.0's), the description, `QUERY_DOC`; `INSTRUCTIONS`, `TOOLS` unchanged.
- [x] RD-5 -- X0: `git diff --stat HEAD -- crates/specengine-code/src/{levels.rs,hash.rs,ron/} Cargo.lock crates/specengine-store/tests/format_history.txt crates/specengine-cli/tests/code_format_history.txt` empty (2026-10-22).

test-engineer (CLI new `tests/layer_c_reads.rs`, `layer_c_common`, `symbol_reads{,_common,_determinism}`; MCP new `common/layer_c.rs` (the snapshot by `sqlite3`, `tree_fp` per canon); new `fixtures/layer-c/symbols-head.json`; the rest below):

- [x] TE-1 -- the golden replay's six files tally joined 29, unmatched 0, agrees 26, disagrees 0; the collision shown twice: `rows_tally_to_resolve_counts`. M6 inline modules dropped in `join_rows` (`resolve`'s counts shift too) -> red.
- [x] TE-2 -- no snapshot: HEAD's bytes (`symbols-head.json`, 44 runs, after `clear_code` too), new keys null: `symbols_text_is_unchanged_without_a_snapshot`. M1 `c_state.unwrap_or("unseen")` -> red.
- [x] TE-3 -- `Cargo.lock` edited: every line `| stale` (`stale_snapshot_marks_every_c_line` pins "Data"'s example); a byte appended: `c unseen | stale`, the moniker not found (`changed_file_rows_are_unseen`). M2 state forced fresh, M3 no BLAKE3 compare -> red.
- [x] TE-4 -- the tier after `exact`, fresh and stale; the shown collision form finds one item: `moniker_query_ranks_after_exact`, `shown_collision_finds_one_item`. M4 after `suffix`, M5 raw matching -> red.
- [x] TE-5 -- the `c` line after `sig`, before the roles (`code-bevy`): `c_line_follows_sig_before_roles`.
- [x] TE-6 -- R12 split: `layer_c_outputs.rs` `EQUAL` drops `symbols`, adds `impact`, `impact --symbol <moniker>`, `show --bindings`, `lock diff`; MCP `get_impact`, `check_binding` for `find_symbols` (`mcp_layer_c.rs`). M7 the moniker search in `lookup_in` -> `outputs_equal_with_and_without_a_snapshot` (`impact --symbol` changed); M8 `symbols()` calls `resolve` -> `reads_mcp_check_gate_verify_never_spawn_layer_c`, `only_resolve_spawns_layer_c`: red.
- [x] TE-7 -- `mcp_code.rs`: CLI parity fresh and stale, four tiers on `find_symbols`, three on `get_impact`, the enums; `mcp_genre`; `PINS` 0.20.0 `bf709371…72e1`, `SCHEMA_PINS` 0.20.0 `225e45ef…8703`, `as_of_0_19_0` hashing to 0.19.0's pin, `get_impact`'s seed tier 0.19.0's word for word; `daemon_keys` 81.
- [x] TE-8 -- `symbols_with_a_snapshot_are_byte_identical_across_roots_and_order`; store `moniker_lookup_searches_its_index` (a SEARCH under `EXPLAIN QUERY PLAN`).
- [x] TE-9 -- `layer_c_reads_cost` (`#[ignore]`, release, warm, median of 5, ra-mini): `symbols --file arena/src/lib.rs` 29.6 -> 27.1 ms, `symbols Health` 25.7 -> 22.8 ms: +0.0, below noise, limit 10 ms.
- Also `a_limited_query_keeps_its_targets_and_c_lines`, `an_unknown_stored_state_degrades_the_read`, `gaps_show_their_states`, `c_lines_count_toward_the_output_cap`, `reads_leave_the_snapshot_and_the_worktree_untouched`; `layer_c_scan.rs` the R7 split scan, `reads_never_call_the_public_layer_c_state`.
- Gates: `INDEX_FORMAT` 12; final run: fmt, clippy (`probes` too) clean; nextest 3 903 of 3 905, 71 skipped (eval guards `import_cli`, `layout_cli`: `symbols-head.json` uncommitted); MCP `probes` 97/97; UI lint 0, `pnpm test` 2 546/2 546, build clean; export, check, parity clean; eval quartet 86/86; `Cargo.lock` unchanged.

spec-writer:

- [x] SW-1 -- the refresh, 2026-10-22; RM-0010's scope.
- [x] SW-2 -- TE-6's line in `layer-c-resolve.md`.
- [x] SW-3 -- new Tier 2 `docs/canon/layer-c-reads.md`; `layer-c.md` (intro, "Tables", "Freshness", "Join"); `symbol-reads.md`; `mcp-code.md`; `plugin-files.md`.
- [x] SW-4 -- 07:30, 07:106; 08:42, :45, :64, AC-10; RM-0010 shipped; RM-0022's figure.
- [x] SW-5 -- READMEs CLI, store, code, MCP, ra; `CLAUDE.md` State; export, check clean.

### Owner check

Read-only zsh, this slice's release `spec`, `specengine-ra` installed; per pilot: `H` a scratch home, `P` the root, `C` a config with `[code]` (its own, else a scheme copy with one), `F` a walked `.rs` file.

```zsh
export HOME="$H"
cd "$P"
s() { spec --config "$C" "$@"; }
J="$HOME/files.json"
git status --porcelain > "$HOME/st"
s resolve --timeout 600
s symbols --file "$F"
git ls-files -co --exclude-standard '*.rs' | while read -r f; do s --json symbols --file "$f" < /dev/null 2> /dev/null; done > "$J"
jq -s 'def t(f): map(f | tostring) | group_by(.) | map({(.[0]): length}) | add; [.[].symbols[]] as $s | {layer_c: .[0].layer_c, omitted: (map(.omitted.symbols // 0) | add), c_state: ($s | t(.c_state)), agrees: ($s | t(.agrees))}' "$J"
jq -rs '.[].symbols[] | select(.agrees == false) | [.path, .start_line, .qpath, .moniker] | @tsv' "$J"
M=$(jq -rs '[.[].symbols[].moniker | select(. != null and contains(" #"))][0]' "$J")
R=$(jq -rn --arg m "$M" '$m | split(" #")[0]')
s --json symbols "$M" | jq -r '.symbols[] | [.tier, .qpath] | @tsv'
s --json symbols "$R" | jq -r '.symbols[] | [.tier, .qpath] | @tsv'
git status --porcelain | diff "$HOME/st" -
```

Expected: `resolve` exit 0, `stored fresh`; `c` lines after `sig`, before roles, last `layer c: fresh (ra_ap <v>)`; `layer_c` `fresh`, `files`, `items` as `resolve` printed; no `unseen`, `unmatched`; counts plus `omitted` = its `items`; with `omitted` 0, `c_state` = its `by_state`, `true`, `false` = its `agrees`, `disagrees`; one `false` row per disagree (Q5); `M` one row, tier `moniker`; `R` none (Q3); the diff empty.

## Out of scope

Monikers in any other read or tool; the daemon, UI, hook, CI, `specengine-ra`; `spec resolve`, its tables, freshness; fixing `agrees`; plugin text; 11b (RM-0022, not triggered), 11c.

## Implementation

| Module | What it does |
|---|---|
| code `layer_c.rs` | `raw_moniker`: a query up to `COLLISION_SEPARATOR`; the CLI holds no `#` |
| store `layer_c.rs` | `LayerCRead`, `layer_c_read_in` over `head_in`, `tree_rows_in`, `resolve_files_in`, `found_in` (`FOUND_SQL` `INDEXED BY code_monikers_by_moniker`) and the picked files' rows and symbols |
| store `code.rs`, `lib.rs` | `code_snapshot_with_layer_c(symbols, kinds, moniker, pick)`, `CodeSnapshot.layer_c`; `code_snapshot` unchanged, so `impact`, `unmapped` and `resolve`'s counts never read layer C |
| CLI `layer_c.rs` | `Tree::of`, `Tree::state_of` (for `state_in`); `join_rows` over `join_file`; `LayerCView::of` (`unmatched`, `unseen`; of two A rows on one C row the first by `ord` gives the discriminator); `UnknownState` |
| CLI `symbols.rs`, `lib.rs` | `pick` (QUERY: the first `limit` hits' files plus the found files; a promoted hit only moves up), the role rows beside only the found items' symbols on equal start and end; `QueryTier`, `merged_hits` (the limit after the merge); the `c` and `layer c:` lines; `SymbolsLayerC`, re-exported |
| MCP `mirror.rs`, `code.rs` | `SymbolsTier` (4), `SnapshotState` (7), `SnapshotFreshness`, `SymbolsLayerC`; `CodeTier` and its doc unchanged; the description 1 957 B, `QUERY_DOC` |
| `plugin.json` | 0.20.0 |

Deviations: (1) iteration 2, from review: a picked file's joined row holding a state this build does not know hides the snapshot from that read (no `c` lines, `layer c:` line, tier; keys and `layer_c` null) with one ``warning: the layer C snapshot holds an unknown state `<x>`; run `spec resolve` ``, the first by path then `ord`, exit unchanged; nothing is blocked, the snapshot advisory. (2) RD-5's literal baseline 5b79b81 already differs at HEAD (`levels.rs` +73/−8 from d45dfce, e07c352): X0 checked against HEAD. (3) `INDEX_FORMAT` is 12, not 11 (roadmap-items).

Review 1: a major (`CodeTier`'s doc leaking into `get_impact`'s schema), the `pick` cost, the unknown state: fixed in iteration 2. Later: every read builds the whole project's run and current maps and `TargetCrates`. Draft risks the tests cite: R1 the separate enum, R2 the tier outside `lookup_in`, R3 the R12 split, R7 no `#` in the CLI.
