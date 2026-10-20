---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, plugin, ui, fixtures]
ref: Phase 6 slice 2b of `roadmap-items.md` (its Q-5, Q-6); split from `roadmap-position.md` (Q-A) and refreshed against 2a (1a8e5ba) on 2026-10-20, decided under the owner's pre-approval (Q-D..Q-H), review open; 05 s3.3; 07 s1.2, s3; 08 s2; developer iteration 1 accepted by the review (one minor, nits); iteration 2, decided under the same pre-approval, accepted by the re-review
shipped: 2026-10-20
adrs: [ADR-0052]
---

# roadmap-surfaces (Phase 6, 2b): `next` by the plan, the package's place, MCP and HTTP reads

Plan item RM-0030. Builds on 2a (`roadmap-position.md`): core `plan_tasks`, the CLI's best-effort queue read (`list_existing_tasks`, no schema step).

## Why

After 2a the owner sees which open item each task executes, but `spec task show --next` still took the lowest-numbered `ready` task, an agent could not see where its task sits, and neither MCP nor the daemon served the plan, which 3a's screen (`roadmap-screen.md`) reads. 2b serves 2a's join on those surfaces: computed per read, never stored (ADR-0052), advisory (ADR-0012, ADR-0039). No ADR: it applies ADR-0052, ADR-0012/0039, ADR-0027 (a key keeps `schema_version` 1), ADR-0035 and 08 AC-3 (the new tool reads only), ADR-0008.

Delivered: `--next` and `get_task {next: true}` take the first `ready` task in the plan's order of the items the tasks execute (`docs/canon/spec-roadmap.md` "Next"); the package's 26th key `roadmap` and the brief's third line (`docs/canon/task-package.md` "Package", "Brief"); MCP `get_roadmap {release?, pool?}`, the nineteenth tool, capped as the CLI; the daemon's `GET /api/projects/:p/roadmap`, uncut (`spec-roadmap.md` "Surfaces", `crates/specengine-http/README.md` "Endpoints"); plugin 0.11.0, `analyze` naming `get_roadmap`; the UI's `TaskRoadmap`. No claim, transition or gate reads the plan; no table, no schema step.

## Data

Moved to the canon at shipping, read there: the package key and the brief line `docs/canon/task-package.md` "Package", "Brief"; the `next` order `docs/canon/spec-roadmap.md` "Next"; `get_roadmap` and the route `spec-roadmap.md` "Surfaces", `docs/canon/mcp-read.md` (`INSTRUCTIONS` 1 932 B, 1 794 less the tasks line, 2 008 with `probes`: 40 B left), `crates/specengine-http/README.md` "Endpoints", "Answers and statuses".

## Decisions

2026-10-20 under the owner's pre-approval, review open; no new ADR.

