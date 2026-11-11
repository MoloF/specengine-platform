---
class: spec
status: draft
scope: [ui]
ref: RM-0098, v5 step 6 (08 s2 Phase 7, "v5 scope"); the owner's v5 Q3 of 2026-11-10; the owner's four requests of 2026-11-11 (Flow left to right, which closes Q14; lost clicks on Flow; wider work-item modals; step 6 and nothing further); the analyst's requirement analysis of 2026-11-11; the orchestrator's click-bug repro and working answers, the owner's review open
adrs: []
---

# handoff-progress: a parent's progress, Ready to accept and Relies on, with Flow left to right again (RM-0098)

## Why

The owner's answer of 2026-11-10 (v5 Q3): a parent task shows "Ready to accept" and a progress bar, and a person accepts it. Nothing closes a parent on its own (ADR-0074 stands). His pipeline's step 5: "the task closes when every subtask is done, or resolved with a comment".

Today the work item lists a parent's children from the Board's one list read, which leaves cancelled cards out (`card-flows.md` "Children"). It shows no total, no cancelled child and no reason, and nothing says what the task waits on.

His requests of 2026-11-11, translated:
1. "Flow should run left to right, as it always did, not top to bottom." He has now looked, which closes Q14 ("top to bottom ... until the owner's look").
2. "When I click blocks in the flow chart they get stuck; after clicking one I can't click another." The orchestrator's repro, on a debug daemon over a scratch copy of a real game corpus at 1440x900, sent synthetic presses at box centres. A 0 px move selected the box. A 2-3 px move between press and release was taken as a pan by d3-zoom, which swallowed the click. Boxes are not draggable, so they carry no `nopan` class, and xyflow 12.12.0's `paneClickDistance` defaults to 1 px. A second cause: the minimap covers the lower-left box at fit. A third, found by reading the code: a press focuses a box, and `onFocus` centres a half-visible box, which moves it from under the pointer before the release.
3. "The work-item modals should be noticeably wider": about 1200 px, with a 360 px side column.
4. Take step 6 (RM-0098), "but only it, nothing further".

A side finding of the orchestrator's looks: an arrow's accessible name reads backwards. For `B follows: A` the served edge runs from A to B, and `edgeName` says "A follows B".

This slice is UI only and issues no ADR. Its rules come from:
- ADR-0074 (no card write waits on another);
- ADR-0012 (nothing is blocked);
- ADR-0077 "The rule" (no rollup reads a card's items);
- ADR-0008 (no project word in the code);
- ADR-0033 and ADR-0055 (Flow lifts nothing).

The rollup precedent is `card-flows.md` "Children". Left to right is a canon change in `spec-flow-layout.md`, made at ship.

## Description and interactions

### Decisions

- **D1. The children read.** The work item reads its card's children itself. This is `CardView` in `CardPanel.tsx`, the modal's and the page's alike.
  - `useChildren(project, id)` (new, `api/queries.ts`) calls `getCards(project, {parent: id, with_cancelled: true}, signal)`, and nothing else.
  - Its key is `queryKeys.children(project, id)` = `["cards", project, {parent: id, with_cancelled: true}]`. It sits under `cardListsOf(project)`, so whatever reads the card lists again reads it again: a `card.*` event (`readMarksAgain`), a write's settle, a gap.
  - It is enabled only while a card is shown.
  - `CardViewProps.handOffs` goes. `BoardView` (l.1357) and `CardPage` (l.64) stop computing a rollup for the work item. The Board's one list read, its tiles' `rollupsOf` (l.399) and the Parent lanes are unchanged.
  - Nothing served changes:
    - `client.ts` `CardsOptions` already carries `parent` and `with_cancelled`;
    - `http.ts` `getCards` sends both;
    - the daemon reads both (`app.rs`, `args.text("parent")`, `args.boolean("with_cancelled")`);
    - `MockClient` filters by both (`cards.ts` `cardListOf`).
- **D2. The reasons, one read per cancelled child** (a working answer).
  - Each cancelled child has one `useCard(project, childId)`, keyed `queryKeys.card`, so it is shared with that child's own modal. An open or accepted child gets none.
  - The reason is `Card.cancellation {by, at, reason}`.
  - A list row keeps `cancelled: boolean`. No served change.
- **D3. The model**, `rollup.ts` `progressOf`, pure ("Data"):
  - It splits the read's rows whose `parent` is the card into the not-cancelled (accepted ones too) and the cancelled, each kept in served order.
  - It counts six words, in this order:
    - accepted, by the row's `accepted`;
    - cancelled, by the row's `cancelled`;
    - then an open child's `category.ts` `categoryOf(flow, column)`: returned, in progress, to do, not in the flow.
  - `done` on an open child counts as in progress (W2). Only a config change puts an open card in `accept.to`.
  - It judges `ready` and `reliesOn` (D5, D6).
  - The words are looked up as `category.ts` looks up its own: none is spelled twice, and no column or flow is compared (`policy.test.ts`).
- **D4. The bar.** It shows on any card with at least one child, open or closed, at the top of Subtasks after the hint.
  - It is a row of segments, one per non-zero word in D3's order, each `flex-grow` its count. It is `aria-hidden`.
  - Under it, a words line in that order, ` · `-joined, each word with its icon and its plural (`board.progress.*`). This line is what a reader and a screen reader get: never colour alone.
  - Fills and icons:

    | Word | Fill | Icon |
    |---|---|---|
    | accepted | `success` | `taskAccepted` |
    | cancelled | `neutral` | `taskCancelled` |
    | returned | `warning` | `cardReturned` |
    | in progress | `information` | `taskInProgress` |
    | to do | `neutral` | `open` |
    | not in the flow | `neutral` | `unknown` |

  - Each fill is its appearance's `--lozenge-<appearance>-text` role. No new colour role.
- **D5. Ready to accept** (Q3; the all-cancelled case is a working answer).
  - It is shown when the parent is open (no `acceptance`, no `cancellation`), has at least one child, and every child is accepted or cancelled. That includes every child cancelled.
  - It is judged from the children read alone. A reason that is pending or failed never hides it (W4).
  - It is a status line, never a control: a `success` lozenge "Ready to accept" and the sentence `board.progress.readyAbout`.
  - It never shows, hides, enables, disables or reorders Accept. Accept stays where `board-screen.md` "Moving" puts it, in `accept.from` only.
- **D6. Relies on** (a working answer). On an open parent only, it lists the children that are neither accepted nor cancelled, in served order. Each is its flow's title (`flowTitleOf`, raw name when the flow is not served) then its ID, a `CardLink`. A plain click replaces the modal's card, as Subtasks' rows do.
  - It never lists an item: ADR-0077 says no rollup reads one.
  - It never lists a `T-` task: Agent tasks stand in their own part.
  - With nothing open it is absent.
- **D7. Cancelled subtasks.** Under the rows, a labelled list of the cancelled children in served order. Each shows:
  - ID (a `CardLink`), title, flow title, status lozenge (`statusLook`) and the Cancelled badge (`cancelledLook`);
  - then its reason as shown text (`shownText`, wrapped, `<bdi>`) and `board.panel.byAt` with `by` and `at`.

  Its read's states: pending `board.progress.reasonReading`; failed `board.progress.reasonFailed` with the daemon's words under `lang="en"`. The other parts stand either way. Subtasks' rows (ID, title, flow title, status, assignee, word: today's `HandOffs`) list the not-cancelled children, as today.
