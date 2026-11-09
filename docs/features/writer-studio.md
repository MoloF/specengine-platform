---
class: spec
status: shipped
shipped: 2026-11-09
scope: [ui]
ref: RM-0084; v4 step 2b of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"), his Flow layout and the defects he hit; Q9, Q10 and the removed zoom buttons are working answers, awaiting the owner's review
adrs: []
---

# writer-studio: Flow in three panes, one scope at a time, the place in the URL (RM-0084)

## Why

On 2026-11-08 the owner could not work on Flow. An edge click fell through to the open group's frame (`interactionWidth={0}`), selecting the group, pushing a hash and re-centring. The panel borrowed Graph's `clamp(22rem, 68vh, 46rem)` beside a short canvas: one flow made the page 8 839 px tall in a 1 086 px viewport. The Stage filter only dimmed and judged no group; the List's `path:line` rows could not be clicked; the about text, notes and an always-open legend took the screen. His layout: a left pane by stage with the scope's groups, bound to the URL; a canvas of the current scope only; a right pane always shown; few canvas actions; the legend a thin bottom bar; a one-line header; one screen, no page scroll. Built over 2a's stage read (RM-0083) and 1a's module (RM-0081), its strings in 1b's catalog (RM-0082).

### Decisions

- **D1** One scope at a time: a group is entered, never opened in place; no frames, open-group set or nested lanes.
- **D2** Three `getFlow` forms, always `trace`: Top `{depth: 0}`, a scope `{root: R, depth: 1}`, a stage `{root: R, stage: S}`; lanes from `getTrace({ref: R, summary: true})`, the scope's text from `getNode(R)`; `getTree` only seeks. No daemon change.
- **D3** The lens root: the module's `root`; else Top's one node, if it holds documents; else the unrooted Top.
- **D4** The place in the hash, `#/<p>/flow[/<name>][?module=&scope=&stage=&edge=]`; old links still select; `specengine.module.<p>` the default.
- **D5** A scope, stage or module change pushes; a selection, a seek's fix and a correction replace; on entering Flow a bare hash takes the last place.
- **D6** Three fixed panes, 15rem, at least 20rem, 22rem; no page scroll above 64rem; the places a disclosure from 64 to 80rem; stacked under 64rem.
- **D7** The panel always mounted. Retired: the gaps column, Hide gaps, `specengine.flow.gaps-hidden`, the Stage filter, the Timeline tab, `--flow-dimmed`.
- **D8** An edge click selects the edge, a 12 px hit; mouse only. **D9** The canvas holds still: the wheel pans, one fit per read; no `onlyRenderVisibleElements`.
- **D10** The List an outline of links; `path:line` in the panel only (Q10). **D11** A gap goes to the AI as a New card, its members the targets, its `cause` the goal; never a `T-` task (Q9).
- **D12** Letter keys by `letterOf`; Space enters a group. **D13** Every chrome string a `flow.` key; `src/flow/` in 1b's scan. **D14** Graph and the Roadmap Chain untouched.

## Description and interactions

How it works now: `docs/canon/spec-flow-screen.md` (studio, reads, gap rows, panel, List, states, keys), `docs/canon/spec-flow-place.md` (the place, history, module, seek, memory, the places pane), `docs/canon/spec-flow-layout.md` (one lane, the fit).

## Data

### The place (`ui/src/app/routes.ts`)

`FlowPlace`, `flowHash`, `parseFlowPlace`: `spec-flow-place.md` "The place".

### The reads

`spec-flow-screen.md` "Reads": seven calls; drawing keys `staleTime: Infinity`, `refetchOnMount: "always"`.

### State, client, tokens

`FlowState {tab, place}` (`spec-flow-place.md` "Memory"); `FlowOptions.stage`, sent by `HttpClient.getFlow`; `NewCardDialog` `preset`; tokens `--flow-places-width`, `--flow-panel-width`, `--flow-canvas-min` added, `--flow-dimmed` and `--flow-frame-surface*` gone; `EDGE_HIT = 12`; `.flow-studio` and its panes' `.flow-*` classes, no `graph-details`.

### Strings (`flow.` in `ui/src/i18n/en.ts`)

The `flow` group, Russian in `ru.ts`; names as built: "Implementation".

### Mocks (`ui/src/mocks/`)

