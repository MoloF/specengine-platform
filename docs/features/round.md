---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli]
ref: round analysis 2026-10-08 (HEAD 8e53d8c), working answers taken by the orchestrator for the owner's review; 06 s5, 07 s2, 08 s2 Phase 2
shipped: 2026-10-08
adrs: [ADR-0038]
---

# Question round

## Why

Agents keep working on their working answers (ADR-0012), so open questions and discrepancies pile up across tasks, and the owner settled them one `spec review` and `spec approve` at a time, in the engine's words, on a terminal. The owner wants them on one sheet in the project's own language, ordered by what matters, to print, mail or read on a screen, answered in one sitting, while each answer is still confirmed by its own key (ADR-0035). 06 s5 planned the round; this slice built both CLI parts on shipped machinery: intake fields, stages, decision records. No new kind, no new storage.

## Description and interactions

`spec round new [--limit N] [--json]` (read-only sheet) and `spec round answer FILE | --staged` (terminal only: parse, check, stage, report, then each item's own question), as `docs/canon/round.md` describes. Reused: `spec inbox`'s scope, intake fields, the package's `open_proposals` rule read the other way, the stage library, the staged approve/reject flows, apply steps 2-3. Agents: nothing new (no MCP tool, no skill, `round` stays reserved, no gate change); `get_proposal` shows a round's stage, `ask-owner` counts it as no answer. The UI part is `ui-round` (Out of scope).

## Data

All canon now: `docs/canon/round.md` ("Selection" and its notes, "Sheet", "JSON" with the item's 19 keys, "Answers" with the grammar and every refusal, "Run" with the report, the walk, the run's JSON, `--staged`). No queue schema change (5 stays); events: one `proposal.staged` per staged answer, then approve's and reject's own.

## Rules and edge cases

The canon's ("Answers", "Run", "Known limits"), `decision-staging.md` "Rule" and `architecture.md#apply` (ADR-0038). 06 s5.3's batch confirmation is superseded by one key per answer; s5.2's R and A records, cancelled assumptions and rephrasing are Out of scope.

## Acceptance criteria

Per test, a scratch `HOME` and git repository; terminal runs under a pseudo-terminal. Tests: CLI `tests/round.rs`, core `tests/round.rs`, CLI `tests/proposal_genre.rs`.

- [x] AC-01 - Open: Q1, D1; also a held question, a rejected and an applied one, an orphan's, another repository's (same slug), an open `update`, an open `create`. `round new --json` lists exactly Q1, D1; `notes` name the held one, the orphan, the other repository's and the two change proposals; the rejected and applied IDs appear nowhere (M: list `approved` items) (`ac01_the_sheet_lists_open_questions_and_discrepancies_of_this_repository_only`).
- [x] AC-02 - H `high`; N1 `normal`, newer, its target in a `ready` task's snapshot; N2 `normal`, older, its target only in a `done` task's: text and JSON order H, N1, N2 (M: count `done` tasks) (`ac02_severity_then_waiting_tasks_then_age_order_the_sheet`; core `order_key_severity_then_waiting_then_age_then_number`).
- [x] AC-03 - No injected clock (the sheet reads none): two runs, text and `--json`, 1.2 s apart, byte-equal, today's date in neither; `git status --porcelain`, `HEAD` and the queue's event count unchanged (M: print the time or an age) (`ac03_the_sheet_is_byte_deterministic_and_writes_nothing`).
- [x] AC-04 - On `fixtures/spec-b`: a question's Cyrillic summary and a discrepancy's Cyrillic labels byte-equal, with the target titles; no `b3:`, worktree path, branch name or `gap_type` value in the text; no node-kind or ID-prefix literal and no `block` in the round's sources (M: print the worktree) (`ac04_project_text_travels_verbatim_and_the_sheet_shows_no_place`; genre `round_ac04_the_round_sources_name_no_prefix_kind_project_or_block`).
- [x] AC-05 - 25 items: 20 listed, note `5 more item(s) not on this sheet: spec round new --limit 25`, JSON `more` 5; `--limit 0`, `201` exit 2 (M: drop the note) (`ac05_the_limit_cuts_the_sheet_with_a_note_and_bounds_itself`).
- [x] AC-06 - Q1's summary holds a line `PR-0009: 1` (an open discrepancy): the sheet, fed back unedited, stages nothing: no `proposal.staged`, no question, every ID `left open`, exit 0 (M: agent text not indented) (`ac06_a_planted_answer_line_in_agent_text_answers_nothing`).
- [x] AC-07 - One file: `ok`, `1` (a discrepancy), own text, `reject: dup`, `?`, blank, an indented and a quoted `PR-0006: ok`, a pair `PR-0007: ok` / `PR-0007: no`: each reported; exactly four staged (working answer, option 1, the text, the reject); PR-0006 untouched; PR-0007 refused naming both lines. A 2 049-byte answer and one holding U+202E: refused, named, nothing staged (M: the last duplicate wins; truncation) (`ac07_every_answer_form_in_one_file`; core `duplicates_refuse_every_line_and_a_blank_slot_is_no_answer`, `caps_are_refused_with_the_byte_count_never_cut`).
- [x] AC-08 - A discrepancy, options 0-2: `ok`, free text and `5` refused naming `0-2`, nothing staged; `[2]` and `2` stage option 2 (M: `ok` takes the recommendation) (`ac08_a_discrepancy_takes_an_option_by_number_only`).
- [x] AC-09 - Answers to an item applied after the sheet, an open update's ID, `PR-9999`, a U+0420 look-alike of `PR-0003`, and two valid ones: the four refused and named (the commit; `spec approve`; `no proposal`; `PR-0003`); the two staged and asked (M: one stale item refuses the file) (`ac09_each_stale_unknown_or_look_alike_answer_is_refused_alone`).
- [x] AC-10 - Stdin a pipe, `FILE` a FIFO with no writer: exit 2 at once with the consent message, `--staged` too. On a terminal: a sparse 8 388 609-byte file exits 2 naming the cap; `project:` missing or another slug: exit 2, no stage, no event (M: the terminal checked after staging) (`ac10_no_terminal_exits_2_before_the_file_or_the_queue_is_read`, `ac10_on_a_terminal_the_file_and_the_project_line_exit_2_with_nothing_staged`).
- [x] AC-11 - Three answers (`ok`, own text, `[1]`), keys `y`, `n`, `y`: three questions, each ending `(..., staged) on <branch> in <worktree>? [y/N]` (a reject's: `, staged)? [y/N]`); two records in two `spec: apply` commits, each name-status `A <path>`; the second item `open` with its stage; `round: 2 applied, 0 rejected, 1 kept staged, 0 refused`, exit 1 (M: one question for the round) (`ac11_one_key_per_answer_on_a_terminal`).
- [x] AC-12 - Two answers; before the first `y` the test restages the first: step 7 refuses it, named; the second is asked and applied (M: the walk stops) (`ac12_a_stage_replaced_while_asked_is_refused_and_the_walk_goes_on`).
- [x] AC-13 - `--staged`, two staged questions (staged in reverse round order) and a staged `update`: two questions in round order; the update in a note, not asked (M: the update is asked) (`ac13_staged_asks_the_staged_questions_in_round_order_and_notes_an_update`).
- [x] AC-14 - ADR-0038 1 235 B with "Cost", `canon:` `docs/canon/architecture.md#apply`, whose text names one `[y/N]` per decision and `spec round answer`; `decision-staging.md` "Rule" names it; `export index`, `check` clean (M: an ADR over the cap).

Also: CLI `a_stage_replaced_before_its_question_is_refused_unasked`, `the_end_of_input_keeps_the_rest_staged`, `round_stage_checks_and_stages_with_no_walk`, `approves_meet_apply_steps_2_and_3_read_only_and_a_reject_still_settles`, `another_repository_s_item_is_refused_and_an_orphan_takes_only_a_reject`, `a_corrupt_row_is_a_note_and_refuses_only_its_answer`; iteration 2: core `the_lines_of_one_id_share_one_list`, `a_malformed_line_naming_a_canonical_id_counts_toward_its_duplicates`, `reject_not_followed_by_its_colon_is_refused`; CLI `a_refusal_names_at_most_ten_lines_and_counts_the_rest`, `four_thousand_answers_for_one_item_give_a_linear_report` (132 B a line, ~58 ms debug), `reject_without_its_colon_is_refused_in_the_report`, `an_orphan_s_kept_reject_names_spec_reject_alone`, `staged_notes_another_repository_s_and_a_gone_one_s_staged_items_unasked`, `a_fifo_swapped_in_after_the_size_check_never_holds_the_open`. Tests: core 18, CLI 31, genre 1; core + CLI nextest 1 084/1 084. Mutations: 30, all red (19 in iteration 1, 11 in iteration 2). Not covered: `record_ready`'s prefix check (unreachable from a config).

## Owner's manual check

1. In a project with open questions: `spec round new > sheet.md`; answer two slots, leave one; `spec round answer sheet.md` in a real terminal (not Claude Code's Bash tool): the report, then one question per answer; `y` writes a record and a `spec: apply` commit, `n` keeps the stage (`spec inbox`: `open (staged)`); `spec round answer --staged` asks it again.
2. A reply with `pr-0003: ok` and a look-alike ID: both refused, named, the rest staged.
3. Two `y` typed fast: the second answers the next question (Known limits).

## Out of scope

`ui-round`, a Phase 4 slice: `GET .../round` (= `round new --json`), `POST .../round/answers` (steps 2-4 only, behind the decision route's fence; `door.rs` admits it as staging only), the Questions screen (sheet, Print, Copy, paste, outcomes, the `--staged` command), print tokens in `tokens.css`. Also: one consent for a round; stored rounds; `update` and `create` items; clarification, defer, `interpretation`; R or A records, `@assumes`, follow-up tasks; MCP tools; the `round` skill; natural-language parsing; multi-line answers; `--note`, `--canon` in the grammar.

## Open

1. **ADR-0038 stays accepted; the owner's explicit review is open.** It rests on working answers: one key per answer in one run, confirmed inside `round answer`. Another answer is a new ADR superseding it.
2. Working answers for the owner: nothing stored (no round table, the file not kept); options keyed as printed (`[1]` or `1`); English frame words; `ok` and `reject:` any case; a blank slot is no answer; 8 MiB file cap; exit 1 when anything is refused or kept.
3. `libc`: `O_NONBLOCK` is a constant per target and type-ahead is not flushed, because 04 s6 limits `libc` to `specengine-eval`'s `ra` feature; allowing it in the CLI (pinned, an ADR) would fix both.
4. Follow-up task: intake accepts CR in a question's text, a decision record refuses it; such an approve stages but never applies.
5. `ui-round`: N `proposal.staged` events from one paste against the outside-change alert's one per write; the 16 384-byte body cap against a quoted reply.

## Implementation

Canon: `round.md` (built), `proposal-apply.md` "Consent", core and CLI READMEs, `CLAUDE.md` State; 06 s5, 07 s2, 08 s2. Iterations: Rust 2; review 1 (consent model holds), changes taken in iteration 2.

| Module | What it does |
|---|---|
| core `round.rs` (new) | bounds; `order_key`; `reading_minutes`; `answers_text` (UTF-8, the failing line); `parse_answers` -> `Answers {projects, answers}`, `Token`, `Refusal` (duplicates: one shared line list per ID) |
| CLI `round.rs` (new) | `round_new` (selection, notes, `Waiting` read once, sheet, JSON); `round_answer` (file `O_NONBLOCK`, grammar, project line, queue checks, stage, report, walk via `RoundTerminal`); `round_stage` (steps 2-4, for the daemon later); `RoundSheet`, `RoundRun` |
| CLI `stage.rs` | `stage_with` split out of `stage`, same checks and order; `staged_command` shared |
| CLI `decide.rs` | `record_ready`: apply steps 2-3 read-only, the `[decision_records]` prefix included; step-3 texts shared |
| CLI `package.rs` | `wanted_nodes`, `lists_proposal` factored out; `assemble` unchanged |
| CLI `lib.rs`, `main.rs` | `Outcome::{RoundSheet, Round}`; `spec round new|answer` (FILE or `--staged`), the terminal check before the file; `Console` (report stdout; questions, walk stderr; `y`/`yes`; end of input) |

Deviations, accepted: the report after staging (a stage lost at step 3 shows refused); notes name IDs per class (10, then `and k more`); a stage replaced before its question refused unasked; `?` counts toward duplicates; option tokens ASCII only (`[ 1 ]`, fullwidth digits: other text); one clock reading per run; exit-2 texts `<file>:`/`<file>:<line>:` ... `; nothing changed` (`round_stage`: `line N:`); JSON `answers[].stage` null unless this run staged it, a kept step's `reason` its kept line (`end of input: ` first on Ctrl-D). Rejected: a malformed line not counted as a duplicate.

Iteration 2: duplicate lists shared and capped at 10 named lines (iteration 1 was quadratic: 4 000 lines, 542 MB); a malformed line counts toward its Latin ID's duplicates; `reject` not followed directly by `:` refused (`rejected ...` stays text); a gone repository's kept line `spec reject <ID>`; `--staged` notes another and a gone repository's staged items; the file opened `O_NONBLOCK`, judged again on its handle; `record_ready` checks the prefix; `Waiting` sets precomputed, items rendered once. Known limits: canon "Known limits".