- **D8. The words and the accept guard.** Every new work-item word is a `board.progress.*` key ("Data"). That group sits outside the scan of `src/board/words.test.ts` (BH-18 of `board-handoff`), which scans `board.handoff`, `board.return` and `board.rollup.{handedOff,taken,returned}`.
  - So "Ready to accept", the owner's own word (v5 Q3), and the bar's "accepted" pass. The guard's scan is unchanged.
  - The test gains one pin: `en.board.progress.ready` is "Ready to accept", and no `board.progress` key is among the scanned words. A later move of the hint into a scanned group turns red.
  - `src/i18n/words.test.ts` (DP-8, U5) still holds: no `card`, `workspace` or `writer` in any English leaf, and no bare "task" in the groups of `T-` tasks. `board.progress` is none of them.
- **D9. Size first** (RM-0098).
  - Before the slice's own bytes, `Shell.tsx` loads `RoadmapView` (l.19) and then `GraphView` (l.11) by `lazy(() => import(...).then((module) => ({ default: module.X })))`, as `CardItems` and `RoundScreen` are.
  - Each route stands in a `Suspense` whose fallback is a `Skeleton`: `app.lazy.roadmap`, `app.lazy.graph`.
  - Their types stay `import type`. `graph/layout.ts` and `graph/geometry.ts` stay in first load: Flow imports them.
  - First-load JS is measured after each move and at the end (S-1).
