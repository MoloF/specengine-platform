---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-10
---

# Card tasks: the `T-` tasks a card links

ADR-0075, built by `card-tasks` (RM-0092, v4 step 3b): a card shows the code-work tasks behind it, read now, and decides nothing by them. The card: `cards.md`; a task: `tasks.md`. Code: model, core, CLI `card.rs`; store `queue/{cards,state,tasks}.rs`; CLI `task.rs` (`linked_tasks`), `state_file.rs`; MCP `cards.rs`, `hub.rs`; HTTP `app.rs`; UI `src/board/CardTasks.tsx`.

<a id="the-rule"></a>
## The rule

A card links the `T-` tasks that carry its code work: a list of task IDs on the card's own row, written by `spec card edit --task` and its MCP and daemon twins, and nowhere else. No task row, proposal, event payload or file records the link, and a task never names its card.

The link decides nothing. No gate, card move, accept, assign, cancel or clear reads it, and no task's approve, plan, claim, report, complete or cancel reads a card. A task's state is read per request only to be shown beside its card: it refuses no card write, moves no card and proves nothing about the card's work. Completing a task leaves its card where it stands; accepting a card leaves its links as they are. Accept stays a person's act (ADR-0056), and nothing is blocked (ADR-0012).

The link is local. A `T-` task lives in one terminal's queue and repository (ADR-0032), and a hub runs no `T-` flow, so a hub keeps a card's task IDs as written, checks no task and reads none: it shows the IDs without state, which lives on the terminal that holds the task. ADR-0075.

## Link

