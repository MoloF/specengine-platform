---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-09
---

# The Flow screen

The flow as a three-pane studio, one scope at a time, lifting nothing (ADR-0033, ADR-0055), its boxes against their cards; `flow-canvas`, `trace-views`, `writer-studio`; code `ui/src/flow/`, `ui/src/trace/`. Reads: `spec-flow.md`, `spec-trace.md`; the place and the places pane: `spec-flow-place.md`; placing, fit, forms: `spec-flow-layout.md`. Gaps and marks stop nothing (ADR-0012); spec changes go through the AI's proposals; cards, a gap's too, are written from the panel.

## Studio

A section, not a Graph mode (`spec graph` needs a REF). `.flow-studio`: places (`--flow-places-width` 15rem), the centre (the canvas, at least `--flow-canvas-min` 20rem, over the bar), the panel (`--flow-panel-width` 22rem).

- **Header**, one line: the `h1` "Flow", the scope's title (` · <stage>` when chosen, an ellipsis when cut), the Canvas and List tabs, one disclosure (`flow.about`; with notes `flow.aboutNotes`) holding the about text, `left_out` and each note of every read shown, once.
- **Canvas**, a `region` "Flow canvas", one read. A group is entered, a scope change, never opened in place: no frames, no open groups. A click selects a box; a double-click, its caret, Space or the panel's Open enters a group; a pane click does nothing; an edge click selects the edge (`EDGE_HIT`, 12 px). The wheel pans; a pinch, or the wheel with xyflow's zoom key, zooms; no zoom or fit buttons. No `onlyRenderVisibleElements`: a read draws at most 200 boxes, each reached by the arrow keys.
- **Bar**, the centre's last row: the Legend toggle (`aria-expanded`) and, past the cap, `flow.notDrawn`. Open, the legend lies over the canvas from its bottom edge, at most half its height, scrolling inside; a focus or click in the canvas closes it.
- **Width.** Above 64rem the studio fills the viewport below the shell (`100dvh` less the measured `--flow-studio-top`), `overflow: hidden`, each pane scrolling inside and positioned, so a hidden label adds no page scroll; `.app-body:has(.flow-studio)` drops the shell's floor. From 64 to 80rem the places fold into a disclosure (`flow.placesToggle`) opening over the canvas. Under 64rem one column (header, places closed, canvas, panel) and the page scrolls: `panOnScroll` off, `preventScrolling` false, a drag pans; the bar sticky, `scroll-padding-bottom` its height. Under 48rem the List is the default tab unless another is remembered.

## Reads

| Picture or pane | Call | When |
|---|---|---|
| Top | `getFlow({depth: 0, trace: true})` | no module, no scope: once, finding the lens root; drawn if it is one |
| Scope R | `getFlow({root: R, depth: 1, trace: true})` | All stages |
| Stage S in R | `getFlow({root: R, stage: S, trace: true})` | `-` for No stage |
| Lanes | `getTrace({ref: R, summary: true})` | each rooted scope, never at Top |
| Scope view | `getNode(R, {with: ["links"]})` | each rooted scope |
| Box N | `getNode(N, {with: ["links"]})`, `getTrace({ref: N})` | N selected |
| Seek | `getTree({})`, Top's read | on demand: a name no drawing read holds, a lens root unknown |

- **No whole read.** Every `getFlow` is one of the three forms (`flowForm`), always `trace`; a stage read carries its scope's root, else it admits the whole stage. No `getTrace` is `{}` or holds `stage`; `getTree` never draws.
- **The lens root**, the scope when the hash names none: the chosen module's `root`; else Top's one node, when Top holds exactly one and it holds documents; else the unrooted Top, offering no stage, only its groups.
- **Kept answers.** Drawing keys: `staleTime: Infinity`, `refetchOnMount: "always"`: a place read before draws at once and reads again on a marking live event or the next entry, so a hand edit shows on coming back. Back and Forward over held places read nothing.
- **Nothing lifted.** Each drawn edge is the read's, unchanged, `count` included; an edge into a group ends on its box. The scope's own node is not drawn: it is the panel's scope view, its edges in the List.

