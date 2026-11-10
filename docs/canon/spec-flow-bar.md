---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-11
---

# The Flow screen: the bar, Gaps and Records for the AI

The canvas bar's overlays and the list of what the canvas leaves out; `flow-scenario`, code `ui/src/flow/FlowView.tsx`, `FlowGaps.tsx`, `FlowLegend.tsx`, `FlowRecords.tsx`. The screen: `spec-flow-screen.md`; the rows: `spec-flow.md` "Gap rows"; the cut: `spec-flow-cuts.md`. Rows and marks stop nothing (ADR-0012).

## The bar

The centre's last row: Legend (the Canvas tab only), then Gaps (on the List tab too), then `flow.notDrawn` past the cap. Each toggle is a subtle compact `Button` with `aria-expanded`. Gaps reads "Gaps", then a `Lozenge`: warning `flow.gap.found` (the rows found) when a row is measured above 0, else neutral `flow.gaps.notAll` when a row is not measured, else success `flow.gap.none`. An open overlay lies over the canvas from its bottom edge, at most half its height, scrolling inside; one at a time; Esc closes it, focus to its toggle; a focus or click in the canvas closes it. The Gaps panel follows the bar in the DOM, so Tab runs from its toggle into it; the grid lays it over the canvas.

## Gaps

The drawing read's ten rows: those found, in served order; those not measured; then the rows at 0 as one line, `flow.gaps.zero`. After a failed re-read, every row in served order, with no state, item or cause.

- **A row:** its `flow.gapMeaning.*` sentence, its state in words (`<n> found`, `Not configured`, `Cannot measure`, the last in `cannot-verify`'s colour and icon), its items; its code and `cause` under a collapsed "Technical details" (`Disclosure`), the cause the daemon's words, `lang="en"`. An unknown code shows raw, `lang="en"`.
- **Items:** eight, then "Show all <n>" (focus to the ninth); members by title joined ` + `; each a button, but `no-start`'s scope, text. Activating one closes the control, then selects its first member by a replace when drawn, else seeks it, the fix a push (`spec-flow-place.md` "The seek"); marks every drawn member, centres the first, focus to the panel heading.
- **Hand to your AI**, beside each measured item: New task preset (`NewCardDialog` `preset`), members as targets in order, the `cause` verbatim as goal, `flow.handTitle` (`flow.handTitleCut` past 120 characters). Create is one `createCard`; no `T-` task. `no-start` hands the scope's root when rooted, nothing at Top.
- **Marks.** A drawn box a row names shows "Gap"; a group box its served `gaps`.
- **Titles.** A member's title is the drawing read's gap `titles` (`spec-flow.md` "Output"), else its name shows; the control reads nothing (RM-0101).

## Records for the AI

- **The button**, subtle, at the foot of the places pane: `flow.records.open` with N, the drawing read's `not_in_flow` for a scope read; in a stage view the held scope read's, with no read of its own, else `flow.records.openPlain`. None at an unrooted Top, at N = 0, or on a read the daemon did not cut (`not_in_flow` null).
- **The dialog** (`Dialog`, 600 px, `aria-modal`) is labelled by its `h2`, `flow.records.title` with N, then `flow.records.about`. Its read, `recordsForm(scope)`, the scope read uncut and without trace, is made only while it is open, and a live mark re-reads it only then. It lists the read's nodes with `in_flow: false` but the scope's own: those with an ID grouped by raw `kind` (sorted, `flow.records.noKind` last), each group a `role="group"` labelled by its heading, entries in served order, each a `DocLink` by title opening the document modal over the dialog (`doc-view.md`), a folded one adding `flow.holds`; ID-less nodes not listed, one line `flow.records.noId` linking the Spec tree at the scope. The first 200 (`RECORDS_SHOWN`), then `flow.records.showAll`, focus to the 201st.
- **States:** a skeleton, `flow.records.reading`; a failure, `flow.records.failed`, the daemon's words `lang="en"`, a Retry `Button`; none out of the flow, `EmptyState` `flow.records.none`. Close or Esc returns focus to the button; a key inside reaches no Flow shortcut.

## The cut's empty states

In the canvas region, each an `EmptyState`, heading level 2:
- the scope's own node out of the flow and nothing else kept, at All stages: `flow.emptyAside` with `flow.emptyAsideMore` (N), Records for the AI its one act, primary;
- a scope in the flow whose documents the cut all dropped: `flow.emptyScope`, Records for the AI a secondary act.

## Tests

`ui/src/flow/FlowView.test.tsx` "the Gaps control (FS-17)", "Records for the AI (FS-18)", "empty states (FS-21)" over `src/test/asideStub.ts`; `labels.test.ts`; `mocks/flow.smoke.test.tsx` on `aside` and `aside40`.
