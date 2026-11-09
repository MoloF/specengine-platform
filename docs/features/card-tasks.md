---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures, docs]
ref: RM-0092, v4 step 3b (08 s2 Phase 7, "v4 scope"); the analyst's requirement analysis of 2026-11-09; the owner's decision 3 of 2026-11-08, Q9 of 2026-11-08 and Q2 of 2026-11-09; W1-W7 working answers, the owner's review open
adrs: [ADR-0075]
---

# card-tasks: the `T-` tasks behind a card (RM-0092, 3b)

## Why

The owner's decision 3 of 2026-11-08: an agent's task shows inside its card, with ADR-0056's `T-` model and the gate kept. Today a card and a task share nothing. Someone who opens a card cannot see whether an agent has a task for it, whether the owner must approve its plan, or whether the spec moved under it; they leave for Tasks, which the default workspaces no longer list (1a), and guess which task is meant. ADR-0056's "Cost" deferred the card-to-task link.

This slice links them: a list of task IDs on the card's own row behind queue schema step 10 and backup format 4. `show`, `get_card` and the daemon serve each linked task's state, read at request time, and the card modal and page get a Tasks block. The link decides nothing (ADR-0075, `docs/canon/card-tasks.md` "The rule"). It is local only (Q2): a hub shows linked IDs without state. The cut: 7 criteria and F at 4-6 pipeline hours.

### Decisions

- **D1** The link is a card column, `cards.tasks`, a JSON array of task IDs, behind schema step 10. It is not an evidence kind (append-only, mixing links with results), not on the task row and not an event payload.
- **D2** Only `edit` links: `spec card edit C --task T...` replaces the whole list in the order given, and a lone `--task -` clears it. `new` takes no `--task` (W1).
- **D3** Each entry is a task ID as `spec task` takes it (`written_task`), listed once (a repeat is refused, not folded), at most 32 (`CARD_TASKS_MAX`).
- **D4** Locally each entry names an existing task: a `tasks` row of the project with that ID, found by ID alone, in any repository of the slug and any state. A corrupt row still exists. The check runs before the write and reads no git. Tasks are never deleted, so the check cannot go stale.
- **D5** One edit is one `card.edited {id, fields, as}`: `fields` gains `tasks`, last in table order, and `revision` rises by one, even when the list is unchanged. There is no new event type (`QUEUE_EVENT_TYPES` stays 25), and the IDs are not in the payload.
- **D6** The read: `show`, `get_card` and `GET .../cards/C` serve `tasks`, the linked tasks in link order, each `{id, status, title, stale}`. Each is read at request time as `spec task list` reads a task of the current repository: `status` and `title` from its row, `stale` its staleness at its compared place (`tasks.md` "Place"). The state is never stored on the card or cached.
- **D7** `status`, `title` and `stale` are `null` together when the task cannot be read here: a corrupt row, no such row, another repository's task (W2), a root in no git repository, or a hub. `stale` alone is `null` when only staleness fails, as in `task list`. Each `null` adds a note that starts with `<id>: `. `show` exits 0 unless the card itself does not read.
- **D8** A card with no link reads as today: no git, any root (`cards.md` "Commands", `open_cards`). Git runs only for a card that holds a link, and only on a terminal.
- **D9** `list`, its rows (21 keys), the tiles and every write's document are unchanged. The board's read stays git-free (W5).
- **D10** On a hub (`specengine-http --public-origin`: its daemon routes and its `/mcp`), `--task` is checked by grammar, repeat and cap alone and kept as written. `show` and `get_card` read no task there. The CLI, the stdio MCP and a local daemon read.
- **D11** The UI writes no link and creates no task (Q9 of 2026-11-08). The block offers commands to copy, built from fixed words and checked IDs only (W4).
- **D12** Only `review` (a plan submitted) awaits the owner. Its row offers `spec task approve <id>` to copy; every other command stays on the task's own screen, one click away (W3).
- **D13** The open card is read again on a `task.*` event, so a plan, claim or completion shows without a reload. The list is not read again.
- **D14** Every new UI string is a catalog key in both languages. The daemon's notes stay English, under `lang="en"` (ADR-0072).
- **D15** Plugin 0.31.0 -> 0.32.0, MINOR: a key a tool takes (`edit_card.task`, `deny_unknown_fields`). `read-spec` stays byte-equal (4 079 of 4 096 B); `edit_card`'s text names `task`.
- **D16** No role runs this build against the owner's data directory. Every test, and the orchestrator's Look, uses a scratch `HOME`, so the owner's queues step to 10 only when he upgrades (Risk 1).

### Questions