`spec card edit C --task T...` (`edit_card {task}`, the daemon's `{"task": [...]}`) replaces the whole list in the order given; a lone `-` (`["-"]`) clears it. `new` takes no `--task`, so a correction card starts with none. `--task` counts as asked, and a re-estimate with `--reason` takes none beside it. In order, exit 1 refusing with the reason last, nothing changed:

1. Exit 2: a look-alike entry (`written_task`, its Latin form named); `[ids]` claiming `T` (`task_prefix_clash`'s line) whenever `--task` is given, on a hub too, a config error found before any task is read.
2. The card's own checks, as any edit (no such card, closed, the other fields).
3. Core `task_links_problem`, asked by the CLI and again by the store under its lock: `` task: "foo" is no task ID; nothing changed `` (a `-` beside others too), `` task: T-0042 is listed twice; nothing changed `` (never folded), `` task: 33 tasks, at most 32; nothing changed `` (`CARD_TASKS_MAX`).
4. Locally, `` task: no task T-0099; nothing changed ``: store `missing_task` finds each ID in one snapshot, in any repository of the slug and any state, a corrupt row counted, no row decoded, no git; a clearing asks nothing. Tasks are never deleted, so this cannot go stale. A hub skips it.
5. A stale `--revision`, at the write.

One edit is one `card.edited {id, fields, as}`, `fields` ending `tasks`, `revision` up by one even for an unchanged list; no new event type, no ID in the payload.

## Read

`show`, `get_card` and `GET .../cards/C` serve `tasks` after `evidence`: `[{id, status, title, stale}]` in link order, each read at request time as `spec task list` reads a task of the current repository. `status` and `title` come from its row (store `read_task`, which hands back a row that does not decode, named), `stale` from its staleness at its compared place (`tasks.md` "Place"; `package.rs` `staleness`, `Roots` caching a root per call). Nothing is stored or cached. Text: `tasks: <id> | <status or -> | <title or ->[ | stale]` per link, the title through `one_line`; `tasks: -` with none.

A card that links none reads as before: no git, no task read, any root. Git runs only for a linked card, and only where the queue holds the tasks. `list`, its 21-key rows, the tiles and every write's document carry no task: the board's read stays git-free.

`status`, `title` and `stale` are `null` together when the task cannot be read here, `stale` alone when only its staleness cannot be told. Each `null` adds a note starting `<id>: ` (`T-1`'s never matches `T-10`'s), after the card's own notes, in link order:

| Case | Note after `<id>: ` |
|---|---|
| the row does not read | `` its row does not read (`<column>`: <reason>); no state shown `` |
| no row (a restored dump, a hand edit) | `no such task in this queue; no state shown` |
| another repository of the slug | `` a task of another repository of the project `<slug>`; its state is read there (`spec task show <id>`) `` |
| no read set up: a root in no git worktree, `[ids]` claiming `T` or `PR`, git or the queue failing | `no state shown: <line>`, the error's line without `spec: ` |
| a hub | `a hub reads no task; its state is on the terminal that holds it` |
| staleness alone | `task list`'s, `` stale unknown: no snapshot (the owner's `spec task approve` freezes one) `` before an approve |

`show` exits 0 whatever its tasks read; 2 only when the card does not read, a corrupt `tasks` included.

**Read: cost.** A linked card's read runs git and, as `task list` does, refreshes the reading root's index once, then reads each task's compared places, up to 32, untimed (`scale-eval.md` has no card rows). MCP `get_card` always asks `--marks`, whose read refreshes too: twice there.

## Screen

A **Tasks** part inside the work item's Development, collapsed (`ui-design.md` "The work item"), modal and page alike, rendered from the card read: no read of its own, no write. The UI writes no link and makes no task (Q9 of 2026-11-08); it copies commands of fixed words and checked IDs.

- **A row per link**: the ID, linked to `#/<p>/tasks/<id>` when `TASK_ID` matches; the title or `tasks.untitled`; the status by `taskStatusLook`, label and icon; `tasks.specChanged` when `stale`. In `review` alone (`awaitsOwner`, `src/tasks/actions.ts`, so `src/board/` names no status): "Awaiting you" and `spec task approve <id>` with Copy. Status `null`: the ID as text, "State not read here".
- **Notes**: a task's notes (`<id>: `) show only on its row, verbatim under `lang="en"`; the card's notes list shows the rest.
- **No link, locally**, an `EmptyState`: "No task yet." and two commands with Copy: `spec task new --nodes <refs> --title "..."`, `<refs>` the card's targets when each matches `^[A-Za-z0-9][A-Za-z0-9._/-]*$`, else `REF`, the title never filled in; `spec card edit <C> --task T-NNNN` when `<C>` is a card ID.
- **The hub view** (`useHubLogin()` resolved): no command, as a terminal's would write its own queue; one sentence, a task runs on a terminal and an agent links one with `edit_card`, under the empty state and the rows alike.
- **Live** (`src/api/queries.ts`): `task.*` and `proposal.applied` (an apply may move a `stale`) invalidate the project's per-card reads, so the open card is read again; the card list never is, for a task.
- Ten keys `board.tasks.*` in both catalogs, the rest reused from `tasks.*`; the daemon's notes English (ADR-0072). No word says "blocked", and the part moves no column.

## MCP and hub

- **`edit_card {task}`**: `[string]` or null, `["-"]` clears; `deny_unknown_fields`, so an older server refuses it. Its text names the field, the cap (asserted against `CARD_TASKS_MAX`) and that no move, accept or gate reads the link; the hub's says the hub holds no task, checks none and shows them without state.
- **`get_card`**: `tasks` (`CardTaskShape`), byte-equal to `show --json`; `GET_DESCRIPTION`, stdio and hub, says each task is read now, `null` with a note starting with its ID where this server cannot read it.
- **The daemon**: `CardEditBody.task`, plus `as`; no new route, query name, fence class or stream type, `CARD_CALLS` ten; on a hub the edit stays in the plain class.
- **Where tasks are read**: `tasks_here` (`cards.md` "Commands") is true for the CLI, the stdio MCP (`seam().is_none()`) and a local daemon (`app.hub.is_none()`), which read and check through the caller's `GitEnv`. A hub (its routes and its `/mcp`) reads no `tasks` row on a card path, serves each linked ID `null` with the hub note, and keeps `--task` IDs as written after the grammar, repeat and cap.
- 29 tools on stdio, 17 on a hub, unchanged; plugin 0.32.0 (`plugin-files.md`).

## Store and backup

- **Step 9 -> 10** (`STEP_10`, `ALTER TABLE cards ADD COLUMN tasks TEXT`), in `migrate`'s one transaction with every due step: `QUEUE_SCHEMA_VERSION` 10, `CARD_COLUMNS` 25, `tasks` last.
- **The column**: a JSON array of task IDs, `'[]'` at `new`, written whole by an edit (`COALESCE(?, tasks)`: a stepped row keeps `NULL` until an edit gives `--task`); `NULL` links none. Bad JSON, no array of strings, or an entry that is no task ID: the card does not read (column `tasks`), `show` exit 2, `list` a note. A repeat or more than 32 from a hand edit reads as stored. `CardEdit.tasks`, `Some` of none clearing.
- **Not yet stepped**: `list_existing_cards` (`open_existing`, no step: `trace`, `impact`, `flow --trace`) reads a schema-9 queue's cards, `tasks` `NULL`; `stored_rows` reads its 24 columns (`card_columns(9)`), `tasks` `None`.
- **Backup** format 4 (`queue-backup.md` "Format"): schema 10, card rows of 25 columns. Format 3 (schema 9, 24) still imports, `tasks` `NULL`, re-exported as `"tasks":null`; a schema-9 DB exports as format 4 unmigrated. Refused, exit 2: a format-4 card row without `tasks`, a format-3 one with it, a format-4 header of seven keys, `"format":5`.

## Known limits

- **The schema step reaches the gate.** The first open by a new `spec`, `specengine-mcp` or `specengine-http` steps a queue from 9 to 10, which every older binary refuses (`SchemaTooNew`): an older `spec` exits 2, daemon answers 503, MCP server fails its tools. The hook's older `spec gate` gives "no verdict (... newer schema)", so `selective` refuses every controlled write until `spec` and the plugin reinstall from one commit (`gate.md` "Verdict"). An older `spec import-state` refuses format 4. Reads that run no step leave a queue at 9: until a queue command steps it, Health's row 4 fails and `spec flow --trace` notes `the queue stands at schema 9; a queue command upgrades it`.

  The owner's upgrade:
  1. With the old build, in each project: `spec export state --out <file>`. This format-3 dump is the way back.
  2. Stop `spec serve`, and quit Claude Code in every project using SpecEngine.
  3. In the clone at the release commit: `git -C <clone> pull`, then `pnpm --dir <clone>/ui install --frozen-lockfile && pnpm --dir <clone>/ui build`, then `cargo install --locked --force --path <clone>/crates/specengine-cli`, the same for `crates/specengine-mcp`, and the same for `crates/specengine-http`.
  4. `claude plugin marketplace update specengine`, then `claude plugin update specengine@specengine` in each project with the plugin; restart Claude Code.
  5. `spec serve` again.
  6. In each project, open the Board or run `spec card list` (exit 0): either steps the queue to 10. With `[gate]`, `spec gate --path <a controlled file>` gives a verdict, not "no verdict".

  The way back: move `<slug>.db` aside, then `spec import-state` the step-1 dump with the old build; whatever changed since is lost. A hub is untouched until it runs this build; a format-4 dump goes only to a hub on this build or later.
- **Two stores on a hub.** A hub card's `T-0042` may name a different task on each terminal, and a hub that imported a terminal's dump holds that terminal's tasks; so the hub checks and reads none, and says the state lives on the terminal.
- **Read cost.** A card with 32 links reads 32 tasks on each open and each `card.*`, `task.*` and `proposal.applied` event; only the open card is re-read.
- ADR-0056's link proving code work stays unbuilt; nothing lists a task's cards.

## Tests

CLI `card_tasks.rs`, `gate.rs` (the gate reads no link), `queue_state.rs`; store `queue_cards.rs`, `queue_state.rs`, `queue_gate.rs`; mcp `mcp_cards.rs`, `mcp_hub.rs`; http `cards.rs`, `hub.rs`, `hub_mcp.rs`, `daemon_keys.rs`; UI `src/board/CardTasks.test.tsx`, `CardModal.test.tsx`, `CardPage.test.tsx`, `src/api/queries.test.tsx`.
