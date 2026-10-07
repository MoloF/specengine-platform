---
class: spec
status: shipped
scope: [specengine]
ref: task-package analysis, readiness check 2026-10-06, all accepted; G1-G4 for ui-tasks; 08 s2 Phase 2, slice 7
shipped: 2026-10-07
adrs: [ADR-0027]
---

# Task package

## Why

Agents got work as chat text: no approved scope, self-assembled context (W 2.3-2.8x a bundle's), spec edits met by chance, nothing to gate. A task: a per-project queue record only the owner moves to `ready`, on a terminal (ADR-0006, ADR-0012); agents get one versioned, stack-neutral package of the approved spec (ADR-0027); later spec edits raise `stale`, never block.

Working answers kept at shipping (08 s2): no new ADR; the compared place; one claim; criteria; no `approve_task`; tasks backed up; `queue.md` apart; no plan needed; nested spans; the linked update's `task_id`; the `snapshot_diff` cap.

## Description and interactions

As built: `docs/canon/tasks.md` "Commands" (CLI, exits), `docs/canon/task-package.md` "MCP" (five tools, `INSTRUCTIONS`, plugin 0.1.5). Kept here while `ui/src` and `ui-home.md` cite it (until `ui-live-tasks`):

- `spec task list [--status S]...`: by number, `<id> | <status> | <title or -> | <targets> | <updated_at>[ | stale]`; JSON `{tasks: [{id, status, title, targets, stale, updated_at}], notes}`; a `null` `stale` adds `<id>: <note>` to `notes`.
- `spec task show T | --next`: text the brief, JSON the package; none found -> exit 1, JSON exactly `{"id": "T-0099", "reason": "no task T-0099 in this repository"}` (`--next`: `id` `null`), MCP an error result.
- Owner only, on a terminal: `approve`, `changes --note`, `cancel`; agent: `plan`, `claim`, `report`, `complete` -> `{id, status, run, notes}`.

## Data

As built: the package `docs/canon/task-package.md` "Package", "Staleness", "Caps"; states, transitions, store, backup `docs/canon/tasks.md`. Kept while `ui/src` cites it:

```json
{"schema_version":1,"id":"T-0001","project":"<slug>","status":"ready","title":"...","goal":"...","profile":null,"stale":true,
"targets":[{"id":"MEC-STAMINA","path":"docs/spec/movement/stamina.md","kind":"...","title":"..."}],
"criteria":[{"ref":"stamina-tuning/AC-07","text":"..."},{"ref":null,"text":"..."}],"affected_nodes":["RULE-STAM-REGEN"],"plan":"...",
"assumptions":[{"proposal":"PR-0003","text":"..."}],
"open_proposals":[{"id":"PR-0003","kind":"question","status":"open","target_ids":["MEC-STAMINA"],"task_id":null,"summary":"..."}],
"owner_notes":[],"bindings":[],"spec_snapshot":{"at":"...","place":{"worktree":"...","root_rel":"","branch":"...","commit":"..."},
"nodes":[{"id":"MEC-STAMINA","path":"...","span_hash":"b3:..."}]},"snapshot_diff":[{"id":"MEC-STAMINA","path":"...","span_hash":"b3:...","diff":"@@ ...","cut":false}],
"claim":{"at":"...","role":"...","worktree":"...","branch":"..."},"runs":[{"run":1,"role":"...","started_at":"...","ended_at":null,"outcome":null,"summary":null,"changed_files":[]}],
"bundle":{"node_ids":["MEC-STAMINA"],"budget":10000,"bundle_hash":"b3:..."},"author":{...},"created_at":"...","updated_at":"...","notes":[]}
```

- Owner rows of the transition table: `approve` from `draft`, `review`, `changes_requested`, `ready`; `changes` from `review`; `cancel` from those and `in_progress`. A run's `outcome` `completed`, `partial`, `failed` or `abandoned`, never moving the state.
- Nullable as built: a `snapshot_diff` entry's `diff` (`cut` `true` past 262 144 B in all; `cut` `false` when git cannot make it, a note); `open_proposals[].summary`; `bundle.bundle_hash` (a note says why). Never `null`: a snapshot node's `id` (an id-less document's path).
- **Review document**: + `task_id` (`null` unbound) after `choice`, 43 keys; inbox entries unchanged (11).

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a`, `-b`, scratch `HOME`, fixed clock, git identity; owner commands via the library, consent yes (terminal checks on a pty). M: the mutation turning it red. Tests: CLI `tasks.rs`, `package.rs` unless named.

- [x] AC-01 -- lifecycle, both fixtures: every table row, cancel from each open state, one event each; any other pair -> exit 1, `dump()` unchanged (`ac01_the_transition_table_on_spec_a`, `_on_spec_b`; core `task_rules.rs` `the_transition_table_is_datas`; store `queue_tasks.rs` `every_change_is_one_event_and_refused_pairs_change_nothing`; M: claim accepts `review`).
- [x] AC-02 -- owner only: approve, changes, cancel off a terminal -> exit 2 before reading; not `y` -> exit 1, no event; no MCP call reaches their states (`ac02_the_owner_commands_run_only_on_a_terminal`, `ac02_a_declined_owner_command_changes_nothing`; MCP `ac14_the_task_tools_and_their_schemas`; M: the terminal check removed).
- [x] AC-03 -- package, genre: 07 s1.2 P2-1 to P2-8, P2-11; `owner_notes[]` `{at, note}`; `task show T-0099` the two keys; `get_task` with both, neither, `next: false` -> an error naming both; no root `oneOf` (`ac03_both_fixtures_give_the_same_versioned_package`, `ac03_an_unknown_task_is_two_keys_and_t_in_ids_stops_every_command`, `ac03_the_profile_changes_only_its_own_value`, `ac03_a_non_rust_project_gets_no_stack_word`; MCP `ac03_get_task_takes_exactly_one_of_its_arguments`, `p2_1_...`, `p2_2_the_package_schema_is_pinned_at_version_1`, `p2_3_the_task_sources_and_the_plugin_name_no_stack_word`; M: a key renamed without a bump; `cargo` in the brief).
- [x] AC-04 -- linked update: `propose discrepancy ... --task T-0001` with a patch: both rows `T-0001` (`ac04_a_discrepancys_linked_update_is_bound_to_the_same_task`; M: the update's NULL).
- [x] AC-05 -- staleness in the compared place, not outside; an edit before a claim flagged after it; `list` = `show`; a deleted file: `kind`, `title` `null`, a removal diff; a nested pair (a section and its document) reports both (`ac05_a_snapshot_node_edited_in_the_compared_place_is_stale`, `ac05_a_deleted_targets_file_reads_as_gone`, `stale_is_unknown_without_a_snapshot_or_its_place`; M: snapshot at claim).
- [x] AC-06 -- refresh: a bound update of `RULE-STAM-REGEN` (step 10; a completion) re-freezes it and `MEC-STAMINA`, two `task.refreshed`; `MEC-STAMINA` edited first stays stale; section-form create refreshes, file-form not; unbound none (`ac06_a_bound_update_applied_refreshes_the_enclosing_nodes`, `ac06_only_nodes_still_frozen_are_refreshed_and_unbound_applies_none`, `ac06_a_completion_by_its_own_commit_refreshes`, `ac06_a_section_form_create_refreshes_and_a_file_form_one_does_not`; store `applied_refreshing_refreshes_only_nodes_still_frozen`; M: every apply refreshes; the target alone).
- [x] AC-07 -- no blocking: open proposals on a `ready` task's node, one bound: approve and claim succeed, both listed, the working answer assumed (`ac07_open_proposals_never_block_a_task`; M: approve refuses).
- [x] AC-08 -- place: another repository's worktree, a plain directory, a detached `HEAD`, a control character -> exit 1, nothing recorded; a bound proposal from another worktree exit 1 naming the claimed one; report, complete likewise (iteration 2) (`ac08_a_claim_names_a_worktree_of_the_tasks_repository_on_a_branch`; store `a_bound_proposal_is_checked_where_it_is_stored`; M: any worktree).
- [x] AC-09 -- one door, isolation: `git status --porcelain` empty after every command and tool; P2-9 with two slugs (`ac09_two_projects_in_one_home_keep_their_tasks_apart`; MCP `ac09_two_projects_in_one_home_see_only_their_own_tasks`, `p2_1_an_agents_run_answers_as_its_twins_and_writes_nothing_under_the_roots`, `mcp_door.rs`; M: a file under the root; no slug filter, red in the store test).
- [x] AC-10 -- keys: review 43, inbox 11; `fixtures/daemon-keys.json` regenerated, http `daemon_keys.rs` green; `ui/src` `Proposal.task_id`, `daemonKeys.test.ts` 43, mocks serve `task_id` (`ac10_the_review_document_names_its_task_and_the_inbox_does_not`; UI `daemonKeys.test.ts`, `MockClient.test.ts`; M: `task_id` after `notes`).
- [x] AC-11 -- backup: export, import, export byte-identical; format-1 dumps of schemas 1-3 restore and re-export as 2; refused headers; only a task queued -> occupied (`ac11_tasks_and_runs_round_trip_through_a_format_2_backup`, `ac11_older_formats_restore_and_bad_headers_are_refused`; store `a_task_alone_makes_the_queue_occupied`; M: tasks left out).
- [x] AC-12 -- size, determinism: every field at its cap, 128 nodes of 8 192 B diffs -> 32 diffs (261 114 B), 96 `null` `cut`, one note; `content` with the tail; two `get_task` byte-identical (`ac12_every_field_at_its_cap_and_the_diffs_within_theirs`; MCP `ac12_a_full_package_is_deterministic_and_its_content_capped`; M: the read time in the package; no total cap).
- [x] AC-13 -- docs: gate clean, worst W 108 313 <= 108 468, the bound set at shipping (above the 108 136 at start by 177: the index root's two canon lines, +245); the ten amended canon pages net -5 B, each full one <= 0; `CLAUDE.md` 5 438, not grown; every heading `crates/` and `ui/src` cite exists (`doc_pointers`); `anonymity`, `mcp_genre`, `check_genre` green (M: a pilot name).
- [x] AC-14 -- texts: `INSTRUCTIONS` 1 878 B (2 035 with `probes`), the tasks line 138 B; `mcp_decision.rs`, `mcp_path.rs`, `mcp_create.rs` re-pinned (BLAKE3 `ddfb9e41...bc7d`); `plugin_skills.rs` thirteen tools, `plugin_files.rs` 0.1.5 (MCP `ac14_the_task_tools_and_their_schemas`; M: a 160 B tasks line, a const assert).

## Implementation

Canon: `docs/canon/tasks.md`, `task-package.md` (new); ten amended; READMEs; 05, 07, 08; `CLAUDE.md`. Three Rust and two UI iterations; review accepted.

| Module | What it does |
|---|---|
| model `task.rs` (new) | `TaskStatus` (10), `RunOutcome`, `TaskPackage` (25 keys) and parts |
| core `task.rs` (new), `project_toml.rs`, `check/config.rs` | `T-NNNN`, `[ids]` clash, `transition`, caps, `PACKAGE_BUDGET`; `profile`; `bundle_task` capped at `u32::MAX` |
| store `queue/tasks.rs` (new), `queue.rs`, `queue/state.rs` | step 4; `change_task` (CAS on `revision`), events; binding; `applied_refreshing`; four tables dumped and restored |
| CLI `task.rs`, `package.rs` (new); `main.rs`, `propose.rs`, `create.rs`, `intake.rs`, `proposals.rs`, `apply.rs`, `state.rs`, `state_file.rs` | the commands, places, criteria dedupe; staleness, diffs, budget, brief; terminal check, `--task`, review `task_id`, refresh, `STATE_FORMAT` 2 |
| MCP `tasks.rs` (new), `mirror.rs`, `intake.rs`, `server.rs`, `read.rs`; plugin | five tools, mirrors, `task_id` on three tools, tasks line, result bound; two skill lines, 0.1.5 |
| UI `provisional.ts`, `SpecChangesPanel.tsx`, `TaskProposalsPanel.tsx`, mocks | `Proposal.task_id`; nullable `diff`, `summary`; "Diff cut" lines; mock T-0200 |

Tests: CLI `tasks.rs` 18, `package.rs` 18, store `queue_tasks.rs` 13, core `task_rules.rs` 6, MCP `mcp_tasks.rs` 8, `proposal_genre.rs` +1; workspace 1 907 passed, 20 skipped, clippy clean; UI 63 files green. Mutations 19 + 10, all red.

Deviations accepted, now canon: (1) `get_task`'s exit 1 without `structuredContent`; (2) additive `*_with_task` (debt: into the request types); (3) a refresh skips untouched nodes; (4) a criterion is a reference only when exactly one; (5) targets fixed at `new`: a vanished one means cancel and recreate; (6) `bundle_hash`, `bundle_task` from the reading root; (7) verbs, prompts; (8) `open_proposals` of the task's repository only; (10) skill trims; (11) required MCP arrays. Rejected: (9) `bundle_task` past `u32::MAX`. Iteration 2: m1 that cap; m2 criteria dedupe, 8 192 B reference texts, `PACKAGE_BUDGET`, the "Size" bound; m3 report, complete only from the claimed worktree; m4 UI nullables; n1 cancel closes an open run; n2 directory comparison; n3 `revision`; n4 no refresh of closed tasks; n5 the skill sentence back; n6 "Queue" wording kept. Iteration 3: the repeat note's `j` counts in the plan as given.

**Open** (`ui-live-tasks`): `TaskBundle.bundle_hash` nullable in `provisional.ts`; the not-cut `null` diff as above; `summary` a question's or discrepancy's text; `ui/src` citations re-pointed to the canon. Tests: comments citing this spec's former "Rules", "Genre", "Backup", "Plugin" re-pointed. Limits: schema 4 refuses older binaries; P2-12 with a real model, `propose_change` with `task_id` over MCP untested. **Owner's check**: in a real terminal on a scratch copy, `spec task new`, `plan`, `approve` (the prompt names worktree, branch, commit), `claim`; plugin 0.1.5 restarted, an agent's `get_task` -> `claim_task` -> `report_run` -> `complete_task`.