**The owner's answers.** Decision 3 of 2026-11-08: an agent task shows inside its card, `T-` and the gate kept. Q9 of 2026-11-08: the UI creates no `T-` task. Q2 of 2026-11-09: local only; a hub card shows linked IDs without state.

**Working answers, the owner's review open:**
- **W1** Only `edit` links, because a task exists before it can be linked.
- **W2** Another repository's task links, since a card serves every repository of the slug, but it reads `null` here, as `spec task show` refuses it here.
- **W3** Only `review` counts as awaiting the owner.
- **W4** No Edit-dialog field for tasks: a person links with the copied command, an agent with `edit_card`.
- **W5** No task on a tile or a list row.
- **W6** On a hub the block's empty state offers no command: a terminal's `spec card edit` would write the terminal's own queue, not the hub's card.
- **W7** The empty state's `spec task new` names the card's targets that are shell-safe, else `REF`.

## Description and interactions

### Surfaces (Rust)

- **Model** `crates/specengine-model/src/card.rs`: `Card.tasks: Vec<String>` and `CARD_TASKS_MAX = 32`.
- **Core** `crates/specengine-core/src/card.rs`: a pure `task_links_problem(&[String]) -> Option<String>` covering the grammar, a repeat and the cap. It is asked by the CLI before the transaction and by the store inside it, as every card rule is (`cards.md` "Rules").
- **Store** `crates/specengine-store/src/queue.rs`: `STEP_10` and `QUEUE_SCHEMA_VERSION` 10. `queue/state.rs`: `CARD_COLUMNS` 25 and `STATE_FORMAT` 4, which still imports 3. `queue/cards.rs`: `CardEdit.tasks: Option<Vec<String>>` (`Some(vec![])` clears), `fields()` with `tasks` last, `'[]'` at insert, and the decode of the column. There is no existence check in the store (D4).
- **CLI** `crates/specengine-cli/src/card.rs`:
  - `CardEditRequest.tasks: Vec<String>` and `CardEditBody.task: Option<Vec<String>>`.
  - `CardShowRequest.git: GitEnv` (the reading repository, as `TaskListRequest.git`).
  - On both requests, `tasks_here: bool`: "this queue holds the project's `T-` tasks". It is `true` for the CLI, the stdio MCP and a local daemon, and `false` on a hub.
  - `CardShowOutcome.tasks: Vec<LinkedTask>`; the document and text lines are in "Data".
  - The task read reuses `task.rs`'s and `package.rs`'s (`same_repository`, `staleness`, `Roots`) through one crate-internal function. A per-task failure becomes a note, never an exit.
  - `--task` joins `asked` and `beside_estimate`: a re-estimate with `--reason` takes no `--task`, as it takes no other field.
- **MCP** `crates/specengine-mcp/src/cards.rs`:
  - `EditCardArgs.task: Option<Vec<String>>`.
  - `CardShowDocument` (`mirror.rs`) gains `tasks`.
  - `EDIT_DESCRIPTION`, the hub's `edit_card` text and `GET_DESCRIPTION` each gain the sentence in "Data". Every hub text stays ASCII and at most 2 048 B.
  - Tool counts are unchanged: 29 on stdio, 17 on a hub.
  - The hub's `get_card` and `edit_card` pass `tasks_here: false`.
- **HTTP** `crates/specengine-http/src/app.rs`:
  - The card read and edit pass `tasks_here: app.hub.is_none()` and the read passes the daemon's `GitEnv`.
  - There is no new route, query name, fence class or stream type, and `door.rs` `CARD_CALLS` stays ten.
- **Plugin** `plugin/specengine/.claude-plugin/plugin.json` 0.32.0; `scripts/deploy/.env.example` 0.32.0 (it must equal `plugin.json`).

### Screen (`ui/src/board/`)

A **Tasks** part (`CardTasks.tsx`, new) in `CardBody` (`CardPanel.tsx`), after Hand-offs, in the modal and on the page alike. It renders from the card read and has no read of its own.

- **A row per link**, in link order, a list:
  - **Status served**:
    - The ID is a link to `#/<p>/tasks/<id>` (`sectionHash`, the ID checked by `TASK_ID`; a failing one is text). Tasks is routed whatever the nav lists (`workspaces.md`).
    - The title, or the word for none.
    - The status by `taskStatusLook`: label and icon, never colour alone; an unknown value gets the neutral badge.
    - With `stale: true`, the `specChanged` mark.
    - In `review`: "Awaiting you" and `spec task approve <id>` with a `CopyButton`. `awaitsOwner(status)` lives in `src/tasks/actions.ts`, the one copy of the transition table, so `src/board/` names no task status.
  - **Status `null`**: the ID as text, "State not read here" (a neutral label with an icon), and the card's notes that start with `<id>: `, verbatim under `lang="en"`.
  - **Every row** shows its own notes the same way (a `stale` that could not be read, say).
