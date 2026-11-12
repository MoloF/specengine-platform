---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0086, v5 core slice 11 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section RM-0086, DO1 to DO8; here DO1 to DO14); the orchestrator's working answers, the owner's review open
adrs: []
---

# department-overview: each department opens on its own numbers, counted by status category over its flows (RM-0086)

## Why

Today's Overview holds two blocks, Tasks and Inbox, and both copy other pages. A department lands on it and learns nothing about its own work: what is to do, in progress, done or returned in its flows, what waits on it, and who handed what back. The lead cannot see the departments side by side. The numbers cannot come from the Board's list, which is cut at 40 000 characters (`cards.md` "Commands", list), and `card-progress.md` already rules that no count comes from it.

After this slice, `spec card counts [--json]` and the daemon's `GET …/cards/counts` serve counts that no clock touches:
- per flow and column: open, accepted and cancelled tasks, and open subtasks;
- per flow: open items on its tasks with the oldest one's `created_at`, parents ready to accept, and the agent tasks awaiting the owner that its open tasks link.

The Overview (`#/<p>`) folds those counts by status category over the chosen department's flows. It shows what waits on that department, its flow's gaps where its sections list Flow, and every department side by side where its `flows` is null.

No ADR. It rests on:
- ADR-0073 and ADR-0076: a department is a lens, never a filter hiding work, its statuses its flows' columns by position;
- ADR-0047: what cannot be measured shows its cause, never 0;
- ADR-0075: agent tasks are local;
- ADR-0074 and ADR-0012: Ready to accept is a hint;
- ADR-0008: no code names a department.

Drift and collisions are out of this cut.

## Description and interactions

### Decisions

- **D1. The daemon counts; the UI folds.** The route serves counts per (flow, column) and per flow, and knows no department or category. The UI maps columns to categories with `src/board/category.ts` and flows to departments with `useWorkspaces`, so the Board's column heads and the Overview agree by construction.
- **D2. Counts read stored columns, decode no row.** They come from one read transaction of grouped statements: `acceptance`, `cancellation` and `parent` by whether they are NULL, and `flow` and `col` as stored. A row the list would note as corrupt still counts where it stands.
- **D3. Ready to accept is `rollup.ts`'s `ready`.** That is an open parent with one child or more, every child accepted or cancelled, a grandchild never counting (`card-progress.md` "The model").
- **D4. No clock.** The document holds timestamps, never an age. The UI computes "oldest N days" against the viewer's day (`useToday()`, `board/due.ts`), as the Board judges its overdue badge.
- **D5. Blocks are chosen by config, never by a name.**
  - Work and Waiting always show.
  - Gaps show where the department's `sections` lists `flow`.
  - Departments side by side shows where its `flows` is null and two or more departments are served.
  - Drift is not built: no Overview reads health, whatever `sections` lists.
- **D6. Agent tasks are local.** A hub serves `agent_tasks: "local_only"` and every `review_agent_tasks` as `null`. The Overview says so in words, never 0.
- **D7. The Overview is replaced.** The Tasks and Inbox regions leave it. `design-sweep` renamed its tokens only, and `design-sweep-b` leaves it alone.
- **D8. No MCP tool, no plugin version, no schema step.** An agent has `list_cards`; the Overview is a viewer's lens, as `spec workspace` is.

"Data" fixes every name, so the Rust and UI lanes never wait on each other.

## Data

### Core, store, CLI library

- **Store** (`crates/specengine-store/src/queue/cards.rs`): `card_counts(&self, tasks_here: bool) -> Result<CardCountRows, QueueError>`, one read transaction:
  1. per (`flow`, `col`): `open`, `accepted`, `cancelled` and `open_subtasks`. A card counts in exactly one of the first three: `cancellation` set is cancelled, else `acceptance` set is accepted, else open. `open_subtasks` counts the open ones with `parent` set.
  2. per parent `flow`: `ready_parents`. These are open parents having a child, with no open child, grouped over `parent` once (never per card).
  3. per card `flow`: `open_items` and `oldest_open_item`. These are the `proposals` rows with `card_id` naming one of the flow's cards (in any state) and `status = 'open'`, with their least `created_at`.
  4. only with `tasks_here`: per `flow`, the distinct `T-` IDs in the `tasks` of its open cards whose `tasks` row, in any repository of the slug, has `status = 'review'`. A `tasks` value that is not a JSON list is passed over, and its card's ID is returned.
