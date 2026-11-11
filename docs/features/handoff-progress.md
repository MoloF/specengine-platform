---
class: spec
status: shipped
shipped: 2026-11-11
scope: [ui]
ref: RM-0098, v5 step 6 (08 s2 Phase 7, "v5 scope"); the owner's v5 Q3 of 2026-11-10; the owner's four requests of 2026-11-11 (Flow left to right, which closes Q14; lost clicks on Flow; wider work-item modals; step 6 and nothing further); the analyst's requirement analysis of 2026-11-11; the orchestrator's click-bug repro and working answers
adrs: []
---

# handoff-progress: a parent's progress, Ready to accept and Relies on, with Flow left to right again (RM-0098)

## Why

The owner's answer of 2026-11-10 (v5 Q3): a parent task shows "Ready to accept" and a progress bar, and a person accepts it. Nothing closes a parent on its own (ADR-0074 stands). His pipeline's step 5: "the task closes when every subtask is done, or resolved with a comment". The work item listed a parent's children from the Board's one list read, which leaves cancelled cards out: no total, no cancelled child, no reason, nothing saying what the task waits on.

His requests of 2026-11-11, translated:
1. "Flow should run left to right, as it always did, not top to bottom." His look closes Q14.
2. "When I click blocks in the flow chart they get stuck; after clicking one I can't click another." The orchestrator's repro, on a debug daemon over a scratch copy of a real game corpus at 1440x900: a press moving 2-3 px was taken as a pan by d3-zoom (xyflow 12.12.0's `paneClickDistance` defaults to 1 px; boxes carry no `nopan`), the minimap covered the lower-left box at fit, and `onFocus` centred a half-visible box from under the pointer.
3. "The work-item modals should be noticeably wider": about 1200 px, with a 360 px side column.
4. Take step 6 (RM-0098), "but only it, nothing further".

A side finding: an arrow's accessible name read backwards ("A follows B" for `B follows: A`).

UI only, no ADR. Its rules: ADR-0074 (no card write waits on another), ADR-0012 (nothing is blocked), ADR-0077 "The rule" (no rollup reads an item), ADR-0008, ADR-0033 and ADR-0055 (Flow lifts nothing).

## Description and interactions

Each decision is canon now; the sections named are `card-progress.md`'s unless another file is.

- **D1** The children read, **D2** one reason read per cancelled child: "Reads". **D3** The model: "The model". **D4** The bar, **D5** Ready to accept, **D6** Relies on, **D7** Cancelled subtasks: "Screen", "The rule". **D8** The words and the accept guard: "Words".
- **D9** Size first: `ui-languages.md` "Size".
- **D10** Flow left to right, **D11** the fit, **D13** clicks: `spec-flow-layout.md` "Layout". **D12** Keys: `spec-flow-screen.md` "Keyboard, motion, width". **D14** Edge names: `spec-flow-layout.md` "Forms".
- **D15** Wider work items: `ui-design.md` "Type, space and size", "The work item".

Working answers taken: reasons read per child (D2); Relies on lists open subtasks only (D6); a long walk pans at 0.92, never shrinks (D11, Q-C); ready when every child is accepted or cancelled, all cancelled included (D5); the click fix Flow's alone (D13, RM-0103); "Ready to accept" keyed outside the guard's scan, pinned (D8); the tile shows no bar or Ready (Q-A); the minimap stays bottom-left (Q-B). The owner's review of Q-A to Q-C is open.

## Data

No served change: the read is `GET .../cards?with_cancelled=true&parent=C`, the reason `GET .../cards/<child>` (`cards.md` "Documents and events"). The model's types: `rollup.ts` `Progress`; the layout's: `layout.ts` `FlowLane`, `FlowDivider`; the tokens: `ui-design.md` "Type, space and size".

## Rules and edge cases

In the canon: `card-progress.md` "The rule", "The model"; `spec-flow-layout.md` "Layout".

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turned red. New tests: `src/board/progress.test.ts` (the model), `src/board/CardProgress.test.tsx` (the view, on stub clients), `src/mocks/progress.smoke.test.tsx` (harbor-sim), `src/app/lazy.test.ts` (the source check), `src/app/lazyJump.test.tsx` (the jump), unless another file is named.

