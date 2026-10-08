---
class: spec
status: shipped
scope: [crates/specengine-http, crates/specengine-cli, ui]
ref: ui-round analysis 2026-10-08 (HEAD 4b9738c), the orchestrator's working answers and Q1-Q3 for the owner's review; 07 s3, 08 s2 Phase 4
shipped: 2026-10-08
---

# UI round

## Why

Open questions and discrepancies pile up while agents work on their working answers (ADR-0012); `round` took answers only as a file in a grammar on a terminal. Now the owner answers them on one screen and sends them at once; only the N `[y/N]` keys stay for the terminal (`spec round answer --staged`; ADR-0035, ADR-0038). Each answer keeps the one-item guarantees: stale choices refused, the round's queue checks, refusals alone, one event per stage, no alert for this tab's writes, no daemon path that walks or decides.

## Description and interactions

Built as `docs/canon/round-ui.md` describes (the two routes, the Questions screen); also `decision-staging.md`, `round.md`, the http, CLI and ui READMEs.

## Data

All canon now: `round-ui.md` "Sheet route" and "Answers route" (the bodies, statuses in order, keys, events); `fixtures/daemon-keys.json` 40 sets. No queue schema, event type or grammar change.

## Rules and edge cases

`round-ui.md` "Per answer" and "Known limits"; working answers W-1 to W-18 and Q1-Q3 (no Print, Copy or paste of the sheet; an option by number, `ok` the working answer; no "take every working answer" control) as taken.

## Acceptance criteria

Http: `tests/round.rs` unless named; M: the mutation that turned it red.

- [x] R-01 - GET `round`, `?limit=1` byte-equal `spec round new --json`; empty queue 200 (M: `limit` ignored) (`r01_the_sheet_is_the_cli_documents_bytes_and_limit_cuts_it`).
- [x] R-02 - Bad `limit`, another name: 400, no event (M: the range left to the CLI, 503) (`r02_a_bad_limit_or_another_name_is_a_400_and_runs_nothing`).
- [x] R-03 - 405 `Allow: GET` / `Allow: POST`, the 404 list names `round` (`methods.rs` `the_round_routes_answer_another_method_405_allow_get_on_the_sheet_post_on_the_answers`; `projects.rs` `ac06_an_unknown_slug_is_a_404_error_body_listing_the_served`).
- [x] R-04 - 403 before 413, 415, 413 unread, 400s, nothing staged (M: the body read before the origin check) (`r04_the_transport_refuses_before_anything_is_read_or_stored`).
- [x] R-05 - Four shuffled answers staged in round order, four events, git untouched (M: request order) (`r05_shuffled_answers_are_staged_in_round_order_and_change_nothing_else`).
- [x] R-06 - 20 entries: 2 staged, 18 refused alone (M: the last duplicate wins; one refusal fails all; `updated_at` read at POST) (`r06_each_unfit_answer_is_refused_alone_and_the_valid_ones_staged`).
- [x] R-07 - `replaced` names the earlier stage, one event (`r07_an_answer_on_a_staged_item_names_the_stage_it_replaced`).
- [x] R-08 - Each waits for its project's turn; a left client's GET dropped (`turn.rs` `r08_a_queued_round_dropped_by_its_client_never_runs`, `r08_the_round_and_its_answers_wait_for_their_own_projects_turn`).
- [x] U-01 to U-10 - `src/round/QuestionsView.test.tsx` (per criterion), `round.live.test.tsx` (U-04, U-06 over `HttpClient`), `drafts.test.ts` (U-03 to U-05), `text.test.ts` (U-08, U-09); `inbox/optionLabels.test.tsx`.
- [x] U-11, U-12 - `mocks/round.test.ts`, `round.smoke.test.tsx`; lint, build (496.97 kB), 74 files / 1 601 tests.
- [x] T-01 - The files above.
- [x] T-02 - `door.rs` `ui_round_t02_the_round_routes_change_nothing_and_a_valid_post_only_stages`, `ui_round_t02_the_scan_catches_a_round_walk_and_its_terminal_but_not_the_staging_call` (M: `round_answer` with an always-yes terminal).
- [x] T-03 - In R-05: `--staged` with `n` asks exactly the staged items, round order, keeps all.
- [x] T-04 - `daemon_keys.rs` `ac09_the_daemons_key_sets_are_fixtures_daemon_keys_json`, 34 -> 40 sets; `daemonKeys.test.ts` (M: `tasks` dropped).
- [x] T-05 - `no_domain.rs` `ac08_*` (the exemption, negative controls); CLI `proposal_genre.rs` `round_ac04_the_round_sources_name_no_prefix_kind_project_or_block`; CLI `round.rs` `ui_round_stage_answers_stage_in_round_order_against_the_updated_at_shown`, `ui_round_stage_answers_refuse_every_entry_of_an_id_named_twice_and_a_look_alike_alone`, `ui_round_a_step_3_refusal_ends_in_each_front_end_s_words_over_one_bare_reason`, `ui_round_the_answers_body_the_project_the_clock_and_the_limit_are_judged_first`.
- [x] T-06 - Http 77/77, CLI 609/609, eval 38/38; Rust mutations 9/9 red; UI's by hand.
- [x] D-01, D-02 - This spec; the canon above; 07 s3, 08 s2, `CLAUDE.md` State.

