---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-core]
owner: owner
reviewed: 2026-10-07
---

# Tasks: commands, transitions, place, store

Phase 2 slice 7 (`docs/features/task-package.md`, ADR-0027): a task is a per-project queue record only the owner moves to `ready`, on a terminal (ADR-0006, ADR-0012); an agent reads its package (`task-package.md`), plans, claims, reports and completes it; spec edits meanwhile raise `stale`, never block. Code: core `task.rs` (IDs, `transition`, caps), store `queue/tasks.rs`, CLI `task.rs`, `package.rs`, MCP `tasks.rs`; library `task_{new,show,list,approve,changes,cancel,plan,claim,report,complete}` (`&Env, &Globals, &<Command>Request`, owner commands a `Consent`).

## Commands

`--root`, `--json` as everywhere; `A` = `propose`'s author flags.

- `spec task new --nodes REF... [--title T] [--goal T] [A]` -> `created T-0001`, `draft`; the nodes resolved as the intake's `node_ids` (ID, `slug/ID`, `.md` path), stored canonical, each once.
- `spec task show T | --next`: text the brief, `--json` the package (`task-package.md`); `--next` the lowest-numbered `ready` task of this repository; both or neither -> exit 2. None found -> exit 1, JSON exactly `{"id": "T-0099", "reason": "no task T-0099 in this repository"}` (`--next`: `id` `null`, `no ready task in this repository`).
- `spec task list [--status S]...`: by number, `<id> | <status> | <title or -> | <targets> | <updated_at>[ | stale]`; JSON `{tasks: [{id, status, title, targets, stale, updated_at}], notes}`; a `null` `stale` adds `<id>: <note>`; other repositories' tasks one note (`<n> task(s) of another repository of the project ... not listed`); a corrupt row skipped, a note.
- Owner only, a terminal's `[y/N]` (stdin no terminal -> exit 2 before anything is read; no `--yes`; no MCP tool: `architecture.md#control`): `approve T` (`approve T-0001 (<title>), freezing <n> node(s) of <worktree> on <branch> at <commit>? [y/N]`), `changes T --note T` (`request changes of T-0001 (<title>)? [y/N]`), `cancel T` (`cancel T-0001 (<status>, <title>)? [y/N]`). Not `y` -> exit 1, no event.
- Agent, no terminal: `plan T --plan-file F|- [--criterion C]... [--affected REF]...`, `claim T --role R --worktree DIR`, `report T --outcome O --summary S [--changed FILE]...`, `complete T`.
- Lines `<verb> T-0001: <status>` (`planned`, `approved`, `returned`, `claimed`, `reported`, `completed`, `cancelled`) + ` (run <n>)` after claim and report; JSON `{id, status, run, notes}`, a refusal too.
- `spec propose update|create|question|discrepancy ... --task T`: "Task-bound proposals".

**Exits** as the queue's: 0 done; 1 refused (no such task, a transition or run that does not allow it, a cap, a place, a declined prompt, a lost compare-and-set); 2 cannot run (`T` in `[ids]`, as a prefix or through `aliases_from`; a look-alike ID, its Latin form named; another repository's task; no terminal; a corrupt row; an unbound place; no git identity at approve).

## Plan