- **D10. Flow left to right** (Q14 closed by the owner's look). The walk runs left to right, as before `flow-scenario` (`spec-flow-layout.md` "Grid" at 406ce2a). It keeps `flow-scenario`'s cut, stage dividers, 160x92 boxes and both floors. `layoutFlow` ("Data", "The layout"):
  - **Columns.** One per distinct distance, left to right.
    - In a column, boxes run top to bottom, ordered by four barycentre sweeps, then eight passes align each column with its neighbours.
    - A forward edge skipping a column takes a passage slot there, so it passes no box.
    - Gaps: `COLUMN_GAP` 136 between columns (a two-line label, `LABEL_WIDTH` 128, and its insets); `BOX_GAP` 32 between the boxes of a column, more where a loop stands; `LANE_PAD` 16.
    - Four columns fit a 1100 px canvas at the 0.92 floor: `BOX_WIDTH`'s reason, kept.
  - **Routes.** Columns decide, never a type or a `kind`:
    - forward: S-curves out of a box's right side into the next one's left, in the gaps (geometry's horizontal forms, no longer swapped);
    - against the walk, a repeat among them: an orthogonal arc out of the source's bottom and into the target's bottom, on its own track below the walk, the shorter arc nearer;
    - within one column: beside the boxes;
    - to itself: a loop under its box (`loopPath`);
    - to or from the band.

    No arc or band edge crosses a box. Ports spread along a side; labels stack.
  - **Stage dividers.** One stands left of the first column holding a box of a stage. It is a vertical rule at `FlowDivider.x`, between that column and the previous one.
    - The rule runs from the top of a words strip, `DIVIDER_HEIGHT` tall above the walk, to below the lowest arc track.
    - Its words sit at its top, beside the rule: at most two lines in the column's width, then an ellipsis. A box's accessible name still says its whole stage.
    - Stages beginning in one column share one divider, their raw words joined ` · `.
    - A picture of fewer than two stages draws none, a stage view draws none, and a band box opens none.
    - It is `aria-hidden`, never focusable, and takes no pointer (D13).
  - **The band** stands below the lowest arc track, its caption over it. It wraps at the walk's width or four boxes, whichever is wider.
  - `swapped`, `swappedPath` and `sideLoopPath` (`geometry.ts` l.141-155) leave the layout. They are deleted if nothing else uses them.
- **D11. The fit.** `geometry.ts` `fitOf(picture, width, height)`:
  - The zoom is the lesser of the two sides' ratios, `(side - 2 * FIT_MARGIN) / picture side`, at most `FIT_MAX_ZOOM` (1) and at least `FLOW_FIT_MIN_ZOOM` 0.92.
  - Along a side the picture fits at that zoom, it is centred. Along a side it overflows, it starts at the margin.
  - So a long walk lands at 0.92 from its first column and is panned, never shrunk (a working answer).
  - `FLOW_PINCH_MIN_ZOOM` 0.5, `c`, "fits once" and the minimap's "only while not whole" are unchanged. Graph keeps its 0.6 and 0.1.
- **D12. Keys**, `layout.ts` `boxAfterKey`:
  - **On the walk.** Right and Left go to the nearest box, by vertical centre, of the next or previous column; with none, they stay. Up and Down move within the column. Down from a column's last box goes to the band's first shelf, nearest by horizontal centre.
  - **In the band.** Left and Right move along a shelf. Up and Down cross shelves, nearest by horizontal centre. Up from the first shelf goes to the last box of the nearest column.
  - An unknown key or box stays. `FlowCanvas` already passes all four arrows.
- **D13. Clicks on Flow** (request 2; Flow only, a working answer):
  - **Press distance.** Flow's `ReactFlow` takes `paneClickDistance={FLOW_CLICK_DISTANCE}`, 5. A press moving 5 px or less between down and up is a click. Past that it is a pan, and d3-zoom swallows the click. Boxes take no `nopan`, so a drag that starts on a box still pans (W10).
  - **The minimap.** `.flow-canvas .react-flow__minimap` takes `pointer-events: none`. It is `pannable={false} zoomable={false}`, so it loses nothing, and a box under it takes its click. It stays bottom-left (W6). Its comment (`FlowCanvas.tsx` l.654) says so.
  - **Words-only nodes.** `WORDS_ONLY` (l.319) gains `style: { pointerEvents: "none" }`. xyflow 12.12.0's `NodeWrapper` sets `pointer-events: all` on every node once `onNodeClick` is set, then spreads `node.style` over it. So a divider's rule or the band's caption never takes a press meant for a box or an arrow under it.
  - **A press moves nothing.** `onFocus` (l.530) centres a box that is out of view only when the focus did not come from a pointer press. A press counts from a `pointerdown` in the canvas (capture phase) until its `pointerup` or `pointercancel`. Tab, the arrow keys, Space's first box and `c` centre as today. "A selection moves nothing" (`spec-flow-layout.md` "Layout").
  - Graph and Roadmap keep xyflow's defaults: a known limit, and a pool item at ship ("Canon at ship").
- **D14. Edge names** (the orchestrator's look).
  - An arrow's accessible name is `From <from> to <to>, <type>[, <kind>][, <when>]`. `<from>` and `<to>` are the end boxes' words (`boxWords`: the title, else the name). The type is raw; the kind is `flow.edgeKind.*`; `when` is clipped as on the canvas.
  - A List row is `to <to title>, <type>[, <kind>][, when <when>][, ×<count>]`, the title its name when it has none.
  - The frames are two catalog keys, `flow.edgeName` and `flow.edgeRow`. The type is data and is quoted nowhere (`policy.test.ts`).
  - `edgeName(edge, fromTitle, toTitle, t)` takes the titles. `FlowCanvas` `edgeOf` passes the laid edge's source and target boxes' words.
- **D15. Wider work items** (request 3).
  - `--modal-width-xl` 75rem (1200 px) sets the card's and the document's modal: `min(var(--modal-width-xl), 100vw - 2rem)`.
  - `--issue-side-width` is 22.5rem (360 px).
  - The one-column breakpoint `@container issue-view` is 44.5rem: the side, `--issue-main-min` 20rem and the body's `--space-400` gap. At full width the main column is about 750 px.
  - The card's and the document's page take `max-width: var(--modal-width-xl)` (W5). `--modal-width-l` goes with its last use.

### The work item's Subtasks, in order

1. The hint (D5), on an open parent that is ready.
2. The bar and its words line (D4), with at least one child.
3. Relies on (D6), on an open parent with an open child.
4. The rows (D7), the not-cancelled children; with no child at all, "None yet." alone, as today.
5. Cancelled subtasks (D7), with a cancelled child.

While the children read is out, Subtasks shows a `Skeleton` labelled `board.progress.reading`. If it fails, it shows `board.progress.failed` with the daemon's words under `lang="en"` and Retry, and every other part of the work item stands. A failed re-read keeps the last parts and says so (`useRetainedFailure`). The view lives in `src/board/CardProgress.tsx` (new), in first load as the work item is.

### Flow

The canvas is redrawn left to right (D10), fitted by both sides (D11), stepped by keys along columns (D12). It loses no click (D13) and names arrows forwards (D14). The List keeps its order and its rows but for D14. The legend's about text and `flow.canvasHint` say left to right. Nothing served changes: `distance`, `stage`, the cut and the gap rows are as read.

## Data

### The children read

```text
GET /api/projects/harbor-sim/cards?with_cancelled=true&parent=C-0001
```

The answer is the card list as served today (`cards.md` "Documents and events", `list`): `{flows, cards, notes, truncated, omitted}`, the rows in the list's order, the browser view never cut (`board-ui.md`). On harbor-sim, `C-0001`'s rows are `C-0019` (copy/edit), `C-0013` (plates/queued), `C-0017` (plates/queued, cancelled) and `C-0016` (plates/struck, accepted). No `marks` is sent: the work item takes its mark from the Board's list (`board-screen.md` "Reads").

The reason read is `GET /api/projects/harbor-sim/cards/C-0017`. It answers `cancellation: {by: "Mira Holt", at: "...", reason: "One plate holds both tables."}`.

### The model (`src/board/rollup.ts`)

```ts
/** A segment's word, in the bar's order: accepted, cancelled, then an open child's category. */
export type ProgressWord = "accepted" | "cancelled" | "returned" | "doing" | "todo" | "stranded";

export interface ProgressSegment {
  word: ProgressWord;
  count: number;
}

export interface Progress {
  /** The children not cancelled, accepted ones too, in served order, each with its hand-off word. */
  children: HandOff[];
  /** The cancelled children, in served order. */
  cancelled: CardRow[];
  /** One per non-zero word, in ProgressWord's order; the counts sum to every child. */
  segments: ProgressSegment[];
  /** The parent open, one child or more, every child accepted or cancelled. */
  ready: boolean;
  /** On an open parent, the children neither accepted nor cancelled, in served order; else empty. */
  reliesOn: CardRow[];
}

/** A card's progress from its children read: rows naming another parent are ignored. */
export function progressOf(rows: readonly CardRow[], flows: readonly CardFlow[], parent: { id: string; open: boolean }): Progress;
```

`rollupsOf`, `HandOff`, `Rollup` and `returnReasonOf` are unchanged: the tiles and the Return reason read them. The words are spelled once, as `category.ts` spells its own.

### Catalog words

These are new keys in both catalogs. English values are below. Plurals are `_one`/`_other` in English and `_one`/`_few`/`_many`/`_other` in Russian (`plural.test.ts`).
- **`board.progress.accepted`** "{{count}} accepted"; **`cancelled`** "{{count}} cancelled"; **`returned`** "{{count}} returned"; **`doing`** "{{count}} in progress"; **`todo`** "{{count}} to do"; **`stranded`** "{{count}} not in the flow". Each is plural.
- **`board.progress.ready`** "Ready to accept"; **`readyAbout`** "Every subtask is accepted or cancelled."
- **`board.progress.reliesOn`** "Relies on"; **`cancelledList`** "Cancelled subtasks".
- **`board.progress.reading`** "Reading the subtasks"; **`failed`** "The subtasks could not be read: {{message}}".
- **`board.progress.reasonReading`** "Reading why it was cancelled"; **`reasonFailed`** "Why it was cancelled could not be read: {{message}}".
- **`flow.edgeName`** "From {{from}} to {{to}}"; **`flow.edgeRow`** "to {{to}}".
- **`flow.legend.about`** becomes "Left to right is the walk from the start; each box sits by its first step. A line under the boxes runs against the walk. A line to a box off the walk shows when one of its ends is selected."
- **`flow.canvasHint`** becomes "The walk runs left to right. The List view holds every box and arrow as text."
- **`app.lazy.roadmap`** "Opening the roadmap"; **`app.lazy.graph`** "Opening the graph".

Reused: `board.modal.handOffs` "Subtasks", `board.panel.noneYet`, `board.panel.byAt`, `board.cancelled`, the shared Retry. No value names a trade, a card or a workspace. Russian lives in `ru.ts` alone and is the ui-developer's, the owner's review open (`ui-languages.md` "Known limits").

### Styles and tokens

```css
/* tokens.css (l.316-321) */
--issue-side-width: 22.5rem;   /* 360 px; was 20.625rem */
--issue-main-min: 20rem;
--modal-width-s: 37.5rem;
--modal-width-m: 50rem;
--modal-width-xl: 75rem;       /* 1200 px; --modal-width-l (60.5rem) goes */

/* app.css */
.card-modal { width: min(var(--modal-width-xl), 100vw - 2rem); max-height: calc(100dvh - 2rem); /* rest as today */ }
.doc-modal { width: min(var(--modal-width-xl), 100vw - 2rem); max-height: calc(100dvh - 2rem); /* rest as today */ }
.card-page { max-width: var(--modal-width-xl); }
.doc-page { max-width: var(--modal-width-xl); /* rest as today */ }
@container issue-view (min-width: 44.5rem) { .issue-view-body { grid-template-columns: minmax(var(--issue-main-min), 1fr) minmax(0, var(--issue-side-width)); } }
.flow-canvas .react-flow__minimap { pointer-events: none; }
.flow-divider { /* a vertical rule: border-left, its words at the top, two lines then an ellipsis */ }
.progress-bar, .progress-segment, .progress-words { /* names the ui-developer's; sizes and space from tokens */ }
```

Each selector keeps its own rule, as today (`board/styles.test.ts` reads `.card-modal`'s). The comments citing 968 px and 330 px (`app.css` l.6529, 6542, 7236; `ui/IssueView.tsx` l.5) say 1200 and 360. No scale exception is added (`scale-exceptions.ts`).

### The layout (`src/flow/layout.ts`)

```ts
export interface FlowBox extends Rect {
  id: string; name: string; node: FlowNode;
  /** Its column on the walk; null in the band. */
  level: number | null;
  /** Its place in its column, top to bottom, or in its band shelf, left to right. */
  order: number;
  shelf: number;
}
/** A stage divider: a vertical rule left of the column where its stages begin, words raw at its top. */
export interface FlowDivider { id: string; level: number; stages: string[]; x: number }
export interface FlowLane extends Rect {
  /** Each walk column's box ids, top to bottom (was `rows`). */
  columns: string[][];
  band: string[][];
  bandY: number | null;
  dividers: FlowDivider[];
}
```

The constants are `COLUMN_GAP` 136, `BOX_GAP` 32, `LANE_PAD` 16 and `DIVIDER_HEIGHT` (two lines of the small size). `BOX_WIDTH` 160 and `BOX_HEIGHT` 92 are unchanged. `FlowCanvas` reads `lanes[0].columns[0][0]` as the first box. `dividerNode(divider, height)` stands at `{x: divider.x, y: 0}`, as wide as its column and as tall as the walk and its arcs. `geometry.ts` exports `FLOW_CLICK_DISTANCE = 5`.

### The mock

- **harbor-sim** (`src/mocks/cards.ts`):
  - `C-0014`, the child of `C-0004`, is accepted: column `struck`, `acceptance {by: "Mira Holt", verdict: "briefed", departs: [], discrepancy: null, note: null}`. So `C-0004`, in `proof` (copy's `accept.from`), is ready beside Accept.
  - `C-0001` is the mixed parent: one accepted (`C-0016`), one cancelled (`C-0017`), one in progress (`C-0019`), one to do (`C-0013`).
  - `C-0003`'s child `C-0015` is returned. `C-0009`'s `C-0018` is to do. `C-0002` has none.
  - Every other card is unchanged.
- Five-child, all-cancelled, dropped-column and unserved-flow parents are unit rows (`aCardRow`), never the mock's.

## Rules and edge cases

- WHEN a card is shown, the work item SHALL read its children once with `parent` and `with_cancelled`, and SHALL read them again on any re-read of the project's card lists. Closing it SHALL read them no more.
- WHEN a child is cancelled, its reason SHALL be read by one `getCard`. An open or accepted child SHALL cause no card read.
- WHEN the parent is open, has a child, and every child is accepted or cancelled (all cancelled included), "Ready to accept" SHALL show. Otherwise it SHALL not. It SHALL never change which acts stand, their order or their state (ADR-0074, ADR-0012).
- WHEN the parent is accepted or cancelled, the bar and both lists SHALL show, and the hint and Relies on SHALL not.
- WHEN an accepted child stands in a column its flow no longer lists, it SHALL count as accepted. An open child in `accept.to` SHALL count as in progress. A child whose flow is not served, or whose column it does not list, SHALL count as not in the flow.
- WHEN a child is in a flow the chosen department hides, it SHALL still count. Hiding is a lens (ADR-0073).
- Only the card's own children SHALL count. A grandchild never counts.
- WHEN the children read is pending or fails, the work item's other parts SHALL stand. No count SHALL be shown from the Board's list in its place.
- Relies on SHALL never name an item (`PR-`) or an agent task (`T-`). An open question on the card SHALL leave Relies on, the bar and Ready as they are (ADR-0077 "The rule").
- The Board tile SHALL keep "<n> subtasks" from the Board's list, which leaves cancelled children out. The tile and the bar count differently, and the canon says so.
- WHEN a pointer press on a Flow box moves 5 px or less before its release, the box SHALL be selected. A press that moves more SHALL pan, and SHALL select nothing.
- WHEN a pointer press focuses a box that is not wholly in view, the view SHALL not move. WHEN Tab or an arrow key focuses one, it SHALL be centred at the current zoom.
- The minimap, a divider and the band's caption SHALL take no pointer event.
- WHEN a picture is wider than the canvas at 0.92, it SHALL land at 0.92 at the left margin. WHEN it is taller, it SHALL land at the top margin. It SHALL never land below 0.92.
- WHEN a stage's words do not fit two lines of their column, they SHALL end in an ellipsis. The box's accessible name keeps the whole stage.
- **No core logic in the UI**: the page judges no card state but by `labels.ts` and `category.ts` look-ups. Kinds, columns and flows are looked up, never compared (`policy.test.ts`).
- **Hub**: the children read and the reason reads are plain reads under the login (`hub.md` "Routes"). Nothing on a hub changes.

### Test churn

Only these change. Every other test passes unchanged. The test-engineer adds what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| `flow/layout.test.ts`: rows, dividers above rows, arcs right of the walk, the band under the last row, the keys, flow200's "12 rows" | columns, dividers left of columns, arcs and band below, the keys of D12, "12 columns" | D10, D12 |
| `flow/geometry.test.ts`: "lets the width decide", the swap and side-loop cases | the fit by both sides; the swap cases go with their functions | D10, D11 |
| `flow/labels.test.ts` l.89-92, 136-144 (`edgeRowWords`, `edgeName`) | D14's frames | D14 |
| `flow/FlowView.test.tsx`: `edgeNamed(...)` labels (l.264, 288), the arrow keys | D14's names, D12's keys | D12, D14 |
| `mocks/flow.smoke.test.tsx`: edge names, the legend's about | D14, the new about | D10, D14 |
| `board/workItem.test.tsx` l.262-267 | 22.5rem, 44.5rem | D15 |
| `board/styles.test.ts` l.113-116 | `--modal-width-xl` (1200 px) | D15 |
| `board/CardModal.test.tsx`, `CardPage.test.tsx`, `BoardView.test.tsx`: the `handOffs` prop; `getCards` counts and `.at(-1)` options with a card open (l.233-317, 576, 1223, 2028; CardModal l.516) | no prop; the children read is the call with `parent`, filtered out of counts that mean the Board's list | D1 |
| `api/queries.test.tsx`, `api/live.test.tsx`: `getCards` counts with a card open | as above | D1 |
| `mocks/board.smoke.test.tsx` l.466-473 (Subtasks' list items) | the rows' list within Subtasks, beside the bar and the Cancelled list | D4, D7 |
| any test pinning `C-0014`'s column or the plates' column counts | `C-0014` accepted in `struck` | "The mock" |
| `app/routes.test.ts`, `mocks/graph.smoke.test.tsx`, the Roadmap's smoke and view tests that query the view right after navigating | `findBy` after the skeleton | D9 |

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red. New tests: `src/board/progress.test.ts` (the model), `src/board/CardProgress.test.tsx` (the view, on stub clients and harbor-sim), `src/app/lazy.test.ts` (the source check), unless another file is named.

- [ ] HP-1 — **The children read.**
  - Opening `C-0001`'s modal, and then its page, each calls `getCards("harbor-sim", {parent: "C-0001", with_cancelled: true})` once, never with `marks`.
  - The Board's own call stays `{marks: true}`, or `{with_cancelled: true, marks: true}` under Show cancelled.
  - A `card.moved` event naming `C-0013` reads it again (`api/live.test.tsx`). After the modal closes, no event reads it.
  - Pending: Subtasks holds a skeleton labelled "Reading the subtasks".
  - Failed: Subtasks holds "The subtasks could not be read: <words>", the words under `lang="en"`, and Retry. Goal, Source, Evidence and Activity stand. A failed re-read keeps the last parts with its line.

  Mutations, each red: the children taken from the Board's list (`rollupsOf`), so `C-0017` is missing; the read sent without `with_cancelled`.
- [ ] HP-2 — **The model.** `progressOf` over `aCardRow` rows:
  - **The words**, in order:
    - accepted, by `accepted`, also in a column its flow dropped;
    - cancelled, by `cancelled`;
    - returned (the flow's `return`);
    - in progress (a middle column, and an open child in `accept.to`);
    - to do (`columns[0]`);
    - not in the flow (an unserved flow, an unlisted column).
  - **Counts and order.** Zero counts are omitted, and the counts sum to the children. Rows of another parent and the parent's own row are ignored. Served order is kept in `children`, `cancelled` and `reliesOn`.
  - **`ready`.** True for an open parent with: one accepted child; one cancelled child; two cancelled; one of each. False with no child, with a returned child, and on an accepted or a cancelled parent.
  - **`reliesOn`.** The open children, neither accepted nor cancelled. Empty on a closed parent.

  Mutations, each red: the category judged before `accepted` (the dropped-column accepted child reads not in the flow); `ready` as every child accepted (the all-cancelled case); segments sorted by count.
- [ ] HP-3 — **The bar.**
  - **On harbor-sim.**
    - `C-0001`'s words line reads "1 accepted · 1 cancelled · 1 in progress · 1 to do", in that order. Each word stands with its icon (`taskAccepted`, `taskCancelled`, `taskInProgress`, `open`).
    - The bar is `aria-hidden`, and its segments' `flex-grow` is 1 each.
    - `C-0003` reads "1 returned" with `cardReturned`; `C-0009` reads "1 to do".
    - `C-0002` reads "None yet." with no bar.
  - **Weights.** A stub parent with 3 accepted and 2 in progress gives `flex-grow` 3 and 2.
  - **Russian.** At 1, 3 and 5 children of one word, the line takes `ru`'s `_one`, `_few` and `_many`.

  Mutations, each red: a segment's words dropped, leaving colour alone; cancelled placed after the categories.
- [ ] HP-4 — **Ready to accept.**
  - `C-0004`, in `proof` with its one child `C-0014` accepted, shows "Ready to accept" and its sentence. Accept stands as on any card in `accept.from`: present, enabled, first of the acts.
  - `C-0001` shows none.
  - A stub parent whose two children are cancelled shows it.
  - A stub parent in its flow's first column, every child accepted, shows it and no Accept.
  - An accepted parent shows none, and neither does a cancelled one.
  - The hint holds no button, link or `aria-disabled`. The acts' accessible names and order are byte-equal with and without it.

  Mutations, each red: Accept hidden until ready; the hint on a closed parent.
- [ ] HP-5 — **Relies on.**
  - On `C-0001` it lists "Copy desk C-0019" then "plates C-0013", each ID a link. A plain click replaces the modal's card (`#/harbor-sim/board/C-0019`).
  - It lists no `C-0016` and no `C-0017`.
  - `C-0008`, which holds the open question `PR-0055` and no child, has none.
  - A stub card with a linked `T-` task and an open item but no child has none.
  - `C-0004`, which is ready, has none, and neither does an accepted parent.

  Mutations, each red: an item's ID in the list; an accepted child in it.
- [ ] HP-6 — **Cancelled, with reasons.**
  - `C-0001`'s "Cancelled subtasks" lists `C-0017`: "Second tide plate", plates, its lozenge, Cancelled, "One plate holds both tables." and "By Mira Holt, <date>".
  - The call log holds one `getCard("harbor-sim", "C-0017")`, and none for `C-0013`, `C-0016` or `C-0019`.
  - Pending: "Reading why it was cancelled".
  - Failed: "Why it was cancelled could not be read: <words>". The bar, the rows and, on the all-cancelled stub, "Ready to accept" still stand.

  Mutations, each red: `getCard` for every child; Ready waiting on the reasons.
- [ ] F-1 — **Columns.** `flow/layout.test.ts`, on the pottery (`src/test/asideStub.ts`) and the stubs:
  - Distances 0, 4 and 9 give three columns. x grows with distance, one distance shares one x, and no column is empty.
  - In the firing run, Mending and Packing share a column, one above the other.
  - Every forward edge runs left to right: its source box's right edge is left of its target's left edge, and its points are clear of every box. A long one takes a passage in each column it skips.
  - flow200: 200 boxes, 12 columns, 30 in the band, 40 edges against the walk. No box overlaps another. The same picture comes from the reverse order, in under 500 ms.

  Mutation, red: the swapped forms kept, which draws top to bottom.
- [ ] F-2 — **Arcs and the band below.**
  - **Arcs.** Every arc's points lie below every walk box's bottom, among them the firing run's repeat, Glost firing back to Dipping in glaze. Each arc has its own track (a distinct y), and the shorter is nearer the walk. No arc point falls in a box.
  - **Loops.** An edge to itself lies under its box, clear of the box below it.
  - **The band.** Its boxes lie below the lowest arc track, its caption over them, wrapped at four boxes on a narrow walk. An edge to or from it crosses no box.

  Mutation, red: arcs on tracks right of the walk.
- [ ] F-3 — **Stage dividers.**
  - The firing run has four: wheelwork, bisque, glazing and crating, opening columns 0, 3, 5 and 7.
  - Each `x` lies left of its column's boxes and right of the previous column's.
  - Each rule spans from the words' strip, above every walk box, to below the lowest arc track.
  - There is none in a stage view, none for fewer than two stages, none for a band box.
  - Stages beginning in one column share one, their words joined ` · ` in column order.

  Mutation, red: a divider at its column's right edge.
- [ ] F-4 — **The fit.** `flow/geometry.test.ts` `fitOf` on a 1100 x 700 canvas:
  - a 900 x 400 picture: zoom 1, centred both ways;
  - a 1100 x 400 picture: zoom (1100 - 2 margins) / 1100, centred;
  - a 3000 x 400 picture: zoom 0.92, x at the margin, y centred;
  - an 800 x 2000 picture: zoom 0.92, y at the margin;
  - a canvas with no size: null.

  Mutations, each red: the width alone deciding (the 800 x 2000 picture reads 1); a floor below 0.92.
- [ ] F-5 — **Keys.**
  - **`boxAfterKey`.**
    - Right and Left reach the nearest box of the next or previous column, and stay at either end.
    - Up and Down move within a column. Down from a column's last box reaches the band's first shelf, nearest by horizontal centre.
    - In the band, Left and Right move along a shelf and Up and Down cross shelves. Up from the first shelf reaches the nearest column's last box.
    - An unknown key or box stays.
  - **`FlowView.test.tsx`.** ArrowRight from the first box focuses the second column's nearest box, and the tab stop moves with it.

  Mutation, red: Up and Down going across columns.
- [ ] F-6 — **Words.**
  - `en.flow.legend.about` starts "Left to right". `en.flow.canvasHint` says the walk runs left to right. Neither holds "top" or "bottom".
  - Both Russian values differ from the English ones. `catalog.test.ts` passes.
  - The legend shows the new about text (`FlowView.test.tsx`).

  Mutation, red: the about text left at top to bottom.
- [ ] C-1 — **A press that moves a little is a click.** `flow/FlowView.test.tsx` in jsdom, each mouse event with `view: window` and `button: 0`:
  - mousedown on a box at (100, 100), mousemove to (103, 101), mouseup, click: the box is selected (its hash by a replace, the panel's heading its title);
  - a second box the same way: selected, and the first no longer;
  - a press that moves 8 px pans and selects nothing.

  Should jsdom not run d3-zoom's gesture, the test-engineer records why in "Implementation". The orchestrator then runs these presses in Chrome, at box centres on harbor-sim at 1440x900, as in the repro.

  Mutation, red: `paneClickDistance` removed. At xyflow's default of 1 px, the 3 px press selects nothing.
- [ ] C-2 — **The minimap takes no pointer.** `flow/styles.test.ts`: `.flow-canvas .react-flow__minimap` holds `pointer-events: none`.

  Mutation, red: the declaration dropped.
- [ ] C-3 — **A press moves nothing.** `FlowView.test.tsx`, with the store's size and transform set so a box lies half outside the view:
  - pointerdown, focus, pointerup and click on it: the transform is unchanged and the box is selected;
  - Tab, then an arrow key bringing focus to a box out of view: it is centred, as today.

  Mutation, red: centring on every focus.
- [ ] C-4 — **Words take no pointer.** `FlowView.test.tsx`:
  - every `.react-flow__node-divider` and `.react-flow__node-band` element has inline `pointer-events: none`;
  - every `.react-flow__node-box` element has `all`.

  Mutation, red: `WORDS_ONLY` without its `style`.
- [ ] E-1 — **Edge names read forwards.** `flow/labels.test.ts`, `FlowView.test.tsx`, `mocks/flow.smoke.test.tsx`:
  - **`edgeName`.**
    - For `{from: "A", to: "B", type: "second_link"}` with titles "Kiln bay" and "Glaze room": "From Kiln bay to Glaze room, second_link".
    - With `kind: "repeat"`, the repeat word follows. In Russian, `ru`'s frame and repeat word.
    - A box with no title gives its name.
  - **`edgeRowWords`.** `edgeRowWords(edge, "Glaze room", enT)` at count 5 is "to Glaze room, second_link, ×5". Kind and `when` follow the type, as today.
  - **On harbor-sim.** The arrow from `RULE-HIGH-WATER` to `RULE-SPRING-WINDOW` is named "From <its title> to <its title>, follows", the titles read from the mock.
  - `policy.test.ts` passes: no link type is quoted.

  Mutation, red: `from` and `to` swapped.
- [ ] M-1 — **Wider work items.** `board/workItem.test.tsx`, `board/styles.test.ts`, `doc/DocModal.test.tsx`:
  - `tokens.css` holds `--modal-width-xl: 75rem;` and `--issue-side-width: 22.5rem;`, and no `--modal-width-l`;
  - `.card-modal` and `.doc-modal` hold `width: min(var(--modal-width-xl), 100vw - 2rem);` and `max-height: calc(100dvh - 2rem);`;
  - `.card-page` and `.doc-page` hold `max-width: var(--modal-width-xl);`;
  - `@container issue-view (min-width: 44.5rem)` keeps today's grid, and the narrow rule `minmax(0, 1fr)` stands.

  Mutations, each red: `.doc-modal` left at `--modal-width-l`; the container query left at 42.5rem.
- [ ] M-2 — **[look] The widths.**
  - At 1440x900 the card modal and the document modal are 1200 px wide, the side column 360 px and the main about 750 px.
  - At 1100 the modal is the viewport less 2rem, two columns. Below 44.5rem of its own width it is one column, main first. At 320 it is the screen.
  - Both themes, both languages; the page is as wide as the modal.
- [ ] S-1 — **[measure] Size first.**
  - `src/app/lazy.test.ts` reads `Shell.tsx`: no value import of `../roadmap/RoadmapView` or `../graph/GraphView`, each reached by `lazy(() => import(...))`, the types by `import type`.
  - The Roadmap and Graph routes show their skeleton (`app.lazy.*`), then the view.
  - A palette jump to either still focuses the view's `h1` once the chunk lands (`Shell.tsx` l.344). The Suspense boundary resolves without re-rendering the Shell, so the jump's effect must not depend on that re-render.
  - After `pnpm build`, first-load JS (`ui-languages.md` "Size") is recorded three times: after `RoadmapView`'s move, after `GraphView`'s, and at the end. The last is at most 845 kB, expected about 825. Gzip and `ru-*.js` are recorded too.
  - Neither view's chunk is in `dist/index.html`'s `modulepreload` list.

  Mutations, each red: the static import of `GraphView` restored (the source test); the jump's focus run only on the Shell's own render (the jump test).
- [ ] L-1 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide. On the mock, and on a scratch copy of a real game corpus whose `[card]` has two departments' flows and a parent with children accepted, cancelled, returned and in progress, made for the look:
  - **The work item.** The bar, Ready to accept beside Accept, Relies on, and Cancelled with reasons on `C-0001`, `C-0003` and `C-0004`. Long Russian words wrap and nothing is cut unseen.
  - **Flow left to right**, on harbor-sim and on the corpus:
    - each divider stands left of its column, its words at the top, long ones in two lines then an ellipsis;
    - arcs and the band stand below;
    - a long walk fits at 0.92 from the left and is panned by the wheel;
    - the minimap shows only while the picture is not whole in view.
  - **Clicks.**
    - Ten boxes in a row, each selected by a real mouse click with a slight move, none lost.
    - A box partly under the minimap is selected through it.
    - A half-visible box that is clicked stays under the pointer.
  - **The accessibility tree.** An arrow's name reads "From ... to ..., follows".
  - **The Roadmap and Graph routes.** The skeleton, then the view. Focus rings are visible.

  Build both binaries first: `spec serve` falls back to the `specengine-http` installed in cargo's bin directory when the debug build has none.
- [ ] G-1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full test run green twice.
  - **Rust.** There is no Rust code. Run `cargo nextest run -p specengine-eval --test doc_pointers --test ui_policy --test anonymity`.
  - **Docs.** The check is clean.

## Out of scope

- Any served change: no `children` on a card read, no `cancellation` on a list row, no Rust, no plugin version.
- The Board tile: no bar, no Ready, its count unchanged. A grandchild's rollup.
- A parent closing, moving or being refused for its children. Accept moved, hidden or disabled by Ready (ADR-0074).
- Items and agent tasks in Relies on or in the bar.
- The click fix in Graph and Roadmap, and their minimaps: a pool item at ship.
- Moving the minimap. Flow's List order. Graph's or Roadmap's direction.
- Counts per department (RM-0086), the design sweep (RM-0099), and any step after 6.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| ui-developer | `src/api/queries.ts` (`useChildren`, `queryKeys.children`); `src/board/{rollup.ts,labels.ts,CardPanel.tsx,CardPage.tsx,BoardView.tsx}`, `CardProgress.tsx` (new); `src/flow/{layout.ts,geometry.ts,FlowCanvas.tsx,labels.ts,FlowList.tsx,FlowLegend.tsx}`; `src/app/Shell.tsx`; `src/styles/{tokens.css,app.css}`; `src/ui/IssueView.tsx` (a comment); `src/i18n/{en,ru}.ts`; `src/mocks/cards.ts`; the tests of HP-1 to HP-6, F-1 to F-6, C-1, C-3, C-4, E-1 and S-1's source check, with "Test churn"'s rows |
| test-engineer, UI | gates and mutations; S-1's figures; C-1 in Chrome if jsdom cannot run it; C-2 and M-1's pins (`flow/styles.test.ts`, `board/styles.test.ts`, `workItem.test.tsx`, `doc/DocModal.test.tsx`); `board/words.test.ts`'s pin (D8); `policy.test.ts` if a scan moves; `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/handoff-progress.md` "Acceptance criteria" with a criterion ID, or an existing canon heading. They never cite "Data" or "Description and interactions", which compaction drops at ship (doc_pointers).

### Work order

The UI lane alone; there is no Rust lane.

1. **Size first** (D9). `RoadmapView` goes lazy; build and record. Then `GraphView`; build and record.
2. **The small fixes**: the clicks (C-1 to C-4), the edge names (E-1) and the widths (M-1). Each stands alone.
3. **Flow left to right** (F-1 to F-6).
4. **The progress** (HP-1 to HP-6), with the mock's `C-0014`.
5. The UI test-engineer runs the gates, the mutations and S-1's last figure. Then the UI review.
6. The orchestrator's looks (M-2, L-1; C-1 in Chrome if needed), on the mock and the corpus copy.
7. Ship: the spec-writer ("Canon at ship").

At most three iterations. There is no cut line: the owner asked for all five parts. The order lands the small fixes first. A part left unfinished is reported, never shipped half (`CLAUDE.md` "Process").

### Risks

- **The accept guard.** `board/words.test.ts` bans "accept" in hand-off, return and rollup words. "Ready to accept" is the owner's word, so it sits outside the scan, pinned (D8).
- **Reads per card.** One more list read per opened card. Then one card read per cancelled child: a parent with 30 cancelled children makes 30, each `show --history`. This is unmeasured, bounded by the cancelled count, and noted as a known limit.
- **Left to right runs off the right.** About four columns fit at 1440 at 0.92; the rest is panned. The owner's look judges it (Q-C).
- **Long stage words** in a column's width: two lines, then an ellipsis.
- **Words-only nodes swallowing presses** (C-4). xyflow gives every node `pointer-events: all` once `onNodeClick` is set.
- **jsdom and d3-zoom.** The gesture needs `view` on each event. Failing that, C-1 runs in Chrome.
- **Lazy views.** Focus and the first query after a navigation wait for the chunk: `findBy`. A palette jump's heading focus must survive the skeleton (S-1).
- **flow200.** Its numbers change shape, from rows to columns. The 500 ms bound holds.
- **Tile against bar.** The tile counts what the Board's list holds; the bar counts every child (Rules).
- **Caps at ship.** `board-screen.md` [146], `spec-flow-screen.md` [75] and `ui-design.md` [17] are near full. The progress takes a new Tier 2 file ("Canon at ship").
- **Size.** 4.21 kB of headroom before the two moves. The progress view is in first load.
- **Anonymity.** The looks' real corpus is never named in the repository.

### Canon at ship

A new Tier 2 file, `docs/canon/card-progress.md` (`scope: [ui]`, a `card*` path, so its index line goes to `docs/index-canon-card.md`), at most 6 KB, holds:
- the children read and its key;
- the model and its words' order;
- the bar;
- Ready to accept;
- Relies on;
- Cancelled and the reason reads;
- the words and Q3's pin;
- Known limits: tile against bar, one read per cancelled child, one level;
- Tests.

Each edit below stays within its cap; free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/spec-flow-layout.md` [7 806] | "Layout": columns left to right, routes, dividers left of columns, the band below the arcs; the fit by both sides; clicks (5 px, the minimap and words-only nodes take no pointer, a press moves nothing). "Forms": the legend's about text, edge names |
| `docs/canon/spec-flow-screen.md` [75] | "Studio" Canvas: left to right; "Keyboard, motion, width" Keys (D12); "Known limits" Minimap: a pointer to "Layout", and Graph and Roadmap losing a click on a short move (net not positive) |
| `docs/canon/board-screen.md` [146] | "Reads": the work item's children read, a pointer; "Writes and dialogs" Rollup: Subtasks by pointer (net not positive) |
| `docs/canon/card-flows.md` [356] | "Children": the work item's progress, a pointer |
| `docs/canon/ui-design.md` [17] | "Type, space and size": `--modal-width-s\|m\|xl` 600, 800, 1200 px, the side 360; "The work item": Main's Subtasks by pointer, Side 360 px, Width 44.5rem and 1200 px (net not positive) |
| `docs/canon/doc-view.md` [2 059] | "Side", at most 360 px |
| `docs/canon/ui-languages.md` [678] | "Size": the figure at RM-0098; `RoadmapView` and `GraphView` among the lazy chunks |
| `ui/README.md` [2 057] | "Canvas": Flow's press of up to 5 px is a click |
| `CLAUDE.md` [4 747]; 08 s2 Phase 7 step 6; RM-0098 | shipped; next the pipeline trial |
| a pool item, the next free `RM-` | Graph's and Roadmap's press distance and minimap (D13), no release |
| this spec | compacted to at most 3 KB |

Q14 is closed in 08 at spec time; nothing else changes before ship. No ADR is issued.

### Working answers and the owner's questions

**The orchestrator's, taken:**
- The reasons are read per cancelled child with `getCard`, UI only, with no served change (D2).
- Relies on lists the open subtasks only: never an item, never a `T-` task (D6).
- A long walk pans at the 0.92 floor and never shrinks below it (D11).
- A parent is ready when every child is accepted or cancelled, all cancelled included (D5).
- The click fix is Flow's alone. Graph and Roadmap are a known limit and a follow-up (D13).
- "Ready to accept" is the owner's word (v5 Q3). It is keyed outside the guard's scan, which stays as it is, plus a pin (D8).
- Size first: `RoadmapView`, then `GraphView`, lazy with skeletons (D9).
- No ADR. Left to right is a canon change at ship; Q14 is closed in 08 now.

**The spec-writer's:**
- W1: the work item reads its children itself, so the modal and the page share one read and one key, and `handOffs` leaves its props.
- W2: an open child in `accept.to` counts as in progress. It waits on an accept, and only a config change puts it there.
- W3: Ready and Relies on show on an open parent only. The bar and the lists show on any.
- W4: Ready is judged from the children read alone. A pending or failed reason never hides it.
- W5: the pages widen with the modals, so a page's main column is never narrower than the modal's (968 px with a 360 px side would leave about 576 px). `--modal-width-l` goes with its last use.
- W6: the minimap stays bottom-left and takes no pointer. Moving it was not asked.
- W7: the accept guard is `src/board/words.test.ts`. `src/i18n/words.test.ts` guards the departments' words and the trades, and is unchanged.
- W8: the mock's `C-0014` is accepted, so a ready parent (`C-0004`) stands in `accept.from` beside Accept. The all-cancelled case is a unit row.
- W9: an arrow's name uses titles, as the owner reads them, the type raw after them. The List drops the type-first form for the same order.
- W10: `paneClickDistance` 5 and no `nopan` on boxes, so a drag that starts on a box still pans a dense picture and the page under 64rem.
- W11: the analysis gave xyflow's default pane click distance as 0. 12.12.0's `ReactFlow` defaults it to 1 px. The effect is the same: 2-3 px is a pan.

**The owner's questions:**
- **Q-A.** Should the Board's tile show the bar or Ready? Working answer: no. Step 6 is the work item. RM-0086 counts by department.
- **Q-B.** Should the minimap move? Working answer: no. It takes no pointer (D13).
- **Q-C.** At 1440 about four columns fit at 0.92. Should a long walk shrink below it? Working answer: no. It is panned; titles stay at 12.9 px or more (D11).
