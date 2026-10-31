---
class: spec
status: draft
scope: [ui]
ref: RM-0069; Phase 7 step 7d (08 s2)
---

# screen-fit: a drop lands on an empty column, a clean flow takes the width (RM-0069)

## Why

Two findings of the browser checks on 2026-10-31. An empty Board column is 61 px tall, so a card dropped below its heading lands on nothing and nothing says so: "moves them" fails silently. And at 1440 px with gaps shown a real flow draws at about zoom 0.6, because a panel of ten `None` rows holds a whole column that says nothing.

Clear, re-mark and Clear all work as they are and are not touched. This slice can run beside any other.

## Description and interactions

**Board.** Columns stand over the free height, so a drop anywhere in a column lands. `section.board-column` already takes the drop (`BoardColumn.tsx:72`), and a drop below every card lands last (`beforeAt` returns `null`), so no code changes. A narrow screen stacks columns as today and takes no height.

**Flow.** When every gap row reads `None` and the read is fresh, the rows are replaced by one line in `.flow-tools`, and the side column goes unless a panel is open. Anything else draws the rows as today: one row found shows all ten. A failed re-read keeps `Not read` and never shows the line, since a person never reads "fine" while the state is unknown (`trace-views.md` "Implementation").

The rows are not moved under the canvas: they would fall below a picture sized to its fit.

## Data

**CSS** (`ui/src/styles/app.css`). `align-items: stretch` alone gives an empty board nothing: every column would match the tallest, and `.view` has no height of its own to fill (`.app-body`'s `min-height` stops above it). So the height is stated:

```css
.board-columns {
  align-items: stretch; /* was start, line 5321 */
  min-height: 60vh;
}

.board-column {
  align-content: start; /* else a grid's rows share the stretch, and the title grows */
}
```

The media block stacking the columns (`grid-auto-flow: row`, line 5661) sets `min-height: 0`: 60vh shared across stacked rows means nothing, and a phone must not scroll past empty boxes.

**Flow state.** `labels.ts`, the one place telling gap states apart, gains:

```ts
/** Every served row measured at 0: the rows have nothing to say. */
export function allClear(gaps: readonly FlowGap[]): boolean;
```

True when `gaps` is non-empty and every row is `state === "measured" && count === 0`. An unknown state or a `null` count is false. The number ten is not written in the UI.

`FlowView.tsx`, at `gapsShown` (line 557): a new `clear = !empty && !topFailed && allClear(top.gaps)`, and `gapsShown = !clear && (empty || !gapsHidden)`. The `no-gaps` and `no-side` classes already do the layout, panel included.

**The line,** in `.flow-tools` after `StageFilter`, in place of the Show or Hide gaps button: an icon and `No gaps: every row reads None`, `role="status"`, the button gone since nothing can be shown. A stored `specengine.flow.gaps-hidden` is left as it is, and takes effect again when a row is found.

**Types, routes, schema:** none.

## Rules and edge cases

- WHEN `top.nodes` is empty, the rows SHALL stand as today: `.flow-tools` is not drawn then, so the line has no home and the rows are the only guide.
- WHEN the top read fails, `clear` SHALL be false: the rows stand with `Not read`, whatever the last read held.
- WHEN a row turns up after a clean read, the rows SHALL come back with no reload, through the same read.
- WHEN a box is selected in the clear state, `no-gaps` alone SHALL hold, so the side column shows the panel, as Hide gaps does (`spec-flow-screen.md` "Gap rows").
- A group read's rows stay unshown, as today: `clear` reads Top's alone.

**Decision D12.** The Board stays CSS-only: `section` already takes the drop.

No ADR.

**Assumption A2.** The canvas zoom rises with the freed column. No number is promised; the gate's look decides.

## Acceptance criteria

- [ ] AC-01 — A test over `app.css?raw` pins `.board-columns` `align-items: stretch` with a `min-height`, `.board-column` `align-content: start`, and `min-height: 0` in the stacked block. M: `align-items: start` restored.
- [ ] AC-02 — By hand or by a headless look, not a repository test (`ui/` has no browser harness): at 1440x900 an empty column stands at least half the viewport high, and a real mouse drop at 80 % of its height moves the card. Recorded in the spec's "Implementation" with the date. M: the same check at `align-items: start` shows the drop lost.
- [ ] AC-03 — Ten clear rows: no region named "Gaps", the line present, `.flow-stage` has `no-gaps` and `no-side`, the Hide gaps button gone. M: the line shown beside rows.
- [ ] AC-04 — One row found: all ten as today, no line. A selected box in the clear state: `no-gaps` without `no-side`. M: `allClear` true on any row at 0; `no-side` kept under a panel.
- [ ] AC-05 — A failed re-read of a clean flow: every row `Not read`, never the line. M: the line shown beside a failed read.
- [ ] AC-06 — `allClear` over an empty list, an unknown state and a `null` count is false. `policy.test.ts` still finds no gap-state branch outside `labels.ts`. M: a state compared in `FlowView.tsx`.

## Out of scope

The Board's drag code. Moving the rows under the canvas. Zooming or new canvas chrome. A clean line on a group's read. A browser harness in `ui/`.

## Implementation

To fill in after implementation.

### Files in scope

`ui/src/styles/app.css`, `ui/src/flow/FlowGaps.tsx`, `ui/src/flow/FlowView.tsx`, `ui/src/flow/labels.ts`; tests beside them.

### Canon at ship

- **`board-ui.md`** (1.2 KB free) "Moving": a column stands over the free height.
- **`spec-flow-screen.md`** "Gap rows", 165 B free, shared with `flow-authoring`. This slice needs about 130 B: `All rows None on a fresh read: one line in the tools, no list, no side column.` plus its place in "Hide gaps". With the other slice's 20 B that leaves about 15 B. Cut, named, if the text overruns: "Known limits", `**The build warns**` (76 B), a figure that belongs to the build and goes stale with it.
