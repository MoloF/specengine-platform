---
class: spec
status: draft
scope: [ui]
ref: RM-0099 part b, v5's UI lane, its third slice after `canvas-clicks` and `design-sweep` (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (part b, the census, the exceptions table, R7, R8, DB-1 to DB-8; here DB1 to DB14); `design-sweep`'s decisions (the pane, one status table, EmptyState levels); the orchestrator's working answers, the owner's review open
adrs: []
---

# design-sweep-b: Health, Graph, Roadmap and Agent tasks on the primitives, the exceptions down to two, RM-0099 closed (RM-0099 b)

## Why

`design-sweep` (RM-0099 a, `docs/features/design-sweep.md`) renamed every legacy size and weight, turned guards G1 and G2 on, and put Inbox, Questions and the Spec tree on the primitives. Four screens still draw their own buttons, empty states and fact lists: Health, Graph, Roadmap and Agent tasks. Eight raw sizes still stand in `SCALE_EXCEPTIONS` under `until: "RM-0099"`. Four header-height constants still assume the old 3.5rem header, which no guard reads. Two canon lines still name RM-0099: the Questions screen sends no `as` and links no card (`card-questions.md`, `round-ui.md`).

This slice is RM-0099's second half, and it closes the item. It puts the four screens on the primitives: Agent tasks gets part a's pane, Health's verdict becomes a Lozenge, Graph's details become a PropertyList with Buttons, and Roadmap gets an EmptyState and a neutral status. It also ends the eight exceptions (G3: the list keeps `.sr-only` and `.tab`) and derives the header heights from `--nav-height` (R8). It checks Graph's box line height against `BOX_HEIGHT` (R7) and makes the Questions screen send `as`. It is UI only: no crate, no plugin version, no hook or CI change. No ADR: it rests on the same decisions as part a (ADR-0011, ADR-0072 with ADR-0024, ADR-0031, ADR-0008, ADR-0076, ADR-0012) and on ADR-0047 (what is measured as zero is not empty).

## Description and interactions

### Decisions

