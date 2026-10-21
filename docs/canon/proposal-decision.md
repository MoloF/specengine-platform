---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
owner: owner
reviewed: 2026-10-15
---

# Proposal kind: decision

Phase 2 (2026-10-09, 06 s5.4): a decision taken during work, by an agent or the owner, raised for the owner to confirm on a terminal and recorded through `[decision_records]` as a question's answer is. The decision is its working answer; its canon diff an optional `proposed_patch`, a linked `update` the owner approves on its own, a second commit. Kind `amendment`: `proposal-amendment.md`. Code: core `intake.rs` (`DECISION_KIND`, `DecisionInput`, `decision_problem`); store `ProposalKind::Decision`, `intake_shape()`, `IntakeShape` (`null_columns`, `problem`); CLI `intake.rs` (`DecisionRequest`, `propose_decision[_with_task]`, `read_decision_input`), `decide.rs` (`first_checks`), `create.rs` (`deciding_kinds`), `package.rs`; MCP `intake.rs` (`report_decision`).

## Naming

`decision` names, each in one place: this kind (`DECISION_KIND`; the tool `report_decision`); a document's `class: decision` (an accepted decision is told by class and status, never by kind, ADR-0008); a project's `[ids]` kind; an interpretation's `decision` (`propose_change`); the stage key `"decision"` (`approve` or `reject`) and the HTTP `/decision` route. No field of this kind is named `decision`: the decision is `working_answer`, its matter `text`. Prose: "a `decision` item"; what approval writes, "its record".

## Raise

`spec propose decision --input F|- A [--task T]` (`A`, `--task`, exits: `agent-intake.md`); the intake document, `--json` too. `F`: UTF-8 JSON <= 8 MiB (`INTAKE_INPUT_MAX_BYTES`), the MCP arguments but `A` and `task_id`, core `DecisionInput` (`deny_unknown_fields`):

```json
{"node_ids": ["RULE-STAM-REGEN"], "text": "Does regeneration wait for rest?",
 "working_answer": "Yes: it starts 1.5 s after the last stamina use.",
 "price_of_other": "Without the wait R-28 is rebalanced.", "severity": null,
 "proposed_patch": {"target": "RULE-STAM-REGEN", "base": "b3:...", "text": "...", "rationale": "..."},
 "distinct_from": null}
```

No flag form: the patch's four fields (text <= 1 MiB) have none, and one shape keeps the MCP twin exact. Intake rules 1-7 as a discrepancy's (`agent-intake.md` "Rules"): form (an unknown or missing field: exit 2); caps in order (`decision_problem`): `node_ids` 1-16, `text` <= 1 024 B, `working_answer`, `price_of_other` <= 2 048, required and not blank, `severity` `high|normal|low` (null: `normal`), `proposed_patch` (rule 4: its target among `node_ids`), `distinct_from`, then the author; targets; place; corpus and queue dedup. Stored: the item, then the patch's `update`, `linked` both ways. Only the queue is written.

**MCP** `report_decision {node_ids, text, working_answer, price_of_other, severity?, proposed_patch?: {target, base, text, rationale}, distinct_from?, task_id?, author_role, author_model?, run?}` = `spec propose decision --input - A [--task T]`, answering as its twin (`IntakeDocument`). Its own argument type: `additionalProperties: false` on the root and `proposed_patch`, `severity` the enum; the intake writers' annotations (`readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` `false`), `_meta` `maxResultSizeChars` 500 000. An unknown field: an error result `failed to deserialize parameters: unknown field ...` (rmcp's, not -32602); a refusal (a patch target outside `node_ids`): an error result with the reason. It never stages, decides or writes under the root. Description 1 744 B of 2 048: a decision you took during work, for the owner to confirm and record; `text` what had to be decided, `working_answer` the decision (keep working on it), `price_of_other`; the governed section first in `node_ids` (the record's `canon:`); `proposed_patch` the rule's new text, approved apart, its own commit.

## Row and dedup

No column or schema step of its own (`STATE_FORMAT` 2). `ProposalKind::Decision`: `family()` `Decides`; `create` -> `Invalid`, `create_intake` stores it. `intake_shape()`, exhaustive over every kind, gives a deciding kind's `IntakeShape`: `Question` for `question` and `decision` (`summary` = `text`, `working_answer`, `price_of_other`; `gap_type`, `evidence`, `options`, `recommendation` NULL), `Discrepancy` (`price_of_other` NULL). The shape is also the queue's dedup family: step 7 compares the rows whose kind shares the item's shape, so a decision and a question of equal normalised text on a shared target hit each other, a discrepancy neither. Corpus dedup unchanged. Reading such a row, `create_intake`'s shape refusals: `agent-intake.md` "Stored".

Review document: `"kind": "decision"`, a question's keys, none added (46). Inbox `PR-0001 | decision | open | RULE-STAM-REGEN | <branch> | <created_at> | normal: Does regeneration wait for rest?`.

## Approve

`spec approve PR [--note T] [--answer T] [--canon REF]` as a question's (`decision-record-template.md` "Flags"; `first_checks` judges by the item's fields): no options, so no flag takes the working answer (`{"working_answer":true}`, consent `(the working answer)`), `--answer T` the owner's (`{"answer":"T"}`; the `cost` slot its `price_of_other`); `--option N` exit 2, no event: `` `--option` names a discrepancy's option, and `PR-0001` is a decision: its working answer, or `--answer T`, answers it; nothing changed ``. Only path targets: `--canon` required. `canon:` is `--canon`, else the first ID target, never taken from the patch. Steps, consent, ID, completion, killed run, reject: `decision-record.md`; one record, one commit, name-status exactly `A <dir>/<ID>.md`; step 10 names the linked update still to approve.

