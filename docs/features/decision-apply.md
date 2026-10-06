---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-mcp]
ref: decision-apply analysis 2026-10-05, refreshed 2026-10-06; 08 Phase 2, slice 5
shipped: 2026-10-06
---

# Decision apply: records from the queue

## Why

`spec reject PR --reason <answer>` was the only way to settle a question or discrepancy: the answer stayed in SQLite, read `rejected`, had no price or `canon:`, never reached a branch or clone. Now one terminal action makes the owner's choice an accepted decision record in the project's own shape: one new file, one `spec: apply PR-...` commit where the item was raised (ADR-0004, ADR-0005, ADR-0032); queue and dedup then return it (08 AC-5). No new ADR: the shape lives in the project's config and template (ADR-0008), the write door and consent are the update's.

Working answers (the owner's, 2026-10-05, session rule): Q1 these two kinds only, the other four `proposal-kinds`; Q2 the record committed alone, a linked update by its own approve; Q3 `--option N` required, a question's working answer or `--answer T`, never the recommendation by default; Q4 the shape only from `specengine.toml`, none -> exit 2, no built-in; Q5 findings shown, never refusing (ADR-0012), structural defects refuse; Q6 no canon diff of the governed section, a note. Refreshed 2026-10-06: D1 canon only from an ID, D2 the plugin in this change.

How it works now: `docs/canon/decision-record.md`.

## Data

Moved to the canon at shipping: `docs/canon/decision-record.md` "Config", "Template", "Queue and documents"; the queue backup `docs/canon/queue-backup.md` "Format"; "Plugin": the root `README.md` "Claude Code plugin".

## Rules and edge cases

Moved to `docs/canon/decision-record.md` "Flags", "Steps", "ID", "Completion", "Killed run", "Reject", "Known limits".

## Acceptance criteria

Setup: temp repos of `fixtures/spec-a` (`[decision_records]` `DEC`, `docs/records/DEC`), `fixtures/spec-b` (`ADR`, `docs/records/ADR`), each `templates/decision.md` committed outside the roots; scratch `HOME`; clock `2026-10-05T12:00:00Z`; a git identity; items raised with `distinct_from` naming their hits; library approve, consent yes. Refused: nothing written, the row as before. M: the mutation turning it red. Tests: CLI `decision_apply.rs` unless named.

- [x] AC-01 -- spec-a discrepancy on `RULE-STAM-REGEN`, 3 options, a tracked file modified, `--option 1`: one commit, one parent, name-status exactly `A docs/records/DEC/DEC-0024.md`, body `DEC-0024: <label 1>`, four trailers; `status: accepted`, `date: 2026-10-05`, `canon: RULE-STAM-REGEN`, option 1's label, effect, price; the other file still modified; `applied` (`ac01_an_option_becomes_one_record_in_one_commit`, `ac01_the_binary_asks_on_the_terminal_and_names_the_record`; M: staging the whole tree).
- [x] AC-02 -- question on `Q-031`, no flag: the working answer, `cost` empty; `--answer T`: `T`, cost `price_of_other` (`ac02_a_question_takes_its_working_answer_or_the_given_one`; M: cost always `price_of_other`).
- [x] AC-03 -- spec-b, Cyrillic labels, `--option 0` -> `docs/records/ADR/ADR-0003.md`, labels verbatim; new sources hold no `[ids]` prefix or kind literal, `Cost`, `docs/`, `records/` (`ac03_spec_b_writes_its_own_record_with_its_labels_verbatim`, `proposal_genre.rs`, core `genre.rs`; M: `"DEC"`).
- [x] AC-04 -- no `[decision_records]`: exit 2 naming it; worktree, git index, `HEAD` unchanged; `open`, one `apply_failed` (step 3); `reject --reason` works (`ac04_no_table_refuses_at_step_3_and_reject_still_settles`; M: a built-in template).
- [x] AC-05 -- `DEC-0007`, `DEC-0023` -> `DEC-0024`; a second worktree without it -> `DEC-0025`; a declined prompt issues nothing (`ac05_the_id_is_the_corpus_or_the_queue_whichever_is_higher`, store `queue_records.rs`; M: issuing from the worktree only).
- [x] AC-06 -- label `x\nstatus: rejected` -> `status: accepted`, one `title`; effect `## X {#RULE-STAM-REGEN}` -> exit 1 naming `options[1].effect`, no prompt; label `{{id}}` literal; U+202E in `summary` -> exit 1 (`ac06_free_text_is_one_pass_and_never_structure`, core `decision_records.rs`; M: raw substitution; a re-scan).
- [x] AC-07 -- stdin a pipe -> exit 2 before reading; consent `n` -> exit 1, path absent, `git ls-files --stage`, `git status --porcelain` as before, `record_id` NULL (`ac07_no_consent_writes_and_issues_nothing`; M: the ID before the prompt).
- [x] AC-08 -- `dir` a missing `docs/records/DEC/new`, a pre-commit hook exiting 1 -> exit 1; file and `new/` gone, no index entry; `open`, `record_id` kept, one `apply_failed` (step 9); no hook -> the same ID, applied (`ac08_a_failed_commit_removes_the_record_its_entry_and_its_directory`; M: the intent-to-add entry left).
- [x] AC-09 -- the consent callback writes X at the path, then yes -> exit 1, X intact, no commit or temporary; the store's create-new on an existing path fails, bytes intact (`ac09_a_file_appearing_meanwhile_is_never_replaced`, store `queue_records.rs`; M: rename over the path).
- [x] AC-10 -- `approved` at step 7, its commit made by hand from `record_text`: approve -> `applied`, no new commit or prompt; another `date:` -> `does not carry the record` (`ac10_an_approved_record_is_completed_by_its_own_commit_only`; M: re-rendering).
- [x] AC-11 -- after AC-01, the same summary on `RULE-STAM-REGEN`: `created: false`, hits corpus `DEC-0024` and queue `PR-0001` (`applied`, `path` the record, `answer` the title, `record` `DEC-0024`); `counts()` unchanged; `get_proposal` = `review --brief --json` (`ac11_a_decided_item_answers_the_same_question_again`; M: the answer from `decision_note`).
- [x] AC-12 -- every MCP tool on a decided item: `git status --porcelain` empty, no tool added; `export state` `queue_schema` 3, 40 columns; schema 1, 2 dumps restore, re-export byte-identical as 3; a v2 DB: `user_version` 3; gate clean, worst W <= 109 484 (`ac12_*` here and in MCP `mcp_decision.rs`; the gate below; M: a column missing).
- [x] AC-13 -- same item, choice, note, clock, identity in two fresh repos -> byte-identical records and messages (`ac13_the_same_decision_renders_the_same_bytes`; M: the wall clock).
- [x] AC-14 -- exit 2: `--option` on a question or update, `--answer` on a discrepancy, both, a discrepancy without `--option`, a question on a path without `--canon`, `--canon "a b"`; `--option 7` of 3 -> exit 1 naming `0-2`; no event (`ac14_the_flags_are_checked_before_anything`; M: an out-of-range option accepted).
- [x] AC-15 -- that question, `--canon docs/spec/movement/stamina.md#regeneration` -> that `canon:`, `targets` `[]`, no `canon-*` introduced; the bare path -> `canon-form` shown, written; `Q-031`, `--canon MEC-STAMINA#RULE-STAM-REGEN` -> it (`ac15_canon_names_the_section_the_record_governs`; M: a path by default).
- [x] AC-16 -- `ask-owner` as the root README's row; `plugin_skills.rs`, `plugin_files.rs` green at `0.1.3`; the three descriptions name `spec approve`, `INSTRUCTIONS` unchanged (MCP `mcp_decision.rs` `ac16_*`; M: the skill unedited; no bump).

## Implementation

Canon: `docs/canon/decision-record.md` (new); `proposal-queue.md`, `proposal-apply.md`, `agent-intake.md`, `queue-backup.md`; the core, store, CLI, MCP READMEs; root `README.md`; 07; 08 Phase 2, AC-5; `CLAUDE.md`. Two iterations; review 1: 1 major, 3 minor, 2 nits, all fixed in iteration 2; check, fmt, clippy (`probes` too) clean.

| Module | What it does |
|---|---|
| core `record.rs` (new), `project_toml.rs` | the table on load; slots, one-pass render, title, ID numbering, `Choice`, `is_escaped`, structure |
| store `queue.rs`, `queue/state.rs`, `worktree.rs` | schema 3, `next_record`, `approve_record_from`, corrupt rows; `create_file`, intent-to-add, `name_status` |
| CLI `decide.rs` (new), `apply.rs`, `preflight.rs` | first checks, steps 2-10, consent, cleanup, notes; `ApproveFlags`, dispatch, reject rule; `place_step`, record completion |
| CLI `proposals.rs`, `inbox.rs`, `intake.rs`, `state_file.rs`, `main.rs` | review keys, applied line, inbox `record_id`, match `record`, dumps 1-3, the flags |
| MCP `mirror.rs`, `intake.rs`; plugin | keys, `Choice` schema, three descriptions; `ask-owner`, 0.1.3 |
| fixtures `spec-{a,b}` | `[decision_records]`, `templates/decision.md` |

Tests: CLI `decision_apply.rs` (26), store `queue_records.rs` (6), core `decision_records.rs` (9), MCP `mcp_decision.rs` (2); expectations moved in 14 files. Mutations, all red: iteration 1 each AC's M (20); iteration 2 m01-m11 (the reject rule, the template checks, the step-6 probe, the killed-run hint, the title, a held record's flags, `A.` as intent-to-add).

Accepted deviations, now canon: (1) a queue hit carries `record`, `path`, `answer` only when applied; (2) the template writes `title: {{title}}`, the engine quotes; (3) a killed run's leftover refuses `exists`, the two ways out named; (4) an approved item holding its record exits 2 on any decision flag; (5) a non-completing trailer commit refuses a new apply at step 4; (6) template file errors at line 1; (7) reject reads history only for an item that may have a commit. Iteration 2: a differing `--option`, `--answer` or `--canon` on a held record exits 2 without a prompt; the title is the answer's first non-blank line; template defects are their own step-3 error, `--canon` blamed only when the template reads back a probe.

Known limits (the canon's): the killed-run hint under a sub-root, tried by hand only; the no-title exit 2, unreachable, untested. Eval `import_cli`, `layout_cli` stay red until the fixture changes are committed. Owner's check, open: a real terminal, `spec approve PR --option N` on a scratch copy: the record in the prompt, `git show --stat` one added file, `spec inbox --all` ` [<ID>]`; plugin 0.1.3 updated and restarted, an `ask-owner` hit with a `record` read by `get_node`.
