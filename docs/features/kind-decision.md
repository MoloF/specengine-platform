---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, ui]
ref: kinds analysis 2026-10-08 (Q1, Q2, W8, W9, slice B criteria; decided under the owner's pre-approval, review open); kind-interpretation "Out of scope"; 06 s5.4; 08 s2 Phase 2
shipped: 2026-10-09
---

# Kind decision

## Why

An agent that decided something during work had no route to the decision records: `create` refuses the `[decision_records]` prefix, records came only from approving a question or discrepancy, so the decision waited as a question the agent had already answered, or stayed in chat. 06 s5.4 names the kind `decision`. Slice B of three (W1), after `kind-interpretation`, before `kind-amendment`. Decided (Q1, Q2, W8, W9): a decision taken during work by an agent or the owner, confirmed on a terminal, recorded through `[decision_records]` like a question, the decision its working answer; its canon diff an optional `proposed_patch`, a linked update applied as a second commit; its own MCP intake tool.

## Description and interactions

Canon: `docs/canon/proposal-decision.md` (naming, raise, row and dedup, approve, the canon diff, stage, round, task, kind lists, backup, MCP texts, plugin, UI, known limits); the splits made room: `decision-record-template.md` (config, template, flags), `agent-intake-documents.md` (intake and review documents, escaping), `round-sheet.md` (the sheet, a question and a decision).

## Data

`docs/canon/proposal-decision.md` "Raise" (the `--input` JSON, `report_decision`'s arguments), "Row and dedup" (a question's columns, no schema step; `intake_shape()`); `docs/canon/agent-intake.md` "Stored" (row reading by shape, `create_intake`'s shape refusals); `fixtures/daemon-keys.json` unchanged.

## Rules and edge cases

**Kind sites**: every deciding-kind judgement by `decides()`, the item's fields or the exhaustive `intake_shape()`, never `== ProposalKind::Question` (AC-T2). Working answers W15-W25, in the canon: W15 naming (`decision` the kind, `report_decision` the tool, no field named `decision`); W16 `--input` JSON, no flag form; W17 no column; W18 dedup families by shape; W19 flags by the item's fields, record and patch in either order; W20 `canon:` never from the patch; W21 assumptions by the item's fields; W22 the prefix refusal names the three kinds; W23 no UI product change; W24 plugin 0.5.0 (MINOR); W25 no intake note without `[decision_records]`.

## Acceptance criteria

A git copy of `fixtures/spec-a` (`DEC`, next `DEC-0024`); spec-b and spec-c where named. Tests: store `queue_decision.rs` (11), CLI `kind_decision.rs` (20), MCP `mcp_report_decision.rs` (4), additions named below. Mutations: 20 Rust (14 + 6), 10 UI, all red. Workspace 2194/2194, UI 1636.

rust-developer
- [x] AC-R1 -- store `ac_r1_a_decision_decides_and_never_applies`, `ac_r1_create_refuses_a_decision_unwritten`, `ac_r1_a_decision_is_stored_with_its_linked_update_and_reads_back_equal`, `a_decision_row_holds_a_question_columns`.
- [x] AC-R2 -- `ac_r2_the_binary_stores_a_decision_and_its_linked_update`, `ac_r2_refusals_store_nothing`, `propose_decision_is_the_unbound_twin`.
- [x] AC-R3 -- store `ac_r3_a_question_and_a_decision_are_one_family_a_discrepancy_another`; `ac_r3_a_question_and_a_decision_hit_each_other_a_discrepancy_neither`.
- [x] AC-R4 -- `ac_r4_option_on_a_decision_is_exit_2_nothing_changed`, `ac_r4_the_binary_asks_on_the_terminal`, `ac_r4_spec_b_records_adr_0003_and_the_patch_apart`, `spec_c_without_records_approve_exits_2_and_reject_settles`, `edge_a_held_decision_completes_as_recorded`, `edge_a_path_only_decision_needs_canon`.
- [x] AC-R5 -- `ac_r4_r5_the_record_then_the_patch_two_commits`, `ac_r4_r5_the_patch_first_then_the_given_answer`.
- [x] AC-R6 -- store `ac_r6_a_decision_stage_takes_an_answer_never_an_option`, `ac_r6_a_decision_record_takes_its_working_answer_or_an_answer`, `ac_r6_a_corrupt_decision_row_is_named`; `ac_r6_a_staged_decision_takes_an_answer_never_an_option`, `ac_r6_a_corrupt_decision_row_is_named_by_review_and_inbox`; http `stage.rs` `kind_decision_ac_r6_a_staged_decision_takes_an_answer_never_an_option`.
- [x] AC-R7 -- `ac_r7_the_round_asks_a_decision_as_a_question`.
- [x] AC-R8 -- `ac_r8_the_task_package_assumes_the_working_answer`.
- [x] AC-R9 -- `ac_r9_create_refuses_the_record_prefix_naming_the_three_kinds`; `iteration2_a_decision_flag_on_an_update_names_the_deciding_kinds`.
- [x] AC-R10 -- `ac_r10_report_decision_is_listed_as_data_says`, `ac_r10_t3_report_decision_answers_as_its_twin_and_writes_only_the_queue`, `ac_r10_a_patch_outside_node_ids_is_an_error_result`, `iteration2_ask_question_names_decisions_among_hits_and_related`; the `INSTRUCTIONS` pins (1 890 B, 2 047 with probes).
- [x] AC-R11 -- `plugin_skills.rs` `kind_decision_ac_r11_ask_owner_and_spec_author_send_a_decision`, `kind_decision_iteration2_ask_owner_tells_an_accepted_decision_from_a_decision_record`; `plugin_files.rs` `PINS` 0.5.0.
- [x] AC-R12 -- store `ac_r12_a_decision_its_update_and_record_round_trip_byte_identical`; `ac_r12_a_backup_with_a_decision_round_trips_byte_identical`.

test-engineer
- [x] AC-T1 -- the tests above; store `a_row_holding_a_column_its_shape_lacks_is_named_corrupt`, `create_intake_refuses_an_item_of_another_shape_unwritten`; `iteration2_a_row_holding_a_column_its_shape_lacks_is_named_by_review_and_inbox`.
- [x] AC-T2 -- `proposal_genre.rs` `kind_decision_ac_t2_no_kind_compared_with_a_question` (`Question` and `Decision`).
- [x] AC-T3 -- `ac_r10_t3_report_decision_answers_as_its_twin_and_writes_only_the_queue`; http `door.rs` green.
- [x] AC-T4 -- http `daemon_keys.rs` `kind_decision_ac_t4_a_decision_is_served_with_the_fixture_key_sets` (41 names, the fixture unchanged).

ui-developer
- [x] AC-U1 -- harbor-sim `PR-0050`, `PR-0051` (`mocks/harbor-sim/fixtures.ts`); `mocks/round.ts`, `mocks/tasks.ts`; no product file changed.
- [x] AC-U2 -- `mocks/decision.smoke.test.tsx` (12): the card, the inbox dialog (the working answer), the Questions screen (an own answer, a number refused), both clients.

spec-writer
- [x] AC-S1 -- this spec.
- [x] AC-S2 -- `proposal-decision.md` (new), the three split pages, `decision-record.md`, `agent-intake.md`, `round.md`, `proposal-{kinds,queue,apply,interpretation}.md`, `task-package.md`, `tasks.md`, `mcp-read.md`, `plugin{,-files}.md`, the root, store, CLI, MCP READMEs, 05-08, `CLAUDE.md`.

## Owner's manual check

Reinstall `specengine-mcp`, restart the daemon, update the plugin to 0.5.0. `spec serve`, `pnpm --dir ui dev`, `?scenario=normal#/harbor-sim/inbox/PR-0050`: Accept stages the working answer; `#/harbor-sim/questions` lists it. In a project: `report_decision` from a session with a patch, `spec round new`, then `spec approve` of the decision and of its patch: two commits.

## Out of scope

`kind-amendment` (Q3, Q4, W10), next. Not here: one commit for record and diff (Q2); `canon:` from the patch; a record superseding an accepted decision; a flag form; an intake note without `[decision_records]`; a note on `--answer` over a patch; a "decision without its canon diff" row; states `changes_requested|deferred|superseded`; follow-up tasks; edit-and-accept; MCP staging forms.

## Open

1. The owner reviews Q1, Q2, W8, W9 and W15-W25.
2. The owner's manual check.

## Implementation

Rust two iterations, UI one (mocks and tests only); reviews accepted each.

| Module | What it does |
|---|---|
| core `intake.rs` | `DECISION_KIND`, `DecisionInput`, `decision_problem` |
| store `queue.rs`, `queue/stage.rs` | `ProposalKind::Decision`; `intake_shape()`, `IntakeShape` (`null_columns`, `problem`): row decode, `choice_fits`, the dedup family, `create_intake`'s shape check; the stage arm |
| CLI `intake.rs` | `DecisionRequest`, `propose_decision[_with_task]`, `read_decision_input` |
| CLI `decide.rs`, `package.rs`, `create.rs`, `apply.rs`, `main.rs` | flags and record sources by the item's fields; assumptions by shape; `deciding_kinds()` for the prefix refusal and `flag_on_apply`; `Propose::Decision`, help texts |
| MCP `intake.rs`, `server.rs`, `lib.rs` | `report_decision` (own argument type, 1 744 B), `ask_question` 2 041 B, the `INSTRUCTIONS` line, the tool list |
| plugin | `ask-owner` (third record, "An accepted decision" hit, body 4 059 of 4 096), `spec-author`, 0.5.0 |
| UI `mocks/` | harbor-sim `PR-0050`, `PR-0051`, round and task mocks, `decision.smoke.test.tsx` |

Deviations, all accepted: 1 W22's text ends `; nothing stored`; 2 `report_decision` has its own argument type; 3 round help texts name decisions; 4 `flag_on_apply` lists the deciding kinds by `decides()` (iteration 2); 5 stricter row reading: a column the shape holds none of is corrupt, named, and `create_intake` refuses an item not of its kind's shape (a gap the tests found, iteration 2); 6 an unknown MCP field is an error result, not -32602; 7 the inbox dialog stages no own answer (the Questions screen does); 8 the MCP test file is `mcp_report_decision.rs` (`mcp_decision.rs` was taken). Draft figures corrected: `INSTRUCTIONS` 1 890 B (2 047 with probes), `ask-owner` body 4 059 B. Left (nits): doc comments in `review.rs`, `preflight.rs`, `round.rs`, `inbox.rs`, `mirror.rs`, model `task.rs`, store `lib.rs` still name two deciding kinds; `ask_question`'s description line on decisions is terse; the empty round sheet keeps `No open questions or discrepancies.`
