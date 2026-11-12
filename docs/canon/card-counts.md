---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card counts: per flow and column, and each department's Overview

Built by `department-overview` (RM-0086), no ADR: it rests on ADR-0073 and ADR-0076 (a department is a lens, its statuses its flows' columns by position), ADR-0047 (what cannot be measured shows its cause, never 0), ADR-0075 (agent tasks are local), ADR-0074 and ADR-0012 (Ready to accept is a hint) and ADR-0008 (no code names a department). Related: `cards.md`, `card-progress.md`, `workspaces.md`. Code: core `card_counts.rs`; store `queue/cards.rs` (`SqliteQueue::card_counts`); CLI `card_counts.rs`; http `app.rs` (`counts`); UI `src/overview/`.

## The rule

The daemon counts, the UI folds: the route serves counts per (flow, column) and per flow and knows no department or category; the UI maps columns to categories with `board/category.ts` and flows to departments with `useWorkspaces`, so the Board's column heads and the Overview agree by construction. The flows are `card_from_toml`'s, the Board's own: no `[[card.flow]]` written (no `[card]`, an empty one, `flow = []`) counts under the default `plain` flow, byte-equal to `spec card list --json`'s `flows`. No count comes from the Board's list or any cut read; none is stored. No clock is read: one queue and config state gives one text and JSON at any root and `HOME`. Every count is a pointer: nothing moves, refuses or gates a task. Writes: none; the queue opened with `open_existing` (no schema step), no lock, no index, no git.

## Counts

`card_counts(tasks_here)`: one deferred read transaction of six fixed statements, none per card; stored columns read, no row decoded, so a row `list` notes as corrupt still counts where it stands. A schema past the build: `SchemaTooNew`; a table the schema lacks counts none.
- `COUNT_COLUMNS`, per stored (`flow`, `col`): each card in one of `open`, `accepted`, `cancelled` (a `cancellation` first, then an `acceptance`), and `open_subtasks`, the open ones with a `parent`, a card naming itself no child.
- `COUNT_UNNAMED`: a `flow` or `col` that is no text counts under its text (`NULL` the word `NULL`, which no `[card]` name spells; an invalid byte replaced), its card noted. `cards` is `STRICT`, so only `NULL` is reachable.
- `COUNT_READY`, per parent's `flow`: open parents with one child or more and none open, grouped over `parent` once: `rollup.ts`'s `ready` (`card-progress.md` "The model"), a grandchild never counting.
- `COUNT_ITEMS` (schema 11 on), per card's `flow`: the open `proposals` whose `card_id` names one of its cards, the card in any state, and their least `created_at`.
- `COUNT_REVIEW`, `COUNT_LINKS` (with `tasks_here`, schema 10 on): per flow, the distinct `T-` IDs its open cards' `tasks` link whose task, in any repository of the slug, is in `review`. A `tasks` that is no JSON list of task IDs is passed over whole and noted, the card still counted.

Core `counts_document(&[CardFlow], &CardCountRows, tasks_here)` orders them: configured flows in config order, then flows no `[card]` lists by name; inside a configured flow its columns in config order, zeros too, then its stranded columns by name; a `by_flow` row per flow, zeros too; IDs by number; notes by card ID, a card's names (`flow`, then `col`) before its links.

## Text and JSON

`spec card counts [--json]` (always `tasks_here`):

```text
card counts | flows <n> | cards <t>: open <o>, accepted <a>, cancelled <c> | agent tasks <counted|local_only>
flow <name> | <title or -> | open items <n>, oldest <at or -> | ready parents <r> | agent tasks in review <IDs ", " | - | local_only>[ | not configured]
  <flow>/<column> | open <o>, accepted <a>, cancelled <c>, open subtasks <s>[ | not in the flow]
[truncated: <k> of <n> lines not shown]
```

`flows` counts the configured flows, `cards` every card counted. Values pass `one_line`. Cut at `OUTPUT_CAP_CHARS` as `list`: lines whole, in order; `<n>` counts every line, the header too. JSON `{flows, columns, by_flow, agent_tasks, notes, truncated, omitted}`; a column `{flow, column, open, accepted, cancelled, open_subtasks}`; a flow `{flow, open_items, oldest_open_item, ready_parents, review_agent_tasks}`; `omitted` the rows cut, `null` uncut. A stranded column carries no mark: `flows` tells it.

```json
{"flow": "copy", "open_items": 3, "oldest_open_item": "2026-11-02T09:00:00Z", "ready_parents": 1, "review_agent_tasks": ["T-0001"]}
```

**Notes**, stderr `note: …` and JSON `notes`, none changing the exit: `` <C>: its `tasks` does not read; its agent tasks are not counted `` (open cards, with `tasks_here`); `` <C>: the stored `<flow|col>` cannot be read: <it is NULL|it is a number, not text|it is a BLOB, not text>; counted under `<name>` ``. **Exit** 0 answered (no queue: every configured column 0, nothing created); 2: no project, a bad `[card]` (`<config>:<line>: <message>`), `C` in `[ids]`, `--config` but the root's own, a newer schema.

## Daemon, hub

`GET /api/projects/:p/cards/counts`: the `--json` document, `View::Browser` (nothing cut), `run_read` in the project's turn as `GET …/cards`; a static segment the router matches before `cards/{id}`, so never a card. A query name: 400 `` `<name>` is no query name of cards/counts: it takes no query name ``, nothing run; another method 405 `Allow: GET`; a bad `[card]`, a newer schema: 503. The 404's route list names `cards/counts` after `cards/<id>`. `tests/door.rs` `CARD_CALLS` 13 (`card_counts`); the GET changes nothing. Key sets 141 to 143 of `fixtures/daemon-keys.json`: `CardCounts` 7, `CardColumnCount` 6, `CardFlowCount` 5, its `flows` `CardFlow`.

A hub mounts it for every login (`tests/hub.rs` `GET_ROUTES`, 31), never reads `[team]`, counts its own queue, by flow and column, never by person; it reads no task: `agent_tasks` `local_only`, every `review_agent_tasks` `null`, no link note. An older daemon or hub answers 404, the Overview its `ErrorPanel`. No MCP tool (an agent has `list_cards`), plugin version, schema step or config key.

## Overview

`#/<p>`: `HomeView({project, workspace, module, workspaces})`, from Shell's choice and `useWorkspaces`: the `h1`, `home.about`, with two departments or more `home.department.named`, then the lazy `DepartmentOverview` (not in first load) in a `Suspense` skeleton. The read `useCardCounts`, key `["cards", p, "counts"]` under `cardListsOf`: a `card.*` event, a card write's settle or a gap reads it again, as do every `proposal.*` but `staged`, `unstaged` and every `task.*`; one read per event, one in flight.

`overview/department.ts`, pure, compares only Flow's section (`SectionId`), never a department, flow or column name: `departmentFlows` (its `flows` that `by_flow` holds, in its order; `null` or none: all), `tallyOf` (column by column: `done += accepted`; an open count to `categoryOf(flow, column)`; `cancelled` apart; `review` the open ones in `accept.from`; `returnedSubtasks` `open_subtasks` in `return`; questions summed, the oldest the least, ready summed; agent tasks the union's size, `null` when `local_only`), `blocksOf`, `ageDays` (calendar days to `useToday()`, no clock).