- **Core** (`crates/specengine-core/src/card_counts.rs`, pure): `counts_document(flows: &[CardFlowShape], rows: &CardCountRows, tasks_here: bool) -> CardCounts`, the JSON's structs.
  - Order: configured flows in config order, then flows no `[card]` lists, by name. Inside a configured flow: its columns in config order, zeros too, then stranded columns by name.
  - Every flow gets a `by_flow` row, zeros too.
  - IDs sort by number.
- **CLI library** (new `crates/specengine-cli/src/card_counts.rs`): `CardCountsRequest {tasks_here: bool}`, `card_counts(env, globals, request) -> Result<CardCountsOutcome, CliError>` and `CardCountsOutcome::in_view(View)`.
  - It reads `[card]` once (`card_from_toml`), then opens the queue once with `SqliteQueue::open_existing`, read-only.
  - No git, no index, no lock, no write.
  - The command `spec card counts [--json]` (`card.rs`, `main.rs`) always passes `tasks_here: true`.

### JSON (`--json`, the daemon's route)

**The scenario**, also the tests' corpus.
- **Config.** Flows:
  - `copy` "Copy": columns `todo`, `doing`, `review`, `returned`, `done`; accept `review` to `done`; return `returned`;
  - `plates` "Plates": `todo`, `drawing`, `done`; accept `drawing` to `done`;
  - `sound` "Sound": `todo`, `check`, `done`; accept `check` to `done`.

  Departments:
  - `words` "Words": sections `flow`, `board`, `questions`, `inbox`; flows `copy`;
  - `print` "Print": sections `board`, `health`, `inbox`; flows `copy`, `plates`;
  - `press` "Press room": sections `inbox`, `board`, `roadmap`, `flow`; no `flows`.
- **Cards.** Each time is `T09:00:00Z`.
  - C-0001 "Write the foreword": `copy`/`review`, open, tasks `T-0001`.
  - C-0002 "Draw the plates": `plates`/`done`, accepted, parent C-0001.
  - C-0003 "Cut the index": `copy`/`todo`, cancelled, parent C-0001.
  - C-0004 "Proof the map": `copy`/`returned`, open, parent C-0005.
  - C-0005 "Print the atlas": `plates`/`todo`, open, tasks `T-0001`, `T-0002`.
  - C-0006 "Record the bell": `sound`/`todo`, open.
  - C-0007 "Set the type": `copy`/`proofing`, a column the config no longer lists, open.
- **Items.**
  - PR-0001: a question on C-0001, open, made 2026-11-02.
  - PR-0002: a question on C-0004, open, made 2026-11-05.
  - PR-0003: a decision on C-0001, applied.
  - PR-0004: a question on C-0003 (cancelled), open, made 2026-11-07.
- **Tasks.** T-0001 is in `review`, T-0002 is `ready`.

`flows` is byte-equal to `spec card list --json`'s `flows` (set `CardFlow`); it is elided here:

```json
{"flows": […],
 "columns": [
  {"flow": "copy", "column": "todo", "open": 0, "accepted": 0, "cancelled": 1, "open_subtasks": 0},
  {"flow": "copy", "column": "doing", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "review", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "returned", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 1},
  {"flow": "copy", "column": "done", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "copy", "column": "proofing", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "todo", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "drawing", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "plates", "column": "done", "open": 0, "accepted": 1, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "todo", "open": 1, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "check", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0},
  {"flow": "sound", "column": "done", "open": 0, "accepted": 0, "cancelled": 0, "open_subtasks": 0}],
 "by_flow": [
  {"flow": "copy", "open_items": 3, "oldest_open_item": "2026-11-02T09:00:00Z", "ready_parents": 1, "review_agent_tasks": ["T-0001"]},
  {"flow": "plates", "open_items": 0, "oldest_open_item": null, "ready_parents": 0, "review_agent_tasks": ["T-0001"]},
  {"flow": "sound", "open_items": 0, "oldest_open_item": null, "ready_parents": 0, "review_agent_tasks": []}],
 "agent_tasks": "counted", "notes": [], "truncated": false, "omitted": null}
```

