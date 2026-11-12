---
class: spec
status: shipped
shipped: 2026-11-12
scope: [ui]
ref: RM-0099 part b, v5's UI lane, its third and last slice after `canvas-clicks` and `design-sweep` (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (DB-1 to DB-8, here DB1 to DB14); the orchestrator's working answers, the owner's review open; one iteration, then the review's iteration 2
adrs: []
---

# design-sweep-b: Health, Graph, Roadmap and Agent tasks on the primitives, the exceptions down to two, RM-0099 closed (RM-0099 b)

## Why

`design-sweep` (RM-0099 a, `docs/features/design-sweep.md`) renamed every legacy size and weight, turned guards G1 and G2 on, and put Inbox, Questions and the Spec tree on the primitives. Health, Graph, Roadmap and Agent tasks still drew their own buttons, empty states and fact lists. Eight raw sizes stood in `SCALE_EXCEPTIONS` under `until: "RM-0099"`, four header offsets assumed the old 3.5rem header, and the Questions screen sent no `as`.

RM-0099's second half closes the item. It puts the four screens on the primitives (an Agent task in part a's pane, Health's verdict a Lozenge), takes the exceptions down to the two kept (G3), derives the header offsets from `--nav-height` (R8), fits Graph's boxes to `BOX_HEIGHT` (R7) and sends `as` from Questions. UI only: no crate, plugin version, hook or CI change. No ADR: it rests on part a's decisions (ADR-0011, ADR-0072 with ADR-0024, ADR-0031, ADR-0008, ADR-0076, ADR-0012) and on ADR-0047 (a measured zero is not empty).

## Description and interactions

The decisions are canon now:
- D1, a Lozenge for a status in a head and for a verdict, Badges on rows: `docs/canon/ui-design.md` "Primitives".
- D2, Health: `health.md` "UI"; "What is left" keeps its measured-zero lines.
- D3, Graph: DB2 and DB3 (Graph has no canon page).
- D4, Roadmap, its tabs untouched: `spec-roadmap-screen.md` "Layout", "States and live".
- D5, the Agent task pane: `ui-design.md` "The work item".
- D6 to D8, the tokens, G3 and the header: "Guards and exceptions", "The header".
- D9, the old empty states gone, the Board's three too: DB6.
- D10, Questions' `as`: `round-ui.md` "Screen"; the card link, a core key: `card-questions.md` "Known limits".
- D11: no data, endpoint, colour, canvas layout or Overview change; `provisional.ts` a comment only.

The owner's questions are open, and their working answers are built: Q-C, a Lozenge for an item's status in its head and for a verdict, Badges on rows; Q-E, the palette and the sync line take tokens only. So are the spec-writer's W1 to W6: the zero lines kept, two acts on Graph's no-edge state, part a's pane, the card link re-pointed, the Board's empty states only, edge labels at 12 px.

## Data

- **R7.** A selected box holds a 2 px border twice, 4 px padding twice, two 2 px gaps and rows of 20 + 20 + 16 px: 72 = `BOX_HEIGHT`, no slack; the Chain draws the same box. Edge labels are 12/16 (were 11/16.5).
- **Offsets.** The icon offsets take `--space-025`, `.palette` `--space-600` (48 px; was `min(10vh, 5rem)`). `.app-body`, the two sticky panes (now 8 px higher) and a narrow task heading offset by `--nav-height` (R8).
- **Keys.** `graph.details.holder` and `tasks.detail.details` ("Details"), `roadmap.emptyNext`; `roadmap.status` is gone. The Russian is a draft for the owner.

## Rules and edge cases

