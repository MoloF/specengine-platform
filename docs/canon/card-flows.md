---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card flows: config, hand-off and return

ADR-0056, ADR-0074, ADR-0076; `[card]` built by `card-store`, the hand-off and the return by `board-handoff` (RM-0085), a flow's `title` and the hand-off by department by `departments` (RM-0096). The commands, the record and the store: `cards.md`; the routes: `board-ui.md`; the screen: `board-screen.md`. Code: core `card_toml.rs` (the config), `card.rs` (the rules); store `queue/cards.rs`; CLI `card.rs`; MCP `cards.rs`, `hub.rs`; http `app.rs`.

## Config

Core `CardConfig::from_toml` (`flow_toml.rs`' family), the first error by line; `ProjectConfig` name-checks `card`. A bad `[card]` exits 2 `<config>:<line>: <message>` for `spec card` alone, is the card routes' 503 (`board-ui.md` "The door") and fails no other command, finding or gate; `spec workspace` serves its flows unjudged with the note (`workspaces.md` "Config").

```toml
[[card.flow]]
name = "copy"
title = "Copy desk"
disciplines = ["copy"]
columns = ["draft", "edit", "proof", "filed"]
accept = { from = "proof", to = "filed" }
moves = [
  { from = "draft", to = "edit", by = ["desk"] },
  { from = "edit", to = "proof" },
]
```

- `[card]` holds `flow`, at most 32, and `split` (`card-split.md` "Config"). A flow's keys (`FLOW_KEYS`, 8): `name`, `title`, `disciplines`, `columns`, `titles`, `accept`, `return`, `moves`. Names (`name`, `columns`, `disciplines`) `[a-z0-9_-]`, 1-64 bytes, each once in its list; 2-32 columns. `titles` any script, at most 64 bytes, default the ID.
- **`title`**, optional, a display name: any script, 1-64 UTF-8 bytes, no control character (`title_problem`, shared with `workspace_toml.rs`); absent, the name; served after `name` in `list`'s `flows[]`, the text output unchanged. Each error at the value's line; `titles` of another type keeps `` `titles` is a table, not <type> ``, an unknown key's message lists the eight:

  ```text
  `title` is a string, not a table: column titles go in `titles`
  `title` is a string, not <an integer | a float | a boolean | a datetime | an array>
  `title` is <N> bytes long; a title has 1 to 64
  `title` holds a control character
  `titles` is a table, not a string: a flow's own title is `title`
  ```
- `accept` required, two columns; `to` is reached only by an accept, `spec card accept` or `POST .../accept`, so it may not be the first column, and a move at it, either end, is a config error.
- `moves` `{from, to, by?}` of two columns, each pair once. No `by`, or `by: []` as served: anyone. No `moves`: every ordered pair of the other columns, expanded by the reader, so moves are looked up, never judged.
- **A new card** takes the first column of the first flow listing its discipline, else of the one flow listing none, else exit 1 naming why.
- **Default**, no `[[card.flow]]`: `PLAIN_FLOW`, TOML through this reader, so pasting it changes nothing: `plain`, titled `plain`, columns `backlog`, `ready`, `in-progress`, `review`, `done`, titled in words, `accept` `review` to `done`, no `moves`, no `return`, `--discipline` free.

### `return`

Optional: the one column of the flow a move enters only with a note. Absent, or `null` as served: none. Core `CardFlow::return_column`, `is_return`; served as `return` after `accept` in `list`'s `flows[]`. The core knows a column whose entry needs a note, never its name (ADR-0008). Each error at its line:

```text
<config>:<line>: `return` is a string, not <a type>
<config>:<line>: `return`: `<k>` is no column of this flow
<config>:<line>: `return`: `<k>` is the first column, where a new card starts
<config>:<line>: `return`: `<k>` is `accept.from`, where a card waits for its accept
<config>:<line>: `return`: `<k>` is `accept.to`, reached only by an accept
```

Each preset's return column is second in `columns` (`presets.md`): gamedev's `sent-back`, software's `reopened`:

```toml
columns = ["backlog", "sent-back", "ready", "in-progress", "in-review", "verify", "done"]
accept = { from = "verify", to = "done" }
return = "sent-back"
```

<a id="the-rule"></a>
## The rule

A hand-off is how one trade asks another for work: a child card, its `parent` the asking card, in the receiving trade's flow, which its discipline picks as for any new card ("Config"). Its `reviewer`, the asking side, is named when the card is created, and may name its mandate: binding, or its department decides (`card-mandate.md`, ADR-0079). A hand-off never moves a card across flows, which stays refused. The receiver takes the card by moving it out of its flow's first column: there is no "take" act. "Accept" stays the asking side's word and act: the child's reviewer accepts its result, never its assignee (ADR-0056).

A return sends work back. It is a move into the one column a flow names as `return`, and it needs a reason, a note that the move's `card.moved` event keeps. Only the child moves. The asking card stays where it is and shows its children as handed off, taken, returned or accepted.

No card write moves, creates or refuses another card; a split (`card-split.md`, ADR-0078) creates a parent and its subtasks in one act and writes no stored card. A parent waits on no child and a child on no parent: each moves, is accepted and is cancelled on its own. A required reason is a transition rule met in the same act, so nothing is blocked (ADR-0012). ADR-0074.

## Hand-off

`spec card new --title T --discipline D --parent C --reviewer N [--mandate M] [--target REF]... [--as N]`, `create_card {…, parent, reviewer}`, `POST .../cards {…, parent, reviewer, as}`: one write, one `card.created` with `reviewer`, the parent untouched.

- **`--reviewer N`**: `assign`'s checks and texts. Off the person grammar (`cards.md` "Who acts"): exit 1, the daemon's 400. Then core `reviewer_problem`: ``reviewer: `agent` can be no reviewer: a person accepts a result``, `local` alike, exit 1, a 409; the store asks it again under its lock. `--reviewer -` is refused on `new`, exit 1, as `--assignee -` is: ``reviewer: `-` clears a name: a new card has none``. Nothing is written on a refusal.
- **Accept**: the child in its `accept.from`, `--as` its reviewer, never its assignee (`cards.md` "Rules").
- **The dialog** (`CardDialogs.tsx` `HandOffDialog`; an open card, the modal, the page, `h` on a tile), its form chosen on open from the Shell's held workspaces read (`useHeldWorkspaces`, never fetching). Two or more departments whose served `disciplines` (`workspaces.md` "Read") is not empty: a required Department select, served order, no default, "Choose a department", one with none not offered; one with several disciplines adds a Discipline select, its first chosen. Fewer, or the read pending or failed on open: Discipline, a datalist of every served flow's disciplines, a hidden flow's too, for the dialog's life. Title, Goal, Targets prefilled; no assignee, criteria, estimate, due or evidence; Reviewer prefilled with the parent's reviewer, else the acting name unless `agent` or `local` (refused above), else empty (`acting.ts` `handOffReviewer`), editable and required locally, the read-only login on a hub; then the required mandate, no default (`card-mandate.md` "Screen"); the parent fixed. Submit: one `POST .../cards` with `title`, `goal`, `discipline`, `target`, `parent`, `reviewer`, `mandate`, `as`; a polite line names the child and its raw `<flow>/<column>`.

## Return

`spec card move C --to K --note T`, `move_card {…, note}`, `POST .../cards/:id/move {to, before, note, revision, as}`.

- **`--note T`** may go with any move and is kept on that move's `card.moved` as `note` (`null` without one), stored untrimmed: the move's own `history` entry is its only home. Grammar as `cancel --reason`: trimmed non-blank, at most `NOTE_MAX` (4 096) bytes. A blank one exits 1 `note: blank; say why it moves`; an over-long one `over_bytes("note", …)`.
- **Into the return column** from another column with no note: exit 1, a 409 with the write document, its reason the last of `notes`, a refusal for MCP; no event, nothing changed (core counts a blank note as none, which the grammar refuses first):

  ```text
  C-0007 enters `sent-back`, its flow's return column: say why with `--note`; nothing changed
  ```

  A reorder inside it (`from` = `to`) needs none, nor does a move out of it.
- **Order of a move's checks**: closed card, the note's grammar, a stale `--revision`, then core's `card_move` (the flow's move, its `by`, the return note). So a move outside `by` into the return column with no note says `by`'s line, not the note's.
- **A return may ask**, on the page alone: the move, then a question on the card, two writes (`card-questions.md` "Screen").