- Every key is present; an absent value is `null`.
- `agent_tasks` is `counted` or `local_only`. With `local_only`, every `review_agent_tasks` is `null`.
- A stranded column (`proofing`) carries no mark of its own: the UI's `categoryOf` and the text tell it from `flows`.
- `omitted` counts the `columns` and `by_flow` rows cut, `null` uncut. The daemon (`View::Browser`) never cuts.

### Text

The scenario's `spec card counts`:

```text
card counts | flows 3 | cards 7: open 5, accepted 1, cancelled 1 | agent tasks counted
flow copy | Copy | open items 3, oldest 2026-11-02T09:00:00Z | ready parents 1 | agent tasks in review T-0001
  copy/todo | open 0, accepted 0, cancelled 1, open subtasks 0
  copy/doing | open 0, accepted 0, cancelled 0, open subtasks 0
  copy/review | open 1, accepted 0, cancelled 0, open subtasks 0
  copy/returned | open 1, accepted 0, cancelled 0, open subtasks 1
  copy/done | open 0, accepted 0, cancelled 0, open subtasks 0
  copy/proofing | open 1, accepted 0, cancelled 0, open subtasks 0 | not in the flow
flow plates | Plates | open items 0, oldest - | ready parents 0 | agent tasks in review T-0001
  plates/todo | open 1, accepted 0, cancelled 0, open subtasks 0
  plates/drawing | open 0, accepted 0, cancelled 0, open subtasks 0
  plates/done | open 0, accepted 1, cancelled 0, open subtasks 0
flow sound | Sound | open items 0, oldest - | ready parents 0 | agent tasks in review -
  sound/todo | open 1, accepted 0, cancelled 0, open subtasks 0
  sound/check | open 0, accepted 0, cancelled 0, open subtasks 0
  sound/done | open 0, accepted 0, cancelled 0, open subtasks 0
```

- The header line: `flows` counts the configured flows.
- A flow line:
  - a flow no `[card]` lists prints `flow <name> | - | … | not configured`;
  - IDs are joined by `, `;
  - values pass `one_line`.
- Notes go to stderr as `note: <n>`, equal to JSON `notes`, sorted by card ID: `` <C>: its `tasks` does not read; its agent tasks are not counted ``.
- Cut at `OUTPUT_CAP_CHARS` as `list` is: lines whole, in order, while they fit, then `[truncated: <k> of <n> lines not shown]`.
- Exit:
  - 0 when answered, a note included. No queue yet gives every configured column at 0, and no `[card]` table gives `flows` empty with every stored column stranded.
  - 2 on a bad `[card]` (`<config>:<line>: <message>`, as every `spec card` command), no project, a newer schema, or usage.

### Daemon, hub

- **Route.** `GET /api/projects/:p/cards/counts` serves `spec card counts --json`, browser view, run as `GET …/cards` is.
  - Locally `tasks_here` is true; on a hub it is false.
  - A query parameter is a 400, nothing run.
  - Other methods get 405 `Allow: GET`.
  - A bad `[card]`, a newer schema or a corrupt queue is a 503, as the card door's (`board-ui.md` "The door").
- **Door test.** `tests/door.rs` `CARD_CALLS` gains `card_counts` (13), and the GET joins the endpoints that change nothing.
- **The hub** mounts the route for every login. `Hub::fenced` (default deny) admits it as a GET any login may make, as `GET …/cards` is (`GET_ROUTES` + 1), and never reads `[team]`. It counts its own queue.
- **Keys** (`fixtures/daemon-keys.json`), three new sets, the document's `flows` reusing `CardFlow`:
  - `CardCounts` (7): `flows`, `columns`, `by_flow`, `agent_tasks`, `notes`, `truncated`, `omitted`;
  - `CardColumnCount` (6);
  - `CardFlowCount` (5).

