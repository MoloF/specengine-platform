---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-model, crates/specengine-store, crates/specengine-import, crates/specengine-cli, fixtures]
ref: owner requirements 2026-09-30 (roadmap and backlog) and 2026-10-20 (Phase 6 before the rest of Phase 3, right after 8a-2); the requirement-analyst's breakdown of 2026-10-20 at 9c08bd8 (staleness audit, Q-1..Q-7, N1..N13, slices 1a-3b), decided 2026-10-20 under the owner's pre-approval, review open; the 2026-09-30 draft `docs/features/roadmap.md`; 05 s3.3; 07 s1.1, s2; 08 s2 Phase 6
adrs: [ADR-0052]
---

# roadmap-items (Phase 6, 1a): plan items in git and `spec roadmap`

## Why

The owner (2026-09-30): set the goal of the work and trace the whole path — place work in a roadmap, set priority, build the chain, separate MVP, V1, V2, roughly estimate timelines, track where we are, see the pool of work not yet distributed or taken into development. On 2026-10-20: needed now, before the rest of Phase 3. Today the path is prose in 08 s2 ("~N weeks", "shipped" stamps, a hand-summed "Total") that drifts and sits in W's top three; parked work is scattered over six "Open minors" sections, four "Later" sections and 08's Phase 5 "Optional". Nothing knows a release, a priority or an estimate: tasks have no `priority` column (`docs/canon/tasks.md` "Store"), `next` takes the lowest-numbered `ready` task.

1a gives the plan a home and one read: plan items as spec files (ADR-0052), the `[roadmap]` table, three typed front-matter keys, and `spec roadmap`, which computes the path (releases in order, the chain, priority ties, range sums, the current release) and the pool from the repository's files alone. The view (1b `roadmap-view.md`), the queue's progress and `next` (2 `roadmap-position.md`) and the screen (3a `roadmap-screen.md`) build on its core function.

## Description and interactions

- **Owner**: writes items. Here the pipeline's spec-writer writes them and the owner commits, as every document here; in a consumer project an item changes as any spec file does: `spec propose create <dir>/<ID>.md` (the file form, `docs/canon/proposal-kinds.md`) or `update`, then `spec approve`. Runs `spec roadmap` for the path, `--release R` for one lane, `--pool` for the work not on the path.
- **Agent**: reads `spec roadmap --json` from Bash (MCP `get_roadmap` is slice 2); proposes new items with `propose_change` kind `create` at `[roadmap] dir`; release and priority change only through a proposal the owner approves (ADR-0004, ADR-0035).
- **`spec check`** gains no code and no rule: an item is a document. Its findings already cover items: `depends-cycle` (warning, `docs/canon/spec-check-graph.md` "Graph warnings"), `ref-dangling` for a `depends_on` naming nothing, `frontmatter-type` for a mistyped key, `status-invalid`, `id-width`, `id-taken`. `spec roadmap` repeats what it needs as notes and never changes the check's verdict.
- **Other reads**: an item is a node of its `[ids]` kind: `spec show`, `search`, `tree --kind`, `graph` (a `depends_on` edge; 7b's impact follows it inward), `bundle`. A shipped or abandoned item is Tier 3 by front-matter, so those reads leave it out unless `--archive`; `spec roadmap` reads it to count it done.
- **Tasks** are unchanged in 1a: a task executes an item by naming it (`spec task new --nodes`); slice 2 reads that.
- **Unchanged**: the queue and its schema, `spec.lock`, `NORM_RECIPE` and every `norm_hash`, the code index, MCP, HTTP, UI, plugin, the gate, the hook, CI.

## Data

### `[roadmap]` in `specengine.toml`

```toml
[roadmap]
prefix     = "RM"                         # required: an [ids] prefix of number shape, not scope = "feature"
dir        = "docs/roadmap"               # required: where new items are created; the [paths] path rules
releases   = ["mvp", "v1", "v2"]          # required: the path's order; non-empty, unique slugs
mvp_cut    = "mvp"                        # optional: one of releases; "to the cut" sums the releases up to it
priorities = ["must", "should", "could"]  # required: highest first; non-empty, unique slugs
unit       = "days"                       # optional: a label printed after every sum, never converted
```

`RoadmapConfig::from_toml(text, &IdScheme) -> Result<Option<RoadmapConfig>, ProjectError>` (core `roadmap_toml.rs`, on the model of `health_toml.rs`): `None` without the table; every value judged, the first error by line reported as `specengine.toml:<line>: message`. Errors:

- another key: the TOML reader's `unknown field`, listing the six;
- a required key missing; a value of another type (`` `roadmap.<key>`: expected <type>, found <type> ``);
- `prefix` not declared in `[ids]`, of `shape = "name"`, or of `scope = "feature"`;
- `dir` breaking the `[paths]` path rules (`checked_path`);
- `releases` or `priorities` empty; an entry not a slug (`grammar::is_slug`: a lowercase letter, then `a-z`, `0-9`, `-`; at most 64 bytes) or repeated (`` `roadmap.releases`: "v1" is repeated ``);
- `mvp_cut` not one of `releases`; `unit` blank, over 32 bytes, or holding a control character.

`ProjectConfig::from_toml` only name-checks `roadmap` (as `health`): a bad table fails `spec roadmap` alone; no other command reads it in 1a.

### Item front-matter

An item is one file, by convention `<dir>/<ID>.md` (nothing checks the name); its body is its goal; its title the H1, or `title:` where the class admits it.

```yaml
---
class: spec
id: RM-0007
status: draft                 # draft | in-progress | shipped | abandoned (the spec class's)
scope: [crates/specengine-core]
release: v1                   # optional: one of [roadmap] releases; absent: the pool
priority: should              # optional: one of [roadmap] priorities; absent: after every priority
estimate: 2-4                 # optional: N or N-M, decimal integers, N <= M, in [roadmap] unit
links:
  depends_on: [RM-0004]       # optional: the chain, advisory
---

# Roadmap view in git
```

**Typed keys** (core `front_matter.rs` `TYPED_KEYS` 27 -> 30; model `Fields` gains `release`, `priority`, `estimate`, each `Option<String>`): `release`, `priority` `KeyType::Text`; `estimate` a new `KeyType::TextOrInteger`, a string as written or a YAML integer kept as its decimal text (`estimate: 3` -> `"3"`); any other value is `frontmatter-type`, kept in `extra`. All three are **never counted** by `norm_input` (`docs/canon/node-revision.md` "Never counted"): `NORM_RECIPE`, every `norm_hash` and every revision stay as they are; moving an item in the plan is no change of its meaning. Import's copied table (`crates/specengine-import/src/layout/typed.rs`) gains the three and `CoreType::TextOrInteger` (written plain when the text is an integer, else quoted). `INDEX_FORMAT` 12: the stored `fields` of a document holding these keys change (`fixtures/spec-a`'s `priority: high` moves from `extra` to `fields`); one new line `12 <dump hash>` in `crates/specengine-store/tests/format_history.txt`.

