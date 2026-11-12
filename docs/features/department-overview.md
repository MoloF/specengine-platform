---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0086, v5 core slice 11 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section RM-0086, DO1 to DO8; here DO1 to DO14); the orchestrator's working answers and ruling D1, the owner's review open
adrs: []
---

# department-overview: each department opens on its own numbers, counted by status category over its flows (RM-0086)

## Why

The Overview held two blocks, Tasks and Inbox, both copies of other pages. A department landing on it learned nothing about its own work: what is to do, in progress, done or returned in its flows, what waits on it, who handed what back; the lead could not see departments side by side. The numbers cannot come from the Board's list, which is cut at 40 000 characters.

Now `spec card counts [--json]` and the daemon's `GET …/cards/counts` serve counts no clock touches, per (flow, column) and per flow, and the Overview (`#/<p>`) folds them by status category over the chosen department's flows: Work, what waits on it, its flow's Gaps where its sections list Flow, every department side by side where its `flows` is null. No ADR: it rests on ADR-0073, ADR-0076, ADR-0047, ADR-0075, ADR-0074, ADR-0012 and ADR-0008. Drift and collisions are out of this cut.

## Description and interactions

Each decision is canon now, in `docs/canon/card-counts.md`: D1 the daemon counts, the UI folds, and D2 stored columns, no row decoded: "The rule", "Counts"; D3 Ready is `rollup.ts`'s `ready`: "Counts"; D4 no clock: "The rule", "Overview"; D5 blocks by config: "Overview"; D6 agent tasks local: "Daemon, hub"; D7 the old regions gone and D8 no MCP tool, plugin version or schema step: "Overview", "Daemon, hub".

The owner's questions, open, their working answers built: **Q1** drift and collisions out of the first cut; **Q2** the UI folds over per (flow, column) counts; **Q3** blocks by `sections` and `flows`, never a name; **Q4** a hub serves `null`, "local only"; **Q5** the Tasks and Inbox regions removed. The spec-writer's: W1 `open_subtasks` per column, the UI picking `return`; W2 IDs served, so a task linked from two flows counts once; W3 tasks of any repository count, no git (a Known limit); W4 the route under `spec card`, a bad `[card]` exit 2 and a 503; W5 `delivery-analytics` and this read share nothing.

## Data

In the canon: `card-counts.md` "Counts", "Text and JSON", "Daemon, hub". **The scenario**, the criteria's (CLI `tests/counts_scene/`, its `data.rs` the pinned JSON and text; UI `mocks/cardCounts.ts` `pinnedCounts`): flows `copy` (`todo`, `doing`, `review`, `returned`, `done`; accept `review` to `done`; return `returned`), `plates` (`todo`, `drawing`, `done`), `sound` (`todo`, `check`, `done`); departments `words` (sections `flow`, `board`, `questions`, `inbox`; flows `copy`), `print` (`board`, `health`, `inbox`; `copy`, `plates`), `press` (`inbox`, `board`, `roadmap`, `flow`; no `flows`). Cards C-0001 `copy`/`review` open, tasks `T-0001`; C-0002 `plates`/`done` accepted, parent C-0001; C-0003 `copy`/`todo` cancelled, parent C-0001; C-0004 `copy`/`returned` open, parent C-0005; C-0005 `plates`/`todo` open, tasks `T-0001`, `T-0002`; C-0006 `sound`/`todo` open; C-0007 `copy`/`proofing`, a column the config dropped, open. Open questions PR-0001 on C-0001 (2026-11-02), PR-0002 on C-0004 (11-05), PR-0004 on C-0003 (11-07); PR-0003 applied. T-0001 in `review`, T-0002 `ready`. Header: `card counts | flows 3 | cards 7: open 5, accepted 1, cancelled 1 | agent tasks counted`.

