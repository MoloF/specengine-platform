---
class: spec
status: shipped
shipped: 2026-11-15
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: RM-0120; the owner's word of 2026-11-15 (08 s2 Phase 7, "Owner's word, 2026-11-15"); release auto-mode, plugin 0.47.0
adrs: [ADR-0081, ADR-0082]
---

# auto-rule: the `[auto]` table, its hash, the predicate and a dry run (RM-0120)

## Why

The owner works alone and spends his time on keys that judge nothing: a task approval per task, a `[y/N]` or a click with a second factor per proposal, a commit and a merge per worktree. He asked for an auto mode set in `specengine.toml`, so he can leave work overnight and read the result in the morning, and for manual mode back when a team arrives. This slice is the ground the other four stand on: the rule, how it is read and hashed, the one pure predicate that decides every delegated act, the events and the apply seam they share, and `spec auto show`, a dry run that writes nothing. It delegates nothing by itself.

## Description and interactions

Shipped as specified. The rule, its reading, hash, switches, reasons and their details, events and trailers: `docs/canon/auto-mode.md`; `spec auto show`, `stop`, `resume`: `docs/canon/spec-auto.md`; the apply seam: `docs/canon/hub-auto.md` "Rules" and `spec-auto-land.md` step 7.

## Data

### `spec auto show`

Text and JSON: `spec-auto.md` "Show"; `stop` and `resume`: "Stop and resume".

### Fixture

`fixtures/auto-mini/`: `specengine.toml` (`[project] slug = "auto-mini"`, `[paths] roots`, `[ids]` `R` requirement, `SC` scenario, `DEC` decision; `[zones]` `code`, `tests`, `docs`, `narrative`; `[gate] mode = "observe"`; no `[auto]`), `docs/spec/core.md` (R-01, R-02), `docs/spec/rooms.md`, `docs/narrative/intro.md` (SC-001), `docs/records/DEC/DEC-0001.md`, `notes/ideas.md`, `src/lib.rs`, `tests/a.rs`; `rules/*.toml`, tables a test appends and commits (`on`, `on-reordered`, `local`, `hub`, `off`, one `bad-<problem>` per problem). Tests copy it and `git init` there.

## Rules and edge cases

- WHEN a project has no `[auto]` table THEN no command but `spec auto *` changes any output, route body, event or row, and `show` reports `off` (RL1); a hub's one exception, an extra turn hold per stored update or create: `hub-auto.md`.
- WHEN the table does not read THEN auto is off with one note; `[auto]` is never `ProjectConfig`'s error.
- The predicate reads nothing: the caller gathers every fact in one snapshot, so one state gives one verdict.
- 0.46.1 and older refuse a file holding `[auto]`: the release order (`hub-ops.md` "Auto") is the mitigation.

## Acceptance criteria

Core `tests/auto_rule.rs`, `tests/auto_predicate.rs`; store `tests/auto_events.rs`; CLI `tests/auto_show.rs`, `auto_manual.rs`, `auto_apply.rs`; mutations as the release's (28 applied, 28 caught).

