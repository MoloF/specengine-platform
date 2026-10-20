---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: the plan items of [roadmap]
---

# Roadmap

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

roadmap: 28 open, 1 done; current mvp; to mvp 2-3 days; total 41-68 days

## release mvp | current | mvp cut | 1 open, 0 done | 2-3 days

1. [RM-0001](../roadmap/RM-0001.md) | Live check: real tasks of each pilot through the full cycle | draft | must | 2-3

## release v1 | 9 open, 1 done | 26-42 days

1. [RM-0003](../roadmap/RM-0003.md) | roadmap-position (Phase 6, 2): live progress, next by the plan, MCP and HTTP reads | draft | must | 4-6
2. [RM-0004](../roadmap/RM-0004.md) | roadmap-screen (Phase 6, 3a): the Roadmap screen, read-only | draft | must | 4-6 | waits on RM-0003
3. [RM-0005](../roadmap/RM-0005.md) | verify-node (6b-2): spec verify --node, MCP check_binding | draft | should | 2-3
4. [RM-0006](../roadmap/RM-0006.md) | bundle-bindings (7a): an agent's reads carry its nodes' code bindings | draft | should | 3-5 | waits on RM-0005
5. [RM-0007](../roadmap/RM-0007.md) | impact (7b): the blast radius of a node, a symbol or a revision | draft | should | 3-5 | waits on RM-0006
6. [RM-0008](../roadmap/RM-0008.md) | tested-evidence (8b): a passing test run becomes evidence in spec.lock | draft | should | 3-5 | waits on RM-0007
7. [RM-0009](../roadmap/RM-0009.md) | health-drift (9b): Health shows drift between spec.lock and the tree | draft | should | 2-4 | waits on RM-0006, RM-0008
8. [RM-0010](../roadmap/RM-0010.md) | layer-c-reads (11a-3): monikers in spec symbols and find_symbols | draft | should | 2-3
9. [RM-0011](../roadmap/RM-0011.md) | Drift screens in the web UI (Phase 4) | draft | could | 3-5 | waits on RM-0009

## release v2 | 7 open, 0 done | 13-23 days

1. [RM-0013](../roadmap/RM-0013.md) | Full migration of pilot A | draft | must | 2-4
2. [RM-0012](../roadmap/RM-0012.md) | MCP staging forms review_proposal, approve_task | draft | should | 1-2
3. [RM-0014](../roadmap/RM-0014.md) | Full migration of pilot B | draft | should | 3-5 | waits on RM-0013
4. [RM-0015](../roadmap/RM-0015.md) | A shared project linked from several projects | draft | should | 2-4
5. [RM-0016](../roadmap/RM-0016.md) | Metrics and compaction | draft | should | 2-3
6. [RM-0017](../roadmap/RM-0017.md) | Notifications | draft | could | 1-2
7. [RM-0018](../roadmap/RM-0018.md) | URL-mode elicitation onto UI cards | draft | could | 2-3

## pool | 11 open, 0 done | 8-13 days, 6 unestimated

- [RM-0029](../roadmap/RM-0029.md) | Hygiene pass: the review nits of 5b-1b, 5c and 8a-2 | draft | should | 1
- [RM-0019](../roadmap/RM-0019.md) | roadmap-moves (Phase 6, 3b): moves from the Roadmap screen | draft | could | -
- [RM-0021](../roadmap/RM-0021.md) | bevy-dump (10b): the project's schedule dump annotates symbols | draft | could | 3-5
- [RM-0024](../roadmap/RM-0024.md) | machine-bindings (12b): a string constant binds its node without a marker | draft | could | -
- [RM-0025](../roadmap/RM-0025.md) | constant-reach (12c): what reaches a bound constant, listed, never evidence | draft | could | - | waits on RM-0024, RM-0006
- [RM-0026](../roadmap/RM-0026.md) | spec mcp: the MCP stdio bridge | draft | could | 1-2
- [RM-0027](../roadmap/RM-0027.md) | Queue export to Beads or Task Master | draft | could | 2-3
- [RM-0028](../roadmap/RM-0028.md) | Spec digest into AGENTS.md or CLAUDE.md for external reviewers | draft | could | 1-2
- [RM-0020](../roadmap/RM-0020.md) | code-index-speed (6c): a faster code index | draft | - | -
- [RM-0022](../roadmap/RM-0022.md) | layer-c-macros (11b): items under attribute macros keep their monikers | draft | - | -
- [RM-0023](../roadmap/RM-0023.md) | layer-c-registration (11c): what the schedule dump cannot give | draft | - | - | waits on RM-0021

## unplaced | 0 open, 0 done | 0 days
