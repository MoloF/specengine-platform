---
class: spec
status: draft
scope: [ui]
ref: the owner's four requests of 2026-11-11 after looking at a hub on v0.36.0 (a modal whose side scrolls apart from its content; more room on Flow; a selection's links in a colour of their own; start and end boxes that cut their text); the analyst's requirement analysis of 2026-11-11; the orchestrator's working answers, the owner's review open
adrs: []
---

# room-and-scroll: room on Flow, a selection's links in the accent, terminators that keep their text, and a modal whose columns scroll apart

## Why

The owner's words of 2026-11-11, translated:
1. "In Jira the modal's content scrolls separately from the sidebar: the right block scrolls on its own, not together with the content."
2. "In the flow chart add more spacing, otherwise it's too cramped. Colour the links differently when we click an active place, now everything is one colour. And the strong rounding of the start/end blocks cuts the text; add more padding or lower the rounding a little. Do what is prettier and more correct."

What the analysis found in the code:
- **The emphasis never reaches the screen.** `FlowCanvas.tsx` `edgeOf` (l.347-366) already knows a selected box's links and the selected arrow, and sets `is-emphasis`. But it gives every edge an inline `stroke` from `typeColour` (l.359), and a repeat an inline `strokeWidth: var(--flow-repeat-width)`. Inline wins over `app.css` l.2563-2566 (`.graph-canvas .react-flow__edge.is-emphasis .react-flow__edge-path`). The marker takes the type's colour too (l.360). Only the label's border turns accent (l.5843). **A CSS-only fix fails silently.**
- **Cramped.** `layout.ts` `COLUMN_GAP` 64 (l.23) gives a 224 px pitch. An edge's words get at most `LABEL_WIDTH` 56 px (l.61, `--flow-edge-label-width` 3.5rem), about 7 characters, so `alternative` breaks. `BOX_GAP` is 32 (l.25). A box is padded 4 and 8 px (`app.css` l.5562).
- **Terminators overrun.** `.flow-box.is-start, .flow-box.is-end` (l.5587-5591) take `--radius-full`, a pill whose ends curve over 46 px of a 92 px box. A box's three rows take about 82 px: two title lines 40, the marks 20, the state 16-18, two 2 px gaps. The inner height is 82 for a plain box (92 - 2 - 8), 80 for a start (2 px border) and 76 for an end (4 px double). So a terminator's rows overrun and run into the curve.
- **One scroller.** In a dialog, `.dialog > .issue-view > .issue-view-body` (l.7440-7445) is the one scroll container, so Main and the side move together.

UI only, no ADR, no plan item: the owner's words are the source, as for `handoff-progress`. Rules come from ADR-0011 (the UI stack), ADR-0072 (two languages), `ui-design.md` "The rule" (every size and colour a token), and `ui/README.md` "Screen rules" (never colour alone).

## Description and interactions

### Decisions

