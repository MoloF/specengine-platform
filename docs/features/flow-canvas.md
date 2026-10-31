---
class: spec
status: shipped
shipped: 2026-10-31
scope: [ui, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0044; Phase 7 step 3 (08 s2); owner's decision 11
---

# flow-canvas: the Flow screen (RM-0044)

## Why

The owner wants a diagram running forward, with branches, reuse and dependencies, folding into groups so one can be studied within the whole, and gaps visible. The daemon computes it (`flow-read.md`); this slice draws it, read-only; the AI writes, through proposals.

## Summary

The section `flow`, fourth of eight: `#/<p>/flow[/<name>]`, read-only, a canvas with collapsing groups and a List as its text twin, gap rows beside it. How it works now: `docs/canon/spec-flow-screen.md`; the read and `gaps`: `docs/canon/spec-flow.md`.

### Reads

Top `depth=0`, a group `root=G&depth=1`, one per open group; no whole-flow read, and the UI lifts nothing. `getTree` on demand is the one wide read, never to draw.

### Layout

`layout.ts`, hand-written and pure: columns from the read's `distance`, a folded node carrying the least of what it holds; arcs by columns, never `kind`; a band for what the walk misses; the cap of 200 boxes.

### Gap rows

Top's ten rows, `Not configured` and `Cannot measure` in words. Activating an item seeks its first member and marks all. A closed group shows its served `gaps`. Hide gaps collapses the side column.

### Panel

A selected box's words, text, links and ways to the Spec tree and Graph; nothing writes.

### Keyboard, motion, width

One tab stop; arrows only move, Right from a lane's last column climbing past the frame; Space opens or closes. Motion takes the tokens. Between 48 and 64rem the order is canvas, panel, gaps; under `48rem` the List comes first.

## Data

```ts
interface FlowOptions { root?: string; depth?: number }  // client.ts
getFlow(project: string, options: FlowOptions, signal?: AbortSignal): Promise<FlowView>;
```

`HttpClient` as `getGraph`: a 404 is the exit-1 document. `FlowMemory` `{tab, open}` in the Shell, per project. `specengine.flow.gaps-hidden` in `localStorage`, per viewer.

**`FlowNode.gaps`:** the documents this read's measured rows name among what the node holds, itself included; a key a tool returns, so plugin 0.22.0, its schema pinned.

**Forms** go by place in the sorted types of Top's `edges` and `inner`; `repeat` is always dotted, under a narrow exception to the `.kind` scan for `edge.kind` in `src/flow/`, the closed `FlowEdgeKind` of `mirror.rs`, an unknown value still shown raw. **The List** is the reads drawn, past the cap too.

### Mock

`ui/src/mocks/flow.ts`, harbor-sim's documents and kinds; ledger-api has no `[flow]`; `large` is 500 documents in 25 groups. Contents: `spec-flow-screen.md` "Mock".

## Acceptance criteria

Each mutation M turns its criterion red.

- [x] AC-01 — Route. `parseHash("#/a/flow/MEC-TIDES")` is a section route; `navOf` gives Flow fourth of eight.
- [x] AC-02 — Reads. Spy on harbor-sim: load calls `{depth: 0}`, `{root: "DOM-HARBOR", depth: 1}`, never `{}`; opening DOM-BERTHS adds its read; closing and reopening adds none. M1: a read with no `depth` on load.
- [x] AC-03 — Nothing lifted. An edge `x7` stubbed in Top draws `x7` with its group open. M2: a count recomputed from the group's read.
- [x] AC-04 — Layers. Boxes at `distance` 0, 4, 9 run left to right; null lies in the band, a child with no flow link too.
- [x] AC-05 — Arcs. Backward edges with and without `kind` arc, a forward one with `kind` curves, no arc point falls in a box. M3: arcing on `kind !== null`.
- [x] AC-06 — Forms. A repeat is dotted whatever types a project holds, on a curve or an arc; types and kinds draw distinct dashes, the legend naming each; `tokens.test.ts` holds 3:1. M4: the repeat's dash by index.
- [x] AC-07 — No quoted type. `policy.test.ts`'s list gains `follows`, `uses`, `supplies`; the Health scan covers `src/flow/` but `labels.ts`; the `.kind` scan passes `edge.kind` under `src/flow/`, and stays red on `node.kind` there and `edge.kind` in `src/tree/`. M5: `edge.type === "follows"` in `layout.ts`.
- [x] AC-08 — Gap rows. Ten as served; ledger-api's rows read "Not configured", cause verbatim, no digit; a stubbed `cannot` reads "Cannot measure" with its icon. M6: `count ?? 0`.
- [x] AC-09 — Gap selection. An item two levels down: its closed group shows its `gaps`; activation calls `getTree` once, runs both group reads, routes the first member, marks all, focus kept. `no-start`'s scope has no button. M7: skipping the chain.
- [x] AC-10 — Panel. Text, links, an href equal to `sectionHash(p, "tree", name)`; the spy sees reads only.
- [x] AC-11 — States and cap. Each case of `spec-flow-screen.md` "States and live", stubbed. `large` opened past 200: 200 boxes, the line, the List whole.
- [x] AC-12 — The trial. `layout.test.ts` on `flow200()` (200 boxes, 12 columns, 30 off the walk, 40 edges against it): no overlap, forward edges run right, arcs clear boxes, reversed input equal, under 500 ms; time and crossings in "Implementation".
- [x] AC-13 — Keyboard, motion, width. As `spec-flow-screen.md` says, each by a test: closing a group leaves focus on its head; under `48rem` the List first. The 320 px clause is uncovered: see "Verification".
- [x] AC-14 — Served count. `spec --json flow WING-LIBRARY --depth 1` on flow-gaps: `gaps` 1, 12, 2 on WING-LEND, WING-BROKEN, WING-HEADLESS, a document two rows name counted once; `get_flow` meets its schema. M8: counting per row.

**Owner's manual check.** `spec serve` over a flow project: fold, open, follow a gap item; judge the look at the cap with `?scenario=large`.

## Verification

Mutations: M1 to M8 and the tester's variants run, none survived. M1 ran on a copy. The developer's own 6 mutations on iteration 2 went red.

| Mutation | Criterion | Tests red |
|---|---|---|
| M1a, M1b | AC-02 | 10, 9 |
| M2 | AC-03 | 2 |
| M3 | AC-05 | 4 |
| M4a, M4b | AC-06 | 3, 3 |
| M5 | AC-07 | 2 |
| M6a, M6b | AC-08 | 6, 1 |
| M7a, M7b | AC-09 | 3, 3 |
| M8 | AC-14 | 10 |

The build warns: main chunk 527.99 kB, over Vite's 500 kB.

Uncovered: AC-13's 320 px clause needs a browser (a headless look saw `scrollWidth` 320); the owner's manual look at the cap.

**The owner's call, decision 11:** the 200-box trial passes with 1,920 forward crossings, "a hairball by eye"; real flows are sparser.

Gates, final run: fmt, clippy, probes 0; `cargo nextest run --workspace` 4120 run, 4119 passed, 71 skipped, the failure `ui_policy` `the_ui_is_a_documentation_root_with_its_readme_and_adr_in_budget`, fixed by trimming `ui/README.md` to its 8 192 B; UI lint 0, vitest 2780 passed, build 0; parity 3 of 3.

## Out of scope

`stage:`, confirmed counts and state lines on boxes (RM-0048); any write, drag or edit; a layout library.

## Implementation

| Module | What it does |
|---|---|
| `layout.ts` | Pure: reads in; absolute boxes, lanes and edges out, inputs sorted first. The cap runs Top as served, then open groups by (nesting, name), partly drawing the last group. Columns are distinct distances; a forward edge skipping a column gets a passage slot there, so it passes no box; four barycentre sweeps order rows; eight isotonic passes pull joins between branches. Columns decide the route: forward S-curves in gaps, arcs orthogonal on an own track, beside, self under the box, band. The band, ports and labels: `spec-flow-screen.md` "Layout" |
| `labels.ts` | Alone tells gap states, codes and `edge.kind` apart; colours and plain dashes go by place in the sorted types of Top's `edges` and `inner`; repeat is always dotted and heavier |
| `FlowCanvas.tsx` | One node type serves all forms; stacking is manual: frame, lane edges, lane boxes. The viewport fits until the owner moves it |
| `queries.ts` | Group reads are enabled only while open, `staleTime: Infinity`, read again on mount; a live event reads only what is open. A read answered with a `reason` stays enabled, so an event can lift the refusal |
| `FlowView.tsx` | A seek runs once per route change or gap item, first member only: `getTree` on demand, closed boxes on the chain open top down, a refused box is skipped, a chain run out ends the seek with a note, the owner's close ends it. |
| Trial | `flow200()` in `src/test/flowStub.ts` (408 edges, random wiring): median 1.9 ms, worst 3.5 ms over 20 runs, 1,920 forward crossings, no overlap, forward edges run right, arcs and band edges clear every box, reversed input equal, 4,956 x 3,952 px |
| Core, CLI, MCP | `FlowNode.gaps` counts the documents this read's measured rows name among what the node holds, itself included. `FlowGap.files` carries each row's files, not printed; `no-start`'s scope has none. The set is taken before any fold or cap, so a depth moves no document's count. The node line ends ` | gaps <g>` above 0. Plugin 0.22.0 |

### Files in scope

Developer: core `src/flow.rs`; CLI `src/flow.rs`; MCP `src/mirror.rs`; `plugin/specengine/.claude-plugin/plugin.json`. UI, new: `ui/src/flow/` `FlowView`, `FlowCanvas`, `FlowGaps`, `FlowDetails`, `FlowList`, `FlowLegend` (`.tsx`), `layout.ts`, `geometry.ts`, `labels.ts`; `ui/src/mocks/flow.ts`; `ui/src/test/flowStub.ts`. UI, changed, in `ui/src/`: `api/client.ts`, `http.ts`, `provisional.ts`, `queries.ts`; `mocks/MockClient.ts`; `app/sections.ts`, `routes.ts`, `Shell.tsx`, `shortcuts.tsx`; `ui/Icon.tsx`; `styles/tokens.css`, `app.css`. UI tests: `flow/FlowView.test.tsx`, `layout.test.ts`, `geometry.test.ts`; `mocks/flow.test.ts`, `flow.smoke.test.tsx`; `api/daemonKeys.test.ts`, `http.test.ts`, `live.test.tsx`, `queries.test.tsx`; `app/App.test.tsx`, `routes.test.ts`; `overview/HomeView.test.tsx`; `palette/Palette.test.tsx`, `options.test.ts`; `policy.test.ts`; `styles/tokens.test.ts`; `test/builders.ts`, `stubClient.ts`. Pins: `fixtures/daemon-keys.json`; CLI `tests/flow_read.rs`; http `tests/daemon_keys.rs`; MCP `tests/mcp_flow.rs`, `plugin_files.rs`, `plugin_roles.rs`, `plugin_skills.rs`.

**Divergences from the draft.** Hide gaps was added. A folded group shows no Start or End until `trace-views`. An edge between a group and its child, in the group's read, is undrawn. Only a gap item's first member is sought.