**The canon diff** is that update: `spec approve <linked>` applies it as an update (`proposal-apply.md`), a second commit `M <path>`; the record's commit holds no canon file. Either may be approved first, neither waits for the other (ADR-0012). `reject PR --reason` settles the decision unrecorded, the reason the owner's answer; the linked update stays, decided on its own. Without `[decision_records]` approve exits 2 at step 3 naming the table; `reject` settles it.

## Elsewhere

- **Stage** (`decision-staging.md`): a question's: approve with no flag or an `answer`; `option` a usage refusal (HTTP 400), a stored `option` stage or choice on the row corrupt; `span_hash` null.
- **Round** (`round.md`, `round-sheet.md`): `Decides`, so on the sheet as a question: `## PR-0005 - decision, normal`, `Working answer:`, `Another answer costs:`; `ok`, own text, `reject:`; a number refused (no options). JSON `kind` `decision`, `options` `[]`. The empty sheet keeps `No open questions or discrepancies.`
- **Task** (`task-package.md` "Package"): `--task`, `task_id` bind it; `open_proposals` lists it, `assumptions` its `working_answer` (by the item's fields).
- **Kind lists** from `family()` `Decides` (`deciding_kinds()`): `create` of the record prefix: `` `DEC-0024`: these records are made by `spec approve` of a `question`, `discrepancy` or `decision` (`spec propose decision` raises one); nothing stored ``; a decision flag on an applying kind: `` `--option` decides a `question`, `discrepancy` or `decision`; `PR-0002` is an update: `spec approve PR-0002` applies it as proposed; nothing changed ``.
- **Backup** (`queue-backup.md`): rows as stored; a decision, its update and record round-trip byte-identical. An older build reads a `decision` row as corrupt by its kind, named (ADR-0017): `spec`, the daemon and `specengine-mcp` from one build.
- **MCP texts** (`mcp-read.md` "Texts"): `INSTRUCTIONS`' queue line `- ask_question, report_discrepancy, report_decision = spec propose question|discrepancy|decision: hits are what is decided or asked; stored only if distinct_from names each; work on your working answer.`; `ask_question`'s description (2 041 B) counts a queued decision among hits and related.
- **Plugin** 0.5.0 (`plugin.md`): `ask-owner`'s third record, "A decision, `report_decision`", told apart from an accepted decision hit; `spec-author` sends a decision taken in the work with the rule's new text as its patch.
- **UI**: no product change. Kinds are strings (ADR-0031); the card, the decision dialog and the Questions screen branch on `options`. Mock: harbor-sim `PR-0050`, its update `PR-0051`; the inbox dialog stages the working answer, an own answer only on the Questions screen.

## Known limits

Accepted at shipping (2026-10-09); none blocks.

- Two commits let a record stand while its patch is rejected or never approved: `canon:` names an unchanged section, only step 10's note says so (no "decision without its canon diff" row).
- `--answer` leaves the linked patch written for the proposed decision: the owner rejects or approves it apart; no note.
- A record supersedes nothing (no `supersedes:` slot): overriding an accepted decision is a question for the owner.
- No intake note without `[decision_records]`; the round's empty sheet names questions and discrepancies only; some doc comments in the sources still name two deciding kinds; `ask_question`'s description lists a queued decision tersely.
- `INSTRUCTIONS` keeps 31 B (`mcp-read.md` "Tools"), `ask-owner`'s body 37 B, the skills' descriptions 30 B.
