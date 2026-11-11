---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, scripts, fixtures]
ref: RM-0104, v5 step 6d (08 s2 Phase 7, "Owner's vision, 2026-11-11"); the analyst's requirement analysis of 2026-11-11 ("MANDATE", M1-M4, V1-V3); the orchestrator's working answers, the owner's review open
adrs: [ADR-0079]
---

# task-mandate: a subtask is binding, or its department decides (RM-0104)

## Why

The owner, 2026-11-11, translated: "a task that goes down must be approved, or stay open and be decided at the department's own discretion; or the writer sends down an approved task, and it must be refined or sent back to the writer for more work with questions." His vision asks for no shifting of responsibility (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"); his draft case is "a new female character", where the sound designer picks the voice and the rest follows the brief.

Today, after `department-split` (RM-0100):
- **A subtask says nothing about who decides.** Hand off and Split make the child with a reviewer, and that is all. The department reads the brief as it likes.
- **A departure has two doors and no sign.** Return with a question (`card-questions.md` "Screen") and Record a decision… stand side by side. Nothing tells the department which one the writer expects.
- **The writer cannot see the departments' decisions from the parent.** Each child's Activity holds its own items, so the writer has to open each child in turn.

This slice adds the mandate. The asking side names it per subtask: `binding` ("Binding") or `open` ("Department decides"). Only the reviewer changes it. It orders the work item's acts and frames two dialogs, and nothing else reads it. The parent gains "Decisions from subtasks", one read of its children's items. ADR-0079 supplements ADR-0056, ADR-0074 and ADR-0077. Its canon is `docs/canon/card-mandate.md` "The rule", and `card-flows.md` "The rule" names the hand-off's mandate. After this slice comes the owner's stop point: the pipeline trial gains a step where the writer sees an open subtask's decision on the parent.

## Description and interactions

### Decisions

- **D1. The field.** `mandate` on the card row: `binding`, `open` or null. Model `Mandate::{Binding, Open}`. Null on every card made before and on any card made without one.
- **D2. Named at creation by the asking side.**
  - `spec card new --mandate M` (a hand-off, or any card with a reviewer).
  - `spec card split --mandate D=M`, once per subtask.
  - A mandate needs a reviewer, because only the reviewer changes it: `new` without `--reviewer` is refused. A split's children always have one (`card-split.md` "The rule").
  - **No default anywhere.** The Hand off and Split dialogs require a choice. The CLI, the tools and the daemon store null when none is given, so older callers keep working.
- **D3. Only the reviewer changes it.**
  - `spec card edit C --mandate M` is one `card.edited` whose `fields` hold `mandate`.
  - It stands only when the acting name equals the card's stored `reviewer`, compared exactly, as `accept` compares one. The store judges it inside the edit's transaction.
  - No reviewer, or another name: exit 1, nothing changed.
  - No clearing: `-` is not a mandate.
  - `assign` leaves the mandate as it is, whoever the reviewer becomes.
- **D4. Per subtask, not per split.** The owner's own case mixes the two in one split: sound decides the voice, art follows the brief. One choice for all would force a second act per differing subtask, which is what a split exists to remove. No "same for all" control (W3).
- **D5. Binding on the page.** The brief stands, and a departure goes back up as a return with a question:
  - Return… and Ask a question… lead the work item's acts, right after Accept.
  - The Return dialog opens with "Ask a question with it" expanded.
  - Record a decision… stays (ADR-0012). Its dialog says the task is binding, and the parent shows such a decision as a departure.
- **D6. Open on the page.** The department decides:
  - Record a decision… leads the acts.
  - Its dialog requires a why. This is a form rule only: the store, the CLI and `report_decision` keep `why` optional, as ADR-0077 has it.
- **D7. Decisions from subtasks.**
  - The card document gains `subtask_items`: every question, discrepancy and decision naming one of the card's children (`parent` exactly the card, cancelled children included, no grandchild), each `card_id` first. One query.
  - The parent's work item shows its decisions in a new part after Subtasks: a binding child's decision is marked as a departure.
  - The writer may object with Ask about it…, which raises a question on that child.
  - Confirming stays where it is: on a terminal, or by a `[team]` login on a hub (ADR-0035, ADR-0061).
- **D8. Nothing reads it.** No move, accept, assign, cancel, clear, `allowed`, rollup, Ready to accept, Relies on, stage, approve, reject, confirm, check or gate looks at a mandate. On the page it only orders acts and frames dialogs (ADR-0012, ADR-0008).
- **D9. Store.** Queue schema 12, `STEP_12` adds `mandate TEXT`, `CARD_COLUMNS` 26.
- **D10. Backup format 6.** It imports formats 5, 4 and 3 with the mandate `NULL` (precedent: RM-0092's step 10 and format 4).
- **D11. Documents and events.**
  - The card document gains `mandate` and `subtask_items`, and a `list` row gains `mandate` (22 keys).
  - `card.created` carries `mandate` always, `null` when none.
  - `card.edited` gains `mandate` only when `fields` hold it, so every other edit's payload stays as it is.
  - The split document's children gain `mandate`.
- **D12. Words.** Stored `binding` and `open`; on screen "Binding" and "Department decides", never "Approved" or "Open", which are status words.
- **D13. No new surface.**
  - No route, no tool, no event type.
  - `CARD_CALLS` stays 12, the tools 30 and 18, a hub's plain writes 7, `[team]` unread.
  - Three bodies and three tool inputs gain one key each.
- **D14. Plugin 0.38.0**, MINOR: three tools take a key and two return new ones. `read-spec` is unchanged (W10).
- **D15. Rollout.**
  - Schema 12 and format 6 stop older binaries. The hook gives no verdict until `spec` and the plugin reinstall, so `selective` refuses controlled writes meanwhile.
  - The hub (`v0.36.0`, schema 11) upgrades straight to this build, only on the owner's word, with a dump first ("Rules and edge cases", Rollout).

### The contract (Rust lane)

- **Model `card.rs`.**
  - `Mandate::{Binding, Open}`: serde `lowercase`, `JsonSchema`, `as_str`, `parse(&str) -> Option<Mandate>` exact (no trim, no case fold).
  - `Card.mandate: Option<Mandate>` after `reviewer`.
- **Core `card.rs`.** Pure, each `Option<String>` or `Result` a refusal's reason ("Data", Messages):
  - `mandate_problem(mandate: Option<Mandate>, reviewer: Option<&str>) -> Option<String>`;
  - `mandate_change_problem(id: &str, reviewer: Option<&str>, actor: &str) -> Option<String>`;
  - `split_mandates(given: &[String], children: &[String]) -> Result<Vec<Option<Mandate>>, String>`, one entry per child in `children` order.
- **Store.**
  - `queue.rs`: `QUEUE_SCHEMA_VERSION` 12, `pub(crate) const MANDATE_SCHEMA: i64 = 12`, `STEP_12`, run by `migrate` in its one transaction with every due step.
  - `queue/state.rs`: `CARD_COLUMNS: [&str; 26]`, `mandate` last; `card_columns` 9 -> 24, 10 and 11 -> 25, 12 -> 26.
  - `queue/cards.rs`: `NewCard.mandate`, `CardEdit.mandate: Option<Mandate>` (`fields()` adds `mandate` after `tasks`). `insert_card` asks `mandate_problem` again under the lock. `change_card`'s edit asks `mandate_change_problem` against the current row and its `actor`. The two payloads are as "Data" gives them.
  - A new `subtask_items(&self, parent: &str) -> Result<CardItems, QueueError>`, one statement.
  - A stored `mandate` that is not `binding` or `open` makes the row corrupt, naming `mandate` (`cards.md` "Store", Corrupt).
- **CLI.**
  - `card.rs`: `CardNewRequest.mandate`, `CardEditRequest.mandate` (`Option<Mandate>`), `CardSplitRequest.mandate: Vec<String>`; `CardNewBody`, `CardEditBody`, `CardSplitBody` gain `mandate`; the documents and text lines as "Data".
  - `main.rs`: the three flags.
  - `state_file.rs`: `STATE_FORMAT` 6.
- **MCP.**
  - `cards.rs`: `CreateCardArgs.mandate`, `EditCardArgs.mandate` (`Option<Mandate>`), `SplitCardArgs.mandate: Option<Vec<String>>`, and the four texts.
  - `hub.rs`: the hub's three write texts.
  - `mirror.rs`: `CardShowDocument` (`mandate`, `subtask_items`), `CardRowShape` and `CardSplitChildShape` (`mandate`), a new `CardSubtaskItemShape`.
- **HTTP.** No source change is expected: the bodies are the CLI's, and the hub's `acting` already binds `as` to the login. Tests alone.
- **Plugin** 0.38.0: `plugin.json`, `scripts/deploy/.env.example`, `scripts/deploy/tag-check.sh`; the tests' pins.

### The view (UI lane)

- **Types** (`src/api/provisional.ts`):
  - `Mandate = "binding" | "open"`;
  - `CardRow.mandate`, `Card.mandate` (`Mandate | null`, after `reviewer`), `Card.subtask_items: CardSubtaskItem[]` (after `items`);
  - `CardSubtaskItem`, written out in set 122's order;
  - `CardCreateBody.mandate?`, `CardEditBody.mandate?`, `CardSplitBody.mandate?: string[] | null`, `CardSplitChild.mandate`.
  - `daemonKeys.test.ts` lists them in `AHEAD_OF_FIXTURE` until the Rust lane's `fixtures/daemon-keys.json` lands; it is empty at ship.
- **Lookups** (`src/board/labels.ts`): `MANDATE_WORDS`, `mandateWord(mandate, t)`, and `leadActs(mandate): readonly ("return" | "ask" | "decide")[]` (binding `["return", "ask"]`, open `["decide"]`, null `[]`). Nothing outside `labels.ts` compares a mandate.
- **First load** (`CardPanel.tsx`, `CardDialogs.tsx`):
  - the acts' order, the Main note, the Details row and the Activity's word;
  - the choice in `HandOffDialog` and `EditCardDialog`, an existing `SelectField`;
  - `ReturnDialog`'s `askOpen`.
- **Lazy**:
  - the Split dialog's per-row choice (`SplitDialog.tsx`, its chunk);
  - `DecideDialog`'s framing and the new `SubtaskDecisions` part with Ask about it… (`CardItems.tsx`, the items' chunk).
- **Words** (`src/i18n/en.ts`, `ru.ts`): "Data", Catalog words.
- **Mocks** (`src/mocks/cards.ts`, `MockClient`): "Data", Mocks.

## Data

### The field

| Value | Screen word | Means |
|---|---|---|
| `binding` | Binding | the brief stands; a departure goes back as a return with a question |
| `open` | Department decides | the department decides and records each decision with its why |
| null | (none) | today's card: no word, the acts and dialogs as before |

Stored as written, lower-case. `Mandate::parse` takes exactly these two; `Binding`, ` open` and `-` are none.

### Messages

Each is a refusal: exit 1, a 409 at the daemon, an error result at MCP, nothing written. The store asks the first two again under its lock.

| Rule | Text |
|---|---|
| a mandate with no reviewer (`new`, the store) | `` mandate: a mandate goes with a reviewer, who alone changes it: name one with --reviewer `` |
| `edit --mandate`, the card has no reviewer | `` mandate: C-0007 has no reviewer, who alone changes its mandate: name one with `spec card assign C-0007 --reviewer N` first; nothing changed `` |
| `edit --mandate` by another name | `` mandate: only C-0007's reviewer `Ana Ruiz` changes its mandate, not `Ben Ode`; nothing changed `` |
| split, an entry off `D=M` | `` mandate: `sound` is not `<discipline>=binding` or `<discipline>=open` `` |
| split, `D` off the name grammar | `mandate: ` then `name_problem`'s text |
| split, `D` no `--child` | `` mandate: `voice` is no --child of this split `` |
| split, `D` twice | `` mandate: `art` is named twice `` |

Usage (exit 2, a 400): `new` or `edit` with `--mandate` off `binding`/`open` is clap's possible-values error, and a body's off value fails decoding as any typed key. `split` with `--mandate` and no `--child` gives `` --mandate names a subtask: give its --child too ``.

### Store: queue schema 12

```sql
-- STEP_12, step 11 -> 12: every card row stepped holds NULL
ALTER TABLE cards ADD COLUMN mandate TEXT;
```

`CARD_COLUMNS` (26): `cards.md` "Store"'s 25, then `mandate`, pinned to `PRAGMA table_info` by the store's test. `card_columns(schema)` gives 24 for 9, 25 for 10 and 11, 26 for 12, and `None` otherwise. `stored_rows` on a schema 10-11 DB reads `mandate` `None`. `proposals`, `events`, `tasks` and `runs` are unchanged. No index.

`subtask_items(parent)` is one deferred read transaction and one statement, by ID number (`ID_ORDER`), each row decoded as `card_items` decodes, an undecodable one into `unreadable`:

```sql
SELECT <COLUMNS> FROM main.proposals
 WHERE project = ?1
   AND card_id IN (SELECT id FROM main.cards WHERE project = ?1 AND parent = ?2)
 ORDER BY length(id), id
```

Below `CARD_ITEMS_SCHEMA` it reads none, as `card_items` does.

### Backup format 6

`STATE_FORMAT` 6 (`queue-backup.md` "Format"). The header keeps its eight keys: `{"format":6,"queue_schema":12,"project":"<slug>",…}`. A `cards` row holds 26 columns, `mandate` last, a string or `null`.

| Format | Queue schema | Card columns | Imported |
|---|---|---|---|
| 3 | 9 | 24 | `tasks`, `mandate` `NULL` |
| 4 | 10 | 25 | `mandate` `NULL` |
| 5 | 11 | 25 | `mandate` `NULL` |
| 6 | 12 | 26 | as written |

Refused, exit 2, as today's defects (`queue-backup.md` "Import" step 2):
- a format-6 card row without `mandate`, and a format-5 one with it;
- format 6 with `queue_schema` other than 12 (`` ... (it holds queue schema 12) ``);
- format 7 or schema 13 (`… upgrade SpecEngine`).

A schema 1-11 DB exports as format 6, schema 12, unmigrated, its cards' `mandate` `null`. The import's header check takes eight keys for 3-6.

### Commands

```text
spec card new --title T ... [--reviewer N] [--mandate binding|open] ... [--as N]
spec card edit C ... [--mandate binding|open] [--revision R] [--as N]
spec card split ... [--child D]... [--mandate D=binding|D=open]... [--reviewer N] [--as N] [--dry-run]
```

Help texts:
- `new` `--mandate`: `The subtask's mandate: binding (a departure from the brief goes back with a question) or open (its department decides and says why). Needs --reviewer, who alone changes it.`
- `edit` `--mandate`: `Change the mandate: binding or open; only the card's reviewer, acting with --as, changes it.`
- `split` `--mandate D=M`: `A subtask's mandate, D one of the --child disciplines: binding or open; once each.`

Checks:
- **`new`**: after the reviewer's (`card-flows.md` "Hand-off"), `mandate_problem`. Then the store asks it again.
- **`edit`**:
  - in today's order (closed card, grammar, stale `--revision`);
  - then the store's `mandate_change_problem` against the row under the lock, whatever else the edit holds;
  - a re-estimate (`--reason`) still takes no other field (`cards.md` "Commands"), `--mandate` included.
- **`split`**:
  - usage first;
  - then, at step 2's fields (`card-split.md` "Command"), `split_mandates` on the `--child` list;
  - a dry run judges it too, as every rule but the persons.
  - A child named by no `--mandate` holds null. A new parent holds none, and a stored parent is never written.

**Text.**
- **`list`** gains a suffix after `<targets>`, so a card with no mandate prints byte-equal to today:

  ```text
  <id> | <flow>/<column> | <position> | <title> | <discipline> | <assignee> | <due> | <targets>[ | mandate <m>][ | cancelled][ | mark <mark>]
  ```

- **`show`**: `mandate: binding` (`-` for none) after `reviewer:`; one line per subtask item after `items:`; `subtask_items: -` when there are none:

  ```text
  subtask_items: C-0003 | PR-0014 | decision | open | <summary>[ | record DEC-0031]
  ```

- **`split`**: a child's line ends ` | mandate <m>` when set.

### Documents and events

- **The card document** (`show --json`, `get_card`, `GET .../cards/:id`), 34 keys:
  - `mandate` after `reviewer`;
  - `subtask_items` after `items`, `[]` when the card has no child or its children no item;
  - each entry `{card_id, id, kind, status, summary, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title}`, `items`' values (`card-questions.md` "Read") with the child's ID first;
  - an unreadable row is left out with `items`' note.
- **`list` row**, 22 keys: `mandate` after `reviewer`.
- **Split document child**, 9 keys: `mandate` last, the asked value or `null`, a dry run's too.
- **Events.** Payload keys are compared as JSON; the CLI and the store write them as every payload is written.

```json
{"id": "C-0003", "flow": "sound", "column": "backlog", "position": 1, "assignee": null,
 "reviewer": "Ana Ruiz", "mandate": "open", "as": "Ana Ruiz"}
{"id": "C-0003", "fields": ["mandate"], "as": "Ana Ruiz", "mandate": "binding"}
```

  - The first is `card.created`, every card's: `mandate` `null` when none, a split's children each with their own.
  - The second is `card.edited` of a mandate edit. With other fields it reads, for example, `"fields": ["title", "mandate"]`. An edit without `mandate` keeps `{id, fields, as}` exactly.
  - No other event changes.

### Daemon bodies

`deny_unknown_fields` as ever, and each decoder's key list in `decoded` names the new key:

| Route | Body gains | Values |
|---|---|---|
| POST `.../cards` | `mandate` after `reviewer` | `"binding"`, `"open"`, `null` or absent: none |
| POST `.../cards/:id/edit` | `mandate` after `task` | `"binding"`, `"open"`; `null` or absent: unchanged |
| POST `.../cards/split` | `mandate` after `child` | `["art=binding", "sound=open"]`; `null` or `[]`: none |

The answers are as today:
- 200 written;
- 400 a body that does not decode, or `split`'s usage rule;
- 409 the refusals of "Messages", with the write's or the split's document, its reason the last of `notes`;
- 503 cannot run.

On a hub all three are plain writes. `acting` binds `as` to the login, so a mandate edit stands only when the login is the card's reviewer: another login gets the 409, and an `as` naming someone else gets today's 403.

### MCP

Inputs (`additionalProperties: false`):
- `create_card` and `edit_card` take `mandate`, `["string", "null"]`, enum `binding`, `open`;
- `split_card` takes `mandate`, `["array", "null"]` of strings, each `D=M`.

Outputs: `get_card` gains `mandate` and `subtask_items`, `list_cards` rows `mandate`, `split_card` children `mandate`.

Stdio acts as `agent`, which is never a reviewer, so a stdio `edit_card` with `mandate` is always the refusal naming `agent`. One argument struct serves both servers (W9).

Text added to each description (ASCII, each total at most 2 048 bytes, the determinism sentence kept):
- `create_card`: `mandate: binding (a departure from the brief goes back as a return with a question) or open (the department decides and says why); needs reviewer; null: none. Nothing reads it to allow or refuse.`
- `edit_card`: `mandate: binding or open; only the card's reviewer changes it, never agent.` On the hub: `... only the card's reviewer, your login, changes it.`
- `split_card`: `mandate: D=binding or D=open per child discipline, each once; a child without one holds none.`
- `get_card`: `mandate: binding, open or null. subtask_items: the items naming one of its subtasks, each with card_id first, by ID, in one read.`

The developer records every byte count, stdio and hub, under "Implementation". `INSTRUCTIONS` and `HUB_INSTRUCTIONS` stay unchanged.

### Key sets and the plugin

`fixtures/daemon-keys.json` goes from 121 to 122 sets:
- 91 `CardRow`, 22: `mandate` after `reviewer`;
- 92 `Card`, 34: `mandate` after `reviewer`, `subtask_items` after `items`;
- 121 `CardSplitChild`, 9: `mandate` last;
- 122 `CardSubtaskItem`, 17: `card_id`, then 116 `CardItem`'s 16.

Plugin 0.38.0, MINOR:
- `plugin.json`, `.env.example` and `tag-check.sh`;
- the last `PINS` entry, and a `SCHEMA_PINS` entry with the schemas' hash moved;
- `spec_ids_hub.rs`;
- `read-spec`'s body unchanged.

### The page

**The choice**, an existing `SelectField`, required, its first option "Choose…" (empty, never sent), then Binding and Department decides. No default.
- **Hand off** (`HandOffDialog`): after Reviewer, locally and on a hub. Submitting with none shows "Choose whether the subtask is binding or its department decides" in the dialog's alert, and nothing is posted. Chosen, the body carries `mandate`.
- **Split** (`SplitDialog`): one select per checked subtask row, labelled "Mandate for <row's flow title>", absent on an unchecked row. Split stays off until every checked row has one, with "Choose a mandate for each checked subtask". It posts `mandate` as `"<discipline>=<value>"`, the checked rows in row order. The plan read is unchanged (`child: null`, `mandate: null`).
- **Edit** (`EditCardDialog`), only on a card with a reviewer:
  - When the acting name (the typed one locally, the login on a hub) equals that reviewer: the select, preset to the stored value, else "Choose…". A changed value sends `mandate`; an unchanged one sends no key.
  - Otherwise, when a mandate is set: the read-only line "Only the reviewer, <name>, changes the mandate." The daemon judges either way.
- New task has no choice (W11).

**The work item** (`ui-design.md` "The work item"):
- **Acts.**
  - `leadActs` moves the named acts, when the card shows them, to right after Accept, in that order. Every other act keeps today's order.
  - Binding: Return… (when its flow's return column is open to it), then Ask a question… (a card with a target).
  - Open: Record a decision… (a card with a target).
  - Null: the acts group is today's, element for element.
- **Main**: before Goal, a note line with the info icon: "Binding: the brief stands. To depart from it, return the task with a question." or "Department decides: record each decision and why."; none when null.
- **Details**: Mandate, after Reviewer: its word; null folds under More fields.
- **Activity**: an event whose payload holds a non-null `mandate` adds "mandate <word>" to its detail; an unknown value is shown raw under `lang="en"`.
- **Return** (`ReturnDialog`, `askOpen`): on a binding card with a target, "Ask a question with it" starts expanded; the person may still close it.
- **Record a decision…** (`DecideDialog`):
  - Binding: a note above the fields, "This task is binding: a decision here departs from its brief, and its reviewer sees it as one."; why stays optional.
  - Open: the Why field reads "Why (required: the department decides)". A blank one is refused by the form with "Say why: this task's department decides.", and nothing is posted.
  - Null: as today.

**Decisions from subtasks** (`SubtaskDecisions`, the items' chunk, rendered under `Suspense` after Subtasks only when `subtask_items` holds a decision):
- one entry per decision, served order;
- the head line:
  - the child's ID as a `CardLink`;
  - its title and flow title from the parent's children read (`useChildren`, no new read), or the ID alone while that read is pending;
  - the child's mandate from that row: binding gets a `warning` lozenge "Departure from a binding task", open a `neutral` one "Department decides", null none;
- then `ItemBlock` for the item;
- then **Ask about it…**, when the child's row is read and has a target: `AskDialog` for `{id: <child>, targets: <the row's targets>}`, its question prefilled "About PR-0014: ". The raise goes to `.../cards/<child>/items`, a plain write.

Questions and discrepancies of the subtasks are served but not shown here: each stands on its child's Activity and in the Inbox. The parent's card read is read again on every `proposal.*` event, as today, so a new decision appears on its own.

### Catalog words

New keys in both catalogs; English values:
- `board.mandate.*`:
  - `label` "Mandate"; `binding` "Binding"; `open` "Department decides"; `choose` "Choose…";
  - `needed` "Choose whether the subtask is binding or its department decides";
  - `bindingAbout` "Binding: the brief stands. To depart from it, return the task with a question.";
  - `openAbout` "Department decides: record each decision and why.";
  - `reviewerOnly` "Only the reviewer, {{name}}, changes the mandate.";
  - `decideBinding` "This task is binding: a decision here departs from its brief, and its reviewer sees it as one.";
  - `whyRequired` "Why (required: the department decides)"; `whyNeeded` "Say why: this task's department decides.";
- `board.split.mandateFor` "Mandate for {{name}}"; `board.split.mandateNeeded` "Choose a mandate for each checked subtask";
- `board.history.mandate` "mandate {{value}}";
- `board.fromSubtasks.*`: `heading` "Decisions from subtasks"; `departure` "Departure from a binding task"; `ask` "Ask about it…"; `about` "About {{id}}: ".

No value names a trade or reads "Approved" or "Open". Russian is the ui-developer's, in `ru.ts` alone, the owner's review open.

### Mocks

`src/mocks/cards.ts`, harbor-sim:
- C-0013 (`plates`, reviewer Mira Holt, parent C-0001) is `binding`, and C-0019 (`copy`, parent C-0001) is `open`; every other card is `null`. Each of the two gains one of C-0001's targets, since Ask and Record need one, and C-0013's `allowed` holds its flow's return column.
- Two new decisions (the next free `PR-` numbers): one on C-0019 with a why, one on C-0013.
- C-0001's `subtask_items` lists both, by ID.

`MockClient`:
- stores `mandate` on create, split (per child) and edit;
- an edit whose `as` is not the reviewer answers the store's 409 line;
- a `mandate` without `reviewer` on create answers its 409;
- the conflict scenario stays as it is.

## Rules and edge cases

- WHEN `--mandate` is given without a reviewer on `new`, the CLI SHALL refuse it with exit 1 and nothing written, and the next card SHALL take the ID the refused one would have.
- WHEN a mandate edit's acting name is not the card's stored reviewer, or the card has none, the store SHALL refuse it under its lock with no event, `revision` unchanged, and the edit's other fields unwritten.
- WHEN `assign` changes or clears the reviewer, the mandate SHALL stay; with no reviewer nobody changes it until one is assigned.
- WHEN an edit sets the mandate a card already holds, it SHALL be written as any edit: one `card.edited`, `revision` + 1.
- WHEN a split names a stored parent, its row SHALL stay byte-equal; a new parent SHALL hold no mandate.
- WHEN a decision is recorded on a binding card, by the page, the CLI or MCP, it SHALL be stored as on any card; the parent's part SHALL mark it a departure.
- WHEN a decision is recorded on an open card without a why by the CLI or MCP, it SHALL be stored; only the page's dialog requires one.
- WHEN a child is cancelled, its items SHALL stay in the parent's `subtask_items`; a grandchild's SHALL NOT appear.
- WHEN a card holds a mandate and no parent, nothing SHALL differ: the mandate frames the page alone.
- WHEN a stored `mandate` is neither `binding` nor `open`, the row SHALL be corrupt: `show` exit 2 naming `mandate`, `list` a note; the gate SHALL still read `tasks` alone.
- **A slip-stop, not a control.** The reviewer compare trusts `--as` on a terminal (ADR-0057). Anyone may `assign` themselves reviewer first, and both writes stay in the card's history.
- **Two stores.** A mandate lives on one queue's card. A hub filled from a terminal's dump holds its mandates as stored.
- **No core logic in the UI.** The page orders acts by a lookup and sends the person's choice; the daemon judges the reviewer.
- Nothing branches on a discipline, flow, column or kind (ADR-0008); the two stored words are the core's, the screen words the catalog's.
- **Scale.** `subtask_items` is one more scan of `proposals` per card read (`card_id` is unindexed, as `card_items`'). The Board's `list` never reads it. Unmeasured beyond V4's Network panel.

### Rollout

**Older binaries.**
- **Schema.** A build before this one meets a schema-12 queue as `SchemaTooNew`:
  - `spec card`, `spec task` and every queue read exit 2 ("a newer schema");
  - the daemon's card and queue routes answer 503, an older MCP server's queue tools error;
  - the `PreToolUse` hook's gate gives no verdict ("Reinstall spec and the plugin from one commit."), so `selective` refuses controlled writes until `spec` and the plugin reinstall, and `observe` passes with a finding.
- **Format.** An older `import-state` refuses a format-6 dump (`… upgrade SpecEngine`).
- **Inputs.** An older MCP server, daemon or CLI refuses a `mandate` key or flag (`deny_unknown_fields`, clap).
- **The rule** (`card-tasks.md` "Known limits"): every terminal, plugin and hub of a project upgrades from one commit. The old build and the last format-5 dump are the way back.

**The hub** runs `v0.36.0` at queue schema 11. It goes straight to this slice's tag, 11 to 12 in one `migrate`, only when the owner says so (`hub-ops.md` "Upgrade"):
1. On `v0.36.0`, dump first: `dump-queue queue once`, `dump-door door once`, `backup upload once`. With no `backup` service the dumps live on tmpfs alone, so copy them off the host.
2. Check out the new tag, set `SPECENGINE_VERSION`, `pull`, `up -d`; every service healthy.
3. Then any `[[card.split]]` rows the project wants (`card-split.md` "Known limits").

Back: the old tag and the format-5 dump by "Restore". A format-6 dump fits only this build or later.

### Test churn

Only these change; every other test passes unchanged. The test-engineers add what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| tests pinning schema 12 or format 6 as too new: CLI `drift_raise.rs`, `card_tasks.rs`, `stage_confirm.rs`, `gate.rs` (`"schema 12"`), `queue_state.rs` (`"format":6`); http `stage.rs` (12 over 11); store `queue_stage.rs`, `queue_state.rs`, `queue_interpretation.rs` | 13 and 7 (13 over 12), after R1's record | D9, D10 |
| `QUEUE_SCHEMA_VERSION` 11, `CARD_COLUMNS` 25 and format 5 pins: store `format.rs` (`assert_eq!(QUEUE_SCHEMA_VERSION, 11)`), `queue_state.rs` (`(48, 17, 11, 25)`); CLI `queue_state.rs` (the header, 25 columns) | 12, 26, format 6 | D9, D10 |
| the card document's 32 keys and the row's 21: CLI `cards.rs`; http `cards.rs`; mcp `mcp_cards.rs`; http `daemon_keys.rs`, `fixtures/daemon-keys.json`; UI `api/daemonKeys.test.ts` | 34 and 22, 122 sets | D11 |
| `card.created` payloads compared as JSON: store `queue_cards.rs`, `card_split.rs`; CLI `cards.rs`, `card_handoff.rs`, `card_split.rs`; http `cards.rs`, `door.rs`, `hub.rs`, `hub_team.rs`, `card_split.rs`; mcp `mcp_cards.rs`, `mcp_door.rs`, `card_split.rs` | `"mandate": null` added | D11 |
| `show` text goldens (CLI `cards.rs`, `card_handoff.rs`, `card_questions.rs`, `card_tasks.rs`) | a `mandate: -` and a `subtask_items: -` line | D11 |
| the split document's children (CLI, http, mcp `card_split.rs`) | `mandate` last | D11 |
| MCP input schemas and texts: `mcp_cards.rs`, `mcp_hub.rs`, `card_split.rs`; `plugin_files.rs` (`SCHEMA_PINS`) | `mandate`; the new byte counts | D13, D14 |
| `plugin_files.rs` `PINS`, http `spec_ids_hub.rs`, `tag-check.sh` | 0.38.0 | D14 |
| UI `HandOffDialog.test.tsx`, `SplitDialog.test.tsx`, `mocks/split.smoke.test.tsx`, `MockClient.test.ts`, `api/http.test.ts` (bodies) | the choice required; `mandate` in the bodies | V1 |

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**; each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation (M:) turns red. New Rust tests live in `card_mandate.rs` of the crate named (core, store, CLI, http, mcp) unless another file is named. A scratch copy of `fixtures/studio-lamp` (the preset's flows, `sound`'s return column `sent-back`) serves the CLI and http ones. The analyst's M1-M4, sharpened against the code, become M1-M7.

- [ ] M1 — **Named at creation.**
  - Core: `Mandate::parse` reads `binding` and `open` and refuses `Binding`, ` open`, `-` and the empty string. `mandate_problem(Some(Binding), None)` gives "Messages"' first line, and `(Some(Open), Some("Ana Ruiz"))` gives none.
  - CLI, studio-lamp:
    - `spec card new --title "Lamp room scene" --discipline narrative --target SCN-ARRIVAL --as "Ana Ruiz"` makes `C-0001`.
    - `spec card new --title "Keeper's voice" --discipline sound --target SCN-ARRIVAL --parent C-0001 --reviewer "Ana Ruiz" --mandate open --as "Ana Ruiz" --json`: exit 0, `C-0002`. `show C-0002 --json` holds `mandate` `open` right after `reviewer`. The two `card.created` carry `mandate` `null` and `open`.
    - `spec card list`: `C-0002`'s line ends `| mandate open`; `C-0001`'s line is byte-equal to the same list on 8a43e5d. The JSON row holds 22 keys.
    - `--mandate binding` without `--reviewer`: exit 1 with the line, nothing written, and the next `new` takes `C-0003`. `--mandate maybe`: exit 2.
  - Store: `create_card` with a `mandate` and no reviewer gives `CardRefused`, no row and no event.
  - Corrupt: `UPDATE cards SET mandate = 'maybe'`: `show` exits 2 naming `mandate`, `list` gives a note, exit 0.

  M: the store not asking `mandate_problem` (its test stores the card); `card.created` carrying `mandate` only when set; the list suffix printed for null (`| mandate -`).
- [ ] M2 — **Per subtask in a split.** CLI, studio-lamp:
  - `spec card split --target SCN-STORM --title "A new keeper" --discipline narrative --child art --child sound --mandate sound=open --mandate art=binding --reviewer "Ana Ruiz" --as "Ana Ruiz" --json`: exit 0. `C-0001`'s `mandate` is null; `C-0002` is `art` with `binding`, `C-0003` is `sound` with `open`. The three `card.created` carry `null`, `binding` and `open`. The document's children hold `mandate` last, and the text's subtask lines end `| mandate binding` and `| mandate open`.
  - Refused with exit 1 and nothing written, each with its "Messages" line:
    - `--mandate voice=open`;
    - `--mandate art=open --mandate art=binding`;
    - `--mandate art`;
    - `--mandate Art=open`.
  - `--dry-run --child art --mandate art=binding`: the plan's child holds `binding`, and the last event's `seq` is unchanged. `--dry-run --mandate art=binding` without `--child`: exit 2.
  - A child named by no `--mandate` holds `null`. `--parent C-0001 --child sound --mandate sound=open`: `C-0001`'s row and `revision` are byte-equal.

  M: mandates paired by position instead of by discipline; a child without one taking its neighbour's.
- [ ] M3 — **Only the reviewer changes it.** After M1's two cards:
  - `spec card edit C-0002 --mandate binding --as "Ana Ruiz" --json`: exit 0, `revision` 2. Exactly one new event, `card.edited` `{id, fields: ["mandate"], as: "Ana Ruiz", mandate: "binding"}`.
  - Refused with exit 1, no event and `revision` unchanged, each with its line:
    - `--as "Ben Ode"`;
    - no `--as` (it names `local`);
    - `--title X --mandate open --as "Ben Ode"` (the title is unwritten too).
  - `--mandate -`: exit 2.
  - `--title X --as "Ben Ode"`: exit 0, its `card.edited` exactly `{id, fields: ["title"], as}`.
  - `--title Y --mandate open --as "Ana Ruiz"`: one `card.edited`, `fields` `["title", "mandate"]`.
  - After `spec card assign C-0002 --reviewer - --as "Ana Ruiz"`: `mandate` stays `open`, and `edit --mandate binding --as "Ana Ruiz"` gives the no-reviewer line. After `assign --reviewer "Ben Ode"`, Ben's edit stands.
  - A cancelled card gives today's closed line.

  M: the compare reading `assignee`; the check skipped when the edit holds another field; `card.edited` without `mandate`.
- [ ] M4 — **Nothing reads it** (ADR-0012). CLI, studio-lamp:
  - Three hand-offs of `C-0001` in `sound`, alike but for `mandate` (null, `binding`, `open`). Each moves `backlog` -> `design`, returns to `sent-back` with a note, moves to `review` and is accepted by Ana Ruiz. A fourth set of three is cancelled.
  - At every step, `allowed`, `column`, `position` and each write's document are equal across the three but for `id` and `revision`.
  - `spec propose decision --card` without `--why` stores on each, as does `spec propose question --card`.
  - With a `selective` config, `spec gate` on a task's path gives the same verdict before and after the mandates are set.
  - UI `rollup.test.ts`: `progressOf` over rows alike but for `mandate` gives equal progress.

  M: `allowed` dropping the return move on a binding card; `create_intake` requiring `why` on an open card's decision.
- [ ] M5 — **Schema 12 and backup format 6.**
  - Store:
    - a new queue sits at `user_version` 12, `table_info(cards)` equal to `CARD_COLUMNS` 26 with `mandate` last;
    - `card_columns` gives 24, 25, 25 and 26 for 9 to 12, and `None` for 13;
    - a schema-11 DB with three cards steps at its first `open`, rows byte-equal but for `mandate` `NULL`; schema 9 reaches 12 in one `migrate`;
    - a hand-made `mandate` column fails step 12, and the DB stays at 11;
    - `open_existing` on 11 reads `mandate` `None` and steps nothing.
  - CLI `queue_state.rs`:
    - the header `{"format":6,"queue_schema":12,…}`, card rows of 26 columns;
    - a binding and an open card round-trip byte-identical;
    - format-5, 4 and 3 dumps import with `mandate` `NULL` and re-export as 6;
    - refused with exit 2: a format-6 row without `mandate`, a format-5 row with it, format 6 at schema 11, format 7.

  M: `STATE_FORMAT` left 5; step 12 committed apart from the other due steps; a format-5 row read as 26 columns.
- [ ] M6 — **Decisions from subtasks, one read.**
  - Store `subtask_items("C-0001")` lists items naming `C-0002` and `C-0003` by ID number, each with its `card_id`. Not listed: an item naming `C-0001`, and one naming a grandchild under `C-0002`. Listed: one naming a cancelled child. A corrupt one goes into `unreadable`.
  - CLI `show C-0001 --json`:
    - `subtask_items` after `items`, in set 122's order;
    - `[]` on a card with no children;
    - the text lines;
    - `GET .../cards/C-0001` equal to `show --history --json`;
    - `get_card` the same, valid against its `outputSchema`.
  - `list` and `GET .../cards` byte-equal before and after items are raised on the children.

  M: `subtask_items` taking grandchildren; the part's read made once per child (see V3).
- [ ] M7 — **The surfaces.**
  - http, local:
    - `POST .../cards` with `mandate: "open"` and a reviewer: 200; without a reviewer, 409 with the line; `mandate: "maybe"`, 400.
    - `POST .../cards/C-0002/edit` `{mandate: "binding", as: "Ana Ruiz"}`: 200; `as: "Ben Ode"`: 409.
    - `POST .../cards/split` with `mandate: ["sound=open"]`: 200; `["voice=open"]`: 409.
    - `door.rs`: `CARD_CALLS` stays 12, and a mandate edit adds one `card.edited` and nothing else.
  - A hub: any login creates and splits with a mandate. The edit with `mandate` answers 200 to the reviewer's login and 409 to another login; an `as` naming another gives 403 with nothing stored.
  - mcp:
    - stdio: `create_card`, `edit_card` and `split_card` hold `mandate` as "Data", with `additionalProperties: false`;
    - `create_card {…, reviewer: "Ana Ruiz", mandate: "open"}` stores it with `as` `agent`;
    - `edit_card {card_id, mandate}` gives an error result naming `agent`, nothing changed;
    - a hub's `/mcp` takes the same, its `edit_card` acting as the login;
    - every text ASCII, at most 2 048 bytes.
  - `daemon_keys.rs`: 122 sets as "Data".

  M: `mandate` missing from `CardEditBody` (400); the hub's edit acting as `local`; `CardSubtaskItemShape` without `card_id`.
- [ ] R1 — **Rollout on today's code.**
  - Recorded at 8a43e5d before any change, these show what an older build meets after this slice:
    - CLI `gate.rs` (its `selective` config, `user_version = 12`: no verdict, naming `schema 12` and "Reinstall spec and the plugin from one commit.");
    - `stage_confirm.rs`, `card_tasks.rs`, `drift_raise.rs` (schema 12 refused, exit 2);
    - `queue_state.rs` (format 6, `upgrade SpecEngine`);
    - store `format.rs` `SchemaTooNew` (it reads `QUEUE_SCHEMA_VERSION + 1`, so it moves alone);
    - http `stage.rs` (a 503 naming 12 and 11).
  - The same tests, moved to 13 and 7, are green here.
  - Also recorded at 8a43e5d (their output goes under "Implementation"; they are not kept as tests, since this build answers otherwise): `spec card new --title T --mandate open` exits 2 with clap's unexpected-argument line, and stdio `create_card {title, mandate}` is refused as an unknown field.

  M: the gate's newer-schema branch giving a verdict.
- [ ] R2 — **The hub's upgrade, dump first.**
  - CLI `queue_state.rs`: a format-5 dump, exported by this build's library from a schema-11 queue built in the test (two cards, an item with `card_id`), imports into an empty schema-12 queue with every card's `mandate` `NULL`, re-exports as format 6, and re-imports byte-identical.
  - The way back is pinned by R1's format-6 refusal at 8a43e5d.
  - The steps of "Rules and edge cases", Rollout, stand in `card-mandate.md` "Known limits" at ship.
  - The live upgrade is the owner's, on his word alone, and is no gate of this slice.

  M: the format-5 reader taking a 26th card column.
- [ ] P1 — **Plugin 0.38.0.**
  - `plugin_files.rs`: `PINS` and `SCHEMA_PINS` end at 0.38.0, the schemas' hash moved.
  - `spec_ids_hub.rs` and `tag-check.sh` read 0.38.0.
  - `read-spec`'s body is byte-equal to 0.37.0's.

  M: `plugin.json` left at 0.37.0; `SCHEMA_PINS` not appended.
- [ ] V1 — **The choice.** UI, on the mock:
  - **Hand off** (`HandOffDialog.test.tsx`): the select sits after Reviewer, at "Choose…". Submitting with none gives `board.mandate.needed` in the alert, nothing posted. With Department decides, the body holds `mandate: "open"`. The same on a hub, Reviewer read-only.
  - **Split** (`SplitDialog.test.tsx`):
    - a select per checked row, none on an unchecked one;
    - Split off, with `board.split.mandateNeeded`, until each checked row has one;
    - checked `art` binding and `sound` open post `mandate: ["art=binding", "sound=open"]` in row order; unchecking `art` drops its entry;
    - still one plan read.
  - **Edit**: Mira Holt acting on C-0013 sees the select preset to Binding. Another name sees `reviewerOnly`, and a card without a reviewer neither. An unchanged value sends no `mandate`.
  - New task has no select.

  M: a default of `binding`; Split sending an unchecked row's mandate.
- [ ] V2 — **The work item.** `workItem.test.tsx` on the mock:
  - **Acts.** C-0013 (binding): Return… and Ask a question… right after Accept's place, then the rest in today's order. C-0019 (open): Record a decision… there. A null card's acts group equals today's, element for element.
  - **Main and Details**: the note line per mandate; Details' Mandate row.
  - **Activity**: the `card.edited` with `mandate: "binding"` reads "mandate Binding".
  - **Return** (`ReturnDialog.test.tsx`): on C-0013, "Ask a question with it" starts expanded; on a null card it starts closed.
  - **Record a decision** (`CardAsk.test.tsx`):
    - on C-0013, the binding note shows, and a blank why posts `why: null`;
    - on C-0019, the Why label is `whyRequired`; blank gives `whyNeeded` and nothing posted, filled posts `why`.
  - `policy.test.ts`: a scan finds `mandate ===` and `mandate !==` only in `labels.ts`, and its control is red.

  M: the order computed by comparing `card.mandate` in `CardPanel.tsx`; why required on a binding card too.
- [ ] V3 — **Decisions from subtasks.** `SubtaskDecisions.test.tsx` (new), mock C-0001:
  - the part after Subtasks with the two decisions in served order;
  - the children's IDs as `CardLink`s with their titles;
  - C-0013's "Departure from a binding task", C-0019's "Department decides";
  - `ItemBlock`'s fields.
  - Ask about it… on C-0019's entry opens `AskDialog` for C-0019 with its targets and "About PR-…: " prefilled, and posts to `.../cards/C-0019/items`.
  - No part on a card whose `subtask_items` holds no decision.
  - `MockClient`'s call count: one `getCard` for C-0001 and one children list read, never one read per child.
  - After `pnpm build`, the part sits in the items' chunk, absent from first load.

  M: a `getCard` per child; the part listing questions.
- [ ] V4 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, on the mock and on a scratch copy of a real game corpus with the preset's flows. Look at:
  - Hand off and Split with the choice, and a refusal for want of one;
  - a binding and an open work item: the acts, the note, Details;
  - Return's open question, and Record a decision's binding note and required why;
  - Edit as the reviewer and as another;
  - the parent's part with a departure, and Ask about it…;
  - long Russian labels wrapping; at 320 the selects filling the width; focus rings visible, nothing cut unseen;
  - the Network panel: one card read per open of a parent.
- [ ] V5 — **[measure] Size.** After `pnpm build`, first-load JS (`ui-languages.md` "Size") is at most 845 kB. Expected +1 to +3 kB on 818.68 kB (RM-0100), the part lazy, the selects and English words in first load. Record the figure, gzip, the items' and `SplitDialog`'s chunks and `ru-*.js`.
- [ ] G1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full test run green twice.
  - **Rust.** The workspace run green, then `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy --test workspace_names`.
  - **Docs.** The check is clean.

## Out of scope

- The writer approving or confirming a department's decision: confirming stays a terminal's or a `[team]` login's (ADR-0035, ADR-0061).
- A mandate on New task, on the Board's tile, in `list` filters, or as a `[card]` default.
- Any refusal, wait or status by mandate: a binding task's decision is recorded and shown, never stopped (ADR-0012).
- Notifications; the department's Overview counts (RM-0086); a subtask told that its parent changed (RM-0105).
- Clearing a mandate; one per split; grandchildren's items on a parent.
- The live hub's upgrade: the owner's, on his word.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-model/src/card.rs`; `crates/specengine-core/src/{card,lib}.rs`; `crates/specengine-store/src/{queue,lib}.rs`, `queue/{cards,state}.rs`; `crates/specengine-cli/src/{card,main,state_file}.rs`; `crates/specengine-mcp/src/{cards,hub,mirror}.rs`; `plugin/specengine/.claude-plugin/plugin.json`; `scripts/deploy/.env.example`, `scripts/deploy/tag-check.sh` |
| test-engineer, Rust | `card_mandate.rs` in core, store, CLI, http and mcp (new); "Test churn"'s Rust rows; `fixtures/daemon-keys.json`; R1's record at 8a43e5d first |
| ui-developer | `src/api/provisional.ts`, `client.ts`, `http.ts`; `src/board/{labels.ts,CardPanel.tsx,CardDialogs.tsx,SplitDialog.tsx,CardItems.tsx}`; `src/i18n/{en,ru}.ts`; `src/mocks/cards.ts`, `MockClient`; the tests of V1-V3 |
| test-engineer, UI | gates, mutations, V5; the UI rows of "Test churn"; `policy.test.ts`, `words.test.ts`; `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/task-mandate.md` "Acceptance criteria" with a criterion ID, or an existing canon heading. Never cite "Data" or "Description and interactions", which compaction drops at ship (doc_pointers).

### Work order

1. The Rust test-engineer records R1 at 8a43e5d before any change.
2. The rust-developer and the ui-developer start together; the contract is fixed above. The UI runs on the mock, the key sets in `AHEAD_OF_FIXTURE`, until `daemon-keys.json` lands.
3. The Rust test-engineer overlaps the Rust lane: M5 first (schema and format), then M1-M3, M6, M7, M4, R2 and P1, with the churn. Then the Rust review.
4. The UI lane completes; the UI test-engineer runs the gates and mutations (V1-V3, V5). Then the UI review.
5. The orchestrator's look (V4) on the mock and the corpus copy.
6. Ship: the spec-writer ("Canon at ship"). Then the owner's stop point.

At most three iterations a lane.

**The cut line**, if the UI lane runs out:
- **Moves to a new plan item**, `task-mandate-ui` (the next free `RM-` number, `must` in `v5`, before the trial), which the ship's spec-writer creates:
  - V2's acts order and dialog framing;
  - V3's part.

  The trial's new step waits for it.
- **Ships**:
  - the Rust part whole (schema 12, format 6, the flags, bodies and tools, `subtask_items`, plugin 0.38.0);
  - V1 (the choice in Hand off, Split and Edit) and the Details row, so the page can name a mandate.

### Risks

- **Older binaries** (D15): schema 12 stops every older `spec`, daemon, hook and hub on the queue at once; `selective` refuses controlled writes until reinstall. One commit for every terminal, plugin and hub.
- **The hub's upgrade**: tmpfs dumps vanish with the container, so copy them off the host before the new tag.
- **The reviewer compare** stops a slip, not a person: `assign` comes first, and both stay in the history.
- **Event consumers**: `card.created` gains a key; the UI reads payloads by key, never by shape.
- **Size**: 26.32 kB of headroom at RM-0100; the part lazy, the selects reusing `SelectField` (V5).
- **Card read cost**: one more unindexed scan per card read (M6, V4).
- **Catalogs**: every key in both, plural forms where a count appears; no status word.
- **Churn**: the key sets, `card.created` comparisons, show goldens, the schema and format pins, the tool texts and the plugin pins.
- **Caps** at ship: below.

### Canon at ship

`docs/canon/card-mandate.md` loses "State" and gains the rest, at most 12 KB: the field and its messages, the store and backup, the commands and documents, the bodies and tools, the screen, Known limits (the rollout, the hub's steps, the slip-stop, the read's cost), Tests.

Each edit below stays within its cap; free bytes now in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/cards.md` [66] | "Commands": `--mandate` on `new`, `edit`; "Store": step 12, 26 columns; "Documents and events": `mandate`, `subtask_items`, 22 row keys, the two payloads (pointers, net not positive) |
| `docs/canon/queue-backup.md` [7] | "Format": `STATE_FORMAT` 6, the header, 26 card columns, 3-5 importing `NULL` (net not positive) |
| `docs/canon/proposal-queue.md` [5], `docs/canon/tasks.md` [49], `docs/canon/card-tasks.md` [89] | schema 12, format 6 where they name 11 and 5 (net not positive) |
| `docs/canon/card-flows.md` [66] | "Hand-off": the dialog's required choice (pointer, net not positive) |
| `docs/canon/card-split.md` [73] | "Command": `--mandate D=M`; "Screen": the per-row choice; its "no storage change" names schema 12 (net not positive) |
| `docs/canon/card-questions.md` [87] | "Screen": Record's why on an open task; "Known limits" Rollout generalised (net not positive) |
| `docs/canon/card-progress.md` [6 072] | "Screen": Decisions from subtasks after Subtasks (pointer) |
| `docs/canon/ui-design.md` [10], `docs/canon/board-screen.md` [55] | "The work item": the lead acts and the Mandate row; "Writes and dialogs": the choice (pointers, net not positive) |
| `docs/canon/proposal-review.md` | key sets 122 |
| `docs/canon/plugin-files.md`, `docs/canon/plugin.md` [412] | 0.38.0, its keys |
| `docs/canon/hub-deploy.md` [8] | the version it names (net zero) |
| `docs/canon/ui-languages.md` [451] | "Size": the figure |
| crate READMEs (cli [1], store [3], mcp [7], core [9], model [35]) | only where they name the column, schema, format or tools' keys; today none does |
| `CLAUDE.md`; 08 s2 Phase 7 step 6d; RM-0104 | shipped |
| this spec | compacted to at most 3 KB |

`docs/index-canon-card.md` gains `card-mandate.md`'s line at spec time already. `card-flows.md` "The rule" changed with ADR-0079 at spec time.

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner did not answer:
- `mandate` is `binding`, `open` or null, named at creation, required in the two dialogs, `--mandate` needing `--reviewer`; only the reviewer changes it by `edit --mandate`, one `card.edited`; no default.
- Binding: departure as Return with a question, shown first; Record stays, noted binding. Open: Record leads, the dialog requires why, the store keeps it optional.
- The writer sees open decisions on the parent, "Decisions from subtasks", one read of the children's items; may object with a question; confirm stays where it is.
- Nothing reads it. Schema 12, `CARD_COLUMNS` 26, format 6 importing 5, 4, 3; rows and both events carry it. Plugin 0.38.0. The hub upgrades straight to 12, on the owner's word.

**The spec-writer's:**
- W1: a mandate per subtask in a split (D4).
- W2: the split's grammar is `--mandate D=M`, and a body's `mandate` a list of the same strings, as `--evidence KIND=VALUE` is. A discipline holds no `=`.
- W3: no "same for all" control: the departments of a split are few, and the choice is one select each.
- W4: `card.created` always carries `mandate`; `card.edited` only when it changes, so no other edit's payload moves.
- W5: the `list` text gains a suffix only when set, so today's goldens hold.
- W6: no clearing (`-` refused); the same value again is an edit like any.
- W7: `assign` leaves the mandate; a card whose reviewer is cleared keeps it, unchangeable until one is named.
- W8: a split's new parent takes no mandate; a stored one is never written.
- W9: one argument struct serves stdio and the hub, so stdio's `edit_card` takes `mandate` and always refuses it (`agent` is never a reviewer).
- W10: `read-spec` is unchanged (4 086 bytes of 4 096); the tool texts carry the mandate.
- W11: New task has no choice: the mandate is about work that goes down.
- W12: the Hand off and Edit choice reuses `SelectField` in first load: a lazy field inside a first-load dialog would appear a frame late and shift focus order, for a few hundred bytes. Measured by V5.
- W13: `subtask_items` serves every kind, so an agent's `get_card` sees the questions too; the page shows the decisions alone.
- W14: Ask about it… raises on the child, prefilled with the decision's ID, so the department sees the objection on its own task.

**The owner's questions:**
- **Q-A.** Should the writer approve an open decision before it stands? Working answer: no. The decision is a queue item, confirmed as any; the writer sees it and may object with a question.
- **Q-B.** One mandate per split, or per subtask? Working answer: per subtask (D4).
- **Q-C.** Should a binding task refuse a recorded decision? Working answer: no (ADR-0012). It is recorded and shown as a departure.
