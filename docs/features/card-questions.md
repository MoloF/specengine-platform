---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, ui, plugin, scripts, fixtures]
ref: RM-0097, v5 step 5 (08 s2 Phase 7, "v5 scope"); the owner's v5 Q5 and Q11 of 2026-11-10; the analyst's requirement analysis of 2026-11-11; the orchestrator's working answers Q1-Q5, the owner's review open
adrs: [ADR-0077]
---

# card-questions: a task asks, and the answer is a recorded decision (RM-0097)

## Why

The owner's pipeline of 2026-11-10 (`08-roadmap.md` Phase 7, "v5 scope"): development sends a task back with a question; the writers' answer becomes a decision record, or a document that proves the work; a developer who decides alone says so; who decided what, and why, stays visible. His Q5: people raise questions and decisions from a task in the UI. His Q11: a decision record names no card, and the card links back to it.

Today:
- **Nothing ties an item to a card.** A question, discrepancy or decision names nodes; a card names nodes; a departing accept makes a discrepancy that does not name its card (`spec-trace.md` "Accepting").
- **A decision says what, never why.** `report_decision` takes `text`, `working_answer`, `price_of_other` (`proposal-decision.md` "Raise").
- **A return keeps one reason**, in the move's event; a question about it lives elsewhere, if anywhere.
- **The page raises nothing.** Only a hub's `/mcp` and a departing accept raise proposals through the daemon (`hub-mcp.md` "Trust", `board-ui.md` "Fence and trust").

This slice links the three deciding kinds to a card, gives a decision a `why`, serves a card's items and their events as one Activity, lets the page ask and record on a card and answer by a stage. ADR-0077 supplements ADR-0056, ADR-0057, ADR-0074 and ADR-0075; its canon is `docs/canon/card-questions.md` "The rule", and `architecture.md#ui` widens by the card's questions and decisions.

## Description and interactions

### Decisions