### The plan (core `roadmap.rs`, pure)

`plan(input: &CheckInput, config: &RoadmapConfig) -> Plan`: no clock, no I/O, no queue; one input in any order gives one `Plan`.

1. **Items**: every parsed document, live or Tier 3, whose front-matter `id:` has `prefix`; never a `{#ID}` section or a `class: generated` document. An ID two files hold (`id-taken`): the first by path is the item, the other is left out with a note.
2. **Done**: `status` `shipped` or `abandoned`; every other item is **open** (`draft`, `in-progress`, an absent or invalid status, which the check reports).
3. **Groups**: each release of `releases` in config order; then **pool** (no `release`); then **unplaced** (a `release` not in `releases`, or mistyped). Each group counts its done items and lists its open ones.
4. **Edges**: each `links.depends_on` entry that resolves, as `spec check` resolves it (`#section` ignored), to another item. An entry naming no item is ignored here (the check reports a dangling one). An item's `waits_on`: its edge targets that are open, in written order, without repeats.
5. **Order in a release**: the strongly connected components of the group's open items over the edges between them (Tarjan, as `depends-cycle`); a component of two or more items, or of one with a self-loop, is a **cycle**. Components go in topological order, each after every component it depends on; of those ready at a step, the smallest key first. An item's key is (priority rank, ID): the rank is its `priority`'s index in `priorities`, `priorities.len()` when absent or off-scale; IDs compare as strings. A component's key is its smallest member's; a cycle's members follow one another in key order, each `cycle: true`, one note per cycle. `position` is 1-based in the group. Edges to items of other groups never reorder a group; they show in `waits_on`.
6. **Order in pool and unplaced**: by key; `position` null.
7. **Estimates**: `N` -> N..N, `N-M` -> N..M (ASCII digits, each at most 1 000 000, N <= M, no sign, no space). Any other text or a mistyped value: **unestimated**, with a note; absent: unestimated, no note.
8. **Sums** (u64) per group over its open items: `min` = the sum of N, `max` = the sum of M over the estimated ones, `unestimated` = the count of the others; an unknown is never a 0. **cut**: over the releases from the first through `mvp_cut` (null without `mvp_cut`); **total**: over every release; pool and unplaced in neither.
9. **current**: the first release in order holding an open item; null when none does.
10. **Notes**, sorted by text, open items only: `<ID>: release "<r>" is not in [roadmap] releases`; `<ID>: release is not a string`; `<ID>: priority "<p>" is not in [roadmap] priorities`; `<ID>: estimate "<e>" is not N or N-M`; `depends_on cycle: <ID>, <ID>…` (members sorted); `<ID>: outside [roadmap] dir <dir>`; `<ID>: also defined in <path>; <path> is read`.

