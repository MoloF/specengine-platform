---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: the plan items of [roadmap]
---

# Roadmap

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

roadmap: 45 open, 57 done; current mvp; to mvp 2-3 days; total 13-21 days, 12 unestimated

## release mvp | current | mvp cut | 1 open, 0 done | 2-3 days

1. [RM-0001](../roadmap/RM-0001.md) | Live check: real tasks of each pilot through the full cycle | draft | must | 2-3

## release v1 | 0 open, 13 done | 0 days

## release v2 | 6 open, 2 done | 11-18 days, 1 unestimated

1. [RM-0016](../roadmap/RM-0016.md) | Metrics and compaction | draft | must | 2-3
2. [RM-0039](../roadmap/RM-0039.md) | check-parity-b3: gap B3 of pilot A, before its id-width debt expires | draft | must | -
3. [RM-0014](../roadmap/RM-0014.md) | Full migration of pilot B | draft | should | 3-5
4. [RM-0015](../roadmap/RM-0015.md) | A shared project linked from several projects | draft | should | 2-4
5. [RM-0018](../roadmap/RM-0018.md) | URL-mode elicitation onto UI cards | draft | could | 2-3
6. [RM-0038](../roadmap/RM-0038.md) | task-stage: the owner's staging form approve_task | draft | could | 2-3

## release v3 | 0 open, 28 done | 0 days

## release v4 | 0 open, 8 done | 0 days

## release v5 | 11 open, 5 done | 0 days, 11 unestimated

1. [RM-0096](../roadmap/RM-0096.md) | departments (Phase 7, v5 step 4): the owner's words, department statuses and directions | draft | must | -
2. [RM-0097](../roadmap/RM-0097.md) | card-questions (Phase 7, v5 step 5): a task asks the writers, and the answer is a recorded decision | draft | must | -
3. [RM-0098](../roadmap/RM-0098.md) | handoff-progress (Phase 7, v5 step 6): a parent shows Ready to accept and a progress bar | draft | must | -
4. [RM-0099](../roadmap/RM-0099.md) | design-sweep (Phase 7, v5 step 7): every other screen on the foundation | draft | must | - | waits on RM-0098
5. [RM-0059](../roadmap/RM-0059.md) | intake-similar (Phase 7, v5 step 8): similar and distinct_from on create | draft | should | -
6. [RM-0060](../roadmap/RM-0060.md) | collision-pass (Phase 7, v5 step 9): candidates, a pair ledger, a skill | draft | should | - | waits on RM-0059
7. [RM-0088](../roadmap/RM-0088.md) | plain-questions (Phase 7, v5 step 10): a question's gist and options, collisions as pairs | draft | should | - | waits on RM-0059, RM-0060, RM-0097
8. [RM-0061](../roadmap/RM-0061.md) | pack (Phase 7, v5 step 11): spec init --pack and a config proposal | draft | could | - | waits on RM-0096
9. [RM-0086](../roadmap/RM-0086.md) | department-overview (Phase 7, v5 step 12): an Overview per department, counted by status category | draft | could | - | waits on RM-0096, RM-0097, RM-0098
10. [RM-0089](../roadmap/RM-0089.md) | hub-department (Phase 7, v5 step 13): a hub's per-login default department | draft | could | - | waits on RM-0096
11. [RM-0100](../roadmap/RM-0100.md) | journeys (Phase 7, v5, optional): one act hands a task to several departments | draft | could | - | waits on RM-0096, RM-0098

## pool | 27 open, 1 done | 7-12 days, 23 unestimated

- [RM-0035](../roadmap/RM-0035.md) | node-history: spec show --history and the node's History tab | draft | should | -
- [RM-0036](../roadmap/RM-0036.md) | impact-daemon: spec impact on the daemon and the graph's impact mode | draft | should | -
- [RM-0040](../roadmap/RM-0040.md) | check-parity: what pilot A's own docs check still does beside spec check | draft | should | -
- [RM-0052](../roadmap/RM-0052.md) | board-in-git (Phase 7, step 10c): the board's snapshot on a dedicated branch | draft | should | -
- [RM-0055](../roadmap/RM-0055.md) | zone-index (Phase 7, step 13): an index-free zone and status: retired | draft | should | -
- [RM-0056](../roadmap/RM-0056.md) | change-record (Phase 7, step 14): what a change invalidated, cost and who accepted it | draft | should | - | waits on RM-0035, RM-0036
- [RM-0070](../roadmap/RM-0070.md) | trial-followups (Phase 7): what the second trial and earlier review left | draft | should | -
- [RM-0017](../roadmap/RM-0017.md) | Notifications | draft | could | -
- [RM-0019](../roadmap/RM-0019.md) | roadmap-moves (Phase 6, 3b): moves from the Roadmap screen | draft | could | -
- [RM-0021](../roadmap/RM-0021.md) | bevy-dump (10b): the project's schedule dump annotates symbols | draft | could | 3-5
- [RM-0024](../roadmap/RM-0024.md) | machine-bindings (12b): a string constant binds its node without a marker | draft | could | -
- [RM-0025](../roadmap/RM-0025.md) | constant-reach (12c): what reaches a bound constant, listed, never evidence | draft | could | - | waits on RM-0024
- [RM-0026](../roadmap/RM-0026.md) | spec mcp: the MCP stdio bridge | draft | could | 1-2
- [RM-0027](../roadmap/RM-0027.md) | Queue export to Beads or Task Master | draft | could | 2-3
- [RM-0028](../roadmap/RM-0028.md) | Spec digest into AGENTS.md or CLAUDE.md for external reviewers | draft | could | 1-2
- [RM-0031](../roadmap/RM-0031.md) | symbol-renamed: delta.symbols.renamed from the anchor pairing | draft | could | -
- [RM-0034](../roadmap/RM-0034.md) | tree-bindings: opt-in binding counters in spec tree, get_tree and the daemon's tree | draft | could | -
- [RM-0037](../roadmap/RM-0037.md) | evidence-level: each node's evidence level, computed per read | draft | could | -
- [RM-0057](../roadmap/RM-0057.md) | draft-overlay (Phase 7, step 15): revisable drafts in reads | draft | could | -
- [RM-0058](../roadmap/RM-0058.md) | reuse-overrides (Phase 7, step 16): overrides, variants, usage reads | draft | could | -
- [RM-0062](../roadmap/RM-0062.md) | writer-skills (Phase 7, step 20): a profile plugin or MCP prompts | draft | could | -
- [RM-0063](../roadmap/RM-0063.md) | Load flakes: tests that fail under full parallel load | draft | could | -
- [RM-0087](../roadmap/RM-0087.md) | roadmap-states (Phase 7, v4 step 5): the owner's states, and views by state, release and timeline | draft | could | -
- [RM-0102](../roadmap/RM-0102.md) | flow-directions (Phase 7, v5 pool): Flow drawn through a kind direction | draft | could | - | waits on RM-0096
- [RM-0020](../roadmap/RM-0020.md) | code-index-speed (6c): a faster code index | draft | - | -
- [RM-0022](../roadmap/RM-0022.md) | layer-c-macros (11b): items under attribute macros keep their monikers | draft | - | -
- [RM-0023](../roadmap/RM-0023.md) | layer-c-registration (11c): what the schedule dump cannot give | draft | - | - | waits on RM-0021

## unplaced | 0 open, 0 done | 0 days
