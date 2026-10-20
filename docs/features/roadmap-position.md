---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, plugin]
ref: Phase 6 slice 2 of `roadmap-items.md` (its Q-5, Q-6, N7); the requirement-analyst's breakdown of 2026-10-20, decided under the owner's pre-approval, review open; 05 s3.3; 07 s1.2, s3; 08 s2
adrs: [ADR-0052]
---

# roadmap-position (Phase 6, 2): live progress, `next` by the plan, MCP and HTTP reads

A compact draft, refreshed against 1a and 1b as built before it starts.

## Why

1a and 1b place work from git alone; the owner still cannot see what is being worked on, `next` still takes the lowest-numbered `ready` task (`crates/specengine-cli/src/task.rs` `--next`; 07 s1.2 "priority later"), and an agent cannot see where its task sits on the path. Slice 2 joins the plan to the queue, read-only, and serves it on every surface.

## Description and interactions

- **`spec roadmap`** also reads the queue, read-only: an item is **in progress** when a task executing it is `in_progress` (its line gains ` | task T-NNNN in_progress`, JSON `tasks: [{id, status}]` per item); open tasks (not `done`, `accepted`, `cancelled`) executing no item are listed under the pool (JSON `pool_tasks`). Without a queue row nothing changes from 1a's output. The 1b view stays git only.
- **Which item a task executes** (N7): of its `targets` with the `[roadmap] prefix`, the best placed: earliest release, then position; pool, then unplaced last; ties by ID. Editing an item after approval may mark its task `stale`, informational as today.
- **`spec task show --next`, MCP `get_task {next: true}`**: `ready` tasks of this repository ordered by their item's (release rank, position, priority rank), then task number; tasks with no item after every placed one, by number. Without `[roadmap]` the order is today's. Advisory: no transition, claim or gate reads the plan (Q-5, ADR-0012); an unfinished predecessor never holds a task.
- **Task package**: a new key **`roadmap`** (the existing `plan` key is the agent's submitted plan): `null`, or `{item, release, position, priority, waits_on}`; the brief gains one line `Roadmap: <ID>, release <r>, position <p> of <n>`. A new key keeps `schema_version` 1 (`docs/canon/task-package.md` "Versioning").
- **MCP** `get_roadmap(release?, pool?)` = `spec roadmap --json` (structuredContent) and its text (content), cut at 40 000 characters with 1a's tail. Read-only; agents propose items, never set release or priority (ADR-0035, 08 AC-3).
- **HTTP** `GET /api/projects/:p/roadmap[?release=R|?pool=true]`: the JSON document, browser view, uncut (`View::Browser`).
- **Plugin** 0.11.0: the nineteenth tool; `get_roadmap` named in the `analyze` or `implement` skill.

## Rules and edge cases

- WHEN the database is deleted, `spec roadmap` SHALL give 1a's git-derived keys unchanged and empty task-derived ones.
- WHEN a project has no `[roadmap]`, `next` and the package SHALL be byte-identical to before.
- WHEN a predecessor is unfinished, `ready`, `claim_task` and the gate SHALL behave as without the chain.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- next; CLI `tests/task_next_roadmap.rs`. M: number order kept -> `next_follows_the_roadmap`; behaviour without `[roadmap]` changed -> `next_without_roadmap_is_lowest_number`.
- [ ] RD-2 -- advisory. M: a predecessor check added -> `unfinished_predecessor_never_holds_a_task`.
- [ ] RD-3 -- position. M: a claimed task ignored -> `claimed_task_marks_its_item_in_progress`. Unmutated: `database_loss_keeps_the_plan`, `best_placed_target_is_the_item`, `task_without_item_joins_the_pool`.
- [ ] RD-4 -- package. M: `schema_version` 2 -> `p2_2_the_package_schema_is_pinned_at_version_1` (`mcp_tasks.rs`); the key named `plan` -> the package key pins.
- [ ] RD-5 -- MCP. M: output differing from the CLI -> `get_roadmap_equals_the_cli`; `mcp_door.rs` and `server.rs`' size asserts green.
- [ ] RD-6 -- HTTP. M: the route capped -> `roadmap_route_is_uncut`; `daemon_keys.rs` and `fixtures/daemon-keys.json` follow the new keys.
- [ ] RD-7 -- plugin. M: the version not bumped -> `tool_schemas_follow_the_plugin_version` (`crates/specengine-mcp/tests/plugin_files.rs`); all nineteen tools named.

test-engineer: the queue cells over `roadmap-a` (a claimed task, a task with two item targets, a task with none, a deleted DB); `get_roadmap` against the CLI on both fixtures; the HTTP route on both.

spec-writer (bytes 2026-10-20; trim before adding, never a raised cap):

- [ ] SW-1 -- `spec-roadmap.md` "Position", "Next", "Surfaces".
- [ ] SW-2 -- `tasks.md` `--next` (11 746 of 12 288); `task-package.md` (10 115); `mcp-read.md` (12 117: 171 B left); `plugin-files.md` (5 072: nineteen, 0.11.0); HTTP README (10 234: 6 B left); MCP README (10 210: 30 B left); 07 s1.2 `get_task`, s3; `CLAUDE.md` State.

## Files

rust-developer: core `roadmap.rs` (the queue join, the next key), model (the package's `roadmap`), CLI `roadmap.rs`, `task.rs`, `package.rs`; MCP the tool, its mirror, `INSTRUCTIONS`; HTTP the route; `plugin/` (version, skill). test-engineer: CLI `tests/task_next_roadmap.rs`, `tests/roadmap.rs`; MCP `tests/mcp_tasks.rs`, `tests/mcp_door.rs`, `tests/plugin_files.rs`; HTTP `tests/daemon_keys.rs`, `fixtures/daemon-keys.json`.

## Dependencies and interactions

- After 1b; about 1 week.
- **6b-2** `verify-node.md`: its plugin MINOR becomes 0.12.0; `INSTRUCTIONS` had 1 892 B without probes after 6b-1 (1 968 with them, of 2 048): `get_roadmap` (about 40 B) and `check_binding` do not both fit without a trim, so 6b-2 is refreshed after this slice.
- **7a** `bundle-bindings.md`: the package, `mirror.rs`, `p2_2…`, `daemon-keys.json`, `provisional.ts` and the `daemonKeys` counts shift here; its RD-9, RD-11 and UI-1 are refreshed after.
- **11a-3** takes a plugin MINOR after 6b-2's.

## Out of scope

The screen (3a); moves (3b); a queue column or schema step (N7); a gate; dates.

## Implementation

Not started.
