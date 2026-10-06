---
class: spec
status: shipped
scope: [ui]
ref: ui-graph analysis 2026-10-06, recommendations accepted by the orchestrator; iterations 1-2, review accepted; the attribution is an owner question (Open)
shipped: 2026-10-06
adrs: []
---

# UI graph: a node's walk on mocks

## Why

The owner sees what a node reaches and what an edit to it touches without opening files. Slice 3 draws one answer of `spec graph` (`docs/canon/spec-cli-graph.md` "spec graph") per view over the mock (ADR-0033); walk, directions, cut and edge states stay in core: the UI never unions, re-walks or infers containment.

Working answers, 2026-10-06 (orchestrator, the owner's standing instruction): a hand-written layered layout, no package, no ADR (ADR-0011 names no layout); the CLI's two modes only; React Flow's attribution hidden, credited as plain text (owner question, Open); edge ends outside `nodes` drawn as stubs; the endpoint uncut, limits in the UI. What became true: `ui/README.md` "Screen rules" (Canvas); the client contract below.

## Data

**Client** (`ui/src/api/client.ts`); query names = the JSON echo keys; absent omitted, arrays repeat the key, `impact`, `archive` only `true`:

```ts
/** `spec graph`: REF, `--impact`, `--type T`..., `--depth N`, `--archive`. */
export interface GraphOptions { ref: string; impact?: boolean; types?: string[]; depth?: number; archive?: boolean }
/** MISSING ENDPOINT GET /api/projects/:p/graph (07 section 3 lists it; uncut; rust-developer, daemon-read "Out of scope") */
getGraph(project: string, options: GraphOptions): Promise<GraphView>;
```

`getGraph("harbor-sim", {ref: "MEC-TIDES#RULE-TIDE-WINDOW", impact: true, types: ["depends_on", "constrains"], depth: 3})` -> `GET /api/projects/harbor-sim/graph?ref=MEC-TIDES%23RULE-TIDE-WINDOW&impact=true&types=depends_on&types=constrains&depth=3`. 404: the exit-1 document, as data; other non-2xx: `ClientError` verbatim (exit 2 -> 503).

**Provisional types** (`ui/src/api/provisional.ts`), citing `docs/canon/spec-cli-graph.md` "spec graph"; `LeftOut`, `LinkState`, `Direction` reused; `type` stays `string` (ADR-0031; `ShownLink`'s "closed table" remark dropped):

```ts
GraphView { ref; reason?; impact; types: FollowedType[]; depth?: number; archive; notes: string[]; left_out: LeftOut; truncated; nodes: GraphNode[]; edges: GraphEdge[] }
FollowedType { type; direction: Direction }
GraphNode { id?; kind?; title?; path; line: number; distance: number; archived }
GraphEdge { src?; type; dst?; written; path; line: number; state: LinkState; reason? }
// `?`: `| null`, never omitted; `impact`, `archive`, `truncated`, `archived` boolean; the rest string
```

**Query key** `["graph", p, {ref, impact, types, depth, archive}]`, every argument present (`false`, `[]`, `null`); `"graph"` is in `READS_AFTER_DECISION`.

**Mock** `graphOf(corpus, options)` (`ui/src/mocks/corpus.ts`), as `crates/specengine-cli/src/graph.rs` `followed` and `crates/specengine-core/src/check/spec_graph.rs` `walk`: REF as `resolveRef`, each holder at distance 0, unresolved the exit-1 document; `types` in the canon's order (`LINK_TYPES`, `IMPACT_LINK_TYPES`), a given type under `impact` the table's direction, else `in`, without `impact` `out`; walk, admission (`left_out`) and order as the canon's "spec graph": a node's links include its nested sections', a visited node's edge is listed, distance >= `depth` not expanded, edges by (type, path, line, written). `harbor-sim` gains `MEC-PILOTAGE` `precedes` `MEC-MOORING` (outside the 12), `RULE-TIDE-WINDOW` `adopts` `RULE-HIGH-WATER` (nested in `MEC-TIDES`), `MEC-NIGHT-PASSAGE` `depends_on` `MEC-TIDES#RULE-TIDE-WINDOW`, the archived document's `depends_on` `MEC-TIDES` (`MEC-TIDES` `left_out.tier3` 1 -> 2). `large`: front-matter `depends_on` per mechanic on its domain, sub-mechanic on its mechanic, `DOM-GEN-02`...`12` on `DOM-GEN-01`: Impact from `DOM-GEN-01` holds 17, 102, 396 nodes at distance 1-3 (516).

## Rules and edge cases

- WHEN an edge end is `null` or names no node THEN a stub, never an anchor, expanded or centred.
- Types styled by position in the chip order, never by name: app code quotes none but `mentions` (`policy.test.ts`); kinds raw; no kind-to-shape or type-to-icon map.
- The screen rules of `ui/README.md` hold; a box clips its title, whole in its accessible name, details and List. Read-only (ADR-0012).

## Acceptance criteria

Verifiers: Vitest, `ui_policy.rs` (`test-engineer`), the gates. M: the mutation turning it red; all 34 applied and red in iteration 1.

- [x] AC-01 - `client.ts?raw` holds `getGraph`'s `MISSING ENDPOINT`; on a counting stub `#/harbor-sim/graph/MEC-TIDES` makes one `getGraph` `{ref: "MEC-TIDES", depth: 2}`, no `getTree`, `getNode`, `search`, `getBundle`; `#/harbor-sim/graph` none, the field focused (M: the comment removed; a view imports `src/mocks/`).
- [x] AC-02 - the four types cite "spec graph"; per type a `satisfies Record<keyof T, true>` record equals the canon's keys (M: an extra `GraphNode` key; `distance` removed; the heading renamed).
- [x] AC-03 - each change of REF, mode, chip, depth or archive: one call, an equal value none; under `slow` the previous answer stays, `aria-busy="true"`; a decision (success, 409) reruns graph keys (M: `types` not in the key; `"graph"` not invalidated).
- [x] AC-04 - Impact sends `impact: true`; chips and `types` sent as Controls; the legend shows each `{type, direction}` as text, icon, pattern (M: chips sorted or hard-coded; `types` sent with all pressed; direction derived from the type name).
- [x] AC-05 - `layout.ts`: each walked node once, in its distance's column; no boxes overlap; handles and arrow as step 6; 20 seeded shuffles of `nodes` and `edges` give identical positions and handles (M: `Math.random`; an array-order tie-break).
- [x] AC-06 - a `null` `dst`: an unresolved stub with `written`, state label + icon, `reason` verbatim; an end outside `nodes`: one section stub per name; neither has an `a[href]` or "Centre here" (M: a stub dropped; `href` from `written`).
- [x] AC-07 - "Focus", "Selected" by text + icon, the selected box's edges emphasised; "Open in spec tree", "Centre here" go to their hashes, the latter one `getGraph`, options kept; Back restores REF and options (M: selection by colour only; options reset on Centre here).
- [x] AC-08 - exactly one box `tabindex="0"`; each key of Keyboard acts; nothing with Ctrl, Meta, Alt or from the field; mounting adds no `document` (nor `window`) keydown listener (M: `panActivationKeyCode` left default; every box `tabIndex 0`).
- [x] AC-09 - a box's accessible name holds name, kind, title, distance; no canvas text matches `/delete|move|drag|connect/i`; `large`, Impact from `DOM-GEN-01`, All: the List holds 516 nodes and every edge (515), each `a[href]` starting `#/` (M: the default `ariaLabelConfig`; the List limited like the canvas).
- [x] AC-10 - `large`, Impact from `DOM-GEN-01`, depth 2: the distance-2 column shows 24 boxes and "+78 more at distance 2"; `layout.ts`, 12 distances x 20 nodes: distances 0-9 drawn (200 boxes), the notice "0-9: 200 of 240"; `truncated: true`: its notice (M: a limit removed; the more-count off by one).
- [x] AC-11 - the States as specified; exit-1 `reason`, `notes` verbatim, Retry calls once more; "Left out" names only non-zero parts (M: a generic "Not found"; an inert Retry).
- [x] AC-12 - hostile titles as text, no `img` element; every `a[href]` starts `#/`, no `.react-flow__attribution`; each `--xy-<name>-default` of `base.css?raw` has `--xy-<name>` set to a `var(--...)`; no `style.css` import (M: `style.css` imported; `proOptions` removed).
- [x] AC-13 - `graphOf` on `harbor-sim`: `types` as Mock in all three cases; an edge to a visited node listed; distance = depth not expanded; Impact from `MEC-TIDES`: the archived link in `left_out.tier3`, listed with `archive`; both orders (M: edges to visited nodes dropped; default types sorted by name).
- [ ] AC-14 - 15 packages; `pnpm lint` (0 warnings), `build`, `test` green, no console output; `ui_policy`, `anonymity`, `doc_pointers` green; `ui/README.md` <= 8 192 B; docs gate clean, worst W <= this draft's 107 858 B (M: a 16th package; a type name in a style map). Open at shipping: `anonymity` `all_text_is_english_no_cyrillic_letters` red on `crates/specengine-cli/tests/decision_apply.rs` 487-496 (decision-apply, in progress), no hit under `ui/`; the rest met: 32 of 33 Rust tests, README 8 187 B, W 107 753 B.

## Owner's manual check

`pnpm --dir ui dev`: the wheel over the canvas scrolls the page (pinch and Controls zoom); `?scenario=large#/harbor-sim/graph/DOM-GEN-01`, Impact, depth 2 opens at 60 % with the focus top left, collapsed columns, then All: smooth pan, zoom, Fit; `#/harbor-sim/graph/MEC-TIDES` in both modes: same-column arcs in the gap, labels on top; releasing a chip keeps the other patterns; keyboard alone; VoiceOver on a box and the List; 200 %: panes stacked; `?scenario=empty`, `error`, `slow`. Then the attribution question.

## Open

- **Owner question, pending**: React Flow's attribution link hidden (`proOptions.hideAttribution`; MIT allows it), credited as plain text "Drawn with React Flow (MIT)"; `pnpm dev`'s console shows React Flow's dev-only attribution warning (tests silent). Else an external link stays in the view.
- **Endpoint** (`rust-developer`): `GET /api/projects/:p/graph` uncut (daemon-read's `View::Browser`; a cut drops every edge, `graph.rs` 220-222), 404 with the exit-1 document.
- **Edge ends** may be nested sections outside `nodes` (`edge_of` names the end itself); an additive key naming the walked node holding one would place section stubs by containment (`rust-developer`, later).
- **Numbers** (200, 24, depth 2, the 60 % fit floor) tuned at the owner's check; about 0.6 s at 3 000 nodes: the kept answer.
- **`.claude/agents/ui-developer.md`** (owner's text): line 11 "with a dagre or ELK layout" -> "with the hand-written layered layout of `docs/features/ui-graph.md`"; provisional types: the slice's exception (ADR-0033).

## Out of scope

Both directions in one view (a `spec graph` flag first); document clusters; semantic zoom; `--format dot|mermaid`; code symbols (Phase 3); SSE (`ui-live`); the endpoint; any edit, drag or connect.

## Implementation

**Route** `#/<p>/graph[/<REF>]`; the node pane has "Show in graph". **Controls** (per project, in memory): REF field; "Outgoing" | "Impact"; chips in the mode's unfiltered `types` order, then `mentions` released (all but `mentions` pressed: no `types`; the last pressed `aria-disabled`; a mode switch resets them; no direction from the other mode's answer); depth 1-6, All (default 2); "Include archive"; tabs "Canvas" | "List".

**Layout** (`layout.ts`, pure, sorted input): 1. a walked box per name; an unresolved stub per `null` end, id `u:<end>:<type>:<path>:<line>:<written>` (URI-encoded, `#n` for duplicates); a section stub per end name outside `nodes`. 2. Walked at `distance`; a section stub that is its first walked edge's walk-from end at `max(0, far.distance - 1)`, a walk-to section stub and every unresolved stub at `far.distance + 1` (edge direction only); no walked end: after the last column. 3. Over 24 boxes: 24 and "+k more at distance d". 4. Columns while all boxes stay <= 200, then a notice. 5. Four barycentre sweeps. 6. Box 240 x 72 at (336c, 88r), handles at side middles; walk-from `src` if followed `out`, else `dst`.

**Canvas** (`GraphCanvas`, lazy, read-only React Flow): drag, select, connect and key options off or `null`, `zoomOnScroll` and `preventScrolling` `false`; one roving `tabIndex 0`. Line pattern by position in the chip order (unfiltered types, `mentions`, unknown types), stable when a chip is released. An edge to the same or an earlier column arcs over the boxes, in one column bent 224 px right into the gap; labels above the boxes, `aria-hidden`. Fit per answer and by our own Fit button: `readableViewport` centres a graph readable at <= 100 %, else 60 % with the focus at the 32 px margin.

**Details**: Enter focuses its heading, Esc returns to the box; "Show in List" focuses the distance heading only when walked nodes are hidden, else `#graph-stubs`, else the List panel ("boxes" wording). **List**: "Not walked (k)" only when k > 0. **Legend**: per `{type, direction}` pattern, icon, words; "Arrows point as the link is written; columns are the distance."; "Drawn with React Flow (MIT)". **Keyboard**: arrows, `j`/`k`, Home/End, Enter, `o`, `c`, Esc, `?`.

| Module | What it does |
|---|---|
| `ui/src/graph/` | `GraphView`, `GraphControls`, `GraphCanvas`, `GraphDetails`, `GraphList`, `GraphLegend`; `layout.ts`, `geometry.ts` (`arcOver`, `readableViewport`), `lines.ts`, `settings.ts` (chips, `patternOrder`) |
| `ui/src/` `api/`, `mocks/`, `app/`, `tree/`, `styles/` | `getGraph`, the types, the key; `graphOf`; route, options memory, keys; "Show in graph"; every `--xy-*`, `base.css` |

Tests: 640 Vitest in 32 files (new `graph/layout`, `geometry`, `settings`, `styles`, `GraphView`, `mocks/graph`, `graph.smoke`); lint 0, build (`GraphCanvas` 188 kB lazy), 15 packages. Iteration 2 fixed the review's 12 items: stub columns, arcs and labels under boxes, a 30 % fit, wheel zoom, shifting patterns, chip directions, stub ids, self-loops, quadratic appends, arrow meaning, note keys, focus on `body`.
