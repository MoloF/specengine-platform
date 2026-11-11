---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-11
---

# The Flow screen

The scenario as a three-pane studio, one scope at a time, lifting nothing (ADR-0033, ADR-0055), its boxes against their cards; `flow-canvas`, `trace-views`, `writer-studio`, `flow-scenario`; code `ui/src/flow/`, `ui/src/trace/`. Reads: `spec-flow.md`, `spec-flow-cuts.md`, `spec-trace.md`; the place and the path: `spec-flow-place.md`; layout, clicks, forms: `spec-flow-layout.md`; the bar, Gaps, Records for the AI: `spec-flow-bar.md`. Gaps and marks stop nothing (ADR-0012); spec changes go through the AI's proposals; cards, a gap's too, are written from the panel.

## Studio

A section, not a Graph mode (`spec graph` needs a REF). `.flow-studio`: places (`--flow-places-width` 15rem), the centre (the canvas, at least `--flow-canvas-min` 20rem, over the bar), the panel (`--flow-panel-width` 22rem).

- **Header**, one line: the `h1` "Flow", the scope's title (` · <stage>` when chosen, an ellipsis when cut), the Canvas and List tabs, one disclosure (`flow.about`; with notes `flow.aboutNotes`) holding the about text, `left_out` and each note of every read shown, once.
- **Canvas**, a `region` "Flow canvas", one read, its cut nodes alone, left to right. A group is entered, a scope change, never opened in place: no frames, no open groups. A click selects a box; a double-click, its caret, Space or the panel's Open enters a group; a pane click does nothing; an edge click selects the edge (`EDGE_HIT`, 12 px). The wheel pans; a pinch, or the wheel with xyflow's zoom key, zooms; no zoom or fit buttons. No `onlyRenderVisibleElements`: a read draws at most 200 boxes, each reached by the arrow keys.
- **Bar**, the centre's last row: Legend, Gaps and, past the cap, `flow.notDrawn` (`spec-flow-bar.md`).
- **Width.** Above 64rem the studio fills the viewport below the shell (`100dvh` less the measured `--flow-studio-top`), `overflow: hidden`, each pane scrolling inside and positioned, so a hidden label adds no page scroll; `.app-body:has(.flow-studio)` drops the shell's floor. From 64 to 80rem the places fold into a disclosure (`flow.placesToggle`) opening over the canvas. Under 64rem one column (header, places closed, canvas, panel) and the page scrolls: `panOnScroll` off, `preventScrolling` false, a drag pans; the bar sticky, `scroll-padding-bottom` its height. Under 48rem the List is the default tab unless another is remembered.

## Reads

| Picture or pane | Call | When |
|---|---|---|
| Top | `getFlow({depth: 0, in_flow: true, trace: true})` | no module, no scope: once, finding the lens root; drawn if it is one |
| Scope R | `getFlow({root: R, depth: 1, in_flow: true, trace: true})` | All stages |
| Stage S in R | `getFlow({root: R, stage: S, in_flow: true, trace: true})` | `-` for No stage |
| Lanes | `getTrace({ref: R, summary: true})` | each rooted scope, never at Top |
| Scope view | `getNode(R, {with: ["links"]})`, its `ancestors` the path | each rooted scope |
| Box N | `getNode(N, {with: ["links"]})`, the document modal's key; its work state and marks from the drawing read | N selected |
| Records | `getFlow({root: R, depth: 1})` (`recordsForm`) | while its dialog is open |
| Seek | `getTree({})`, Top's read | on demand: a name no drawing read holds, a lens root unknown |

- **No whole read.** Every drawing `getFlow` is one of the three forms (`flowForm`), always `trace` and `in_flow`; a stage read carries its scope's root, else it admits the whole stage. No `getTrace` is `{}` or holds `stage`; `getTree` never draws.
- **The lens root**, the scope when the hash names none: the chosen module's `root`; else Top's one node, when Top holds exactly one and it holds documents; else the unrooted Top, offering no stage, only its groups.
- **Kept answers.** Drawing keys: `staleTime: Infinity`, `refetchOnMount: "always"`: a place read before draws at once and reads again on a marking live event or the next entry, so a hand edit shows on coming back. Back and Forward over held places read nothing.
- **Nothing lifted.** Each drawn edge is the read's, unchanged, `count` included; an edge into a group ends on its box. The scope's own node is not drawn: it is the panel's scope view, its edges in the List.

