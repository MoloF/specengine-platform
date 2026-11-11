---
class: spec
status: shipped
shipped: 2026-11-12
scope: [ui]
ref: RM-0103, v5's UI lane, its first slice (08 s2 Phase 7, "Owner's word, 2026-11-12"); the two follow-ups the UI review of RM-0098 named (docs/features/handoff-progress.md "Implementation"); the analyst's analysis of 2026-11-12 (CC-1 to CC-10, here CK1 to CK10); the orchestrator's working answers
adrs: []
---

# canvas-clicks: Graph and Roadmap keep their clicks, and a lazy route survives a redeploy (RM-0103)

## Why

Step 6 fixed Flow's clicks, and the fix was Flow's alone. On Graph and on Roadmap's Chain tab, a press that moved 2-3 px was taken as a pan (xyflow 12.12.0's 1 px default), so the click was lost. Graph's minimap took the pointer over a box, and Graph re-centred a box a mouse press focused, so the box slid away from under the pointer.

After any redeploy, such as the hub's coming upgrade, every page loaded before it failed on its next lazy chunk: Roadmap, Graph, a dialog, the Russian catalog. Retry could not recover, because `React.lazy` caches the rejection until the page reloads.

The owner's word of 2026-11-12 took this item before his pipeline trial, as the UI lane's first slice beside `card-context`. UI only: no crate, plugin version, hook or CI change. No ADR: it rests on ADR-0011 and ADR-0072 (no new string).

## Description and interactions

Each decision is canon now. D1 one click distance, D2 no minimap takes the pointer, D3 Graph centres only on a keyboard focus: `docs/canon/spec-flow-layout.md` "Layout" (Clicks), `docs/canon/spec-roadmap-screen.md` "Layout" (Chain), `ui/README.md` "Screen rules" (Canvas). D4 a reload once per build: `docs/canon/ui-languages.md` "Size" (Reload), "Switcher". D5 nothing else moves.

Working answers taken: the name `CLICK_DISTANCE`, `FLOW_CLICK_DISTANCE` gone with no alias (W1); Flow's own minimap declaration kept, step 6's C-2 pins it (W2); Graph's `onFocus` still roves on a press, only its centring gated (W3); `installReload` before `readyLanguage()`, so the first Russian chunk is covered (W4); criteria named CK, never read as `card-context`'s CC (W5).

## Data

In the canon: `spec-flow-layout.md` "Layout" (Clicks); `ui-languages.md` "Size" (Reload): `ui/src/app/reload.ts` `RELOAD_MARK` `"specengine.reloaded"`, `ReloadDeps` `{storage, reload}`, `onPreloadError(build, deps)`, `installReload(target, build, deps)`. No served change, no string, no scale exception.

## Rules and edge cases

In the canon: `spec-flow-layout.md` "Layout" (Clicks), "Known limits"; `ui-languages.md` "Size", "Switcher", "Known limits".

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turned red. In jsdom, d3-zoom runs once each event's `view` is set by `defineProperty` (`ui/src/test/press.ts`, shared by Flow's, Graph's and the Chain's tests). CK*n* is the analysis's CC-*n*.

- [x] CK1 — **Graph: a slight move is a click.** `graph/GraphCanvas.test.tsx`: mousedown on a box at (100, 100), mousemove to (103, 101), mouseup, click. The box is selected: its details show it, and the hash follows. A second box the same way is selected. A press that moves 8 px pans and selects nothing. Mutation, red: `paneClickDistance` removed from `GraphCanvas`.
- [x] CK2 — **Roadmap's Chain: the same.** A 3 px press on an item's box leads to `#/<p>/roadmap/<ID>`. An 8 px press selects nothing. Mutation, red: the prop removed from `RoadmapChain`.
- [x] CK3 — **One constant.** `graph/geometry.test.ts`: `CLICK_DISTANCE` is 5. A source test (`?raw`) finds `paneClickDistance={CLICK_DISTANCE}` once in each of `FlowCanvas.tsx`, `GraphCanvas.tsx` and `RoadmapChain.tsx`, each importing it from `graph/geometry`. `FLOW_CLICK_DISTANCE` stands nowhere in `ui/src`. Mutation, red: `paneClickDistance={1}` on Graph.
- [x] CK4 — **Graph's minimap takes no pointer.** `graph/styles.test.ts`: the `.graph-canvas .react-flow__minimap` rule holds `pointer-events: none`. `GraphCanvas.tsx` holds `<MiniMap pannable={false} zoomable={false} />`. Mutation, red: the declaration dropped.
- [x] CK5 — **Graph: a press moves nothing.**
  - The store's size and transform put a box half out of view. A pointer press focuses it with `matches(":focus-visible")` stubbed false: the transform is unchanged, and the box is selected and roving.
  - Tab, or an arrow key, bringing focus to a box out of view (`:focus-visible` true) centres it.

  Mutation, red: centring on every focus.
- [x] CK6 — **Reload once per build.** `app/reload.test.ts`, `onPreloadError` with a fake storage and a counting `reload`:
  1. a first failure (no mark) calls `reload` once, stores the build, and returns `true`;
  2. a mark for this build: no reload;
  3. a mark for another build: a reload, and the mark becomes this build;
  4. `getItem` throws: no reload, nothing thrown;
  5. `setItem` throws: no reload, nothing thrown;
  6. `storage()` itself throws: no reload;
  7. through `installReload` on an `EventTarget`, a dispatched cancelable `vite:preloadError` is never `defaultPrevented`.

  Mutations, each red: the guard removed (case 2); a reload after the write threw (case 5); `preventDefault` called (case 7).
