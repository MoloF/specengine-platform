---
class: spec
status: shipped
shipped: 2026-11-15
scope: [crates/specengine-cli, crates/specengine-store]
ref: RM-0122; the owner's word of 2026-11-15 ("set a task or a pack of tasks ... leave development overnight"); release auto-mode, plugin 0.47.0
adrs: [ADR-0081]
---

# auto-land: a pack started on a terminal, approved and landed by the rule (RM-0122)

## Why

Locally the owner approves each task, then commits and merges each worktree, since the plugin's session never commits. ADR-0081 replaces those keys with one: he starts a pack on a terminal, and SpecEngine's code approves each pack task whose plan the rule covers and lands each finished one on an integration branch he merges in the morning. This slice is every `spec` command the runner (S4) calls; the runner itself starts the agents.

## Description and interactions

Shipped: `spec auto start`, `approve`, `next` and the line binding of `spec task new` (`docs/canon/spec-auto.md`), `spec auto land` and `hold` (`docs/canon/spec-auto-land.md`), on S1's rule, predicate, events and seam (`docs/canon/auto-mode.md`).

### Lanes

A task's lane: `spec-auto.md` "Next", **Lanes** (`auto_next::Lanes`); advisory, never a right.

## Data

Events and trailers: `auto-mode.md`. Scratch paths under the data directory: the lock `<slug>.auto-land` (and `.takeover`), `auto/<slug>/land-<pid>/`, its `-home` and `.msg`; nothing under the project root but the claim's commits and the branch's ref.

## Rules and edge cases

A land never pushes, checks out a branch or writes `spec.lock`; a removed or narrowed rule acts on the next act; a checked-out integration branch holds every land; a cancelled pack task is finished; a held item is never offered again, the owner's `spec auto land` re-judging it; two lands never run at once; a red integration tip holds every land. `spec auto approve` takes `review` alone and `start` has no `--max-tasks` (against the analyst's draft).

## Acceptance criteria

CLI tests over scratch `fixtures/auto-mini` with `rules/on.toml` committed: `tests/auto_start.rs` (pty), `auto_approve.rs`, `auto_land.rs` (a bare `origin`), `auto_next.rs`, `auto_line.rs`.

- [x] LD1 — **Start asks once, on a terminal** (`ld1_start_asks_once_on_a_terminal`, `ld1_start_refuses_before_asking`, `ld1_start_lists_the_rules_notes`). Mutation: accepting a pipe.
- [x] LD2 — **Approve moves only a pack's covered task** (`ld2_approve_moves_only_a_packs_covered_task`). With `on.toml` SC-001 sits in `narrative`, so the canon's order holds it `excluded-zone` before `excluded-kind`: the test takes either. Mutation: membership skipped.
- [x] LD3 — **Land: commit, apply, check, merge** (`ld3_land_commits_applies_checks_and_merges`, `ld3_a_red_check_or_a_config_change_holds`, `ld3_expired_debt_holds_check`, `ld3_new_debt_alone_holds_check`, `ld3_nothing_new_and_an_already_merged_tip`, `ld3_a_path_outside_the_root_is_uncovered`, `ld3_a_changed_document_declaring_an_excluded_kind_holds`, `ld3_removing_an_excluded_kind_holds_too`, `ld3_left_lists_every_open_bound_row`, `ld3_a_refused_apply_leaves_a_note_and_lands`). Mutations: a held proposal applied; a red check merged; a config change committed.
- [x] LD4 — **Never main, never a push** (in `ld3_land_commits_applies_checks_and_merges`): `main` and the bare `origin`'s refs unchanged, no push. Mutation: a `git push` after the merge.
- [x] LD5 — **The lock untouched** (the same test): `spec.lock`'s bytes equal before and after. Mutation: a lock accept inside land.
- [x] LD6 — **No rule, no auto** (`ld6_start_with_no_rule_is_off`, `auto_approve.rs` `ld6_no_rule_no_auto`).
- [x] LD7 — **One land at a time; a conflict aborts clean** (`ld7_one_land_at_a_time_and_a_conflict_aborts_clean`, `ld7_a_land_killed_mid_way_leaves_nothing_stuck`, `ld7_a_dead_lock_and_a_stale_scratch_worktree_are_taken`, `ld7_two_lands_racing_for_a_stale_lock`). Mutation: no lock, then no `update-ref` compare.
- [x] LD8 — **Off mid-run** (`ld8_off_mid_run`). Mutation: the frozen rule alone.
- [x] LD9 — **A line binds once** (`ld9_a_line_binds_once`). Mutation: binding a bound line.
- [x] LD10 — **Next is one function of the state** (`ld10_next_walks_a_task_through_its_acts`, `ld10_order_overlap_lanes_and_slots`, `ld10_a_busy_line_waits_and_a_pack_of_another_branch_is_skipped`, `ld10_until_holds_an_approve_and_lets_a_land_run`). Mutation: the overlap ignored.
- [x] LD11 — **Holds finish an item** (`auto_next.rs` `ld11_holds_finish_an_item`, `auto_land.rs` `ld11_ld12_a_held_task_lands_and_a_checked_out_branch_never_moves`; strengthened before its mutation ran). Mutation: `next` returning held items.
- [x] LD12 — **A checked-out branch is never moved** (the same test). Mutation: the check removed.

