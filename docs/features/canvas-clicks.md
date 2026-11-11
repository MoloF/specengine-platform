---
class: spec
status: draft
scope: [ui]
ref: RM-0103, v5's UI lane, its first slice (08 s2 Phase 7, "Owner's word, 2026-11-12"); the two follow-ups the UI review of RM-0098 named (docs/features/handoff-progress.md "Implementation"); the analyst's analysis of 2026-11-12 (canvas-clicks, CC-1 to CC-10, here CK1 to CK10); the orchestrator's working answers, the owner's review open
adrs: []
---

# canvas-clicks: Graph and Roadmap keep their clicks, and a lazy route survives a redeploy (RM-0103)

## Why

Step 6 fixed Flow's clicks (`docs/canon/spec-flow-layout.md` "Layout", Clicks), and the fix was Flow's alone. On Graph and on Roadmap's Chain tab, a press that moves 2-3 px is still taken as a pan, so the click is lost. That is xyflow 12.12.0's 1 px default. Graph's minimap also takes the pointer over a box, and Graph re-centres a box a mouse press focuses, so the box slides away from under the pointer.

After any redeploy, such as the hub's coming upgrade, every page loaded before it fails on its next lazy chunk: Roadmap, Graph, a dialog, the Russian catalog. Retry cannot recover, because `React.lazy` caches the rejection until the page reloads.

The owner's word of 2026-11-12 takes this item now, before his pipeline trial: build the rest to the end, in parallel where safe (`08-roadmap.md` Phase 7, "Owner's word, 2026-11-12"). It is the UI lane's first slice, beside `card-context`'s Rust lane, and RM-0099's `design-sweep` follows it. It is UI only: no crate, no plugin version, no hook or CI change. No ADR: it rests on ADR-0011 (the stack and the canvas) and ADR-0072 (no new string in either catalog).

## Description and interactions

### Decisions

- **D1. One click distance.** `CLICK_DISTANCE = 5` is exported from `ui/src/graph/geometry.ts`, which `flow/geometry.ts` and `RoadmapChain.tsx` already import. Flow's `FlowCanvas`, Graph's `GraphCanvas` and Roadmap's `RoadmapChain` each pass `paneClickDistance={CLICK_DISTANCE}`. `FLOW_CLICK_DISTANCE` leaves `flow/geometry.ts`, with no alias; its doc comment moves with the constant.
- **D2. Minimaps take no pointer.** The existing rule `.graph-canvas .react-flow__minimap` (`app.css`, about line 2604) gains `pointer-events: none`. Every canvas wrapper carries `.graph-canvas` (Flow's `graph-canvas flow-canvas`, Roadmap's `graph-canvas chain-canvas`), so the rule covers all three. Roadmap draws no minimap, so the item's "their minimaps" was wrong there. Flow's own `.flow-canvas .react-flow__minimap` declaration stays, since step 6's C-2 pins it. Graph's `<MiniMap pannable={false} zoomable={false} />` is unchanged.
- **D3. Graph centres only on a keyboard focus.** `GraphCanvas`'s `onFocus` keeps its roving update (`onRove`) on any focus, then returns before centring unless `event.target.matches(":focus-visible")`, as Flow's does. Roadmap's Chain has `nodesFocusable={false}` and no `onFocus`, so it needs none.
- **D4. A lazy chunk gone after a redeploy reloads the page once per build.**
  - A new module `ui/src/app/reload.ts` handles Vite's `vite:preloadError` event on `window`. If the `sessionStorage` mark does not hold this build, it stores the build and reloads.
  - If the mark holds this build, or reading or writing it throws, it does nothing. The route then keeps today's error and Retry, and an unguarded reload cannot loop on a broken deploy.
  - It never calls `preventDefault`: the import would then resolve to `undefined`, which `lazy` turns into a different error.
  - `main.tsx` installs it once, before `readyLanguage()`, so the Russian chunk is covered too.
  - It is silent: no new string. Every asset is served `no-store` (`docs/canon/ui-design.md`), so the reload lands on the new build.
- **D5. Nothing else moves.** No minimap moves, and no zoom floor or layout changes. Retry is not made to recover once the guard has tripped.

## Data

### The constant (`ui/src/graph/geometry.ts`)