### The UI line

- **Types** (`provisional.ts`, citing DO10): `CardCounts`, `CardColumnCount`, `CardFlowCount`; `agent_tasks` is `KnownAgentTasks | Unlisted`, `review_agent_tasks` is `string[] | null`.
  - `client.ts`: `getCardCounts(project, signal)`; `http.ts`: the GET.
  - The mock (`src/mocks/cardCounts.ts`) derives the counts from harbor-sim's mock cards by these rules, so the Board and the Overview agree. `?scenario=local-only` serves the hub's shape, and the empty project serves no flow.
- **The read** (`queries.ts`): `useCardCounts(project)`, key `["cards", project, "counts"]`.
  - The key sits under `cardListsOf(project)`, so a `card.*` event, a card write's settle or a gap reads it again.
  - So do every `proposal.*` but `staged` and `unstaged`, and every `task.*`.
  - A read in flight is aborted by the next one.
- **The model** (`src/overview/department.ts`, pure). It is the one place a section is compared, by `sections.ts`' constant, and it compares no department, flow or column name:
  - `departmentFlows(counts, workspace)`: the department's `flows` that `by_flow` holds, in its order. With `null` or no workspace, every `by_flow` flow.
  - `tallyOf(counts, flows) -> DepartmentTally {todo, doing, review, returned, done, stranded, cancelled, questions, oldest, returnedSubtasks, ready, agentTasks}`, column by column over those flows:
    - `done += accepted`;
    - an open count goes to its column's `categoryOf(flow, column)`;
    - `cancelled` stays apart;
    - `review += open` in its flow's `accept.from`;
    - `returnedSubtasks += open_subtasks` in its flow's `return`;
    - per flow, `questions` sums, `oldest` is the least, and `ready` sums;
    - `agentTasks` is the size of the union of the IDs, `null` when `local_only`.
  - `blocksOf(workspace, workspaces) -> {gaps, side}`: `gaps` when `sections` lists Flow, or with no workspace read; `side` when `flows` is `null` and two or more workspaces are served.
  - `ageDays(createdAt, today)`: calendar days from the viewer-local date of `createdAt` to `today`.
- **The view.** `HomeView({project, workspace, module, workspaces})`, which Shell passes from `choice` and `useWorkspaces`. It shows:
  - its `h1` (`home.title`);
  - with two or more departments, `home.department.named`;
  - then `const DepartmentOverview = lazy(…)` in a `Suspense` with a skeleton, which `lazy.test.ts` pins.

  `src/overview/DepartmentOverview.tsx` shows four blocks, in order:
  1. **Work** (`h2`): a `<table>` with a caption, one row per department flow (its served title in a `bdi`; a flow not configured shows its name raw, `lang="en"`). Its columns are To do, In progress, Waiting for review (a part of In progress, so a row's sum leaves it out), Returned and Done, each `<th>` with its category's icon (`categoryMark`), and its `tfoot` is "All flows" with two flows or more. Below it, when not zero: "<n> not in the flow" and "<n> cancelled".
  2. **Waiting on this department** (`h2`): a `PropertyList` with these rows:
     - Waiting for review, Returned subtasks and Ready to accept (`board.progress.ready`), each linking the Board;
     - Open questions, with "oldest N days" or "oldest today", linking the Inbox;
     - Agent tasks awaiting you, linking Agent tasks; with `local_only`, `home.department.agentTasksLocal` and no digit; an unlisted word raw, `lang="en"`.
  3. **Gaps** (`h2`, by `blocksOf`): `useFlowRead(project, flowForm(root, null))`, the chosen module's root when it is rooted, else none (Flow's own read, shared cache).
     - Every measured row with a count above 0 shows its `gapLook` and `gapMeaning`, in `gapsInOrder` order.
     - Then `home.department.gapsNone` when nothing is found, and `gapsNotMeasured` with the count of rows not measured.
     - Then Open Flow.
     - It has its own skeleton, and its own `ErrorPanel` with Retry.
  4. **Departments side by side** (`h2`, by `blocksOf`): a `<table>` with one row per served department in served order (the title in a `bdi`). Its columns are To do, In progress, Waiting for review, Returned, Done, Open questions, Oldest question and Ready to accept. Its `tfoot` is "All flows", each flow counted once.
