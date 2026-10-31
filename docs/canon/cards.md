---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp]
owner: owner
reviewed: 2026-11-01
---

# Cards: commands, flows, store

ADR-0056, built by `card-store`: a card is the lighter record of any trade's work, a queue row, never a file or a commit. `T-` stays the code-work task; `spec task approve` takes no `C-`. The project's `[card]` gives each discipline its columns, moves and who makes them; no code reads a column's name. Marks, clearing, verdicts: `spec-trace.md`. Code: model `card.rs`; core `card.rs`, `card_toml.rs`; store `queue/cards.rs`; CLI `card.rs`; MCP `cards.rs`.

## Commands

```text
spec card new --title T [--goal T] [--discipline D] [--target REF]... [--criterion C]...
  [--assignee N] [--estimate E] [--due DATE] [--parent C] [--corrects C] [--as N]
spec card list [--flow F] [--column K] [--discipline D] [--assignee N] [--no-target]
  [--with-cancelled] [--marks]
spec card show C [--history] [--marks]
spec card move C [--to K] [--before C] [--revision R] [--as N]
spec card edit C [new's flags but --discipline, --assignee] [--evidence KIND=VALUE]...
  [--reason T] [--revision R]
spec card assign C [--assignee N] [--reviewer N] [--revision R] [--as N]
spec card cancel C --reason T [--revision R] [--as N]
spec card accept C --as N (--briefed | --departs REF...) [--note T] [--revision R]
spec card clear (C | --group REF) ... --reason T --as N
```

Globals as the queue's. In `edit`, `assign` a value `-` clears, and on `new` it is no assignee; `--target`, `--criterion` replace their list, `--evidence` appends. `--target` resolves as `task new --nodes` through `resolved_list`, stored canonical; a repeat is refused. A manual card has no targets. `move`, `edit`, `assign` with nothing asked: a `Usage` outcome in the library, `{id, reason}`, still exit 2 (`or_usage_error`). Library `card_{new,list,show,move,edit,assign,cancel,accept,clear,clear_group}` (`&Env, &Globals, &Card<Verb>Request`; writers `now`, a person's act a `Consent`); the daemon's `Card*Body`, `in_view`: `board-ui.md`.

`open_cards`, never `open_context`: no git runs, so any root works; `accept --departs` alone reads git.

- **Lines** `created|moved|edited|cancelled|accepted|cleared C-0001: plain/backlog`, `assigned C-0001: <assignee or ->, reviewer <reviewer or ->`. JSON of a write or a refusal `{id, flow, column, position, revision, allowed, notes}`; no such card, exit 1, `{id, reason}`, `no card C-0099`.
- **`list`**, by flow, column (config order), position; a column `[card]` no longer places last in its flow, such a flow last of all, cancelled cards last in their column:

```text
flow <name> | <column> <n> | ...
<id> | <flow>/<column> | <position> | <title> | <discipline> | <assignee> | <due> | <targets>[ | cancelled][ | mark <mark>]
[truncated: <k> of <n> cards not shown]
```

  `-` for none. A line per flow, empty columns too; `<n>` counts the listed cards: a filter moves it, a cancelled card has left its column. No card: flow lines only, exit 0. Filters AND, exact, untrimmed. Each stranded (flow, column) is one note, `<n> card(s) in <flow>/<column>, from <first ID>: ...`. Cut at `OUTPUT_CAP_CHARS`, JSON the printed rows.
- **`show`**: a `key: value` line per document key but `notes`, `-` for null, cut at the same cap, tail `[truncated: <k> of <n> lines not shown]`.
- **`cancel`** takes the card out of its column (`position` 0, the column closes up), `reason` required. Only `--with-cancelled` lists it.
- **`accept`**, **`clear`**, a person's acts (ADR-0057): `spec-trace.md`.

**Exits** as the queue's. 1 refused: no such card, a move outside its flow, a closed card, a cap, a declined prompt, a stale `--revision`. 2 cannot run: `C` in `[ids]`, a look-alike ID (its Latin form named), a bad `[card]`, `--as` or `--revision` off their grammar, no terminal, a corrupt row, a newer schema, nothing asked.

## Who acts

