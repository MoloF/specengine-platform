---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-31
---

# The Flow screen

The flow drawn read-only, lifting nothing (ADR-0033, ADR-0055); `flow-canvas`, 2026-10-31; code `ui/src/flow/`. The read: `spec-flow.md`. Gaps stop nothing (ADR-0012); writing stays with the AI, through proposals.

## Route

`#/<p>/flow[/<name>]`, nav fourth of eight, the name only selecting. A section, not a Graph mode: `spec graph` needs a REF, and its chips, impact switch and grid fit neither groups nor gap rows.

## Reads

| Read | Query | For |
|---|---|---|
| Top | `depth=0` | top boxes, edges lifted to them, gap rows, `left_out`, `notes` |
| Group G | `root=G&depth=1` | G's children, edges among them; one per open group |

- **No whole-flow read.** Every `getFlow` carries a `depth`; a whole-flow read fails at volume. `getTree` on demand is the one wide read, for a gap activation or a route name no read holds, never to draw.
- **Groups.** `holds > 0` is a closed group; a sole top group starts open. Group reads are enabled only while open, `staleTime: Infinity`, `refetchOnMount: "always"`, so a hand edit, which raises no event, shows on coming back. Closing keeps the answer; reopening reads nothing unless an event marked it.
- **Nothing lifted.** Every drawn edge is the drawing read's, unchanged, `count` included.
- **A crossing edge stays on the frame.** A read keeps only edges with both ends in scope, so G's read holds none across G. The enclosing read's lifted edge ends on G's frame or box, left port the entry, right port the exit; the child it reaches is in the panel's links.
- **An edge between a group and its child**, in the group's read, is undrawn and shows in the List and the panel.

## Layout

`layout.ts`, hand-written and pure: reads in; absolute boxes, lanes and edges out, inputs sorted first, a lane per read, nested.

- **Layer.** The box's `distance` in the read drawing it: a folded node carries the least of what it holds. A scoped walk restarts at 0, so each lane stands alone.
- **Grid.** Columns are the lane's distinct distances. A forward edge skipping a column gets a passage slot there, so it passes no box. Four barycentre sweeps order rows; eight isotonic passes pull joins between branches. A box is keyed by path and line; a second holder of an ID gets its own id, and links land on the first.
- **Routes.** Columns decide, never `kind`: forward S-curves in the gaps; against the walk an orthogonal arc on its own track under the lane; same column beside; to itself under the box; to or from the band. No arc or band edge crosses a box. Ports spread along a side; labels stack.
- **Not on the walk.** `distance` null: definitions, unreached steps, a child with no flow link, and children entered from outside, whom the read's own walk does not reach, so the caption is true to the read. The band is a grid with free gutters; edges to one band box share a trunk by (target, type, kind); linked neighbours take a direct line.
- **Boxes.** Name, title, `kind`, marks, gaps; a closed group adds a caret and `<holds> documents`. `start` shows "Start" at Top and "Entry" in a group's lane; a folded group shows no Start or End until `trace-views`.
- **Cap.** `CANVAS_LIMIT`, 200 boxes: Top as served, then open groups by (nesting, name), partly drawing the last group; past it undrawn, with its edges; a group with no room stays closed; `<k> of <n> boxes not drawn: close a group, or read the List`.
- **Stacking and view.** One node type serves all forms; stacking is manual: frame, lane edges, lane boxes. The viewport fits until the owner moves it, then keeps the box in hand in place.
- **Stages:** no band. RM-0048 owns `stage:`; until then a stage is a parent document, one parent each, so no stage cuts across groups.

## Forms

`labels.ts` alone tells gap states, codes and `edge.kind` apart. `FlowEdge` needs no type word: `from` and `to` are in reading direction, and layers and arcs ask no type.

- **Type.** Colour `--flow-edge-0` to `-3` and plain dash go by place in the sorted types of Top's `edges` and `inner`, which hold every edge, so folding moves no dash.
- **Kind.** `FLOW_DASHES`, seven distinct: four by type, three by `kind`; `repeat` alone is dotted, in every project, and heavier. An unknown value shows raw, with its type's dash.
- **Label.** `kind`, `when` (clipped at 24 characters, whole in the List), `x<count>` above 1. `FlowLegend` prints each sample, words raw, and the arc, "runs against the walk".

## Gap rows

A list beside the canvas, outside the tabs: Top's ten rows as served, unknown states last; a group read's rows are not shown.