- **D1. Room** (request 2; Q3 a working answer). `COLUMN_GAP` 96: a 256 px pitch, 235.5 on screen at the 0.92 floor. `LABEL_WIDTH` 88 and `--flow-edge-label-width` 5.5rem: the gap less 4 px each side, about 12 characters a line. `BOX_GAP` 48. `BOX_HEIGHT` 104. `.flow-box` is padded `--space-100` and `--space-150` (8 and 12 px). Unchanged: `BOX_WIDTH` 160, `FLOW_FIT_MIN_ZOOM` 0.92, the band's and arcs' steps (`TRACK_STEP`, `ARC_PAD`, `BAND_GAP`, `LANE_PAD`), `DIVIDER_HEIGHT`.
- **D2. Terminators** (request 2, "lower the rounding"). Start and end boxes round at `--radius-xl`, a new 16 px step, against a plain box's 8. They drop their own `padding-inline`, so the box's padding stands. Their accent borders (a start 2 px solid, an end 4 px double) and their Start and End marks stay: the shape, the border and the words still say it without colour. Inner heights at 104: plain 86, start 84, end 80. An end box's full three rows overrun by at most 2 px, centred into its 8 px padding, so nothing meets the border or a 16 px corner.
- **D3. The emphasis, inline in `edgeOf`** (request 2, "colour the links"; Q1 a working answer). While a box is selected, every drawn edge at it is emphasised: in, out, off-walk and its own loop. While an arrow is selected, it alone is. An emphasised edge's inline style is `stroke: var(--graph-edge-emphasis)` and `strokeWidth: var(--graph-edge-width-emphasis)`, over a repeat's width too. Its dash (`dashOf`) is kept. Its marker's `color` is `var(--graph-edge-emphasis)`, and its label keeps `is-emphasis`. In and out take the same accent. The selection's source does not matter: a click, the List, a gap item, the hash.
- **D4. The fade.** While at least one drawn edge is emphasised, every other drawn edge takes `is-faded`: its group's opacity is `--flow-edge-faded-opacity` 0.4, path and marker alike. Its label takes `is-faded` too: `color: var(--text-subtlest)`, never an opacity, so its words stay at 4.5:1 or more. With nothing emphasised, nothing fades: no selection, Esc, or a selected box with no drawn edge. Boxes, the band's caption and dividers never fade or dim. Emphasised edges come last in the edges array, so at the same `zIndex` 1 they paint over faded ones on a shared trunk, and stay under the boxes' 2.
- **D5. Flow only.** Every rule of the selection's look (D3, D4) is under `.flow-canvas` or on `.flow-edge-label`. Flow's wrapper also carries `.graph-canvas` (`FlowCanvas.tsx` l.626), so a `.graph-canvas` rule would reach Graph. Graph and Roadmap are unchanged.
- **D6. The legend says it.** After the arc's row, `FlowLegend` lists a sample line at `var(--graph-edge-emphasis)` and `var(--graph-edge-width-emphasis)`, inline, with `flow.legend.selected`. A status is never colour alone: the panel and the box's `Selected` tab still say what is selected.
- **D7. The modal's columns scroll apart** (request 1; Q2 a working answer). CSS only, in a dialog, inside `@container issue-view (min-width: 44.5rem)` (two columns):
  - the body clips (`overflow: hidden`), its one row `minmax(0, 1fr)`, `align-items: stretch`, `padding: 0`;
  - Main and the side each scroll (`overflow-y: auto`, `min-height: 0`, `overscroll-behavior: contain`), each padded `--space-050`, moved from the body so focus rings are not clipped;
  - the head (breadcrumbs, tools, title, acts) stays. A short card stays short: the row takes its taller column's height up to the dialog's.

  At one column above 36rem the body stays the one scroller, as today. Under 36rem the dialog scrolls as one, the head with it (DM-21), as today. Pages (`#/<p>/board/<C>`, a document's page) keep one window scroll: nothing outside `.dialog >` changes. The card's and the document's modals share it.

### Flow

The canvas is roomier (D1). Its terminators keep their text (D2). Selecting a box or an arrow lights its links in the accent and fades the rest (D3, D4), and the legend says so (D6). Accessible names, the List, the panel, keys, clicks and the fit are unchanged.

## Data

### Constants (`src/flow/`)

| Name | File | Was | Becomes |
|---|---|---|---|
| `COLUMN_GAP` | `layout.ts` l.23 | 64 | 96 |
| `BOX_GAP` | `layout.ts` l.25 | 32 | 48 |
| `LABEL_WIDTH` | `layout.ts` l.61 | 56 | 88 |
| `BOX_HEIGHT` | `geometry.ts` l.46 | 92 | 104 |

### Tokens and styles

```css
/* tokens.css */
--radius-xl: 16px;                  /* after --radius-l (l.306); the scale's seventh radius */
--flow-edge-faded-opacity: 0.4;     /* in the Flow block, beside --flow-repeat-width (l.222) */
--flow-edge-label-width: 5.5rem;    /* was 3.5rem (l.257); its comment says 96 px */

/* app.css */
.flow-box { padding: var(--space-100) var(--space-150); /* rest as today (l.5555) */ }
.flow-box.is-start,
.flow-box.is-end { border-radius: var(--radius-xl); }   /* no padding-inline; l.5587 */
.flow-canvas .react-flow__edge.is-faded { opacity: var(--flow-edge-faded-opacity); }
.flow-edge-label.is-faded { color: var(--text-subtlest); }

@container issue-view (min-width: 44.5rem) {
  .dialog > .issue-view > .issue-view-body {
    overflow: hidden; grid-template-rows: minmax(0, 1fr); align-items: stretch; padding: 0;
  }
  .dialog > .issue-view > .issue-view-body > .issue-view-main,
  .dialog > .issue-view > .issue-view-body > .issue-view-side {
    min-height: 0; overflow-y: auto; overscroll-behavior: contain; padding: var(--space-050);
  }
}
```

The base `.dialog > .issue-view > .issue-view-body` (l.7440) and the `max-width: 36rem` block (l.7447) are unchanged. No scale exception is added (`scale-exceptions.ts`).

### `edgeOf` (`FlowCanvas.tsx`)

```ts
type EdgeLook = "plain" | "emphasis" | "faded";
type LinkData = { laid: LaidEdge; look: EdgeLook };   // was { laid; emphasis: boolean } (l.65)
function edgeOf(laid: LaidEdge, ends: {...}, types: readonly string[], look: EdgeLook, t: Translate): LinkEdge;
```

- **emphasis**: style `{stroke: "var(--graph-edge-emphasis)", strokeDasharray: dashOf(...), strokeWidth: "var(--graph-edge-width-emphasis)"}`, `markerEnd.color` the same accent, `className` `is-emphasis`.
- **faded**: today's style and marker, `className` `is-faded`.
- **plain**: as today.

The edges memo (l.473-478) judges `look` over the drawn edges: emphasised by today's predicate; faded when another drawn edge is emphasised. It orders emphasised edges last, keeping the rest in layout order. `LinkView`'s label class follows `look`.

### Catalog

`flow.legend.selected`, English: "a selected box's lines in and out, or a selected line; the others fade". Russian lives in `ru.ts` alone and is the ui-developer's, the owner's review open (`ui-languages.md` "Known limits").

### Comments that state old numbers

`layout.ts` l.18-21 (224 and 206 px), `tokens.css` l.256 (64 px), `app.css` l.5586 ("a stadium") and l.7424-7426 ("the body scrolls inside") say the new.

## Rules and edge cases

- WHEN a box is selected, every drawn edge with it as an end SHALL be emphasised. WHEN an arrow is selected, it alone SHALL be. An emphasised edge SHALL keep its dash.
- WHEN at least one drawn edge is emphasised, every other drawn edge SHALL be faded, and its words SHALL take `--text-subtlest` with no opacity. WHEN none is emphasised, none SHALL be faded.
- A box, the band's caption and a divider SHALL never take `is-faded`, an opacity or `.is-dimmed` (ST-6, narrowed by name).
- Graph and Roadmap SHALL draw as before: no rule of this slice reaches `.graph-canvas` without `.flow-canvas`.
- WHEN a work item stands in a dialog at two columns, Main and the side SHALL scroll on their own and the head SHALL stay. At one column the body SHALL scroll. Under 36rem the dialog SHALL scroll as one. A page SHALL scroll the window alone.
- Every size, radius and colour SHALL be one `var()` of a token (`scale.test.ts`, `ui-design.md` "The rule").

### Test churn

Only these change. Every other test passes unchanged.

| Test | Becomes | Why |
|---|---|---|
| `flow/layout.test.ts` l.214-220, "steps a column 224 px" | 256 px; `LABEL_WIDTH + 8 === COLUMN_GAP` holds | D1 |
| `flow/layout.test.ts` l.222-231, "two and a half pitches" | three columns begun, more than two pitches | D1 |
| `flow/styles.test.ts` l.116-120, ST-6 "dims nothing" | narrowed by name (RS-6): `--flow-dimmed` and `.is-dimmed` still absent, `is-faded` only on edges and labels, no opacity on a box, divider or band | D4, D5 |
| `styles/scale.test.ts` l.101 `RADII`, l.294 the family's 6 | `16px` added, 7 | D2 |
| `ui/IssueView.test.tsx` l.100-104 | its title names one column; RS-8's case beside it | D7 |
| `board/styles.test.ts` l.120 | passes; its comment names one column | D7 |
| any test pinning a laid coordinate in px, or reading edges by DOM position with a selection | from the constants; by name | D1, D4 |

## Acceptance criteria

A browser look is marked **[look]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red.

- [ ] RS-1 — **Room.** `flow/layout.test.ts`, `flow/styles.test.ts`:
  - `COLUMN_GAP` 96, `BOX_GAP` 48, `LABEL_WIDTH` 88, `BOX_HEIGHT` 104. `BOX_WIDTH` 160 and `FLOW_FIT_MIN_ZOOM` 0.92 are unchanged.
  - A line A, B, C steps 256 px a column, and `LABEL_WIDTH + 8` equals `COLUMN_GAP`.
  - Two boxes of one column stand at least `BOX_HEIGHT + BOX_GAP` (152) apart.
  - `tokens.css` holds `--flow-edge-label-width: 5.5rem;`, pinned as `LABEL_WIDTH / 16`.

  Mutations, each red: `COLUMN_GAP` back to 64; `--flow-edge-label-width` left at 3.5rem.
- [ ] RS-2 — **Words in their gap, columns in view.**
  - `labelSize` of an edge whose only word is `alternative` is one line: height 22, width at most 88.
  - The firing run at `CANVAS_1440` (567 px) fits at 0.92. Three columns' first boxes begin in view, and more than two pitches (235.52 px each) lie between the first box's left and the canvas's right.

  Mutation, red: `LABEL_WIDTH` left at 56 (two lines).
- [ ] RS-3 — **Boxes and terminators.** `flow/styles.test.ts`, `styles/scale.test.ts`:
  - `.flow-box` holds `padding: var(--space-100) var(--space-150);`.
  - The rule of `.flow-box.is-start, .flow-box.is-end` holds `border-radius: var(--radius-xl);` and no `padding`. `.flow-box.is-start` keeps `border: 2px solid var(--flow-start);` and `.flow-box.is-end` keeps `border: 4px double var(--flow-end);`.
  - `tokens.css` holds `--radius-xl: 16px;`. `RADII` lists `16px`, and the radius family counts 7.

  Mutations, each red: the terminators back at `var(--radius-full)`; `--radius-xl` at 20px (outside `RADII`).
- [ ] RS-4 — **The emphasis is inline, in `edgeOf`.** `flow/FlowView.test.tsx` on `studioClient(reads())`. G's children: A→B, B→C (alternative), C→E, E→B (a repeat), and D→A, which is off the walk.
  - **B selected by a click.** The paths of A→B, B→C and E→B carry inline `stroke: var(--graph-edge-emphasis)` and `stroke-width: var(--graph-edge-width-emphasis)`. E→B keeps `stroke-dasharray` `1 7`, and B→C keeps its kind's dash. The marker that each path's `marker-end` names holds a `polyline` whose inline `stroke` and `fill` are `var(--graph-edge-emphasis)`.
  - C→E keeps its type's colour (`typeColour`) on its path and its marker, and no inline width.
  - **A selected.** D→A is drawn and emphasised, with A→B.
  - **C→E selected from the List.** It alone is emphasised.
  - In DOM order, the emphasised edges' groups follow every other edge's.

  Mutations, each red:
  - `edgeOf` leaves the type's colour inline and relies on the `.is-emphasis` rule (the CSS-only fix);
  - the marker keeps the type's colour;
  - a repeat keeps `var(--flow-repeat-width)` when emphasised.
- [ ] RS-5 — **The fade.** The same stub, and `flow/styles.test.ts`:
  - **A selected.** The `.react-flow__edge` groups of B→C, C→E and E→B carry `is-faded`; those of A→B and D→A do not. The labels of B→C and E→B carry `is-faded`, not `is-emphasis`.
  - **B selected.** B→C's label carries `is-emphasis`.
  - Every box node's class and inline style are what they are with nothing selected, but the selected box's own `is-selected` and tab. That holds for the band's caption and the dividers too.
  - **After Esc**, no element carries `is-faded`, and no path carries the emphasis.
  - A stub picture whose walk holds one box with no edge, selected: nothing carries `is-faded`.
  - **CSS.** `.flow-canvas .react-flow__edge.is-faded` holds `opacity: var(--flow-edge-faded-opacity);`, and `tokens.css` holds `--flow-edge-faded-opacity: 0.4;`. `.flow-edge-label.is-faded` holds `color: var(--text-subtlest);` and no `opacity`.

  Mutations, each red: an edge faded whenever it is not emphasised (everything fades with nothing selected); the label faded by `opacity`.
- [ ] RS-6 — **Flow only; boxes never dim.** `flow/styles.test.ts` (ST-6, narrowed); `graph/GraphView.test.tsx` unchanged:
  - Every rule whose selector holds `is-faded` holds `.flow-canvas` or `.flow-edge-label`.
  - None holds `.flow-box`, `.flow-divider`, `.flow-band` or `.react-flow__node`.
  - No rule whose selector holds `.flow-box`, `.flow-divider`, `.flow-band`, or `.react-flow__node` under `.flow-canvas`, sets `opacity`.
  - `--flow-dimmed` and `.is-dimmed` stay absent from both style files.
  - Graph's emphasis test (l.475-477) passes as it is, and no Graph edge carries `is-faded`.

  Mutations, each red: the fade written as `.graph-canvas .react-flow__edge.is-faded`; `.flow-box.is-faded { opacity: ... }`.
- [ ] RS-7 — **The legend says it.** The legend's test, `i18n/catalog.test.ts`:
  - The open legend lists a row after the arc's. Its sample `line` has inline `stroke: var(--graph-edge-emphasis)` and `stroke-width: var(--graph-edge-width-emphasis)`, and its words are `en.flow.legend.selected`.
  - `ru.flow.legend.selected` exists and differs from the English.

  Mutations, each red: the key missing from `ru.ts`; the sample at `var(--graph-edge)`.
- [ ] RS-8 — **The modal's columns scroll apart.** `ui/IssueView.test.tsx`, with a helper that reads `@container` blocks as `mediaBlocks` reads `@media`:
  - **Inside `@container issue-view (min-width: 44.5rem)`.**
    - `.dialog > .issue-view > .issue-view-body` holds `overflow: hidden;`, `grid-template-rows: minmax(0, 1fr);`, `align-items: stretch;` and `padding: 0;`.
    - `... > .issue-view-main` and `... > .issue-view-side` each hold `min-height: 0;`, `overflow-y: auto;`, `overscroll-behavior: contain;` and `padding: var(--space-050);`.
  - **Outside it**, the body keeps `overflow: auto;`, `min-height: 0;` and `padding: var(--space-050);`. Under 36rem, today's rules stand.
  - No rule without `.dialog >` gives `.issue-view-main`, `.issue-view-side` or `.issue-view-head` an `overflow`.

  Mutations, each red:
  - Main's `overflow-y: auto` dropped;
  - the split written outside the container query;
  - the split written as `.issue-view-body > .issue-view-main`, without `.dialog >`.
- [ ] RS-9 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, plus 1440x600 for the side's own scroll. On harbor-sim, and on a scratch copy of a real game corpus whose flow has a start, an end, a repeat and an off-walk link, made for the look. Build both binaries first: `spec serve` falls back to the installed `specengine-http` when the debug build has none.
  - **Room.** A pitch of about 236 px on screen. `alternative` on one line. At 1440 with both panes, three columns begun.
  - **Terminators.** Corners at 16 px, and every row inside the border. A long Russian title in two lines is cut by no corner.
  - **A selected box.** Its lines in and out turn accent and heavier, dashes kept, arrowheads accent. The rest fade, their words subtlest and readable. Boxes and dividers are unchanged.
  - **A selected arrow, from the List.** It alone turns accent. Esc restores every line.
  - **The legend's new row**, in both languages.
  - **The modal.** On `C-0001` and on MEC-HARBOUR-LOG's document:
    - at 1440x900, the wheel over Main scrolls Main alone; the side and the head stay;
    - at 1440x600, the wheel over the side scrolls it alone;
    - a column's end scrolls nothing under the scrim;
    - Tab to the last control of each column shows its whole focus ring;
    - at 700 wide (one column), the body scrolls; at 320, the dialog scrolls as one;
    - the card's and the document's pages scroll the window alone.
  - **Graph and Roadmap.** A selected Graph node looks as before, and nothing fades.

  Red if: a selected box's lines keep their type's colours (the CSS-only fix); a terminator's text meets its border or corner; the side moves with Main.
- [ ] G-1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full UI test run green twice. First-load JS after `pnpm build` is recorded, at most 845 kB.
  - **Rust.** There is no Rust code. Run `cargo nextest run -p specengine-eval --test doc_pointers --test ui_policy --test anonymity`.
  - **Docs.** The check is clean.

## Out of scope

- Graph's and Roadmap's selection, fade and edge colours. Flow's List, panel, names, keys, clicks and fit.
- Different colours for in and out (Q1). A box at the other end of an emphasised link highlighted. Any type's colour or dash changed.
- The band's and arcs' gaps, the 0.92 floor, `BOX_WIDTH`, a collapsible pane.
- A page split into scrolling columns (Q2). The head scrolling away above 36rem.
- A selected end box losing its double border to the selection's 2 px (`app.css` l.5619, today's behaviour; its End mark and the Selected tab still say it).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| ui-developer | `src/flow/{layout.ts,geometry.ts,FlowCanvas.tsx,FlowLegend.tsx}`; `src/styles/{tokens.css,app.css}`; `src/i18n/{en,ru}.ts`; the tests of RS-1 to RS-8 with "Test churn"'s rows: `src/flow/{layout,styles,FlowView}.test.*`, `src/styles/scale.test.ts`, `src/ui/IssueView.test.tsx`, `src/board/styles.test.ts` (a comment) |