`--as N` is self-declared, recorded and compared with a move's `by`, never verified, so `by` stops a slip, not a person; on a hub the proxy's login acts (`hub.md`). Absent: `local`. A person (`card_person_problem`, no ID: ADR-0057): trimmed, one line, at most 128 bytes, no control character, U+2028, U+2029 or bidi mark, any script and inner spaces, compared exactly. Off it, `--as` exits 2 and `--assignee`, `--reviewer` exit 1; each a 400 at the daemon. A tool acts as `agent`, so `by = ["lead"]` holds against tools, and a flow admits them by listing `agent`. `agent`, `local` are no one's name: refused as a reviewer, at `assign` and `accept`, and as a clearer (`clearer_problem`).

A control point exists only where a flow writes `by`; the default writes none.

## MCP

Five tools, as `mcp-read.md`, acting as `agent`: reads `list_cards`, `get_card`; writes `create_card`, `edit_card`, `move_card`, annotated as `claim_task`. Arguments: the twin's flags in snake case, `C` as `card_id`, no `as`. No tool for `assign`, `cancel`, `accept` or `clear`. `{id, reason}` drops `structuredContent`, as `get_task`. `read-spec` names the five.

## Config

Core `CardConfig::from_toml`, `flow_toml.rs`' family, the first error by line; `ProjectConfig` only name-checks `card`. A bad `[card]` exits 2 `<config>:<line>: <message>` for `spec card` alone and fails no other command, finding or gate.

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
```

- `[card]` holds `flow` alone, at most 32. Names (`name`, `columns`, `disciplines`) `[a-z0-9_-]`, 1-64 bytes, each once in its list; 2-32 columns. `titles` any script, at most 64 bytes, default the ID.
- `accept` required, two columns; `to` is reached only by an accept, `spec card accept` or `POST .../accept`, so it may not be the first column, and a move at it, either end, is a config error.
- `moves` `{from, to, by?}` of two columns, each pair once. No `by`, or `by: []` as served: anyone. No `moves`: every ordered pair of the other columns, expanded by the reader, so a reader looks moves up and never judges them.
- **A new card** takes the first column of the first flow listing its discipline, else of the one flow listing none, else exit 1 naming why.
- **Default**, no `[[card.flow]]`: `PLAIN_FLOW`, TOML through this same reader, so a project pasting it changes nothing: `plain`, columns `backlog`, `ready`, `in-progress`, `review`, `done`, titled in words, `accept` `review` to `done`, no `moves`, `--discipline` free.

## Store

Queue step 8 -> 9 (`proposal-queue.md` "Store"); `events`, `proposals` unchanged. `STRICT`, `TEXT` but `position`:

```text
cards(id PRIMARY KEY, project, flow, col, position INTEGER, title, goal, discipline, targets,
  snapshot, criteria, assignee, reviewer, estimate, due, parent, corrects, evidence,
  acceptance, cancellation, created_by, created_at, updated_at, revision)
```

`CARD_COLUMNS` 24, pinned to `PRAGMA table_info`; `col` is the key `column`, an SQL word. JSON columns: `targets`, `criteria` string lists; `evidence` `[{kind, value, by, at}]`; `acceptance` `{by, at, note, verdict, departs, discrepancy}`, no `verdict` reading `briefed`; `cancellation` `{by, at, reason}`, `reason` <= `NOTE_MAX`; `snapshot` an array of `CardFrozen` (`spec-trace-record.md`), or `NULL`. A card holds no path: every repository of the slug sees one board.

API: `create_card`, `get_card`, `list_cards` (a corrupt row into `unreadable`), `card_history`, `change_card(id, seen, change, actor, now)`, `accept_card`, `list_existing_cards`, each write one `Immediate` transaction, one event; refusals `QueueError::CardRefused`.

- **Fields.** Caps as tasks'; title required, a control character stored, printed escaped; `estimate` `parse_estimate`'s, no unit, no sum; `due` `is_calendar_date`; `parent`, `corrects` existing cards, no `parent` cycle, a card never correcting itself.
- **Evidence.** `--evidence KIND=VALUE`: `url` (`http://`, `https://`), `path` (`<clean relative path>@<blob>`, lower-case hex of any length), `build`, `note`; one line, at most 1024 bytes, 32 entries. Nothing is fetched: a reference, never a binary.
- **Order.** `position` dense from 1 per (flow, `col`). A move leaves its column at `position` 0, closes it up, then opens the target or goes last. `new` goes last; `--before C`, another card of that column, takes its place. Neighbours take `position` alone, no `revision`, `updated_at` or event, so a card passed by loses no compare-and-set. `--to` its own column is a reorder: no flow rule, no `by`. Ties by ID.
- **IDs.** `C-` and 4 or more digits, highest + 1 in the transaction, never deleted or reused. `card_prefix_clash` as `task_prefix_clash`, for `spec card` alone. Unique per `<slug>.db`: a hub must be the one minter, filled by `spec import-state`, never merged.
- **`revision`**: decimal text from `1`, raised by every change.
- **Corrupt** (bad JSON, a bad ID, a non-decimal `revision`, a bad `position`, a column that does not read): `show` exit 2 naming the column, `list` a note, the row unreadable, never a failed list. The gate reads `tasks` alone, so a corrupt card row closes no gate.

