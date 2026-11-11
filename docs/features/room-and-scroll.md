---
class: spec
status: shipped
shipped: 2026-11-11
scope: [ui]
ref: the owner's four requests of 2026-11-11 after looking at a hub on v0.36.0 (a modal whose side scrolls apart from its content; more room on Flow; a selection's links in a colour of their own; start and end boxes that cut their text); the analyst's requirement analysis of 2026-11-11; the orchestrator's working answers, the owner's review open
adrs: []
---

# room-and-scroll: room on Flow, a selection's links in the accent, terminators that keep their text, and a modal whose columns scroll apart

## Why

The owner's words of 2026-11-11, translated:
1. "In Jira the modal's content scrolls separately from the sidebar: the right block scrolls on its own, not together with the content."
2. "In the flow chart add more spacing, otherwise it's too cramped. Colour the links differently when we click an active place, now everything is one colour. And the strong rounding of the start/end blocks cuts the text; add more padding or lower the rounding a little. Do what is prettier and more correct."

What the analysis found: `edgeOf` already marked a selection's links `is-emphasis`, but set every edge's `stroke`, a repeat's width and the marker's colour inline from its type, and inline wins over any stylesheet rule, so the emphasis never reached the screen and a CSS-only fix would fail silently. A 224 px pitch left an edge 56 px of words, about 7 characters. Start and end boxes were pills whose ends curved over 46 px of a 92 px box, and their three rows (about 82 px) overran an 80 or 76 px inside. In a dialog the work item's body was the one scroller, so Main and the side moved together.

UI only, no ADR, no plan item: the owner's words are the source. Rules: ADR-0011, ADR-0072, `ui-design.md` "The rule", `ui/README.md` "Screen rules" (never colour alone).

## Description and interactions

Each decision is canon now.
- **D1** Room: `spec-flow-layout.md` "Layout" (Columns). **D2** Terminators: "Forms" (Boxes).
- **D3** the emphasis inline in `edgeOf`, **D4** the fade, **D5** Flow only, **D6** the legend's row: "Forms" (Selection, and Type's exception: a lit line shows its type by dash alone).
- **D7** The modal's columns scroll apart: `ui-design.md` "The work item" (Width); `board-screen.md` "The card modal and page".

Working answers taken: one accent for in and out (Q1); pages keep one window scroll, only a dialog's columns split (Q2); a 96 px column gap (Q3); lit edges paint last at `zIndex` 1; the split only at two columns, one column keeping the body's scroller; the body's padding moved to the columns for focus rings; a marker keeps its 14 units and grows with its stroke; the legend's row says the selection in words. The owner's Q-A is open: at 1440 with both panes about 2.2 pitches show; the pane stays his call (`spec-flow-layout.md` "Known limits").

## Data

Constants `layout.ts` `COLUMN_GAP` 96, `BOX_GAP` 48, `LABEL_WIDTH` 88, `geometry.ts` `BOX_HEIGHT` 104; tokens `--radius-xl` 16px, `--flow-edge-faded-opacity` 0.4, `--flow-edge-label-width` 5.5rem; `FlowCanvas.tsx` `EdgeLook` `"plain" | "emphasis" | "faded"`, `edgeOf(laid, ends, types, look, t)`; catalog `flow.legend.selected`. No served change, no scale exception.

## Rules and edge cases

In the canon: `spec-flow-layout.md` "Forms" (Selection), "Known limits"; `ui-design.md` "The work item".

## Acceptance criteria

A browser look is marked **[look]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red.

- [x] RS-1 — **Room.** `flow/layout.test.ts`, `flow/styles.test.ts`:
  - `COLUMN_GAP` 96, `BOX_GAP` 48, `LABEL_WIDTH` 88, `BOX_HEIGHT` 104. `BOX_WIDTH` 160 and `FLOW_FIT_MIN_ZOOM` 0.92 are unchanged.
  - A line A, B, C steps 256 px a column, and `LABEL_WIDTH + 8` equals `COLUMN_GAP`.
  - Two boxes of one column stand at least `BOX_HEIGHT + BOX_GAP` (152) apart.
  - `tokens.css` holds `--flow-edge-label-width: 5.5rem;`, pinned as `LABEL_WIDTH / 16`.

  Mutations, each red: `COLUMN_GAP` back to 64; `--flow-edge-label-width` left at 3.5rem.
