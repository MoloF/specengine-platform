---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin]
ref: RM-0045; Phase 7 step 4 (08 s2)
adrs: [ADR-0056]
---

# card-store: cards, flows, events, CLI, MCP (RM-0045)

## Why

One board must carry every trade's work, and `T-` cannot (ADR-0056). This slice is the lighter record: store, flows, CLI, MCP. Step 6's fields stand here without behaviour.

## Description and interactions

### CLI

```text
spec card new --title T [--goal T] [--discipline D] [--target REF]... [--criterion C]...
  [--assignee N] [--estimate E] [--due DATE] [--parent C] [--corrects C] [--as N]
spec card list [--flow F] [--column K] [--discipline D] [--assignee N] [--no-target]
  [--with-cancelled]
spec card show C [--history]
spec card move C [--to K] [--before C] [--revision R] [--as N]
spec card edit C [new's flags but --discipline, --assignee] [--evidence KIND=VALUE]...
  [--revision R]
spec card assign C [--assignee N] [--reviewer N] [--revision R] [--as N]
spec card cancel C --reason T [--revision R] [--as N]
spec card accept C --as N [--note T] [--revision R]
```

Flags as the queue's. In `edit`, `assign` a value `-` clears; `--target`, `--criterion` replace, `--evidence` appends. `--target` resolves as `task new --nodes`; a manual card has none. Nothing asked: exit 2. Library `card_*` as `task_*`, `card_accept` with a `Consent`. No `open_context`, no git: any root works.

Lines `created|moved|edited|cancelled|accepted C-0001: plain/backlog`, `assigned C-0001: <assignee>, reviewer <reviewer>`. JSON of a write or refusal `{id, flow, column, position, revision, allowed, notes}`; no such card: exit 1, `{id, reason}`, `no card C-0099`.

`list`, by flow, column (config order), position, `-` for none:

```text
flow <name> | <column> <n> | ...
<id> | <flow>/<column> | <position> | <title> | <discipline> | <assignee> | <due> | <targets>
[truncated: <k> of <n> cards not shown]
```

The board starts empty: flow lines only, nothing seeded, no `T-`. Filters AND, exact; `--no-target`: manual cards. Cap as `spec flow`'s. `show`: `key: value` lines, cut at `OUTPUT_CAP_CHARS`.

`accept`, a person's act, needs a terminal as `task approve`: piped stdin exits 2 first. Prompt `accept C-0001 (<title>) as <N>? [y/N]`; not `y`: exit 1, no event.

`cancel` takes the card out of its column (`position` 0). Only `--with-cancelled` lists it, last in its column, ending ` | cancelled`.

**Exits** as `tasks.md`'s.

### Who acts

`--as N` is self-declared, recorded in the event, compared with `by`, never verified: until the hub `by` stops a slip, not a person. Absent: `local`. N: `author_field_problem`'s grammar. A tool acts as `agent`, so `by = ["lead"]` holds against tools; a flow admits them by listing `agent`. The hub fills `as` from its header.

### MCP

As `mcp-read.md`. Reads `list_cards`, `get_card`; writes, annotated as `claim_task`: `create_card`, `edit_card`, `move_card`. Arguments: the twin's flags in snake case, `C` as `card_id`, no `as`. No tool for `assign`, `cancel` or `accept`.

**`INSTRUCTIONS`**, 0 B left. After the tasks line, 112 B:

`- list_cards, get_card, create_card, edit_card, move_card = spec card list|show|new|edit|move; no tool accepts.`

Cut, 113 B: ` A REF naming nothing or a call that cannot run is an error result with the reason.`, `; the three above take task_id`. Total 2 047 B.

## Data

### `specengine.toml`

Core `card_toml.rs`, the family of `flow_toml.rs`: `ProjectConfig` only name-checks `card`. A bad `[card]` exits 2 `<config>:<line>: <message>` for `spec card` alone. Fixture `fixtures/card-press`:

```toml
[[card.flow]]
name = "copy"
disciplines = ["copy"]
columns = ["draft", "edit", "proof", "filed"]
accept = { from = "proof", to = "filed" }
moves = [
  { from = "draft", to = "edit", by = ["desk"] },
  { from = "edit", to = "proof" },
]

[[card.flow]]
name = "plates"
disciplines = ["plates"]
columns = ["queued", "blocked", "checked", "struck"]
accept = { from = "checked", to = "struck" }
```

