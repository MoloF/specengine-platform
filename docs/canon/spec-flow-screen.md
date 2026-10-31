---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-31
---

# The Flow screen

The flow drawn, lifting nothing (ADR-0033, ADR-0055), its boxes against their cards; `flow-canvas`, `trace-views`; code `ui/src/flow/`, `ui/src/trace/`. The reads: `spec-flow.md`, `spec-trace.md`. Gaps and marks stop nothing (ADR-0012); a spec change stays with the AI, through proposals; a card is written from the panel.

## Route

`#/<p>/flow[/<name>]`, nav fourth of eight, the name only selecting. A section, not a Graph mode: `spec graph` needs a REF, and its chips, impact switch and grid fit neither groups nor gap rows.

## Reads

| Read | Query | For |
|---|---|---|
| Top | `depth=0&trace=true` | top boxes, edges lifted to them, gap rows, `left_out`, `notes` |
| Group G | `root=G&depth=1&trace=true` | G's children, edges among them; one per open group |
| Lanes | trace `summary=true` | the Stage filter, the Timeline |
| Lane S | trace `stage=S`, `-` for none | its steps; one per open lane |
| Node R | trace `ref=R` | the panel's Cards block |

- **No whole read.** Every `getFlow` carries a `depth`, always `trace`; a whole-flow read fails at volume. `getTrace` is never `{}` here. `getTree` on demand is the one wide read, for a gap activation or a route name no read holds, never to draw.
- **Groups.** `holds > 0` is a closed group; a sole top group starts open. Group reads are enabled only while open, `staleTime: Infinity`, `refetchOnMount: "always"`, so a hand edit, which raises no event, shows on coming back. Closing keeps the answer; reopening reads nothing unless an event marked it.
- **Nothing lifted.** Every drawn edge is the drawing read's, unchanged, `count` included.
- **A crossing edge stays on the frame.** A read keeps only edges with both ends in scope, so G's read holds none across G. The enclosing read's lifted edge ends on G's frame or box, left port the entry, right port the exit; the child it reaches is in the panel's links.
- **An edge between a group and its child**, in the group's read, is undrawn and shows in the List and the panel.

## Layout

`layout.ts` places each read, the view fits it and the edges take their forms: `spec-flow-layout.md`.

- **Boxes.** Name, title, `kind`, marks, gaps; a closed group adds a caret and `<holds> documents`. `start` shows "Start" at Top and "Entry" in a group's lane, a closed group too, `end` "End": the read folds both. A box is 92 px tall, Flow only (`geometry.ts` `BOX_HEIGHT`). It shows its served rollup; a closed group its `counts`, `3 confirmed, 1 suspect, 2 no card`, zero parts left out, and a bar (`TraceBar.tsx`), a group's rollup covering its own cards only. A no-card step is hollow; open frames say nothing; no served rollup, no line.
- **Cap.** `CANVAS_LIMIT`, 200 boxes: Top as served, then open groups by (nesting, name), partly drawing the last group; past it undrawn, with its edges; a group with no room stays closed; `<k> of <n> boxes not drawn: close a group, or read the List`.
- **Stages:** no band: the Stage filter and the Timeline say them, across groups.

## Gap rows

A list beside the canvas, outside the tabs: Top's ten rows as served, unknown states last; a group read's rows are not shown. All rows None on a fresh read: one line in the tools, no list, no side column.

