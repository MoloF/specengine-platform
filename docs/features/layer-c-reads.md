---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-mcp, plugin]
ref: Phase 3 plan, slice 11a-3, split from 11a `layer-c.md` 2026-10-19, refreshed 2026-10-22 after the owner's TE-6 (Q1-Q5 owner pre-approved 2026-10-22, Q4 and L2 earlier; review open); 07 s1.2; 08 s2, AC-10
adrs: [ADR-0049]
---

# layer-c-reads (11a-3): monikers in `spec symbols` and `find_symbols`

## Why

No read shows 11a-2's layer C snapshot. An agent asking about an item sees, beside layer A's qpath, layer C's state and moniker, whether it agrees with layer A's crate and modules, and can query by it; `find_symbols` the same. Only the owner's `spec resolve` makes a snapshot; reads show it. TE-6 (`layer-c-resolve.md`) says build it. Display only, inside ADR-0049 (`architecture.md#code-identity`).

## Description and interactions

- **`spec symbols`**, every form: "Data" while a snapshot exists; without one, today's text byte for byte.
- **`find_symbols`**: the mirror key for key (`mcp-code.md` "Mirrors"), its own four-word tier enum; `CodeTier` keeps three (`mirror.rs:1294`; R1). The description (1 755 of 2 048 B) gains the tier and the `c` facts; `QUERY_DOC`: "A qualified path, a suffix of one, a bare name or a layer C moniker, as one value." `INSTRUCTIONS`, `TOOLS` (22), other tools unchanged.
- **Plugin**: a key a tool returns is a MINOR (`plugin-files.md` "Changing it"): the next free one after review-form's 0.19.0, at shipping (0.20.0 if that commits first, A1), a `PINS` line, a `SCHEMA_PINS` entry; `read-spec` unchanged.

## Data

### spec symbols

`spec symbols 'arena impl#[Health] #2'` on `fixtures/ra-mini`, stale:

```
arena::impl Health #2 | impl_item | arena/src/lib.rs:23-27 | layer a | mentions 0 | moniker
  sig impl Health
  c moniker | arena impl#[Health] #2 | agrees | stale
symbols: 1 of 1; code files 6 (.rs 6, .ron 0), skipped 0
layer c: stale (ra_ap 0.0.352); spec resolve refreshes it
```

- **`c` line**: `  c <c_state>[ | <moniker>][ | agrees|disagrees][ | stale]`, right after `  sig …`, before roles and bindings (`symbol-reads.md` "spec symbols"); item targets only; fields through `one_line`; counted toward `OUTPUT_CAP_CHARS`.
- **`layer c:` line**, after the summary: `layer c: fresh (ra_ap <v>)`, or the stale form above.
- **`c_state`**: the joined row's state, `moniker`, `local`, `none`, `unresolved` or `not_loaded` (`join_file`, `layer-c.md` "Join"); `unmatched`, the file unchanged since the run and no row joins; `unseen`, the file not in `code_resolve_files` or its BLAKE3 changed.
- **`moniker`**: the shown form (`shown_moniker`), `<raw> #<value>` for a collision; null unless the state is `moniker`. **`agrees`** per "Join"; null prints no word.

### JSON

- Each target, after `roles`: `c_state`, `moniker` (string or null), `agrees` (bool or null); `SYMBOL_KEYS` 15 -> 18; null while absent (never the word `absent`) and on `module`, `ron`.
- Top level, after `code`: `layer_c`, null or `{state, protocol, ra_ap, host, toolchains, degraded, workspaces, files, items}`: `state` `fresh` | `stale`, the rest `code_resolve`'s columns (`layer-c.md` "Tables"), `toolchains` a list; `SYMBOLS_KEYS` 11 -> 12. Exit 1: `layer_c` null, as `code`.

### Tier moniker

