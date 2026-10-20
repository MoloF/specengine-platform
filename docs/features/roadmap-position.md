---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 6 slice 2a of `roadmap-items.md` (its Q-5, Q-6, N7); the requirement-analyst's refresh of 2026-10-20 against 1a (725f338) and 1b (b88b680), split and decided under the owner's pre-approval (Q-A..Q-H), review open; 05 s3.3; 08 s2; developer iteration 1, the review accepted it (nits only)
shipped: 2026-10-20
adrs: [ADR-0052]
---

# roadmap-position (Phase 6, 2a): the tasks on the plan in `spec roadmap`

Split on 2026-10-20 (Q-A): this is 2a, the join in `spec roadmap` and the tolerant task read; 2b `docs/features/roadmap-surfaces.md` serves it (`next`, the package, MCP, HTTP, plugin 0.11.0, UI types). Plan items: RM-0003 (2a, shipped), RM-0030 (2b).

## Why

1a and 1b place work from git alone: the owner sees the path but not what is being worked on, and the plan cannot say which open item has an approved task waiting or a claimed one running. 2a joins the plan to this repository's task rows, read-only and per read (ADR-0052: position is computed, never stored, `docs/canon/architecture.md#storage`), on the one surface that already prints the plan.

Delivered: `spec roadmap` ends an item's line with its open tasks and prints the open tasks executing no open item after the pool's items; JSON `tasks` per item and `pool_tasks`; task lines count in the cut; the join's notes go to stderr only; no schema step, no table, no new exit. The view and core's lines stay git-only. No ADR: it applies ADR-0052, ADR-0012, ADR-0039 and ADR-0008. How it works now: `docs/canon/spec-roadmap.md` "Position", "Notes", "Command", "Cut"; the store read: `docs/canon/tasks.md` "API".

## Decided

2026-10-20 under the owner's pre-approval, review open.