- **A row:** code raw, its meaning, a state in words: `None` at 0, `<n> found`; `Not configured` or `Cannot measure` with `cause` verbatim and no number, the latter in the `cannot-verify` colour and icon; another state a neutral badge.
- **Items:** eight, then "Show all <n>", members joined ` + `; each a button, but `no-start`'s scope, text.
- **Top not read.** A failed re-read of Top takes every row's state, items and cause ("Not read") and the top level's Gap marks; its line adds "Top-level boxes show no state until it reads.", not on the Timeline. Boxes under a group whose read landed keep theirs.
- **Activating one** routes to the item's first member, marks all, centres the first; focus stays. In the List it scrolls to the row.
- **A seek** runs once per route change or gap item, first member only. A name no read holds takes `getTree`, and closed boxes on its `parent` chain open top down. A refused box is skipped; a chain with a failed group read keeps its seek for Retry; a chain run out ends it with a note; the owner's close ends it.
- **Marks.** A drawn box a row names shows "Gap"; a closed group its served `gaps`, `<g> with gaps`, by its own read's rows, so a nested group goes by its sub-flow's. An open group says only what names it.
- **Hide gaps**, or Show gaps, collapses the side column, `aria-expanded` following; `localStorage` key `specengine.flow.gaps-hidden`, per viewer, for every project, `on` when hidden, removed when shown. Refused storage keeps the choice for the view alone. A selected box brings the side column back for its panel; an empty flow always shows rows. The line stands in this button's place; focus lost with it goes to the results.

## Panel

A click or Enter routes the name. Beside the canvas: the box's words, `path:line`, `distance`, stage, a group's `inner` and Open or Close; the Cards block; text through `src/markdown/`; links by `LinksPanel`; "Open in Spec tree", "Open in Graph"; "Changes go through your AI, as proposals." The spec is read only here; a card is a queue row, written from this block. Esc closes, focus back on the box.

**Cards block.** `src/trace/CardsBlock.tsx`, here and in the tree's `NodePane`: the rollup, a read of more than one node its counts; each card once, its mark, `flow/column`, a link to the Board; `uses` rows; New card, the node preset. A suspect card's Clear is the Board's dialog and finishes on the page; a read of more than one node with one to clear adds Clear all, no effect only, one POST (`ClearGroupDialog`). With no row for the node (`rowOf`) it says the state is not known, never another's. A failed read: the daemon's words, Retry; a failed re-read adds that what stands below is from the last read.

## States and live

- Loading: a skeleton, `aria-busy`, until Top lands; a group opening stays closed, "Opening".
- Failed Top: `ErrorPanel`, verbatim, Retry. Failed group read or re-read: the picture kept, a line, Retry. A failed re-read leaves no state above its line: Top's or a group's drops that read's badges, counts and gaps (`withoutStates`), the legend's state rows and the accessible names with them; the Cards block drops its state row. An unknown mark word is a neutral badge, never a clean tile. Each Retry hands focus on when the read settles, not when `data` changes.
- Notes: the daemon's, of every read shown, group reads too, each once.
- Empty: "No flow in this project yet. Next: have your AI add flow links."; the rows stay.
- No `[flow]`: the canvas drawn; over the rows `<k> rows need [flow] rules in specengine.toml`. Bad `[flow]`: `Cannot measure`, `<config>:<line>: <message>`.
- A name no read or chain finds: a note and a Tree link. A group read with `reason`: it closes, a note.
- Live: `flow` and `cards` are in `READS_OF_THE_SPEC`: an apply or a gap reads what is open and marks the rest; a stage does not. `card.*` and decision events read the flows and traces again, so badges and lanes fill with no reload.

## Timeline

The third tab (`TimelineView.tsx`, `timeline.ts`). Lanes in served order, "No stage" last, from `getTrace({summary: true})`, shared with the filter and read again as the tab mounts.

- **A lane:** the stage, `<c> of <n> confirmed`, a bar from served `counts`, each part labelled, never by colour alone.
- **Open:** one `{stage}` read, enabled while open and kept closed, as a group's; steps by `distance`, off the walk last, each linked to the flow with its place, "Step <n>" from one, or "Step unknown", and its rollup. A `no card` step keeps its place, hollow, "No card".
- **States.** Failed `summary`: `ErrorPanel`, Retry; a failed re-read of it or a lane: a line that what stands below is from the last read, Retry. No step with a card: "No step has a card yet. Next: open a step and press New card." No `stage:` anywhere: "Next: give documents a `stage:` key" over one "No stage" lane, or none when `stages` is `[]`. A bad `[flow]`: lanes by name, the daemon's note.

## Stage filter