```ts
/**
 * How far a press may move between down and up and still be a click, in px: past it, it pans. Every
 * canvas passes it as `paneClickDistance`; a box takes no `nopan`, so a drag that starts on one still
 * pans. React Flow's own default, 1 px, takes a hand's tremor for a pan
 * (docs/features/canvas-clicks.md "Acceptance criteria", CK3).
 */
export const CLICK_DISTANCE = 5;
```

`FlowCanvas.tsx` imports it from `../graph/geometry` and passes it where `FLOW_CLICK_DISTANCE` stands today (about line 683). `GraphCanvas.tsx` passes it in its `<ReactFlow>` props (lines 477-508), and `RoadmapChain.tsx` in its own (lines 333-381).

### The style (`ui/src/styles/app.css`)

```css
/* No canvas's minimap pans or zooms, so it takes no pointer: a box under it takes its click
   (docs/features/canvas-clicks.md "Acceptance criteria", CK4). */
.graph-canvas .react-flow__minimap {
  overflow: hidden;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-s);
  pointer-events: none;
}
```

### The reload (`ui/src/app/reload.ts`, new)

```ts
/** The sessionStorage key: the build a failed chunk has already reloaded once. */
export const RELOAD_MARK = "specengine.reloaded";

export interface ReloadDeps {
  /** Called inside the try: reaching sessionStorage may throw too. */
  storage: () => Pick<Storage, "getItem" | "setItem">;
  reload: () => void;
}

/** One `vite:preloadError`: true when it reloaded. Never calls preventDefault. */
export function onPreloadError(build: string, deps: ReloadDeps): boolean;

/** Adds the one `vite:preloadError` listener to `target`. */
export function installReload(target: Pick<Window, "addEventListener">, build: string, deps: ReloadDeps): void;
```

`onPreloadError`, in order:

1. Read the mark: `deps.storage().getItem(RELOAD_MARK)`. If it throws, return `false`.
2. If the mark equals `build`, return `false`.
3. Write it: `deps.storage().setItem(RELOAD_MARK, build)`. If it throws, return `false`.
4. Call `deps.reload()` and return `true`.

`main.tsx`, on the line before `void readyLanguage().then(boot);`:

```ts
installReload(window, import.meta.url, { storage: () => window.sessionStorage, reload: () => window.location.reload() });
```

`import.meta.url` of `main.tsx` is the entry chunk's URL in a build, carrying its content hash, so the mark allows at most one automatic reload per build. A tab that reloads onto a new build that is broken too may reload once more, for that build, and then stops. The literal `"vite:preloadError"` stands once in non-test sources, in `reload.ts`'s `addEventListener` call, a code position for `policy.test.ts`.

## Rules and edge cases

- WHEN a press moves at most 5 px between down and up on a Graph box or a Chain item, the canvas SHALL take it as a click: Graph selects the box, and the Chain navigates to `#/<p>/roadmap/<ID>`. Past 5 px it SHALL pan and select nothing.
- WHEN a mouse, touch or pen press focuses a Graph box partly out of view, the view SHALL NOT move. Tab or an arrow key that brings focus to a box out of view SHALL still centre it.
- WHEN a Graph box lies under the minimap, a click on it there SHALL select it.
- WHEN `vite:preloadError` fires, the page SHALL reload at most once per build, and never when the mark cannot be read or written. The event SHALL never be `defaultPrevented`.
- After a redeploy, a failed Russian chunk now reloads the page before `language.failed` shows. A failed Retry no longer keeps the page on the old build.
- A reload drops in-memory screen state, such as unstaged Questions drafts, as any reload does. This happens only after a chunk has already failed.

### Test churn

- `flow/geometry.test.ts` (about line 142): the constant's pin moves to `graph/geometry.test.ts` as `CLICK_DISTANCE` 5.
- `FlowCanvas.tsx`'s import line changes.
- `flow/styles.test.ts` C-2 is unchanged.

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turned red. In jsdom, d3-zoom runs once each event's `view` is set by `defineProperty`, as in `flow/FlowView.test.tsx`'s C-1 (`docs/features/handoff-progress.md` "Implementation"). The trick may become a shared helper in `ui/src/test/` that Flow's, Graph's and the Chain's tests use. CK*n* is the analysis's CC-*n*.