## Children

`spec card list --parent C`, `list_cards {parent}`, `GET .../cards?parent=C`: the cards whose `parent` is exactly C, with every flow line. None: an empty list, exit 0. C off the card-ID grammar exits 2, a look-alike naming its Latin form; at the daemon that is a 503 with the CLI's line. A second `parent` in the query is a 400 (`query.rs`), `parent` last in its names.

**Rollup** (UI `rollup.ts`, `labels.ts`; nothing served): a card's children are the list's rows whose `parent` is its ID, a cancelled child left out; the work item reads its own children, cancelled ones too (`card-progress.md`). Each child's word, looked up and never judged: accepted when `accepted`; returned in its flow's `return` column; handed off in its flow's first column; taken in any other column. A child whose flow is not served shows its `<flow>/<column>` with no word; a child in a hidden flow still counts.

## MCP and hub

Arguments stay `deny_unknown_fields`, so an older server refuses the new keys. `create_card`'s and `move_card`'s descriptions name `reviewer` and `note`, `list_cards`' names `parent` and `return`, its flows `title` (0.35.0); each hub text ASCII and at most 2 048 B, 18 hub tools. `move_card`'s `note` is free text or null, never a staged choice: `mcp_stage.rs` exempts it from decision-staging's walk, as `edit_card`'s `reason`. Plugin 0.31.0 (`plugin-files.md`); `read-spec` says a hand-off is a child card and a return needs a note.

