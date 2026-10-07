---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: decision-staging analysis 2026-10-06, the owner's decisions as recommended; readiness check 2026-10-07; 08 s2 Phase 2
shipped: 2026-10-08
adrs: [ADR-0034, ADR-0035]
---

# Decision staging

## Why

The UI could not decide: the decision POST answered 403 and the card offered a command to copy. The owner decided (2026-10-06): no authentication inside SpecEngine, ever (ADR-0034); a decision is **staged anywhere, confirmed only on a terminal** (ADR-0035). This slice built it: one optional staged choice per open proposal, written by `specengine-http`, shown and confirmed by the existing `spec approve|reject` `[y/N]`. Supersedes `daemon-read` AC-03 (the POST's 403). Working answers Q1-Q5 (readiness check 2026-10-07) taken: Q1 the compare-and-set key stays `updated_at`, the gap documented; Q2 a 409 carries the refused review document; Q3 `ask-owner` line, plugin 0.1.6; Q4 a completion records the staged flags; Q5 no `--answer`, `--canon` from the UI.

## Description and interactions

The owner stages in the UI's dialog; `spec approve|reject PR` on a terminal confirms. Agents read `staged`, `staged_at` (`get_proposal`, `spec review --json`); no tool stages; `ask-owner` "Whose words count": a staged choice is no answer. A stage never touches a task's `task_id`, snapshot or package.

## Data

All of it is canon now: `docs/canon/decision-staging.md` ("The stage", "Queue": schema 5, store ops, events; "Daemon": the CLI library, the route; "Terminal"; "Staleness"; "Documents": review, inbox, keys, backup; "UI").

## Rules and edge cases

The canon's ("Rule", "Queue" incl. the 1-second gap, "Terminal" incl. the completion's compare, "Threat model").

## Acceptance criteria