- [x] RL1 — **Manual mode, byte for byte** (`auto_manual.rs` `rl1_manual_mode_is_byte_for_byte_with_no_table_or_an_off_rule`, `auto_show.rs` `rl1_no_table_shows_off`). (a) The test diff since `v0.46.1` adds tests and extends the closed lists by design (`RawFile`'s keys, `tests/door.rs`, `GET_ROUTES`, `fixtures/daemon-keys.json`, `QUEUE_EVENT_TYPES`, the plugin's `PINS` and file set, the version pins, `store_reads.rs` `AFTER_THE_GOLDEN`, the shard set split by `docs/index-decisions-0.md`), plus the UI's nav and palette assertions, edited for Activity in the nav ("Implementation"). (b) On `auto-mini` with no table, then with `rules/off.toml`, the manual night gives equal stdout, stderr and exits; no `auto.*` event, `task.approved` `{"id":"T-0001"}`, `proposal.applied` `{"id","commit"}`, no `Delegated-by:`; `show --json` `off`, `no [auto] table in specengine.toml@HEAD`. Mutations: a missing table read as `both`; `by` always written.
- [x] RL2 — **Every reason, one row** (`auto_predicate.rs` `rl2_every_reason_one_row`, `the_reasons_are_the_canons`; `auto_show.rs` `rl2_show_lists_each_rows_reason`, `rl2_show_on_a_hub_author_and_cap`). Mutation: any one check removed.
- [x] RL3 — **`HEAD`, not the tree** (`auto_show.rs` `rl3_head_not_the_tree`): a working-tree edit changes no verdict and adds the note; a linked worktree on another branch reads the main worktree's `HEAD`. Mutations: the working tree; the current worktree's `HEAD`.
- [x] RL4 — **A team turns it off** (`auto_predicate.rs`, `auto_show.rs` `rl4_a_team_turns_it_off`). Mutation: `> 2`.
- [x] RL5 — **Malformed, never failing** (`auto_rule.rs` `rl5_each_bad_rule_is_off_with_one_note_at_its_line`, `auto_show.rs` `rl5_a_malformed_table_never_fails`). Mutation: the reader's error propagated.
- [x] RL6 — **Exclusion beats cover** (`auto_predicate.rs` `rl6_exclusion_beats_cover`). Mutation: the cover checked first.
- [x] RL7 — **The hash** (`auto_rule.rs` `rl7_the_hash_is_the_canonical_forms_and_ignores_order_and_comments`): `on` and `on-reordered` equal a pinned canonical string; a glob or a `scenario` prefix changes it. Mutations: hashing the text; the globs left out.
- [x] RL8 — **Frozen and now** (`auto_predicate.rs` `rl8_frozen_and_now`). Mutations: the frozen rule alone; the rule now alone.
- [x] RL9 — **Stop and resume** (`auto_show.rs` `rl9_stop_and_resume`, `stop_needs_no_project`). Mutation: `resume` without the terminal check.
- [x] RL10 — **The config accepts the table** (`auto_rule.rs` `rl10_the_project_config_accepts_the_table`, `auto_show.rs` `rl10_the_config_accepts_the_table`). Mutation: `auto` left out of `RawFile`.
- [x] RL11 — **The seam** (`auto_apply.rs` `rl11_*`, four): one commit, `Delegated-by:` last, `proposal.applied` with `by`, `rule`, `pack`; a hub's identity and no pack; a row no longer applying refused; a row changed since judged `Moved`, nothing written. Mutations: the trailer before `Base-commit:`; the consent yes without the compare.
- [x] RL12 — **Events** (`auto_events.rs` `rl12_each_auto_event_round_trips_in_the_canons_key_order`, `a_task_approved_by_rule_logs_its_keys_and_a_plain_one_does_not`, `by_rule_events_and_the_days_count`): keys in the canon's order, `null`s written, the dump keeps them, `user_version` 14, `"format":7`. Mutation: a schema step.

## Out of scope

Starting, approving, landing, the runner, the hub's apply, the feed (S2-S5). A rule key per proposal kind. Departments as rights (ADR-0073). Reading `[auto]` from the working tree.

## Implementation

| Module | What it does |
|---|---|
| core `auto_toml.rs` (new) | `AutoRule::from_toml` -> `AutoRead`, `AutoNote`, `canonical_form`, `rule_hash` (BLAKE3), `branch_problem`, `id_prefix` |
| core `auto.rs` (new) | `Reason` (29), `Verdict`, `Surface`, `Switches`, `ProposalFacts`, `TaskFacts`, `PackFacts`, `effective`, `judge_switches`, `judge_proposal`, `judge_approve`, `judge_land_paths`, `is_config_target`, `is_outside`, `DELEGATED_BY_TRAILER` |
| core `project_toml.rs`, `gate.rs`, `Cargo.toml` | `RawFile.auto` name-checked; `Zone::from_globs` for a frozen rule; `blake3` a direct dependency (1.8.7) |
| store `queue/auto.rs` (new), `queue.rs`, `queue/state.rs` | the four `auto.*` events, `record_auto`, `auto_events`, `approve_task_by_rule`, `rule_applies_on` (counted in SQL), `applied_payload`; `ByRule` keys after a payload's own, stored sorted; `set_by_rule` |
| CLI `auto_read.rs` (new) | `load_auto(env, project, git, surface)` -> `EffectiveRule` (`absent`, `notes`), `proposal_facts`, `task_facts` (nodes resolved in the reading root), `stop_file` |
| CLI `auto_apply.rs` (new), `apply.rs`, `create.rs` | `delegated_approve` (`DelegatedApproveRequest.expected: Option<Seen>`), `Delegation`, the `Delegated` guard resetting `set_by_rule` on drop, `delegated_message` |
| CLI `auto.rs` (new), `main.rs`, `lib.rs` | `auto_show`, `auto_stop`, `auto_resume`; the `auto` subcommand |
| fixture, tests | `fixtures/auto-mini`; the test files above, `tests/auto_common/` |

**Departures**, accepted: `load_auto` takes the project and git rather than globals and a root; there is no `LandFacts` (S3's `judge_land`, `judge_pack_act`, `judge_task_paths`); the switch's why for a rule that does not read has no `; auto is off` tail (the note keeps it); a `proposal-kind` detail takes its kind's article; `show` reports `logins` 0 with no `[team]` and `team: <why>` when `[team]` does not read; exit-1 refusals print only `spec: <reason>`, holds their own line; `resume --json` is `{"path","stopped":false,"notes":[]}`; a delegated completion carries the by-rule events but no trailer (it makes no commit); `spec task show --json`'s `spec_snapshot` shows no `by`. Orchestrator's decision: `blake3` in core directly, at the workspace's pin.