## Out of scope

Pushing; merging `auto` into the main branch (the owner's, each morning); retrying a held item automatically; delegated lock writes, tests or drift; cards in a pack; un-holding by a command.

## Implementation

| Module | What it does |
|---|---|
| CLI `auto_pack.rs` (new) | `Pack` (`live`, `ended`, `facts`), `Item` (`finished`), `auto_state`, `auto_start`, `auto_hold`, `--lines` reading |
| CLI `auto_approve.rs` (new) | `auto_approve` |
| CLI `auto_land.rs` (new) | `auto_land`, the eleven steps, `LandLock` (create-new, `take_over`), the scratch sweep, `earlier_merge` (recovery), held signals |
| CLI `auto_next.rs` (new) | `auto_next`, `Lanes`, `--busy` parsing |
| CLI `task.rs`, `group.rs`, `main.rs`, `lib.rs` | `SPECENGINE_AUTO_LINE` binding; `Forwarding` holding `SIGINT`, `SIGTERM`, `SIGHUP`; the five subcommands |
| core `auto.rs` | `judge_land`, `judge_pack_act`, `judge_task_paths`, `judge_pack_switches`, `is_outside` |
| store `queue/auto.rs`, `queue/tasks.rs`, `worktree.rs` | `record_auto_checked` (one live pack per task, in the transaction), `create_task_bound`; `WorktreeGit::run` |

**Decisions** (the orchestrator's): `until` leaves a pack live for its lands (`Pack::ended`; `show` marks plans and implements `hold | until`, `approve` logs it), so a task never finishing there joins no new pack until cancelled or held; step 5 judges a changed spec document's IDs at the claim and at the merge base, file-level and strict; step 9 checks against the old tip (`CheckedTree::Changed` after `reset --soft`), any verdict but clean holding, new debt and expired debt included; a path outside the root `../…`, `uncovered`; nothing new to merge is a recovery, `nothing to land`, or a merge making no commit, all `place`; signals held from step 3, an interrupted land exit 2 recording nothing; a dead lock taken over by `flock` on `<lock>.takeover`; `left` every open row bound to the task, applies limited to the claim's worktree, refused or moved rows a note each; `next` skips a pack whose own switches fail and answers `wait` for any `--busy` item; a refused start deletes the branch it made by compare-and-set.

**Departures**: a `T` that is no task ID exits 1; an empty pack refused by the `max_tasks` message; `--lines -` exits 2; a failed merge naming no path holds `place`. Untested, needing a race: a merge making no commit; the transaction's re-check of one live pack per task.