- [x] AC-01 -- docs: ADR-0034 (1 333 B), ADR-0035 (1 248 B) with `canon:` and "Cost"; the scan `specengine/[t]oken|first write [e]ndpoint` over live docs, READMEs, `crates/*/src`, `ui/src` finds nothing; every touched canon within cap (`spec check` clean).
- [x] AC-02 -- a schema-4 DB opens as 5, both NULL; `PROPOSAL_COLUMNS` 43 = `PRAGMA table_info`; schema 6 exit 2 (store `queue_stage.rs` `ac02_schema_5_holds_43_columns_and_a_schema_4_database_opens_as_5`; CLI `stage_confirm.rs` `ac02_a_queue_of_a_newer_schema_exits_2_naming_both`; http `stage.rs` `ac02_..._answers_503_to_post_and_delete`).
- [x] AC-03 -- staging an open `update`, `question`, `discrepancy` -> 200 with `staged`; `git status --porcelain` empty, `HEAD`, branches, status unchanged; one `proposal.staged` each, seen by a subscriber < 1 s; `door.rs`, `no_domain.rs` green (http `stage.rs` `ac03_...`).
- [x] AC-04 -- each refusal of `decision-record.md` "Flags", a decision flag on an update or a create, via POST: the CLI's message, 400 or 409, nothing stored; a path-only question's approve 400 (http `ac04_...`).
- [x] AC-05 -- `approved`, `applied`, `rejected` -> 409; an orphan's approve 409, its reject stored; unknown ID 404; a stale `updated_at` 409 with the current document (http `ac05_...`).
- [x] AC-06 -- a second POST replaces the stage, its event the new one; DELETE NULLs both, one `proposal.unstaged`; DELETE, nothing staged: 200, no event (http `ac06_...`; store `stage_sets_replaces_and_unstage_clears_with_one_event_each`).
- [x] AC-07 -- pty, `{option: 1, note: "n"}` staged on a question: `spec approve PR` prints `staged <at>:`, the command, the option line, the question marked `staged` (a record's mid-line: `(option 1, staged) on <branch> in <worktree>? [y/N]`); `y` -> record option 1, note `n`, stage NULL, `.approved` with `staged_at`; `n` -> exit 1, stage kept; no TTY -> exit 2 unread (CLI `ac07_a_staged_approve_is_shown_and_confirmed_on_a_terminal`).
- [x] AC-08 -- option 1 staged, a typed option -> the unused-stage note, the typed option, empty note (CLI `ac08_...`).
- [x] AC-09 -- reject staged: `spec reject PR` stores its reason in `decision_note`; nothing staged exit 2 naming `--reason`; approve staged exit 2 naming `spec approve` (CLI `ac09_*`, two).
- [x] AC-10 -- the stage replaced between prompt and `y` in the same second -> exit 1, no commit: at step 7 `` `PR-0001` not applied (step 7): PR-0001 changed since the question: its staged choice was replaced or removed; nothing changed ``, at a completion alike (CLI `ac10_*`, two; `a_completion_refuses_a_stage_that_changed_during_the_question`).
- [x] AC-11 -- staged at hash H, the target changed after -> the note at the prompt and in review, applied as it rebases; changed only before -> no note; a file-form create's `span_hash` null (CLI `ac11_*`, three).
- [x] AC-12 -- leaving `open` clears the stage, no `.unstaged`; staged on another state: a named corrupt row; export, import, export byte-identical with stages; format-1 (schemas 1-3), format-2 schema-4 dumps restore, stages NULL; format 1 at schema 4 refused; a schema-4 DB exports its tasks (store `ac12_*`, four; CLI `ac12_*`, three).
- [x] AC-13 -- `http.ts` (16 types), the http README, `proposal-queue.md` list the same 7 proposal events; a stage event re-reads that inbox and proposal, no task read (UI `http.test.ts`, `live.test.tsx`, `queries.test.tsx`).
- [x] AC-14 -- UI: accept, option 2, a note -> one POST, its body exactly the stage plus `updated_at`; the card: Staged, the command, Copy (no free text), Unstage (DELETE); still listed; a 409 -> `ClientError` 409, the last note; clarification, defer send nothing; the mock alike; 34 key sets (`InboxView.test.tsx`, `staging.live.test.tsx`, `client.test.ts`, `ownStages.test.ts`, `MockClient.test.ts`, `daemonKeys.test.ts`).
- [x] AC-15 -- `tools/list` has no stage tool; `get_proposal` = `review --brief --json` with both keys; `ask-owner` names a stage no answer, 0.1.6 in `PINS`; a POST with a foreign `Origin`, `text/plain`, an oversize body or no `Sec-Fetch-Site` -> refused, nothing stored, no `Access-Control-*` (mcp `mcp_stage.rs`, two; http `ac15_...`, `the_page_is_judged_before_the_path_is_decoded`, `methods.rs`; `plugin_files.rs`).
- [x] AC-16 -- an `open` proposal, approve staged, completed by its own commit: the question ends ` (staged)? [y/N]`; `y` -> the staged flags recorded, `.approved` with `staged_at` (CLI `ac16_...`).

Also: CLI `a_staged_value_with_lf_cr_or_tab_is_shown_on_one_line`, `a_question_staged_with_an_answer_and_canon_is_recorded_with_them`. New Rust tests: store `queue_stage.rs` 7, CLI `stage_confirm.rs` 17, http `stage.rs` 7, mcp `mcp_stage.rs` 2. Workspace: 1 947 passed, 20 skipped; fmt, clippy (+ probes) clean. UI: 66 files, 1 484 tests; lint, build clean (main chunk 492.41 kB, no warning). Mutations red: Rust 15 named + 3 (iteration 2), UI 20. `quoted()` without the LF case stays green by design: `Staging::preface` escapes the same characters again (defence in depth); only removing both turns it red.

## Owner's manual check

1. **Schema 5 upgrade**: the first run of this build migrates the queue to schema 5; older binaries then refuse it (`SchemaTooNew`): stop the daemon, reinstall `spec`, `specengine-mcp`, `specengine-http` from one commit (`spec export state` first keeps a backup).
2. `specengine-http --root <project>`, `pnpm --dir ui dev`: Accept with an option and a note -> "Staged <at>", `spec approve PR-...`, Copy, Unstage; the proposal stays listed.
3. On a terminal, `spec approve PR-...` prints the staged line and the option, then `[y/N]`; `n` keeps the stage, `y` applies it.
4. A stage from a second tab raises the alert in the first; Unstage; `?scenario=conflict`.

## Out of scope

Task staging; `changes_requested`, `deferred`; `--answer`, `--canon` from the UI (Q5); MCP elicitation, URL mode (08 Phase 5); a raw keypress; `spec stage`; a Unix socket; age-based cleanup; any authentication (ADR-0034).

**Owner's text for `.claude/agents/ui-developer.md`** (lines 32-33): "the owner's choice, staged, is the UI's only writing action; before it a diff, after it the staged choice and its `spec` command, confirmed on a terminal (ADR-0035);".

## Implementation

Canon: `decision-staging.md` (built), `proposal-{apply,queue,kinds}.md`, `queue-backup.md`, `tasks.md`, `architecture.md#apply`, `#ui`; store, CLI, http, UI and root READMEs; `CLAUDE.md` State; 06, 07 s3, 08 s1-s2. Iterations: Rust 2, UI 2; review 1 accepted with changes, all taken in iteration 2.

| Module | What it does |
|---|---|
| store `queue/stage.rs` (new) | `Stage`, `StagedChoice`, strict decode, `Stage::problem`, named corrupt rows, `stage_if`, the two events |
| store `queue.rs`, `queue/state.rs` | schema 5 (`STEP_5`), 43 columns; `Seen.staged`, `Decision.staged_at`; `stage_from`, `unstage_from`; CAS ops compare the stage; `stored_rows` at `>= 4` |
| CLI `stage.rs` (new) | `stage`, `unstage`, `StageBody`, `StageOutcome`, `StageCause`; `Staging`: one-line preface, marks, `replaced`, `changed_since`; the 4 096 caps |
| CLI `apply.rs`, `decide.rs`, `create.rs`, `preflight.rs`, `review.rs` | staged flags through apply, record, new file, completion, reject; step 7 and completion compares; step 5's span hash; the staleness note |
| CLI `proposals.rs`, `inbox.rs`, `state_file.rs`, `lib.rs`, `main.rs` | review 45 keys, inbox 12, `open (staged)`; formats 1-3 / 4-5; `Outcome::Stage`; `--reason` optional |
| http `app.rs`, `answer.rs`, `main.rs` | POST, DELETE, `only_stage`; checks slug, page, ID, type, size, body; `answered` on `Outcome::Stage`; `--help` |
| mcp `mirror.rs`; `plugin/` | `staged`, `staged_at` in the output schema; `ask-owner`, 0.1.6 |
| UI `provisional.ts`, `client.ts`, `http.ts`, `queries.ts`, `ownStages.ts` | types; `stageDecision`, `unstageDecision`; 409/404 -> `ClientError`; 16 event types; re-reads; own-write tracking |
| UI `DecisionDialog.tsx`, `StagedChoice.tsx` (lazy), `ProposalCard.tsx`, `ProposalList.tsx`, `InboxView.tsx`, `OutsideStageAlert.tsx`, `Shell.tsx`, `decisions.ts`, mocks | dialog, staged section, Copy, Unstage, list mark, alert; the mock and `conflict` |

Deviations, accepted: the record question's marker mid-line (AC-07 reworded); the staleness note only when the preview applies or rebases; `span_hash` null also when steps 2-4 fail or the own commit is on the branch; more corrupt shapes refused; `Decision.staged_at` checked by CAS ops; `--note`, `--reason` over 4 096 B refused by `spec approve|reject` too; a step-7 refusal for a replaced stage logs its `apply_failed`; unstage takes an orphan, another repository 409; 404, 409 bodies the review document; http's unit test `a_proposal_id_is_pr_and_digits` moved to `stage.rs`; the stage outcome a `QueueCommand::Review` document; UI: the alert on every screen of the project, options numbered `[0]`, a 404 document rejects with its note. Iteration 2: canon cited without its path in CLI sources; the staged preface one line (LF, CR, TAB escaped); `--help` rewritten; `Sec-Fetch-Site` before the path ID; a completion refuses a stage that appeared or changed, shown or not; `StagedChoice`, `DecisionDialog` lazy (main chunk 492.41 kB); an unstage tracked only when a stage was cached.