- [ ] CK1 — **Graph: a slight move is a click.** In jsdom: mousedown on a box at (100, 100), mousemove to (103, 101), mouseup, click. The box is selected: its details show it, and the hash follows as today. A second box the same way is selected. A press that moves 8 px pans and selects nothing. Mutation, red: `paneClickDistance` removed from `GraphCanvas`.
- [ ] CK2 — **Roadmap's Chain: the same.** A 3 px press on an item's box leads to `#/<p>/roadmap/<ID>`. An 8 px press selects nothing. Mutation, red: the prop removed from `RoadmapChain`.
- [ ] CK3 — **One constant.** `graph/geometry.test.ts`: `CLICK_DISTANCE` is 5. A source test (`?raw`) finds `paneClickDistance={CLICK_DISTANCE}` once in each of `FlowCanvas.tsx`, `GraphCanvas.tsx` and `RoadmapChain.tsx`, each importing it from `graph/geometry`. `FLOW_CLICK_DISTANCE` stands nowhere in `ui/src`. Mutation, red: `paneClickDistance={1}` on Graph.
- [ ] CK4 — **Graph's minimap takes no pointer.** `graph/styles.test.ts`: the `.graph-canvas .react-flow__minimap` rule holds `pointer-events: none`. `GraphCanvas.tsx` holds `<MiniMap pannable={false} zoomable={false} />`. Mutation, red: the declaration dropped.
- [ ] CK5 — **Graph: a press moves nothing.**
  - The store's size and transform put a box half out of view. A pointer press focuses it with `matches(":focus-visible")` stubbed false: the transform is unchanged, and the box is selected and roving.
  - Tab, or an arrow key, bringing focus to a box out of view (`:focus-visible` true) centres it.

  Mutation, red: centring on every focus.
- [ ] CK6 — **Reload once per build.** `app/reload.test.ts`, `onPreloadError` with a fake storage and a counting `reload`:
  1. a first failure (no mark) calls `reload` once, stores the build, and returns `true`;
  2. a mark for this build: no reload;
  3. a mark for another build: a reload, and the mark becomes this build;
  4. `getItem` throws: no reload, nothing thrown;
  5. `setItem` throws: no reload, nothing thrown;
  6. `storage()` itself throws: no reload;
  7. through `installReload` on an `EventTarget`, a dispatched cancelable `vite:preloadError` is never `defaultPrevented`.

  Mutations, each red: the guard removed (case 2); a reload after the write threw (case 5); `preventDefault` called (case 7).
- [ ] CK7 — **Installed once, at the bootstrap.** A source test over every non-test `ui/src` source finds:
  - the literal `vite:preloadError` once, in `app/reload.ts`;
  - `installReload(` called once, in `main.tsx`, before `readyLanguage()`.

  Mutation, red: the call removed from `main.tsx`.
- [ ] CK8 — **[measure] Size.** After `pnpm build`, first-load JS is at most 845 kB. It is recorded with its gzip and `ru-*.js` raw and gzip, against RM-0104's 823.45 kB. The entry chunk holds `vite:preloadError` (Vite's preload helper still dispatches it in Vite 8.3.1) and `specengine.reloaded`.
- [ ] CK9 — **[look]** In Chrome, in both themes:
  - Graph and Roadmap's Chain: ten boxes in a row, each selected by a real click with a slight move;
  - a Graph box partly under the minimap is selected through it;
  - a half-visible Graph box, once clicked, stays under the pointer.
  - Redeploy: rebuild with a changed `RoadmapView`, restart `spec serve`, and open the Roadmap in a page loaded before. It reloads once and shows the view. With this build's mark preset in `sessionStorage`, it shows its error and Retry and does not reload.
- [ ] CK10 — **Gates.** UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green. The eval tests (`ui_policy`, `anonymity`, `doc_pointers`) and `spec check` pass.

## Out of scope