**Corrected at ship**, where the code ruled:
1. No `[card]` table counts under the default `plain` flow, as the Board, by D1 (the orchestrator's ruling); the draft's "no `[card]` table gives `flows` empty with every stored column stranded" was wrong.
2. `counts_document` takes `&[CardFlow]`, the type `list` serves; the draft's `CardFlowShape` does not exist.
3. A `tasks` does not read when it is no JSON list of text or an entry is no task ID, not only when it is no list; passed over whole, noted.
4. A `NULL` or non-text `flow` or `col` counts under its text (`NULL` for `NULL`) and is noted; the draft said nothing. `cards` is `STRICT`, so only `NULL` is reachable.
5. Six statements per call, `COUNT_UNNAMED` beside the draft's five.

## Rules and edge cases

In the canon: `card-counts.md` "The rule", "Known limits". **Rollout**: no schema step, backup format, config key or MCP tool; a daemon or hub before this slice answers 404 and the Overview shows its `ErrorPanel`; a hub takes it at its next upgrade, the owner's call.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Rows are written through the store with fixed `now`s. Corpora are built at run time with invented names. "The scenario" is "Data"'s.

- [x] DO1 — **Uncapped.** 700 cards with 120-character titles cut `spec card list --json` (`truncated` true). `spec card counts --json` still sums `open` + `accepted` + `cancelled` to 700, as the route does. Mutation, red: the counts taken from `card_list`'s rows.
- [x] DO2 — **The fold** (`department.test.ts`, over the scenario's document, `today` 2026-11-12):
  - `words`: to do 0, in progress 1, waiting for review 1, returned 1, done 0, not in the flow 1, cancelled 1, questions 3, oldest 10 days, returned subtasks 1, ready 1, agent tasks 1.
  - `print`: to do 1, done 1, the rest as `words`.
  - `press` and "All flows": to do 2, the rest as `print`.
  - An open card in `accept.to` is done.
  - Mutations, each red: `accept.to` counted in progress; an accepted card counted in its column's category; a cancelled card counted in its column's category.
- [x] DO3 — **No health, Flow only by config** (`HomeView.test.tsx`, stub client). For `print`, whose sections list `health`, `getHealth` and `getFlow` are called 0 times; for `words`, `getFlow` once, with the chosen module's root when it is rooted. Mutations, each red: a health read on the Overview; the flow read made whatever the sections.
- [x] DO4 — **The hub.** The hub's `GET …/cards/counts` answers `agent_tasks: "local_only"` with every `review_agent_tasks` `null`. The Overview shows `agentTasksLocal` and no digit in that row. Mutations, each red: `[]` served on a hub; `null` shown as 0.
- [x] DO5 — **No clock.** The three key sets are pinned and hold no age. Two runs over one queue are byte-identical, at another root and `HOME` too. In the UI, moving `today` from 2026-11-12 to 2026-11-14 turns "oldest 10 days" into "oldest 12 days" with no read. Mutations, each red: an `oldest_open_item_days` key served; the age computed from `Date.now()`.
- [x] DO6 — **No name.** `workspace_names.rs`, `no_domain.rs` and `policy.test.ts` pass. The scenario's blocks follow `sections` and `flows` alone: Gaps for `words` and `press`, the side table for `press` only. Mutation, red: `name === "writing"` in `department.ts` (`workspace_names.rs`).
- [x] DO7 — **Size.** After `pnpm build`, first-load JS is at most 845 kB, and the `DepartmentOverview` chunk is not among first load's files. Its figure is recorded against the slice before. Mutation, red: a static import of `DepartmentOverview` in `HomeView.tsx` (`lazy.test.ts`).
- [x] DO8 — **Ready equals the rollup.** Store: 300 parents across 3 flows, seeded deterministically, each with 0-4 children open, accepted or cancelled, plus closed parents and grandchildren: `ready_parents` per flow equals a brute count by `card-progress.md`'s rule. UI: over the mock, `tallyOf(…).ready` equals the parents whose children's `progressOf(…).ready` holds. Mutations, each red: a parent ready on any accepted child; a parent with no child counted; a grandchild counted.
- [x] DO9 — **Text and JSON as pinned.** `spec card counts` prints the scenario's text byte for byte and `--json` its document. A flow no `[card]` lists prints its `not configured` line, its columns `not in the flow`; in DO2's test it counts under `press` alone. No `[card]`: the default `plain` flow, its cards in its columns (corrected at ship). No queue: zeros, exit 0. A bad `[card]`: exit 2 with its line. A card whose `tasks` is `"x"`: its note, still counted in `columns`, exit 0. Mutations, each red: a stranded column dropped; `open_subtasks` counting accepted children; an item on a cancelled task left out.
- [x] DO10 — **The daemon serves it, the UI types it.** `GET …/cards/counts` is byte-equal to `spec card counts --json`; `?flow=copy` is a 400, a POST a 405, a bad `[card]` a 503. The fixture holds the three sets, `provisional.ts` types them, `daemonKeys.test.ts` is green and `AHEAD_OF_FIXTURE` empty at the end. `door.rs` lists `card_counts`. Mutations, each red: a set left out of the fixture; `tasks_here` true on a hub.
- [x] DO11 — **[measure] One read.** A test seam counts the store's statements per call: the same over the scenario and over 2 000 cards. Seeded through the store (`crates/specengine-eval/tests/card_counts_scale.rs`, `#[ignore]` above 1 000): 20 000 cards in 6 flows, 4 000 with a parent, 2 000 items, 500 tasks with 1 000 links; `spec card counts --json`, warm, release, at most 300 ms, the store call timed alone too; times in "Implementation". Past the bar, "Known limits" names an index on `cards(parent)`, a queue schema step. Mutation, red: items counted per card.
- [x] DO12 — **The Overview on the mock.** The blocks show per department as DO6 says; the side table ends in "All flows"; each row links its page; `local-only` shows the hub's line; the empty project shows the `EmptyState`, a failed read the `ErrorPanel` with Retry. `catalog.test.ts`, `words.test.ts` and `scale.test.ts` pass. Mutations, each red: a `home.department` key missing in `ru.ts`; the side table shown for a department with `flows` set.
- [x] DO13 — **[look]** The orchestrator's, in Chrome, in both themes and both languages, at 1440x900 and 1100x800: the mock's departments, each with its blocks; the side table; the hub's local-only line; the empty project; a failed read; the Gaps block with a gap and with none.
- [x] DO14 — **Gates.** `cargo nextest run` is green for `specengine-core`, `-store`, `-cli` and `-http`, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Drift on the Overview (health's code index on each landing); collisions (`collision-pass`'s ledger); delivery's figures, RM-0089's hub default department and an MCP tool; counting by person, a stored or cached count; the Board's tile counts; the old Tasks and Inbox regions.

## Implementation

The UI lane built ahead in a worktree and was merged onto `main`; the Rust lane on `main`, reviewed (ACCEPT) and given iterations 1b and 2. DO1 to DO10 and DO12 ticked at ship, DO11, DO13 and DO14 on the orchestrator's word.

| Module | Does |
|---|---|
| core `card_counts.rs` (new), `lib.rs` | `ColumnTally`, `OpenItems`, `CardCountRows`, `UnreadName`, `AgentTasks`, `CardColumnCount`, `CardFlowCount`, `CardCounts`; `unread_tasks_note`, `unread_name_note`; `counts_document` (order, zero rows, IDs by number, notes by card ID) |
| store `queue/cards.rs`, `queue.rs` | `SqliteQueue::card_counts(tasks_here)`: one deferred transaction, `COUNT_COLUMNS`, `COUNT_UNNAMED`, `COUNT_READY`, `COUNT_ITEMS`, `COUNT_LINKS`, `COUNT_REVIEW`; `count_*`, `task_links`, `unread_why`, `stored_name`; the `#[cfg(test)]` seam `queue/card_counts_seam.rs` (SQLite's statement trace) |
| cli `card_counts.rs` (new), `lib.rs`, `main.rs` | `CardCountsRequest {tasks_here}`, `card_counts`, `CardCountsOutcome::in_view`, the text, the cut, the stderr notes; `Outcome::CardCounts`; `spec card counts` (`tasks_here` true) |
| http `app.rs`, `answer.rs` | the route beside `cards/{id}`, handler `counts`, `tasks_here = app.hub.is_none()`, `run_read`; `cards/counts` in the 404 list and the read routes' doc |
| ui `api/{provisional,client,http,queries}.ts`, `app/Shell.tsx`, `overview/{HomeView,DepartmentOverview}.tsx`, `overview/department.ts`, `ui/served.tsx`, `i18n/{en,ru}.ts`, `styles/app.css`, `mocks/{cardCounts,MockClient,scenario,workspaces}.ts`, `test/{countsStub,stubClient,builders}.ts`; `health/{AmendmentsPart,DriftPart,LeftRegion}.tsx`, `tree/{BindingsPanel,LockDiffPanel}.tsx` (imports) | `KnownAgentTasks`, `CardCounts`, `CardColumnCount`, `CardFlowCount`; `getCardCounts` (judged: an older daemon's 404 never read as counts); `useCardCounts` and its re-read; the lazy Overview; the fold; `home.department.*`; `.department-*`; the mock's counts. Removed: `overview/{TasksRegion,InboxRegion}.tsx`, `parts.tsx`, `tally.ts` and its test, `home.tasks.*`, `home.inbox.*`, `.home-*` |

**The ruling, D1.** Iteration 1 followed the draft's "no `[card]` gives `flows` empty" through a new `card_toml::written_flows`, and asked the owner, since the Board places those cards in `plain`'s columns. The orchestrator ruled the clause wrong; iteration 1b removed `written_flows` (`card_toml.rs` unchanged), counted through `card_from_toml`'s fallback, and re-pinned the test (`do9_no_card_table_counts_under_the_boards_default_flow`, `do9_cards_of_the_default_flow_count_in_its_columns`).

**Rust departures**, canon now: (1) to (5) under "Data"; (6) a card naming itself is no child of its own, in `COUNT_COLUMNS` (since iteration 2) and `COUNT_READY`, as `rollup.ts`; (7) the tail's `<n>` counts the header; (8) the hub's text prints the word `local_only`; (9) exit 2 also on `C` in `[ids]` and on `--config` naming a file but the root's own, as `card list`; (10) notes by card ID, a card's `flow`, then `col`, then its links.

**Review.** Rust ACCEPT, asking iteration 2: a non-string `tasks` entry does not read, noted; a NULL `flow` or `col` noted; a self-parent no child in `COUNT_COLUMNS`; an `app.rs` comment. Verified: the counts per the spec, `COUNT_READY` equal to `rollup.ts`'s `ready`, the SQL safe, about 10 ms per statement at 20 000 cards, determinism byte-equal, the route's 400 and 405, the hub's `local_only`, no `[team]` read, D1 on a project with no `[card]`. No UI review is recorded.

**Iteration 2** built them, the sixth statement `COUNT_UNNAMED` among them, each such card still counted.

**Exact strings**: the header `card counts | flows <n> | cards <t>: open <o>, accepted <a>, cancelled <c> | agent tasks <counted|local_only>`; a flow line `flow <name> | <title or -> | open items <n>, oldest <at or -> | ready parents <r> | agent tasks in review <IDs ", " | - | local_only>`, ` | not configured` appended; a column line `  <flow>/<column> | open <o>, accepted <a>, cancelled <c>, open subtasks <s>`, ` | not in the flow` appended; the tail `[truncated: <k> of <n> lines not shown]`; the notes `` <C>: its `tasks` does not read; its agent tasks are not counted `` and `` <C>: the stored `<flow|col>` cannot be read: it is NULL; counted under `NULL` `` (`it is a number, not text`, `it is a BLOB, not text`); the 400 `` `flow` is no query name of cards/counts: it takes no query name ``.

**UI departures**, canon now (`card-counts.md` "Overview"): (1) `Notes` and `Stored` moved from `overview/parts.tsx` to `src/ui/served.tsx`, `.home-row-time` now `.stored-time`; (2) harbor-sim's mock gained a third department, `harbourmaster` (`flows` null, no disciplines), for the side table; (3) Gaps reuses `flow.reading` and `flow.readFailed`; (4) each Waiting row is a count, then an "Open …" link; (5) the Waiting `h2` outside the PropertyList's region, so no two regions share a name; (6) the side table's Oldest question reuses `oldest_*`, "None" with no question; (7) `home.about` stays above the department line; (8) the empty state replaces Work, Waiting and the side table, Gaps still shown by config, and since D1 it shows only on the mock; (9) a failed re-read shows the `ErrorPanel` above the kept numbers, no new string; (10) Open Flow last in Gaps, Work with no link; (11) the palette now reads agent tasks and the inbox once when opened on the home, which no longer reads them first.

**Pins moved**: http `tests/hub.rs` `GET_ROUTES` 30 to 31 and `CARD_ROUTES` 10 to 11; `door.rs` `CARD_CALLS` 12 to 13; `methods.rs`' 404 list; `kind_amendment`'s set count 140 to 143; `daemonKeys.test.ts` `ALL` 143, `AHEAD_OF_FIXTURE` empty.

**Numbers.** First load 829.97 kB (829 965 B) after the merge, -5.33 from 835.33, gzip 241.9 kB; `DepartmentOverview-*.js` 10.39 kB (3.17 gzip), outside it; `ru-*.js` 159.11 kB. UI 198 files, 5 615 tests after the merge; the UI's 19 mutations red, DO6's literal checked against a Python copy of `workspace_names.rs`' rule in the UI lane.

**DO11**, release, warm, 20 000 cards (4 000 with a parent), 2 000 items, 500 tasks, 1 000 links: `spec card counts --json` a median of 17-18 ms over three runs (bar 300 ms), the library read 15 ms, the store call alone 14-15 ms; no index on `cards(parent)` needed. The seam: 12 statements with tasks, 9 without, the same at 7 cards and at 2 000.

**DO13**, the orchestrator's look, on a scratch copy of a real game corpus (10 cards, 1 cancelled, departments dev and qa), a debug daemon, the main tree's UI build, light, Russian, 1440x900: dev's Work showed its flow at to do 5, in progress 1, waiting for review 0, returned 1, done 0, and 1 cancelled; Waiting showed returned subtasks 1 and open questions 7 (oldest 1 day), each linked; Gaps none; qa's flow to do 2, the rest 0; every number equal to `spec card counts` on that queue. Not looked at: the dark theme, English, 1100 wide, the hub's local-only line, the side table (no department with null `flows`).

**DO14.** core and store 1 308 of 1 308; http 611 of 611; CLI green, its reds load flakes or a README mid-edit, green on rerun; the eval guards and `card_counts_scale` 59 of 59; `daemonKeys` 437 of 437; the Rust lane's 17 mutations red.

**Test differences**, the test-engineer's: (1) `genre` caught ADR IDs in `card_counts.rs`, fixed at ship; (2) the no-`[card]` ruling (D1); (3) a `tasks` list with a non-string entry or a non-task ID does not read; (4) the name notes and the self-parent rule ("Data" 4, departure 6); (5) only a `NULL` `flow` or `col` is reachable, `cards` being `STRICT`, a number or BLOB never. `shards_repo` re-pinned for `card-counts.md`.