- [x] HP-1 — **The children read.**
  - Opening `C-0001`'s modal, and then its page, each calls `getCards("harbor-sim", {parent: "C-0001", with_cancelled: true})` once, never with `marks`.
  - The Board's own call stays `{marks: true}`, or `{with_cancelled: true, marks: true}` under Show cancelled.
  - A `card.moved` event naming `C-0013` reads it again (`api/live.test.tsx`). After the modal closes, no event reads it.
  - Pending: Subtasks holds a skeleton labelled "Reading the subtasks".
  - Failed: Subtasks holds "The subtasks could not be read: <words>", the words under `lang="en"`, and Retry. Goal, Source, Evidence and Activity stand. A failed re-read keeps the last parts with its line.

  Mutations, each red: the children taken from the Board's list (`rollupsOf`), so `C-0017` is missing; the read sent without `with_cancelled`.
- [x] HP-2 — **The model.** `progressOf` over `aCardRow` rows:
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
- [x] HP-3 — **The bar.**
  - **On harbor-sim.**
    - `C-0001`'s words line reads "1 accepted · 1 cancelled · 1 in progress · 1 to do", in that order. Each word stands with its icon (`taskAccepted`, `taskCancelled`, `taskInProgress`, `open`).
    - The bar is `aria-hidden`, and its segments' `flex-grow` is 1 each.
    - `C-0003` reads "1 returned" with `cardReturned`; `C-0009` reads "1 to do".
    - `C-0002` reads "None yet." with no bar.
  - **Weights.** A stub parent with 3 accepted and 2 in progress gives `flex-grow` 3 and 2.
  - **Russian.** At 1, 3 and 5 children of one word, the line takes `ru`'s `_one`, `_few` and `_many`.

  Mutations, each red: a segment's words dropped, leaving colour alone; cancelled placed after the categories.
- [x] HP-4 — **Ready to accept.**
  - `C-0004`, in `proof` with its one child `C-0014` accepted, shows "Ready to accept" and its sentence. Accept stands as on any card in `accept.from`: present, enabled, first of the acts.
  - `C-0001` shows none.
  - A stub parent whose two children are cancelled shows it.
  - A stub parent in its flow's first column, every child accepted, shows it and no Accept.
  - An accepted parent shows none, and neither does a cancelled one.
  - The hint holds no button, link or `aria-disabled`. The acts' accessible names and order are byte-equal with and without it.

  Mutations, each red: Accept hidden until ready; the hint on a closed parent.
- [x] HP-5 — **Relies on.**
  - On `C-0001` it lists `C-0019` (Copy desk) then `C-0013` (plates), each line as a Subtasks row: its ID a link, title, flow title (iteration 2, L2; iteration 1 showed the flow title and ID alone). A plain click replaces the modal's card (`#/harbor-sim/board/C-0019`).
  - It lists no `C-0016` and no `C-0017`.
  - `C-0008`, which holds the open question `PR-0055` and no child, has none.
  - A stub card with a linked `T-` task and an open item but no child has none.
  - `C-0004`, which is ready, has none, and neither does an accepted parent.

  Mutations, each red: an item's ID in the list; an accepted child in it.
- [x] HP-6 — **Cancelled, with reasons.**
  - `C-0001`'s "Cancelled subtasks" lists `C-0017`: "Second tide plate", plates, its lozenge, Cancelled, "One plate holds both tables." and "By Mira Holt, <date>".
  - The call log holds one `getCard("harbor-sim", "C-0017")`, and none for `C-0013`, `C-0016` or `C-0019`.
  - Pending: "Reading why it was cancelled".
  - Failed: "Why it was cancelled could not be read: <words>". The bar, the rows and, on the all-cancelled stub, "Ready to accept" still stand.

  Mutations, each red: `getCard` for every child; Ready waiting on the reasons.
