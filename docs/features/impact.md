---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, fixtures]
ref: Phase 3 slice 7b-1 of RM-0007 (split 2026-10-21; 7b-2 is `docs/features/impact-since.md`); Q-A..Q-J and the review's points, owner pre-approved, review open; 07 s1.2, s2; 08 s2; 2 iterations
shipped: 2026-10-21
adrs: [ADR-0046]
---

# impact (7b-1): the blast radius of a node or a code symbol

## Why

Before a change an agent had no single call for what it reaches: `spec graph --impact` (nodes only, no MCP twin), `show --bindings` per node, then the tasks. Now `spec impact REF | --symbol QPATH` and MCP `get_impact` give the nodes the impact walk reaches, each bound symbol with its `sync`, the tests to run and this repository's open tasks on them; the `implement` skill calls it per target. Nothing shown changes an exit, status, claim or verdict. No ADR (ADR-0046); 7b-2 adds `--since` and `delta.symbols`.

## Description and interactions

Canon: `docs/canon/spec-impact.md`; pointers in `spec-cli-graph.md`, `code-reads.md`, `symbol-index.md`, `mcp-read.md`, `plugin.md`, `plugin-files.md`. Unchanged: `spec graph`, `show`, `bundle`, `verify`, `check`, gate, CI, `specengine-http`, `ui/`, both index formats, the queue schema, `spec roadmap`'s output; no crate or dependency added (MCP's stay 7).

## Data

### Command

`spec impact [REF] [--symbol QPATH] [--depth N] [--json]`. Help, verbatim:

- the command: "Show what a change to a node or a code symbol reaches: nodes, bound code with its sync, tests, open tasks; read-only."
- REF: "Any form `show` takes."; `--symbol`: "Looked up as `spec symbols QUERY`."; `--depth` graph's: "Distance from REF at most; default unbounded."

Usage, before discovery: `spec: impact takes exactly one of REF and --symbol`; no match: ``--symbol `<QPATH>` names no symbol``. Seeds, walk: `spec-impact.md` "Command", "Radius".

### Rows and tasks

Rows: one `set_bindings` over the reached set and the nodes within it. `code` is `SetBindings.code`, `null` without `[code]` or under causes (1), (3), (8); `--symbol` falls back to the lookup walk's. Tasks: `open_tasks(.., GitPlace::Unnamed)`. Under that place no note carries a path, and the root is never string-replaced:

- the data directory or the queue cannot be read (`db_path`, `open_existing`, `list_existing_tasks` fail): `tasks not shown: the queue cannot be read (spec roadmap names the cause)`;
- git does not run or fails in the root: `tasks not shown: git in the project root failed (spec roadmap names the cause)`;
- `spec roadmap` (`GitPlace::Root`) keeps its detailed texts.

### Output

#### Text

`spec-impact.md` "Output", in its order.

#### JSON

Keys, in order, as `spec-impact.md` "Output". `truncated` `null` uncut, else `{nodes, symbols, tests, tasks, seeds, delta}`; `symbols`, `tests` `null` for a `null` list, `seeds` `null` for REF, `delta` `null`.

### Cut

`spec-impact.md` "Cut": header and summary always, items whole within 40 000 characters, the JSON the printed items.

### Exits and degrade

`spec-impact.md` "Exits and degrade": 0 answered, 1 names nothing, 2 cannot run; the REF form degrades as 7a's reads.

### MCP `get_impact`

`{id?, symbol?, depth?}`, its field docs verbatim in `read.rs`. Form error: ``spec: get_impact takes exactly one of `id` and `symbol` `` + `\n`. Mirror `ImpactDocument`: `truncated` nullable (`null` uncut), its `symbols`, `tests`, `seeds` nullable. **Description** `IMPACT_DESCRIPTION`, 1 898 B, verbatim in `read.rs`, pinned by `mcp_impact.rs`.

### INSTRUCTIONS

Q-B: `PROBE_INSTRUCTIONS` removed; both builds send `INSTRUCTIONS` alone. The reads line ends `get_roadmap(release?, pool?) = roadmap; get_impact(id|symbol, depth?) = impact.` 1 977 -> 2 017 B (31 left), 1 879 without the tasks line.