In the canon: `ui-design.md` "Primitives" (one primary), `health.md` "UI" (cannot-check never clean's look), `round-ui.md` "Screen" (`as`, none when empty). An empty state holds at most two acts, the rest controls; keys, focus and the Chain's clicks behave as before.

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turned red.

- [x] DB1 — **Health.** Check again is a `Button`. The verdict is a `Lozenge`: `cannot_check` gives `discovery` with the `cannotCheck` icon, `clean` `success`, `"weird"` neutral, raw, `lang="en"`. The counts are a `PropertyList`. No finding and a no-match filter are `.empty-state-box`es, Clear filters the second's primary. A row measured at zero still shows "None: …". Mutations, each red: cannot-check mapped to `success`; the counts back in `dl.check-counts`; a zero row as an `EmptyState`.
- [x] DB2 — **Graph's details and empty states.** A holder's facts are a `PropertyList` of `graphFactsOf`'s rows. The two links carry `button button-quiet button-compact`, and Close is a `Button`. No REF and no edge are `.empty-state-box`es; with impact off, mentions offered and archive off, the no-edge state shows exactly two acts, Show impact primary, and the type chips and the archive toggle stay in `GraphControls`. Mutations, each red: the old `dl.facts` markup; three acts in the empty state.
- [x] DB3 — **Graph's boxes fit (R7).** A source test computes a selected box's content height from `app.css` and `tokens.css`, at most `BOX_HEIGHT` from `graph/layout.ts`; `.graph-edge-label` takes the small pair. Mutations, each red: `.graph-box` at `var(--line-height-document)`; `.graph-box-meta` with no line height of its own; `BOX_HEIGHT` set to 64.
- [x] DB4 — **Roadmap.** An empty plan is an `EmptyState` with `roadmap.emptyNext`, the tabs after it. A lane card's status is a neutral `Lozenge`, the value raw. Read again is a `Button`, and the tabs are those before this slice, in order. Mutations, each red: `"done"` as success; the empty plan as a `<p>`.
- [x] DB5 — **Agent tasks.** A task shows Breadcrumbs (Project, Agent tasks, its `T-` ID with `aria-current="page"`), the pane (`issue-view-pane`, side before main), a status `Lozenge` from `taskStatusLozenge` (`review` warning, `"weird"` neutral and raw), and Created and Updated in a `PropertyList`. No task chosen, no such task, an empty list and a no-match filter are `EmptyState`s; the list keys, the heading's focus and Retry's focus stay green. Mutations, each red: `div.empty-state` restored in `TasksView.tsx`; no Breadcrumbs; the status back as a Badge in the head.
- [x] DB6 — **Primitives in the swept directories.** Amended at ship: the Board keeps its other raw buttons (D9). `ui/primitives.test.ts` reads the non-test `.tsx` of `src/{health,graph,roadmap,tasks,palette}/`, `src/app/SyncLine.tsx` and `src/board/CardPanel.tsx`: no `className="button`, no `"button button-` literal, no `"empty-state"` or `"empty-filter"` class. It reads `src/board/BoardView.tsx` for the old empty states only; Dismiss, the show-all-flows toggle and New task stay raw. `app.css` holds no `.empty-state` or `.empty-filter` selector. Mutations, each red: a raw `<button className="button">` in `CheckRegion.tsx`; `.empty-filter` left in `app.css`.
- [x] DB7 — **G3.** `SCALE_EXCEPTIONS` is exactly `.sr-only` and `.tab`, both `until: null`, and its type reads `until: null;`. The four icon offsets take `var(--space-025)`, `.palette` `var(--space-600)`, the edge label the small pair. Mutations, each red: `.graph-edge-label` back at `0.6875rem`; `.palette` back at `min(10vh, 5rem)`; an extra exception entry.
- [x] DB8 — **Header heights.** A source test finds `.app-body`'s `min-height`, the panes' `top` and `max-height` and the task heading's `scroll-margin-top` naming `var(--nav-height)`, and no `3.5rem`, `4.5rem`, `5.5rem` or `7rem` in a `top`, `min-height`, `max-height` or `scroll-margin-top` of `app.css`. Mutation, red: `.app-body` back at `calc(100vh - 3.5rem)`.
- [x] DB9 — **Questions sends `as`.** With "Acting as" holding `Ann`, Stage posts each stage with `"as": "Ann"`; empty, no `as` key; on a hub, the login. Unstage's DELETE sends no body. Mutations, each red: `as` dropped from `answerOf`; `as: null` sent when empty.
- [x] DB10 — **Strings.** The three new keys are in `en.ts` and `ru.ts`, and a key with no user is in neither; `catalog.test.ts` and `words.test.ts` pass. Mutations, each red: `tasks.detail.details` missing in `ru.ts`; "card" in `roadmap.emptyNext`.
- [x] DB11 — **[measure] Size.** First-load JS is at most 845 kB: 827.44 kB in the worktree against part a's 825.27, and 831.15 kB on `main` with RM-0105. The figures are in "Implementation".
- [x] DB12 — **[look]** The orchestrator's, 2026-11-12, in Chrome on the worktree's build over the mock.
  - Health, `cannot-check`, 1440x900, light, Russian: Check again a Button; the verdict lozenge in the discovery look, never green; the cause lines; the counts a PropertyList with its caveat; "What is left" unchanged.
  - Agent task T-0107, 1920x1080, dark, Russian: the pane, its trail (Project, Agent tasks, T-0107), title, status lozenge and owner commands, Details (Created, Updated) on the right; the waiting note; the tabs in main.
  - Roadmap's Chain, 1920x1080, dark, Russian: the boxes' task cells on one line. The MVP cut's label overlaps the `hull` lane's title, likely older than this slice (`spec-roadmap-screen.md` "Known limits").
  - Not looked at (tests cover them): Graph (details, boxes, edge labels), the palette, the sync line and the icons, the sticky panes, every empty state (the Board's too), Health's other verdicts, Roadmap's Lanes, the Agent task list and its one column at 1440, English, 1100x800.
- [x] DB13 — **RM-0099 is closed.** `grep -rn "RM-0099" ui/src` finds only `RoadmapView.test.tsx`'s test ID, `grep -rn "RM-0099" docs/canon` nothing, and `RM-0099.md` reads `status: shipped`.
- [x] DB14 — **Gates.** On `main` after the merge: UI lint has 0 problems, the build passes, and `pnpm test` is green with 188 files and 5,386 tests. The eval tests `ui_policy`, `anonymity` and `doc_pointers` pass, 98 of 98, and `spec check` passes.

## Out of scope

- The Overview (RM-0086); Flow, the document view and the Board beyond its three empty states; the shell's raw buttons.
- The Questions sheet's card link (`card_id` on `RoundItem`, a core key) and its item head (`plain-questions`).
- New data or endpoints, any Rust, colours, the canvases' layout, minimaps and zoom floors, the filter chips' design, `RoadmapView`'s tabs.

## Implementation

Worktree-built, reviewed ACCEPT, then iteration 2; applied onto `main` beside `upstream-signal`, no conflict. 51 files in `ui/src/`:

| Module | Does |
|---|---|
| `styles/{app,tokens}.css`, `scale-exceptions.ts` | the eight sizes on tokens, the header rules; dead rules and `--roadmap-status` out |
| `styles/{scale,tokens}.test.ts`, `flow/styles.test.ts`, `ui/primitives.test.ts` | G3, DB3, DB6 to DB8 |
| `health/*`, `graph/*`, `roadmap/*`, `tasks/*` | `verdictLozenge`, `graphFactsOf`, `itemStatusLozenge`, `CHAIN_TASKS_SHOWN`, the task pane, `taskStatusLozenge`; Buttons, PropertyLists, EmptyStates |
| `palette/Palette.tsx`, `board/{BoardView,CardPanel}.tsx` | Close a Button; the Board's three EmptyStates |
| `round/{drafts.ts,RoundScreen.tsx}` | `answerOf(item, draft, as)`, fed `actingName(acting)` |
| `ui/EmptyState.tsx`, `i18n/{en,ru}.ts`, `api/provisional.ts` | `titleId?`, `titleLang?`; the keys; the `StageBody` comment |

**Exceptions**: 10 before, `.sr-only` and `.tab` kept and, until RM-0099, the four icon offsets, `.palette`'s margin, `.graph-box`'s line height, `.graph-edge-label`'s size and line height; 2 after. `ICON_ALIGNMENT` and `GRAPH_LAYOUT` are gone.

**Departures**, accepted by the review:
1. DB6 reads `BoardView.tsx` for the old empty states only.
2. The task list's filter chips stay raw, as Health's.
3. `EmptyState` gained `titleId` (Graph's REF field, described by its heading) and `titleLang` (the card view's no-such-card title, the daemon's reason).
4. `tasks.none` reads "Create one: {{command}}", `tasks.noneYet` the title.
5. The task list has an sr-only `h2` over its `h3` empty states.
6. Graph's facts fold: a live holder shows "More fields (1)", Flags.
7. The Roadmap status lozenge shows the raw word, no prefix.
8. The verdict and task-state lozenges keep an sr-only "Verdict: " / "State: ".
9. A task's re-read alert moved from its head to the top of main.
10. Open levels: the Board notices `h2`, the card view `h3`, no spec change `h2`.
11. The `StageBody` comment said Questions sends no `as`.

**Iteration 2**: that comment fixed; `--roadmap-status` removed, pinned absent; a Chain box's tasks one line, `CHAIN_TASKS_SHOWN` = 2 cells then "N more", the twin test matching each card's first two and cells plus count to its total; the waiting note in `--text-primary`; the status lozenge with an sr-only "Status: " and no `lang` (plan content); the Questions hub test waits for `getTeam`.

**Numbers**: worktree tests 185 files, 5,307 before; 186, 5,359 after iteration 1 and 5,350 after 2, green twice each; every DB mutation red, 6 more in iteration 2. First load 827.44 kB in 21 files, gzip 239.11 (part a 825.27, 237.06): the entry 7.6 kB smaller, EmptyState, Lozenge, Badge and ActOff split out and preloaded. `ru-*.js` 153.67 / 35.27 kB, CSS 144.41 kB. On `main` with RM-0105: 831.15 kB, `ru-*.js` 154.72 / 35.49 kB.

**Open**: with two holders Graph draws two regions named "Details"; `RoadmapView.test.tsx` pins the tabs, so `delivery-analytics` edits it; the Chain's cut label overlap.