Each call replaces plan, criteria and affected nodes. A criterion that is exactly one reference (ID, `slug/ID`, `.md` path) is a reference, else free text, verbatim; a reference naming nothing -> exit 1 `criteria[i]: ...`. A repeat (the same canonical reference or text) is kept once where it first stands, note `criteria: <n> repeat(s) kept once: criteria[i] (as criteria[j]), ...` (10 named, then `... and <k> more`, the CLI's `...` one U+2026); `j` counts in the plan as given. Caps: `task-package.md` "Caps".

## Transitions

Model `TaskStatus`, ten states (05 s3.3): `draft`, `analysis`, `review`, `changes_requested`, `ready`, `in_progress`, `in_review`, `done`, `accepted`, `cancelled`; `analysis`, `in_review`, `accepted` known, never entered. Core's pure `transition(status, action)` (`TaskAction`), checked again by the store:

| Action | From | To | Event |
|---|---|---|---|
| new | - | `draft` | `task.created` |
| plan | `draft`, `changes_requested` | `review` | `task.planned` |
| approve | `draft`, `review`, `changes_requested`, `ready` | `ready`, snapshot (re)frozen | `task.approved` |
| changes | `review` | `changes_requested`, the note appended | `task.changes_requested` |
| claim | `ready` | `in_progress`, a run opened | `task.claimed` |
| report | `in_progress`, its run open | run closed, state kept | `task.run_reported` |
| complete | `in_progress`, its run closed | `done` | `task.completed` |
| cancel | the five above `done` | `cancelled`, an open run's `ended_at` set (outcome, summary `null`) | `task.cancelled` |

Other pairs -> exit 1 ``T-0001 is <status>: `<action>` needs <states>; nothing changed``; report with no run open, complete with one open, likewise named. A run's `outcome` never moves the state. Cancel retires the task, not its proposals.

**Approve** freezes targets, criteria references and affected nodes, each once in that order, at most 128, from disk in the caller's worktree; a node that does not read -> exit 1 `<field>[i]: <why>; nothing can be frozen of it`. Targets are fixed at `new`: a target gone or moved is never re-targeted; cancel and make a new task.

## Place

ADR-0032. `new` records the repository (its git common dir). `approve` records the snapshot's place: the caller's worktree, `root_rel`, branch, `HEAD`, and the git identity as `by`. `claim`'s `--worktree` (CLI: current-directory relative; MCP: the server's) canonical, one of the task's repository's worktrees (`git worktree list`, never a bare entry), on a branch (git only reads), no control character; else exit 1 naming why (missing, no worktree of the repository, another repository, a detached `HEAD`). Another repository's task (same slug) -> exit 2 naming it, `run the command there`.