- **No link, locally** (no resolved hub login): "No task yet" and two commands, each with a `CopyButton`:
  - `spec task new --nodes <refs> --title "..."`. `<refs>` are the card's targets if each matches `^[A-Za-z0-9][A-Za-z0-9._/-]*$`; with none, or any other, the word `REF`. The title is never inserted.
  - `spec card edit <C> --task T-NNNN`, `<C>` checked by the card-ID form.
- **No link, on a hub** (`useHubLogin()` resolved): "No task yet" and one sentence: a task runs on a terminal, and an agent links one with `edit_card`. There are no commands (W6).
- **Live**: in `src/api/queries.ts`, a `task.*` event reads the project's card queries again (the open card), never the list (D13).
- **Nothing moves**: the part is inside the modal's scrolling body, so BH-1 holds. No status word says "blocked" (ADR-0012).
- **Mock** `src/mocks/cards.ts`: cards linked to tasks in `review`, `in_progress` with `stale: true`, `done`, one `null` with its note, and one card with none.

### Roles, files, order

The contract in "Data" is fixed, so the three roles start together.

| Role | May touch | Does |
|---|---|---|
| rust-developer | `crates/specengine-{model,core,store,cli,mcp,http}/src`, `plugin/`, `scripts/deploy/.env.example` | "Surfaces (Rust)"; plugin 0.32.0 |
| test-engineer | `crates/*/tests`, `fixtures/` | new `crates/specengine-cli/tests/card_tasks.rs`; extends store `queue_cards.rs`, `queue_state.rs`, `queue_gate.rs`; CLI `queue_state.rs`, `gate.rs`; mcp `mcp_cards.rs`, `mcp_hub.rs`, `plugin_files.rs`, `plugin_roles.rs`, `plugin_skills.rs` (0.32.0); http `cards.rs`, `hub.rs`, `hub_mcp.rs`, `daemon_keys.rs`, `spec_ids_hub.rs` (0.32.0); `fixtures/daemon-keys.json`; every `Card { .. }` a test builds gains `tasks`; at ship, the shard pins ("Canon at ship") |
| ui-developer | `ui/**` but `ui/README.md`, `ui/dist` | `src/board/CardTasks.tsx` and `CardTasks.test.tsx` (new), `CardPanel.tsx`, `src/tasks/actions.ts` (`awaitsOwner`) and its test, `src/api/{provisional,queries}.ts` and their tests, `src/mocks/**`, `src/i18n/{en,ru}.ts`, `src/styles/app.css` |

**Order.**
1. The test-engineer writes the Rust tests red from "Data", alongside the rust-developer (about 2 h).
2. The ui-developer works against the mock from the start (about 2 h).
3. Once a side lands, its mutations run: each M once, restored byte-equal (`cmp`), recorded in "Implementation".
4. The orchestrator runs the Look with a scratch `HOME` (D16).

Nobody touches `docs/`, `.claude/` or another role's files.

Code comments cite canon headings that exist now and survive the ship, never this spec's: `card-tasks.md` "The rule"; `cards.md` "Commands", "Store", "Documents and events", "Backup", "Rules"; `tasks.md` "Store", "Backup"; `queue-backup.md` "Format", "Import"; `board-screen.md` "The card modal and page"; `ui/README.md` "Screen rules". `card-tasks.md` "State" goes at ship, so cite none of it. The provisional `CardTask` type cites `docs/canon/cards.md` "Documents and events".

### Canon at ship

- **`docs/canon/card-tasks.md`** goes from stub to whole, under 12 KB. "State" goes and "The rule" stays. It gains:
  - "Link": the flag, D2-D5 and the messages;
  - "Read": the document, the line, the notes, git, the cost;
  - "Screen": the part, commands, hub and live re-read;
  - "MCP and hub";
  - "Store and backup";
  - "Known limits": Risk 1's upgrade steps verbatim, Risks 2 and 3;
  - "Tests".
- **`cards.md`** (11 598 B, 690 left):
  - `edit`'s line gains `[--task T]...` and "Commands" a pointer;
  - "Store" says "Queue steps 9 and 10", has `tasks` in the table, `CARD_COLUMNS` 25 and the column's JSON;
  - "Documents and events" has `tasks` after `evidence` and a pointer;
  - "Backup" says "Format 4";
  - the exit list adds "a task not found".
