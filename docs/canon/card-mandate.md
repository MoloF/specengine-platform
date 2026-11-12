---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card mandate: a subtask is binding, or its department decides

ADR-0079, built by `task-mandate` (RM-0104, plugin 0.38.0). Related pages: the card, `cards.md`; a hand-off, `card-flows.md`; a split, `card-split.md`; a card's questions and decisions, `card-questions.md`; a parent's subtasks, `card-progress.md`. Code: model, core `card.rs`; store `queue/{cards,state}.rs`; CLI `card.rs`, `state_file.rs`; MCP `cards.rs`, `mirror.rs`; UI `src/board/{labels.ts,CardPanel.tsx,CardItems.tsx}`.

<a id="the-rule"></a>
## The rule

A card may hold a mandate: `binding` or `open`, shown as "Binding" and "Department decides".
- **Who names it.** The asking side, when it makes a card with a reviewer: a hand-off, or each subtask of a split. A card made without one holds none, as does every card made before.
- **Who changes it.** Only the card's reviewer, by an edit that is one `card.edited`. The reviewer is compared as an accept compares one: on a terminal, a name nothing verifies; on a hub, the login.

**Binding.** The brief stands. Departing from it goes back up as a return with a question, which the work item offers first.

**Open.** The department decides. It records each decision on the task and says why.

**Either way:**
- A decision on the task can still be recorded (ADR-0012).
- The asking side sees its subtasks' decisions on the parent, a departure from a binding task marked as such. It may object with a question.
- Confirming an answer stays where it is (ADR-0035, ADR-0061).

**A mandate decides nothing.** None of these reads it:
- no move, accept, assign, cancel, `allowed`, rollup or Ready to accept;
- no stage, approve, reject or confirm;
- no check and no gate.

On the page it only orders the acts and frames the dialogs. The store keeps a decision's `why` optional on any card. ADR-0079.

## The field

`mandate` on the card row: `binding`, `open` or null (no word, the page as before). Model `Mandate::{Binding, Open}`: serde `lowercase`, `ALL`, `as_str`, `parse` exact (`Binding`, ` open`, `-` none); `Card.mandate` after `reviewer`. On screen "Binding" and "Department decides", never "Approved" or "Open", which are status words; in Russian the field reads "who decides", binding "the one who set the task", open "the department".

## Messages

Each a refusal: exit 1, a 409, an MCP error result, nothing written. Core, pure: `mandate_problem(mandate, reviewer)`, `mandate_change_problem(id, reviewer, actor)`, `split_mandates(given, children)`, one per child in `children` order; the store asks the first two again under its lock.

| Rule | Text |
|---|---|
| a mandate, no reviewer (`new`) | `` mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer `` |
| `edit --mandate`, no reviewer | `` mandate: C-0007 has no reviewer, who alone changes its mandate: name one with `spec card assign C-0007 --reviewer N` first; nothing changed `` |
| `edit --mandate`, another name | `` mandate: only C-0007's reviewer `Ana Ruiz` changes its mandate, not `Ben Ode`; nothing changed `` |
| split, an entry off `D=M` | `` mandate: `sound` is not `<discipline>=binding` or `<discipline>=open` ``, the entry one line, cut at 72 B (`MANDATE_ENTRY_ECHO_MAX`) |
| split, `D` off the name grammar | `mandate: ` then `name_problem`'s text |
| split, `D` no `--child` | `` mandate: `voice` is no --child of this split `` |
| split, `D` twice | `` mandate: `art` is named twice `` |

