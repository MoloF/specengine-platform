---
class: spec
status: shipped
shipped: 2026-11-14
scope: [ui]
ref: RM-0114; the owner's report of 2026-11-14 from the hosted hub (0.46.0, project `dorm`), written in Russian, paraphrased; patch release 0.46.1
adrs: []
---

# board-subtasks: the Board shows all tasks, tasks only or subtasks only (RM-0114)

## Why

The owner, 2026-11-14, using the hosted hub: the Board has no filter that shows only top-level tasks, without their subtasks, and none for the opposite, every subtask, as Jira has. A split (`docs/canon/card-split.md` "The rule") makes a parent task and one subtask per department, so a department's board mixes both levels. After a few splits, neither level can be read on its own.

## Description and interactions

A **Show** select between Assignee and Lanes picks All tasks, Tasks only or Subtasks only, per viewer and project: one more cut of the Board's one list, ANDed with Discipline and Assignee, with a "Subtask of <parent>" line on each subtask's tile. Column heads never move; counts follow the level, a parent's rollup does not. Canon: `docs/canon/board-lanes.md` "Show", where the Lanes rules moved too (`board-screen.md` was at its cap).

## Data

No daemon change: every `CardRow` already carries `parent`. `localStorage` `specengine.board.level.<p>`: `all`, `tasks`, `subtasks`; anything else reads `all`. Module `src/board/level.ts`: `BoardLevel`, `BOARD_LEVELS`, `levelKey`, `recallLevel`, `rememberLevel`, `matchesLevel`. Eight catalog keys `board.level.*` in both languages: `label` "Show", `all` "All tasks", `tasks` "Tasks only", `subtasks` "Subtasks only", `parentOf` "Subtask of {{parent}}" (a `fill` slot), `noTasks` "No task without a parent here.", `noSubtasks` "No subtask here.", `showAll` "Show all tasks".

## Rules and edge cases

`board-lanes.md` "Show". In short: under `tasks` no parented row is drawn and a parent's rollup still counts every child; under `subtasks` only parented rows, each naming its parent with a link to its modal; `all` is the board as before, no parent line. The level reads `parent` alone (`policy.test.ts`); no word of it is built on "accept" or its Russian stem (`words.test.ts`).

## Acceptance criteria

All but BS12 are vitest under `ui/`. The fixture list: in one flow, parent C-0001 "Weekly run" with subtasks C-0002 and C-0003, lone task C-0004, and subtask C-0005 whose parent C-0009 the list does not hold. Evidence: the tests citing each ID; the UI review, ACCEPT after iteration 2; the whole UI suite (231 files, 6 212 tests), lint, typecheck and the build green, first load 842.92 kB (gzip 247.73 kB) within 845 kB.

- [x] BS1 — **The predicate** (`src/board/level.test.ts`). `matchesLevel` keeps all five rows for `all`, C-0001 and C-0004 for `tasks`, and C-0002, C-0003, C-0005 for `subtasks`. Mutations: `tasks` and `subtasks` swapped; `all` dropping parented rows.
- [x] BS2 — **Storage** (`level.test.ts`). Nothing stored reads `all`; `subtasks` reads `subtasks`; `SUBTASKS`, `""` and `parent` read `all`. A `localStorage` whose `getItem` and `setItem` throw: `recallLevel` gives `all`, `rememberLevel` does not throw. `rememberLevel("harbor", "tasks")` writes exactly `specengine.board.level.harbor` and leaves `specengine.board.lanes.harbor`. Mutation: the lanes key reused.
- [x] BS3 — **Tasks only** (`BoardView.test.tsx`). Only C-0001 and C-0004 are drawn, C-0001's tile reads "2 subtasks", and each head's count equals the tiles drawn in it. Mutations: rollups from the rows shown; head counts from the whole list.
- [x] BS4 — **Subtasks only.** C-0002, C-0003 and C-0005 are drawn, not C-0001 or C-0004. C-0002's tile holds "Subtask of" with a link to `#/<p>/board/C-0001` reading `C-0001` and `Weekly run`; C-0005's reads `C-0009` alone, to `#/<p>/board/C-0009`; clicking C-0002's opens C-0001's modal though it is not drawn. Mutations: the link omitted; its `href` the subtask's own ID.
- [x] BS5 — **All unchanged.** Under `all` no tile holds "Subtask of", and the tiles and head counts equal the render with no level stored. Mutation: the parent line drawn under `all`.
- [x] BS6 — **Composition.** Subtasks only with Parent lanes: lanes C-0001 and C-0009, no Everything else. Tasks only with Parent lanes: Everything else alone. Discipline set to C-0002's, Subtasks only: C-0002 alone. A cancelled subtask only with Show cancelled and Subtasks only both on. Mutation: lanes built from the unfiltered rows.
- [x] BS7 — **Persisted per project.** Choosing Subtasks only stores `subtasks` under `specengine.board.level.<p>`; a remount shows it selected and applied; another project's board reads `all`. Mutation: the key without the project.
- [x] BS8 — **Empty states.** No subtask under Subtasks only: "No subtask here." and "Show all tasks", whose click shows every tile and stores `all`. Tasks only over subtasks alone: "No task without a parent here." Discipline with no match: "No task matches the filters."; Clear filters keeps the level. Mutation: Clear filters resetting the level.
- [x] BS9 — **A routed hidden card.** Under Subtasks only, `#/<p>/board/C-0004` opens C-0004's modal; closing it focuses the board's heading, the select still Subtasks only. Mutation: the modal gated on the row being drawn.
- [x] BS10 — **Heads still.** A stranded subtask makes "Not in the flow" a head; the head row's labels and order are equal under `all`, `tasks` and `subtasks`. Mutation: heads from the rows shown.
- [x] BS11 — **Strings and gates.** The eight keys in both catalogs (`catalog.test.ts`), `policy.test.ts` and `words.test.ts` pass; the whole UI suite, lint, typecheck and the build pass, within the size budget.
- [x] BS12 — **[look]** The orchestrator's, 2026-11-14, Chrome at 1440x900 on the mock's split cards: the select stands between Assignee and Lanes, no column header moves, a parent line wraps cleanly. The toolbar wraps "Acting as" to a second row, a width it already had.

## Out of scope

A daemon, CLI or MCP change, a `parent` query; the level in the URL or the Shell's screen state; a parent line under All tasks; filtering by depth.

## Implementation

| Module | What it does |
|---|---|
| UI `src/board/level.ts` (new) | the level, its key, recall and remember, `matchesLevel` |
| UI `src/board/BoardView.tsx` | the Show select, the cut in `visible`, Parent lanes per level, the empty states, the routed card, focus back to the subtask's tile |
| UI `src/board/BoardCard.tsx` | `Parent`, the "Subtask of" line, the parent in the tile's accessible name |
| UI `src/i18n/en.ts`, `ru.ts`, `styles/app.css` | the eight keys, the line's style |
| tests | `level.test.ts`, `BoardView.test.tsx`, `words.test.ts` |

**Departures**, accepted: the owner's word "task" for a card (ADR-0076): "All tasks", "Show all tasks" for the spec's "All cards", "Show all cards"; closing a modal opened from a parent link returns focus to the subtask's tile; the parent link sits out of the Tab order, the parent said in the tile's name.

**Release.** The six slices RM-0114 to RM-0119 ship in one commit as the patch 0.46.1: a hub runs tagged images and `tag-check.sh` ties the tag to the plugin's version, so `plugin.json`, a `PINS` entry, `.env.example` and the tests pinning 0.46.0 moved to 0.46.1, with no plugin text or schema change (`docs/canon/plugin-files.md` "Changing it").