A select beside the tabs, hidden on the Timeline, its options the served lanes': All stages, each stage, No stage; `timeline.ts` compares nothing but the served value. The Canvas dims other boxes and open frames' heads (`--flow-dimmed`, 0.72), keeping the layout; the List hides boxes and their edges. A group with no stage of its own is not judged (`outsideStage`, `keptInList`): not dimmed, not called outside, and the List keeps every group with its Open, the way to what it holds. A stage no lane serves any more reads as All. Failed `summary`: a line and Retry over the tabs.

## Keyboard, motion, width

`ui/README.md` "Screen rules" applies; Canvas, List and Timeline tabs, kept per project in the Shell with the open groups, the filter and the open lanes (`FlowState`), never in the hash.

- **Keys,** one tab stop. Arrows only move: Up, Down in a column; Left, Right across; Left from a first column to the frame head; Right on an open head into its lane; Right from a lane's last column climbs past the frame. Enter selects, focus to the panel heading. Space opens or closes a group, focus kept. `o` the Tree, `c` centres, Esc closes; `?` lists them.
- **Focus.** A focused box leaving the picture hands focus to its enclosing head. Each box's name says its title, marks and gaps.
- **Motion.** Viewport moves take `--duration-*`; frames never animate.
- **Width.** Above `64rem`: the side column. Between 48 and 64rem the order is canvas, panel, gaps. Under `48rem`: gaps above, panel below, the List unless a tab is remembered, no sideways page scroll at 320 px.

## Rules

- No spec write; no hold word (ADR-0012); no link type quoted in app code. `src/flow/`, `src/trace/` branch on no rollup, stage or mark word but through `labels.ts` (`policy.test.ts`).
- The one exception to the `.kind` scan of `policy.test.ts`: `edge.kind` under `src/flow/`, the closed `FlowEdgeKind` of `mirror.rs`. `FlowNode.kind` stays scanned.
- Texts: `labels.ts`, the views; pinned by tests.

## Mock

`ui/src/mocks/flow.ts`, harbor-sim's documents and kinds: DOM-HARBOR over four groups, every link type and `kind`, arcs with a kind and without, a self-repeat, a definition two levels down for `getTree`, a child of DOM-BERTHS with no flow link. ledger-api has no `[flow]`. `empty`: rows and no node. `large`: 500 documents in 25 groups. `ui/src/mocks/trace.ts` serves `spec trace` over the mock board, `withTrace` the badges. The mock alone may lift an edge and join a rollup. `flow200()` in `src/test/flowStub.ts` is the trial's input.

## Known limits

- **Decision 11, the owner's call.** The 200-box trial with random wiring passes (median 1.9 ms, no overlap) with 1,920 forward crossings, "a hairball by eye". The look at the cap, `?scenario=large`, is the owner's to judge. A layout library would need the owner's allowlist.
- No round trip against a real daemon in a browser yet.
- **320 px** is unproven by a test: it needs a browser. A headless look saw `scrollWidth` 320.
- Exit steps carry no mark: core `Steps.leaving` is not served.
- Only an item's first member is sought; the others are marked if drawn.
- A crossing edge's child is named in the panel's links, not on the canvas.
- In the List and the panel a group in flight has no "Opening" word and cannot be closed.
- With no room left, the List shows the group's section while its row says "Open".
- A Retry answered with `reason` drops focus to `body`; rare.
- A refused group is re-read only on a live event, so after a hand edit its note stays until the view is left or the page reloaded.
- A failed group cannot be dismissed: the line stays until a Retry or Open succeeds.

## Tests

`ui/src/flow/FlowView.test.tsx`, `layout.test.ts`, `geometry.test.ts`, `timeline.test.ts`; `ui/src/trace/CardsBlock.test.tsx`; `ui/src/mocks/flow.test.ts`, `trace.test.ts`, `flow.smoke.test.tsx`; `api/live.test.tsx`, `queries.test.tsx`, `http.test.ts`; `policy.test.ts`, `styles/tokens.test.ts`.