`report` and `complete` run only in the claimed worktree (the project root's worktree top, compared as a directory): elsewhere exit 1 `` `T-0001` is claimed in the worktree <w> on `<b>`: `report` runs there, not in <here>; nothing recorded ``.

**Compared place** (staleness, refresh, the package's targets and criteria): the claim's worktree and branch, else the snapshot's (its `root_rel` there), else the reading root. Repositories and worktrees compare as directories (`same_repository`, `same_dir`), never as strings.

## Task-bound proposals

`--task T` (MCP: `task_id` on `propose_change`, `ask_question`, `report_discrepancy`): a task of this repository, neither `done` nor `cancelled`; once claimed, raised in the claimed worktree. Else exit 1 `--task: <reason>` (another repository; closed; ``is claimed in the worktree <w>: a proposal bound to it is raised there, not in <here>``); not a task ID exit 1; a look-alike or `T` in `[ids]` exit 2. Checked once the place is bound and again in the inserting transaction (`QueueError::TaskRefused`). `task_id` never changes; a discrepancy's linked update takes it in the same transaction. Review document: `task_id` (`null` unbound) after `choice`, 45 keys, text line `task_id: <T or ->`; inbox entries none (12 keys).

**Refresh**: WHEN `spec approve` applies a task-bound `update` or section-form `create` (step 10 or a completion: `proposal-apply.md`) whose worktree, branch and `root_rel` are the task's compared place THEN, in the transaction recording `applied` (`applied_refreshing`), each snapshot node of that file enclosing or inside the target whose pre-apply `span_hash` is its snapshot hash takes the applied text and hash: one `task.refreshed` each, `updated_at` and `revision` raised, note ``T-0001: its snapshot of <IDs> refreshed by `PR-0001` ``. A node the apply left byte for byte takes nothing, no event. Nothing refreshes for a file-form create, an unbound proposal, another place, a `done` or `cancelled` task. Nested spans: editing a section by hand changes its enclosing document's span too, so that document stays stale.

## Store

Queue step 4 (`QUEUE_SCHEMA_VERSION` 3 -> 4, now 5; one `Immediate` transaction; `proposal-queue.md` "Store"), `STRICT`, `TEXT` but `run`:

```
tasks(id PRIMARY KEY, project, git_common_dir, status, title, goal, targets, plan, criteria,
  affected_nodes, owner_notes, snapshot, claim, author, created_at, updated_at, revision)
runs(task_id, run INTEGER, role, worktree, branch, author, started_at, ended_at, outcome,
  summary, changed_files, PRIMARY KEY (task_id, run))
proposals: + task_id   -- column 41 of 43
```

`TASK_COLUMNS` 17, `RUN_COLUMNS` 11, pinned to `PRAGMA table_info`. JSON columns: `targets`, `affected_nodes` canonical IDs or paths; `criteria` `{ref, text}` (a reference's text `null`); `owner_notes` `{at, note, by}`; `snapshot` `{at, by, place, nodes: [{id, path, span_hash, text}]}`; `claim` `{at, role, worktree, branch}`; a task's and a run's `author` as a proposal's. IDs `T-` and 4 or more digits, highest + 1, never deleted; runs numbered from 1 per task.

Each op one `Immediate` transaction with its event, a compare-and-set on `TaskSeen.revision`: `revision` decimal text from `1`, raised by every change and refresh (two changes within one second never overwrite each other); lost -> exit 1 `` `T-0001` changed since this run read it: it is <status> since <updated_at>; nothing changed ``. Events `task.created`, `.planned`, `.approved`, `.changes_requested`, `.claimed`, `.run_reported`, `.completed`, `.cancelled`, payload `{id}` (+ `run` for claim and report), `.refreshed` `{id, proposal, node}`. Corrupt (bad JSON, an unknown state or outcome, a non-decimal `revision`, a bad ID): named (`show` exit 2 naming the column), skipped by `list` with a note.

API: `create_task`, `get_task`, `list_tasks`, `change_task(id, &TaskSeen, &TaskChange, now)`, `binding_problem`, `claimed_elsewhere`; `ProposalQueue::create_with_task`, `create_intake_with_task` (debt: fold into the request types). A schema-4 database refuses older builds (`SchemaTooNew`): restart the daemon and reinstall `specengine-mcp` from one commit.

## Backup

`STATE_FORMAT` 2; commands, bounds and import steps: `queue-backup.md`. Header `{"format":2,"queue_schema":5,"project":"<slug>","proposals":p,"tasks":t,"runs":r,"events":e}`, then proposals by number, tasks by number, runs by (task, `run`; a number), events by `seq`. A schema 1-4 database exports unmigrated as format 2, schema 5 (1-3 without tasks or runs).

Import takes format 1 (five header keys, queue schema 1-3, proposals and events; later columns, `task_id` too, `NULL`) or 2 (seven keys, schemas 4-5). Refused, exit 2: a format-2 header of six keys or schema 3, a format-1 of seven; a task ID not as the queue writes it; a task, or a (task, `run`), repeated; a run of a task the dump lacks; a count off the header (format 2 names four).

Count lines `wrote <D>: <p> proposal(s), <t> task(s), <r> run(s), <e> event(s)`, `restored <p> proposal(s), <t> task(s), <r> run(s), <e> event(s) into <db>`; prompt `restore <p> proposal(s), <t> task(s), <r> run(s) and <e> event(s) of <slug> from <FILE> into <db>? [y/N]`; the occupied refusal names the four counts; `--json` `{path|db, proposals, tasks, runs, events}`. `QueueCounts`, `StoredQueue` + `tasks`, `runs` (`StoredTask {columns: [_; 17]}`, `StoredRun {run, columns}`): a task alone makes a queue occupied.