- [x] F-1 — **Columns.** `flow/layout.test.ts`, on the pottery (`src/test/asideStub.ts`) and the stubs:
  - Distances 0, 4 and 9 give three columns. x grows with distance, one distance shares one x, and no column is empty.
  - In the firing run, Mending and Packing share a column, one above the other.
  - Every forward edge runs left to right: its source box's right edge is left of its target's left edge, and its points are clear of every box. A long one takes a passage in each column it skips.
  - flow200: 200 boxes, 12 columns, 30 in the band, 40 edges against the walk. No box overlaps another. The same picture comes from the reverse order, in under 500 ms.

  Mutation, red: the swapped forms kept, which draws top to bottom.
- [x] F-2 — **Arcs and the band below.**
  - **Arcs.** Every arc's points lie below every walk box's bottom, among them the firing run's repeat, Glost firing back to Dipping in glaze. Each arc has its own track (a distinct y), and the shorter is nearer the walk. No arc point falls in a box.
  - **Loops.** An edge to itself lies under its box, clear of the box below it.
  - **The band.** Its boxes lie below the lowest arc track, its caption over them, wrapped at four boxes on a narrow walk. An edge to or from it crosses no box.

  Mutation, red: arcs on tracks right of the walk.
- [x] F-3 — **Stage dividers.**
  - The firing run has four: wheelwork, bisque, glazing and crating, opening columns 0, 3, 5 and 7.
  - Each `x` lies left of its column's boxes and right of the previous column's.
  - Each rule spans from the words' strip, above every walk box, to below the lowest arc track.
  - There is none in a stage view, none for fewer than two stages, none for a band box.
  - Stages beginning in one column share one, their words joined ` · ` in column order.

  Mutation, red: a divider at its column's right edge.
- [x] F-4 — **The fit.** `flow/geometry.test.ts` `fitOf` on a 1100 x 700 canvas:
  - a 900 x 400 picture: zoom 1, centred both ways;
  - a 1100 x 400 picture: zoom (1100 - 2 margins) / 1100, centred;
  - a 3000 x 400 picture: zoom 0.92, x at the margin, y centred;
  - an 800 x 2000 picture: zoom 0.92, y at the margin;
  - a canvas with no size: null.

  Mutations, each red: the width alone deciding (the 800 x 2000 picture reads 1); a floor below 0.92.
- [x] F-5 — **Keys.**
  - **`boxAfterKey`.**
    - Right and Left reach the nearest box of the next or previous column, and stay at either end.
    - Up and Down move within a column. Down from a column's last box reaches the band's first shelf, nearest by horizontal centre.
    - In the band, Left and Right move along a shelf and Up and Down cross shelves. Up from the first shelf reaches the nearest column's last box.
    - An unknown key or box stays.
  - **`FlowView.test.tsx`.** ArrowRight from the first box focuses the second column's nearest box, and the tab stop moves with it.

  Mutation, red: Up and Down going across columns.
- [x] F-6 — **Words.**
  - `en.flow.legend.about` starts "Left to right". `en.flow.canvasHint` says the walk runs left to right. Neither holds "top" or "bottom".
  - Both Russian values differ from the English ones. `catalog.test.ts` passes.
  - The legend shows the new about text (`FlowView.test.tsx`).

  Mutation, red: the about text left at top to bottom.