Not covered: a 503 on the round routes; the corrupt-row note; the same-second gap; step 7 after a daemon POST (CLI round AC-12 only); SSE delivery; 200 answers' `record_ready` cost; a full workspace run.

## Owner's manual check

`specengine-http --root <project>`, `pnpm --dir ui dev`; open `#/<slug>/questions`, pick answers, Stage, Copy; in a real terminal in the root `spec round answer --staged`: one question per staged answer.

## Out of scope

Paste, Print, Copy of the sheet, print tokens (Q1); browser confirmation; `--note`, `--canon` here; batch unstage; `update`, `create` items; an MCP tool; the `round` skill; the CR follow-up.

## Open

1. ADR-0038 stays accepted; the owner's explicit review is open (`round` "Open" 1).
2. Q1-Q3 and W-1 to W-18 await the owner's review.
3. Code cites this spec's "Data" for shapes now in `round-ui.md`: a later task re-points the citations.

## Implementation

Rust 1 iteration, UI 3 (2: 5 minors, 5 nits fixed; 3: the bar's outcome line, labels escaped in the Inbox); both reviews accepted.

| Module | What it does |
|---|---|
| CLI `round.rs`, `lib.rs` | `round_stage_answers`, `RoundAnswersBody::from_json` (decode, project, at most 200), `round_limit`; `queue_checked` (`Unfit`, the bare step 2-3 reason) shared with the text route; `stage_planned`; `Outcome::RoundAnswers` |
| http `app.rs`, `answer.rs` | `round`, `round_answers` (`same_origin`, `json_typed`, `ROUND_BODY_MAX`), `only_round_answers` 405, `round` in the 404 list; `RoundAnswers` 200 by type |
| UI `src/round/` | `QuestionsView` (lazy `RoundScreen`), `RoundItemCard`, `drafts.ts`, `memory.ts` (the Shell's store), `text.ts` (`shownSlot`, `utf8Bytes`) |
| UI `src/api/`, `mocks/`, `inbox/` | `getRound`, `stageRoundAnswers`, `useRound`, `useStageRoundAnswers`, `useRoundPosting`, sheets in `useLiveQueue`, `useStageDecision`'s `shown`; the mock; `shownSlot` on labels |

Deviations, accepted, each as `round-ui.md` words it: the 405 text (`answers` is a `LINK_TYPES` word); `spec: ` on the 400s; the over-200 text; an approve with `canon` judged by `stage_with`; a look-alike equal to another entry's ID a duplicate first; a corrupt row an answer names refused alone, `notes` only from the waiting tasks (Data said a note); the `no_domain.rs` exemption; the door's three type bans. UI: the hint once in a panel; blank own text left out; trimming before sending; "Keep my answer for this version"; the limit per project; the Shell's store; the sticky bar; labels escaped in the Inbox too; `NotBuilt` gone; a 0.38 kB lazy mock chunk; the "draft pushed past the limit" nit left as specified.
