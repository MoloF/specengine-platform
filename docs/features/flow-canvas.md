---
class: spec
status: draft
scope: [ui, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0044; Phase 7 step 3 (08 s2); owner's decision 11
---

# flow-canvas: the Flow screen (RM-0044)

## Why

The owner wants a diagram running forward, with branches, reuse and dependencies, folding into groups so one can be studied within the whole, and gaps visible. The daemon computes it (`flow-read.md`); this slice draws it, read-only; the AI writes, through proposals.

## Description and interactions

**A new section, not a Graph mode.** `flow`, fourth in the nav: `#/<p>/flow[/<name>]`, the name only selecting. `spec graph` needs a REF; its chips, impact switch and grid fit neither groups nor gap rows.

### Reads

All `getFlow`, always with a `depth`: the UI lifts nothing, and a whole-flow read, which fails at volume, is never made.

| Read | Query | For |
|---|---|---|
| Top | `depth=0` | top boxes, edges lifted to them, gap rows, `left_out`, `notes` |
| Group G | `root=G&depth=1` | G's children, edges among them; one per open group |

`holds > 0` is a closed group. Opening runs its read; closing keeps the answer cached. A sole top group starts open. Every drawn edge is the drawing read's, unchanged.

**A crossing edge stays on the frame.** A read keeps only edges with both ends in scope, so G's read holds none across G. The enclosing read's lifted edge ends on G's frame or box: left port the entry, right port the exit; the child it reaches is in the panel's links. In a group's lane `start` shows "Entry", at Top "Start".

### Layout

Pure, `src/flow/layout.ts`: a lane per read, nested, absolute positions.

- **Layer.** The box's `distance` in the read drawing it: a folded node carries the least of what it holds, so a group sits by its earliest step. A scoped walk restarts at 0, so each lane stands alone.
- **Grid.** Columns: the lane's distinct layers in order. Rows: four barycentre sweeps, as `layoutGraph`. A box is `BOX_WIDTH` x `BOX_HEIGHT`: name, title, `kind`, Start, End, Entry, gaps; a closed group adds a caret and `<holds> documents`; an open one is its lane under that head.
- **Not on the walk.** Layer null (definitions, unreached steps, a child with no flow link): a band under the lane.
- **Against the walk.** Target column left of the source: an arc under the lane, through the column gaps, on its own track in the lane's arc band, crossing no box. Same column: into the gap on its right. To itself: a loop under the box. Columns decide, never `kind`, so an undeclared loop shows too.
- **Cap.** `CANVAS_LIMIT`, 200 boxes, from Top as served, then open groups by (nesting, name). Past it undrawn, with its edges; a group with no room stays closed; a line `<k> of <n> boxes not drawn: close a group, or read the List`.
- **Trial (decision 11).** AC-12, before any library; failing it, or the owner's eye, stops the slice.

**Stages: no band yet.** RM-0048 owns `stage:`. Until then a stage is a parent document, its group the stage, layers ordering them; one parent each, so no stage cuts across groups.

### Gap rows

A list beside the canvas, outside the tabs: Top's ten rows as served, unknown states last; a group read's rows are not shown.

- A row: code raw, its meaning (`labels.ts`), a state in words: `None` at 0, `<n> found`; `Not configured` or `Cannot measure` with `cause` verbatim and no number, the latter in the `cannot-verify` colour and icon; another state a neutral badge.
- Items, eight then "Show all <n>", members joined ` + `; each member a button, but `no-start`'s scope, text.
- Activating one routes to the item's first member, marks all, centres the first; focus stays. A member not drawn: one on-demand `getTree(p)`, never needed to draw, gives its `parent` chain; each closed box on it opens, top down; failing, the picture stays, with a line. The List scrolls to the row.
- Canvas: a drawn box a row names shows an icon; a closed group the icon and its served `gaps`, `<gaps> with gaps`. Each read counts by its own rows, so a nested group goes by its sub-flow's.

### Panel

A click or Enter routes the name. Beside the canvas: the box's words, `path:line`, `distance`, a group's `inner` and Open or Close; text through `src/markdown/`; links by `LinksPanel`; "Open in Spec tree", "Open in Graph"; "Changes go through your AI, as proposals." Nothing writes. Esc closes, focus back on the box.

### States

- Loading: a skeleton, `aria-busy`, until Top lands; a group opening stays closed, "Opening".
- Failed Top: `ErrorPanel`, verbatim, Retry. Failed group read or re-read: the picture kept, a line, Retry.
- Empty: "No flow in this project yet. Next: have your AI add flow links."; the rows stay.
- No `[flow]`: the canvas drawn; over the rows `<k> rows need [flow] rules in specengine.toml`. Bad `[flow]`: `Cannot measure`, `<config>:<line>: <message>`.
- A name no read or chain finds: a note and a Tree link. A group read with `reason`: it closes, a note.

### Keyboard, motion, width

`ui/README.md` "Screen rules" applies; Canvas and List tabs, kept per project.

- **Keys**, one tab stop. Arrows only move: Up, Down in a column; Left, Right across; Left from a first column to the frame head; Right on an open head into its lane. Enter selects, focus to the panel heading. Space opens or closes a group, focus kept. `o` the Tree, `c` centres, Esc closes; `?` lists them.
- **Focus.** A focused box leaving the picture hands focus to its enclosing head. Each box's name says its title, marks and gaps.
- **Motion.** Viewport moves take `--duration-*`, 0 when reduced; frames never animate.
- **Width.** Side column above `64rem`. Under `48rem`: the List unless a tab is remembered, gaps above, panel below, no sideways page scroll at 320 px.
- **Live.** `flow` joins `READS_OF_THE_SPEC`.

### Files in scope

- **New:** `ui/src/flow/{FlowView,FlowCanvas,FlowGaps,FlowDetails,FlowList,FlowLegend}.tsx`, `{layout,geometry,labels}.ts`, `FlowView.test.tsx`, `{layout,geometry}.test.ts`; `ui/src/mocks/flow.ts`, `flow.test.ts`, `flow.smoke.test.tsx`.
- **Rust:** core `src/flow.rs`, CLI `src/flow.rs`, MCP `src/mirror.rs`, `plugin.json` 0.22.0 (a key a tool returns).
- **Pins:** `fixtures/daemon-keys.json`, `daemonKeys.test.ts`, CLI `tests/flow_read.rs`, MCP `tests/mcp_flow.rs`, `plugin_files.rs` `PINS`, `SCHEMA_PINS`.
- **Changed**, in `ui/src/`: `api/provisional.ts`, `client.ts`, `http.ts`, `http.test.ts`, `queries.ts`, `queries.test.tsx`, `live.test.tsx`; `mocks/MockClient.ts`; `app/sections.ts`, `routes.ts`, `routes.test.ts`, `App.test.tsx`, `Shell.tsx`, `shortcuts.tsx`; `ui/Icon.tsx`; `styles/tokens.css`, `tokens.test.ts`, `app.css`; `policy.test.ts`.
- **Unchanged:** other crates, `src/graph/`.
- **Canon at shipping:** new `spec-flow-screen.md`; `spec-flow.md`, `plugin-files.md`; 07 s3; `flow-links.md`.

## Data

```ts
interface FlowOptions { root?: string; depth?: number }  // client.ts
getFlow(project: string, options: FlowOptions, signal?: AbortSignal): Promise<FlowView>;
```

Key `["flow", p, root, depth]`. `HttpClient` as `getGraph`: a 404 is the exit-1 document. `FlowMemory` `{tab, open}` in the Shell, per project.

**No quoted type.** `ui/src/policy.test.ts` quotes no link type and compares no `.kind`. `FlowEdge` needs neither: `from` and `to` are in reading direction, so an arrow takes no branch; layers and arcs ask no type; a type's colour and plain dash go by index, as `patternAt`, in the sorted types of Top's `edges` and `inner`, which hold every edge, so folding moves no dash.

**Exception: a repeat is always dotted.** The `.kind` scan skips `edge.kind` under `/src/flow/` only: `FlowEdge.kind` is no project vocabulary but the closed `FlowEdgeKind` of `mirror.rs`. `FlowNode.kind` stays scanned, and an unknown value still shows raw, with its type's dash.

- Dash: `FLOW_DASHES`, seven distinct: three by `kind` in `labels.ts`, `repeat` dotted; four by type, none dotted. Colour `--flow-edge-0` to `-3` by type, >= 3:1. Shape by columns.
- Label: `kind`, `when` (clipped at 24 characters, whole in the List), `x<count>` above 1.
- `FlowLegend`: each type's and kind's sample, words raw; the arc, "runs against the walk".

**`FlowNode.gaps`, a small Rust addition:** the distinct documents named by gap rows among what the node holds, itself included. Measured rows, before any fold or cap; `no-start`'s scope is no document. A text node line ends ` | gaps <g>` above 0.

**The List** is the reads drawn, past the cap too: a section per read, its boxes, a closed group with Open, then each edge as `spec flow` prints it.

**Mock.** `ui/src/mocks/flow.ts`, harbor-sim's documents and kinds: DOM-HARBOR over DOM-BERTHS and DOM-WATER, every link type and `kind`, backward edges with and without one, a gap item two levels down. No crossing edge in a group's read; a child with no flow link. ledger-api: no `[flow]`. `large`: 500 documents in 25 groups.

## Rules and edge cases

- WHEN a gap row counts anything, the screen SHALL change no status or control (ADR-0012).

## Acceptance criteria

Each mutation M turns its criterion red.

- [ ] AC-01 — Route. `parseHash("#/a/flow/MEC-TIDES")` is a section route; `navOf` gives Flow fourth of eight.
- [ ] AC-02 — Reads. Spy on harbor-sim: load calls `{depth: 0}`, `{root: "DOM-HARBOR", depth: 1}`, never `{}`; opening DOM-BERTHS adds its read; closing and reopening adds none. M1: a read with no `depth` on load.
- [ ] AC-03 — Nothing lifted. An edge `x7` stubbed in Top draws `x7` with its group open. M2: a count recomputed from the group's read.
- [ ] AC-04 — Layers. Boxes at `distance` 0, 4, 9 run left to right; null lies in the band, a child with no flow link too.
- [ ] AC-05 — Arcs. Backward edges with and without `kind` arc, a forward one with `kind` curves, no arc point falls in a box. M3: arcing on `kind !== null`.
- [ ] AC-06 — Forms. A repeat is dotted whatever types a project holds, on a curve or an arc; types and kinds draw distinct dashes, the legend naming each; `tokens.test.ts` holds 3:1. M4: the repeat's dash by index.
- [ ] AC-07 — No quoted type. `policy.test.ts`'s list gains `follows`, `uses`, `supplies`; the Health scan covers `src/flow/` but `labels.ts`; the `.kind` scan passes `edge.kind` under `src/flow/`, and stays red on `node.kind` there and `edge.kind` in `src/tree/`. M5: `edge.type === "follows"` in `layout.ts`.
- [ ] AC-08 — Gap rows. Ten as served; ledger-api's rows read "Not configured", cause verbatim, no digit; a stubbed `cannot` reads "Cannot measure" with its icon. M6: `count ?? 0`.
- [ ] AC-09 — Gap selection. An item two levels down: its closed group shows its `gaps`; activation calls `getTree` once, runs both group reads, routes the first member, marks all, focus kept. `no-start`'s scope has no button. M7: skipping the chain.
- [ ] AC-10 — Panel. Text, links, an href equal to `sectionHash(p, "tree", name)`; the spy sees reads only.
- [ ] AC-11 — States and cap. Each case of "States", stubbed. `large` opened past 200: 200 boxes, the line, the List whole.
- [ ] AC-12 — The trial. `layout.test.ts` on `flow200()` (200 boxes, 12 columns, 30 off the walk, 40 edges against it): no overlap, forward edges run right, arcs clear boxes, reversed input equal, under 500 ms; time and crossings in "Implementation".
- [ ] AC-13 — Keyboard, motion, width. As that section says, each by a test: closing a group leaves focus on its head; under `48rem` the List first.
- [ ] AC-14 — Served count. `spec --json flow WING-LIBRARY --depth 1` on flow-gaps: `gaps` 1, 12, 2 on WING-LEND, WING-BROKEN, WING-HEADLESS, a document two rows name counted once; `get_flow` meets its schema. M8: counting per row.

**Owner's manual check.** `spec serve` over a flow project: fold, open, follow a gap item; judge the look at the cap with `?scenario=large`.

## Out of scope

`stage:`, confirmed counts and state lines on boxes (RM-0048); any write, drag or edit; a layout library.

## Implementation

Left for the developer.