- [x] CK7 — **Installed once, at the bootstrap.** `app/reload.test.ts`, a TypeScript-parsed check over every non-test `ui/src` source, finds:
  - the literal `vite:preloadError` once, in `app/reload.ts`;
  - `installReload(` called once, in `main.tsx`, before `readyLanguage()`;
  - no dynamic `import()` chained directly with `.catch` or a two-argument `.then`, and the one `import("./ru")` a variable's own initializer (iteration 2).

  Mutations, each red: the call removed from `main.tsx`; `russian ??= import("./ru").then(ok, fail)`.
- [x] CK8 — **[measure] Size.** After `pnpm build`, first-load JS is at most 845 kB, recorded with its gzip and `ru-*.js` raw and gzip, against RM-0104's 823.45 kB. Vite 8.3.1 dispatches `vite:preloadError` from its preload helper, `preload-helper-*.js`, a first-load chunk; the entry chunk holds the listener and `specengine.reloaded`. Measured: 823.88 kB (gzip 238.09), `ru-*.js` 152.30 kB (gzip 35.12).
- [x] CK9 — **[look]** In Chrome, planned in both themes, looked at in dark only ("Implementation"):
  - Graph and Roadmap's Chain: boxes in a row, each selected by a real click with a slight move;
  - a Graph box partly under the minimap is selected through it;
  - a half-visible Graph box, once clicked, stays under the pointer;
  - redeploy: a page loaded before a rebuild with a changed `RoadmapView` opens the Roadmap: it reloads once and shows the view. With this build's mark preset in `sessionStorage`, it shows its error and Retry and does not reload.
- [x] CK10 — **Gates.** UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green. The eval tests (`ui_policy`, `anonymity`, `doc_pointers`) and `spec check` pass.

## Out of scope

- Moving any minimap; Graph's and Roadmap's zoom floors or layout.
- Making Retry recover once the reload guard has tripped.
- RM-0099's sweep: tokens, primitives, the looks of Graph and Roadmap.
- `provisional.ts`, any string, any Rust.

## Implementation

| Module | Does |
|---|---|
| `src/graph/geometry.ts`, `src/flow/geometry.ts` | `CLICK_DISTANCE` 5 for every canvas; `FLOW_CLICK_DISTANCE` gone |
| `src/flow/FlowCanvas.tsx`, `src/graph/GraphCanvas.tsx`, `src/roadmap/RoadmapChain.tsx` | `paneClickDistance={CLICK_DISTANCE}`; Graph's `onFocus` roves on any focus, centres only on `:focus-visible` |
| `src/styles/app.css` | `.graph-canvas .react-flow__minimap` `pointer-events: none` |
| `src/app/reload.ts` (new), `src/main.tsx` | `RELOAD_MARK`, `onPreloadError`, `installReload`, installed with `import.meta.url` before `readyLanguage()` |
| `src/i18n/index.ts` | `loadLanguage`: the bare `import("./ru")`, its handlers chained on the result |
| `src/test/press.ts` (new), the tests of CK1-CK7 | the shared jsdom press, Flow's tests on it too; `graph/GraphCanvas.test.tsx`, `app/reload.test.ts` (new) |

**Departures**, accepted: (1) `main.tsx`'s `reload` callback in braces, as lint asks; (2) CK8: the dispatch is in `preload-helper-*.js`, not the entry; (3) CK7's check lives in `app/reload.test.ts`; (4) CK9's redeploy swapped two scratch builds under `vite preview`, not a restarted `spec serve`, and the Chain took two short presses, not ten.

**Review**: REWORK, one major, fixed in iteration 2, its diff and build checked by the orchestrator; no second review.
- Major: Vite 8 moved a `.then(ok, fail)` written on `import("./ru")` inside its preload wrapper, whose failure handler swallowed the rejection, so a lost Russian chunk dispatched no `vite:preloadError`. Iteration 2: the bare import, `.then` on its result, outside the wrapper in the built `Icon-*.js`; CK7's third check pins it.
- Nits: `flow/geometry.ts` cited `handoff-progress` C-1, now F-4 only (iteration 2); the reload cannot tell a redeploy from a network blip, and a mouse Close of Graph's details does not centre its box (as on Flow): accepted, known limits.
- The reviewer verified the mark changes per build and CK1, CK2 and CK5's mutations red.

**Numbers**, iteration 2: UI 5 216 tests in 179 files, green twice; lint 0; eval `doc_pointers`, `ui_policy` 45 of 45; ten named mutations red. First load 823.88 kB (gzip 238.09), from 823.45; `ru-*.js` 152.30 kB (gzip 35.12).

**Look** (CK9), 2026-11-12, Chrome, CDP mouse, dark, Russian, 2056x1030, the daemon on scratch clones of this repository and a game corpus:
- Graph (ADR-0012, 136 nodes): ten boxes in a row, each selected by a press moving about 3 px; an 8 px press panned, selecting nothing; a box under the minimap selected through it; a half-visible box selected, the view unmoved.
- Chain: 2 px presses led to RM-0039's and RM-0014's routes; an 8 px press panned, leading nowhere.
- Redeploy: on old build A, Roadmap reloaded the tab once onto B, showing B's change, the mark A's entry URL. The mark preset to B, swapped back to A: no reload, the route's error and Retry.
- Light not looked at: the change is behaviour plus one `pointer-events` rule, theme-independent.

**Known limits**: `spec-flow-layout.md` and `ui-languages.md` "Known limits".