- Moving any minimap.
- Graph's and Roadmap's zoom floors or layout.
- Making Retry recover once the reload guard has tripped.
- RM-0099's sweep: tokens, primitives, the looks of Graph and Roadmap.
- `provisional.ts`, any string, any Rust.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| ui-developer | `ui/src/graph/geometry.ts`, `ui/src/flow/geometry.ts`, `ui/src/flow/FlowCanvas.tsx`, `ui/src/graph/GraphCanvas.tsx`, `ui/src/roadmap/RoadmapChain.tsx`, `ui/src/styles/app.css` (the one rule), `ui/src/app/reload.ts` (new), `ui/src/main.tsx`; the tests of CK1-CK7 (`graph/geometry.test.ts`, `graph/styles.test.ts`, `graph/GraphView.test.tsx` or a new `graph/GraphCanvas.test.tsx`, `roadmap/RoadmapChain.test.tsx`, `app/reload.test.ts`, the source test), `flow/geometry.test.ts`'s moved pin; a shared jsdom press helper in `ui/src/test/` if made |
| test-engineer, UI | the gates and the mutations (CK10, each CK's mutation), CK8's measure; `policy.test.ts` and `ui_policy.rs` only if a pin moves |

Comments cite `docs/features/canvas-clicks.md` "Acceptance criteria" with a criterion ID, never "Data" (`doc_pointers`; compaction drops it at ship). `card-context`'s UI line (`provisional.ts`, the mock, the builder, `daemonKeys.test.ts`) is that slice's own. It shares no file with this one.

### Work order

1. The ui-developer: D1-D3 with CK1-CK5 first, then D4 with CK6 and CK7.
2. The test-engineer: the gates and the mutations, then CK8. Then the UI review.
3. The orchestrator's look (CK9): the clicks on the mock or a corpus copy, the redeploy on `spec serve`.
4. Ship: the spec-writer ("Canon at ship"). Then `design-sweep` (a).

The Rust lane (`card-context`) runs beside it in the same tree, and the owner commits each slice on its own. At most three iterations.

### Risks

- **jsdom and d3-zoom:** the press tests need step 6's event setup. A shared helper keeps the three canvases' tests alike.
- **Vite's event:** if the build no longer dispatches `vite:preloadError` for a failed dynamic import, the handler never runs. CK8 checks the entry chunk, and CK9 the behaviour.
- **A reload loses unsaved screen state**, but only after a chunk has already failed (Rules).
- **`ui/README.md`** is at 8 191 of 8 192 B (`ui_policy.rs`), so its Canvas line changes at net zero (below).

### Canon at ship

Free bytes now are in brackets.

| File | Edit |
|---|---|
| `docs/canon/spec-flow-layout.md` [2 266] | "Layout", Clicks: `CLICK_DISTANCE` (`graph/geometry.ts`), shared by Graph and the Chain; Graph's minimap and focus rule alike. "Known limits": the RM-0103 line goes |
| `docs/canon/spec-roadmap-screen.md` [9 266] | Chain: a press moving up to 5 px is a click |
| `ui/README.md` [1] | Canvas: "the wheel scrolls the page, a press of up to 5 px a click; Flow's pans, zooms by key;" (the same bytes, reordered: every canvas's click) |
| `docs/canon/ui-languages.md` [393] | "Size": the reload once per build (the mark, its key, silent, never `preventDefault`, both storage calls guarded), and the slice's figure. "Switcher": after a redeploy, a failed chunk reloads before `language.failed` shows. "Known limits": the RM-0103 line becomes "once the guard has tripped, a route's Retry fails until a reload by hand" (net within 393) |
| `CLAUDE.md`; 08 s2 Phase 7 "Owner's word, 2026-11-12", the UI lane; RM-0103 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The owner's word of 2026-11-12:** build now, before the pipeline trial; his trial then judges the fixed clicks.

**The orchestrator's**, the analyst's recommendations: one constant in `graph/geometry.ts`; the minimap rule on `.graph-canvas`; Graph's `:focus-visible` gate; the reload once per build, its mark in `sessionStorage` holding the entry module's URL, both storage calls guarded, no reload when either throws, never `preventDefault`, installed once in `main.tsx` before boot, silent, documented in `ui-languages.md` "Size" (not `ui/README.md`, at its cap).

**The spec-writer's:**
- W1: the name is `CLICK_DISTANCE`. `FLOW_CLICK_DISTANCE` goes with no alias, since a second name for one value invites a drift.
- W2: Flow's own minimap declaration stays: step 6's C-2 pins it, and it costs nothing.
- W3: Graph's `onFocus` still roves on a press. Only the centring is gated, as on Flow.
- W4: `installReload` runs before `readyLanguage()`, not just before `boot`, so the first Russian chunk is covered.
- W5: the criteria are CK1-CK10, not the analysis's CC-1 to CC-10, so they never read as `card-context`'s CC1-CC12 in a comment.

**The owner's questions:** none open beyond the analysis's, which concern `design-sweep`.