QUERY only; order `exact`, `moniker`, `suffix`, `name` (`symbol-reads.md` "Lookup"): (1) raw = the query up to its first `layer_c::COLLISION_SEPARATOR` (`" #"`, `specengine-code`'s; R7); (2) `code_monikers` rows with `moniker` = raw, through `code_monikers_by_moniker`; (3) rows of `unseen` files dropped; (4) joined to layer A rows with `join_file`; (5) kept where the shown moniker equals the query. Each item once, at its best tier, ordered as today; the limit after the merge; a stale snapshot answers too. The store adds one raw-text search, the CLI merges; `SymbolTier`, `LOOKUP_SQL`, `lookup_in` unchanged (shared by `impact.rs:450`, store `roles.rs`; R2). Q3: a raw query whose moniker collides finds nothing here; only the shown `<raw> #<v>` does.

## Rules and edge cases

- **Absent** (never resolved; dropped by `clear_code` or an `INDEX_FORMAT` change): WHEN absent, the text SHALL be today's bytes, the new keys null, no moniker statement run.
- **Stale** (`tree_fp` mismatch, `layer-c.md` "Freshness"): values kept, every `c` line ends `| stale`, changed files' rows `unseen`; the tier answers (R9).
- **Partial**: a reply ending before `done` is never stored, a failed `spec resolve` keeps the last snapshot; gaps in a complete one (A3): `unseen`, `unmatched`, `unresolved`, `not_loaded`, `layer_c.degraded`.
- **Determinism**: today's row order; values only from stored rows, stamp and tree bytes, read in `code_snapshot`'s transaction, the state from that read (`state_of`).
- **Limits**: reads never spawn `specengine-ra` or call the public `layer_c_state()`, write only the data dir, keep every exit code; `INDEX_FORMAT` 11 (A4). A moniker never binds, locks, judges or becomes a finding.

## Acceptance criteria

M: mutation -> the test turning red.

rust-developer (CLI `src/symbols.rs`, `src/layer_c.rs`; store `src/code.rs`, `src/layer_c.rs`, `src/lib.rs`; code `src/layer_c.rs`; MCP `src/mirror.rs`, `src/code.rs`; `plugin.json`):

- [ ] RD-1 -- text, keys and tier as in "Data".
- [ ] RD-2 -- one per-file join, factored out of `join_counts`, serves `resolve`'s counts and the reads.
- [ ] RD-3 -- the layer C read inside the code snapshot's transaction; `layer_c_state()` reads the tree once.
- [ ] RD-4 -- MCP as described.
- [ ] RD-5 -- X0: `git diff --stat 5b79b81 -- crates/specengine-code/src/{levels.rs,hash.rs,ron/}` empty; `Cargo.lock`, `INDEX_FORMAT` 11, `format_history.txt` unchanged.

test-engineer (CLI new `tests/layer_c_reads.rs`, `layer_c_common`, `symbol_reads_common/mod.rs` (18/12 keys), `symbol_reads{,_determinism}.rs`, `layer_c_{outputs,scan}.rs`; store `src/symbols.rs` tests; MCP `mcp_code.rs`, `mcp_layer_c.rs`, `plugin_files.rs`; replies in `fixtures/layer-c/`; snapshots by 11a-2's Perl fake on a scratch `fixtures/ra-mini`; `specengine-ra` never built):

- [ ] TE-1 -- the golden replay's six files: `c` lines tally joined 29, unmatched 0, agrees 26, disagrees 0; the collision shown twice as `… #<v>`. M6 inline modules dropped -> `rows_tally_to_resolve_counts`.
- [ ] TE-2 -- no snapshot: every form's text HEAD's bytes, new keys null. M1 a `c` line while absent -> `symbols_text_is_unchanged_without_a_snapshot`.
- [ ] TE-3 -- `Cargo.lock` edited: every line `| stale`, `layer c: stale …`; a byte appended to a file: its rows `c unseen | stale`, its moniker not found. M2 state unread -> `stale_snapshot_marks_every_c_line`; M3 no BLAKE3 check -> `changed_file_rows_are_unseen`.
- [ ] TE-4 -- the tier after `exact`, fresh and stale; the shown collision form finds one item. M4 after `suffix` -> `moniker_query_ranks_after_exact`; M5 raw matching -> `shown_collision_finds_one_item`.
- [ ] TE-5 -- the `c` line after `sig`, before the roles (`code-bevy`, `frameworks` set).
- [ ] TE-6 -- R12 split (R3): `layer_c_outputs.rs` `READS`' equal set drops `symbols`, adds `impact PERK-REGEN`, `impact --symbol '<a moniker>'`, `show --bindings PERK-REGEN`, `lock diff PERK-REGEN`; `mcp_layer_c.rs` `calls()` swaps `find_symbols` for `get_impact`, `check_binding`; R5's never-spawn set keeps both. M7 tier in `lookup_in` -> `outputs_equal_with_and_without_a_snapshot`; M8 `symbols.rs` calls `resolve` -> `reads_mcp_check_gate_verify_never_spawn_layer_c`, `only_resolve_spawns_layer_c`.
- [ ] TE-7 -- `code_tools_equal_the_cli` (with a snapshot), `mcp_genre` green; `PINS`, `SCHEMA_PINS` appended (`tool_schemas_follow_the_plugin_version`); `get_impact`'s schema the previous version's; `daemon_keys.rs` 81.
- [ ] TE-8 -- two roots, reversed creation: identical bytes; the moniker statement a SEARCH under `EXPLAIN QUERY PLAN`.
- [ ] TE-9 -- `layer_c_reads_cost` (`#[ignore]`, release, warm, median of 5), ra-mini: `symbols --file arena/src/lib.rs`, `symbols Health`; a snapshot adds ≤ 10 ms.

spec-writer:

- [x] SW-1 -- this refresh, 2026-10-22 (R1-R3, R8, Q1-Q5); RM-0010's scope gains `crates/specengine-code`.
- [x] SW-2 -- TE-6's line in `layer-c-resolve.md`.
- [ ] SW-3 -- canon at shipping: `layer-c.md` l.11 ("no read shows the snapshot yet"), l.110 with TE-6's finding, the reads' contract (R4); `symbol-reads.md` "Lookup" tier row, "spec symbols" pointer, keys; `mcp-code.md` "Mirrors", "Texts", "Tests"; `plugin-files.md`, `plugin.md` version.
- [ ] SW-4 -- 07:30, 07:106; 08:42 (TE-6 done), 08:45 (11b not triggered, the figure), 08 AC-10 (9.1 / 10.7 s, 2.76 / 3.46 GiB); RM-0010 shipped; RM-0022's body the figure (Q1).
- [ ] SW-5 -- READMEs CLI, store, code, MCP, `crates/specengine-ra/README.md` l.56 (the figure); `CLAUDE.md` State; export, check clean.

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

Monikers in any other read or tool; the daemon (no route serves `spec symbols`; `fixtures/daemon-keys.json` 81), UI, hook, CI, `specengine-ra`; `spec resolve`, its tables, freshness; fixing `agrees`; plugin text; 11b (RM-0022), 11c.

## Decisions and risks

- **Q1** RM-0022 stays `draft`, its body 0.01 % and 0.25 %, re-measured on a new pilot. **Q2** `impact --symbol`, `get_impact` keep three tiers (canon). **Q3** "Tier moniker". **Q4** a `c` line per item (~+50 B; snapshots opt-in). **Q5** A's 17 disagrees: no change, listed by the owner check; a follow-up only if a rule is wrong. **L2** the next MINOR here.
- **A1** review-form commits first (0.19.0, `TOOLS` 22, pins). **A2** the `r#` reading checked on both pilots. **A3** partial: gaps in a complete snapshot. **A4** no index or format change. **A5** eval's totals are its own scan's.
- **R1** `CodeTier::Moniker` would change `get_impact`'s outputSchema. **R2** the tier in `lookup_in` would seed `impact`, role links. **R3** two shipped tests red by design (TE-6). **R4** `symbol-reads.md` 11 692, `layer-c.md` 12 268 of 12 288 B: a pointer, likely a new Tier 2 `layer-c-reads.md`. **R5** description 293 B left, `INSTRUCTIONS` none. **R6** fewer targets fit the cap. **R7** no `#` literal in the CLI. **R8** stale draft pointers dropped (:220, 07:104, "0.19.0 now", S6, S7). **R9** while editing, mostly stale. **R10** shared with review-form: `plugin.json`, `tests/plugin_files.rs`, `src/mirror.rs`, `tests/common/read.rs`, MCP, CLI `src/lib.rs`; `plugin-files.md`, `mcp-code.md` "Tests", `plugin.md`; `CLAUDE.md` State; 07 s1.2; 08.
- **Needs**: 11a-2, TE-6 (done), review-form (RM-0012) committed. From 11a-2's review: `tree_files` to walk deepest-first, stopping at the first known directory.

## Implementation

Not started.