**On a hub** both acts are plain writes for any login, `as` bound to it (`hub-team.md` "Acts"). Naming a reviewer at creation is what `assign` already allows (ADR-0060 "Cost"). Accepting the child takes a `[team]` login that is its reviewer. Every login reads every reason (ADR-0065 "Cost").

## Known limits

- **Older binaries refuse `return`, `title` and `split`** (before `board-handoff`, `departments`, `department-split`): an older `spec card` exits 2, an older daemon or the `v0.29.1` hub answers 503 on the Board, an older MCP server or plugin fails the card tools (pinned on today's code by an unknown key: CLI `cards.rs` `dp5_an_unknown_flow_key_exits_2_at_its_line`, `ac05`, MCP `a_bad_card_table_stops_the_card_tools_alone`). Add a key only after every terminal, plugin and hub serving the project runs a build that reads it (ADR-0073's rule); the presets' headers say so.
- **History keeps 64 events**: a returned card changed 64 times since has lost its reason; the screen says so.
- **Hub accepts**: a writer must be a `[team]` login to accept a hand-off's result.
- No `return_column` on the card row: the reason lives only in the event (ADR-0074 "Cost").

## Tests

core `workspace_toml.rs` (the key, the five errors, the preset), `departments.rs` (`title`, its five messages); CLI `departments.rs` (`flows[0]` keys, the text byte-equal to 5bec07e's); CLI `card_handoff.rs` on `fixtures/studio-lamp` (hand-off, return, `--parent`, nothing waits on another card); store `queue_cards.rs`; mcp `mcp_cards.rs`, `mcp_stage.rs`, `plugin_files.rs`; http `cards.rs`, `query.rs`, `door.rs`, `hub_team.rs`, `hub.rs`, `daemon_keys.rs`, `no_domain.rs`; eval `workspace_names.rs`; `fixtures/card-press` byte-equal, `return` null. Core `card_move` has no unit test of its own; the store's `board_handoff_by_and_the_flows_moves_come_before_the_return_note` pins the order: `by` and the flow's moves refuse before the return note.
