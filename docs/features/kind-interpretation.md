---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli, ui]
ref: kinds analysis 2026-10-08 (W1-W11, slice A criteria; Q1-Q5 decided under the owner's pre-approval, review open); 06 s5.4; 08 s2 Phase 2
shipped: 2026-10-09
---

# Kind interpretation

## Why

06 s5.4: when an accepted decision becomes a rule only after interpretation, the proposal has kind `interpretation` and shows "how the decision was applied and why interpretation was needed". It was a plain `update`: the decision and the reading taken vanished at approval. Slice A of three (W1): this, then `kind-decision`, then `kind-amendment` ("Out of scope"). Groundwork for B and C: kind names only as core constants (the MCP enum too); behaviour keyed on `applies()`/`decides()` or the row's shape, never a kind list.

## Description and interactions

Canon: `docs/canon/proposal-interpretation.md` (propose, the decision test and its refusals, apply, the note on an update, the step-10 target, stage, round, tasks, backup, MCP, plugin, UI). An `update` naming an accepted decision (`class: decision`, live, `status: accepted`, never an `[ids]` kind) with two labelled texts; applied as an update, one commit `M <path>`, the trailer `Interprets: <ID>`; completion by `Proposal:` alone.

## Data

`docs/canon/proposal-interpretation.md` "Row and review" (queue step 6, the `interpretation` column `{decision, how, why}`, 44 columns, the review document's 46 keys and text block), "Apply" (the commit), "Elsewhere" (backup `queue_schema` 6); `fixtures/daemon-keys.json` (`Proposal` + `interpretation`, the `Interpretation` set).

## Rules and edge cases

`docs/canon/proposal-interpretation.md` "The decision", "Note on an update" (never a refusal, ADR-0012), "Known limits". Genre: no `"interpretation"` literal in `specengine-{store,cli,mcp}/src`.

## Acceptance criteria

A git copy of `fixtures/spec-a` (`RULE-STAM-REGEN` in `DEC-0023`'s `canon:` section, `DEC-0007` superseded by it). Tests: store `queue_interpretation.rs` (6), CLI `kind_interpretation.rs` (27), MCP `mcp_interpretation.rs` (2), additions named below; mutations 20 + 10 Rust red, the UI's by hand.

- [x] AC-R1 -- store `ac_r1_an_interpretation_applies_decides_nothing_and_is_stored_by_create`, `ac_r1_the_fields_go_with_an_interpretation_only_within_their_caps`.
- [x] AC-R2 -- `ac_r2_propose_interpretation_stores_pr_0001_and_refuses_an_id_change`.
- [x] AC-R3 -- `ac_r3_a_decision_that_is_not_live_and_accepted_is_refused`, `ac_r3_a_decision_superseded_after_propose_fails_step_4`, `spec_c_a_decision_is_told_by_its_class_not_an_ids_kind`, `a_decision_with_broken_front_matter_is_refused_not_a_panic`.
- [x] AC-R4 -- `ac_r4_how_and_why_are_required_not_blank_and_capped`, `ac_r4_bidi_controls_are_escaped_in_text_and_raw_in_json`; store `ac_r4_controls_are_stored_raw`.
- [x] AC-R5 -- store `ac_r5_a_corrupt_interpretation_column_is_named`; CLI `ac_r5_a_corrupt_interpretation_row_is_named_by_review_and_inbox`.
- [x] AC-R6 -- `ac_r6_approve_commits_the_body_and_interprets_last`, `..._the_consent_question_names_the_decision_first`, `..._an_interpretation_rebases_or_is_refused_with_the_conflict`, `..._a_hand_commit_with_proposal_alone_completes_it`, `..._a_completion_does_not_check_the_decision_again`, `..._a_decision_flag_on_an_interpretation_is_exit_2`, `the_consent_line_names_the_stored_decision_at_step_4s_path`.
- [x] AC-R7 -- `ac_r7_review_brief_and_inbox_carry_the_interpretation`.
- [x] AC-R8 -- `round.rs` `kind_interpretation_ac_r8_an_interpretation_is_a_change_proposal_of_the_round`; http `stage.rs` `kind_interpretation_ac_r8_a_staged_approve_holds_the_span_hash`.
- [x] AC-R9 -- `ac_r9_a_bound_interpretation_is_listed_and_refreshes_the_snapshot`.
- [x] AC-R10 -- `ac_r10_an_update_in_a_decisions_canon_section_is_noted_never_refused`, `ac_r10_spec_b_notes_adr_0001_and_interprets_it`, `ac_r10_the_step_10_note_names_interpretation_and_a_recorded_pr_is_refused`; `w7_in_review_is_judged_in_the_recorded_root`, `w7_in_review_survives_a_preview_stopped_at_step_3`, `w7_in_review_is_absent_when_the_recorded_worktree_is_gone`, `w7_never_flags_a_section_form_create`, `step_10_with_a_linked_update_names_it_not_an_interpretation`, `step_10_canon_target_follows_each_canon_form`.
- [x] AC-R11 -- `ac_r11_the_kind_enum_and_the_three_fields_are_listed`, `ac_r11_propose_change_takes_an_interpretation_as_its_twin`; `mcp_genre.rs` `kind_interpretation_the_change_kinds_are_no_literal_of_the_server`.
- [x] AC-R12 -- `plugin_skills.rs` `kind_interpretation_ac_r12_the_plugin_texts_name_the_kind_and_its_fields`; `plugin_files.rs` `PINS` 0.4.0.
- [x] AC-R13 -- `ac_r13_a_backup_with_an_interpretation_round_trips_byte_identical`; store `ac_r13_rows_with_an_interpretation_restore_byte_identical`.
- [x] AC-T1 -- the tests above.
- [x] AC-T2 -- `proposal_genre.rs` `kind_interpretation_ac_t2_no_interpretation_literal_outside_core`.
- [x] AC-T3 -- `mcp_door.rs` `ac08_reads_write_nothing_under_the_project_root`, http `door.rs` `ac03_no_endpoint_changes_the_repository_or_the_queue`, each with an interpretation raised over MCP.
- [x] AC-T4 -- http `daemon_keys.rs` `ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json` (41 names); UI `daemonKeys.test.ts`, unconditional.
- [x] AC-T5 -- store `ac_t5_a_schema_5_queue_opens_as_6_and_7_is_too_new`.
- [x] AC-U1 -- `ui/src/api/provisional.ts` `Interpretation`; `daemonKeys.test.ts`.
- [x] AC-U2 -- `inbox/interpretation.test.tsx` (7 cases); `policy.test.ts` (the kind-comparison mutation red).
- [x] AC-U3 -- `mocks/interpretation.smoke.test.tsx` (2).
- [x] AC-S1, AC-S2 -- this spec; the canon above, `proposal-{kinds,queue,apply}.md`, `decision-record.md`, `decision-staging.md`, `round.md`, `tasks.md`, `task-package.md`, `agent-intake.md`, `queue-backup.md`, `mcp-read.md`, `plugin.md`, the store, CLI, MCP, UI READMEs, 05-08, `CLAUDE.md`.

## Owner's manual check

Reinstall `specengine-mcp`, restart the daemon (queue schema 6), update the plugin to 0.4.0. `spec serve`, `pnpm --dir ui dev`, `?scenario=normal#/harbor-sim/inbox/PR-0049`: the card's Interpretation section, the stage dialog names `DEC-0012`; then over the daemon `spec propose interpretation` in a project and its card.

## Out of scope

Next slices, questions decided (owner's pre-approval, review open), not re-asked:

- **`kind-decision`** (Q1, Q2, W8, W9): a decision taken during work (agent or owner), confirmed on a terminal, recorded through `[decision_records]` like a question, the decision its working answer; no flag records `{"working_answer":true}`, `--answer` overrides, `--option` exit 2; an optional `proposed_patch` a linked update; record and canon diff two commits; dedup: question and decision one family; its own MCP intake tool; on the round sheet; a package assumption. `decide.rs:791-810` takes every deciding non-question for a discrepancy, `package.rs:470-477` assumes from question/discrepancy only: both change there.
- **`kind-amendment`** (Q3, Q4, W10): a file-form create with >= 1 `amends` link to a live node, its target never written, `A <path>`; the amended ID never in `target_ids`; applied when its document is Tier 3 by status; a producer of unapplied ones, the Health row measured (never 0 unmeasured); may slip to Phase 4/5.
- Here: interpretations on the round sheet; states `changes_requested|deferred|superseded`; follow-up tasks, `has_open_proposal`, edit-and-accept, MCP staging forms; a "decision without its canon diff" row; pilot B's amendment relation.

## Open

1. The owner reviews Q1-Q5 and W12-W14 (a target inside its decision's document refused; the document's `id:` stored; the consent line, `Interprets:` last, the key after `staged_at`).
2. The owner's manual check.

## Implementation

Rust two iterations, UI three; reviews accepted each.

| Module | What it does |
|---|---|
| core `intake.rs`, `proposal.rs`, `record.rs`, `check` | the kind constant, cap, fields, `Interpretation`, `with_article`; `commit_message_interpreting` and its labels, trailer; `REJECTED`; `CANON_LINK` |
| store `queue.rs`, `queue/state.rs`, `queue/stage.rs` | `ProposalKind::Interpretation`, step 6, the column and its decoding, 44 columns; stages by `applies()`, the match exhaustive |
| CLI `interpret.rs` (new) | propose, `decision_named`, `decision_step`, `canon_note`, `canon_target`, `recorded_canon_note` |
| CLI `preflight.rs`, `apply.rs`, `review.rs`, `propose.rs`, `decide.rs`, `proposals.rs`, `main.rs` | `StepRead` and the step-4 check; consent line and commit; the note (`plain_update`); step 10; the review key and text; the subcommand |
| MCP `intake.rs`, `mirror.rs`, `server.rs` | the enum from `ChangeKind`, the three fields, `INSTRUCTIONS` 1 889 B (2 046 with probes) |
| plugin | `propose-spec-change`, `spec-author`, 0.4.0 |
| UI `provisional.ts`, `ProposalCard.tsx`, `InterpretationSection.tsx` (new), `DecisionDialog.tsx`, `main.tsx`, mocks | the type, the section, the dialog line, the mock client loaded lazily, harbor-sim `PR-0049` |

Kind sites: store articles -> `with_article()`; `new_proposal_problem`'s base and new-ID rules `!= Create`, the fields required or refused; `stage.rs` by `applies()`; `preflight.rs:248`, `target_ids`, the linked patch's `Update`, the refresh: right as they were; `decide.rs:791-810` unreachable here, `package.rs:470-477` gives no assumption (no intake): slice B.

Deviations, all accepted: 1 the commit text in core; 2 more `--decision` wordings (an unknown or non-deciding `PR-`, a rejected one, a document without `class:`), an id-less decision stored by path; 3 the step-4 check after the span hash read, a completion never checks; 4 review's note from the recorded root's step-4 index, a gone worktree none; 5 the step-10 target by `canon_target`, only without a linked update; 6, 7 the skill and `CHANGE_DESCRIPTION` compressed, `INSTRUCTIONS` "a tail on narrowing them"; 8 the MCP field stays `decision`, `mcp_stage`'s guard admits it on `propose_change` only; 9 caps before Creation 1. UI: the decision read by `GET .../nodes/<ID>`; a lazy chunk; the dialog line; the mock round counts it a change; main bundle 424.61 kB, the mock client its own chunk. Left: the 3 review nits ("Known limits"); the UI's loading state without a skeleton.