### Plugin 0.15.0

MINOR (a new tool): `plugin.json`, `PINS` `a13a791146c0534242c528604fa0de8fbc3e93a1d5483f9c2c5f507cd1efcf44`, `SCHEMA_PINS` `70ff3720d4b3b7b13c62093d80a4c88cae2094e4580bd1e1d3003a41ed083581`; `TOOLS` 21. Only `implement` names `get_impact`, in step 1 (body 2 976 -> 3 284 of 4 096 B; verbatim in its `SKILL.md`, pinned by `plugin_skills.rs`, `plugin_files.rs`).

## Acceptance criteria

M: a counted mutation and the test it turns red; 9 of 9 killed. CLI `tests/impact.rs`, MCP `tests/mcp_impact.rs` unless named.

- [x] RD-1 -- `impact_nodes_equal_graph_impact` (`fixtures/impact/`, `spec-a`; `--depth` 0, 1, none): `nodes`, `left_out` graph's. M1: the shared walk's `depends_on` turned outgoing.
- [x] RD-2 -- `impact_takes_exactly_one_input` (CLI exit 2; MCP the error result before discovery); `get_impact_arguments_are_the_flag_names`. M2: both inputs accepted.
- [x] RD-3 -- `impact_rows_are_the_reached_sets_rows`: `show --bindings`' rows of the reached set, once, verify's order, split by relation. M3: within reduced to the node only.
- [x] RD-4 -- `symbol_impact_reaches_the_enclosing_binding`, `mentions_never_seed_impact`, `symbol_naming_no_symbol_exits_1`. M4: only the marker's own item seeds. M5: mentions seed.
- [x] RD-5 -- `impact_ref_never_fails_on_code`, a leg per cause: no `[code]` (rows, `code` `null`, no `bindings:` note), a bad `[code]`, a walk gap, an unmerged lock (`sync -`); `nodes`, `tasks`, exit unchanged; `symbol_impact_needs_code`. M6: the REF form exits 2 on a walk gap.
- [x] RD-6 -- `impact_lists_open_tasks_only`, `impact_tasks_are_this_repositorys`, `impact_task_notes_hold_no_root` (an unreadable queue, failing git: the fixed notes, no path). M7: `done` tasks kept. M9: the store error back under the unnamed place.
- [x] RD-7 -- `impact_cut_names_narrowing`, a 1 000-match name tier cutting `seeds.symbols`. M8: the JSON's nodes from all, not the printed.
- [x] RD-8 (not counted) -- `get_impact_equals_the_cli` per exit, `get_impact_mirror_is_impact_json`, `get_impact_documents_meet_every_key`; `INSTRUCTIONS` 2 017 B; `build_graph.rs` 7.
- [x] TE-1 -- `fixture_matches_the_shipped_reads`: `fixtures/impact/` (`expected.json`), every impact type, seed place and `TaskStatus`, another repository's task.
- [x] TE-2 -- `impact_rows_equal_show_bindings_on_every_corpus` (`code-a`, `anchor-states`, `verify-tests`, `verify-ecs`).
- [x] TE-3 -- `impact_is_byte_identical_across_roots_order_and_crlf`, `impact_json_holds_every_key`, `impact_text_prints_its_json`.
- [x] TE-4 -- `impact_cost.rs` (`#[ignore]`, release, warm, median of five): a 10 000-node radius `impact HUB-CORE` 584 ms (`--json` 585), `graph --impact` 540; a 1 000-match name tier `--symbol` 582; `code-a` plus 10 000 files `impact GOAL-01` 771, `--symbol step` 703, `impact PREM-001` (5 000 bound items) 468, `show GOAL-01 --bindings` 259, `graph GOAL-01 --impact` 6.
- [x] TE-5 -- `impact_on_another_genre_reads_no_code` (`spec-c`); `mcp_genre`.
- [x] TE-6 -- `mcp_impact.rs`, `mcp_door`, `mcp_size` (`get_impact` maximum 100 621 characters, `{id}` 100 485); the schema conforms on an uncut REF, a no-`[code]` and a cut `--symbol` document; `TOOLS` 21; `plugin_skills.rs` (`impact_implement_names_get_impact`), `plugin_files.rs` 0.15.0.
- [x] TE-7 -- `impact_writes_nothing_under_the_root`: status, `.git/index` mtime, objects, queue rows unchanged.
- [ ] TE-7 pilots -- the owner's: `impact_pilot` (`#[ignore]`, release, scratch `HOME`), latency per form on both pilots.
- [x] SW-1, SW-2 -- canon `spec-impact.md` 9 005 B, `code-reads.md` 12 263; `mcp-read.md` 12 266, `symbol-index.md` 12 286, `spec-cli-graph.md` 12 281; READMEs CLI, store 10 239, MCP 10 224.