- **States.**
  - Counts pending: a skeleton for blocks 1, 2 and 4.
  - Failed: one `ErrorPanel` with the daemon's words under `lang="en"` and Retry. A failed re-read keeps the last numbers with its line (`useRetainedFailure`).
  - No flow and no column served: `EmptyState` level 2 `home.department.empty`, its description linking Agent tasks and Inbox.
  - `notes` show as a note list under `lang="en"`.
  - Workspaces pending: the blocks wait. Failed: every flow, Gaps shown, no side table.
- **Removed**: `TasksRegion.tsx`, `InboxRegion.tsx`, `parts.tsx`, `tally.ts`'s home functions left with no user, their `home.tasks.*` and `home.inbox.*` keys, and their `.home-*` rules.
- **Strings** (`en.ts`, `ru.ts`; the Russian is a draft with plural forms), under `home.department`:
  - `named`, `loading`, `failed`, `work`, `workCaption`;
  - `column.{flow,todo,doing,review,returned,done,department,questions,oldest,ready}`, `allFlows`;
  - `stranded_one|_other`, `cancelled_one|_other`;
  - `waiting`, `reviews`, `returnedSubtasks`, `questions`, `oldest_one|_other`, `oldestToday`;
  - `agentTasks`, `agentTasksLocal` ("Local only: agent tasks live on each terminal");
  - `gaps`, `gapsNone`, `gapsNotMeasured_one|_other`;
  - `openFlow`, `openBoard`, `openInbox`, `openTasks`;
  - `side`, `sideCaption`, `empty`, `emptyText`, `notes`.

  No `en` leaf says "card", "writer", "workspace" or "blocked".
- **CSS**: `.department-*` in `app.css`, scale tokens only, no `SCALE_EXCEPTIONS` entry.

## Rules and edge cases

- WHEN the counts are read, no clock SHALL be read. One queue and config state SHALL give one text and JSON, byte-identical at other roots and `HOME`s.
- No count SHALL come from the Board's list or from any read that is cut.
- WHEN a card is accepted, the Overview SHALL count it as done, whatever its column. An open card SHALL count by `categoryOf`, an open card in `accept.to` as done. A cancelled card SHALL count as cancelled alone.
- WHEN a parent is open with one child or more and none open, it SHALL count once in its own flow's `ready_parents`. A parent with no child, a closed parent and a grandchild SHALL NOT count.
- An open item SHALL count on its card's flow whatever the card's state.
- WHEN one `T-` task is linked from two flows of one department, it SHALL count once there.
- A flow two departments list SHALL count in both, and "All flows" SHALL count each flow once.
- A flow no `[card]` lists SHALL count only under departments whose `flows` is null, its columns "not in the flow".
- WHEN a department's `sections` lists `health`, the Overview SHALL still read no health (`health.md` "UI": the home reads none).
- WHEN `sections` does not list `flow`, it SHALL read no flow.
- No code SHALL compare a department's, flow's or column's name with a literal (`workspace_names.rs`, `policy.test.ts`).
- Every count is a pointer: nothing here moves, refuses or gates a task (ADR-0012). Ready to accept is a status line, never a control (`card-progress.md` "The rule").
- Writes: none. The read opens the queue read-only, takes no lock, refreshes no index and runs no git.

**Rollout.** There is no schema step, backup format, config key or MCP tool. A daemon or hub older than this slice answers the route with 404, and the Overview shows its `ErrorPanel`. The hub takes it at its next upgrade, the owner's call.

