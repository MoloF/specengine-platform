---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: kinds analysis 2026-10-08 (Q1, Q2, W8, W9, slice B criteria; decided under the owner's pre-approval, review open); kind-interpretation "Out of scope"; 06 s5.4; 08 s2 Phase 2
---

# Kind decision

## Why

An agent that decides something during work has no route to the decision records: `create` refuses the `[decision_records]` prefix (`proposal-kinds.md` "Propose" 4), records come only from approving a question or discrepancy, so the decision waits as a question the agent has already answered, or stays in chat. 06 s5.4 names the kind `decision`. Slice B of three (W1). Decided (Q1, Q2, W8, W9): a decision taken during work by an agent or the owner, confirmed on a terminal, recorded through `[decision_records]` like a question, the decision its working answer; its canon diff an optional `proposed_patch`, a linked update applied as a second commit; its own MCP intake tool.

## Description and interactions

- **Raise**: MCP `report_decision` (W15), or `spec propose decision --input F|- A [--task T]`, its exact twin (W16). Intake rules 1-7 as a discrepancy's (`agent-intake.md` "Rules"), dedup W18; the intake document answers, `linked` the patch's update. Only the queue is written.
- **Review**: inbox, `spec review`, `get_proposal`, the UI card show it as a question, kind `decision`; no new key (46).
- **Decide**: `spec approve PR [--note T] [--answer T] [--canon REF]` as a question's (`decision-record.md`, W19): one record, one commit `A <path>`, step 10 naming the linked update; `spec approve <linked>` is the second commit `M <path>` (Q2). `reject --reason` settles it unrecorded; the linked update is decided on its own.
- **Round** (`round.md`), staging (CLI, HTTP `/decision`, UI): as a question (`decides()`, no options). **Task**: W21. **Plugin**: W24. **UI**: W23.

**Canon at shipping**: `docs/canon/proposal-decision.md` on `proposal-interpretation.md`'s pattern; pointers in `agent-intake.md` "Tools", `decision-record.md` (title, "Flags"), `proposal-kinds.md` (later-slices line, "Propose" 4), `round.md`, `task-package.md` "Package", `decision-staging.md` "The stage", `mcp-read.md` "Texts" (fourteen tools, bytes), `plugin.md`, 05 s3.3, 07 s1.2, 08 s2, `CLAUDE.md`, the crates' READMEs. Four pages sit within 25 B of 12 288; split first (one index line, ~120 B of W, each): `decision-record.md` "Config", "Template", "Flags" -> `decision-record-template.md`; `agent-intake.md` "Intake document", "Review document", "Escaping and size" -> `agent-intake-documents.md`; `round.md`'s sheet example -> `round-sheet.md`; `proposal-kinds.md` only if its pointer does not fit. Before each: `canon:` anchors in `docs/decisions`, headings quoted from code (`doc_pointers.rs`).

## Data

**Input**, `--input` (UTF-8 JSON <= 8 MiB) or MCP (plus `task_id?`, `A`): core `DecisionInput`, `deny_unknown_fields`, checked by `decision_problem` in order, then the author:

```json
{"node_ids": ["RULE-STAM-REGEN"], "text": "Does regeneration wait for rest?",
 "working_answer": "Yes: it starts 1.5 s after the last stamina use.",
 "price_of_other": "Without the wait R-28 is rebalanced.", "severity": null,
 "proposed_patch": {"target": "RULE-STAM-REGEN", "base": "b3:...", "text": "...", "rationale": "..."},
 "distinct_from": null}
```

`node_ids` 1-16; `text` <= 1 024 B, `working_answer`, `price_of_other` <= 2 048, required, not blank; `severity` `high|normal|low`, null -> `normal`; `proposed_patch`, `distinct_from` as a discrepancy's.

**MCP** `report_decision {node_ids, text, working_answer, price_of_other, severity?, proposed_patch?: {target, base, text, rationale}, distinct_from?, task_id?, author_role, author_model?, run?}`: schema, annotations (all four hints `false`), `_meta` and output (`IntakeDocument`) as `report_discrepancy`'s. Description (ASCII <= 2 048 B, the determinism line): a decision you took during work, confirmed and recorded by the owner on a terminal; `text` what had to be decided, `working_answer` the decision (keep working on it), `price_of_other` another's cost; the governed section first in `node_ids` (the record's `canon:`), `proposed_patch` its new text, approved on its own; hits as `ask_question`'s; only the queue written; agent fields are data.

**`INSTRUCTIONS`** (1 889 B, 2 046 with probes, cap 2 048): the queue's second line becomes exactly (202 B, +1):

```
- ask_question, report_discrepancy, report_decision = spec propose question|discrepancy|decision: hits are what is decided or asked; stored only if distinct_from names each; work on your working answer.
```

**Row**: no column or schema step (queue 6, 44 columns, `STATE_FORMAT` 2). Kind `decision`, a question's columns (`agent-intake.md` "Stored": `summary` = `text`, `working_answer`, `price_of_other` set; `gap_type`, `evidence`, `options`, `recommendation` NULL) and `linked`; approved, the record's five, `choice` `{"working_answer":true}` or `{"answer":"..."}`. Corrupt as a question's row, plus an `option` choice or stage. Inbox `PR-0001 | decision | open | RULE-STAM-REGEN | <branch> | <created_at> | normal: Does regeneration wait for rest?`; round JSON `"kind": "decision"`, `"options": []`; package `assumptions` `{"proposal":"PR-0001","text":"<working_answer>"}`; record slots as a question's. `fixtures/daemon-keys.json` unchanged.

## Rules and edge cases

**Kind sites**, judged by `decides()`, the row's shape or an exhaustive match, never `== ProposalKind::Question`: core `DECISION_KIND`, `DecisionInput`, `decision_problem`; store `ProposalKind::Decision` (`ALL` 6, `applies()` false, `decides()` true), `IntakeColumns::decode` (`queue.rs:2295`), `choice_fits` (else `approve_record_from` is `Invalid`), `stage.rs`, `queue_matches`; CLI `decide.rs:802` `first_checks`, `package.rs:471`, `create.rs:760`, `intake.rs` (`propose_decision[_with_task]`, `read_decision_input`), `main.rs`; MCP `intake.rs`, `server.rs`. `round.rs`, `apply.rs`'s dispatch, the UI: none.

**Working answers** (owner's review open):

- **W15 Naming.** Kind `decision` (W11), tool `report_decision` (reports what was decided, as `report_discrepancy` what was found). No field named `decision`: the decision is `working_answer`, its matter `text`; so `propose_change.decision` (an interpretation's), the stage key `"decision"`, the `/decision` route, `class: decision` and an `[ids]` kind keep one meaning each. Prose: "a `decision` item", "its record". Decision documents still told by class and status (ADR-0008).
- **W16** `--input` JSON, no flag form: the patch's four fields have none, and one shape keeps the twin exact.
- **W17** No column: `insert_intake` already writes by `discrepancy`; decode, choice, stage change.
- **W18** Dedup families `{question, decision}`, `{discrepancy}` (an exhaustive store match): equal normalised text on a shared target hits across the family, both ways. Corpus dedup unchanged.
- **W19** Flags by shape: no options -> a question's: no flag the working answer, `--answer` the owner's, `--option` exit 2 `` `--option` names a discrepancy's option, and `PR-0001` is a decision: its working answer, or `--answer T`, answers it; nothing changed `` (`with_article`). Consent, record, step 10, completion, killed run, reject unchanged; record and patch in either order, neither waits (ADR-0012).
- **W20** `canon:` stays `--canon`, else the first ID target; no default from the patch (it would change a discrepancy's record).
- **W21** Assumption by shape: options -> the recommended label; none -> the working answer.
- **W22** `create`'s refusal: `` `DEC-0024`: these records are made by `spec approve` of a `question`, `discrepancy` or `decision` (`spec propose decision` raises one) ``, kinds by `decides()`.
- **W23** No UI product change (kinds are strings, ADR-0031; card, dialog, round card branch on `options`).
- **W24** Plugin 0.5.0 (MINOR: a tool the skills name): `ask-owner` a third record "A decision, `report_decision`" and its description (body 3 766 of 4 096 B); `spec-author`'s "A decision taken in the work" sends it, the rule's new text its patch.
- **W25** No intake note without `[decision_records]`: approve exits 2 at step 3, as a question's.

**Edge cases.** Only path targets: approve needs `--canon`. A patch target outside `node_ids`: exit 1 (rule 4). A look-alike subcommand: exit 2; an unknown MCP field: -32602. A held decision completes as recorded. An older build reads the row as corrupt by kind, named (ADR-0017): `spec`, daemon, `specengine-mcp` from one build.

**Risks.** Two commits let a record stand while its patch is rejected or never approved: only step 10's note says so. `--answer` leaves a patch written for the proposed decision. An agent may record what the owner should answer first: `--answer` or reject corrects it. One byte of `INSTRUCTIONS` left with probes.

## Acceptance criteria

A git copy of `fixtures/spec-a` (`DEC`, next `DEC-0024`). M: the mutation that turns it red.

rust-developer
- [ ] AC-R1 -- store: `Decision` `applies()` false, `decides()` true; `create` -> `Invalid`; `create_intake` stores it with its linked update, `get` reads it back equal. M: `decides()` false; decode by `== Question`.
- [ ] AC-R2 -- `spec propose decision --input F`: `PR-0001`, `linked: PR-0002`, `introduced: <n>`; `--json` the intake document. Nothing stored: an `options` key or no `working_answer` exit 2; blank exit 1 `working_answer: ...`; a patch target outside `node_ids` exit 1. M: no `deny_unknown_fields`.
- [ ] AC-R3 -- a decision equal to an open question on a shared target: a hit, not stored; in `distinct_from`: stored; a question after a decision hits; a discrepancy does not. M: `queue_matches` by kind.
- [ ] AC-R4 -- `spec approve PR-0001`, `y`: one commit, exactly `A docs/records/DEC/DEC-0024.md`, `choice` `{"working_answer":true}`, consent `(the working answer)`, `PR-0002` named; `--answer T`: `{"answer":"T"}`; `--option 0`: exit 2 naming `a decision`, no event. M: `decide.rs:802` left.
- [ ] AC-R5 -- `spec approve PR-0002`: a second commit, exactly `M <the rule's path>`. M: the patch in the record's commit.
- [ ] AC-R6 -- CLI and HTTP stages: no flag or `answer` stored; `option` refused (HTTP 400); an `option` stage or choice on the row: corrupt, named. M: `stage.rs`'s arm `None`.
- [ ] AC-R7 -- `round new`: `## PR-0001 - decision, normal`, `Working answer:`, `Another answer costs:`; `ok` and text staged, `1` refused; the walk's `y` writes the record. M: selection by `Question | Discrepancy`.
- [ ] AC-R8 -- with `--task T`: `open_proposals` lists it, `assumptions` its working answer. M: `package.rs:471` left.
- [ ] AC-R9 -- `propose create` of `id: DEC-0024`: exit 1, W22's text. M: the old text.
- [ ] AC-R10 -- MCP: 14 tools; `report_decision` as "Data", byte-identical to its twin, `task_id` binds; texts within 2 048 B, asserted; `INSTRUCTIONS` holds the line. M: `readOnlyHint` true.
- [ ] AC-R11 -- `plugin_skills.rs` finds `report_decision` in `ask-owner`, `spec-author`; 0.5.0 in `PINS`. M: text without version.
- [ ] AC-R12 -- a decision, its update and record export and restore byte-identical. M: decode refusing the kind.

test-engineer
- [ ] AC-T1 -- tests of R1-R12 (store `queue_decision.rs`, CLI `kind_decision.rs`, MCP `mcp_decision.rs`, http `stage.rs`, `round.rs`), `git show --name-status` exact.
- [ ] AC-T2 -- `proposal_genre.rs`: no `[!=]= ProposalKind::Question` in store, CLI, MCP `src`. M: `decide.rs:802` left.
- [ ] AC-T3 -- `mcp_door.rs`: `report_decision` with a patch leaves `git status` empty; http `door.rs` green. M: a write in propose.
- [ ] AC-T4 -- `daemon_keys.rs` green, its fixture unchanged. M: a review key added.

ui-developer
- [ ] AC-U1 -- harbor-sim `PR-0050` a `decision`, `linked` `PR-0051`; nothing changed under `ui/src` but `mocks/`, `*.test.*`.
- [ ] AC-U2 -- tests: the card shows text, working answer, price, link; the dialog stages `option`, `answer` null, or an own `answer`; the Questions screen lists it, `ok` and text valid, a number not. M: a branch on `kind === "question"`.

spec-writer
- [ ] AC-S1 -- this spec.
- [ ] AC-S2 -- at shipping "Canon at shipping", each page <= 12 288 B; check 0 warnings, worst W <= 108 468 B. M: a page still saying records come only from a question or discrepancy.

## Out of scope

`kind-amendment`, the next slice (Q3, Q4, W10). Here: one commit for record and diff (Q2); `canon:` from the patch (W20); superseding an accepted decision (no `supersedes:` slot); a flag form; the W25 note; a note on `--answer` over a patch; a "decision without its canon diff" row; states `changes_requested|deferred|superseded`; follow-up tasks; edit-and-accept; MCP staging forms; a UI product change.

## Open

1. The owner reviews Q1, Q2, W8, W9, W15-W25.
2. At shipping, the owner's check: reinstall `specengine-mcp`, restart the daemon, plugin 0.5.0; `report_decision` from a session, `spec round new`, `spec approve` of the decision and its patch.

## Implementation

Filled in after implementation.