## Out of scope

`--since`, `delta.symbols`, `get_impact.since` (7b-2); `renamed`; documents as seeds; a call-graph radius; `--archive`, `--type`; an HTTP route, the graph's impact mode (RM-0011); `--format`; other repositories' tasks listed; layer C monikers.

## Implementation

| Module | What it does |
|---|---|
| CLI `impact.rs` (new) | `impact(&Env, &Globals, &ImpactRequest {reference, symbol, depth, git}) -> ImpactOutcome` (`SeedSymbol`, `ImpactRow {row, binding}`, `ImpactTask`, `truncated()`), `render_text`, `Serialize`; seeds, `radius`, the cut |
| CLI `graph.rs` | `Reach {walk, left_out}`, `reach` (the one walk of `graph_with_view` and `impact`), `impact_reach`; `node_of`, `node_line`, `NodeJson::of` crate-visible |
| CLI `roadmap.rs` | `open_tasks(.., GitPlace) -> OpenTasks {here, others, notes}`, `GitPlace {Root, Unnamed}`, `others_note(n, slug, command)`; `spec roadmap` on them, unchanged |
| CLI `bindings.rs` | `SetBindings.code`; `SetJudged.walk: SetWalk {files, skipped}`, `set_files` carrying the retry refresh's `skipped` |
| CLI `lib.rs`, `main.rs` | `Outcome::Impact`, exports; the subcommand and its help |
| store `code.rs`, `symbols.rs` | `SnapshotSymbols::LookupWithFiles {query, name}`: the hits and every symbol of each best-tier hit's file, one transaction; `files_symbols_in` prepares the per-file statement once per snapshot (`file_symbols_in` delegates to it) |
| MCP `read.rs`, `mirror.rs`, `server.rs` | `get_impact`, `ImpactArgs`, `IMPACT_DESCRIPTION`, the form check; `ImpactDocument` (`Impact{Input,Seeds,SeedSymbol,Node,Task,Truncated}`); `PROBE_INSTRUCTIONS` removed |
| `plugin/` | 0.15.0; `implement` step 1 |

`node_set.rs` unchanged: `named_set` serves the tasks as is.

Divergences (owner-approved):

- Under `GitPlace::Unnamed` git and queue failures are fixed sentences: the draft's root replaced by `.` leaked paths (a subdirectory root, a worktree's common dir, the git config, the data directory) and mangled names.
- RD-5's no-`[code]` leg means no `bindings:` note: its fixture's other-repository task note is right.
- The review's `prepare_cached` became `files_symbols_in`: rusqlite's `cache` feature is not enabled, so no dependency changes.
- Known limits: `LookupWithFiles` reads each hit file whole; the REF form costs about 3x `show --bindings` on `code-a` (771 against 259 ms): it judges the whole radius.

`mcp_size`'s `get_impact` maximum: 100 621 characters (text 40 091, `structuredContent` 60 530; `--symbol` over a 3 000-row node; `{id}` 100 485), about a fifth of the 500 000 cap. Tests: nextest workspace 3 705 run, 55 skipped; eval `import_cli`, `layout_cli` pass once `fixtures/impact/` is committed.