The test-engineer runs the gates and mutations and writes nothing. Comments cite `docs/features/room-and-scroll.md` "Acceptance criteria" with a criterion ID, or an existing canon heading, never "Data" or "Description and interactions", which compaction drops at ship (`doc_pointers`).

### Work order

The UI lane alone.
1. **The modal** (D7, RS-8): CSS only, standing alone.
2. **Room and terminators** (D1, D2; RS-1 to RS-3).
3. **The emphasis and the fade** (D3 to D6; RS-4 to RS-7). The inline fix in `edgeOf` goes first, then the fade and the legend.
4. The test-engineer runs the gates and the mutations; then the UI review.
5. The orchestrator's look (RS-9), on the mock and the corpus copy.
6. Ship: the spec-writer ("Canon at ship").

At most three iterations. A part left unfinished is reported, never shipped half (`CLAUDE.md` "Process").

### Risks

- **The silent CSS fix.** A rule beside `.graph-canvas .react-flow__edge.is-emphasis` would pass a class test and change nothing on screen. RS-4 reads the inline style and the marker.
- **ST-6.** The guard bans dimming because the retired Stage filter dimmed boxes (`writer-studio` ST-6). It is narrowed by name, never dropped: boxes, dividers and the band still never dim.
- **Hue.** The accent sits near `--flow-edge-0`, in dark mode most (`#a5b4fc` against `#7fb8ff`). Width (2.75 against 1.5) and the fade carry the difference; the look judges it.
- **Faded lines fall under 3:1** against the canvas by design. The emphasised ones carry the meaning, and faded words stay at `--text-subtlest` (4.5:1 or more).
- **Dash only.** On an emphasised line the type's colour gives way to the accent, so its type shows by dash alone. The List, the panel and the arrow's name carry it; the canon notes the exception.
- **Marker size.** `markerUnits` is `strokeWidth`, so an emphasised head grows with its stroke, as Graph's and a repeat's already do. The look judges it.
- **Fewer columns in view.** At 1440 with both panes, about 2.2 pitches show at 0.92 against 2.5, the third box about a third in view. That was the owner's request (Q3); the collapsible pane stays his call.
- **An end box** is tight at 104: 80 px inside, for rows of about 82 (D2).
- **Keyboard scroll.** Safari scrolls a column without a focused control in it only by pointer. That is a known limit at ship.
- **A short wide viewport.** At 1440x600 the head takes its height before the columns. They scroll, and the look checks it.
- **Caps.** `ui-design.md` has 30 bytes free ("Canon at ship").
- **Anonymity.** The look's real corpus is never named in the repository.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/spec-flow-layout.md` [5 040] | "Layout" Columns: gap 96, pitch 256 (235.5 on screen), words 88 px, about 12 characters, `BOX_GAP` 48, a box 104 tall padded 8 and 12. "Forms": Type, the emphasis exception; new **Boxes** (terminators at `--radius-xl`, their borders and marks); new **Selection** (D3, D4, D6). "Known limits": Columns in view about 2.2 pitches; faded lines under 3:1 by design |
| `docs/canon/ui-design.md` [30] | "Space": `--radius-xl` 16 px. "Width": in a dialog the head stays; two columns scroll apart, one column scrolls its body; under 36rem the dialog scrolls; a page scrolls the window. "Known limits": Safari's keyboard scroll. Net not positive: drop "The header"'s "(at 1440 px in Russian ... 90 px)" and "Contrast"'s "Lowest at ship" sentence (the tests compute both) |
| `docs/canon/board-screen.md` [153] | "Modal": "its body scrolling" becomes "its columns scrolling apart" |
| `CLAUDE.md`; 08 s2 Phase 7, step 6b | shipped; next the pipeline trial |
| this spec | compacted to at most 3 KB |

No ADR. `spec-flow-screen.md` [83] is not touched.

### Working answers and the owner's questions

**The orchestrator's, taken:**
- Q1: one accent for in and out.
- Q2: pages keep one window scroll; only a dialog's columns split.
- Q3: a column gap of 96.

**The spec-writer's:**
- W1: emphasised edges paint last at `zIndex` 1 (D4).
- W2: the modal's split stands only at two columns, so one column keeps today's single scroller (D7).
- W3: the body's padding moves to the columns at two columns, for focus rings (D7).
- W4: a marker keeps its 14 units and grows with its stroke (Risks).
- W5: the legend's row says the selection in words, as a status must (D6).

**The owner's question:**
- **Q-A.** At 1440 with both panes, about 2.2 columns show. Is that enough, or should the pane collapse? Working answer: as is; the pane is his call (`spec-flow-layout.md` "Known limits").