## Gap rows

In the bar's Gaps control: `spec-flow-bar.md` "Gaps". The scope view lists none.

## Panel

`role="complementary"` "Details", always mounted, at `--flow-panel-width`, scrolling inside.

- **Titles first.** Boxes, List rows, places, gap items and headings show a node's `title`, else its ID, else its path; an untitled group, its ID. IDs, raw kinds, places, `distance`, start or end and `inner` sit under one collapsed "Technical details" (`ui.technicalDetails`).
- **The scope**, nothing selected: its title (`flow.topTitle` at Top), counts and bar (the node's `counts`), its text's first paragraph, Open document (primary), New task (the scope as target), Split to departments… (`card-split.md`); name, `kind`, path folded. An unrooted Top: no document, no act.
- **A box**: `backTo(scope)` on top, acting as Esc; its title (`h2`); a line of its stage, work state (a `Lozenge` of the read's `rollup`) and the canvas's marks (suspect, departed, Gap), each a label and an icon; its text's first paragraph (the first top-level `paragraph` `src/markdown/` parses, anchors as `DocLink`s); Open document (primary), New task (a card made noted under the acts until the selection changes), Split to departments…, a group's Open; its `group` by title as a place link unless the scope; folded: name, kind, place, distance, start and end, `inner` per type. Its text, links and cards are the document modal's (`doc-view.md`). The last box stays mounted, hidden, while the scope or an arrow shows, so coming back reads nothing.
- **An edge**: From and To, each a link selecting that end (a group end adds Open); Type, raw; Kind, `flow.edgeKind.*` (unknown raw) or `none`; When, or `none`; Links, the `count`; `path:line`, the first written.

**Cards block.** `src/trace/CardsBlock.tsx`, in the tree's `NodePane` and the document view (its own New task off there): the rollup, a read of more than one node its counts; each card once, its mark, `flow/column`, a link to the Board; `uses` rows; New task, the node preset. A suspect card's Clear is the Board's dialog and finishes on the page; a read of more than one node with one to clear adds Clear all, no effect only, one POST (`ClearGroupDialog`). With no row for the node (`rowOf`) it says the state is not known, never another's. A failed read: the daemon's words, Retry; a failed re-read adds that what stands below is from the last read.

## List

The canvas's text twin, in served order, past the cap too: each box an `<a>` selecting it by a replace (a group adds `<holds> documents` and Open); under it its outgoing edges, each a link selecting the edge, `to <to title>, <type>[, <kind>][, when <when>][, ×<count>]`; the scope's own edges first, under its name; in a stage view, boxes under their `group`. No row prints `path:line`; the panel does.

## States and live

- Loading: a skeleton, `aria-busy`, until the drawing read lands; the places pane its own.
- Failed drawing read: `ErrorPanel` in the canvas, verbatim, Retry. A failed re-read keeps the picture, a line and Retry, and drops what it cannot vouch for: badges, counts, gaps (`withoutStates`), the legend's state rows, those words in accessible names; rows "Not read". An unknown mark word is a neutral badge. Retry hands focus on when the read settles.
- Empty, each an `EmptyState`: "No flow in this project yet. Next: have your AI add flow links."; a scope holding nothing `flow.emptyScope`; the cut's two (`spec-flow-bar.md`); a stage of no node `flow.emptyStage` (`-`: `flow.emptyNoStage`); a scope naming nothing, the read's `reason`, the places at the lens root. The rows stand.
- No `[flow]`: the canvas drawn, rows needing it `Not configured`. No start: derived starts; none, `distance` null, stages in the summary's order, boxes "Not on the walk", `no-start` and `unreachable` say why. Bad `[flow]`: `Cannot measure`, `<config>:<line>: <message>`.
- A picture landing is announced politely (`flow.showing`, `flow.showingStage`).
- Live: `flow` and `cards` are in `READS_OF_THE_SPEC`: an apply or a gap reads what is shown and marks the rest; `card.*` and decision events read flows and traces again.

## Keyboard, motion, width

`ui/README.md` "Screen rules" applies; the Canvas and List tabs are kept per project in the Shell (`spec-flow-place.md` "Memory"). Width: "Studio".

- **Keys,** one tab stop on the canvas, none on edges: the List is the keyboard's way to an arrow. Arrows move among boxes only: Left and Right to the nearest box of the next or previous column, Up and Down within one, then to the band and across its shelves. Enter selects, focus to the panel heading. Space enters a group, focus to the new picture's first box. `o` the Tree, `c` centres (`letterOf`); Esc closes an open overlay, else clears the selection, focus back where it was made; `?` lists them.
- **Focus** stays on the link that changed the scope while it stands; lost, on the canvas region, never `body`. A box's name says its title, stage, marks and gaps.
- **Motion.** Viewport moves take `--duration-*`.

## Rules

- No spec write; no hold word (ADR-0012); no link type quoted in app code, `type` raw. `src/flow/`, `src/trace/` branch on no rollup, stage or mark word but through `labels.ts` (`policy.test.ts`).
- The one exception to the `.kind` scan of `policy.test.ts`: `edge.kind` under `src/flow/`, the closed `FlowEdgeKind` of `mirror.rs`. `FlowNode.kind` stays scanned.
- Every chrome string of `src/flow/` is a `flow.` key (`ui-languages.md`) or a `labels.ts` word, data a placeholder; `src/flow/` is scanned.

## Mock

`ui/src/mocks/flow.ts`, harbor-sim: DOM-HARBOR's groups, DOM-PILOTS among them, so `landfall` and `quayside` each span two; MEC-HARBOUR-LOG's long title, body and token; every flow link type and `kind`, arcs, a self-repeat, a definition two levels down, a child of DOM-BERTHS with no flow link. `getFlow` cuts `stage`, then `in_flow`, after the fold; under a ROOT it serves Top's rows, keeping items with a member in scope. ledger-api: no `[flow]`. `empty`: rows, no node. `large`: 500 documents in 25 groups. `no-gaps`: ten rows at None. `aside`, `aside40` (`src/mocks/harbor-sim/aside.ts`): `fixtures/flow-aside`'s pottery under harbor-sim's slug, the second with 5 680 records, no modules; `in_flow`, the cut, `not_in_flow`, `types`, `ancestors` pinned to the CLI's numbers; membership is the mock's alone. `mocks/trace.ts` serves `spec trace`, a REF's lanes by its own walk; `withTrace` the badges. Only the mock may lift an edge or join a rollup. `flow200()` (`src/test/flowStub.ts`) is the trial's input.

## Known limits

- **Decision 11, the owner's.** The 200-box trial with random wiring passes (no overlap, under 500 ms, fewer than 2,500 forward crossings); the look at the cap, `?scenario=large`, is his to judge. A layout library needs his allowlist.
- **Cost.** At 100k a scope change waits about 1.5 s on two O(N) reads, each taking a hub's project turn (`scale-eval.md` "Baseline").
- **Two hashes, one picture.** Before the lens root is known, the path's top entry writes `scope=<id>`, the lens root's picture under a second hash.
- **Minimap.** Over a box of a tall flow at fit; a click passes through.
- **Zoom.** No keyboard zoom or refit, no zoom or fit buttons (a working answer for the owner); the List is the keyboard path.
- The hidden box panel keeps its node query live.
- Edges answer the mouse only; only an item's first member is sought; exit steps carry no mark (core `Steps.leaving` is not served).
- 320 px and the still page are proven by looks, not tests: jsdom lays nothing out.

## Tests

`ui/src/flow/*.test.*` (`FlowView.test.tsx`, `src/test/asideStub.ts` the pottery's, `styles.test.ts` the CSS pins); `ui/src/mocks/flow.smoke.test.tsx` (harbor-sim: history, a stage, the hand-off, bounded reads), `flow.test.ts`, `trace.test.ts`; `app/routes.test.ts`; `trace/CardsBlock.test.tsx`; `markdown/Markdown.test.tsx` (the first paragraph); `api/*.test.*`; `policy.test.ts`, `styles/tokens.test.ts`.
