---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, plugin, ui, fixtures]
ref: Phase 6 slice 2b of `roadmap-items.md` (its Q-5, Q-6); split from `roadmap-position.md` (Q-A) on 2026-10-20, decided under the owner's pre-approval (Q-D, Q-E, Q-F, Q-H), review open; 05 s3.3; 07 s1.2, s3; 08 s2
adrs: [ADR-0052]
---

# roadmap-surfaces (Phase 6, 2b): `next` by the plan, the package's place, MCP and HTTP reads

A compact draft, refreshed against 2a (`docs/features/roadmap-position.md`, RM-0003) as built before it starts. Plan item RM-0030.

## Why

After 2a the owner sees the tasks on the plan, but `next` still takes the lowest-numbered `ready` task (07 s1.2), an agent cannot see where its task sits, and neither MCP nor the daemon serves the plan, which 3a's screen needs.

## Description and interactions

- **`spec task show --next`, MCP `get_task {next: true}`**: `ready` tasks of this repository by (the listing index of the item each executes, canon `spec-roadmap.md` "Position"; then number); tasks executing no open item after them, by number. Without `[roadmap]`, or when no ready task executes an item, the choice is today's. Advisory: no transition, claim or `spec gate` reads the plan; an unfinished predecessor never holds a task (ADR-0012, ADR-0039).
- **Task package** (`docs/canon/task-package.md`): key `roadmap` after `bundle`, `null` or the task's place; the brief's header gains a third line. `schema_version` stays 1 ("Versioning").
- **MCP `get_roadmap(release?, pool?)`**: one CLI library call (`roadmap_with_view`, `View::Capped`): content `spec roadmap 2>&1`, `structuredContent` `--json`, 1a's cut and tail; in `read.rs`, read annotations, the `_meta` size key. `release` with `pool: true` -> the CLI's usage error (exit 2). The description says items change only through `propose_change` and the owner (ADR-0035, 08 AC-3) and names no vocabulary (`mcp_genre`).
- **HTTP `GET /api/projects/:p/roadmap[?release=R | ?pool=true]`**: the JSON document, `View::Browser`, uncut (`truncated` false, `omitted` 0). Both parameters -> 400, nothing run; exit 1 -> 404 with the exit-1 document; no `[roadmap]` (exit 2) -> 503 with the reason (HTTP README "Answers and statuses").
- **Plugin 0.11.0**: the nineteenth tool, the package's key and `next`'s field doc (inside `get_task`'s inputSchema, so `SCHEMA_PINS` changes); `get_roadmap` named in the `analyze` skill.
- **UI (types only)**: `TaskPackage.roadmap`; the roadmap document's types come with 3a.

## Data

Package key (`specengine-model` `TaskPackage`, MCP `mirror.rs`):

```json
"roadmap": {"item": "RM-0011", "group": "release", "release": "beta", "position": 5,
            "of": 8, "priority": "should", "waits_on": ["RM-0010"]}
```

`group` `release`, `pool` or `unplaced`; `release` as written, null outside a release or mistyped; `position` null outside a release; `of` the group's open items; `waits_on` the item's. `null`: no `[roadmap]`, a bad table (package note `roadmap: <why>`, as `bundle: <why>`), or no open item executed. Brief, the header's third line (`SECTIONS` stays 10): `Roadmap: RM-0011, release beta, position 5 of 8[, waits on RM-0010]`; `Roadmap: <ID>, pool, <of> open`; `Roadmap: <ID>, unplaced, <of> open`; none when `null`, so the brief is byte-identical (wording confirmed at the refresh).

`INSTRUCTIONS` Reads line gains `; get_roadmap(release?, pool?) = roadmap` (40 B): 1 892 -> 1 932 B, 1 754 -> 1 794 less the tasks line, 1 968 -> 2 008 with probes, 40 B left.

## Decisions

Recorded 2026-10-20 under the owner's pre-approval, review open; no new ADR:

- **Q-D** 7 keys after `bundle`, the brief line third in the header. Cost of the other: the draft's 5 keys cannot print "of <n>" or tell pool from unplaced.
- **Q-E** the package's plan is read at the reading root, as `bundle_hash` (`task-package.md:37`); `assemble` already refreshes that index. Cost of the other: a second index, a branch-dependent position.
- **Q-F** a bad `[roadmap]` on `--next`: number order plus the package note. Cost of the other: exit 2 blocks reading a task over a typo in the plan.
- **Q-H** only `TaskPackage` goes to 26 keys; the roadmap document's key sets and a `TaskRoadmap` set come with 3a. Cost of the other: 57+ sets and UI churn now.

## Acceptance criteria

rust-developer, eight mutations (M6 dropped to an unmutated cell; M12, M13 added):

- [ ] RD-5 -- next, CLI `tests/task_next_roadmap.rs` (new). M5: number order kept -> `next_follows_the_roadmap`; M13: a bad `[roadmap]` exits 2 -> `bad_roadmap_falls_back_with_a_note`. Unmutated: `tasks_without_an_item_come_last`, `next_without_roadmap_is_lowest_number` (spec-a; roadmap-a without the table).
- [ ] RD-6 -- advisory. M7: a predecessor check added -> `unfinished_predecessor_never_holds_a_task` (next returns it, `claim` passes, `spec gate` passes).
- [ ] RD-7 -- package. M8: the key dropped, or null for an item task -> `package_roadmap_places_the_task` (7 keys after `bundle`, the brief line). Unmutated: `p2_2_the_package_schema_is_pinned_at_version_1` (red for `schema_version` 2); `PACKAGE_KEYS` 26; without `[roadmap]` the brief byte-identical.
- [ ] RD-8 -- MCP, `tests/mcp_roadmap.rs` (new). M9: output differing from the CLI -> `get_roadmap_equals_the_cli` (content, `structuredContent`, exits 0, 1, 2, both fixtures); M12: the read opening the queue with schema steps -> `ac18_reads_add_no_table` (`mcp_index.rs`, `get_roadmap` added to its calls). Plus `TOOLS` 19, the `mcp_door` arm, the `INSTRUCTIONS` re-pins.
- [ ] RD-9 -- HTTP, `tests/roadmap.rs` (new). M10: the route capped -> `roadmap_route_is_uncut`. Plus the 400 for both parameters, the 404 naming `roadmap` (4 files), `daemon_keys` 26.
- [ ] RD-10 -- plugin. M11: the version kept -> `tool_schemas_follow_the_plugin_version` (`plugin_files.rs`). Plus `get_roadmap` named in `analyze` (`plugin_skills.rs` 19), PINS and SCHEMA_PINS 0.11.0, the 0.10.0 assertions restated.

ui-developer:

- [ ] UI-1 -- `ui/src/api/daemonKeys.test.ts` green with `TaskPackage.roadmap` (KEYS, CITED, the count 26), `npm test` and `tsc` clean.

## Churn (the test-engineer's unless marked)

- **MCP**: `INSTRUCTIONS.len()` 1 892 at `mcp_create.rs:395`, `mcp_path.rs:368`, `mcp_tasks.rs:247`, `mcp_decision.rs:358`; 1 968 with probes at `mcp_create.rs:428`, `mcp_code.rs:1615`; `mcp_code.rs:1526-1627` `PLANNED_INSTRUCTIONS` text, `:1591`, `:1596`, `PLANNED_BLAKE3` and the less-tasks-line hash; `mcp_path.rs:352-370`, `mcp_decision.rs:345-360` (1 754, both hashes); `TOOLS: [&str; 18]` (`tests/common/read.rs:75`) and `plugin_skills.rs:1469`; `mcp_door.rs` a `calls_of` arm (`:113`), the called set (`:566`); `mcp_index.rs:71-95` calls; `PACKAGE_V1` (`mcp_tasks.rs:386`) `roadmap.*` optional.
- **CLI**: `PACKAGE_KEYS` 25 -> 26 (`tests/task_common/mod.rs:36`).
- **HTTP**: the 404 route list (`roadmap` after `health`) pinned in `tests/methods.rs`, `health.rs`, `ui.rs`, `tasks.rs`; `tests/tasks.rs:9,43,45,334` (25 keys); `daemon_keys.rs:1136` (25 -> 26), `:1153` (`notes` at 25); `fixtures/daemon-keys.json` via `SPECENGINE_WRITE_DAEMON_KEYS=1`.
- **Plugin**: `plugin_files.rs:1318` (`PINS.last()` 0.10.0), `:1540-1556` ("0.9.0's schemas are today's without `roles`"), `:221` ("6b-2's 0.11.0" -> 0.12.0).
- **UI** (ui-developer): `ui/src/api/provisional.ts:1160` `roadmap: TaskRoadmap | null` with a `docs/canon/task-package.md` "Package" citation; `daemonKeys.test.ts:273-299` (KEYS, CITED), `:724-725` (25 -> 26, title too); `ui/src/mocks/tasks.ts`, `ui/src/test/builders.ts` (`roadmap: null`). No generated types (`ui/src/api/types.ts:1-3`).
- **Texts "lowest-numbered"**: `tasks.md:18`, 07 `:34` (spec-writer); the code's in Files.

spec-writer (bytes by `wc -c` 2026-10-20; trim first, never a raised cap):

- [ ] SW-1 -- `spec-roadmap.md` (11 276 / 12 288 after 2a, 1 012 B left) "Next", "Surfaces": pointers to `task-package.md`, `mcp-read.md`.
- [ ] SW-2 -- `tasks.md:18` (11 881 / 12 288 after 2a) `--next`, about +110 B.
- [ ] SW-3 -- `task-package.md` (10 115 / 12 288) "Package" 26 keys and the bullet, "Brief", "MCP"; at most 450 B.
- [ ] SW-4 -- `mcp-read.md` (12 117, 171 B left): "Binary" `:15` pointer (about 45 B); "Texts" `:32` 1 932 / 2 008 / 40 B left (byte-neutral).
- [ ] SW-5 -- MCP README (10 210, 30 B left): none while the tool sits in `read.rs`.
- [ ] SW-6 -- HTTP README (10 234, 6 B left): the route row (about 80 B) after a trim of at least 74 B; `:50` "`omitted` null" -> 0 for the roadmap.
- [ ] SW-7 -- `plugin-files.md` (5 072): "nineteen", 0.11.0 with the PINS hash, the MINOR examples; `plugin.md:22` (9 568) the `analyze` row.
- [ ] SW-8 -- model README (9 032) "26 keys"; CLI README (10 148, 92 B left) `{tree,show,search,graph,roadmap}_with_view`; core README none.
- [ ] SW-9 -- 07 `:34`, a `get_roadmap` row, `:133`; 08 s2 Phase 6; `CLAUDE.md` State; RM-0030 shipped, the view regenerated; 6b-2 and 7a refreshed.

## Files

- **rust-developer**: core `src/roadmap.rs` (the next key); model `src/task.rs`; CLI `src/task.rs:73,586-591`, `src/main.rs:406`, `src/package.rs`, `src/roadmap.rs` (`roadmap_with_view`), `src/lib.rs`; MCP `src/read.rs`, `src/mirror.rs:774-806`, `src/server.rs:39-73`, `src/tasks.rs:40,184`; HTTP `src/app.rs` (route, handler, 404 text `:263-273`); `plugin/specengine/.claude-plugin/plugin.json`, `skills/analyze/SKILL.md`.
- **test-engineer**: the new files above; the churn; `fixtures/daemon-keys.json`; the PINS hash before SW-7.
- **ui-developer** (required): the UI churn.

## Dependencies and risks

- After 2a ships; about 3 days, 2-3 iterations. Overflow seam: the HTTP route (only 3a reads it).
- **`INSTRUCTIONS`**: 40 B left after this; 6b-2 (`verify-node.md`, +46 B: 1 978 / 2 054, 6 B over), 7a (about 33 B) and 7b (46 B) together cut about 85 B. 6b-2 takes 0.12.0 and `TOOLS` 20; 7a's package, `mirror.rs`, `daemon-keys.json`, `provisional.ts` counts shift here; 11a-3's MINOR follows 6b-2's.
- **HTTP notes**: the join's notes are stderr only (canon `spec-roadmap.md` "Notes"), so the route's JSON lacks them; 3a decides at the refresh whether it needs a key.
- **The shared reader** (2a review): `list_existing_tasks` decodes every row of the project, a `runs` query per task, so a corrupt `runs` row of a closed or another repository's task adds a note; a narrow SELECT when `get_roadmap` reuses it is an option, not a requirement.
- **Version skew**: `spec`, `specengine-mcp`, `specengine-http` from one commit.

## Out of scope

The screen (3a); moves (3b); a queue column or schema step; a gate; dates; the roadmap document's daemon key sets (3a).

## Implementation

Not started.