**Test churn.**
- Daemon: `door.rs` (`CARD_CALLS`), `no_domain.rs`, the 404 route listing, the hub's route lists, `daemon_keys.rs` and `daemonKeys.test.ts` (3 sets), and the CLI usage golden for `spec card`.
- UI: `HomeView.test.tsx` is rewritten, along with the smoke tests and `liveScreens.test.tsx` that read the home, and `lazy.test.ts`.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Rows are written through the store with fixed `now`s, as its tests do. Corpora are built at run time with invented names. "The scenario" is the one in "Data". DO1 to DO8 follow the analysis's, adjusted to this cut.

- [ ] DO1 — **Uncapped.** 700 cards with 120-character titles cut `spec card list --json` (`truncated` true). `spec card counts --json` still sums `open` + `accepted` + `cancelled` to 700, as the route does. Mutation, red: the counts taken from `card_list`'s rows.
- [ ] DO2 — **The fold** (`department.test.ts`, over the scenario's document, `today` 2026-11-12):
  - `words`: to do 0, in progress 1, waiting for review 1, returned 1, done 0, not in the flow 1, cancelled 1, questions 3, oldest 10 days, returned subtasks 1, ready 1, agent tasks 1.
  - `print`: to do 1, done 1, the rest as `words`.
  - `press` and "All flows": to do 2, the rest as `print`.
  - An open card in `accept.to` is done.
  - Mutations, each red: `accept.to` counted in progress; an accepted card counted in its column's category; a cancelled card counted in its column's category.
- [ ] DO3 — **No health, Flow only by config** (`HomeView.test.tsx`, stub client).
  - For `print`, whose sections list `health`, `getHealth` is called 0 times, and so is `getFlow`.
  - For `words`, `getFlow` is called once, with the chosen module's root when it is rooted.
  - Mutations, each red: a health read on the Overview; the flow read made whatever the sections.
- [ ] DO4 — **The hub.** The hub's `GET …/cards/counts` answers `agent_tasks: "local_only"` with every `review_agent_tasks` `null`. The Overview shows `agentTasksLocal` and no digit in that row. Mutations, each red: `[]` served on a hub; `null` shown as 0.
- [ ] DO5 — **No clock.**
  - The three key sets are pinned and hold no age.
  - Two runs over one queue are byte-identical, at another root and `HOME` too.
  - In the UI, moving `today` from 2026-11-12 to 2026-11-14 turns "oldest 10 days" into "oldest 12 days" with no read.
  - Mutations, each red: an `oldest_open_item_days` key served; the age computed from `Date.now()`.
- [ ] DO6 — **No name.** `workspace_names.rs`, `no_domain.rs` and `policy.test.ts` pass. The scenario's blocks follow `sections` and `flows` alone: Gaps for `words` and `press`, the side table for `press` only. Mutation, red: `name === "writing"` in `department.ts` (`workspace_names.rs`).
- [ ] DO7 — **Size.** After `pnpm build`, first-load JS is at most 845 kB, and the `DepartmentOverview` chunk is not among first load's files. Its figure is recorded against the slice before. Mutation, red: a static import of `DepartmentOverview` in `HomeView.tsx` (`lazy.test.ts`).
- [ ] DO8 — **Ready equals the rollup.**
  - Store: 300 parents across 3 flows, seeded deterministically. Each has 0-4 children, each child open, accepted or cancelled, plus closed parents and grandchildren. `ready_parents` per flow equals a brute count by `card-progress.md`'s rule.
  - UI: over the mock, `tallyOf(…).ready` equals the parents whose children's `progressOf(…).ready` holds.
  - Mutations, each red: a parent ready on any accepted child; a parent with no child counted; a grandchild counted.
- [ ] DO9 — **Text and JSON as pinned.**
  - `spec card counts` prints "Text"'s lines byte for byte, and `--json` prints "JSON"'s document.
  - A flow no `[card]` lists prints its `not configured` line, its columns `not in the flow`; in DO2's test it counts under `press` alone.
  - No queue: zeros, exit 0. A bad `[card]`: exit 2 with its line.
  - A card whose `tasks` is `"x"`: its note, still counted in `columns`, exit 0.
  - Mutations, each red: a stranded column dropped; `open_subtasks` counting accepted children; an item on a cancelled task left out.
- [ ] DO10 — **The daemon serves it, the UI types it.**
  - `GET …/cards/counts` is byte-equal to `spec card counts --json`.
  - `?flow=copy` is a 400, a POST a 405, and a bad `[card]` a 503.
  - The fixture holds the three sets, and `provisional.ts` types them. `daemonKeys.test.ts` is green, and `AHEAD_OF_FIXTURE` is empty at the end.
  - `door.rs` lists `card_counts`.
  - Mutations, each red: a set left out of the fixture; `tasks_here` true on a hub.
- [ ] DO11 — **[measure] One read.**
  - A test seam counts the store's statements per call: the same over the scenario and over 2 000 cards.
  - Seeded through the store (`crates/specengine-eval/tests/card_counts_scale.rs`, `#[ignore]` above 1 000): 20 000 cards in 6 flows, 4 000 with a parent, 2 000 items, 500 tasks with 1 000 links.
  - `spec card counts --json`, warm, release, takes at most 300 ms, and the store call is timed alone too. Times go in "Implementation".
  - Past the bar, "Known limits" names an index on `cards(parent)`, a queue schema step.
  - Mutation, red: items counted per card.
- [ ] DO12 — **The Overview on the mock.**
  - The blocks show per department as DO6 says.
  - The side table ends in "All flows".
  - Each row links its page.
  - `local-only` shows the hub's line.
  - The empty project shows the `EmptyState`, and a failed read shows the `ErrorPanel` with Retry.
  - `catalog.test.ts`, `words.test.ts` and `scale.test.ts` pass.
  - Mutations, each red: a `home.department` key missing in `ru.ts`; the side table shown for a department with `flows` set.
- [ ] DO13 — **[look]** The orchestrator's, in Chrome, in both themes and both languages, at 1440x900 and 1100x800:
  - the mock's departments, each with its blocks;
  - the side table;
  - the hub's local-only line;
  - the empty project;
  - a failed read;
  - the Gaps block with a gap and with none.
- [ ] DO14 — **Gates.**
  - `cargo nextest run` is green for `specengine-core`, `-store`, `-cli` and `-http`, then the full run once.
  - The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass.
  - UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

- Drift on the Overview, which would need health's code index on each landing.
- Collisions (`collision-pass`'s ledger).
- Delivery's figures (`delivery-analytics`), RM-0089's hub default department, and an MCP tool.
- Counting by person, and a stored or cached count.
- The Board's tile counts (`card-progress.md` "Known limits").
- The Overview's old Tasks and Inbox regions.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{card_counts,lib}.rs`; `crates/specengine-store/src/queue/cards.rs`; `crates/specengine-cli/src/{main,lib,card}.rs` and a new `card_counts.rs`; `crates/specengine-http/src/{app,hub}.rs` |
| test-engineer, Rust | `card_counts.rs` in core, store, CLI and http `tests/`; DO8's brute count; DO11's seam and `crates/specengine-eval/tests/card_counts_scale.rs`; `door.rs`, `daemon_keys.rs`, `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`, once both lanes compile) with, in the same change, `daemonKeys.test.ts`'s three sets and the counts out of `AHEAD_OF_FIXTURE`; the churn |
| ui-developer | "The UI line": `provisional.ts`, `client.ts`, `http.ts`, `src/mocks/cardCounts.ts`, `queries.ts`, `src/overview/{HomeView,DepartmentOverview}.tsx`, `src/overview/department.ts`, the removed files, `src/app/Shell.tsx` (HomeView's props), `src/app/lazy.test.ts`, `src/styles/app.css`, `en.ts`, `ru.ts`. The three sets stay in `AHEAD_OF_FIXTURE` until the fixture lands |
| test-engineer, UI | `department.test.ts`, `HomeView.test.tsx`, the mock smoke test; DO7's figure; DO2-DO6 and DO12's mutations |

Comments cite `docs/features/department-overview.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after core slice 10 (`plain-questions`, its pair half) ships, one Rust slice at a time. The UI starts after `design-sweep-b` has merged (both edit `app.css` and the catalogs).
2. The ui-developer lands the UI line behind `AHEAD_OF_FIXTURE`. The rust-developer goes store, core, CLI, http.
3. The Rust test-engineer overlaps: store and core (DO8, DO9), CLI (DO1, DO5, DO11), http (DO4, DO10). Then the Rust and UI reviews.
4. The orchestrator's look (DO13). Ship: the spec-writer ("Canon at ship").

At most three iterations a lane.

### Risks

- **First load.** The blocks are lazy, the removed regions free bytes, and the strings stay in the catalog. Past 845 kB, the strings move into the chunk's own catalog part (`ui-languages.md` "Size").
- **The hub** gets one more GET per login on each landing, plus Flow's read where a department lists Flow. The flow read is shared with Flow; its whole-read timings are `scale-eval.md`'s `get_flow_top` and `get_flow_root`.
- **`ready_parents`** groups over `parent`. With no index on it, SQLite builds an automatic one per call. DO11 measures this.
- **A department's word.** "Ready to accept" reuses `board.progress.ready`, the owner's word, which `src/board/words.test.ts` pins.

### Canon at ship

Free bytes now are in brackets. "Net" means trimmed in the same file to fit.

| File | Edit |
|---|---|
| `docs/canon/card-counts.md` (new, Tier 2, `card*` shard) | the counts (semantics, JSON, text, route, hub); the Overview (blocks by config, the fold, ages, states); "Known limits" (agent tasks of any repository, the hub's local-only, the measured cost); "Tests" |
| `docs/canon/cards.md` [73] | "Commands": `counts` in the synopsis, a pointer. Net |
| `docs/canon/board-ui.md` | "Routes": the GET row; `CARD_CALLS` 13 |
| `docs/canon/workspaces.md` [14] | "Screen", Landing: the Overview is the chosen department's (a pointer). Net |
| `docs/canon/hub-team.md` [34] | "Answers": `GET_ROUTES` + 1. Net |
| `docs/canon/card-progress.md` | "Known limits": the Overview's ready count is `ready`'s (a pointer) |
| `docs/canon/ui-languages.md` [9] | "Size": the figure. Net |
| READMEs: store [3], cli [1], http [4] | a pointer each. Net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 11; RM-0086 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q1. Drift and collisions in the first cut? | No | Drift builds health's code index on each landing (seconds) and needs a hub branch; collisions read `collision-pass`'s ledger |
| Q2. Who folds to departments and categories? | The UI, over per (flow, column) counts | A served fold repeats `category.ts` in Rust and turns every config change into a route change |
| Q3. Blocks per department? | By config: `sections` and `flows` | By name breaks ADR-0008 and `workspace_names.rs` |
| Q4. Agent tasks on a hub? | `null`, "local only" | 0 would claim a measurement; a hub reads no task (ADR-0075) |
| Q5. Keep the Tasks and Inbox regions? | No: both copy their pages, one click away in the nav, and the waiting rows link there | Two more reads per landing and their first-load bytes |

**The spec-writer's:**
- W1: `open_subtasks` is per column, not "returned subtasks" per flow. The UI picks the flow's `return` column, and one key serves any column.
- W2: `review_agent_tasks` serves IDs, not a count. A task linked from two flows of one department then counts once.
- W3: tasks of any repository of the slug count, with no git, so the landing stays cheap. A card shows such a task as "not read here" (`card-tasks.md`). "Known limits" says so.
- W4: the route sits under `spec card`. A bad `[card]` exits 2 there, and is a 503 at the route, as with every card read.
- W5: `delivery-analytics` (Risks) expected this slice to read its rows. Its rows judge delivery at an `as_of` and hold no column, so they cannot give categories. This read holds no date, so neither reads the other. Overdue and lead time stay on Roadmap's Delivery tab.