| | Answer |
|---|---|
| Q-A | split at the schema seam: 2a core, store, CLI; 2b (RM-0030, after RM-0003) the rest; RM-0004 (3a) after RM-0030 |
| Q-B | an item shows every open task (not `done`, `accepted`, `cancelled`) with its state; in progress when one is `in_progress`, never merged with the item's own `status:` |
| Q-C | a task executes the first open item among its targets in the listing order over the whole plan; only done targets or none: a pool task; `--release R` filters what prints, never re-places |
| Q-G | a store read with no schema step (`list_existing_tasks` over `open_existing`, the gate's precedent); 2b's `get_roadmap` reuses it, so mcp-read AC-18 stays green |

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [x] RD-1 -- the join, core `tests/roadmap_position.rs` (new, pure, over `roadmap-a` and `roadmap-b`'s parsed plans). M1: the first target as written used -> `best_placed_target_is_the_item`; M2: the result following row order -> `position_ignores_task_row_order`. Unmutated: `done_item_targets_join_the_pool`, `non_item_targets_join_the_pool`, `tasks_by_number` (`T-9999` before `T-10000`; one row each of `done`, `accepted`, `cancelled` placed nowhere).
- [x] RD-2 -- the CLI, `tests/roadmap_position.rs` (new; `task_common`'s `Pair` on git copies of `roadmap-a`, slug `tide-runner`; the binary). M3: a claimed task ignored -> `claimed_task_marks_its_item_in_progress`. Unmutated: `every_open_state_is_shown_by_number`, `pool_tasks_print_after_the_pool_items`, `database_loss_keeps_the_plan`, `other_repository_tasks_are_noted` (one note; the other repository shows them without it), `closed_tasks_are_not_listed`, `no_git_worktree_reads_no_tasks` (1a's bytes, no note, exit 0), `roadmap_creates_no_queue_table` (the tables `spec tree` leaves, `user_version` 0), `unreadable_queue_is_one_note` (newer schema, a corrupt row, `T` in `[ids]`: exit 0, one note, the plan's lines).
- [x] RD-3 -- the cut, same file. M4: task lines not counted -> `roadmap_cut_counts_task_lines` (505 items, 3 pool tasks, a cell on the first: `n` 508 with no flag, 500 with `--release`, the cap held, `omitted` = `k`).
- [x] RD-4 -- the view stays git-only: `view_ignores_the_queue`, `export_writes_the_view_without_a_data_directory`, `view_lines_are_the_roadmap_lines` (CLI `tests/roadmap_view.rs`) green unchanged; core `tests/roadmap_genre.rs` green over the three sources it scans.

test-engineer:

- [x] TE-1 -- `fixtures/roadmap-{a,b}/expected.json` gain `tasks: []` after each item's `cycle` and `pool_tasks: []` after `groups`; `expected.txt` unchanged; `roadmap_a_matches_expected`, `roadmap_b_matches_expected` and the rest of CLI `tests/roadmap.rs` green; core `tests/roadmap.rs` `fixture_plans_match_expected_json` green, its `plan_json` writing the two keys empty (the plan reads no queue).

spec-writer (bytes by `wc -c` after / cap):

- [x] SW-0 -- the refresh and split; `roadmap-surfaces.md`; RM-0003 retitled, RM-0030 added, RM-0004 re-chained; `roadmap-screen.md`, `verify-node.md` re-pointed.
- [x] SW-1 -- `docs/canon/spec-roadmap.md` 11 276 / 12 288 (+1 140; 1 012 B left for 2b): "Position" (new), "Notes" (the join's, stderr only), "Command" (`RoadmapRequest.git`, the queue read, the cell and task lines, two JSON keys), "Cut" (`n` with the pool's task lines); trimmed "Items" and "Config" wording. The five code citations of "Position" resolve (`doc_pointers`).
- [x] SW-2 -- RM-0003 shipped and the view regenerated; 08 s2 Phase 6 names 2a; `CLAUDE.md` State 6 291 / 16 384; `docs/canon/tasks.md` "API" names `list_existing_tasks` (11 881 / 12 288). No change: core README (its `plan` pointer covers `plan_tasks`), store README ("ops named in those canons"), CLI README, `architecture.md`.

## Risks

- **Two meanings of "in progress"**: the item's `status: in-progress` and a task's `in_progress`; both print verbatim, canon "Position" says which is which.
- **Version skew**: install `spec`, `specengine-mcp`, `specengine-http` from one commit (1b's rule).

## Out of scope

`next`, the package, MCP, HTTP, plugin, UI (2b); the screen (3a); moves (3b); a queue column or schema step (N7); a gate; dates.

## Implementation

| Module | What it does |
|---|---|
| core `src/roadmap.rs` | `PlanTask {id, status, title, targets}` (`is_open`), `PlanTasks {items, pool}` (`of`, `in_progress`, `is_empty`), `plan_tasks` (listing index per item, tasks sorted by `task_key` then the row), module doc; `plan`, `summary`, `header`, `line`, `cells`, `entry`, the view untouched |
| core `src/lib.rs` | re-exports `PlanTask`, `PlanTasks`, `plan_tasks`; module line |
| store `src/queue/tasks.rs` | `SqliteQueue::list_existing_tasks`: one deferred read-only transaction, newer `user_version` `SchemaTooNew`, no `tasks` table empty; `list_tasks` shares the row reader `listed_tasks`, behaviour unchanged |
| CLI `src/roadmap.rs` | `RoadmapRequest.git` (`Default` dropped); `queue_tasks` (the read order, notes), `repository` (git common dir), `task_cell`, `pool_task_line`, `cut` counting both, `RoadmapOutcome.{tasks, shown_pool_tasks, task_notes}`, JSON `tasks`, `pool_tasks`; module doc |
| CLI `src/main.rs` | `RoadmapRequest { git: process_git(env) }`; the `roadmap` help names the open tasks (not in the spec's file list, pinned by nothing) |
| CLI `src/lib.rs` | the module line ("never the queue" gone) |
| tests, fixtures | core `tests/roadmap_position.rs` (new), `tests/roadmap.rs` `plan_json`; CLI `tests/roadmap_position.rs` (new); `fixtures/roadmap-{a,b}/expected.json` |

No new dependency, no schema step; nothing in MCP, HTTP, `plugin/`, `ui/`, `daemon-keys.json`, `PACKAGE_KEYS` or `INSTRUCTIONS`. Deviations: none against this spec. Known, kept: the shared reader decodes every row of the project, so a corrupt `runs` row of a closed task or of another repository's task adds its `; not shown` note, as `task list` does (a narrow SELECT is 2b's option). A micro pass after the review, done, no output change: the doc comments of `RoadmapOutcome.messages` and `.truncated` updated; closed rows skipped before `same_repository`.

Tests (the final tree): nextest core, code, CLI, MCP, HTTP, store, model, import 3 150 passed, 12 skipped; eval `build_graph`, `doc_pointers`, `anonymity`, `layout_cli`, `import_cli` 149 of 152, `doc_pointers` red only for the "Position" citations (resolved by SW-1), the two fixture-status guards green once committed; clippy `--all-targets`, fmt clean; `spec check` clean. Mutations 4 of 4 killed: M1 `best_placed_target_is_the_item`, M2 `position_ignores_task_row_order`, M3 (`in_progress` rows skipped) `claimed_task_marks_its_item_in_progress`, M4 `roadmap_cut_counts_task_lines` and `pool_task_lines_fill_the_cap`. New: core `roadmap_position.rs` (5), CLI `roadmap_position.rs` (13, beyond RD-2/RD-3 `pool_task_lines_fill_the_cap`, `pool_task_title_prints_on_one_line_escaped`, `failing_git_is_one_note`); TE-1 both `expected.json`, core `roadmap.rs` `plan_json`. Not covered: a store-level test of `list_existing_tasks` (covered end to end); the database-error and older-schema notes.
