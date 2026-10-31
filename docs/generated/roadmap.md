---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: the plan items of [roadmap]
---

# Roadmap

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

roadmap: 42 open, 21 done; current mvp; to mvp 2-3 days; total 13-21 days, 21 unestimated

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

## release v3 | 20 open, 5 done | 0 days, 20 unestimated

1. [RM-0046](../roadmap/RM-0046.md) | board-ui (Phase 7, step 5): the board, filters, the card panel, write routes | draft | must | -
2. [RM-0047](../roadmap/RM-0047.md) | card-trace (Phase 7, step 6): targets, suspect marks, clearing, re-estimates, rollup | draft | must | -
3. [RM-0048](../roadmap/RM-0048.md) | trace-views (Phase 7, step 7): flow badges, stage:, timeline lanes, a node's cards | draft | must | - | waits on RM-0046, RM-0047
4. [RM-0049](../roadmap/RM-0049.md) | hub-identity (Phase 7, step 8): a public origin, an identity header, a person in Author | draft | should | -
5. [RM-0050](../roadmap/RM-0050.md) | hub-confirm (Phase 7, step 9): the UI confirms under a named login | draft | should | - | waits on RM-0049
6. [RM-0051](../roadmap/RM-0051.md) | hub-git (Phase 7, step 10): sole writer, merge and push, the index in the commit | draft | should | - | waits on RM-0049
7. [RM-0035](../roadmap/RM-0035.md) | node-history: spec show --history and the node's History tab | draft | should | - | waits on RM-0047, RM-0051
8. [RM-0036](../roadmap/RM-0036.md) | impact-daemon: spec impact on the daemon and the graph's impact mode | draft | should | - | waits on RM-0047, RM-0051
9. [RM-0052](../roadmap/RM-0052.md) | board-in-git (Phase 7, step 10b): the board's snapshot on a dedicated branch | draft | should | - | waits on RM-0051
10. [RM-0053](../roadmap/RM-0053.md) | hub-mcp (Phase 7, step 11): /mcp with a session, identity to author, hub tools | draft | should | - | waits on RM-0049
11. [RM-0054](../roadmap/RM-0054.md) | scale-reads (Phase 7, step 12): an edge table, a trusted index, reads at 100 000 nodes | draft | should | -
12. [RM-0055](../roadmap/RM-0055.md) | zone-index (Phase 7, step 13): an index-free zone and status: retired | draft | should | - | waits on RM-0054
13. [RM-0056](../roadmap/RM-0056.md) | change-record (Phase 7, step 14): what a change invalidated, cost and who accepted it | draft | should | - | waits on RM-0047, RM-0051, RM-0035, RM-0036
14. [RM-0017](../roadmap/RM-0017.md) | Notifications | draft | could | - | waits on RM-0049
15. [RM-0057](../roadmap/RM-0057.md) | draft-overlay (Phase 7, step 15): revisable drafts in reads | draft | could | - | waits on RM-0053
16. [RM-0058](../roadmap/RM-0058.md) | reuse-overrides (Phase 7, step 16): overrides, variants, usage reads | draft | could | -
17. [RM-0059](../roadmap/RM-0059.md) | intake-similar (Phase 7, step 17): similar and distinct_from on create | draft | could | - | waits on RM-0054
18. [RM-0060](../roadmap/RM-0060.md) | collision-pass (Phase 7, step 18): candidates, a pair ledger, a skill | draft | could | - | waits on RM-0059
19. [RM-0061](../roadmap/RM-0061.md) | pack (Phase 7, step 19): spec init --pack and a config proposal | draft | could | -
20. [RM-0062](../roadmap/RM-0062.md) | writer-skills (Phase 7, step 20): a profile plugin or MCP prompts | draft | could | - | waits on RM-0053

## pool | 15 open, 1 done | 7-12 days, 11 unestimated

- [RM-0040](../roadmap/RM-0040.md) | check-parity: what pilot A's own docs check still does beside spec check | draft | should | -
- [RM-0019](../roadmap/RM-0019.md) | roadmap-moves (Phase 6, 3b): moves from the Roadmap screen | draft | could | - | waits on RM-0051
- [RM-0021](../roadmap/RM-0021.md) | bevy-dump (10b): the project's schedule dump annotates symbols | draft | could | 3-5
- [RM-0024](../roadmap/RM-0024.md) | machine-bindings (12b): a string constant binds its node without a marker | draft | could | -
- [RM-0025](../roadmap/RM-0025.md) | constant-reach (12c): what reaches a bound constant, listed, never evidence | draft | could | - | waits on RM-0024
- [RM-0026](../roadmap/RM-0026.md) | spec mcp: the MCP stdio bridge | draft | could | 1-2
- [RM-0027](../roadmap/RM-0027.md) | Queue export to Beads or Task Master | draft | could | 2-3
- [RM-0028](../roadmap/RM-0028.md) | Spec digest into AGENTS.md or CLAUDE.md for external reviewers | draft | could | 1-2
- [RM-0031](../roadmap/RM-0031.md) | symbol-renamed: delta.symbols.renamed from the anchor pairing | draft | could | -
- [RM-0034](../roadmap/RM-0034.md) | tree-bindings: opt-in binding counters in spec tree, get_tree and the daemon's tree | draft | could | -
- [RM-0037](../roadmap/RM-0037.md) | evidence-level: each node's evidence level, computed per read | draft | could | -
- [RM-0063](../roadmap/RM-0063.md) | Load flakes: tests that fail under full parallel load | draft | could | -
- [RM-0020](../roadmap/RM-0020.md) | code-index-speed (6c): a faster code index | draft | - | -
- [RM-0022](../roadmap/RM-0022.md) | layer-c-macros (11b): items under attribute macros keep their monikers | draft | - | -
- [RM-0023](../roadmap/RM-0023.md) | layer-c-registration (11c): what the schedule dump cannot give | draft | - | - | waits on RM-0021

## unplaced | 0 open, 0 done | 0 days