Usage, exit 2, a 400: `new`, `edit` `--mandate` off `binding`/`open` (clap's possible values; a body fails decoding), `-` too: no clearing. `split --mandate` without `--child`: `` --mandate names a subtask: give its --child too ``, before the no-`--child` line.

## Store and backup

Queue schema 12: `STEP_12` `ALTER TABLE cards ADD COLUMN mandate TEXT`, in `migrate`'s one transaction with every due step; stepped rows `NULL`; no index. `CARD_COLUMNS` 26, `mandate` last; `card_columns(schema)` 24 at 9, 25 at 10-11, 26 at 12; `stored_rows`, `open_existing` on 10-11 read `None`, stepping nothing. Another value makes the row corrupt, naming `mandate`; the gate reads `tasks` alone.

`insert_card` asks `mandate_problem` again; `change_card` asks `mandate_change_problem` of the current row and `actor` inside the edit's `Immediate` transaction, whatever else it holds: refused, no field, no event, `revision` unchanged. `CardEdit.mandate`, `fields()` after `tasks`. `assign` leaves it.

`subtask_items(parent) -> CardItems`: one read, one statement, by ID number, the items whose `card_id` is a card with `parent` exactly `parent` (cancelled children too, no grandchild), decoded as `card_items` (`card-questions.md` "Read"), a bad one into `unreadable`.

Backup from format 6 (`queue-backup.md` "Format"): `{"format":6,"queue_schema":12,…}`, a card row of 26 columns. Formats 3-5 (schemas 9-11) import with `mandate` `NULL`. Refused, exit 2: a format-6 card row without `mandate`, a format-5 one with it, format 6 not at 12, format 7 (`… upgrade SpecEngine`). A schema 1-11 DB exports as format 6, schema 12, unmigrated.

## Commands

```text
spec card new ... [--reviewer N] [--mandate binding|open] ... [--as N]
spec card edit C ... [--mandate binding|open] [--revision R] [--as N]
spec card split ... [--child D]... [--mandate D=binding|D=open]... [--reviewer N] [--as N] [--dry-run]
```

- **`new`**: `mandate_problem` after the reviewer's checks (`card-flows.md` "Hand-off"); a refused card takes no ID.
- **`edit`**: today's checks, then the store's: the acting name equal to the stored `reviewer`, exactly, as `accept`'s. One `card.edited`, `revision` + 1, the held value too. A re-estimate takes no `--mandate`.
- **`split`**: `split_mandates` at step 2's fields (`card-split.md` "Command"), a dry run too; an unnamed child holds null, a new parent none, a stored one is never written.
- **Text.** `list`: a suffix `[ | mandate <m>]` after `<targets>`, only when set. `show`: `mandate: <m>` (`-`) after `reviewer:`; after `items:` a line per subtask item, `subtask_items: C-0003 | PR-0014 | decision | open | <summary>[ | record DEC-0031]`, else `subtask_items: -`. `split`: a child's line ends ` | mandate <m>` when set.

## Documents and events

- **The card document**, 35 keys: `mandate` after `reviewer`; `subtask_items` after `items`, `[]` when none, each `card_id` then `items`' 18 keys, an unreadable row left out with `items`' note.
- **A `list` row**, 22 keys, `mandate` after `reviewer`; **a split child**, 9 keys, `mandate` last, a dry run's too.
- **`card.created`** always carries `mandate`, `null` when none: `{id, flow, column, position, assignee, reviewer, mandate, as}`. **`card.edited`** adds it only when `fields` hold it: `{id, fields: ["mandate"], as, mandate}`; any other edit keeps `{id, fields, as}`.
- **Key sets** (`fixtures/daemon-keys.json`, 122): `CardRow` 22, `Card` 35, `CardSplitChild` 9, `CardSubtaskItem` 17.

## Bodies and tools

POST `.../cards` takes `mandate` after `reviewer` (`null` none), `.../cards/:id/edit` after `task` (`null` unchanged), `.../cards/split` after `child`, `["art=binding", "sound=open"]`. 400 no decode or `split`'s usage; 409 a refusal, the document, its reason last of `notes`. No new route, tool or event type: `CARD_CALLS` 12, tools 30 and 18, a hub's plain writes 7. On a hub `as` is the login: a mandate edit stands for the reviewer's login, another gets the 409, an `as` naming another a 403.

MCP: `create_card`, `edit_card` take `mandate` as `MandateArg`, the model's type wrapped with a schema from `Mandate::ALL` (the model holds no `JsonSchema`): enum `binding`, `open`, nullable; `split_card` a nullable `D=M` list; outputs `CardMandateShape`, `CardSubtaskItemShape`. One argument struct serves stdio and a hub; stdio acts as `agent`, never a reviewer, so its `edit_card` with `mandate` is always refused. Texts: stdio `get_card` 1 541 B, `create_card` 1 716, `split_card` 1 749, `edit_card` 1 703; a hub's 1 769, 1 764, 1 710. `INSTRUCTIONS`, `HUB_INSTRUCTIONS`, `read-spec` unchanged.

## Screen

Only `labels.ts` compares a mandate (`policy.test.ts`): `leadActs` (binding Return and Ask, open Record), `asksOnReturn`, `mandateWord`, `mandateMark`. The daemon judges every write.
- **The choice**, a required `SelectField`, "Choose…" (never sent), Binding, Department decides; no default. **Hand off**: after Reviewer; none chosen, `board.mandate.needed`, nothing posted. **Split**: one per checked row (`card-split.md` "Screen"). **Edit**, a card with a reviewer: the acting name (a hub's login) equal to it gets the select at the stored value, a change sending `mandate`; another, a mandate set, its own line "Only the reviewer, <name>, changes the mandate." New task has none.
- **The work item** (`ui-design.md` "The work item"): `leadActs`' acts, when shown, right after Accept, the rest as before; a note before Goal; Details' Mandate after Reviewer, an icon (binding an anchor, open a fork) and the word; Activity's "mandate <word>", an unknown value raw under `lang="en"`.
- **Return** on a binding card opens "Ask a question with it" expanded. **Record a decision…**: binding, a note that it departs from the brief; open, Why required by the form alone, nothing posted without.
- **Decisions from subtasks** (`SubtaskDecisions`, the items' lazy chunk, after Subtasks when `subtask_items` holds a decision): per decision, served order, the child's `CardLink`, title and flow title (`useChildren`, no new read), a lozenge (binding `warning` "Departure from a binding task", open `neutral`), `ItemBlock`, then Ask about it…: `AskDialog` on the child, "About PR-0014: " prefilled. Questions show on the child and in the Inbox. Answer… and Ask about it… stay off while a card write is out.

Words `board.mandate.*`, `board.split.mandate*`, `board.history.mandate`, `board.fromSubtasks.*`; `words.test.ts` bans status, verdict and mark words and a value repeating the field's word. Mock: C-0013 binding, C-0019 open, under C-0001.

## Known limits

- **Rollout.** Older binaries refuse schema 12 (`spec` exit 2, a daemon's queue routes 503, MCP's queue tools fail), format 6 and a `mandate` key or flag; the hook's gate gives no verdict, so `selective` refuses controlled writes until reinstall. Every terminal, plugin and hub upgrades from one commit (`card-tasks.md` "Known limits"' steps); the old build and the last format-5 dump are the way back.
- **The hub's upgrade**, on the owner's word (`hub-ops.md` "Upgrade"): on `v0.36.0`, `dump-queue queue once`, `dump-door door once`, `backup upload once`, dumps copied off the host when no `backup` service runs; then the new tag, `SPECENGINE_VERSION`, `pull`, `up -d`, every service healthy (11 to 12 in one `migrate`); then any `[[card.split]]` rows. A format-6 dump fits only this build or later.
- **A slip-stop, not a control.** The compare trusts `--as` on a terminal (ADR-0057); anyone may `assign` themselves reviewer first, both writes in the history. With no reviewer nobody changes it.
- **The departure mark** reads the child's current mandate, not the one it held when the decision was recorded.
- **Health's row 4** fails on a schema-11 queue, below this build's schema, until a queue command steps it.
- **Echo.** A split `D` off the name grammar echoes `name_problem`'s text, raw line breaks included.
- **Read cost.** One more unindexed scan of `proposals` per card read, never on the Board's `list`; unmeasured.
- Subtasks carry the parent's title, told apart by discipline alone (the owner's Q-C, open). A mandate lives on one queue's card; a hub filled from a dump holds it as stored.

## Tests

`card_mandate.rs` in core (7), store (12), CLI (18), http (6), mcp (4); CLI `queue_state.rs` (format 6, a format-5 dump's round trip); http `daemon_keys.rs` (122); `plugin_files.rs` (`PINS`, `SCHEMA_PINS` 0.38.0). UI `board/{HandOffDialog,SplitDialog,EditMandate,ReturnDialog,CardAsk,SubtaskDecisions,workItem,labels,words,rollup,styles}.test.*`, `mocks/mandate.smoke.test.tsx`, `MockClient.test.ts`, `policy.test.ts`. Criteria: `docs/features/task-mandate.md`.
