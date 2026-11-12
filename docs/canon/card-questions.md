---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card questions: the items a card names, and its Activity

ADR-0077: a card asks, and the answer is a recorded decision. The card: `cards.md`; the items: `agent-intake.md`, `proposal-decision.md`; a record: `decision-record.md`; a stage: `decision-staging.md`.

<a id="the-rule"></a>
## The rule

A question, a discrepancy or a decision may name one card, its `card_id`, set when it is raised and never changed. The card is one of the same queue, in any state; no other kind names one, and an item names at most one. A decision may also say why it was taken. Its record names no card: the card reads its items back, each with its state and, once approved, its record.

The page raises a question or a decision on a card through the card door: one write under the acting name, the typed one locally (ADR-0057), the login on a hub, where it is a plain write any login makes. A return that asks is two writes, the move first. An answer is a stage on the item, confirmed as any stage is: on a terminal, or on a hub by a Confirm its `[team]` allows (ADR-0035, ADR-0061).

The link decides nothing. No card move, accept, assign, cancel or clear, no `allowed`, rollup or gate reads a card's items, and no stage, approve, reject or confirm reads an item's card. A card with an open question moves and is accepted as any other; an answered question moves no card (ADR-0012). ADR-0077.

## Store

Queue schema 11 (`STEP_11`) adds TEXT `card_id`, then `why`: `PROPOSAL_COLUMNS` 48, `proposal_columns(8..=10)` 46; all due steps in `migrate`'s one transaction, no index. Both `NULL` but where set:
- `card_id`: a question, discrepancy or decision raised with a card (a departing accept's discrepancy names its card); never changed by approve, reject, stage, unstage, reopen, a completion or an import. A linked update holds none.
- `why`: a decision's alone (`ProposalKind::holds_why`, `Intake.why`), at most 2 048 bytes (`ANSWER_MAX`), not blank when given, stored as given.

`create_intake`, in its transaction after the task binding, before the dedup: no card row of the project -> `QueueError::CardRefused {id, reason: "no card C-0099"}`, nothing written, no ID taken; `card_id` is judged against the kind and grammar too, `NewIntake.actor` (`card_raise`'s trailing `as`) by the person grammar; `IntakeShape::problem` refuses a `why` off a decision (`` a question holds no `why` ``). A departing accept checks its card in its own transaction.

**Corrupt**, named (`get`, `list` fail; `inbox` skips it, a note; `card_items` lists it unreadable): `` proposal PR-0001: the stored `card_id` cannot be read: `` `it is set on an update, which names no card` or `` `X-7` is no card ID ``; `` the stored `why` cannot be read: it is set on a question, which holds none ``. A blank or over-long `why` from a hand edit reads as stored.

**Review document**: `card_id`, `why` after `interpretation`, `null` where absent; text lines `card_id: C-0007`, `why: <text>` after `staged_at`'s. The inbox's entries are unchanged. **Backup**: from format 5, 48 columns; format 4 imports with both `NULL` (`queue-backup.md` "Format").

## Raise

`--card C` on `spec propose question|discrepancy|decision` (`agent-intake.md` "Tools"): `The card (C-0001) this is raised for: one of this queue's, in any state`. `card_id` in any `--input` is an unknown field (exit 2), as `why` is in a discrepancy's. In the intake's rules (`agent-intake.md` "Rules"):
- **Rule 2**, after `distinct_from`, before the author: the written ID through `parse_card_id`. A look-alike exits 2 with the card commands' line, prefixed `card_id: `; no ID in any script exits 1, `` card_id: no card `C-07`: a card ID is `C-` and 4 or more digits, as `spec card list` lists it ``.
- **Rule 3**: `[ids]` claiming `C` exits 2 (`card_prefix_clash`) whenever `--card` is given.
- **Step 7**, in the store's transaction: no such card exits 1, the intake document with `id` `null`, `created` false, the last note `card_id: no card C-0099`; no event, no ID.

A decision's `why`, in `DecisionInput` after `price_of_other`, is judged after it: `why: blank: give a value`, `why: 2049 bytes; at most 2048`.

**MCP**, stdio and a hub's `/mcp` alike: `ask_question`, `report_discrepancy`, `report_decision` take `card_id` (`["string","null"]`) after `distinct_from`, `report_decision` `why` after `price_of_other`; a hub's still take no `task_id`. `get_proposal` returns the two keys, `get_card` `items` (`CardItemShape`). Plugin 0.36.0; `ask-owner` names both keys (`plugin.md` "Skills").

<a id="the-card-door"></a>
## The card door

`POST /api/projects/:p/cards/:id/items`, `CardRaiseBody {kind, node_ids, text, working_answer, price_of_other, why, severity, distinct_from, as}`, `deny_unknown_fields`:

- `kind` `question` or `decision`, else 400 `` `kind`: a card raises a `question` or a `decision` ``; a `why` on a question, 400 `` `why` goes with a `decision` ``.
- `node_ids`, `text`, `working_answer`, `price_of_other` required, by the intake's rules; `severity`, `distinct_from`, `why` optional. No `proposed_patch`, `task_id` or author field: a person raises from the page.
- `as`, trimmed, judged by `persons_named`; `null` reads `local`.

The door is `board-ui.md` "The door"'s, then one `card_raise` call (the eleventh of `CARD_CALLS`; handler `raise_on_card`) in the project's turn, the card read first, the place the served root's. 200: the intake document `{id, created, hits, related, linked, diagnostics, notes}`, stored or not; 400: a body or a person refused; 404: `{id, reason}`, `no card C-0099`, before any intake rule; 409: an intake rule's refusal, the document, its reason the last note; 503: cannot run (the CLI's exit 2: a look-alike, `[ids]` claiming `C` or `PR`, a corrupt row, a newer schema); another method: 405, `Allow: POST`. The row's author is `human`, no role, model or run, on a hub with its `login`; the event `proposal.created {"id":"PR-0012","as":"dev"}`, every other raise's staying `{"id":…}`. On a hub it is the sixth plain write: `hub.rs` `WRITES` holds `items`, `hub::acting` binds `as` to the login, `[team]` is not read.

## Stage

`StageBody` takes an optional `as` (the card person grammar; off it, 400 `spec: as: <problem>; nothing changed`) in `POST .../proposals/:id/decision` and in each stage of `POST .../round/answers`; `unstage` takes none. Absent or `null`: `proposal.staged {"id","staged","staged_at"}` byte for byte; given, a trailing `"as"`. On a hub the login stands, and a body's `as` naming another is a 403.

## Read

`spec card show C [--history] --json`, `get_card`, `GET .../cards/:id`: `items` after `tasks`, the items naming it by ID number, read now, no git: `[{id, kind, status, summary, working_answer, why, author, created_at, updated_at, staged_at, decided_by, decided_at, decision_note, record_id, record_path, record_title}]`, each value the row's, `author` its object. Text: `items: PR-0012 | question | applied | <summary> | record DEC-0031` per item after `tasks:`, `items: -` with none. A row that does not read is left out with a note, `PR-0014: its row does not read (<column>: <reason>); not listed`; `show` exits 0. Store `card_items(id) -> CardItems {items, unreadable}`, `item_history(ids, limit)`. A parent's `subtask_items`: `card-mandate.md`.

`history` (`--history`, else `null`): the card's last 64 `card.*` events and its items' last 64 `proposal.*` events (a payload whose `id` is an item's), merged by `seq`, oldest first; a linked update's are not an item's. `list`, its rows and every write's document are unchanged. Scale: one scan per card read, never on the Board's `list`; unmeasured.

## Record

`{{why}}` (`decision-record-template.md` "Template"), body only: a decision's `why` when the record takes its working answer; empty under `--answer` (the choice is then the owner's), for a question, a discrepancy, a decision without one. A record names no card (Q11).

## Screen

- **Activity** replaces the work item's History, its blocks and dialogs lazy (`CardItems.tsx`). Items whose `proposal.created` is not in `history` come first, under "Earlier items", by ID; then every `history` entry in served order: a `card.*` as before; a `proposal.*` its word (`labels.ts` `ITEM_EVENT_WORDS`), the item's ID and "by <who>" (the payload's `as`, else `author.login`, else `author.role`); an unknown type raw under `lang="en"`. A `proposal.created` opens the item's block: `kind` raw in a neutral badge under `lang="en"`, the status lozenge, the summary, the working answer and why; Staged <at> in the shared date format; the record as a `DocLink` by `record_title` with `decided_by`; a `decision_note`, the answer on a rejected item; Answer… on an open item. The card is read again on every `proposal.*` live event and after a confirm.
- **Return** (`ReturnDialog`, first load), on a card with a target: "Ask a question with it", closed (open on a binding task), the question, a working answer and what another answer would cost, all required, and the targets, at least one checked. The move first, then the raise; a refused move asks nothing; a raise failed or not stored leaves the card returned and the dialog open on its fields, its hits listed, Ask anyway (naming them in `distinct_from`) or Ask again (no answer) resending the raise alone; every hit named and nothing stored: the too-many line. The stored ID is announced with the return.
- **Ask a question…, Record a decision…** follow Return… on any card with a target, open or closed, a mandate leading and framing them (`card-mandate.md` "Screen"); Record sends `why` or `null`, required by the form on an open task; hits as in Return; a 400 or 409 in the dialog's alert, its fields kept.
- **Answer…**: the Questions screen's controls by structure (`round-ui.md` "Screen"), each with its price, staged through `stageDecision` with `as`; then the local command and Copy (`decision-staging.md` "UI"), or a hub's Confirm (the lazy `ConfirmDialog`) where `allowed` lists `confirm`; off (`ActOff`) where it lacks `stage`. At most one primary button; off while a card write is out.
- **Inbox**: a proposal shows "Task C-0008", a link to `#/<p>/board/C-0008`, and a decision's Why.
- Focus never lands on a read-only field or the body; the status region always exists. Nothing outside `labels.ts` compares a kind, status or event type (`policy.test.ts`); the chrome says "Ask a question", never a trade (`words.test.ts`).

## Known limits

- **Rollout.** Older binaries refuse a newer queue (`SchemaTooNew`; the hook's gate gives no verdict, so `selective` refuses controlled writes until reinstall) and a newer dump; an older `spec approve` refuses a `{{why}}` template. Every terminal, plugin and hub upgrades from one commit (`card-tasks.md` "Known limits"' steps), the old build and its last dump the way back; a hub dumps first (`card-mandate.md` "Known limits").
- **Who asked.** A local Activity names whoever "Acting as" holds; empty goes out as `local`. Follow-up: default to the hub login, or ask once locally.
- **Two stores.** A card ID names a card of one queue: a terminal's `--card` never reaches a hub's card. A hub filled from a terminal's dump holds its cards and items as stored.
- **Dedup.** A card's target linked to an accepted decision makes every question on it a hit: Ask anyway is the way through; over 64 hits nothing is stored (`agent-intake.md` "Dedup").
- The Questions screen links no card: `RoundItem` holds no `card_id`, a core key.

## Tests

`tests/card_questions.rs` in core, store, CLI and http; CLI `queue_state.rs`; MCP `mcp_intake.rs`, `mcp_report_decision.rs`, `mcp_cards.rs`, `mcp_hub.rs`; http `stage.rs`, `door.rs`, `hub_team.rs`, `hub_confirm.rs`, `hub_mcp.rs`; UI `src/board/{CardActivity,CardAsk,CardAnswer,ReturnDialog}.test.tsx`, `inbox/ProposalCard.test.tsx`, `policy.test.ts`, `i18n/words.test.ts`. Criteria: `docs/features/card-questions.md`.