- **D1. The link.** A `question`, `discrepancy` or `decision` row may hold one `card_id`, set when it is raised and never changed by any later write (approve, reject, stage, reopen, a completion, an import). It names a card of the same queue, in any state (open, accepted, cancelled), in any repository of the slug: a card holds no path (`cards.md` "Store"). Missing: exit 1, nothing stored, no ID taken. No other kind holds one; a linked update (a `proposed_patch`) holds none.
- **D2. A decision's `why`**, optional: at most 2 048 bytes (`ANSWER_MAX`), not blank when given, stored as given. Only a `decision` holds it. A record shows it only through the new opt-in template slot `{{why}}` (Q1); a record names no card (Q11).
- **D3. Queue schema 11 and backup format 5.** `ALTER TABLE proposals ADD COLUMN card_id`, then `why`; `PROPOSAL_COLUMNS` 48; every due step in `migrate`'s one transaction, so a schema-9 or -10 queue (the live hub's is 10) reaches 11 at its first open by this build. Format 5 dumps 48 columns; format 4 still imports, the two `NULL`. No index ("Rules and edge cases", Scale).
- **D4. The card read serves `items`**, after `tasks`: every item naming the card, by ID number, 16 keys ("Data"), read now, no git. `--history` adds the items' last 64 `proposal.*` events to the card's last 64 `card.*` events, merged by `seq`. `list`, its rows and every write's document are unchanged: the board's read stays as it is.
- **D5. Raising with a card from the CLI and MCP.** `spec propose question … --card C`; `spec propose discrepancy|decision --input F --card C`, the flag outside the JSON as `--task` is; `why` inside `DecisionInput`. MCP `ask_question`, `report_discrepancy`, `report_decision` take `card_id`, `report_decision` `why`, on stdio and a hub's `/mcp`; descriptions trimmed to fit ("Data").
- **D6. The card door raises.** `POST /api/projects/:p/cards/:id/items` with a `question` or a `decision`, one library call `card_raise`, the eleventh of `CARD_CALLS`. It reads the card first (missing: 404), raises through the intake's rules with the card set, author `human` (on a hub with its `login`), and keeps the acting name as a trailing `as` on the item's `proposal.created`, never a column. No `proposed_patch`, `task_id` or author field: a person raises from the page (W4).
- **D7. On a hub it is the sixth plain write**: `hub.rs` `WRITES` gains `items`; `hub::acting` binds `as` to the login; `[team]` is not read for it. Any login asks (ADR-0060's plain class).
- **D8. Answering is staging** (Q5): no new write. `StageBody` takes `as` (Q2); a local daemon passes it to `stage_as`, a trailing `as` on `proposal.staged`, as a hub's stage already carries; absent, the event is byte for byte today's. A hub binds a sent `as` to the login. On a local daemon an answer ends at the stage and its record comes from a terminal (Q3, ADR-0035); on a hub, from a Confirm where `allowed` lists `confirm` (ADR-0061).
- **D9. The lifecycle.** A departing accept's discrepancy names the card accepted (`departure_intake`). Cancelling a card leaves its items on it; a correction card starts with none and reaches them through `corrects`.
- **D10. The link decides nothing** (ADR-0012, ADR-0077): no card write, `allowed`, rollup or gate reads an item; no stage, approve, reject or confirm reads the card. An answered question moves no card.
- **D11. Return can ask** (Q4). The Return dialog's optional part asks a question with the return: the question, a working answer and what another answer would cost, all required, as the intake's shape is. Two writes, the move first; a refused move asks nothing; a failed or unstored ask leaves the card returned and the dialog open on its fields.
- **D12. Activity** replaces the work item's History: one list in served `seq` order, card events as today, items' events by `labels.ts`' type map, and each item's block at its `proposal.created` (W7).
- **D13. Ask a question…, Record a decision…, Answer…, the Inbox link.** Two acts on a card with a target, open or closed (W10); Answer… on an open item, by the Questions screen's controls; the Inbox's proposal shows its task and a decision's why. The chrome says "Ask a question" and never "writers".
- **D14. The review document** gains `card_id` and `why`, after `interpretation` (`spec review`, `get_proposal`, `GET .../proposals/:id`); the inbox's entries do not change.
- **D15. Plugin 0.36.0**: three tools take new keys, `get_proposal` and `get_card` return new ones. `ask-owner` names `card_id` and `why`, its body still 4 096 bytes ("Data").
- **D16. Rollout.** Older binaries refuse a schema-11 queue (`SchemaTooNew`) and a format-5 dump; an older `spec approve` refuses a template using `{{why}}`. Every terminal, plugin and hub of a project upgrades from one commit; a hub dumps first ("Rules and edge cases", Rollout).

### The contract (Rust lane)

- **Core `intake.rs`.** `DecisionInput.why: Option<String>` (`#[serde(default)]`, after `price_of_other`, so serialization keeps the struct's order); `decision_problem` judges `optional("why", …, ANSWER_MAX)` right after `price_of_other`. Core `record.rs`: `Slot::Why`, after `DecidedBy`, `Slot::ALL` 16, free text (body only).
- **Store `queue.rs`.** `STEP_11` (two `ALTER TABLE … ADD COLUMN … TEXT`), `QUEUE_SCHEMA_VERSION` 11. `Intake.why: Option<String>`; `NewIntake.card_id: Option<String>` and `NewIntake.actor: Option<String>` (`Some` only from `card_raise`: a trailing `as` on the item's `proposal.created`). `IntakeShape::problem(kind, …)` refuses a `why` on a question or a discrepancy (`` a question holds no `why` ``). `create_intake_with_task`, inside its `Immediate` transaction, after the task binding and before the dedup: a `card_id` with no card row of the project → `QueueError::CardRefused {id, reason: "no card C-0099"}`, nothing written, no ID taken. `Proposal.card_id`, `Proposal.why`; the decode's corrupt cases ("Data").
- **Store `queue/state.rs`.** `PROPOSAL_COLUMNS` 48, `proposal_columns(11)` 48, `(8..=10)` 46; `card_columns(10..=11)` 25; `stored_rows` of a schema-10 DB gives the two `None`.
- **Store `queue/cards.rs`.** `card_items(id) -> CardItems {items, unreadable}` (`project` and `card_id` equal, by ID number, a row that does not read into `unreadable`); `item_history(ids, limit)`, `card_history`'s scan for `proposal.%` events whose payload `id` is one of `ids`, newest first, then reversed.
- **CLI `intake.rs`, `main.rs`.** `QuestionRequest.card`, `DiscrepancyRequest.card`, `DecisionRequest.card` (`Option<String>`, the written ID); `--card C` on the three subcommands. The written ID judged after the caps ("Data"); `[ids]` claiming `C` refused at rule 3 whenever `--card` is given (`card_prefix_clash`). `departure_intake` takes the card's ID.
- **CLI `card.rs`.** `card_show` serves `items` and the merged `history`; `CardRaiseBody` (`deny_unknown_fields`), `CardRaiseRequest`, `card_raise(&Env, &Globals, &CardRaiseRequest)`, the card read first, then the question's or decision's intake with `card_id` and `actor` set.
- **CLI `review.rs`, `state_file.rs`, `stage.rs`.** The review document's two keys and text lines; `STATE_FORMAT` 5; `StageBody.actor` (`#[serde(rename = "as")]`, optional, `card_person_problem`'s grammar), passed as `StageRequest.actor` by a local daemon.
- **MCP.** `QuestionArgs`, `DiscrepancyArgs`, `DecisionArgs` and their `Hub*` twins gain `card_id`; the decision pair `why`; `mirror.rs` `ReviewDocument.card_id`, `.why`, `CardDocument.items` (`CardItemShape`); the texts of "Data".
- **HTTP `app.rs`, `hub.rs`, `answer.rs`.** The route, its handler (no `card_` or `propose` prefix: `raise_on_card`), `WRITES` five, the answers of "Data"; the stage handler passes the body's `as`, a hub binding it with `acting`.
- **Plugin** 0.36.0 (`plugin.json`, `PINS`, `SCHEMA_PINS`, `scripts/deploy/.env.example`, `spec_ids_hub.rs`, `tag-check.sh`); `skills/ask-owner/SKILL.md` as "Data".

### The view (UI lane)

- **Types** (`src/api/provisional.ts` and the card and inbox types): `CardItem`, `Card.items`, `Proposal.card_id`, `Proposal.why`, `IntakeDocument`, `IntakeMatch`, `CardRaiseBody`, the stage body's `as`. `daemonKeys.test.ts` lists the new keys in `AHEAD_OF_FIXTURE` until the Rust lane's `fixtures/daemon-keys.json` lands; empty at ship.
- **Client** (`src/api/client.ts`, `http.ts`, `MockClient`): `raiseOnCard(project, id, body)`, its answer the intake document, sharing the card writes' mutation key (`useCardWriting`) and their re-reads; `stageDecision(project, id, stage, updatedAt, as)`.
- **Activity** (`CardPanel.tsx` main column; the item blocks and Answer… in a lazy `src/board/CardItems.tsx`): D12, "Data".
- **Return** (`CardDialogs.tsx` `ReturnDialog`): D11, its question part in first load.
- **Acts** (`CardActs.tsx`; the two dialogs in `CardItems.tsx`): D13.
- **Answer…** (`CardItems.tsx`): the Questions screen's controls (`round-ui.md` "Screen": by structure, never a kind string), staged through `stageDecision`; the local command and Copy as `decision-staging.md` "UI"; a hub's Confirm by `inbox/ConfirmDialog`, lazy, where `useAllowed` lists `confirm`; off (`ActOff`) where it lacks `stage` (`hub-team.md` "Screen").
- **Inbox** (`src/inbox/ProposalCard.tsx`): "Task C-0008", a link to `#/<p>/board/C-0008`, and a decision's Why.
- **Labels** (`src/board/labels.ts`): `ITEM_EVENT_WORDS`, the `proposal.*` type-to-word map beside `EVENT_WORDS`; nothing else in `src/board/` spells an event type.
- **Words** (`src/i18n/en.ts`, `ru.ts`): "Data", "Catalog words".
- **Mocks**: "Data", "Mocks and fixtures".

## Data

### Queue schema 11

```sql
ALTER TABLE proposals ADD COLUMN card_id TEXT;
ALTER TABLE proposals ADD COLUMN why TEXT;
```

`PROPOSAL_COLUMNS`: the 46 of today, then `card_id`, `why`. Both `NULL` on every stepped row, and on every row but those D1 and D2 name.

**Corrupt, named** (`get`, `list` fail naming row and column; `inbox` skips it with a note; `card_items` lists it as unreadable):

```text
proposal PR-0001: the stored `card_id` cannot be read: it is set on an update, which names no card
proposal PR-0001: the stored `card_id` cannot be read: `X-7` is no card ID
proposal PR-0001: the stored `why` cannot be read: it is set on a question, which holds none
```

A blank or over-long `why` from a hand edit reads as stored.

### Raising with a card (CLI)

```text
spec propose question ID… --text T --working-answer W --price-of-other P [--severity S]
  [--distinct-from X]… [--card C] [A] [--task T]
spec propose discrepancy --input F|- [--card C] [A] [--task T]
spec propose decision --input F|- [--card C] [A] [--task T]
```

`--card`'s help: `The card (C-0001) this is raised for: one of this queue's, in any state.` In the intake's rules (`agent-intake.md` "Rules"), in order:
- **Rule 2**, after `distinct_from` and before the author: the written ID through `parse_card_id`. A look-alike exits 2 with the card commands' line (`card_look_alike_message`), prefixed `card_id: `. Not an ID in any script: exit 1, `` card_id: no card `C-07`: a card ID is `C-` and 4 or more digits, as `spec card list` lists it ``.
- **Rule 3**, with the config: `[ids]` claiming `C` exits 2 with `card_prefix_clash`'s line, whenever `--card` is given; without it the raise is as today.
- **Step 7**, in the store's transaction: no such card, exit 1, the intake document with `id` `null`, `created` false, and the last note `card_id: no card C-0099`. No event, no ID.

### `why` and `DecisionInput`

```json
{"node_ids": ["RULE-STAM-REGEN"], "text": "Does regeneration wait for rest?",
 "working_answer": "Yes: it starts 1.5 s after the last stamina use.",
 "price_of_other": "Without the wait R-28 is rebalanced.",
 "why": "Two playtests read instant regeneration as a bug.",
 "severity": null, "proposed_patch": null, "distinct_from": null}
```

`why: blank: give a value`, `why: 2049 bytes; at most 2048`, after `price_of_other`'s refusals. `why` in a discrepancy's `--input` is an unknown field (exit 2), as is `card_id` in any `--input`: the card is the flag.

### The card door: `POST .../cards/:id/items`

`CardRaiseBody`, every key below, `deny_unknown_fields`:

```json
{"kind": "question", "node_ids": ["SCN-STORM"], "text": "Which bell rings at dusk?",
 "working_answer": "The harbour bell, as in chapter one.", "price_of_other": "Two scenes rewritten.",
 "why": null, "severity": null, "distinct_from": null, "as": "dev"}
```

- `kind` is `question` or `decision`; anything else is a 400, `` `kind`: a card raises a `question` or a `decision` ``. A `why` with `question` is a 400, `` `why` goes with a `decision` ``.
- `node_ids`, `text`, `working_answer`, `price_of_other` are required, judged by the intake's rules; `severity`, `distinct_from` and `why` are optional.
- `as` is the acting name, judged by `persons_named`; `null` reads `local`.

The door is `board-ui.md` "The door"'s: slug, same-origin, the path's ID, JSON, `CARD_BODY_MAX`, decode. Then one `card_raise` call in the project's turn, at one clock reading, its place the served root's (`propose update` step 5):

| Answer | When |
|---|---|
| 200 | the intake document `{id, created, hits, related, linked, diagnostics, notes}`, stored or not (`created`) |
| 400 | a body or a person refused, the CLI's line |
| 404 | `{id, reason}`, `no card C-0099`: the card read first, before any intake rule |
| 409 | refused by an intake rule (exit 1): the intake document, its reason the last of `notes` |
| 503 | cannot run: a look-alike in `node_ids`, `[ids]` claiming `C` or `PR`, a place outside a worktree, a corrupt row, a newer schema |

Any other method: 405, `Allow: POST`. The stored row: author `{"type":"human","role":null,"model":null,"run":null}`, on a hub with `"login":"<login>"`. The event is `proposal.created {"id":"PR-0012","as":"dev"}`; a hub's `as` is its login. Every other raise's `proposal.created` stays `{"id":…}`.

### The stage's `as`

`POST .../proposals/:id/decision` takes `as` beside the stage's keys (`decision-staging.md` "Daemon"); so does each stage in `POST .../round/answers`, since `RoundAnswersBody` wraps `StageBody`:

```json
{"decision": "approve", "option": null, "answer": null, "canon": null, "editorial": null, "note": null, "updated_at": "2026-11-11T09:30:00Z", "as": "ana"}
```

Absent or `null`: no `as` in the event, byte for byte today's. Given locally: `proposal.staged {"id","staged","staged_at","as":"ana"}`. On a hub the login stands; a sent `as` naming another is a 403, the plain class's line. `unstage` takes no body and no `as`.

### The card read: `items` and `history`

`spec card show C [--history] --json`, `get_card`, `GET .../cards/:id`; `items` after `tasks`:

```json
"items": [{"id": "PR-0012", "kind": "question", "status": "applied",
  "summary": "Which bell rings at dusk?", "working_answer": "The harbour bell, as in chapter one.",
  "why": null, "author": {"type": "human", "role": null, "model": null, "run": null},
  "created_at": "2026-11-11T09:00:01Z", "updated_at": "2026-11-11T10:02:00Z", "staged_at": null,
  "decided_by": "Owner <owner@example.invalid>", "decided_at": "2026-11-11T10:02:00Z",
  "decision_note": null, "record_id": "DEC-0031", "record_path": "docs/records/DEC/DEC-0031.md",
  "record_title": "The harbour bell"}]
```

Each value is the row's, `author` its object (`login` when it has one). Text: one line per item, after `tasks:`:

```text
items: PR-0012 | question | applied | Which bell rings at dusk? | record DEC-0031
items: PR-0013 | decision | open | Does the lamp burn oil?
items: -
```

The summary goes through `one_line`; ` | record <id>` appears only with a `record_id`. A row that does not read is left out, with a note after the card's own notes: `PR-0014: its row does not read (<column>: <reason>); not listed`. `show` exits 0 however its items read.

`history` (`--history`, else `null`) takes the card's last 64 `card.*` events (`HISTORY_MAX`) and its items' last 64 `proposal.*` events (a payload whose `id` is an item's), merged by `seq`, oldest first, in the same shape `[{seq, type, at, payload}]`. A linked update's events are not an item's. `get_card` asks `--history` only when its `history` argument does, as today.

### The review document

`…, interpretation, card_id, why, notes` (`proposal-review.md` "Review document"). `card_id` and `why` are `null` where absent. Text lines: `card_id: C-0007` and `why: <text>`, the latter printed as `working_answer` is. The inbox's entries and text are unchanged.

### The record's `{{why}}` slot

| Slot | Value |
|---|---|
| `why` * | a `decision`'s `why` when the record takes its working answer (no flag); else empty: `--answer T` (the choice is the owner's then, W2), a question, a discrepancy, a decision without one |

\* Free text: body only (`decision-record-template.md` "Template"). Example template lines, after `{{cost}}`'s block: `Why:`, a blank line, `{{why}}`. An older build reading such a template exits 2 at approve step 3, `` <template>:<line>: `{{why}}` is no slot: … ``, so the slot goes into a template only after every terminal and hub approving records runs this build.

### Backup format 5

```text
{"format":5,"queue_schema":11,"project":"lantern-keep","proposals":2,"tasks":0,"runs":0,"cards":1,"events":4}
```

Proposals rows hold 48 columns, `card_id` and `why` last. Format 4 (schema 10, 46 columns) imports with both `NULL` and re-exports as format 5 with `"card_id":null,"why":null`; formats 1-3 as today, their later columns `NULL`. A schema 1-10 DB exports as format 5, schema 11, unmigrated. Refused, exit 2, nothing changed: a format-5 proposal row without `card_id` or `why`; a format-4 row with either; format 5 with `queue_schema` not 11 (``... (it holds queue schema 11)``); `"format":6` (``… upgrade SpecEngine``).

### MCP

Input schemas (`additionalProperties: false` kept): `ask_question`, `report_discrepancy`, `report_decision` gain `card_id` (`["string", "null"]`) after `distinct_from`; `report_decision` gains `why` (`["string", "null"]`) after `price_of_other`; a hub's three alike, still with no `task_id`. Outputs: `get_proposal` the review document's two keys; `get_card` `items` (`CardItemShape`, the 16 keys, `author` the `ProposalAuthor` shapes).

Description edits, stdio and hub alike; bytes as `concat!` yields them, each at most 2 048 and ASCII:

| Tool | Edits | Bytes |
|---|---|---|
| `ask_question` | the twin gains `[--card C] ` before `--author-role`; drop ` (stored by its id, else its path),` for `,`; `severity: high, normal (absent) or low; it only orders the queue.` becomes `severity: high, normal (absent) or low.`; `queued questions or decisions with other text.` becomes `queued items with other text.`; after the `distinct_from:` line, `card_id: the card (C-0001) it is asked for, in any state.` | stdio 2 041 -> 2 033; hub 2 038 -> 2 030 |
| `report_decision` | `distinct_from, author_role (required), …` becomes `distinct_from, card_id, author_role (required), …`; then the line `why: why you decided so, at most 2048 bytes; null: none.` | 1 744 -> 1 810; hub 1 692 -> 1 758 |
| `report_discrepancy` | the same `card_id` insertion | 1 727 -> 1 736; hub 1 705 -> 1 714 |
| `get_card` | after the `tasks:` sentence: `items: the questions, discrepancies and decisions that name it {id, kind, status, summary, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title}, by ID, read now; with history, their last 64 proposal.* events join it by seq.` | 1 099 -> 1 411 |
| `get_proposal` | `a question's, discrepancy's or decision's fields,` becomes `… fields (its card_id; a decision's why),` | 1 118 -> 1 150 |

`INSTRUCTIONS` are unchanged.

### Key sets and the plugin

`fixtures/daemon-keys.json`, 115 to 118 sets:
- 3 `Proposal`, 48 to 50: `card_id`, `why` after `interpretation`;
- 92 `Card`, 31 to 32: `items` after `tasks`;
- 116 `CardItem` `[id, kind, status, summary, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title]`;
- 117 `IntakeDocument` `[id, created, hits, related, linked, diagnostics, notes]`;
- 118 `IntakeMatch` `[id, source, status, path, answer, record]`.

The stage body is a request, not a key set.

Plugin 0.36.0: `plugin.json`, the last `PINS` and `SCHEMA_PINS` entries, `scripts/deploy/.env.example`, `spec_ids_hub.rs`, `tag-check.sh`. `ask-owner`, two edits, its body staying 4 096 bytes (the file 4 374):

| Was | Becomes |
|---|---|
| `` your decision as the `working_answer`, and `price_of_other`; `` | `` your decision as the `working_answer`, `price_of_other`, and why in `why`; `` |
| ``On a task, pass its `task_id` too; `get_proposal`'s `task_id` names it.`` | ``On a task, pass its `task_id`; for a card, its `card_id`.`` |

### Catalog words

New keys in both catalogs; English values:
- **`board.panel.activity`** "Activity", replacing `board.panel.history`.
- **`board.activity.*`**: one word per `proposal.*` type: Raised, Answer staged, Staged answer taken back, Approved, Recorded, Rejected, Approve refused; plus "Earlier items".
- **`board.items.*`**: Working answer, Why, Answer (a rejected item's `decision_note`), Record, Staged {{at}}, Decided by.
- **`board.ask.*`**: Ask a question…, the dialog's title "Ask a question on {{id}}", Question, Working answer, What another answer would cost, About (the targets), Ask, Asking…, "Asked {{id}}", "Already answered or asked:", Ask anyway, Ask again, and the too-many line "More earlier answers match than this dialog can name: ask on a terminal, naming each in --distinct-from."
- **`board.return.ask`** "Ask a question with it"; the line "{{id}} sent back; the question was not asked".
- **`board.decide.*`**: Record a decision…, What had to be decided, The decision, What another answer would cost, Why (optional), Record, and "Raised {{id}} for the owner to confirm".
- **`board.answer.*`**: Answer…, "Answer {{id}}". The controls reuse `round.*`; the staged line and Copy reuse the inbox's.
- **`inbox.proposal.task`** "Task {{id}}"; **`inbox.proposal.why`** "Why".

No value names a trade: "writer" appears in none ("Acceptance criteria" U5). Russian is the ui-developer's, in `ru.ts` alone, the owner's review open (`ui-languages.md` "Known limits").

### Activity, in order

1. **Earlier items.** The items whose `proposal.created` is not in `history` come first, by ID.
2. **The thread.** Then every `history` entry in served order:
   - a `card.*` entry as History draws it today;
   - a `proposal.*` entry: its word, the item's ID, and "by <who>". Who is the payload's `as`, else the item's `author.login`, else its `author.role`, else nothing;
   - a `proposal.created` of an item also opens the item's block.
3. **The block.** The `kind` sits raw in a neutral badge under `lang="en"`, then the status lozenge and the summary. Below them, the working answer and the why (agent text: `shownText`, wrapped, `<bdi>`).
4. **Its state.** Staged {{at}}; the record as a `DocLink` to `record_id`, labelled by `record_title`, with `decided_by`; or a rejected item's `decision_note` as its answer. An open item gets Answer….

Nothing compares `kind`, `status` or a type outside `labels.ts`.

### Mocks and fixtures

- **harbor-sim** (`src/mocks/cards.ts`, `src/mocks/inbox.ts`):
  - `C-0008`, in `plates`' return column `blocked`, gains question `PR-0055` (open, asked by "Noor Ainsley" through the page);
  - and decision `PR-0056` (applied, `why` set, its record `DEC-0013`), with their events in its history;
  - `POST .../items` stores a question or a decision into the mock's inbox;
  - `?scenario=conflict`: a page's first raise answers one corpus hit, not stored, and Ask anyway stores it.
- **Rust tests** run on scratch copies of `fixtures/studio-lamp` (return columns, hand-offs) and `fixtures/spec-a` (`[decision_records]`; its template gains the `{{why}}` lines in the copy only). `fixtures/daemon-keys.json` takes the sets above; no fixture's `expected.json` changes.

## Rules and edge cases

- WHEN `card_id` names no card of the queue, the raise SHALL exit 1 (route: 404), store nothing, take no ID and log no event; a card in any state SHALL be named.
- WHEN a raise naming a card meets a hit, it SHALL store nothing, as any intake; `distinct_from` naming each hit stores it. A hit is no refusal: exit 0, 200.
- WHEN the Return dialog's question is on, the move SHALL be sent first and the raise only after a 200. A refused move SHALL send no raise. A raise that fails or stores nothing SHALL leave the card returned and the dialog open on its fields, with Ask again or Ask anyway resending only the raise.
- WHEN a raise's answer is lost (status 0) and is sent again, the first, if stored, SHALL be the second's queue hit (same text, shared target): no item is stored twice.
- WHEN Ask anyway's answer stores nothing and every listed hit is already named, the dialog SHALL show the too-many line and offer no Ask anyway. Over 64 hits (`agent-intake.md` "Dedup") nothing is ever stored.
- WHEN a card is cancelled, accepted, moved or corrected, its items SHALL stay as they are; a correction card's `items` SHALL be `[]`.
- WHEN an item is approved, rejected, staged, reopened or completed, its `card_id` SHALL stay; when its card is cancelled meanwhile, each SHALL run as before.
- WHEN a template uses `{{why}}`, a question, a discrepancy, an `--answer` approve and a decision without one SHALL fill it empty; the slot outside the body SHALL be a template error.
- WHEN a stage carries no `as`, its event SHALL be byte-equal to today's; the round route's answers SHALL take `as` per stage, and this slice's page sends it from the card's Answer… and the inbox's decision dialog only (W5).
- WHEN a hub login outside `[team]` opens a card, Ask a question… and Record a decision… SHALL be on and Answer… off, with its line; the route SHALL bind `as` to the login.
- **Two stores.** A card ID names a card of one queue: a terminal's `C-0007` and a hub's are different cards, and `--card` on a terminal never reaches a hub's (that card is named through the hub, its page or `/mcp`). A hub filled from a terminal's dump holds that terminal's cards and their items as stored.
- **No core logic in the UI**: it judges no caps, hits or card states; the daemon answers, the page shows it. Kinds, statuses and event types are looked up, never compared, outside `labels.ts` (`policy.test.ts`).
- Nothing branches on a card's flow, column, discipline or a department (ADR-0008); no word of a trade enters the chrome.
- **Scale.** No index: `card_items` reads the slug's `proposals` rows with the card's ID, one scan per card read, as `missing_task` does; `item_history` scans `proposal.%` events newest first, as `card_history` scans `card.%`. Both run only on a card's read, never on the Board's `list`. Unmeasured (`scale-eval.md` has no card rows).

### Rollout

**Older binaries.** The first open by a new `spec`, `specengine-mcp` or `specengine-http` steps a queue to 11, which every older binary refuses (`SchemaTooNew`):
- an older `spec` exits 2, an older daemon answers 503, an older MCP server fails its tools;
- the hook's older `spec gate` gives "no verdict (… newer schema)", so `selective` refuses every controlled write until `spec` and the plugin reinstall from one commit (`gate.md` "Verdict");
- an older `spec import-state` refuses format 5;
- an older `spec approve` refuses a `{{why}}` template.

**The owner's upgrade**, card-tasks' six steps with the format-4 dump as the way back (`card-tasks.md` "Known limits"). The way back: move `<slug>.db` aside and import the step-1 dump with the old build; what changed since is lost.

**The hub** runs v0.32.0 at schema 10. Its upgrade is `hub-ops.md` "Upgrade":
1. A dump on the old version first: `dump-queue`, `dump-door`, `backup upload`, both snapshot IDs noted.
2. The new tag; the queue steps 10 -> 11 at the first open.
3. The way back is the old tag and `.env`, then the noted queue snapshot by "Restore" step 6.

A format-5 dump goes only to a hub on this build or later.

### Test churn

Only these change; every other test passes unchanged. The test-engineers add what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| every test pinning queue schema 10, `PROPOSAL_COLUMNS` 46 or backup format 4 (store `queue_state.rs`, `queue_intake.rs`, `queue_kinds.rs`, `queue_stage.rs`, `queue_drift.rs`, `queue_amendment.rs`, `queue_interpretation.rs`, `queue_tasks.rs`; CLI `queue_state.rs`, `intake_state.rs`, `card_tasks.rs`, `cards.rs`, `decision_apply.rs`, `drift_raise.rs`, `kind_amendment.rs`, `kind_interpretation.rs`, `proposal_kinds.rs`, `proposal_path.rs`, `tasks.rs`; MCP `mcp_code.rs`) | 11, 48, format 5 | D3 |
| the newer-schema probes, after M1's run (CLI `stage_confirm.rs` `ac02`, `gate.rs` `ac06`, `queue_state.rs` `ac06`, `trace_views.rs` `ac03`; store `queue_state.rs`, `queue_gate.rs`; http `stage.rs` `ac02`, `edges.rs`, `tail.rs`, `flow.rs`) | user_version 12, "this build knows 11", `format-6`, `schema-12` | D3, D16 |
| core `record.rs` slot tests (`Slot::ALL`, the known-slots list in "is no slot") | 16 slots, `why` last | D2 |
| CLI `cards.rs`, `card_handoff.rs`, `card_tasks.rs`, `card_trace.rs` goldens of `show --json` | `items` after `tasks` | D4 |
| http `cards.rs`, `daemon_keys.rs`, `door.rs` (`CARD_CALLS` 11, the second exception), `methods.rs`, `hub.rs` unit tests of `class`; `fixtures/daemon-keys.json`; UI `api/daemonKeys.test.ts` | 118 sets; the route | D4, D6, D7 |
| MCP `mcp_intake.rs`, `mcp_decision.rs`, `mcp_report_decision.rs`, `mcp_cards.rs`, `mcp_hub.rs` (schemas, texts), `plugin_files.rs`, `plugin_skills.rs` (ask-owner 4 374 / 4 096), `plugin_roles.rs`; http `spec_ids_hub.rs` | 0.36.0 | D5, D15 |
| UI `board/workItem.test.tsx`, `CardModal.test.tsx`, `CardPage.test.tsx` (History becomes Activity), `ReturnDialog.test.tsx`, `inbox` stage-body tests, `mocks/MockClient.test.ts`, `policy.test.ts` (the event regex) | the new shapes and words | D8, D11-D13 |

## Acceptance criteria

A browser look is marked **[look]**; a timed or counted run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red. New Rust tests live in `card_questions.rs` of the crate named (store, CLI, http), unless another file is named.

- [ ] S1 — **Schema step 11.** Store:
  - a new queue stands at `user_version` 11; `PRAGMA table_info(proposals)` equals `PROPOSAL_COLUMNS`, 48, `card_id` and `why` last; `proposal_columns` gives 48 for 11, 46 for 8 to 10, `None` for 12;
  - a schema-10 DB built by steps 1-10, one row of each kind and a card, steps to 11 at its first `SqliteQueue::open`, every row byte-equal but the two new `NULL` columns; a schema-9 DB reaches 11 in one `migrate`;
  - a schema-9 DB whose `proposals` already has a hand-made `card_id` column fails at step 11 and stays at 9 with no `cards.tasks`;
  - `open_existing` on a schema-10 DB reads its proposals, both `None`, and steps nothing.

  Mutations, each red: each step committed alone (the last case reads 10); `why` added before `card_id`.
- [ ] S2 — **Raise with a card.** CLI, on a scratch `studio-lamp`:
  - a card `C-0001` with a target; `spec propose question <its target> … --card C-0001 --json` stores `PR-0001`, whose `spec review --json` has `card_id` `C-0001`;
  - a decision and a discrepancy by `--input` with `--card` alike; an accepted card and a cancelled one each take an item;
  - `--card C-0099`: exit 1, `id` null, last note `card_id: no card C-0099`, no event, and the next stored raise takes the next free ID;
  - `--card C-07`: exit 1 with "Data"'s line; a look-alike (Cyrillic first letter): exit 2, naming `C-0001`;
  - `[ids]` claiming `C`: exit 2, the clash line, with `--card` only;
  - store: `create_intake` naming a missing card is `CardRefused`, nothing written.

  Mutations, each red: the existence check after the insert (an ID taken); a closed card refused.
- [ ] S3 — **A decision's `why` and the slot.**
  - **Core.** `decision_problem`: a blank `why`, then 2 049 bytes, refused with "Data"'s lines; 1 024 `é` (2 048 bytes) read. A blank `why` beside a bad patch is `why`'s refusal.
  - **CLI.** A decision's `--input` with `why` stores it (`spec review --json`, text `why: …`); a question's `why` is `null`; a discrepancy's `--input` holding `why` exits 2.
  - **The record**, on a scratch `spec-a` whose template gains the `{{why}}` lines: approving the decision with no flag writes its why; `--answer T` writes the slot empty, as does approving a question; `{{why}}` in the front-matter is a template error, exit 2.
  - **Store.** `Intake.why` on a question is `Invalid`; a question row with `why` set by SQL is corrupt, named with "Data"'s line.

  Mutations, each red: `why` filled under `--answer`; `why` accepted on a question.
- [ ] S4 — **Lifecycle and corrupt rows.**
  - **Corrupt.** By SQL: `card_id` on an `update` row, `X-7` on a question: `spec review` exits 2 naming `card_id`, `spec inbox` skips the row with a note.
  - **The link's life.** A departing accept (`spec card accept C --as N --departs REF …`) stores its discrepancy with `card_id` the card. Approve with a record, reject, stage, unstage and a reopen each leave `card_id` as raised (the row read after each).
  - **Cards.** `spec card new --corrects C` gives `items: []`; cancelling a card keeps its items.

  Mutations, each red: `departure_intake` without the card; a correction listing its original's items.
- [ ] S5 — **Backup format 5.** CLI `queue_state.rs`:
  - **Export.** The header is "Data"'s, eight keys; proposals rows hold 48 columns.
  - **Round trip.** A queue with a carded question and a decision with `why` round-trips byte-identical.
  - **Format 4.** A format-4 dump (schema 10, 46 columns) imports both `NULL` and re-exports as format 5.
  - **Refused**, each exit 2 with nothing changed: a format-5 row without `card_id`; a format-4 row with it; format 5 at `queue_schema` 10; `"format":6`.

  Mutations, each red: `STATE_FORMAT` left 4; a format-4 row accepted with the new columns.
- [ ] K1 — **The card read, deciding nothing.** CLI and http `cards.rs`, on a scratch `studio-lamp`:
  - `spec card show C-0001 --json`: `items` after `tasks`, set 116's keys in order, by ID number; an applied decision with `record_*`; a rejected question with `decision_note`; `items: []` with none; from a root in no git worktree it reads the same;
  - `--history`: 70 moves and 70 item events give 128 entries, each list's newest 64, merged by `seq`; a linked update's events absent;
  - text: "Data"'s `items:` lines;
  - a corrupt item row: left out, its note, exit 0;
  - `spec card list --json` and `GET .../cards` byte-equal before and after the card's items are raised;
  - **ADR-0012**: beside an open question and an open decision, a card moves into its return column and out, is assigned, accepted and cancelled with documents and events equal (but `items`) to a twin's without. `spec gate --path` gives the same verdict. A question whose card was cancelled is approved and rejected as one without.

  `GET .../cards/C-0001` byte-equal to `show --history --json`. Mutations, each red: `card_move` refusing while an item is open; the two event lists capped together at 64.
- [ ] L1 — **CLI surface.**
  - `spec propose question --help`, `discrepancy --help` and `decision --help` each list `--card C` with "Data"'s help;
  - `spec review PR --json`'s keys end `interpretation, card_id, why, notes`;
  - `spec inbox`: an item raised with `--card` lists exactly as its twin raised without (set 2 unchanged).

  Mutation, red: the review document without `card_id`.
- [ ] L2 — **MCP, stdio.** `mcp_intake.rs`, `mcp_report_decision.rs`, `mcp_cards.rs`:
  - **Schemas.** The three tools' input schemas hold `card_id` `["string","null"]`, `report_decision` `why`, root `additionalProperties: false`.
  - **Twins.** A call with both stores them; `structuredContent` equals the CLI's `--json`; `get_proposal` serves both; `get_card` serves `items`, valid against `outputSchema`.
  - **Texts.** The descriptions equal "Data"'s edits and byte counts; each names `card_id`.

  Mutations, each red: `CardItemShape` without `decision_note`; the question text left untrimmed (over 2 048).
- [ ] L3 — **The card door.** http `card_questions.rs`, a scratch `studio-lamp` on a local daemon:
  - **Raises.** A question body with `as: "dev"` gives 200, `created` true; its review has `card_id` `C-0001` and the human author; the event is `{"id":"PR-0001","as":"dev"}`. `as: null` gives `"as":"local"`. A decision with `why` stores it.
  - **Refusals, nothing stored.** `C-0099` is a 404 `{id, reason}` with no event. The same question again is a 200, `created` false, its hit `PR-0001`, then stored with `distinct_from`. A cap is a 409, its reason the last note. An unknown key, `kind: "update"` and `why` on a question are 400s. A look-alike node is a 503, non-JSON a 415, no same-origin a 403, GET a 405 `Allow: POST`.
  - **The door.** `door.rs`: `CARD_CALLS` 11 with `card_raise`, `card_ask` caught, `propose*` still forbidden. `card_writes_touch_only_the_queue` admits a raise's one `proposal.created`, git, files and other proposals unchanged.

  Mutations, each red: the handler calling `propose_question`; the event without `as`; the card not read first (`C-0099` stored or 409).
- [ ] L4 — **The stage's `as`.** http `stage.rs`, CLI `stage.rs` tests:
  - a local POST of a stage with `as: "ana"` logs `proposal.staged` ending `"as":"ana"`;
  - without `as` or with `null`, the payload is exactly `{"id","staged","staged_at"}`, as today;
  - a round answer's stage with `as` stages under it;
  - `as` off the person grammar is a 400; `spec approve` confirms the stage as before.

  Mutations, each red: `"as":null` written; the `as` stored in a column (`staged` changes).
- [ ] M1 — **Rollout, proven on today's code**; no older build is run.
  - **First, at a62c81c**, before the Rust lane lands, the test-engineer runs and records with the commit: CLI `stage_confirm.rs` `ac02_a_queue_of_a_newer_schema_exits_2_naming_both`, `gate.rs` `ac06_each_no_verdict_is_exit_2_naming_its_cause`, `queue_state.rs` `ac06_a_newer_or_another_projects_dump_is_refused` (its `format-5`, `schema-11`); store `queue_state.rs` `open_existing_refuses_a_newer_schema`; http `stage.rs` `ac02_a_queue_of_a_newer_schema_answers_503_to_post_and_delete`. Each is what a 0.35 binary does with this slice's queue and dump; they then move to 12 and format 6 ("Test churn").
  - **New**, green before and after the slice, each with `colour`, unknown both times:
    - CLI `m1_an_unknown_template_slot_exits_2_at_its_line` (`spec-a`, `{{colour}}` in the body): exit 2, stderr starting `` templates/decision.md:<that line>: `{{colour}}` is no slot: ``;
    - CLI `m1_an_unknown_decision_input_key_exits_2` (`"colour": 1` in `--input`);
    - MCP `m1_an_unknown_argument_fails_report_decision`, an error result `failed to deserialize parameters: unknown field `colour``.

  Mutation, red: the slot's line taken from the front-matter.
- [ ] H1 — **A hub's sixth plain write.** http `hub_team.rs` and `hub.rs`:
  - `Remote-User: mira`, outside `[team]`: the route gives 200; the author holds `"login":"mira"`, the event `"as":"mira"`;
  - `as: "ben"`: 403 ``refused: `as` names ben; this request's login is mira``;
  - `class(POST, ".../cards/C-0001/items")` is Plain; `.../items/x` and an encoded `items` are `None`;
  - `the_step_up_regex_is_the_confirm_class` still matches the four alone.

  Mutations, each red: `items` classed Team; `acting` skipped.
- [ ] H2 — **A hub's stage under the login.** `hub_team.rs`:
  - a `member` stages on a carded question: `proposal.staged` ends `"as":"mira"`;
  - a body `as` naming another is a 403;
  - a login outside `[team]` is a 403.

  Mutation, red: the body's `as` taking precedence over the login.
- [ ] H3 — **A hub's `/mcp`.** `hub_mcp.rs`, `mcp_hub.rs`:
  - `ask_question` with `card_id` stores it with `author.login`;
  - `report_decision` takes `why`; `task_id` is still absent;
  - the hub texts are "Data"'s bytes, ASCII.

  Mutation, red: `HubQuestionArgs` without `card_id`.
- [ ] H4 — **Answered on a hub, read back.** `hub_confirm.rs`'s hub with `[decision_records]` and a `{{why}}` template:
  - a carded decision with `why`, staged by a `member`, confirmed by a covering `lead`, commits its record holding the why;
  - `GET .../cards/C-0001` then lists the item `applied` with `record_id`;
  - its `history` ends with `proposal.approved` and `proposal.applied`.

  Mutation, red: the confirm's record rendered without the slot.
- [ ] U1 — **Activity.** `src/board/CardActivity.test.tsx` (new) and `workItem.test.tsx`, on harbor-sim's `C-0008`:
  - "Activity" stands where History stood; its entries keep served order; the return move shows with its note;
  - `PR-0055`'s block: `question` raw under `lang="en"`, the open lozenge, the summary, the working answer, "by Noor Ainsley" from the event's `as`;
  - `PR-0056`'s block: its why, a `DocLink` to `DEC-0013` by its title, and `decided_by`;
  - an item whose creation is not in `history` stands under "Earlier items"; an unknown type stays raw.

  Mutations, each red: entries sorted by `at` (the mock holds two entries whose `at` order reverses their `seq` order); the block chosen by `kind`.
- [ ] U2 — **Return with a question.** `ReturnDialog.test.tsx`:
  - **The part.** A card with targets has a closed "Ask a question with it"; opened, it shows the three required fields and the targets, each checked (1 to 16 required). A card with no target has no such part.
  - **Order.** Submit sends `POST .../move`, then `POST .../items` with `{kind: "question", node_ids, text, working_answer, price_of_other, as}`.
  - **A refused move** sends no raise and shows the refusal.
  - **Not stored.** A 200 with `created` false keeps the dialog, says the card is sent back and lists the hit. Ask anyway posts the raise alone, `distinct_from` the hit's ID.
  - **No answer.** A status 0 offers Ask again, resending the raise alone. With every listed hit named and nothing stored: the too-many line, no Ask anyway.
  - **Unchanged.** With the part closed, the one move is byte-equal to today's.

  Mutations, each red: the raise sent first; Ask anyway resending the move.
- [ ] U3 — **Ask a question…, Record a decision….** `src/board/CardAsk.test.tsx` (new):
  - both acts follow Return… on a card with a target, open or closed; none without one;
  - Ask posts `kind: "question"`; Record posts `kind: "decision"`, `why` when given, else `null`;
  - hits behave as in U2; a 400 or 409 shows in the dialog's alert, which keeps its fields.

  Mutation, red: Record sending `why: ""`.
- [ ] U4 — **Answer….** `src/board/CardAnswer.test.tsx` (new):
  - **Local.** An open item's Answer… reads `getProposal` once, offering Take the working answer, Your answer and Reject with reason, by structure. Stage posts `.../proposals/PR-0055/decision` with the stage, `updated_at` and `as`; then Staged <at> and `spec approve PR-0055` with Copy (`spec reject …` after a reject), with no Confirm.
  - **A hub.** `allowed` with `confirm` adds Confirm (the lazy `ConfirmDialog`); `stage` alone has none; outside `[team]` Answer… is off (`aria-disabled`, its line).
  - **Closed or refused.** An applied item has no Answer…; a 409 keeps the dialog.

  Mutations, each red: Copy for an ID off `^PR-[0-9]{4,}$`; Answer… on outside `[team]`.
- [ ] U5 — **The Inbox link, words, policy.**
  - `ProposalCard.test.tsx`: `card_id` shows "Task C-0008" linking `#/harbor-sim/board/C-0008`; a decision's `why` under its working answer; neither when `null`.
  - `policy.test.ts`: the event-literal scan becomes `/["'\`](card|proposal)\.[a-z_]+["'\`]/`, `labels.ts` alone exempt; its control `'const created = "proposal.created";'`, which today's scan passes, is red.
  - `words.test.ts`: no `en` leaf holds `/\bwriters?\b/i`, no `ru` leaf the escaped stem of the Russian word for a writer; `catalog.test.ts` passes.

  Mutation, red: a `"proposal.staged"` literal in `CardPanel.tsx`.
- [ ] U6 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900 and 320 wide, on the mock and on a scratch copy of a real game corpus with a return column and `[decision_records]` configured:
  - the work item's Activity: a return, an open question, a staged one, an answered decision with its record and why, a departing accept's discrepancy;
  - the Return dialog with its question part, then not stored with a hit and Ask anyway;
  - Ask a question… and Record a decision…;
  - Answer…: the local command; a hub's Confirm (the mock with a hub login);
  - the Inbox's proposal with its task and why;
  - long Russian text wrapping; at 320 each dialog filling the screen; focus rings visible, nothing cut unseen.
- [ ] U7 — **[measure] Size.** After `pnpm build`, first-load JS (`ui-languages.md` "Size") is at most 845 kB; expected +3 to +8 kB on 827.62, Activity's blocks and the three dialogs lazy in `CardItems`, the Return part in first load. Should it pass 845, `RoadmapView` leaves first load first (lazy at its route, as `RoundScreen`), then `GraphView`. Record the figure, gzip and `ru-*.js`.
- [ ] D1 — **[measure] Dedup on a real corpus.** A scratch copy of a real game corpus on a local daemon. For the five card targets with the most accepted decisions among their `spec show --links`, raise a question from a card through the route:
  - record each one's hits, related and outcome;
  - Ask anyway stores each;
  - any target over 64 hits is recorded with its count, and the too-many line is seen.
- [ ] P1 — **Plugin 0.36.0.**
  - `plugin_files.rs`: `PINS` and `SCHEMA_PINS` end at 0.36.0, the schemas' hash moved; `spec_ids_hub.rs` and `tag-check.sh` read 0.36.0.
  - `plugin_skills.rs`: ask-owner's body is 4 096 bytes (file 4 374), holding "Data"'s two sentences, every content rule passing.

  Mutations, each red: `plugin.json` left at 0.35.0; the `card_id` sentence dropped.
- [ ] G1 — **Gates.**
  - **UI.** Lint 0, `tsc` clean, the full test run green twice.
  - **Rust.** The workspace run green, then `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy --test workspace_names`.
  - **Docs.** The check is clean.

## Out of scope

- `card_id` on an update, a create, an interpretation, an amendment, a drift item or a linked update; changing or clearing an item's card; an item naming two cards.
- A question's addressee (a department or a person): whoever reads it answers.
- Raising a discrepancy from the page (a departing accept still makes one); a `proposed_patch` from the page.
- Item counts on the Board's tiles; a parent's rollup, Ready to accept and progress (RM-0098).
- A card moving when its question is answered; anything waiting on an item.
- A page confirm on a local daemon (ADR-0035).
- The Questions screen sending `as` or linking a card; a card filter in the inbox (RM-0099's sweep may take them).
- A record's `card` slot (Q11).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{intake,record}.rs`; `crates/specengine-store/src/queue.rs`, `queue/{state,cards}.rs`; `crates/specengine-cli/src/{intake,main,card,review,state_file,stage,decide}.rs`; `crates/specengine-mcp/src/{intake,hub,cards,mirror}.rs`; `crates/specengine-http/src/{app,hub,answer}.rs`; `plugin/specengine/.claude-plugin/plugin.json`, `plugin/specengine/skills/ask-owner/SKILL.md`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `card_questions.rs` in store, CLI and http (new); M1's run at a62c81c and its three new tests; "Test churn"'s Rust rows; `fixtures/daemon-keys.json` |
| ui-developer | `src/api/provisional.ts` and the card and inbox types, `client.ts`, `http.ts`; `src/board/{CardPanel,CardDialogs,CardActs}.tsx`, `CardItems.tsx` (new, lazy), `labels.ts`; `src/inbox/ProposalCard.tsx`; `src/i18n/{en,ru}.ts`; `src/mocks/{cards,inbox}.ts`, `MockClient`; the tests of U1-U5 |
| test-engineer, UI | gates, mutations, U7; `policy.test.ts`, `words.test.ts`; `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/card-questions.md` "Acceptance criteria" with a criterion ID, or an existing canon heading; never "Data" or "Description and interactions", which compaction drops at ship (doc_pointers).

### Work order

1. The rust-developer and the ui-developer start together; the contract is fixed above. The UI runs on the mock, the new keys in `AHEAD_OF_FIXTURE` until the Rust lane's `daemon-keys.json` lands.
2. The Rust test-engineer first runs M1 at a62c81c and records it. Then the three `m1_*` tests, green on today's code. Then the Rust lane, overlapping the UI lane: S1-S5, K1, L1-L4, H1-H4, P1. Then the Rust review.
3. The UI lane completes; the UI test-engineer runs the gates and mutations (U1-U5, U7); then the UI review.
4. The orchestrator's look (U6) and D1, on the corpus copy.
5. Ship: the spec-writer ("Canon at ship").

At most three iterations a lane.

**The cut line**, if the UI lane runs out:
- **Moves to a new plan item**, `card-questions-ui`, the next free `RM-` number: Activity, the question in Return, Ask a question… and Record a decision…, Answer… and the Inbox link. The ship's spec-writer creates the item, `must` in `v5`, before the pipeline trial.
- **Ships**: the Rust part whole (schema 11, format 5, `--card`, `card_id`, `why`, `{{why}}`, `items`, the merged `history`, the route, the stage's `as`, the hub's plain write, MCP, plugin 0.36.0).
- **The UI lane still lands the types** (`daemonKeys.test.ts` with `AHEAD_OF_FIXTURE` empty) and U5's policy and words. History then shows an item's events by their raw type, as any unknown type.

### Risks

- **The migration** (D16): a project whose terminal, plugin or hub lags loses its queue there, and `selective` refuses every controlled write until the reinstall; the live hub at schema 10 needs its dump first.
- **Dedup on real corpora**: a card's target linked to an accepted decision makes every page question a hit; Ask anyway is the way through, and over 64 hits nothing is stored (D1 measures it).
- **The description cap**: `ask_question` sits at 2 041 of 2 048 bytes, the hub's at 2 038; "Data"'s trims leave 15 and 18.
- **The policy**: `policy.test.ts` bans event words in `src/board/`, so the type map sits in `labels.ts` and the scan widens to `proposal.*`.
- **ADR-0012**: K1's twin test is the guard; review checks that no card rule calls `card_items`.
- **No core logic in the UI**: the page judges no hit, cap or state, only the daemon's answer and its own last request.
- **The genre**: the chrome says "Ask a question", never a trade (U5).
- **Two stores**: a terminal's `--card` names the terminal's card, never a hub's.
- **Size**: 17.38 kB of headroom; the work item is in first load, so the blocks and dialogs go lazy (U7).
- **Churn**: about 23 Rust test files pin schema 10 or format 4; three key sets and three new; the plugin pins.
- **Anonymity**: the looks' real corpus is never named in the repository.
- **Caps** at ship: below.

### Canon at ship

`docs/canon/card-questions.md` loses "State" and gains the rest, at most 12 KB:
- the link and its refusals;
- the raise by CLI, MCP and the route;
- the stage's `as`;
- store and backup;
- the read, `items` and `history`;
- the `{{why}}` slot;
- the screen (Activity, Return, the acts, Answer…, the Inbox link);
- MCP and hub;
- Known limits: the rollout and the owner's upgrade, two stores, dedup's limits;
- Tests.

Each edit below stays within its cap; free bytes now in brackets. "Net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/proposal-queue.md` [4] | "Store": `QUEUE_SCHEMA_VERSION` 11, `PROPOSAL_COLUMNS` 48, the row `\| 11 \| card_id, why \| 48 \| card-questions.md "Store" \|` (net not positive) |
| `docs/canon/queue-backup.md` [226] | "Format": 5, the header, a format-4 dump's import; "Store" 48 |
| `docs/canon/tasks.md` [105] | "Backup": format 5 imports 4 |
| `docs/canon/agent-intake.md` [935] | "Tools": `card_id` in the three rows, `why` in `report_decision`'s; "Rules": the card's place in rules 2, 3 and step 7; "Stored": a pointer for the two columns |
| `docs/canon/agent-intake-documents.md` [8 740] | "Review document": `card_id`, `why` |
| `docs/canon/proposal-review.md` [7 385] | "Review document": the key list |
| `docs/canon/proposal-decision.md` [3 011] | "Raise": `why`, `--card`, the MCP texts' bytes |
| `docs/canon/decision-record-template.md` [7 458] | "Template": the `why` row and its rollout rule |
| `docs/canon/decision-staging.md` [3] | "Daemon": the body's `as`, a pointer (net not positive) |
| `docs/canon/cards.md` [199] | "Documents and events": `items` after `tasks`, `history`'s items; "Rules": no rule reads an item |
| `docs/canon/card-flows.md` [481] | "Return": a return may ask (pointer) |
| `docs/canon/board-ui.md` [5 720] | "Routes": the row; "The door": the answers; "Fence and trust": `CARD_CALLS` 11, the second exception, six plain writes |
| `docs/canon/board-screen.md` [273] | "Writes and dialogs": the Return part, the two acts, Answer… (pointer to `card-questions.md`) |
| `docs/canon/ui-design.md` [16] | "The work item": History becomes Activity (net not positive) |
| `docs/canon/hub.md` [16] | "Routes": the six plain writes, `items` |
| `docs/canon/hub-team.md` [2] | "Acts": six plain writes; "Stages": a sent `as` bound to the login (net not positive) |
| `docs/canon/hub-mcp.md` [2] | "Trust": raising proposals is the daemon's for `/mcp` and a card's items route (net not positive) |
| `docs/canon/serve.md` [2] | "Trust": the threat model adds raising a question or decision on a card under any name (net not positive) |
| `docs/canon/plugin-files.md` [5 165] | 0.36.0: the keys taken and returned |
| `docs/canon/plugin.md` [362] | `ask-owner`'s row: `card_id`, `why` |
| `docs/canon/round-ui.md` [3 959] | "Screen": the card's Answer… reuses the controls |
| `docs/canon/ui-languages.md` [901] | "Guards": `words.test.ts`'s "writer" ban |
| `docs/canon/hub-deploy.md` [3] | "0.35.0, `departments`'s" becomes "0.36.0, `card-questions`'s" (+3 B: trim) |
| `crates/specengine-http/README.md` [21] | "One door": a card's raise (`card_raise`) (net not positive) |
| `crates/specengine-store/README.md` [3], `crates/specengine-cli/README.md` [10], `crates/specengine-mcp/README.md` [10] | only where they name the schema, format or tool keys (net not positive) |
| `CLAUDE.md`; 08 s2 Phase 7 step 5; RM-0097 | shipped |
| this spec | compacted to at most 3 KB |

`docs/index-canon-card.md` gains the line at spec time already. `architecture.md#ui` changed with ADR-0077 at spec time.

### Working answers and the owner's questions

**The orchestrator's (Q1-Q5), taken:**
- Q1: a decision's `why` reaches its record through the opt-in slot `{{why}}`, filled for decisions only (D2, "Data").
- Q2: stages from the page send `as`, kept as a trailing `as` on `proposal.staged` (D8).
- Q3: on a local daemon an answer ends at a stage; the record comes from a terminal (D8).
- Q4: the question in Return requires a working answer and its price (D11).
- Q5: an answer is a stage on the question (D8).

**One spec, one ADR**, the lanes in parallel, the cut line above.

**The spec-writer's:**
- W1: `items` carries `decision_note`, a 16th key the analysis lacked: a rejected item's reason is its answer.
- W2: `{{why}}` is empty under `--answer`, since the record's choice is then the owner's.
- W3: the library call is `card_raise` and the handler `raise_on_card`. A missing card is a 404 before any intake rule (cards are never deleted); the store asks again in its transaction.
- W4: the route takes no `proposed_patch`, `task_id` or author field.
- W5: `StageBody.as` serves both stage routes; this slice's page sends it from the card and the inbox's decision dialog, not yet from the Questions screen.
- W6: no index on `card_id` (Scale).
- W7: an item's block stands at its `proposal.created`; items created before the history's 64 come first, under "Earlier items".
- W8: Ask anyway names the listed hits; with all named and nothing stored, the too-many line.
- W9: the Return part asks about the card's targets, 1 to 16 checked, and is offered only on a card with a target.
- W10: the two acts stand on any card with a target, open or closed, as D1 takes a card in any state.

**The owner's questions:**
- **Q-A.** Should the Questions screen send `as` too (W5)? Working answer: in RM-0099's sweep.
- **Q-B.** Should an answered question offer to move its card back (D10)? Working answer: no; the person moves it, and nothing waits (ADR-0012, ADR-0074).
- **Q-C.** Should a question name its addressee, a department or a person? Working answer: not in `v5` (Out of scope).