## Documents and events

`show`, `get_card`, every key present, absent `null`; `mark` to `correction` `--marks`', which `get_card` always asks: `{id, flow, column, position, title, goal, discipline, targets, mark, on, discrepancy, correction, snapshot, criteria, assignee, reviewer, estimate, due, parent, corrects, evidence, acceptance, cancellation, created_by, created_at, updated_at, revision, allowed, history, notes}`.

- `allowed` `[{to, by}]`, per read: moves only, `by: []` anyone. A closed card, or one whose flow is gone: empty. `accept` is never in it: the board finds it through `accept.from`.
- `history`, `--history` alone (else `null`): the card's last 64 `card.*` events (`HISTORY_MAX`), oldest first, `[{seq, type, at, payload}]`; a payload that does not parse is passed over.

`list`: `{flows, cards, notes, truncated, omitted}`; `omitted` `null` when nothing is cut. `flows` `[{name, disciplines, columns: [{id, title}], moves: [{from, to, by}], accept: {from, to}}]`, `moves` expanded, for empty columns. A row `{id, flow, column, position, title, discipline, targets, assignee, reviewer, estimate, due, parent, corrects, updated_at, revision, accepted, cancelled, mark, on, discrepancy, correction}`, 21 keys; the last four `--marks`' (`spec-trace.md` "The read"), else `null`.

Events, the `as` the acting name: `card.created {id, flow, column, position, assignee, as}`; `card.moved {id, from, to, position, as}`, a reorder `from` = `to`; `card.accepted` the same and `verdict`, `discrepancy`; `card.edited {id, fields, as}`, the changed keys; `card.assigned {id, assignee, reviewer, as}`; `card.cancelled {id, column, as}`; `card.cleared`, `card.reestimated`, `card.refreshed`: `spec-trace-record.md` "Events".

## Backup

Format 3: `queue-backup.md`, `tasks.md` "Backup".

## Rules

Each exit 1, nothing changed; core's, pure, asked twice.

- **Outside the flow**: ``C-0001 is in review: no move to `ready`; allowed: `in-progress`, `spec card accept`; nothing changed``. `accept.to` is never allowed, so this covers it.
- **`as` outside `by`**: ``C-0001 is in ready: the move to `review` is `lead` or `owner`'s, not `local`'s; nothing changed``.
- **Closed**: ``C-0001 is accepted: it takes no write; a correction is a new card, `spec card new --corrects C-0001`; nothing changed``, `cancelled` likewise; `accept` too. `clear` still reaches an accepted card.
- **Stale**: `` `C-0001` changed since this run read it: it is in <column> since <updated_at>; nothing changed ``, whether `--revision` or the run's own read lost.
- **Accept**: the card in `accept.from`; `--as` its `reviewer` (none set: naming `spec card assign`), never its `assignee`.
- **Dropped from config**: a column no longer listed is served last in its flow with a note, `allowed` the flow's first column, anyone's. A flow no longer listed: a note, `allowed` empty, `edit` and `assign` still work.

No lock or task is read, and a proposal only for a departure's state, so a rejected brief stops no card (ADR-0012).
