---
class: spec
status: shipped
shipped: 2026-10-31
scope: [ui]
ref: RM-0069; Phase 7 step 7d (08 s2)
---

# screen-fit: a drop lands on an empty column, a clean flow takes the width (RM-0069)

## Why

Two findings of the browser checks on 2026-10-31. An empty Board column was 61 px tall, so a card dropped below its heading landed on nothing and nothing said so: "moves them" failed silently. And at 1440 px with gaps shown a real flow drew at about zoom 0.6, because a panel of ten `None` rows held a whole column that said nothing.

## Summary

What became true is in `docs/canon/`. The Board: `board-ui.md` "Moving", CSS alone (D12), since `section.board-column` already takes the drop. The Flow: `spec-flow-screen.md` "Gap rows". A stored `specengine.flow.gaps-hidden` is left as it is and holds again when a row is found. A2: the canvas zoom rises with the freed column; no number is promised, and the gate's look decides.

## Acceptance criteria

- [x] AC-01 — A test over `app.css?raw` pins `.board-columns` `align-items: stretch` with a `min-height`, `.board-column` `align-content: start`, and `min-height: 0` in the stacked block. M: `align-items: start` restored.
- [x] AC-02 — By hand or by a headless look, not a repository test (`ui/` has no browser harness): at 1440x900 an empty column stands at least half the viewport high, and a real mouse drop at 80 % of its height moves the card. Recorded in the spec's "Implementation" with the date. M: the same check at `align-items: start` shows the drop lost.
- [x] AC-03 — Ten clear rows: no region named "Gaps", the line present, `.flow-stage` has `no-gaps` and `no-side`, the Hide gaps button gone. M: the line shown beside rows.
- [x] AC-04 — One row found: all ten as today, no line. A selected box in the clear state: `no-gaps` without `no-side`. M: `allClear` true on any row at 0; `no-side` kept under a panel.
- [x] AC-05 — A failed re-read of a clean flow: every row `Not read`, never the line. M: the line shown beside a failed read.
- [x] AC-06 — `allClear` over an empty list, an unknown state and a `null` count is false. `policy.test.ts` still finds no gap-state branch outside `labels.ts`. M: a state compared in `FlowView.tsx`.

## Out of scope

The Board's drag code. Moving the rows under the canvas. Zooming or new canvas chrome. A clean line on a group's read. A browser harness in `ui/`.

## Implementation

| Module | What it does |
|---|---|
| `ui/src/styles/app.css` | `.board-columns` `align-items: stretch`, `min-height: 60vh`, `0` when stacked; `.board-column` `align-content: start`; `.flow-gaps-clear` |
| `ui/src/flow/labels.ts` | `allClear`, a non-empty list with every row `measured` at 0 |
| `ui/src/flow/FlowGaps.tsx` | `GapsClear`, the `role="status"` line |
| `ui/src/flow/FlowView.tsx` | `clear`, `gapsShown`, the line in the button's place, the focus hand-off |
| Tests | `ui/src/board/styles.test.ts`, `ui/src/flow/labels.test.ts`, `ui/src/flow/FlowView.test.tsx`, `ui/src/policy.test.ts` |

Beyond the spec: `FlowView.tsx` hands lost focus to `.flow-results` with `preventScroll`, on the clean turn only, since that turn takes the rows and their button; `.flow-gaps-clear` stands as tall as the button, so the tools do not jump when a row turns up. Six existing tests got a found row, because `tenGapRows()` is all None.

**AC-02, 2026-10-31.** Headless Chrome 156 at 1440x900, a real mouse drag through CDP on the production build, `?scenario=normal#/ledger-api/board`: the empty column is 224x532 (half the viewport is 450), and a drop at 80 % of its height moved the card. A control with `.board-columns{align-items:start;min-height:0}` injected gave 224x61 and the drop was lost. Limits: the mock judged the move, and the control is an injected style.

**Pending, not done.** The Flow line has not been seen through its real React path, since no mock serves an all-None flow; it is looked at on a real project at the gate.

**Known and accepted.** `clear` stands on the last read while a re-read is in flight, as the `None` rows did. "No gaps" reads Top's rows alone.