- [x] RS-2 — **Words in their gap, columns in view.**
  - `labelSize` of an edge whose only word is `alternative` is one line: height 22, width at most 88.
  - The firing run at `CANVAS_1440` (567 px) fits at 0.92. Three columns' first boxes begin in view, and more than two pitches (235.52 px each) lie between the first box's left and the canvas's right.

  Mutation, red: `LABEL_WIDTH` left at 56 (two lines).
- [x] RS-3 — **Boxes and terminators.** `flow/styles.test.ts`, `styles/scale.test.ts`:
  - `.flow-box` holds `padding: var(--space-100) var(--space-150);`.
  - The rule of `.flow-box.is-start, .flow-box.is-end` holds `border-radius: var(--radius-xl);` and no `padding`. `.flow-box.is-start` keeps `border: 2px solid var(--flow-start);` and `.flow-box.is-end` keeps `border: 4px double var(--flow-end);`.
  - `tokens.css` holds `--radius-xl: 16px;`. `RADII` lists `16px`, and the radius family counts 7.

  Mutations, each red: the terminators back at `var(--radius-full)`; `--radius-xl` at 20px (outside `RADII`).
- [x] RS-4 — **The emphasis is inline, in `edgeOf`.** `flow/FlowView.test.tsx` on `studioClient(reads())`. G's children: A→B, B→C (alternative), C→E, E→B (a repeat), and D→A, which is off the walk.
  - **B selected by a click.** The paths of A→B, B→C and E→B carry inline `stroke: var(--graph-edge-emphasis)` and `stroke-width: var(--graph-edge-width-emphasis)`. E→B keeps `stroke-dasharray` `1 7`, and B→C keeps its kind's dash. The marker that each path's `marker-end` names holds a `polyline` whose inline `stroke` and `fill` are `var(--graph-edge-emphasis)`.
  - C→E keeps its type's colour (`typeColour`) on its path and its marker, and no inline width.
  - **A selected.** D→A is drawn and emphasised, with A→B.
  - **C→E selected from the List.** It alone is emphasised.
  - In DOM order, the emphasised edges' groups follow every other edge's.

  Mutations, each red:
  - `edgeOf` leaves the type's colour inline and relies on the `.is-emphasis` rule (the CSS-only fix);
  - the marker keeps the type's colour;
  - a repeat keeps `var(--flow-repeat-width)` when emphasised.
- [x] RS-5 — **The fade.** The same stub, and `flow/styles.test.ts`:
  - **A selected.** The `.react-flow__edge` groups of B→C, C→E and E→B carry `is-faded`; those of A→B and D→A do not. The labels of B→C and E→B carry `is-faded`, not `is-emphasis`.
  - **B selected.** B→C's label carries `is-emphasis`.
  - Every box node's class and inline style are what they are with nothing selected, but the selected box's own `is-selected` and tab. That holds for the band's caption and the dividers too.
  - **After Esc**, no element carries `is-faded`, and no path carries the emphasis.
  - A stub picture whose walk holds one box with no edge, selected: nothing carries `is-faded`.
  - **CSS.** `.flow-canvas .react-flow__edge.is-faded` holds `opacity: var(--flow-edge-faded-opacity);`, and `tokens.css` holds `--flow-edge-faded-opacity: 0.4;`. `.flow-edge-label.is-faded` holds `color: var(--text-subtlest);` and no `opacity`.

  Mutations, each red: an edge faded whenever it is not emphasised (everything fades with nothing selected); the label faded by `opacity`.
- [x] RS-6 — **Flow only; boxes never dim.** `flow/styles.test.ts` (ST-6, narrowed); `graph/GraphView.test.tsx` unchanged:
  - Every rule whose selector holds `is-faded` holds `.flow-canvas` or `.flow-edge-label`.
  - None holds `.flow-box`, `.flow-divider`, `.flow-band` or `.react-flow__node`.
  - No rule whose selector holds `.flow-box`, `.flow-divider`, `.flow-band`, or `.react-flow__node` under `.flow-canvas`, sets `opacity`.
  - `--flow-dimmed` and `.is-dimmed` stay absent from both style files.
  - Graph's emphasis test (l.475-477) passes as it is, and no Graph edge carries `is-faded`.

  Mutations, each red: the fade written as `.graph-canvas .react-flow__edge.is-faded`; `.flow-box.is-faded { opacity: ... }`.