Blocks, by config alone: **Work** (a table per flow, To do, In progress, Waiting for review a part of In progress, Returned, Done, "All flows" with two or more; then "<n> not in the flow", "<n> cancelled"); **Waiting on this department** (Waiting for review, Returned subtasks, Ready to accept, Open questions with "oldest N days", Agent tasks awaiting you, each a count then an Open link; `local_only` `agentTasksLocal`, no digit); **Gaps** where `sections` lists Flow or no department was read (Flow's own read, the module's root when rooted; its own skeleton, error and Retry; Open Flow last); **Departments side by side** where `flows` is null and two or more are served (one row each, "All flows" counting each flow once). No Overview reads health. Pending: skeletons; failed: one `ErrorPanel` with Retry, a failed re-read above the kept numbers; nothing served: `EmptyState` `home.department.empty`, Gaps still where config says; `notes` under `lang="en"`. Strings `home.department.*`; CSS `.department-*`, scale tokens only. `Notes`, `Stored`: `src/ui/served.tsx`.

## Known limits

- **Agent tasks of any repository** of the slug count, with no git, so the landing stays cheap; a card shows such a task "not read here" (`card-tasks.md`).
- **A `NULL` name** counts under `NULL`, beside a real flow or column of that text; the note names each card.
- **On a hub** agent tasks are local-only: no count, the line says so.
- **Two rules for Ready**: the store's SQL and `rollup.ts`, tied by tests alone (store `card_counts.rs`, UI `department.test.ts`).
- **The empty state** shows only when nothing is served; a daemon always serves `plain` at least.
- **Cost**: `ready_parents` groups over `parent` with no index (SQLite builds one per call). At 20 000 cards, 2 000 items and 1 000 links the CLI answers in 17-18 ms, the store call 14-15 ms: no index is needed. The statements per call are fixed: 12 with tasks, 9 without.

## Tests

core, store, CLI, http `tests/card_counts.rs` (the scenario: CLI `tests/counts_scene/`); store `queue/card_counts_seam.rs` (statements per call); eval `card_counts_scale.rs`; http `door.rs`, `hub.rs`, `methods.rs`, `daemon_keys.rs`, `no_domain.rs`; UI `overview/department.test.ts`, `HomeView.test.tsx`, `mocks/cardCounts.test.ts`, `home.smoke.test.tsx`, `lazy.test.ts`, `daemonKeys.test.ts`, `policy.test.ts`.
