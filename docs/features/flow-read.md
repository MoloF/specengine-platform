---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0043; Phase 7 step 2 (08 s2); owner's decisions 2026-10-30, pre-approved, review open
adrs: [ADR-0047, ADR-0055]
---

# flow-read: `spec flow`, typed edges, lifted edges and gap rows (RM-0043)

## Why

`spec graph` walks from one REF and knows no groups or gaps. One flowchart is not enough either: sequence, reuse, supply and requirement mean different things, so the read returns typed edges and a screen picks what it draws. Gaps are computed rows; they stop nothing (ADR-0012, ADR-0047).

## Summary

`spec flow [ROOT] [--depth N] [--json]`, MCP `get_flow {root?, depth?}` and the daemon's `GET .../flow?root=&depth=`: the walk over `follows`, typed edges in reading direction, edges lifted with counts when a group folds, and ten gap rows that stop nothing. How it works now, with its output, `[flow]`, the row table and the readings: `docs/canon/spec-flow.md`.

### Output

Gap rows first, all ten in table order, then nodes and edges. JSON, every key present, absent `null`: `{root, reason, depth, notes, left_out, truncated, omitted_nodes, omitted_edges, gaps, nodes, edges}`; a gap `{code, state, count, items, cause}`; a node `{id, kind, title, path, line, distance, group, start, end, holds, inner}`, `inner` `[{type, count}]`; an edge `{from, to, type, kind, when, count, path, line}`. The text lines, the cap and the exits: `docs/canon/spec-flow.md` "Output".

### Depth

`--depth` counts from the roots (0), as `spec tree`. A document deeper than N shows as its ancestor at N. A shown node's `distance` is the least of what it holds, itself included, null only when nothing it holds is reached; `start` and `end` stay its own. `--depth` never changes a gap row.

### Steps