## Gap rows

In the panel's scope view: the drawing read's ten rows as served, unknown states last; all None on a fresh read, one line.

- **A row:** code raw, its meaning, a state in words: `None` at 0, `<n> found` with its `cause`; `Not configured` or `Cannot measure` with `cause` verbatim and no number, the latter in the `cannot-verify` colour and icon; another state a neutral badge.
- **Items:** eight, then "Show all <n>", members joined ` + `; each a button, but `no-start`'s scope, text. Activating one selects its first member by a replace (undrawn: a seek, its fix a push), marks every drawn member, centres the first, focus to the panel heading.
- **Hand to your AI**, beside each measured item: New card preset (`NewCardDialog` `preset`), members as targets in order, the `cause` verbatim as goal, `flow.handTitle` (`flow.handTitleCut` past 120 characters). Create is one `createCard`; no `T-` task. `no-start` hands the scope's root when rooted, nothing at Top.
- **Marks.** A drawn box a row names shows "Gap"; a group box its served `gaps`.

## Panel

`role="complementary"` "Details", always mounted, at `--flow-panel-width`, scrolling inside.

- **The scope**, nothing selected: title, name, `kind`, counts and bar (the node's `counts`); text and links; Open in Spec tree; New card (the scope as target); the gap rows. At Top: `flow.topTitle` and the rows.
- **A box**: its words, `path:line`, `distance`, stage, a group's `inner` and Open, its `group` with Open unless the scope, `backTo(scope)` on top, acting as Esc; the Cards block; text through `src/markdown/`; links by `LinksPanel`; "Open in Spec tree", "Open in Graph"; "Changes go through your AI, as proposals." The last box stays mounted, hidden, while the scope or an arrow shows, so coming back reads nothing.
- **An edge**: From and To, each a link selecting that end (a group end adds Open); Type, raw; Kind, raw or `none`; When, or `none`; Links, the `count`; `path:line`, the first written.

**Cards block.** `src/trace/CardsBlock.tsx`, here and in the tree's `NodePane`: the rollup, a read of more than one node its counts; each card once, its mark, `flow/column`, a link to the Board; `uses` rows; New card, the node preset. A suspect card's Clear is the Board's dialog and finishes on the page; a read of more than one node with one to clear adds Clear all, no effect only, one POST (`ClearGroupDialog`). With no row for the node (`rowOf`) it says the state is not known, never another's. A failed read: the daemon's words, Retry; a failed re-read adds that what stands below is from the last read.

## List

The canvas's text twin, in served order, past the cap too: each box an `<a>` selecting it by a replace (a group adds `<holds> documents` and Open); under it its outgoing edges, each a link selecting the edge, `<type> <to name>, <to title>[, <kind>][, when <when>][, x<count>]`; the scope's own edges first, under its name; in a stage view, boxes under their `group`. No row prints `path:line`; the panel does.

## States and live

- Loading: a skeleton, `aria-busy`, until the drawing read lands; the places pane its own.
- Failed drawing read: `ErrorPanel` in the canvas, verbatim, Retry. A failed re-read keeps the picture, a line and Retry, and drops what it cannot vouch for: badges, counts, gaps (`withoutStates`), the legend's state rows, those words in accessible names; rows "Not read". An unknown mark word is a neutral badge. Retry hands focus on when the read settles.
- Empty: "No flow in this project yet. Next: have your AI add flow links."; a scope holding nothing `flow.emptyScope`, a stage of no node `flow.emptyStage` (`-`: `flow.emptyNoStage`); a scope naming nothing, the read's `reason`, the places at the lens root. The rows stand.
- No `[flow]`: the canvas drawn, rows needing it `Not configured`. No start: derived starts; none, `distance` null, stages in the summary's order, boxes "Not on the walk", `no-start` and `unreachable` say why. Bad `[flow]`: `Cannot measure`, `<config>:<line>: <message>`.
- A picture landing is announced politely (`flow.showing`, `flow.showingStage`).
- Live: `flow` and `cards` are in `READS_OF_THE_SPEC`: an apply or a gap reads what is shown and marks the rest; `card.*` and decision events read flows and traces again.

## Keyboard, motion, width

`ui/README.md` "Screen rules" applies; the Canvas and List tabs are kept per project in the Shell (`spec-flow-place.md` "Memory"). Width: "Studio".

- **Keys,** one tab stop on the canvas, none on edges: the List is the keyboard's way to an arrow. Arrows move among boxes only, Up and Down in a column, Left and Right across. Enter selects, focus to the panel heading. Space enters a group, focus to the new picture's first box. `o` the Tree, `c` centres (`letterOf`); Esc closes the legend if open, else clears the selection, focus back where it was made; `?` lists them.
- **Focus** stays on the link that changed the scope while it stands; lost, on the canvas region, never `body`. A box's name says its title, marks and gaps.
- **Motion.** Viewport moves take `--duration-*`.

## Rules

- No spec write; no hold word (ADR-0012); no link type quoted in app code, `type` raw. `src/flow/`, `src/trace/` branch on no rollup, stage or mark word but through `labels.ts` (`policy.test.ts`).
- The one exception to the `.kind` scan of `policy.test.ts`: `edge.kind` under `src/flow/`, the closed `FlowEdgeKind` of `mirror.rs`. `FlowNode.kind` stays scanned.
- Every chrome string of `src/flow/` is a `flow.` key (`ui-languages.md`) or a `labels.ts` word, data a placeholder; `src/flow/` is scanned.

## Mock

`ui/src/mocks/flow.ts`, harbor-sim: DOM-HARBOR's groups, DOM-PILOTS among them, so `landfall` and `quayside` each span two; MEC-HARBOUR-LOG's long title, body and token; every flow link type and `kind`, arcs, a self-repeat, a definition two levels down, a child of DOM-BERTHS with no flow link. `getFlow` cuts `stage` after the fold; under a ROOT it serves Top's rows, keeping items with a member in scope. ledger-api: no `[flow]`. `empty`: rows, no node. `large`: 500 documents in 25 groups. `no-gaps`: ten rows at None. `mocks/trace.ts` serves `spec trace`, a REF's lanes by its own walk; `withTrace` the badges. Only the mock may lift an edge or join a rollup. `flow200()` (`src/test/flowStub.ts`) is the trial's input.

## Known limits

- **Decision 11, the owner's call.** The 200-box trial with random wiring passes (median 1.9 ms, no overlap) with 1,920 forward crossings, "a hairball by eye"; the look at the cap, `?scenario=large`, is his to judge. A layout library needs his allowlist.
- **Cost.** At 100k a scope change waits about 1.5 s on two O(N) reads, each taking a hub's project turn (`scale-eval.md` "Baseline").
- **Up** comes from held reads: `spec-flow-place.md` "Known limits".
- **Zoom.** No keyboard zoom or refit; the zoom and fit buttons are gone, a working answer for the owner to confirm. The List is the keyboard path.
- The hidden box panel keeps its node and trace queries live.
- Edges answer the mouse only; only an item's first member is sought; exit steps carry no mark (core `Steps.leaving` is not served).
- 320 px and the still page are proven by looks, not tests: jsdom lays nothing out.

## Tests

`ui/src/flow/*.test.*` (`FlowView.test.tsx` on a stub client, `styles.test.ts` the CSS pins); `ui/src/mocks/flow.smoke.test.tsx` (harbor-sim: history, a stage, the hand-off, bounded reads), `flow.test.ts`, `trace.test.ts`; `app/routes.test.ts`; `trace/CardsBlock.test.tsx`; `api/*.test.*`; `policy.test.ts`, `styles/tokens.test.ts`.