- **A row:** code raw, its meaning, a state in words: `None` at 0, `<n> found`; `Not configured` or `Cannot measure` with `cause` verbatim and no number, the latter in the `cannot-verify` colour and icon; another state a neutral badge.
- **Items:** eight, then "Show all <n>", members joined ` + `; each a button, but `no-start`'s scope, text.
- **Activating one** routes to the item's first member, marks all, centres the first; focus stays. In the List it scrolls to the row.
- **A seek** runs once per route change or gap item, first member only. A name no read holds takes `getTree`, and closed boxes on its `parent` chain open top down. A refused box is skipped; a chain with a failed group read keeps its seek for Retry; a chain run out ends it with a note; the owner's close ends it.
- **Marks.** A drawn box a row names shows "Gap"; a closed group its served `gaps`, `<g> with gaps`, by its own read's rows, so a nested group goes by its sub-flow's. An open group says only what names it.
- **Hide gaps** collapses the side column. The toggle reads "Hide gaps" or "Show gaps", `aria-expanded` following; `localStorage` key `specengine.flow.gaps-hidden`, per viewer, for every project, `on` when hidden, removed when shown. Refused storage keeps the choice for the view alone. A selected box brings the side column back for its panel; an empty flow always shows rows. At 1440 px the canvas goes from 816 to 1,184 px.

## Panel

A click or Enter routes the name. Beside the canvas: the box's words, `path:line`, `distance`, a group's `inner` and Open or Close; text through `src/markdown/`; links by `LinksPanel`; "Open in Spec tree", "Open in Graph"; "Changes go through your AI, as proposals." Nothing writes. Esc closes, focus back on the box.

## States and live

- Loading: a skeleton, `aria-busy`, until Top lands; a group opening stays closed, "Opening".
- Failed Top: `ErrorPanel`, verbatim, Retry. Failed group read or re-read: the picture kept, a line, Retry. Retry hands focus on when the read settles, not when `data` changes.
- Empty: "No flow in this project yet. Next: have your AI add flow links."; the rows stay.
- No `[flow]`: the canvas drawn; over the rows `<k> rows need [flow] rules in specengine.toml`. Bad `[flow]`: `Cannot measure`, `<config>:<line>: <message>`.
- A name no read or chain finds: a note and a Tree link. A group read with `reason`: it closes, a note.
- Live: `flow` is in `READS_OF_THE_SPEC`: an apply or a gap reads what is open and marks the rest; a stage does not.

## Keyboard, motion, width

`ui/README.md` "Screen rules" applies; Canvas and List tabs, kept per project in the Shell with the open groups, never in the hash.

- **Keys,** one tab stop. Arrows only move: Up, Down in a column; Left, Right across; Left from a first column to the frame head; Right on an open head into its lane; Right from a lane's last column climbs past the frame. Enter selects, focus to the panel heading. Space opens or closes a group, focus kept. `o` the Tree, `c` centres, Esc closes; `?` lists them.
- **Focus.** A focused box leaving the picture hands focus to its enclosing head. Each box's name says its title, marks and gaps.
- **Motion.** Viewport moves take `--duration-*`, 0 when reduced; frames never animate.
- **Width.** Above `64rem`: the side column. Between 48 and 64rem the order is canvas, panel, gaps. Under `48rem`: gaps above, panel below, the List unless a tab is remembered, no sideways page scroll at 320 px.

## Rules

- No write; no hold word (ADR-0012); no link type quoted in app code.
- The one exception to the `.kind` scan of `policy.test.ts`: `edge.kind` under `src/flow/`, the closed `FlowEdgeKind` of `mirror.rs`. `FlowNode.kind` stays scanned.
- Texts: `labels.ts`, the views; pinned by tests.

## Mock

`ui/src/mocks/flow.ts`, harbor-sim's documents and kinds: DOM-HARBOR over DOM-WATER, MEC-PILOTAGE, MEC-FAIRWAY and DOM-BERTHS, every link type and `kind`, arcs with a kind and without, a self-repeat, a definition two levels down for `getTree`, a child of DOM-BERTHS with no flow link. ledger-api has no `[flow]`. `empty`: rows and no node. `large`: 500 documents in 25 groups. The mock alone may lift an edge. `flow200()` in `src/test/flowStub.ts` is the trial's input.

## Known limits

- **Decision 11, the owner's call.** The 200-box trial with random wiring passes (median 1.9 ms, no overlap) with 1,920 forward crossings, "a hairball by eye". Real flows are sparser; the look at the cap, `?scenario=large`, is the owner's to judge. A layout library would need the owner's allowlist.
- **320 px** is unproven by a test: it needs a browser. A headless look saw `scrollWidth` 320.
- **The build warns** that the main chunk is 527.99 kB, over Vite's 500 kB.
- Exit steps carry no mark: core `Steps.leaving` is not served.
- Only an item's first member is sought; the others are marked if drawn.
- A crossing edge's child is named in the panel's links, not on the canvas.
- In the List and the panel a group in flight has no "Opening" word and cannot be closed.
- With no room left, the List shows the group's section while its row says "Open".
- A Retry answered with `reason` drops focus to `body`; rare.
- A refused group is re-read only on a live event, so after a hand edit its note stays until the view is left or the page reloaded.
- A failed group cannot be dismissed: the line stays until a Retry or Open succeeds.

## Tests

`ui/src/flow/FlowView.test.tsx`, `layout.test.ts`, `geometry.test.ts`; `ui/src/mocks/flow.test.ts`, `flow.smoke.test.tsx`; `api/live.test.tsx`, `queries.test.tsx`, `http.test.ts`; `policy.test.ts`, `styles/tokens.test.ts`.
