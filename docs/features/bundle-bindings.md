---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: Phase 3 slice 7a-2 of RM-0006, after 7a-1 `show-bindings.md` (shipped 2026-10-21); drafted 2026-10-10 (Q1-Q14, WA-1..WA-17), refreshed and split at 035dd7b 2026-10-21 (Q-A..Q-H under the owner's pre-approval, review open); refreshed again before it starts; 05 s5.3, s6; 07 s1.2, s2; 08 s2
adrs: [ADR-0046]
---

# bundle-bindings (7a-2): the bundle's layers 5 and 9 and the package's `bindings`

## Why

A bundle says nothing about code: layers 5 and 9 are empty (`docs/canon/spec-cli-bundle.md` "Layers"), and the package's `bindings` is always `[]` (`docs/canon/task-package.md` "Package"). An agent starting a task from `get_context_bundle` or `get_task` still has to find the code bound to its targets and the tests that verify them on its own.

Delivered here: 7a-1's rows (`docs/canon/code-reads.md`) in the bundle's layers 5 (bindings) and 9 (tests), and in the package's `bindings` with a brief section; the bundle and task descriptions; plugin 0.14.0; the UI's types and bundle panel; the daemon keys. RM-0006 ships with this slice. ADR-0046 as in 7a-1; ADR-0027 (the package stays at `schema_version` 1: a promised key filled).

## Description and interactions

- **Reused from 7a-1 (`docs/canon/code-reads.md`), unchanged**: the row ("The binding row"), the seam (`judge_set`, `set_bindings`), "Degrade", the refresh rule. The sync is `verify --node`'s by construction (Q-B).
- **An agent** gets rows in `get_context_bundle`, `get_task` and `get_task {next}`; **the owner** gets them in `spec bundle`, `spec task show` and the daemon's bundle and task JSON, and the bundle panel shows them as lines (UI-2).
- **Freshness**: `bundle` and `task show` refresh the code index once per command; `task show`'s bundle and its rows share one refresh (stale point 10: `package.rs`' `bundle_of` calls `bundle()`, which would refresh it again).
- **Crates**:
  - core: doc comments only (`check/bundle.rs` :22, :40, :45; `BundleLayer` keeps both variants);
  - CLI: `bundle.rs` (the sets from core's `BundleLayers` output, the fit in `assemble`, the tail, the JSON) and `package.rs` (the rows, the cap, the brief);
  - MCP: `mirror.rs` (layer rows, the package's row schema), `read.rs` and `tasks.rs` (the descriptions);
  - the UI; the plugin.
- **Unchanged**: every exit; `TASK_PACKAGE_SCHEMA_VERSION` 1 and its 26 keys; without `[code]`, the bundle's text and `bundle_hash` byte for byte.

## Data

### The binding row

7a-1's (`docs/canon/code-reads.md` "The binding row"): 14 keys, verify's order. A placed bundle row is the row plus `tokens_est` (15 keys).

### Layers 5 and 9

Q2-Q4. Core `bundle_layers` gives the targets, layer 4's candidates and `SpecGraph::within`; the CLI builds two sets from them, by node, never by file, and runs one judge over their union:

| # | Key, heading | Rows |
|---|---|---|
| 5 | `bindings`, Bindings | relation not `verifies`; the node a target or within one; lock-only rows included |
| 9 | `tests`, Tests | relation `verifies`; the node a target, within one, or a layer-4 candidate (placed or not) |

- **Form**: show's row line without its two-space indent, then `  sig <signature>` when not `null`; one form per row, in or out, never degraded.
- **Fit**: the canon order 1-4, 5, 6-8, 9 and the same greedy (`spec-cli-bundle.md` "Fitting"); the frame reserve's n counts every row. `tokens_est` stays the whole-body estimate, a sum of thousandths with one rounding (core `tokens.rs`); running counts only if TE-3 breaches its bound, and then summed in thousandths.
- **Left out**: a tail entry `{name: <symbol, else file>, title: "<relation> <node>", path: <file>, line: <line, else 1>, tokens_est, layer: "bindings" | "tests"}`.
- **JSON**: `layers.bindings` and `layers.tests` hold the placed rows.
- `bundle_hash` names the body, so it moves with code drift, and the package's `bundle.bundle_hash` with it (ADR-0046 "Cost").
- **No `[code]`**: both layers empty, no heading, no note.

### Task package

Q11, Q-H. `bindings` holds the rows of the targets, the `affected_nodes` and their sections. That is `task_set` without the criteria refs: layer 9 carries the criteria's tests.
- **Where**: from the reading root, as `bundle_hash` (`task-package.md` "Package"), in row order.
- **Cap**: at most `PACKAGE_BINDINGS_MAX` 64; past it one note `bindings: <n> more row(s) left out; spec show <REF> --bindings lists them`.
- **Budget**: rows count in `PACKAGE_BUDGET`; after the diffs and reference texts, rows leave from the last, with the same note.
- **Brief**: a Bindings section after Targets, the row lines; none when `[]`.
- **Schema**: `schema_version` 1. The mirror's item `{"type": "object"}` becomes the row's schema, every key required; `p2_2` gains the row's key paths.
- **No `[code]`**: `[]`, no note.

### Degrade

7a-1's table (`docs/canon/code-reads.md` "Degrade"); `bundle` and `task show` are silent without `[code]`, and give one `note: bindings: <cause>` per command otherwise. A listed row is placed or left out as a judged one, its `sync` `null`. `impact.md` (7b) degrades the same way.

### Descriptions and plugin 0.14.0

- `BUNDLE_DESCRIPTION` names the code bindings and tests among the layers.
- `GET_TASK_DESCRIPTION` (`tasks.rs`) names the brief's bindings.
- Plugin 0.14.0 (MINOR: schemas changed): `read-spec` step 1 (:14) names the bindings and tests among the bundle's contents; `PINS`, `SCHEMA_PINS` 0.14.0.

### UI and daemon keys

- **UI-1, types**:
  - `ui/src/api/provisional.ts`: `BindingRow`; the bundle row with `tokens_est`; `BundleLayers.bindings` and `.tests` as rows, not `BundleItem[]` (:928-953); `ShownNode.bindings` `{rows, omitted} | null` (7a-1's key); `TaskPackage.bindings: BindingRow[]` (:1200);
  - pins: `provisional.test.ts` (:188-198, the cited `ShownNode` list :302, :290), `daemonKeys.test.ts` (66 -> 67 types, its "sixty-six" :1001);
  - `fixtures/daemon-keys.json` gains `BindingRow`, from `daemon_keys.rs`' `health_code_repo`; `TaskPackage` stays at 26 keys.
- **UI-2, `tree/BundlePanel.tsx`**: layers 5 and 9 render rows as lines (place, binding, sync), not through `ItemRow` (:96-103), which would render `undefined` paths. Their tail entries are not linked as tree nodes (:113), so a qpath never becomes a broken tree link.
- Mocks and builders follow: `mocks/corpus.ts`, `mocks/tasks.ts`, `test/builders.ts`.

## Rules and edge cases

- WHEN a read runs from two roots, another cwd, in reverse creation order or on CRLF copies, its text, JSON and `bundle_hash` SHALL be byte-identical.
- WHEN `[code]` is absent, the bundle's text and `bundle_hash`, and `task show`'s brief, SHALL equal the bytes before 7a-2.
- WHEN a row is judged, its `sync` SHALL equal `spec verify --node`'s `state` for that key.
- WHEN 1 000 code files bind nodes outside the bundle, the bundle SHALL be byte-identical (rows by node, never by file).
- Door and genre as 7a-1.

## Acceptance criteria

M: the mutation that must turn the named test red (CLI `tests/bundle_bindings.rs` new, `bundle_fit.rs`, `package.rs`):

- [ ] RD-1 -- constant cost. `bundle_is_unchanged_by_unrelated_code`. M: rows taken by the target's file.
- [ ] RD-2 -- the sets. `bindings_layer_holds_target_rows_only`, `tests_layer_reaches_criteria_tests`. M: a neighbour's or a `verifies` row in layer 5.
- [ ] RD-3 -- parity. `bundle_sync_equals_verify_node`. M: a second state path.
- [ ] RD-4 -- the fit. `bindings_follow_criteria`, `frame_reserve_counts_binding_rows`; `bundle_fit.rs` asserts the budget bound with rows. M: rows before layer 2, or the reserve without rows.
- [ ] RD-5 -- no `[code]`. `no_code_table_bundle_is_unchanged` on `spec-a`, `spec-b` (text, `bundle_hash`, brief). M: an empty heading or a note.
- [ ] RD-6 -- degrade. `bundle_survives_a_broken_code_layer`, bundle and `task show`. M: exit 2 propagated.
- [ ] RD-7 -- the package. `package_bindings_are_capped_with_a_note`; `p2_2` with the row's paths. M: no cap, or `schema_version` 2.
- [ ] RD-8 -- mirrors. The walker over the bundle and task tools; `daemon_keys.rs`; 0.14.0 `PINS`, `SCHEMA_PINS`. M: a key without its mirror.
- [ ] UI-1 -- the types and their pins (67 types). M: a key missing.
- [ ] UI-2 -- `NodePane.test.tsx`: rows as lines, tail entries unlinked. M: rows passed through `ItemRow`.
- [ ] TE-1 -- the differential of 7a-1's TE-1 over the bundle's and package's sets.
- [ ] TE-2 -- determinism (the first rule) for bundle, `task show`, `get_task`.
- [ ] TE-3 -- scale: 1k and 10k unrelated code files leave the body identical; warm latency recorded.
- [ ] TE-4 -- genre: `spec-c` unchanged in the CLI and MCP; the P2-3 scan.
- [ ] TE-5 -- MCP: `mcp_size` maxima with rows (`PACKAGE_BUDGET + OUTPUT_CAP_CHARS` at most 500 000); `mcp_door`; the owner's pilots by hand (bundle, `get_task` against 1 s; `specengine-eval w` medians unchanged; no pilot name).
- [ ] SW-1 -- at shipping, re-measured then:
  - `docs/canon/code-reads.md` (8 937 B at 7a-1, cap 12 288): the sets, the fit, the package, the rest to `spec-cli-bundle.md`, `task-package.md`;
  - pointers in `spec-cli-bundle.md` (11 975: layers 5, 9, "Not yet"), `task-package.md` (10 606: ":32"), `mcp-read.md`, `plugin-files.md`;
  - the CLI, MCP and core READMEs; `ui/README.md` (8 177 of 10 240);
  - 05:207; `architecture-code.md#sync-in-reads`; `CLAUDE.md` State; RM-0006 shipped; this spec compacted.

## Out of scope

Tree counters (RM-0011, Q-C); `tested` (8b adds it: RM-0008 waits on RM-0007, which waits on RM-0006); layers B and C; `impact`, `delta.symbols` (7b); `spec bundle --task`, `get_context_bundle {task_id}`, the `bundles` log, `compact`; UI screens (RM-0011); evidence levels; writing markers.

## Open

Decided (owner pre-approved, review open):

| Q | Answer | The other answer costs |
|---|---|---|
| Q2 layer 5 | the targets' and within rows, not `verifies`, lock-only included | markers alone hide `marker_gone` |
| Q3 layer 9 | `verifies` rows, layer 4's criteria too; never run (ADR-0044) | misses a feature criterion's test |
| Q4 priority | canon order 1-4, 5, 6-8, 9; a row in or out | reordering the canon on a guess |
| Q11, Q-H package | the targets, `affected_nodes` and their sections | criteria refs fill the 64-row cap sooner; layer 9 carries their tests |
| Q-C tree | deferred to RM-0011, opt-in | a whole-tree judge per `get_tree` and daemon tree load |
| Q-G UI | types, plus `BundlePanel` rows as lines | types alone do not compile, or the panel renders `undefined` paths |

## Risks

- **The fit**: it re-estimates the whole body per candidate (3 000: 1.13 s, `spec-cli-bundle.md` "Open"), and a document target can hold hundreds of rows. TE-3 bounds it.
- **Size**: a placed row's JSON is up to about 1 KB (a 512 B signature, an unbounded qpath); `mcp_size` is re-measured.
- **Latency**: `get_task` refreshes the code index once, and the daemon does so per request (MCP's 1 s: `mcp-read.md` "Latency").
- **Worktrees**: package rows show the reading root's code to an agent in another worktree (accepted, as `bundle_hash`).
- **Caps**: the canon pointers have 8-73 B of room; the detail goes to `code-reads.md`.

## Implementation

Not started. To record: modules, TE-3's and TE-5's figures, divergences with reasons.