Steps: `[[flow.step]]`'s, every document holding or receiving `follows`, the starts and ends. A live document's `follows` that does not resolve (dangling, another project's) gives no edge and keeps the document a step; an unresolved `uses`, `supplies` or `depends_on` does not. A step whose only `follows` is unresolved, with no marked start, is a derived start with distance 0.

`unjoined-parallel` walks only splits of two or more branches, breadth-first, one step per branch in turn, stopping at the first step all branches reached; a one-branch split is listed with no walk; a split with no join still walks each branch to its end.

## Acceptance criteria

- [x] AC-01 — The walk. `spec flow --root fixtures/flow-relay`: exit 0, first line `flow all | nodes 8, edges 10, depth -`; LEG-START, LEG-DRAW, LEG-RUN, LEG-FALL, LEG-SCORE, LEG-END at distance 0 to 4, then KIT-OIL, KIT-TORCH at `-`; seven rows `not_configured`, three `measured` at 0; identical bytes from copies in opposite file orders. M1: without the sort nodes fall back to path order, so the copies still match and the order pins go red.
- [x] AC-02 — Typed edges. `--json` on flow-gaps holds all four types; a `follows` edge has `from` its target, any other `from` its source. M2: `from` always the link's source turns `LEG-START -> LEG-DRAW` red.
- [x] AC-03 — Planted gaps. `spec flow WING-BROKEN --root fixtures/flow-gaps`: exit 0, rows in order at `0, 1, 1, 2, 1, 1, 1, 3, 1, 1`: `two-starts` LND-A + LND-B; `unreachable` LND-STRAY; `no-way-to-end` LND-DEAD, LND-STRAY; `trap-cycle` LND-TRAP-A + LND-TRAP-B; `unjoined-parallel` LND-B; `cycle-no-repeat` LND-SPIN-A + LND-SPIN-B; `definition-unused` PLT-FEED-A, PLT-FEED-B, PLT-UNUSED; `definition-once` PLT-ONCE; `supply-unfed` PLT-FEED-A + PLT-FEED-B. `spec flow WING-HEADLESS`: `no-start` names it, `unreachable` `cannot`.
- [x] AC-04 — Loops are not gaps. `spec flow WING-LEND`: ten rows `measured` at 0, LND-OPEN `end` with its `repeat` edge listed; flow-relay with a scratch `[flow]` by `paths`: the same. M3: keeping `repeat` edges in `cycle-no-repeat` turns both red. M4: calling every cycle a trap turns WING-LEND red.
- [x] AC-05 — Lifted edges. `spec flow WING-LIBRARY --depth 1`: `nodes 4`, one edge `WING-LEND -> WING-BROKEN | follows | x2`. At depths 0, 1, 2 and unbounded, per type the edges' `count` plus the nodes' `inner` is equal, and the gap rows are byte-equal. M5: counting a merged pair once gives `x1`.
- [x] AC-06 — Genre. The same binary answers on flow-relay, flow-hire, flow-gaps and `spec-c`, the last `nodes 0`, exit 0; `check_genre.rs`, `no_domain.rs` and the CLI and MCP genre scans pass over the new sources. M6: a literal kind in core `flow.rs` turns the scan red.
- [x] AC-07 — Surfaces. `get_flow` equals the CLI, meeting its schema; `mcp_door` and `read_only.rs` pass; the daemon's route is uncut. A generated corpus past 40 000 characters: text cut at a line with the tail, every gap row printed, JSON the printed items. M7: JSON keeping edges past the cut turns it red.
- [x] AC-08 — Config. No `[[flow.end]]`: its two rows `not_configured` with the cause. A rule with no selector or an unknown key: those rows `cannot`, naming `<config>:<line>`, exit 0, `spec check` and `spec tree` unchanged. A `when` on a scratch key selects its document. M8: `not_configured` printing 0 `measured` turns it red.

## Verification

Mutations: 10 run and killed, none survived. M1 to M8 stand from iteration 1, on the earlier source; M9 and M10 ran on iteration 2.

| Mutation | Criterion | Tests red |
|---|---|---|
| M1 | AC-01 | 2 |
| M2 | AC-02 | 22 |
| M3 | AC-04 | 9 |
| M4 | AC-04 | 8 |
| M5 | AC-05 | 3 |
| M6 | AC-06 | 1 |
| M7 | AC-07 | 2 |
| M8 | AC-08 | 4 |
| M9, an unresolved `follows` drops its document | Steps | 2 |
| M10, a folded node takes only its own `distance` | Depth | 1 |

No hand-worked value was wrong. Unpinned: cause wording beyond the `no [[flow.<table>]] rule: ` prefix; three or more `parallel` branches.

Iteration 2: CLI `flow_read.rs` 45 tests, core 13; `a_folded_node_keeps_its_own_distance` became `a_folded_node_takes_the_least_distance_it_holds`. Chained run, `cargo nextest run -p specengine-core -p specengine-cli -p specengine-mcp -p specengine-http`: 2883 passed, 37 skipped; clippy `--all-targets` and fmt check 0; `daemonKeys.test.ts` 287 of 287.

Gates, final run: fmt, clippy, probes clippy 0; `cargo nextest run --workspace` 4108 run, 4107 passed, 71 skipped, the one failure `doc_pointers` `every_cited_heading_exists_verbatim_on_one_line`, fixed by restoring this file's "Output" and "Files in scope"; UI lint 0, vitest 2557 passed, build 0; parity after docs 3 of 3.

## Out of scope

The canvas (RM-0044); stages (RM-0048); `pack:`, overrides (RM-0058); `--type`, `--archive`; silencing `unknown-key`; a resolved-edge table and the read's cost in `parent:` depth (RM-0054).

## Implementation

| Area | What it does |
|---|---|
| Model | `FLOW_LINK_TYPES`, with `USES` and `SUPPLIES` consts |
| Config | Core `flow_toml.rs` reads `[flow]` table by table and never fails: a table's first error by line stays on it, an unknown key in `[flow]` marks all four. Selectors share `health_toml.rs`' reader and `check/health.rs`' judge (`documents_selected`) |
| Read | Core `flow.rs` `flow_read` takes resolved links from `SpecGraph::edges`, one admitted holder per link, builds the scope and steps, computes the ten rows before any fold, then folds by depth. Tarjan is iterative. A shown node's `distance` is the least of what it holds, itself included, null only when nothing it holds is reached; `start` and `end` stay its own. A live document's `follows` that does not resolve (dangling, another project's) gives no edge and keeps the document a step. `unjoined-parallel` walks only splits of two or more branches, breadth-first, one step per branch in turn, stopping at the first step all branches reached; a one-branch split is listed with no walk |
| CLI | `flow.rs` resolves ROOT, caps and renders, and owns the causes' wording; `GAP_ITEMS_SHOWN` is 50, for text and capped JSON |
| MCP | `get_flow` with the closed `FlowDocument` mirror, `FLOW_DESCRIPTION` and `FlowNode.distance` saying the same; `INSTRUCTIONS` 2 048 B |
| Daemon | The route, `View::Browser` |
| Plugin | 0.21.0; `read-spec` names `get_flow` in step 2 |

### Files in scope

Developer: model `src/link.rs`, `src/lib.rs`; core `src/flow.rs`, `src/flow_toml.rs` (new), `src/health_toml.rs`, `src/check/health.rs`, `src/check/mod.rs`, `src/project_toml.rs`, `src/lib.rs`; CLI `src/flow.rs` (new), `src/lib.rs`, `src/main.rs`; MCP `src/read.rs`, `src/mirror.rs`, `src/server.rs`; http `src/app.rs`; `plugin/specengine/.claude-plugin/plugin.json`, `plugin/specengine/skills/read-spec/SKILL.md`. UI developer (ADR-0033's exception, no screen): `ui/src/api/provisional.ts`, `ui/src/api/daemonKeys.test.ts` (86 sets; also edited by the tester on the orchestrator's instruction). Tester: `fixtures/flow-gaps`, `fixtures/daemon-keys.json`; new core and CLI `tests/flow_read.rs`, MCP `tests/mcp_flow.rs`, http `tests/flow.rs`; core `tests/check_genre.rs`; CLI `tests/genre.rs`, `read_only.rs`, `kind_amendment.rs`; MCP `tests/common/read.rs` (`TOOLS` 23), `mcp_stdio`, `mcp_size`, `mcp_door`, `mcp_genre`, `plugin_files`, `plugin_skills`, `plugin_roles`, `mcp_decision`, `mcp_path`, `mcp_code`, `mcp_create`, `mcp_tasks`, `mcp_session`, `mcp_review_form`, `mcp_stage`, `mcp_default`; http `tests/daemon_keys.rs`, `methods.rs`, `parity.rs`, `projects.rs`; eval `tests/import_support`.

**Divergences from the draft.** `definition-*` and `supply-unfed` list what is in scope and read links over every live document, where the draft read every live document for both. `unjoined-parallel` looks beyond the scope, so a sub-flow is not called broken. A scope with no step fires `no-start`. Gap rows are never cut, so one item can pass `OUTPUT_CAP_CHARS`: a known limit, `spec-flow.md`.