- [x] RS-7 — **The legend says it.** The legend's test, `i18n/catalog.test.ts`:
  - The open legend lists a row after the arc's. Its sample `line` has inline `stroke: var(--graph-edge-emphasis)` and `stroke-width: var(--graph-edge-width-emphasis)`, and its words are `en.flow.legend.selected`.
  - `ru.flow.legend.selected` exists and differs from the English.

  Mutations, each red: the key missing from `ru.ts`; the sample at `var(--graph-edge)`.
- [x] RS-8 — **The modal's columns scroll apart.** `ui/IssueView.test.tsx`, with a helper that reads `@container` blocks as `mediaBlocks` reads `@media`:
  - **Inside `@container issue-view (min-width: 44.5rem)`.**
    - `.dialog > .issue-view > .issue-view-body` holds `overflow: hidden;`, `grid-template-rows: minmax(0, 1fr);`, `align-items: stretch;` and `padding: 0;`.
    - `... > .issue-view-main` and `... > .issue-view-side` each hold `min-height: 0;`, `overflow-y: auto;`, `overscroll-behavior: contain;` and `padding: var(--space-050);`.
  - **Outside it**, the body keeps `overflow: auto;`, `min-height: 0;` and `padding: var(--space-050);`. Under 36rem, today's rules stand.
  - No rule without `.dialog >` gives `.issue-view-main`, `.issue-view-side` or `.issue-view-head` an `overflow`.

  Mutations, each red:
  - Main's `overflow-y: auto` dropped;
  - the split written outside the container query;
  - the split written as `.issue-view-body > .issue-view-main`, without `.dialog >`.
- [x] RS-9 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, plus 1440x600 for the side's own scroll. On harbor-sim, and on a scratch copy of a real game corpus whose flow has a start, an end, a repeat and an off-walk link, made for the look. Build both binaries first: `spec serve` falls back to the installed `specengine-http` when the debug build has none.
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
- [x] G-1 — **Gates.**
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

| Module | Does |
|---|---|
| `src/flow/layout.ts`, `geometry.ts` | `COLUMN_GAP` 96, `BOX_GAP` 48, `LABEL_WIDTH` 88; `BOX_HEIGHT` 104 |
| `src/flow/FlowCanvas.tsx` | `EdgeLook`; `edgeOf` sets a lit edge's stroke, width (`widthOf`) and marker inline; the rest fade while any is lit; lit edges last |
| `src/flow/FlowLegend.tsx`, `src/i18n/{en,ru}.ts` | `Sample`'s `width`; the row `flow.legend.selected` |
| `src/styles/{tokens,app}.css` | `--radius-xl`, `--flow-edge-faded-opacity`, the label width; the box's padding, the terminators, the fade; a dialog's two scrollers |
| the tests of RS-1 to RS-8 | ST-6 narrowed by name |

**Numbers.** UI 5 082 tests in 172 files, twice; eval 58; 18 named mutations red; first load 815.45 kB (gzip 235.80), `ru` 149.24 kB. No Rust.

**Departures**, accepted: (1) RS-6's Graph side is checked in `flow/styles.test.ts`; (2) its `blocksOf` reads any at-rule; (3) new `IssueView.test.tsx` helpers; (4) mutation RS-8a split a shared rule; (5) Graph's older `.graph-canvas .react-flow__edge.is-emphasis` rule still matches a lit Flow edge, same values, the inline style winning; (6) no generated types. **Review** ACCEPT; its test-only follow-ups done (`/faded/` with a positive control; the head takes no overflow).

**Looks** (RS-9), 2026-11-11, Chrome:
- A scratch copy of a real game corpus, dark, Russian, 1440x900: terminators at 16 px, rows inside the border (91 of 100, 79 of 96); a selected box's 3 lines accent at 2.75 px, drawn last, the other 12 faded at 0.4 in their type colours; the document modal's Main scrolled 600, the side, body and head still; two columns in view with both panes.
- The mock, light, English, C-0001's modal: at 1440x600 two tracks, dialog and body `overflow: hidden`, Main and side `auto`, Main scrolling (the side fits); at 700x900 one track, the body the one scroller; at 320x640 the dialog the one scroller, no horizontal overflow.
- The mock's `aside` scenario, KRUN-FIRING, 1440x900, light: a pitch of 235 on screen, 3 columns begun; "repeat", "alternative" and "cracked" each on one line (16 px).
- Graph: no `is-faded` anywhere.
- Not looked at: Tab focus rings at a column's end, an arrow selected from the List, the pages.

**Known limits**: `spec-flow-layout.md` and `ui-design.md` "Known limits".