| | Answer |
|---|---|
| Q-D | 7 keys after `bundle`; the brief line third in the header; `schema_version` 1 |
| Q-E | the plan read at the reading root, as `bundle_hash` (`task-package.md` "Package") |
| Q-F | a bad `[roadmap]` on `--next`: number order plus the note, never exit 2 |
| Q-G | `get_roadmap` reuses 2a's queue read: no table (mcp-read AC-18) |
| Q-H | only `TaskPackage` gains a key (26); the roadmap document's UI types and daemon key sets come with 3a |
| refresh | 40 B at the end of the reads' line; `waits on` in the brief only in a release; the HTTP 400 carries the CLI's line; `READ_TOOLS` stays 4 |
| iteration 2 | the package builds its plan from input with file bytes, as `spec roadmap` does, so `waits_on` and `position` equal `spec roadmap --json` even for a `depends_on` written through a look-alike alias (the review's minor); the UI union is `RoadmapGroupKind`, leaving `RoadmapGroup` to 3a's group object |

## Acceptance criteria

rust-developer, eight mutations (M = the mutation that must turn the named test red):

- [x] RD-5 -- `next`, CLI `tests/task_next_roadmap.rs` (new; `task_common`'s `Pair` on git copies of `roadmap-a`, slug `tide-runner`). M5 (number order kept) -> `next_follows_the_roadmap`: ready tasks on beta's sixth item, its first and a pool item, in that number order: the first item's task first, then the sixth's, then the pool's. M13 (a bad `[roadmap]` exits 2) -> `bad_roadmap_falls_back_with_a_note`: exit 0, number order, `roadmap` `null`, one `roadmap: specengine.toml:<line>: ...` note. Unmutated: `tasks_without_an_item_come_last`, `next_without_roadmap_is_lowest_number` (spec-a; roadmap-a with the table removed: no note, brief without the line).
- [x] RD-6 -- advisory, same file. M7 (a predecessor check added) -> `unfinished_predecessor_never_holds_a_task`: the task on beta's fifth item (waiting on the fourth, open) is `--next`'s, `spec task claim` exits 0, `spec gate` allows the claimed worktree's write.
- [x] RD-7 -- package, same file. M8 (the key dropped, or `null` for an item task) -> `package_roadmap_places_the_task`: over `roadmap-a`, `{"item":"RM-0011","group":"release","release":"beta","position":5,"of":8,"priority":"should","waits_on":["RM-0010"]}`, a pool item's (`position` `null`, `of` 2) and an unplaced item's (`release` `"someday"`, `of` 1) in key order after `bundle`, each brief's third line; a non-item and a `done` task `null`, no line. Unmutated: `brief_roadmap_line_wording`; MCP `p2_2_the_package_schema_is_pinned_at_version_1` green (red for `schema_version` 2); `PACKAGE_KEYS` 26 green in CLI `package.rs`, HTTP `tasks.rs`; CLI `package.rs`'s brief pins green unchanged.
- [x] RD-8 -- MCP, `tests/mcp_roadmap.rs` (new). M9 (an answer differing from the CLI) -> `get_roadmap_equals_the_cli`: `assert_parity` against `common::read::library`'s arm, both eras, git copies of `roadmap-a` and `roadmap-b` with one ready task on an item; `{}`, `{"release": "beta"}`, `{"pool": true}` exit 0, `{"release": "nope"}` exit 1, `{"release": "beta", "pool": true}` and a copy without `[roadmap]` exit 2. M12 (the read opening the queue with schema steps) -> `ac18_reads_add_no_table` (`mcp_index.rs`) over a git copy of `roadmap-a` whose `HOME` holds the index alone: exits 0, 1, 2, both eras, the `CREATE` statements unchanged. Unmutated: read annotations, `_meta`, description pins, `flat_input_problems` empty, no `$ref`; `TOOLS` 19, `mcp_door`, `valid_args`, the `INSTRUCTIONS` re-pins.
- [x] RD-9 -- HTTP, `tests/roadmap.rs` (new). M10 (the route capped) -> `roadmap_route_is_uncut`: 505 release items: the CLI's `--json` `truncated` true; the route 200, `truncated` false, `omitted` 0, every item, byte-equal to `render_json` of `roadmap_with_view(.., View::Browser)`. Unmutated `roadmap_route_answers_as_the_cli`: 200 byte-equal, 404 the exit-1 document, the 400s (`release` with `pool=true`, an unknown name, `pool=yes`) with a fresh `HOME` left empty, 503 on a served spec-a. Plus `health.rs:280`, `daemon_keys` 26.
- [x] RD-10 -- plugin. M11 (the version kept) -> `tool_schemas_follow_the_plugin_version` (`plugin_files.rs`). Plus analyze's list holds `get_roadmap` (`plugin_skills.rs`), `TOOLS` 19, `PINS`, `SCHEMA_PINS` 0.11.0, the 0.10.0 ties restated.

test-engineer:

- [x] TE-1 -- the churn green; `fixtures/daemon-keys.json` regenerated (`SPECENGINE_WRITE_DAEMON_KEYS=1`): `TaskPackage` 26 keys, `roadmap` after `bundle`, no other set changed, 56 sets.
- [x] TE-2 -- the 8 mutations killed; full nextest, clippy `--all-targets`, fmt, eval `doc_pointers` green; a case with `depends_on` written through a look-alike alias (iteration 2): `package_roadmap_follows_a_legacy_alias`.

ui-developer:

- [x] UI-1 -- `TaskPackage.roadmap: TaskRoadmap | null` after `bundle`, `TaskRoadmap` citing `docs/canon/task-package.md` "Package"; lint, test, build clean (after iteration 2's rename).

spec-writer (bytes by `wc -c` before -> after / cap):

- [x] SW-1 -- `docs/canon/spec-roadmap.md` 11 276 -> 12 243 / 12 288: "Next", "Surfaces" (new, last), "Command" `roadmap_with_view`, `roadmap_flags` (+967, 17 B over the 950 planned).
- [x] SW-2 -- `docs/canon/tasks.md` 11 881 -> 11 951: `--next` by "Next".
- [x] SW-3 -- `docs/canon/task-package.md` 10 115 -> 10 606: "Package" `"roadmap":null`, 26 keys, the bullet, `roadmap` from the reading root; "Brief" the third line; "MCP" `next` by "Next", the `INSTRUCTIONS` quote shortened (+491).
- [x] SW-4 -- `docs/canon/mcp-read.md` 12 117 -> 12 172: "Binary" `get_roadmap`, "Schemas" `group`, "Texts" 1 932, 1 794, 2 008, 40 B left.
- [x] SW-5 -- MCP README 10 210: none.
- [x] SW-6 -- HTTP README 10 234 -> 10 232: the route row, "Query" the `release`+`pool=true` 400, "Browser view" `roadmap`'s `omitted` 0, "Worktrees", "Tests"; trimmed "Live tail" types to their canons, three `docs/canon/` prefixes, two clauses.
- [x] SW-7 -- `docs/canon/plugin-files.md` 5 072 -> 5 125 with the hash: nineteen, 0.11.0 and its `PINS`, the MINOR examples; `docs/canon/plugin.md` 9 568 -> 9 623, the `analyze` row; root `README.md` 7 518 "nineteen tools".
- [x] SW-8 -- model README 9 032 "26 keys"; CLI README 10 148 -> 10 173 `{tree,show,search,graph,roadmap}_with_view`, `roadmap_flags`; core README: none.
- [x] SW-9 -- 07 s1.2 `get_task`'s `next` and `roadmap`, a `get_roadmap` row, s3 `roadmap` in the route list; 08 s2 Phase 6; `CLAUDE.md` State 6 311 / 16 384; RM-0030 shipped, the view regenerated; `roadmap-screen.md` (3a) re-pointed at the canon, `verify-node.md` (6b-2: `TOOLS` 20, `INSTRUCTIONS` over 2b, a cut of at least 6 B), `layer-c-reads.md` (11a-3: 0.12.0) refreshed.

## Risks

- **`INSTRUCTIONS`**: 40 B left; 6b-2's 46 B clause needs a cut of at least 6 B (`verify-node.md` RD-8); 7a (about 33 B) and 7b (46 B) about 85 B more in all.
- **Plugin versions**: 6b-2 (`TOOLS` 20) and 11a-3 take the next MINORs from 0.12.0 in shipping order.
- **HTTP notes**: the join's notes are stderr only, so the route's JSON lacks them; 3a decides at its refresh whether it needs a key.
- **The shared reader**: `list_existing_tasks` decodes every row of the project, so a corrupt row of a closed or another repository's task adds a note on `get_roadmap` too; a narrow SELECT stays an option.
- **Version skew**: install `spec`, `specengine-mcp`, `specengine-http` from one commit.

## Out of scope

The screen (3a); moves (3b); the roadmap document's daemon key sets and UI types (3a); a queue column or schema step; a gate; dates; `spec task list`'s order.

## Implementation

| Module | What it does |
|---|---|
| core `src/roadmap.rs`, `src/lib.rs` | private `Listing` (open items in listing order) shared by `plan_tasks` (unchanged), `task_item` (an open task's item and group) and `next_order` (listing index, `task_key`, the row; none last) |
| model `src/task.rs`, `src/lib.rs` | `TaskRoadmap {item, group, release, position, of, priority, waits_on}`; `TaskPackage.roadmap` after `bundle` |
| CLI `src/package.rs` | `ReadingPlan {Absent, Unread, Read}`, `reading_plan` (once per command, in `Roots`), `plan_task`, `task_roadmap` (the key; `roadmap: <why>` once), `roadmap_line`; `Roots::config_text` (the config read once), `index_quietly` (stderr unchanged) |
| CLI `src/corpus.rs` | `add_bytes`: the reading root's input gains file bytes, so the plan is `spec roadmap`'s |
| CLI `src/task.rs`, `src/main.rs` | `next_task` by `next_order`, the plan read only when a task is `ready`; the `next` doc and help |
| CLI `src/roadmap.rs`, `src/lib.rs` | `roadmap_with_view(.., View)` (`roadmap` = `Capped`), `uncut`, `roadmap_flags` |
| MCP `src/read.rs`, `src/mirror.rs` | `get_roadmap`, `RoadmapArgs`, its description (1 617 B); `TaskRoadmap`, `RoadmapDocument` (`group` closed, a task's `status` `TaskState`) |
| MCP `src/server.rs`, `src/tasks.rs` | `INSTRUCTIONS` +40 B; `get_task`'s description (1 475 B), the `next` doc |
| HTTP `src/app.rs` | the route after `health`: query and `roadmap_flags` before `run`, `View::Browser`; the 404's list |
| `plugin/` | 0.11.0; `analyze` step 1 names `roadmap`, `get_roadmap` |
| UI `src/api/provisional.ts`, mocks | `TaskPackage.roadmap`, `TaskRoadmap`, `RoadmapGroupKind` |

No new dependency, table or schema step. Deviations: none. Kept (review nit): with the compared place in another worktree, the reading root's index is refreshed twice; a config that cannot be read exits 2 in `bundle_of` before the note, as before.

Tests: nextest core, code, store, model, import 1 355, CLI 1 560, MCP 130, HTTP 121, eval (`build_graph`, `doc_pointers`, `anonymity`, `layout_cli`, `import_cli`, `ui_policy`) 178, all passed; clippy `--all-targets -D warnings`, fmt clean; UI `pnpm test` 1 969, lint, build clean. Mutations 8 of 8 killed, each by its criterion's test; M9 only by the 600-item cut case (small fixtures render alike capped and uncut), M12 on a `roadmap-a` copy. New: CLI `task_next_roadmap.rs` (with `package_roadmap_follows_a_legacy_alias`: a fullwidth-digit alias body under a look-alike legacy prefix, red on iteration 1's index-only plan, `waits_on` `[]` and `position` 2 against the CLI's `["RM-0010"]` and 5), MCP `mcp_roadmap.rs`, HTTP `roadmap.rs`; `fixtures/daemon-keys.json` (`TaskPackage` 26).