`spec-flow-screen.md` "Mock": DOM-PILOTS under DOM-HARBOR (MEC-PILOT-CALL `landfall`, MEC-HARBOUR-LOG `passage`, MEC-PILOT-RETURN `quayside`); no `depends_on` flow edge; `getFlow` takes `stage`, `getTrace` a `ref` with `summary`.

## Rules and edge cases

In the canon: R1 the reads only, R5-R8 states (`spec-flow-screen.md` "Reads", "States and live"); R2 an unserved module or a stage at an unrooted Top, R3 the seek, R4 an undrawn arrow, each dropped or fixed by a replace (`spec-flow-place.md`); R6 the 200-box cap, the List whole; R9 focus, R10 the legend and bar, R11 the wheel under 64rem ("Keyboard, motion, width", "Studio"); R12 nothing blocked ("Rules").

## Acceptance criteria

Tests on the mocks, looks on the built UI; each named mutation turns its check red.

- [x] **ST-1 The place round-trips** (`app/routes.test.ts`): 200+ places across module, scope (`/ # ? & = , + %`, a space), stage (`-`, a Cyrillic word) and select; the old form selects; a name beats an `edge`. M1 `URLSearchParams`, M2 the old form lost, M3 edge parts unencoded.
- [x] **ST-2 Back and reload** (`mocks/flow.smoke.test.tsx`): enter DOM-BERTHS, choose `quayside` (pushes), select MEC-MOORING, then Esc's clear (replaces): `history.length` +2; Back, Back; a fresh mount draws the hash; a bare hash takes the last place. M4 a selection pushing, M5 `FlowState` the store, M6 the module bar replacing.
- [x] **ST-3 An edge click selects it**: the 12 px path fills the panel, `edge=` by a replace, no read, the viewport still. M7 `interactionWidth={0}`, M8 no `onEdgeClick`, M9 the click reaching the pane or a box.
- [x] **ST-4 The page holds still** (`flow/styles.test.ts`): `dvh` less the shell, `overflow: hidden`, panes `overflow: auto`, `min-height: 0`; no `clamp(`, `68vh`, `align-items: start`; no `graph-details` in Flow's DOM. M10.
- [x] **ST-5 The panel is always there**: the scope's title, counts, ways out, ten rows; a box and Esc swap in one element; `specengine.flow.gaps-hidden` neither read nor written. M11, M12.
- [x] **ST-6 A stage narrows**: `landfall` under DOM-HARBOR is one rooted read: RULE-HIGH-WATER, RULE-SPRING-WINDOW, MEC-PILOTAGE, MEC-PILOT-CALL, MEC-FAIRWAY absent; two groups in the pane; no `--flow-dimmed`. M13, M14.
- [x] **ST-7 One-line header**: `h1`, title, two tabs, one disclosure holding the about text and each note once. M15.
- [x] **ST-8 Legend at the bottom**: the centre's last row, open absolute at `bottom: 0`; first Esc closes it, the second clears; a canvas focus closes it. M16, M17.
- [x] **ST-9 The List outline**: every node and edge, past the cap; `<a>` rows with `flowHash` `href`s, replacing; no `path:line`. M18-M20.
- [x] **ST-10 Landmarks and keys**: `nav` "Flow places", `region`, `complementary`, one `h1`; one canvas tab stop, no focusable edge. M21, M22.
- [x] **ST-11 Narrow widths**: CSS pins under 64rem and from 64 to 80rem; `panOnScroll` off under 64rem. M23.
- [x] **ST-12 Hand to your AI** (smoke): `cycle-no-repeat`'s MEC-FAIRWAY + MEC-PILOTAGE: both targets, the cause verbatim, the title; one `createCard`, no task call. M24, M25.
- [x] **ST-13 The reads stay bounded** (smoke): (a) four reads; (b) a stage one; (c) a box two; (d) a scope three; (e) Back and Forward none; (f) the old link `#/harbor-sim/flow/MEC-PILOT-RETURN`, ten reads (the box's two included), replaced by `?module=&scope=DOM-PILOTS`; (g) `module=berths`, no `{depth: 0}`. M26-M28.
- [x] **ST-14 Strings in the catalog** (`policy.test.ts`): `src/flow/` in LG-10's scan. M29.
- [x] **ST-form.** `pnpm lint`, `pnpm build` (726.99 kB), `pnpm test` (3581) at iteration 2; eval `ui_policy`, `anonymity`, `doc_pointers`, `build_graph`, `workspace_names`; `spec check` 0 errors, 0 warnings; `src/graph/`, `src/roadmap/`, `.graph-details*`, `.roadmap-*` unchanged.
- [x] **L1-L5**, looks on 2026-11-09: "Implementation".

## Out of scope

Canvas editing; a `T-` task; a card's discipline from the workspace (RM-0085); Graph and the Chain; any daemon, CLI or MCP change; the other screens' strings (RM-0090), `tree/LinksPanel.tsx` and `trace/CardsBlock.tsx` in Flow's panel too; a parent chain beyond Up; exit steps; a lifted edge's members; journeys; module filters on the Board and Roadmap; `[[flow.exclude]]`; RM-0070.

## Implementation

Review ACCEPT, minors fixed in iteration 2 with two look defects: a hidden label in a static pane grew the page's scroll; a wrapped shell header (97 px at 1100) pushed it past the viewport. Main chunk 726.99 kB, +12.19 kB, mostly strings.

**Departures, for the owner's review.** (1) SA1, that a rooted read gives its root's holder, was false: `group` is null, so Up comes from held reads (`useHeldHolder`); a fresh deep mount says `flow.upStart` and may skip a level; a held holder shows its ID, not its title. (2) ST-2: Esc's clear stands in for MEC-OLD-QUAYS, archived, in no read. (3) ST-13(f) counts ten reads, not eight. (4) ST-2, 6, 12, 13 run in `mocks/flow.smoke.test.tsx` (policy AC-05); ST-12 types the discipline `copy`. (5) Keys: `flow.about`/`aboutNotes`, `emptyNoStage`, `handTitleCut`, `showingStage`; lanes reuse `flow.confirmed`. (6) D5: the last place replaces a bare hash on entry only. (7) The zoom and fit buttons are removed: no keyboard zoom or refit, the List the keyboard path; for the owner to confirm. (8) `layoutFlow` is one read, one lane: nesting, frames, `FlowBox.form/group/nesting/noRoom` gone. (9) `.app-body:has(.flow-studio)` drops the shell's floor above 64rem; every scrolling pane is positioned. (10) One hidden box panel stays mounted, its queries live. (11) The places pane's canon sits in `spec-flow-place.md`, past this spec's 4 096 B plan.

**Looks, 2026-11-09**, Chrome, exact-size iframe, CDP input, all pass on the re-run: `scrollHeight == innerHeight` at 1440x900, 1920x1080, 1100x800 (root, DOM-PILOTS, DOM-HARBOR `landfall`); L1 canvas rect still, header 36 px; L2 the arrow DOM-WATER-MEC-PILOTAGE by midpoint and +5 px, history and transform unchanged; L3 `scrollWidth` 320; L4 canvas 480 px, Places over it; L5 `spec serve` on a scratch `fixtures/trace-almanac`: BOOK-ALMANAC, `gather` STEP-CALL and STEP-SORT, console clean.

| Module | What it does |
|---|---|
| `app/routes.ts`, `Shell.tsx` | the place; `FlowState`; the module bar off Flow |
| `flow/FlowView.tsx`, `FlowPlaces.tsx`, `FlowPanel.tsx` | studio, reads, history, seek; places; panel |
| `flow/FlowCanvas.tsx`, `FlowList.tsx`, `FlowGaps.tsx`, `FlowLegend.tsx` | one read, edge clicks; outline; rows, hand-off; legend |
| `flow/layout.ts`, `geometry.ts`, `labels.ts`, `timeline.ts`, `clicks.ts` | one lane; `EDGE_HIT`; words; lanes; `isPlainClick`. `FlowDetails.tsx`, `TimelineView.tsx` removed |
| `api/client.ts`, `http.ts`, `queries.ts` | `stage`; `flowForm`, `useHeldHolder`, `useTopOnDemand` |
| `board/CardDialogs.tsx`, `workspace/ModuleBar.tsx`, `i18n/` | `preset`; the Tree's bar; `flow` keys |
| `styles/app.css`, `tokens.css`; `mocks/` | the studio; DOM-PILOTS, `stage` |