### `spec roadmap [--release R | --pool] [--json]`

Reads the index the call updates, as `spec tree` does (CLI `corpus::indexed`, the check's input), never the queue; writes nothing under the project root. `--release R`: that release's group; `--pool`: the pool and unplaced groups; neither: every group. The first line and the JSON's top-level keys are always the whole plan's.

```text
roadmap: <o> open, <d> done; current <release or none>[; to <cut> <sum>]; total <sum>
release <name>[ | current][ | mvp cut] | <o> open, <d> done | <sum>
  <position>. <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …][ | cycle] | <path>
pool | <o> open, <d> done | <sum>
  - <ID> | <title or -> | <status or -> | <priority or -> | <estimate or ->[ | waits on <ID>, …] | <path>
unplaced | <o> open, <d> done | <sum>
  - <ID> | … as pool … | release <as written, or mistyped>
[truncated: <k> of <n> items not shown; add --release R or --pool]
```

`<sum>`: `<min>-<max>` (`<min>` when equal), then ` <unit>` when set, then `, <u> unestimated` when u > 0. An estimate prints normalized (`3`, `2-4`); a title through the one-line rule. JSON, every key always present:

```json
{"release": null, "pool": false, "reason": null, "prefix": "RM", "dir": "docs/roadmap", "unit": "days",
 "current": "mvp", "counts": {"open": 12, "done": 6},
 "cut": {"release": "mvp", "min": 4, "max": 9, "unestimated": 1},
 "total": {"min": 7, "max": 14, "unestimated": 1},
 "groups": [
  {"group": "release", "name": "mvp", "current": true, "mvp_cut": true, "open": 3, "done": 5,
   "sum": {"min": 4, "max": 9, "unestimated": 1},
   "items": [{"position": 1, "id": "RM-0012", "title": "Plan items in git", "path": "docs/roadmap/RM-0012.md",
              "status": "in-progress", "release": "mvp", "priority": "must",
              "estimate": {"min": 2, "max": 4}, "waits_on": [], "cycle": false}]},
  {"group": "pool", "name": null, "current": false, "mvp_cut": false, "open": 4, "done": 1,
   "sum": {"min": 2, "max": 3, "unestimated": 3}, "items": []}],
 "notes": [], "truncated": false, "omitted": 0}
```

`estimate` null when unestimated; an item's `release` as written (null when absent or mistyped).

**Cut**: everything before the tail is at most 40 000 characters (`cap::OUTPUT_CAP_CHARS`): lines print whole, in order, until the next would pass the cap; the tail counts the items not shown (`k`) of those the request lists (`n`). JSON holds what the text prints: the groups whose header printed, their printed items, `truncated`, `omitted` = `k`.

**Exits** (the CLI's streams, `crates/specengine-cli/README.md`): 0 printed (notes, cycles and the check's findings never change it); 1 `--release R` naming no release: stderr `spec: no release "R" in [roadmap] releases (<list>)`, with `--json` the document with `reason` set and `groups` empty; 2 cannot run: no `specengine.toml`, a config error, no `[roadmap]` (`specengine.toml has no [roadmap]`), a bad `[roadmap]` (`specengine.toml:<line>: …`), the index not updatable, `--release` with `--pool` (usage).

### This repository (SW-4, with the code)

`[ids] RM = { kind = "plan-item", width = 4 }`; `[roadmap]` as above; `[classes] spec` optional `["ref", "shipped", "adrs", "id", "links", "release", "priority", "estimate"]`; the shard `{ path = "docs/index-roadmap.md", claims = ["docs/roadmap/*.md"] }` on the index entry and in its `writes`. Releases (N11): `mvp` the rest of Phases 0-2, `v1` Phases 3, 4 and 6, `v2` Phase 5.

### Fixtures `roadmap-a`, `roadmap-b`

Each a corpus with its own `specengine.toml` (`[project]`, `[ids]`, `[roadmap]`), at least five `.md` files, `expected.json` (the `spec roadmap --json` value) and `expected.txt` (its stdout); each cell below named by a comment in its `specengine.toml`.

| | roadmap-a | roadmap-b |
|---|---|---|
| prefix, kind | `RM` width 4, `plan-item` | `WORK` width 3, `work-package` |
| `[roadmap]` | `dir = "plan"`; `alpha`, `beta`, `launch`; `mvp_cut = "beta"`; `must`, `should`, `could`; `unit = "days"` | `dir = "work"`; `now`, `next`, `later` (not alphabetical); `p0`, `p1`, `p2`; no `mvp_cut`, no `unit` |
| titles | English | non-Latin |

Both hold: a release whose items are all done (`current` skips it); a chain of three against its priorities; an equal-priority tie (ID order) and an unequal one without edges; a two-item cycle and an item depending on it; a `depends_on` on a done item, on a non-item document and on an item of a later release; an integer, a range, a bad (`4-2`) and an absent estimate; an off-scale priority; an `in-progress` item; a shipped and an abandoned item; two open pool items and a done one; an unknown release; an item outside `dir`.

## Rules and edge cases

- WHEN an item's predecessor is unfinished the system SHALL still list, order and count it; nothing is gated or refused (`docs/canon/architecture.md#control`, ADR-0012).
- WHEN `spec roadmap` runs it SHALL write nothing under the project root and read no queue table; its index update writes only the data directory (`#storage`).
- WHEN the same bytes sit under two roots, or the files were created in another order, the outputs SHALL be byte-identical.
- WHEN an item is unestimated the sums SHALL count it in `unestimated`, never as 0 (Q-4).
- WHEN a release, priority, unit or prefix is needed the core SHALL take it from `[roadmap]` only: core `roadmap.rs`, `roadmap_toml.rs` and CLI `roadmap.rs` name none (ADR-0008).
- WHEN a project has no `[roadmap]`, every other command SHALL behave as before; an item-shaped document is an ordinary node.
- Single user: no assignee, capacity or date (`#distribution`).

## Acceptance criteria

rust-developer (M1-M8: the mutation that must turn the named test red; tests by the test-engineer):

- [ ] RD-1 -- `[roadmap]`; core `tests/roadmap_toml.rs`. Unmutated: one cell per error with its message and line, `release_repeated_is_refused` among them; `no_table_is_none`; `roadmap_table_is_name_checked` (`spec check` and `spec tree` run on `roadmap-a`; a `[roadmapp]` table is an unknown key).
- [ ] RD-2 -- typed keys. **M1** `estimate: 3` read as `frontmatter-type` -> `estimate_integer_reads` (core `tests/front_matter.rs`; there too `estimate: "2-4"` reads, `estimate: [1]`, `release: [v1]`, `priority: {}` are `frontmatter-type`). **M2** `INDEX_FORMAT` kept at 11 -> `roadmap_items_pin_format_12_with_one_new_history_line` (store `tests/format.rs`, replacing `layer_c_pins_format_11_with_one_new_history_line`, whose line stays verbatim). Unmutated: `format_dump` and `the_fixture_dumps_exercise_every_serialised_field` read `roadmap-a` beside `spec-a`, `spec-b` (`SERIALISED` gains `fields.release`, `fields.priority`, `fields.estimate`); `norm_recipe.rs` `te2_the_never_counted_keys_are_the_typed_keys_minus_the_counted_six` lists the three, `never_counted_changes()` changes each, `NORM_RECIPE` unchanged; eval `layout_cli.rs` `import_s_typed_key_table_is_core_s_and_core_reads_by_it` at 30 with the new arm.
- [ ] RD-3 -- order; core `tests/roadmap.rs`. **M3** ties broken by ID alone -> `ties_break_by_priority_then_id`. **M4** a cycle's members dropped or listed twice -> `cycle_members_stay_listed_with_a_note`. Unmutated: `releases_follow_config_order`, `chain_beats_priority`, `cycle_dependent_follows_the_cycle`, `edges_across_groups_never_reorder`, `waits_on_lists_open_predecessors_only`, `off_scale_priority_ranks_last_with_a_note`, `duplicate_id_reads_the_first_by_path`.
- [ ] RD-4 -- sums; core `tests/roadmap.rs`. **M5** an unestimated item counted as 0 -> `unestimated_items_counted_apart`. Unmutated: `done_items_leave_the_sums`, `cut_sums_releases_through_mvp_cut`, `no_cut_without_mvp_cut`, `current_is_the_first_release_with_open_work`, `bad_estimate_is_unestimated_with_a_note`, `estimate_bounds` (1 000 000 reads, 1 000 001 does not).
- [ ] RD-5 -- groups and exits; CLI `tests/roadmap.rs`. Unmutated: `item_without_release_is_pool_only`, `unknown_release_is_unplaced`, `release_flag_lists_one_group`, `pool_flag_lists_pool_and_unplaced`, `release_flag_naming_nothing_exits_1` (text and JSON), `no_roadmap_table_exits_2`, `bad_roadmap_table_exits_2_naming_its_line`, `release_with_pool_is_usage`.
- [ ] RD-6 -- genre. **M6** a hard-coded `"mvp"` -> `roadmap_sources_name_no_vocabulary` (core `tests/roadmap_genre.rs`: no string literal of core `roadmap.rs`, `roadmap_toml.rs` or CLI `roadmap.rs` equals a prefix, kind, release, priority or unit of either fixture or of this repository's `[roadmap]`). Core and CLI `genre.rs` green.
- [ ] RD-7 -- read-only, determinism, cut; CLI `tests/roadmap.rs`. **M7** output following walk or insertion order -> `roadmap_is_byte_identical_across_roots_and_order` (two copies of each fixture, files created in reverse order; text and JSON). **M8** the cut without its tail -> `roadmap_cut_names_its_narrowing` (500 items in one release: at most 40 000 characters before the tail, `k` + shown = 500, JSON `omitted` = `k`; `--release` of another release uncut). Unmutated: `roadmap_writes_nothing_under_the_root` (`git status --porcelain=v1 -z --untracked-files=all` and `HEAD` unchanged).

test-engineer:

- [ ] TE-1 -- the two fixtures, matched by `roadmap_a_matches_expected`, `roadmap_b_matches_expected` (the whole output, then `--release` of each release and `--pool` as its slices).
- [ ] TE-2 -- the churn of a typed `priority`: `fixtures/spec-a/expected.json` loses `unknown-key:9`; http `tests/daemon_keys.rs`'s debt entry `unknown-key` subject `priority` takes `playtest`, keeping a warning in debt; every other suite green (`rg -n 'priority' crates/*/tests fixtures/spec-a` lists the candidates).

spec-writer (bytes measured 2026-10-20; caps: Tier 2 12 288, Tier 1 and index 10 240, Tier 0 16 384, ADR 1 536):

- [x] SW-1 -- ADR-0052 (1 240) with `architecture.md` `#storage`, `#universal` (11 428 -> 11 759); this spec and the three slice drafts; `docs/features/roadmap.md` abandoned as split; 08 s2 Phase 6, s4.1.
- [ ] SW-2 -- at shipping: new Tier 2 `docs/canon/spec-roadmap.md` (the table, the keys, the plan's rules, the command; about 6 KB), its line in `docs/index-canon-spec.md` (3 358), not `docs/index-canon.md` (9 512).
- [ ] SW-3 -- at shipping, a pointer each to `spec-roadmap.md`, trimming first: core README (10 124, 116 B left: `roadmap.rs`, `roadmap_toml.rs`, `TYPED_KEYS` 30), store README (10 067: `INDEX_FORMAT` 12), CLI README (10 038: the command), import README (9 994: the copied table), model README (8 917: `Fields`); `node-revision.md` "Never counted" (12 282, 6 B left: about 30 B trimmed elsewhere in it).
- [ ] SW-4 -- with the code, in its commit (the hook builds the checker from the working tree): the root `specengine.toml` as "This repository"; `docs/README.md` "Generated" five -> six shards (10 185, 55 B left: trim first); `spec-check-graph.md` s11.6's "less its `spec-*` shard" -> "less two of its shards", byte-neutral (12 273, 15 B left).
- [ ] SW-5 -- at shipping: seed the items once (N8): one per unshipped slice of 08 s2 (release, priority, estimate from 08), the four "Later" sections (`anchor-states`, `constant-census`, `lock-levels-verify`, `verify-task`) and 08's Phase 5 "Optional" as pool items; no shipped history (N13). Then 08:41's build order, 05:70 (a task's priority is its item's), 07 s2 (`spec roadmap`; `spec new`, `spec edit` stay unbuilt), `CLAUDE.md` State.

## Risks

- **Mentions**: once `[ids]` has `RM`, an inline `RM-<digits>` in a live document is a mention, dangling unless seeded; these specs write `RM-NNNN` or keep examples in fenced blocks.
- **Version skew**: an older `spec`, `specengine-mcp` or `specengine-http` rejects a config holding `[roadmap]` and needs `INDEX_FORMAT` 12: reinstall all three from one commit.
- **Caps**: SW-3's and SW-4's files have 6-246 B left; detail goes into `spec-roadmap.md`, a cap is never raised.
- **Typed keys in other corpora**: one already using `priority:`, `release:` or `estimate:` loses its `unknown-key` and may gain `frontmatter-type` (a list, a mapping); pilot shapes are unverified (08 s4.1), pilot import stays Phase 5.
- **Overrun**: if iteration 1 overruns, split at the CLI seam: core, model, store, import first; the CLI second.

## Out of scope

The committed view and its drift (1b); the queue, `next`, the package, MCP, HTTP (2); the screen (3a); moves from the screen (3b, parked); dates, velocity, burndown, capacity, assignees (ADR-0017); a gate on the chain (ADR-0012); release goal documents (Q-3); harvesting "Open minors" (N8); back-filling shipped history (N13); `spec new`, `spec edit`; tracker sync, cross-project plans, agent write tools (ADR-0004).

## Decisions

Decided 2026-10-20 under the owner's pre-approval, review open, as the analyst recommended; the other answers and their costs: the analyst's breakdown of 2026-10-20 and `docs/features/roadmap.md` before its split.

| Q | Answer |
|---|---|
| Q-1 | one `spec` file per item; tasks execute it; position computed (ADR-0052) |
| Q-2 | after 8a-2 (the owner) |
| Q-3 | a flat ordered `releases` list and `mvp_cut`; goal documents later, as an optional key |
| Q-4 | `estimate: N` or `N-M` in `unit`, an integer read as N-N; unestimated never 0; no dates |
| Q-5 | advisory: no transition or gate reads the plan |
| Q-6 | the owner (here the pipeline) fills the pool; agents propose `create` at `dir` |
| Q-7 | seed at 1a; at 1b the view replaces 08 s2's list and "Total" in one change |
| N1 | class `spec`; the kind from `[ids]`; items found by `[roadmap] prefix` |
| N2 | `RM`, width 4 |
| N3 | `[roadmap] dir`, here `docs/roadmap/`, live shard `docs/index-roadmap.md` |
| N4 | proposals and `spec approve`; here the pipeline |
| N5 | the view an extra output of the index entry, judged by `index-drift`, `index-missing` (1b) |
| N6 | existing codes plus notes; an unknown release unplaced (refined: an off-scale priority ranks last, a bad estimate is unestimated, both placed with a note) |
| N7 | a task executes the best-placed item among its targets (2) |
| N8 | parked lists not harvested; seeded once |
| N9 | a seventh UI section (3a) |
| N10 | typed `release`, `priority`, `estimate`; `INDEX_FORMAT` 12 (refined: `estimate` `TextOrInteger`; all three never counted) |
| N11 | `mvp`, `v1`, `v2` as "This repository" |
| N12 | before 3b, an ADR letting an apply commit carry SpecEngine-rendered outputs; 3b parked |
| N13 | no back-fill |

## Dependencies and ordering

After 8a-2 (766f836). First of Phase 6 (08 s2): 1a -> 1b `roadmap-view` -> 2 `roadmap-position` -> 3a `roadmap-screen` (on mocks from 1a), then 6b-2. 1b and 2 call `plan` as is.

## Implementation

Not started. Files by role: rust-developer: core `src/roadmap.rs`, `src/roadmap_toml.rs` (new), `src/front_matter.rs`, `src/project_toml.rs`, `src/lib.rs`; model `src/node.rs`; store `src/lib.rs`; import `src/layout/typed.rs`; CLI `src/roadmap.rs` (new), `src/main.rs`, `src/lib.rs`. test-engineer: core `tests/roadmap.rs`, `tests/roadmap_toml.rs`, `tests/roadmap_genre.rs`, `tests/front_matter.rs`, `tests/norm_recipe.rs`; store `tests/format.rs`, `tests/format_history.txt`; eval `tests/layout_cli.rs`; CLI `tests/roadmap.rs`; http `tests/daemon_keys.rs`; `fixtures/roadmap-a`, `fixtures/roadmap-b`, `fixtures/spec-a/expected.json`. spec-writer: SW-1..SW-5. To record: modules, deviations with reasons.