- **D1. Lozenge marks status in a head and a verdict; Badge stays on rows** (part a's D4). New Lozenges: Health's check verdict, an Agent task's status in its head, and a Roadmap item's status on its lane card (neutral). Every row Badge stays: findings, severity, sync, link state, a list row's task state, Roadmap's marks.
- **D2. Health.** Its buttons become `Button`s, the verdict a `Lozenge`, the check counts a `PropertyList`, and the Findings' empty states `EmptyState`s. The rows of "What is left" keep their measured-zero lines. A zero there is a measurement, and an EmptyState would read as nothing measured (ADR-0047).
- **D3. Graph.** The details' facts become a `PropertyList`, and its acts take `Button` or `buttonClasses`. Its two empty states become `EmptyState`s. The boxes take step line heights that fit `BOX_HEIGHT` exactly (R7), and the edge labels take the small pair.
- **D4. Roadmap, its structure kept.** Read again, Retry and the lane jumps become `Button`s. The item status is a neutral `Lozenge`, and "no item" is an `EmptyState`. `RoadmapView.tsx`'s tabs, `RoadmapTab`, its `Suspense` and its lazy imports are not touched, so `delivery-analytics`' Delivery tab (core slice 6) lands in either order.
- **D5. Agent tasks is a pane.** The task is an `IssueView layout="pane"`, as Inbox's proposal and the Spec tree's node are (part a's D3). It has Breadcrumbs, a status Lozenge, `OwnerCommands` as its acts and its facts as a `PropertyList`. Its empty states become `EmptyState`s.
- **D6. The palette and the sync line take tokens.** The four icon offsets become `--space-025`, and the palette's top margin becomes `--space-600`. The palette's Close becomes a `Button`.
- **D7. Guard G3.** `SCALE_EXCEPTIONS` is exactly `.sr-only` and `.tab`, both kept, and the type names no item.
- **D8. Header heights come from `--nav-height`** (R8).
- **D9. The old empty states end.** `.empty-state` and `.empty-filter` leave `app.css` with their last users. Those users include the Board's three notices, which take `EmptyState` and nothing else.
- **D10. Questions sends `as`.** Each staged answer carries the acting name, as the Inbox's decision dialog does. The card link is re-pointed, not built: `RoundItem` holds no `card_id`, which is a core key.
- **D11. Nothing else moves.** No new data, endpoint or `provisional.ts` edit. No colour changes, and no canvas layout, minimap or zoom changes. The Overview is untouched, since RM-0086 replaces it.

## Data

### Health (`ui/src/health/`)

- **The verdict** (`CheckRegion.tsx`). `labels.ts` gains `verdictLozenge(value: CheckVerdict, t): LozengeLook & {lang?: "en"}` from the `VERDICT` table:
  - clean is `success` and observed is `warning`;
  - fails is `danger`, and cannot-check is `discovery`;
  - each keeps today's icon;
  - an unknown value is `neutral`, icon `unknown`, raw, under `lang="en"`.

  It replaces the Badge in `p.check-verdict`, and cannot-check never takes clean's appearance or icon. The "checked again" announcement keeps `verdictLook(…).label`.
- **Buttons.** Check again is a default `Button` that keeps `aria-disabled` while fetching, and so are `AmendmentsPart.tsx`'s and `codeRows.tsx`'s two raw buttons.
- **The counts.** `dl.check-counts` becomes a `PropertyList` labelled `health.check.counts`. It has one row per count, in today's order, keyed by the count's key. A cannot-check report keeps `health.check.countsCannot` above it.
- **Findings** (`FindingsRegion.tsx`). No finding is an `EmptyState` with today's heading and line. A filter with no match is an `EmptyState` with its line, and Clear filter as its primary. Each is one level below its region's heading.

### Graph (`ui/src/graph/`, `app.css`)

- **Details** (`GraphDetails.tsx`):
  - Each holder's `dl.facts` becomes a `PropertyList` labelled `graph.details.holder`. Its rows come from `graphFactsOf(holder, t): Property[]`, in today's order, `null` where today draws nothing.
  - The two links (to the Spec tree, and Graph from this node) take `buttonClasses("subtle", "compact")`.
  - The stub's act is a `Button`, and Close is `Button appearance="subtle"` with its `sr-only` part kept.
  - The edge lines and the marks stay.
- **Empty states** (`GraphView.tsx`):
  - No REF is an `EmptyState` level 2: `graph.empty`, with the REF forms and `graph.emptyNext` as its description.
  - No edge is an `EmptyState` level 2: `graph.noEdge`, with today's lines as its description.
  - The no-edge state has three conditional acts: Show impact, Follow mentions, Include archive. The first one shown is `primary`, the second `secondary`. A third stays reachable as the control it already is in `GraphControls` (the mode, the type chips, the archive).
- **Show** (`GraphControls.tsx`): `Button appearance="primary" type="submit"`.
- **Boxes (R7).**
  - `.graph-box` takes `line-height: var(--line-height-body)` (20 px; was 1.3 × 14 = 18.2), and `.graph-box-meta` takes `var(--line-height-small)` (16 px).
  - A selected box (`border-box`) holds its 2 px border twice (4), its padding of 4 px twice (8), two gaps of 2 px (4), and the three rows (20 + 20 + 16 = 56). That makes 72 = `BOX_HEIGHT` (`graph/layout.ts`), with no slack.
  - Roadmap's chain draws the same box.
- **Edge labels.** `.graph-edge-label` takes `font-size: var(--font-size-small)` (12 px; was 11) and `line-height: var(--line-height-small)` (16 px; was 16.5).

### Roadmap (`ui/src/roadmap/`)

- **Buttons.** Read again is a `Button` with `icon="retry"`. The failed re-read's Retry is a `Button`. `RoadmapLanes.tsx`'s two raw buttons are `Button appearance="subtle" spacing="compact"`.
- **The item's status** on a lane card is a `Lozenge`: `text.ts`'s `itemStatusLozenge(value)`, `neutral`, with today's icon, the value raw under `lang="en"`. No plan word is mapped (`ui_policy.rs` bans item-status literals). The other marks stay Badges: Waits on, In a cycle, Current, MVP cut, unestimated, and its tasks' states.
- **No item.** `emptyPlanText`'s no-item case becomes an `EmptyState` level 2 where the line stood. Its title is today's line, and its description is `roadmap.emptyNext`. "All done" stays its line. The tabs stand below it, as today.

### Agent tasks (`ui/src/tasks/`)

- **The task** (`TaskDetail.tsx`). The `article.task-pane`, with its `aria-labelledby`, `aria-busy` and key handler, wraps an `IssueView layout="pane"`. As part a defines it, the side comes before main in the DOM, and two columns start at 60rem of the pane's own width.
  - Breadcrumbs:
    - Project: `homeHash(project)`, the project's name, else the slug;
    - Agent tasks: `sectionHash(project, "tasks")`, `app.sections.tasks.label`;
    - the `T-` ID, current, `mono`.
  - Title: today's `h1` (the ID, then the title or `tasks.untitled`), its `id`, `ref` and `tabIndex={-1}` kept.
  - Meta: the status `Lozenge`, then today's spec-changed Badge and frozen note.
  - Actions: `OwnerCommands`.
  - Side: a `PropertyList` labelled `tasks.detail.details`, with Created and Updated (today's `dl.facts`).
  - Main: the notes, the waiting note, the version notice, the tabs and their panels.
- **The status lozenge** (`labels.ts`). `taskStatusLozenge(value: TaskStatus, t): LozengeLook & {lang?: "en"}` uses `taskStatusLook`'s labels and icons:
  - draft and analysis are neutral;
  - review and changes requested are warning;
  - ready, in progress and in review are information;
  - done and accepted are success;
  - cancelled is neutral;
  - an unknown value is neutral, icon `unknown`, raw, under `lang="en"`.

  The list's rows, the Overview and a card's Tasks part keep the Badge.
- **Empty states.**
  - No task chosen (`TasksView.tsx`) is level 2.
  - No such task (`TaskDetail.tsx`) is level 2, with the reason verbatim under `lang="en"` and the back link as its description.
  - An empty list (`TaskList.tsx`) is level 3.
  - A filter with no match is level 3, with Clear filter as its primary.
  - No spec change (`SpecChangesPanel.tsx`) is one level below its panel's heading.
- **Buttons.** `TaskList.tsx`'s three become `Button`s, and so does any other raw `className="button…"` in `src/tasks/`.

### The palette, the sync line, the icons

- **Icon offsets.** `.waiting-note .icon`, `.sync-part .icon`, `.cut-note .icon` and `.palette-error .icon` take `margin-top: var(--space-025)`. A 16 px icon on a 20 px body line then sits 2 px down, centred on the first line (it was 0.15-0.2em, 2.1-2.8 px).
- **The palette.** `.palette` takes `margin-top: var(--space-600)` (48 px; was `min(10vh, 5rem)`, 80 px at 900 px tall). Its Close (`Palette.tsx`) is `Button appearance="subtle"`.
- **The sync line** draws no button, so it only gives up its exception.

### The exceptions and guard G3 (`ui/src/styles/`)

`SCALE_EXCEPTIONS` keeps two entries, `.sr-only`'s and `.tab`'s, both `until: null`.
- `ScaleException.until` becomes `null`, and the header comment drops "the plan item that removes it".
- `ICON_ALIGNMENT` and `GRAPH_LAYOUT` go.
- In `scale.test.ts`, the exception test (about lines 399-402) takes `toHaveLength(2)`, the `until` set `{null}`, and the source match `until: null;`. "Lists no exception that matches nothing" stands.
- A new entry is a kept one only: `until: null` with its reason.

### Header heights (R8, `app.css`)

| Rule | Was | Becomes |
|---|---|---|
| `.app-body` `min-height` | `calc(100vh - 3.5rem)` | `calc(100vh - var(--nav-height))` |
| `.tree-pane`, `.tasks-pane` `top` | `4.5rem` | `calc(var(--nav-height) + var(--space-200))` |
| `.tree-pane`, `.tasks-pane` `max-height` | `calc(100vh - 5.5rem)` | `calc(100vh - var(--nav-height) - var(--space-400))` |
| `.task-column h1` `scroll-margin-top`, under 48em | `7rem` | `calc(2 * var(--nav-height) + var(--space-100))` |

`--nav-height` is 48 px, so the two panes stick 8 px higher and reach 8 px further down. `.app-body:has(.flow-studio)`'s `min-height: 0` stays. `flow/styles.test.ts` (about line 108) pins the new value.

### The old empty states

`.empty-state` and `.empty-filter`, with every rule under them, leave `app.css` once nothing renders them. Their last users are the four screens above and three on the Board. The Board's three take `EmptyState` and nothing else:
- `BoardView.tsx`'s no-task notice;
- its no-match notice, with Clear filters as its primary;
- `CardPanel.tsx`'s empty state.

A `board-notice` wrapper stays if its layout needs it.

### Questions (`ui/src/round/`)

- **`as`.** `drafts.ts`'s `answerOf(item, draft, as: string | null)` adds `as` to the stage when it is not `null`. `RoundScreen.tsx` passes `actingName(acting)` from `useActing()` (`board/acting.ts`): the stored "Acting as" name, or a hub's login. The Inbox's dialog does the same. Nothing new shows on screen. `null` sends no key, and Unstage sends none.
- **No card link.** `RoundItem` holds no `card_id`. Adding one is a core key and a key-set step, which this UI-only slice does not take ("Canon at ship").
- **Titles over IDs.** RM-0088 sent the Questions screen's "titles over IDs" to RM-0099. `plain-questions` (RM-0088's question half) restructures the round item's head to lead with the gist, so this slice leaves that head to it.

### Strings

Three new keys, in `en.ts` and `ru.ts` (the Russian a draft for the owner):
- `graph.details.holder` "Details";
- `tasks.detail.details` "Details";
- `roadmap.emptyNext` "A plan item is a document your [roadmap] table selects; Read again shows it once it is saved."

Every other string reuses an existing key. A key whose last user goes leaves both catalogs.

## Rules and edge cases

- WHEN a verdict is cannot-check, its Lozenge SHALL be `discovery` with the `cannotCheck` icon. It SHALL never take clean's appearance or icon. An unknown verdict SHALL be neutral and raw, under `lang="en"`.
- WHEN a Graph or chain box is selected and shows its three rows, they SHALL fit `BOX_HEIGHT` with no overflow.
- WHEN an empty state has more than two acts, it SHALL hold two (the first `primary`), and the rest SHALL stay reachable as controls.
- WHEN an item, a dialog or a view's header shows acts, at most one SHALL be primary (`ui-design.md` "Primitives").
- WHEN the acting name is empty, a staged answer SHALL send no `as`. Otherwise it SHALL send the trimmed name. Unstage SHALL send none.
- Agent tasks' list keys, the heading's focus on selection, `useRetryFocus`, Graph's roving focus and its `:focus-visible` gate, and the Chain's clicks (`canvas-clicks`) SHALL behave as today.
- A Roadmap read SHALL draw the same tabs, in the same order, as before this slice.
- A row of "What is left" measured at zero SHALL keep "None: …" and "Measured over …", never an EmptyState.

### Test churn

- Health, Graph, Roadmap and Agent tasks tests that read `.empty-state`, `.empty-filter`, `dl.facts` or the verdict's Badge read the primitives instead.
- `flow/styles.test.ts` (about line 108), `scale.test.ts` (about lines 399-402), and Board tests that find `.empty-state.board-notice`.
- `round/drafts.test.ts` (`answerOf`'s third argument) and `QuestionsView.test.tsx` (the POST body).
- Tests that find a raw `button` by class still find `Button`'s `button` class.

## Acceptance criteria

A browser look is marked **[look]**, a counted run **[measure]**. Every other criterion is a test that the named mutation turns red. DB1 to DB5 follow the analysis's DB-1 to DB-4, Graph split in two. From DB6 on they are renumbered.

- [ ] DB1 — **Health.**
  - Check again is a `Button`.
  - The verdict is a `Lozenge`: `cannot_check` gives `discovery` with the `cannotCheck` icon, `clean` gives `success`, and `"weird"` gives neutral, raw, `lang="en"`.
  - The counts are a `PropertyList`.
  - No finding, and a filter with no match, are `.empty-state-box`es, and Clear filter is the second one's primary.
  - A row measured at zero still shows "None: …".
  - Mutations, each red: cannot-check mapped to `success`; the counts back in `dl.check-counts`; a zero row as an `EmptyState`.
- [ ] DB2 — **Graph's details and empty states.**
  - A holder's facts are a `PropertyList` with the rows of `graphFactsOf`.
  - The two links carry `button button-quiet button-compact`, and Close is a `Button`.
  - No REF and no edge are `.empty-state-box`es. With impact off, mentions offered and archive off, the no-edge state shows exactly two acts, Show impact `button-primary`, and the type chips and the archive toggle still stand in `GraphControls`.
  - Mutations, each red: the old `dl.facts` markup; three acts in the empty state.
- [ ] DB3 — **Graph's boxes fit (R7).** A source test reads `app.css` and `tokens.css` and computes a selected box's content height from the tokens (border, padding, two gaps, body, body, small). The result is at most `BOX_HEIGHT`, imported from `graph/layout.ts`. `.graph-edge-label` takes the small pair.
  - Mutations, each red: `.graph-box` at `var(--line-height-document)`; `.graph-box-meta` with no line height of its own; `BOX_HEIGHT` set to 64.
- [ ] DB4 — **Roadmap.**
  - An empty plan is an `EmptyState` with `roadmap.emptyNext`, and the tabs follow it.
  - A lane card's status is a neutral `Lozenge` with the value raw.
  - Read again is a `Button`, and the tabs are the ones before this slice, in order.
  - Mutations, each red: a status coloured by its value (`"done"` as success); the empty plan as a `<p>`.
- [ ] DB5 — **Agent tasks.** A task shows:
  - Breadcrumbs: Project, Agent tasks, its `T-` ID current, with `aria-current="page"`;
  - the pane: `issue-view-pane`, with side before main;
  - a status `Lozenge` from `taskStatusLozenge` (`review` is warning, `"weird"` neutral and raw);
  - Created and Updated in a `PropertyList`.

  No task chosen, no such task, an empty list and a no-match filter are `EmptyState`s. The list keys, the heading's focus and Retry's focus stay green.
  - Mutations, each red: `div.empty-state` restored in `TasksView.tsx`; no Breadcrumbs; the status back as a Badge in the head.
- [ ] DB6 — **Primitives in the swept directories.** A source test reads the non-test `.tsx` of `src/{health,graph,roadmap,tasks,palette}/`, `src/app/SyncLine.tsx`, `src/board/BoardView.tsx` and `src/board/CardPanel.tsx`. It finds no `className="button`, no `"button button-` literal, and no `"empty-state"` or `"empty-filter"` class. `app.css` holds no `.empty-state` or `.empty-filter` selector.
  - Mutations, each red: a raw `<button className="button">` in `CheckRegion.tsx`; `.empty-filter` left in `app.css`.
- [ ] DB7 — **G3.**
  - `SCALE_EXCEPTIONS` is exactly `.sr-only` and `.tab`, both `until: null`, and its type reads `until: null;`.
  - The four icon offsets take `var(--space-025)`, `.palette` takes `var(--space-600)`, and the edge label takes the small pair.
  - Mutations, each red: `.graph-edge-label` back at `0.6875rem`; `.palette` back at `min(10vh, 5rem)`.
- [ ] DB8 — **Header heights.** A source test finds the four rules of "Header heights" each naming `var(--nav-height)`, and no `3.5rem`, `4.5rem`, `5.5rem` or `7rem` in a `top`, `min-height`, `max-height` or `scroll-margin-top` declaration of `app.css`.
  - Mutation, red: `.app-body` back at `calc(100vh - 3.5rem)`.
- [ ] DB9 — **Questions sends `as`.**
  - With "Acting as" holding `Ann`, Stage posts each answer's stage with `"as": "Ann"`.
  - Empty, it sends no `as` key.
  - With a hub login, it sends the login.
  - Unstage's DELETE sends no body.
  - Mutations, each red: `as` dropped from `answerOf`; `as: null` sent when empty.
- [ ] DB10 — **Strings.**
  - The three new keys are in `en.ts` and `ru.ts`, and a key with no user is in neither.
  - `catalog.test.ts` and `words.test.ts` pass.
  - Mutations, each red: `tasks.detail.details` missing in `ru.ts`; "card" in an `en` string of this slice.
- [ ] DB11 — **[measure] Size.** After `pnpm build`, first-load JS is at most 845 kB. It is recorded with gzip and `ru-*.js` (raw and gzip) against `design-sweep`'s figure, and its change explained.
- [ ] DB12 — **[look]** The orchestrator's, in Chrome, in both themes and both languages, with RM-0093's DevTools measure (`design-foundation.md` DF-10), at 1440x900 and 1100x800, and 1920x1080 for the pane's two columns:
  - Health: clean, observed, fails, cannot-check (`?scenario=cannot-check`), no finding, a filter with no match;
  - Graph: details of one and of two holders, no REF, no edge; boxes at their longest names, focused and selected, inside their borders; edge labels;
  - Roadmap: lanes with statuses, the Chain's boxes, an empty plan;
  - Agent tasks: a task with every tab, no task chosen, an unknown task, at 1440 (one column) and 1920 (two);
  - the palette's place and an error, the sync line on a hub, and the waiting and cut notes' icons on their first line;
  - the Spec tree and Agent tasks panes sticking under the header; the Board's three empty states.
- [ ] DB13 — **RM-0099 is closed.**
  - `grep -rn "RM-0099" ui/src` finds only test data (`RoadmapView.test.tsx`'s ID).
  - After "Canon at ship" (and part a's), `grep -rn "RM-0099" docs/canon` finds nothing.
  - `RM-0099.md` reads `status: shipped`.
- [ ] DB14 — **Gates.** UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green. The eval tests (`ui_policy`, `anonymity`, `doc_pointers`) and `spec check` pass.

## Out of scope

- The Overview: `department-overview` (RM-0086) replaces it.
- Primitives on Flow, the document view and the Board, but for the Board's three empty states (D9).
- The shell's raw buttons (`App.tsx`, `RegionBoundary.tsx`, `Shell.tsx`, `shortcuts.tsx`, `views.tsx`, `WorkspaceControls.tsx`). They render the class the primitive does, and no guard is added over them.
- A card link on the Questions sheet (`card_id` on `RoundItem`, a core key), and the round item's head (`plain-questions`).
- New data or endpoints, any Rust, `provisional.ts`, colours, the canvases' layout, minimaps and zoom floors, the filter chips' design, and `RoadmapView`'s tabs.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| ui-developer | `ui/src/styles/app.css`, `scale-exceptions.ts`; `ui/src/{health,graph,roadmap,tasks,palette}/**`, `ui/src/app/SyncLine.tsx`, `ui/src/board/{BoardView,CardPanel}.tsx`, `ui/src/round/{drafts.ts,RoundScreen.tsx}`; `ui/src/i18n/{en,ru}.ts`; the tests of DB1-DB6, DB9 and "Test churn" |
| test-engineer, UI | G3 and DB3 in `styles/scale.test.ts`, DB8 in `flow/styles.test.ts` or a new styles test, DB6's source test; the gates and every DB mutation; DB11's measure |

Comments cite `docs/features/design-sweep-b.md` "Acceptance criteria" with a criterion ID, never "Data", since compaction drops it at ship (`doc_pointers`).

### Work order

1. Start once `design-sweep` (part a) has merged onto `main`, since both edit `app.css`, the scale files and both catalogs.
2. The ui-developer does the CSS first, alone: the eight exceptions, the boxes, the header heights. The test-engineer lands G3, DB3 and DB8 on it, and every gate is green before any screen moves.
3. Then the screens, each one's tests green before the next: Health, Graph, Roadmap, Agent tasks, the palette, the Board's three notices, Questions.
4. The test-engineer runs the gates and the mutations, then DB11. Then the UI review.
5. The orchestrator's look (DB12). Ship: the spec-writer ("Canon at ship").

It runs beside the core lane in the same tree and shares only documents with it. The owner commits each slice on its own. At most three iterations.

### Risks

- **Agent tasks restructured** (R9). The pane churns the list-and-detail tests (keys, focus, Retry), which is the main iteration-cap risk. The CSS lands first, apart from it.
- **Zero slack in the boxes.** A font with a taller line box, or a wrapped meta row, overflows by pixels. DB3 proves the arithmetic, and only DB12 proves the drawing.
- **Shared ground.** `delivery-analytics` edits `RoadmapView.tsx` and `app.css`, and `plain-questions` edits `round/`. Whichever lands second rebases. D4 and D10 keep the overlap to separate lines.
- **`ui/README.md`** is at 8 191 of 8 192 B, and nothing here changes its rules, so it is untouched.

### Canon at ship

Free bytes now are in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/ui-design.md` [15] | "Guards and exceptions": two, both kept, `until: null`. "The work item": the pane on Agent tasks too. "Known limits": the `.app-body` line goes; sticky panes assume a one-row header, which may wrap under 68.75rem. Net not positive |
| `docs/canon/card-questions.md` [225] | "Known limits": "The Questions screen links no card: `RoundItem` holds no `card_id`." |
| `docs/canon/round-ui.md` | "Screen": each stage sends the acting name as `as` (`board-screen.md` "The acting name"), a hub's login. Net zero |
| `docs/canon/spec-roadmap-screen.md` [9 161] | "Layout": the status a neutral Lozenge. "States and live": no item an EmptyState |
| `docs/roadmap/RM-0088.md` | its line sending the Questions screen's titles over IDs to RM-0099 points at its question half |
| `docs/roadmap/RM-0099.md`, `CLAUDE.md`, 08 s2 Phase 7 "Owner's word, 2026-11-12" (the UI lane) | `status: shipped`, `shipped:`; RM-0099 closed |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q-C (part a's). Lozenge or Badge? | Lozenge for an item's status in its head and for a check verdict; Badge stays on rows | Every Badge a Lozenge maps about 60 tone roles onto six appearances, and the canon's distinctions blur |
| Q-E. The palette and the sync line? | Tokens only: the four icon offsets `--space-025`, the palette's offset `--space-600` | A size token for the palette keeps its `10vh` behaviour for one more name in `tokens.css` |

**The spec-writer's:**
- W1: "What is left" keeps its zero lines. A measured zero is not an empty state (ADR-0047), so DB1 guards the line.
- W2: Graph's no-edge state keeps two acts, as `EmptyState` allows. The third act is already a control above the canvas.
- W3: Agent tasks takes part a's pane, so its one column holds below 60rem of the pane's own width, as Inbox's and the tree's do.
- W4: the card link on the Questions screen needs a core key, so it is re-pointed rather than built. The `as` is UI only and is built. If `plain-questions`, which already grows `RoundItem`, takes `card_id`, the limit closes there.
- W5: the Board gives up only its three empty states, so the two old rules can leave `app.css`.
- W6: the edge labels grow 1 px (11 to 12): the small step is the nearest, and the labels draw above the edges, which have room.