- **`board-screen.md`** (159 B left): "The card modal and page" adds "**Tasks**, its `T-` tasks (`card-tasks.md` "Screen")" to the body list.
- **`tasks.md`** "Backup" (166 B left): format 4 and schema 10, format 3 now among the imported, and "a format-3 or -4 of seven".
- **`queue-backup.md`** "Format", "Import" (400 B left): `STATE_FORMAT` 4, the header example, `CARD_COLUMNS` 25, a schema 1-9 DB exporting as format 4, and the keys (`3, 4: eight`) and schemas (`10 (4)`) of step 2.
- **`proposal-queue.md`** "Store" (1 B left): `QUEUE_SCHEMA_VERSION` 10, and the row `| 10 | cards.tasks | 46 | card-tasks.md "Store and backup" |`. Make room by trimming the "Connection" bullet by about 60 B.
- **`plugin-files.md`**: current 0.32.0; MINOR's list gains "0.32.0 `edit_card`'s `task`".
- **The shard split.** `docs/index-canon.md` is at 9 771 B before this spec and about 10 040 B with `card-tasks.md`'s line. The spec-writer and the test-engineer make it in one pass, then F runs.
  - The new shard is `docs/index-canon-card.md`, claiming `docs/canon/card*.md` (`cards.md`, `card-flows.md`, `card-tasks.md`, about 750 B).
  - **`specengine.toml`**: the generator entry becomes, verbatim, the block below.
  - **`docs/README.md` "Generated"**: from "Here eight: ..." to "... `docs/index-canon.md` (the rest of", the text becomes as below. That is 10 236 of 10 240 B, measured.

    > Here nine: `docs/index-archive.md`, every Tier 3 line; live, `docs/index-canon-spec.md` (`spec-*`, the commands' canon), `docs/index-canon-proposal.md` (`proposal-*`, the queue's), `docs/index-canon-hub.md` (`hub*`, the hub's), `docs/index-canon-card.md` (`card*`, the cards'), `docs/index-canon.md` (the rest of

  - **The test-engineer** moves the pins: CLI `shards_repo.rs` (`SHIPPING`, `POINTERS`, "eight live shards"), `parity.rs`, store `parity_config/mod.rs` (`INDEX_LIVE_SHARDS` 8, the card claim fourth), `check_parity.rs`.
- **`08-roadmap.md`** row 3b, `RM-0092`, and `CLAUDE.md`'s state line: shipped, plugin 0.32.0.
- **Untouched**: the crate READMEs (each within 20 B of its cap), `hub-mcp.md`, `hub-team.md` (2 B left each), `architecture.md` (no rule changes; ADR-0056's "the gate never reads a card" stands).
- **This spec** is compacted to 3 KB.

```toml
[[generators]]
command = "cargo run -q -p specengine-cli -- export index"
writes  = ["docs/index.md", "docs/index-archive.md", "docs/index-canon-spec.md", "docs/index-canon-proposal.md", "docs/index-canon-hub.md", "docs/index-canon-card.md", "docs/index-canon.md",
           "docs/index-decisions.md", "docs/index-crates.md", "docs/index-roadmap.md", "docs/generated/roadmap.md"]
index   = true
roadmap = "docs/generated/roadmap.md"
gate    = "cargo run -q -p specengine-cli -- check"
# Live shards hold Tier 2 canon (cited by path, found by scope; the `spec-*` command canons, the `proposal-*` queue canons, the `hub*` hub canons and the `card*` card canons first, the first matching claim wins), decisions (read by id), crate READMEs (by path) and plan items: docs/README.md "Generated".
shards  = [{ path = "docs/index-archive.md", tier3 = true },
           { path = "docs/index-canon-spec.md", claims = ["docs/canon/spec-*.md"] },
           { path = "docs/index-canon-proposal.md", claims = ["docs/canon/proposal-*.md"] },
           { path = "docs/index-canon-hub.md", claims = ["docs/canon/hub*.md"] },
           { path = "docs/index-canon-card.md", claims = ["docs/canon/card*.md"] },
           { path = "docs/index-canon.md", claims = ["docs/canon/*.md"] },
           { path = "docs/index-decisions.md", claims = ["docs/decisions/*.md"] },
           { path = "docs/index-crates.md", claims = ["crates/*/README.md"] },
           { path = "docs/index-roadmap.md", claims = ["docs/roadmap/*.md"] }]
```

## Data

### Store

```sql
-- STEP_10, in migrate's one Immediate transaction with every due step; user_version 10
ALTER TABLE cards ADD COLUMN tasks TEXT;
```

`CARD_COLUMNS` 25, with `tasks` last and pinned to `PRAGMA table_info`; `proposal_columns` stays 46. The column holds a JSON array of task IDs: `'[]'` at `new`, written whole by an edit. In a row stepped from 9 it is `NULL`, which reads as `[]`. If the value is bad JSON, not an array of strings, or has an entry that is no task ID, the card row does not read (column `tasks`), as for every column: `show` exits 2 and `list` gives a note (`cards.md` "Store"). Repeats or more than 32 entries in a hand-edited row are read as stored.

### Commands

```text
spec card edit C [new's flags but --discipline, --assignee] [--task T]... [--evidence KIND=VALUE]...
  [--reason T] [--revision R] [--as N]
```

The output line (`edited C-0001: <flow>/<column>`) and the write document `{id, flow, column, position, revision, allowed, notes}` are unchanged. Refusals exit 1, change nothing, and the reason is the last note:
- `` task: no task T-0099; nothing changed `` (locally only);
- `` task: T-0042 is listed twice; nothing changed ``;
- `` task: 33 tasks, at most 32; nothing changed ``;
- `` task: "foo" is no task ID; nothing changed `` (a `-` beside other entries too);
- a closed card, a stale `--revision`, `as` outside `by`: as any edit.

Exit 2: a look-alike (`\u{0422}-0042`, a Cyrillic Te; its Latin form named, as `written_task`), or `T` claimed by `[ids]` (`task_prefix_clash`'s line), the latter only when `--task` is given.

**`show`** prints one line per link, after `evidence`, as a list key does: `tasks: <id> | <status or -> | <title or ->[ | stale]`. With no link it prints `tasks: -`. The title goes through `one_line`, escaped.

```text
tasks: T-0001 | review | Light the north lamp
tasks: T-0002 | in_progress | Wire the switch | stale
tasks: T-0003 | - | -
```

### Documents

The `show --json` and `get_card` document gains `tasks` after `evidence`: `{..., corrects, evidence, tasks, acceptance, ...}`. `fixtures/daemon-keys.json`: `Card` gains `"tasks"` after `"evidence"`, and a new `CardTask` entry is `["id", "status", "title", "stale"]`. A task's notes follow the card's own notes, in link order.

```json
"tasks": [
  {"id": "T-0001", "status": "review", "title": "Light the north lamp", "stale": false},
  {"id": "T-0002", "status": "in_progress", "title": "Wire the switch", "stale": true},
  {"id": "T-0003", "status": null, "title": null, "stale": null}
],
"notes": ["T-0003: a task of another repository of the project `studio-lamp`; its state is read there (`spec task show T-0003`)"]
```

Notes, each starting with `<id>: `:

| Case | `status`, `title`, `stale` | Note after `<id>: ` |
|---|---|---|
| the row does not read | null | `` its row does not read (`<column>`: <reason>); no state shown `` |
| no such row (a restored dump, a hand edit) | null | `no such task in this queue; no state shown` |
| another repository of the slug | null | `` a task of another repository of the project `<slug>`; its state is read there (`spec task show <id>`) `` |
| the root is in no git repository | null | `no state shown: <git's line, as spec task list exits with it>` |
| a hub | null | `a hub reads no task; its state is on the terminal that holds it` |
| staleness unknown or failing (no snapshot, a place, an index) | status and title served, `stale` null | `task list`'s note (`stale unknown: no snapshot (...)`), or the failure's line |

### MCP and HTTP

- **`edit_card {card_id, ..., task}`**: `task` is `[string]` or null, and `["-"]` clears.
  - `EDIT_DESCRIPTION` adds: "task: the T- tasks it links, each an existing task of this queue, listed once, at most 32; it replaces the list, - clears it. The link decides nothing: no move, accept or gate reads it."
  - The hub's `edit_card` text adds: "task: T- IDs as a terminal writes them, listed once, at most 32; it replaces the list, - clears it. This hub holds no task: it checks none and get_card shows them without state."
- **`get_card`**: its document gains `tasks`.
  - `GET_DESCRIPTION`, shared by stdio and a hub, adds: "tasks: each linked task {id, status, title, stale}, read now as spec task list reads it; status null, with a note that starts with its ID, for a task this server cannot read: a corrupt row, another repository, a hub."
  - Its content and `structuredContent` stay byte-equal to the CLI's `show --json`.
- **HTTP**:
  - `POST .../cards/:id/edit` takes `{"task": ["T-0001", "T-0002"]}` (`CardEditBody.task`), plus `as`.
  - `GET .../cards/:id` serves `tasks`.
  - On a hub, edit stays in the plain class: any login, with `as` bound to it.

### UI types, keys

- `src/api/provisional.ts`: `Card.tasks: CardTask[]` and `interface CardTask {id: string; status: TaskStatus | null; title: string | null; stale: boolean | null}`.
- New catalog keys under `board.tasks.*` in `en.ts` and `ru.ts`; task status words reuse `tasks.status.*` and `tasks.specChanged`.
- There is no new `localStorage` key.

## Rules and edge cases

- **R1** WHEN `spec card edit` gets `--task`, the system SHALL replace the card's whole list with the given IDs in order, or clear it with a lone `-`, as one `card.edited` whose `fields` hold `tasks` in table order.
- **R2** WHEN an entry is no task ID, is repeated, is the 33rd, or (locally) names no task row of the project, the system SHALL exit 1 and change nothing. A look-alike, or a `T` claimed by `[ids]`, SHALL exit 2.
- **R3** No gate verdict and no card move, accept, assign, cancel or clear SHALL read `cards.tasks`. No task approve, plan, claim, report, complete or cancel SHALL read a card (ADR-0075).
- **R4** WHEN a card holding links is read on a terminal, the system SHALL read each task now and serve it, or `null` with a note (D7), and `show` SHALL exit 0.
- **R5** WHEN a card holds no link, `show` SHALL run no git and read no task.
- **R6** On a hub the system SHALL read no `tasks` row on a card path, and SHALL keep `--task` IDs as written after the grammar, repeat and cap checks.
- **R7** A closed card (accepted or cancelled) SHALL take no link edit (the existing closed refusal). An accept SHALL leave the links. A correction card (`--corrects`) SHALL start with none.
- **R8** The UI SHALL write no link, create no task, and copy only fixed words and checked IDs or references.
- Edge cases:
  - a `done` or `cancelled` task links and shows its state;
  - one task linked from several cards shows on each;
  - `--task` with other fields is one event;
  - a task's `null` title shows `-`, and the word for none in the UI.

### Risks

- **Risk 1 The schema step reaches the gate (R2 of the analysis).** The first open by any new `spec`, `specengine-mcp` or `specengine-http` steps a project's queue from 9 to 10.
  - Every older binary then refuses that queue with `SchemaTooNew`: an older `spec` exits 2, an older daemon answers 503, and an older MCP server fails its tools.
  - The hook runs `spec gate` from `PATH`. An older `spec` there gives "no verdict (... newer schema)", so under `selective` it refuses every controlled write until `spec` and the plugin reinstall from one commit (`gate.md` "Verdict").
  - An older `spec import-state` refuses a format-4 dump (`format` above its build's).

  **Mitigations:**
  - D16;
  - one commit for the three binaries;
  - the steps below, quoted in `card-tasks.md` "Known limits" at ship and in the release note.

  **The owner's upgrade steps:**
  1. With the old build, in each project: `spec export state --out <file>`. This format-3 dump is the way back.
  2. Stop `spec serve`, and quit Claude Code in every project using SpecEngine.
  3. In the clone at the release commit: `git -C <clone> pull`, then `pnpm --dir <clone>/ui install --frozen-lockfile && pnpm --dir <clone>/ui build`, then `cargo install --locked --force --path <clone>/crates/specengine-cli`, the same for `crates/specengine-mcp`, and the same for `crates/specengine-http`.
  4. `claude plugin marketplace update specengine`, then `claude plugin update specengine@specengine` in each project with the plugin; restart Claude Code.
  5. `spec serve` again.
  6. Check: `spec card list` exits 0. In a project with `[gate]`, `spec gate --path <a controlled file>` gives a verdict, not "no verdict".

  **The way back**: move `<slug>.db` aside, then `spec import-state` the step-1 dump with the old build; whatever changed since is lost.

  A hub is untouched until it runs this build. A format-4 dump goes only to a hub on this build or later.
- **Risk 2 Two stores on a hub (R3 of the analysis).** A hub card's `T-0042` names a different task on each terminal, and a hub that imported a terminal's dump holds that terminal's tasks.
  - **Mitigation**: the hub checks no ID and reads no task (D10), so it shows no state that might be another terminal's.
  - Each hub note says the state lives on the terminal that holds the task, and `edit_card`'s hub text says so too.
  - The block's empty state offers a hub no command (W6).
  - A known limit at ship.
- **Risk 3 Read cost.** A card with 32 links reads up to 32 tasks' compared places (git, and an index refresh per root) on each modal open, on each `card.*` and `task.*` event, and on each apply.
  - **Mitigation**: `Roots` caches per call, and only the open card is re-read.
  - The list and the board stay git-free (D9), and a card with no link costs what it costs today.
  - A known limit at ship; untimed (`scale-eval.md` has no card rows).
- **Risk 4 Older MCP servers refuse `task`** (`deny_unknown_fields`). Plugin 0.32.0 and Risk 1's one commit.
- **Risk 5 Canon caps.** See "Canon at ship".
- **Risk 6 First-load size.** The part is small, against 794.35 of 845 kB after 3a. F prints the size.

## Acceptance criteria

Look = the orchestrator's dated check in Chrome on `spec serve` over a scratch copy of `fixtures/studio-lamp`, with a scratch `HOME`, its tasks made by `spec task new` and `spec task plan`. M = the mutation that must turn the named test red.

- [ ] **CT-1 Link.**
  - **CLI**, `crates/specengine-cli/tests/card_tasks.rs`, on `studio-lamp` with `T-0001` and `T-0002` made by `spec task new`:
    - `spec card edit C-0001 --task T-0001 --task T-0002` exits 0. The row's `tasks` is `["T-0001","T-0002"]`, exactly one new event is `card.edited` with `fields: ["tasks"]`, and `revision` rose by 1.
    - `--task T-0099` exits 1 with `task: no task T-0099`, and `SqliteQueue::dump()` is byte-equal before and after.
    - `--task \u{0422}-0001` (a Cyrillic Te) exits 2 naming `T-0001`. `T-0001` twice exits 1 (`listed twice`). `foo` exits 1. 32 links are stored; 33 exit 1 (`at most 32`). `[ids]` claiming `T` exits 2.
    - `--task -` stores `[]` with one `card.edited {fields: ["tasks"]}`.
    - With `--estimate` and `--reason`: refused as any field beside a re-estimate.
  - **MCP and daemon**: mcp `mcp_cards.rs` `edit_card {task}` and http `cards.rs` `POST .../edit {"task": [...]}` store the same list and event; `["-"]` clears.
  - M1: `fields()` omits `tasks`. M2: the existence check removed.
- [ ] **CT-2 Read.** `card_tasks.rs`, mcp `mcp_cards.rs`, http `cards.rs` and `daemon_keys.rs`.
  - `show C-0001 --json` serves `tasks` after `evidence`, with each entry's status and title. After `spec task plan T-0001`, the next `show` serves `review`, while the card's `revision` is unchanged.
  - After `T-0001` is approved and a target's text is edited on disk, `stale` is `true`.
  - Each of the following gives `status`, `title` and `stale` `null`, the table's note, and `show` exit 0:
    - SQL setting `T-0002`'s `status` to `bogus`;
    - `T-0003`'s `git_common_dir` set to another repository's;
    - a deleted row;
    - a root outside git.
  - The text lines are as in "Data".
  - An unlinked card at a root outside git reads as before, exit 0.
  - `get_card`'s `structuredContent` and `GET .../cards/C-0001` serve the same `tasks` and task notes as the CLI's document.
  - `fixtures/daemon-keys.json` pins `Card` and `CardTask`.
  - `list --json` rows keep their 21 keys.
  - M3: the `same_repository` test dropped (`T-0003` served). M4: a corrupt task row propagated (exit 2).
- [ ] **CT-3 The gate reads no link.** CLI `gate.rs`, in a `[gate] mode = "selective"` scratch project with one controlled path. `spec gate --path P` runs with no task claimed (refused) and with `T-0001` claimed here (passes). Each state runs three times: with no card, with `C-0001` linking `T-0001`, and with that card's `tasks` set by SQL to `not json`, so the card row does not read. In each state, stdout, stderr and the exit code are byte-equal across the three. M5: `open_for_gate` also reads `cards` and fails on an unreadable row.
- [ ] **CT-4 Store and backup.** Store `queue_cards.rs`, `queue_state.rs`, `queue_gate.rs`; CLI `queue_state.rs`.
  - A schema-9 DB made with the step-9 SQL, holding one card, opens at `user_version` 10, with `PRAGMA table_info(cards)` equal to `CARD_COLUMNS` (25, `tasks` last) and the old card reading `tasks: []`.
  - A schema-8 DB reaches 10 in one open.
  - A DB at `user_version` 11 makes `spec card show` exit 2 with the `SchemaTooNew` line naming 11 and 10, nothing changed, and makes `open_for_gate` return no verdict with the reinstall hint. That is the path an older build meets at 10.
  - `spec export state` writes the header `{"format":4,"queue_schema":10,...}` and card rows of 25 columns. Export, import into a fresh `HOME`, and export again give byte-identical dumps, links included.
  - A format-3 dump imports, with `tasks` `NULL`, and re-exports as format 4 with `"tasks":null`.
  - Each of these is refused, exit 2, nothing written:
    - a format-4 header of seven keys;
    - a format-4 card row without `tasks`;
    - a format-3 card row with `tasks`;
    - `"format":5`, which says "upgrade SpecEngine".
  - M6: `STEP_10` left out of `migrate`. M7: format 3 read with 25 card columns.
- [ ] **CT-5 The Tasks part.** `ui/src/board/CardTasks.test.tsx` (new), `CardModal.test.tsx`, `CardPage.test.tsx`, `src/tasks/actions.test.ts`, `api/queries.test.tsx`, `api/daemonKeys.test.ts`, `i18n/catalog.test.ts`, `policy.test.ts`.
  - The modal and the page each show the part.
  - A row's ID links to `#/<p>/tasks/T-0001`, with the title, the status's label and icon, and the `specChanged` mark when `stale` is true.
  - A `review` row alone shows "Awaiting you" and `spec task approve T-0001` with a copy button.
  - A `null` row has no link, shows "State not read here", and shows its note verbatim with `lang="en"`.
  - Empty, locally: "No task yet", then `spec task new --nodes QS-0001 --title "..."`, or `REF` for a manual card or an unsafe target, and `spec card edit C-0001 --task T-NNNN`.
  - Empty, with a hub login: "No task yet" and no command.
  - A `task.*` event refetches the open card once and the list never.
  - Every new key exists in `ru.ts`, and `src/board/` names no task status.
  - **Look** at 1440x900 on a card with three links (`review`, `in_progress` stale, one `null`):
    - the modal and the page show them;
    - the column headers' rects are equal before and after opening;
    - the copied approve command, pasted into a terminal, asks its `[y/N]`;
    - under Russian, the chrome is Russian and the notes English.
  - M8: `awaitsOwner` true for every state. M9: the hub branch removed (commands on a hub). M10: the `task.*` refetch removed.
- [ ] **CT-6 It decides nothing.** `card_tasks.rs` runs on twin scratch projects, identical except that one has `C-0001` linking `T-0001` (`in_progress`).
  - `spec card move` along the flow to `accept.from`, then `spec card accept C-0001 --briefed --as <reviewer>`: each command's stdout, stderr and exit code are equal across the twins.
  - `spec task report` then `spec task complete T-0001` leave the linked card's column, position, `revision`, `updated_at` and events unchanged, with no `card.*` event.
  - `spec task approve` and `claim` outputs are equal across the twins.
  - M11: a move refused while a linked task is not `done`. M12: completing a task moves its cards.
- [ ] **CT-7 Hub.** http `hub.rs` and `hub_mcp.rs`, mcp `mcp_hub.rs`. In a hub-served project whose queue holds `T-0001` (`review`, by SQL), with `C-0001` linking `T-0001` and `T-0042`:
  - `GET .../cards/C-0001`, as a login, serves every entry `null` with the hub note per ID; the hub's `get_card` serves the same.
  - `POST .../cards/C-0001/edit {"task": ["T-0042"]}`, with no `T-0042` anywhere, is stored, and the event's `as` is the login.
  - `["X-1"]`, a repeat, and 33 entries are refused as locally.
  - The hub's `edit_card` text names `task` and says the hub checks none; it is ASCII and at most 2 048 B, with 17 tools.
  - M13: the read ignores `tasks_here` (`T-0001` shows `review`). M14: the edit ignores it (`T-0042` refused).
- [ ] **F Form.**
  - Rust: clippy and rustfmt clean; the full workspace `cargo nextest run`; eval `doc_pointers`, `anonymity`, `ui_policy`.
  - Pins: `PINS` and `SCHEMA_PINS` at 0.32.0, as are `plugin_roles.rs`, `plugin_skills.rs`, `spec_ids_hub.rs` and `.env.example`; `read-spec` byte-equal.
  - UI: `pnpm lint`, `pnpm test`, `pnpm build`, with first-load JS printed and at most 845 kB.
  - `spec check` 0 errors and 0 warnings after the ship's canon and shard split.

## Out of scope

- creating or approving a `T-` task from the UI (Q9);
- a Tasks field in the Edit dialog (W4);
- a link on `new` (W1);
- tasks on tiles, list rows or `list --task`;
- any state on a hub, or hub-unique `T-` IDs;
- a task naming its card;
- any rule reading the link;
- dashboards and counts (RM-0086);
- removing Tasks as a section (the default workspaces already leave it out of the nav, `workspaces.md`);
- timing the read at scale.

## Implementation

Filled in after implementation.