- [x] C-1 — **A press that moves a little is a click.** `flow/FlowView.test.tsx` in jsdom, each mouse event with `view: window` and `button: 0`:
  - mousedown on a box at (100, 100), mousemove to (103, 101), mouseup, click: the box is selected (its hash by a replace, the panel's heading its title);
  - a second box the same way: selected, and the first no longer;
  - a press that moves 8 px pans and selects nothing.

  jsdom ran d3-zoom's gesture once each event's `view` was set by `defineProperty` (departure 11).

  Mutation, red: `paneClickDistance` removed. At xyflow's default of 1 px, the 3 px press selects nothing.
- [x] C-2 — **The minimap takes no pointer.** `flow/styles.test.ts`: `.flow-canvas .react-flow__minimap` holds `pointer-events: none`.

  Mutation, red: the declaration dropped.
- [x] C-3 — **A press moves nothing.** `FlowView.test.tsx`, with the store's size and transform set so a box lies half outside the view:
  - a pointer press that focuses it, the focus not `:focus-visible` (`matches` stubbed), then pointerup and click: the transform is unchanged and the box is selected (iteration 2, R1; iteration 1 tracked the press from `pointerdown`);
  - Tab, then an arrow key bringing focus to a box out of view: it is centred, as today.

  Mutation, red: centring on every focus.
- [x] C-4 — **Words take no pointer.** `FlowView.test.tsx`:
  - every `.react-flow__node-divider` and `.react-flow__node-band` element has inline `pointer-events: none`;
  - every `.react-flow__node-box` element has `all`.

  Mutation, red: `WORDS_ONLY` without its `style`.
- [x] E-1 — **Edge names read forwards.** `flow/labels.test.ts`, `FlowView.test.tsx`, `mocks/flow.smoke.test.tsx`:
  - **`edgeName`.**
    - For `{from: "A", to: "B", type: "second_link"}` with titles "Kiln bay" and "Glaze room": "From Kiln bay to Glaze room, second_link".
    - With `kind: "repeat"`, the repeat word follows. In Russian, `ru`'s frame and repeat word.
    - A box with no title gives its name.
  - **`edgeRowWords`.** `edgeRowWords(edge, "Glaze room", enT)` at count 5 is "to Glaze room, second_link, ×5". Kind and `when` follow the type, as today.
  - **On harbor-sim.** The arrow from `RULE-HIGH-WATER` to `RULE-SPRING-WINDOW` is named "From <its title> to <its title>, follows", the titles read from the mock.
  - `policy.test.ts` passes: no link type is quoted.

  Mutation, red: `from` and `to` swapped.
- [x] M-1 — **Wider work items.** `board/workItem.test.tsx`, `board/styles.test.ts`, `doc/DocModal.test.tsx`:
  - `tokens.css` holds `--modal-width-xl: 75rem;` and `--issue-side-width: 22.5rem;`, and no `--modal-width-l`;
  - `.card-modal` and `.doc-modal` hold `width: min(var(--modal-width-xl), 100vw - 2rem);` and `max-height: calc(100dvh - 2rem);`;
  - `.card-page` and `.doc-page` hold `max-width: var(--modal-width-xl);`;
  - `@container issue-view (min-width: 44.5rem)` keeps today's grid, and the narrow rule `minmax(0, 1fr)` stands.

  Mutations, each red: `.doc-modal` left at `--modal-width-l`; the container query left at 42.5rem.
- [x] M-2 — **[look] The widths.**
  - At 1440x900 the card modal and the document modal are 1200 px wide, the side column 360 px and the main about 750 px.
  - At 1100 the modal is the viewport less 2rem, two columns. Below 44.5rem of its own width it is one column, main first. At 320 it is the screen.
  - Both themes, both languages; the page is as wide as the modal.
- [x] S-1 — **[measure] Size first.**
  - `src/app/lazy.test.ts` reads `Shell.tsx`: no value import of `../roadmap/RoadmapView` or `../graph/GraphView`, each reached by `lazy(() => import(...))`, the types by `import type`.
  - The Roadmap and Graph routes show their skeleton (`app.lazy.*`), then the view.
  - A palette jump to either still focuses the view's `h1` once the chunk lands; on Graph with no REF its REF field then takes the focus, as before the move (departure 6). The Suspense boundary resolves without re-rendering the Shell, so `Landed` runs the jump's focus when the view commits.
  - After `pnpm build`, first-load JS (`ui-languages.md` "Size") is recorded three times: after `RoadmapView`'s move, after `GraphView`'s, and at the end. The last is at most 845 kB, expected about 825. Gzip and `ru-*.js` are recorded too ("Implementation", Numbers).
  - Neither view's chunk is in `dist/index.html`'s `modulepreload` list.

  Mutations, each red: the static import of `GraphView` restored (the source test); the jump's focus run only on the Shell's own render (the jump test).
- [x] L-1 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide. On the mock, and on a scratch copy of a real game corpus whose `[card]` has two departments' flows and a parent with children accepted, cancelled, returned and in progress, made for the look:
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
- [x] G-1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full test run green twice.
  - **Rust.** There is no Rust code. Run `cargo nextest run -p specengine-eval --test doc_pointers --test ui_policy --test anonymity`.
  - **Docs.** The check is clean.

## Out of scope

- Any served change: no `children` on a card read, no `cancellation` on a list row, no Rust, no plugin version.
- The Board tile: no bar, no Ready, its count unchanged. A grandchild's rollup.
- A parent closing, moving or being refused for its children. Accept moved, hidden or disabled by Ready (ADR-0074).
- Items and agent tasks in Relies on or in the bar.
- The click fix in Graph and Roadmap, and their minimaps: RM-0103 in the pool.
- Moving the minimap. Flow's List order. Graph's or Roadmap's direction.
- Counts per department (RM-0086), the design sweep (RM-0099), and any step after 6.

## Implementation

| Module | Does |
|---|---|
| `src/api/queries.ts` | `useChildren`, `queryKeys.children` |
| `src/board/{rollup,labels}.ts`, `CardProgress.tsx`, `CardLink.tsx` | `progressOf`; the words' looks; Subtasks' parts |
| `src/board/{CardPanel,CardModal,CardPage,BoardView}.tsx` | the work item reads its own children; `handOffs` gone |
| `src/flow/{layout,geometry,labels}.ts`, `FlowCanvas.tsx`, `FlowView.tsx` | columns; `fitOf`; `boxAfterKey`; `FLOW_CLICK_DISTANCE`, `WORDS_ONLY`, the focus rule; `edgeName`, `edgeRowWords` |
| `src/app/Shell.tsx` | `RoadmapView`, `GraphView` lazy with skeletons; `Landed` |
| `src/styles/{tokens,app}.css`, `ui/IssueView.tsx` | the widths; the minimap, dividers, bar |
| `src/i18n/{en,ru}.ts`, `src/mocks/cards.ts` | the new keys; `C-0014` accepted |

**Numbers.** UI 5 063 tests in 172 files, twice; lint 0, `tsc` clean; eval 58/58; every named mutation red, iteration 2's three too (the gap back at 136, Relies on without the title, centring on every focus). S-1: first load 829.91 kB after `RoadmapView`, 806.98 after `GraphView`, 815.11 at the end (gzip 235.71), from 840.79; chunks `RoadmapView` 12.68 kB, `GraphView` 24.38; `ru` 149.07; neither preloaded.

**Iteration 2**, from the looks and the review: L1 below; L2 Relies on lines as Subtasks rows; L3 the middle dot its own `aria-hidden` element; R1 centring only on `:focus-visible`, the document press tracking gone (so no stuck right-click); a failed reason read no live region.

**Departures**, accepted: (1) `FlowDivider` adds `y`, `width`, `height`; (2) an arc uses a box's bottom only when it is its column's lowest; (3) a band edge takes a trunk over the band; (4, 5, 8) new files `mocks/progress.smoke.test.tsx`, `app/lazyJump.test.tsx`, `board/CardLink.tsx`; (6) a Graph jump with no REF ends on its REF field; (7) `Landed`; (9) press tracking, dropped by R1; (10) `progressOf` takes the read's flows; (11) C-1 in jsdom, `view` by `defineProperty`; (12) no type generation; (13) flow200 under 2 500 forward crossings per column gap, `DIVIDER_HEIGHT` 40, the Russian the developer's, a bar contrast test; (14) L1: the canvas is 567 px at 1440 with both panes, so 3.5 columns cannot fit at 0.92: `COLUMN_GAP` 64, pitch 224 (206 on screen, from 272), about 2.5 columns in view; edge words in 56 px.

**Looks**, 2026-11-11, a debug daemon on a scratch copy of a real game corpus, both themes and both languages: the document modal 1200/360/750 at 1440 light, 1068/360/618 at 1100 dark, no overflow; the card modal 1200/360/750; Roadmap and Graph a busy skeleton, then the `h1`; Flow left to right at 0.92 from the left margin, 3 of 7 columns begun, dividers left with words on top, a 3 px press selects (synthetic pointer events, no physical mouse), the minimap takes no pointer, names "From ... to ..."; `C-0001`'s bar, words, Relies on, a reason with by and at; 320 dark, nothing overflows. Touch focus (R1) by tests alone.

**Known limits**: `card-progress.md`, `spec-flow-layout.md`, `ui-languages.md`.
