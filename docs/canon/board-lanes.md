---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-14
---

# Board lanes and the Show level: how a board groups and cuts its cards

The Board's two selects beside Discipline and Assignee (`board-screen.md` "Flows, filters, lanes"). Lanes moved here from `board-screen.md` at its cap; Show built by `board-subtasks` (RM-0114), no ADR, its words the owner's "task" for a card (ADR-0076). Code: UI `src/board/swimlanes.ts`, `level.ts`, `BoardView.tsx`, `BoardCard.tsx`. Criteria: `docs/features/board-subtasks.md`.

## Lanes

A select, None (the default), Assignee or Parent, in `localStorage` `specengine.board.lanes.<p>`; unknown or missing reads None. Assignee lanes by name folded with `normalize("NFC").toLowerCase()`, ties by the raw name, then Unassigned. Parent lanes by the parent ID's number, each head the parent's ID (a link to its modal), title, flow title and status lozenge, a hidden flow's too, then Everything else. Unassigned and Everything else stand only when they hold a card. A same-flow parent is both a head and a tile. A head spans the grid, label sticky at the left. A lane collapses (`aria-expanded`) to its cells' counts, no tile, and stays collapsed through re-reads for the visit (Shell screen state).

## Show

A select between Assignee and Lanes, with Lanes' look: **All tasks** (`all`, the default, the board as before), **Tasks only** (`tasks`: rows whose `parent` is null), **Subtasks only** (`subtasks`: rows with a `parent`, at any depth). Per viewer and project in `localStorage` `specengine.board.level.<p>` (`level.ts` `recallLevel`, `rememberLevel`): unknown, missing or a refusing storage reads `all`, a refused write keeps the choice for the page. Never in the URL or the Shell's screen state.

- **One more cut** (`matchesLevel`, `parent` alone), ANDed with Discipline and Assignee where the rows shown are cut: every flow tab, Show cancelled's rows, every lane mode. `getCards` is unchanged and sends no `parent`.
- **Heads and counts.** Column heads come from every standing row of the flow, so none moves with the level. A column's, a lane's and a collapsed lane's counts are of the rows shown; a parent's "<n> subtasks" rollup and Returned count stay the whole list's.
- **The parent line**, under Subtasks only: after the source, "Subtask of <link>", the parent's ID and title from the whole list, any flow, else its ID alone, to `#/<p>/board/<parent>`, above the stretched title link. The link is out of the Tab order, as the source link; the tile's accessible name says the parent. It opens the parent's modal as a tile does, one history entry; closing it returns focus to the subtask's tile. All tasks draws no parent line.
- **With Lanes.** Parent lanes under Subtasks only: one lane per parent, no Everything else. Under Tasks only: Everything else alone. Assignee lanes cut the rows shown.
- **Empty.** With Discipline or Assignee set: "No task matches the filters." and Clear filters, which leaves the level. Else Tasks only says "No task without a parent here.", Subtasks only "No subtask here.", each with the primary "Show all tasks", which sets and stores `all`. No card at all: the board's empty state.
- **Routes and writes.** A URL naming a hidden card opens its modal (`getCard`; closing focuses the board's heading); a route never changes the level. A card written while hidden lets focus go and the polite region says so. A drop never sets or clears a parent. The tab stop and the arrows walk drawn tiles only.
- **Words.** `board.level.*`, eight keys in both catalogs; none built on "accept" or its Russian stem (`words.test.ts`); `src/board/` branches on no column, flow, discipline or event word (`policy.test.ts`).

## Tests

UI `src/board/level.test.ts` (BS1, BS2), `BoardView.test.tsx` (BS3-BS10), `swimlanes.test.ts`, `words.test.ts`, `policy.test.ts`.