- Names `[a-z0-9_-]`, 1-64 bytes; at most 32 flows, 2-32 columns. `titles = { id = "text" }` any script, 64 bytes, default the ID.
- `accept` required, two columns; `to` reached only by `spec card accept`. An end of `moves` at `accept.to`: a config error.
- No `by`: anyone. No `moves`: every ordered pair of the other columns.
- A new card takes the first column of the flow listing its discipline, else of the one flow with no `disciplines`, else exit 1.
- **Default**, no `[[card.flow]]`: core's `PLAIN_FLOW`, TOML through this reader: `plain`, columns `backlog`, `ready`, `in-progress`, `review`, `done`, titled Backlog, Ready, In progress, Review, Done; `accept` `review` to `done`; no `moves`.

### Store

Queue step 8 -> 9 (`user_version`; the hook's older `spec` gives no verdict: reinstall from one commit); `events`, `proposals` unchanged. `STRICT`, `TEXT` but `position`:

```text
cards(id PRIMARY KEY, project, flow, col, position INTEGER, title, goal, discipline, targets,
  snapshot, criteria, assignee, reviewer, estimate, due, parent, corrects, evidence,
  acceptance, cancellation, created_by, created_at, updated_at, revision)
```

`CARD_COLUMNS` 24; `col` is the key `column`. JSON: `targets` as tasks'; `criteria` strings; `evidence` `[{kind, value, by, at}]`; `acceptance` `{by, at, note}`; `cancellation` `{by, at, reason}`, `reason` as a note; `snapshot` `NULL`, RM-0047's. Each op one `Immediate` transaction, one event, `revision` as tasks'.

- **Fields.** Caps as tasks'; title required. `estimate` `parse_estimate`'s, no unit. `due` `is_calendar_date`. `parent`, `corrects` existing cards, no `parent` cycle.
- **Evidence.** `url` (`http://`, `https://`), `path` (`<path>@<hex blob>`), `build`, `note`; one line, 1024 bytes, 32 entries. Nothing is fetched, never a binary.
- **Order.** `position` dense from 1 per (flow, `col`). `new` and a move go last; `--before C`, C in that column, takes its place; both columns stay dense. A shifted neighbour takes no `revision` or event, so it loses no compare-and-set. `--to` its own column is a reorder, no rule. Ties by ID.
- **IDs.** `C-` and 4 or more digits, highest + 1 in the transaction, never reused; `card_prefix_clash` and the rest as `task.rs`'s. Unique per `<slug>.db`, so the hub must be the one minter, filled by `spec import-state`, never merged; a row holds no path, so a dump moves as it stands.

### Documents and events

`show`: the columns in order, less `project`, plus `allowed` `[{to, by}]`, `history` (`--history`: the last 64 `card.*` events, `[{seq, type, at, payload}]`; else `null`) and `notes`; absent `null`. `list`: `{flows, cards, notes, truncated, omitted}`; `flows` `[{name, disciplines, columns: [{id, title}], moves, accept}]`, `moves` expanded, for empty columns; a row `{id, flow, column, position, title, discipline, targets, assignee, reviewer, estimate, due, parent, corrects, updated_at, revision, accepted, cancelled}`.

`card.created {id, flow, column, position, assignee, as}`; `card.moved`, `card.accepted {id, from, to, position, as}`; `card.edited {id, fields, as}`; `card.assigned {id, assignee, reviewer, as}`; `card.cancelled {id, column, as}`.

### Backup

`STATE_FORMAT` 3: `{"format":3,"queue_schema":9,"project":…,"proposals":p,"tasks":t,"runs":r,"cards":c,"events":e}`; `{"cards":{…}}` rows by number after the runs. Counts gain `<c> card(s)`, `QueueCounts`, `StoredQueue` `cards`; a card alone occupies a queue. Import keeps formats 1, 2, takes 3 at schema 9, refuses a format-3 header of seven keys or another schema, a format-2 of eight, and cards as `tasks.md` refuses tasks. A schema 1-8 DB exports as format 3, unmigrated.

## Rules and edge cases

- WHEN a move is asked THEN pure core `card_move(flow, from, to, as)` decides, and the store again under the lock, exit 1:
  - ``C-0001 is in review: no move to `ready`; allowed: `in-progress`, `spec card accept`; nothing changed``;
  - `as` outside `by`: naming `by`.
- WHEN `--revision` is stale THEN exit 1, tasks' line with `in <column>` for `<status>`.
- WHEN `accept` runs THEN the card stands in `accept.from` and `--as` is its `reviewer`, never its `assignee`; `agent`, `local` can be no reviewer.
- WHEN a card is accepted or cancelled THEN every write on it exits 1 naming `--corrects`.
- WHEN config drops a card's column THEN reads serve it last with a note, `allowed` the first column; its flow: `allowed` empty.
- No column word in code but `PLAIN_FLOW`, so a `blocked` column means nothing. A lead's control point exists where a flow writes `by`; the default has none.
- `spec card` reads no proposal, lock or task: a rejected brief stops no card (ADR-0012). Reads take no `now`: `due` raw.
- The gate reads `tasks` alone; `task approve` takes no `C-`. A corrupt card row: `show` exit 2 naming the column, `list` a note, unseen by task reads.

## Acceptance criteria

- [ ] AC-01 — Seven writes on one `fixtures/card-press` card: `git status --porcelain --ignored` empty, `HEAD` unchanged, 7 `card.*` events. M: a card move writing a file turns `mcp_door` red.
- [ ] AC-02 — A move outside `moves` exits 1 naming the allowed moves, row and events unchanged; `move --revision 1` after a move exits 1, likewise. M: `card_move` dropping the check.
- [ ] AC-03 — `draft` to `edit` without `--as desk` exits 1 naming `desk`, `move_card` too; `--as desk` records it. M: `by` ignored.
- [ ] AC-04 — `accept` piped exits 2; on a pty the assignee, a non-reviewer, `agent` exit 1, the reviewer's `y` moves it; an accepted card refuses every write; MCP sources hold no `card_accept`. M: `move_card` reaching `accept.to`.
- [ ] AC-05 — No `[card]`: one flow line of five zeros, a new card in `plain/backlog`; `PLAIN_FLOW` pasted: `list --json` byte-identical. A bad `[card]`: `spec tree`, `spec check` unchanged. M: a branch on a column word.
- [ ] AC-06 — Two `HOME`s replaying `new`, `move --before`: `list --json`, `export state` byte-identical, a neighbour's `revision` unchanged. M: ordering by `updated_at`.
- [ ] AC-07 — A schema-8 queue opens at 9, rows unchanged; cards round-trip byte-identical; a format-2 dump restores; "Backup"'s refusals exit 2. M: `stored_rows` skipping `cards`.
- [ ] AC-08 — Bad JSON in `cards.evidence`, an `id` `X`: `spec gate --json` as before, `task list` unchanged, `card show` exit 2, `card list` a note. M: `GateQueue::tasks` reading `cards`.
- [ ] AC-09 — U+0421 exits 2 naming the Latin form; `C` in `[ids]` exits 2 for `spec card` alone; `list` works with no `.git`; `--due 2001-01-01` raw, no derived key; evidence `file=x`, a `data:` URL exit 1.
- [ ] AC-10 — Each tool equals its twin; `INSTRUCTIONS` 2 047 B; `copy`, `plates`, `spec-a`, `-b`, `-c` run one code; no fixture word, no `blocked`, in card sources.
- [ ] AC-11 — `cancel` needs `--reason`; one `card.cancelled`, the column closes up, only `--with-cancelled` lists it; later writes exit 1.

## Out of scope

The board and its routes, a page's accept among them: RM-0046. Suspect marks, `snapshot`, clearing, re-estimates, verdicts, a warning on a move to `ready`, and a rollup that ignores cancelled cards: RM-0047.

## Files in scope

- **New:** model, core, CLI `src/card.rs`; core `src/card_toml.rs`; store `src/queue/cards.rs`; MCP `src/cards.rs`; `tests/queue_cards.rs`, `cards.rs`, `mcp_cards.rs`; `fixtures/card-press/`.
- **Changed:** each `lib.rs`; core `project_toml.rs`; store `queue.rs`, `queue/state.rs`; CLI `main.rs`, `state.rs`, `state_file.rs`, `task.rs`; MCP `mirror.rs`, `server.rs`; `plugin.json`, next MINOR.
- **Pins:** `INSTRUCTIONS`, `TOOLS` 28, `queue_schema`, `SCHEMA_PINS`, `mcp_door.rs`, genre scans, `read_only.rs`.
- **Unchanged:** `queue/gate.rs`, `crates/specengine-http`, `ui/`.
- **Canon at shipping:** new `cards.md`; `tasks.md`, `queue-backup.md`, `proposal-queue.md`, `mcp-read.md`; `architecture.md#tasks`: a move to `ready` is a convention until the hub.

## Implementation
