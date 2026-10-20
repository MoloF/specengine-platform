---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-model, crates/specengine-store, crates/specengine-import, crates/specengine-cli, fixtures]
ref: owner requirements 2026-09-30 (roadmap and backlog) and 2026-10-20 (Phase 6 before the rest of Phase 3, right after 8a-2); the requirement-analyst's breakdown of 2026-10-20 at 9c08bd8 (Q-1..Q-7, N1..N13, slices 1a-3b), decided under the owner's pre-approval, review open; the 2026-09-30 draft `docs/features/roadmap.md`; 05 s3.3; 07 s1.1, s2; 08 s2 Phase 6; developer iterations 1 and 2 (the review's nits), review accepted both
shipped: 2026-10-20
adrs: [ADR-0052]
---

# roadmap-items (Phase 6, 1a): plan items in git and `spec roadmap`

## Why

The owner (2026-09-30): set the goal of the work and trace the whole path — place work in a roadmap, set priority, build the chain, separate MVP, V1, V2, roughly estimate timelines, track where we are, see the pool of work not yet distributed or taken into development. On 2026-10-20: needed now, before the rest of Phase 3. The path was prose in 08 s2 ("~N weeks", "shipped" stamps, a hand-summed "Total") that drifts; parked work was scattered over "Open minors", "Later" sections and 08's Phase 5 "Optional"; tasks had no priority and `next` took the lowest-numbered `ready` task.

Delivered: plan items as spec files (ADR-0052), the `[roadmap]` table, three typed front-matter keys, `INDEX_FORMAT` 12, and `spec roadmap`, which computes the path and the pool from the repository's files alone; how it works now: `docs/canon/spec-roadmap.md`. This repository's open work seeded once, 29 items in `docs/roadmap/`. The view (1b `roadmap-view.md`), the queue's progress and `next` (2 `roadmap-position.md`) and the screen (3a `roadmap-screen.md`) call its `plan` as is.

## Data

### `[roadmap]` in `specengine.toml`

The table, its errors and this repository's values: `docs/canon/spec-roadmap.md` "Config". `RoadmapConfig` carries `scheme` and `paths` beside the six keys (deviation 1, accepted): `CheckInput` holds neither, and `plan(input, config)` must resolve `depends_on` exactly as the check's `Resolver` (`[ids]` scopes, `[paths] features`) while keeping the signature 1b and 2 call.

### Item front-matter

Keys and typing: `spec-roadmap.md` "Items". Decided (owner pre-approved, review open): "every `norm_hash` unchanged" holds for values of the key's own type (strings; an integer `estimate`); a wrongly typed value under `release`, `priority` or `estimate` is `frontmatter-type` and nulls its document's `norm_hash` like any wrongly typed key (`node-revision.md` "Null"), where it used to be an `unknown-key` warning. Carving the three out would change which findings null the hash (`norm.rs`), outside this slice.

### The plan and the command

`spec-roadmap.md` "Plan", "Notes", "Command", "Cut", with the readings below ("Decided").

## Acceptance criteria

rust-developer (Mn: the mutation that must turn the named test red; tests by the test-engineer):

- [x] RD-1 -- `[roadmap]`; core `tests/roadmap_toml.rs`: one cell per error with its message and line, `release_repeated_is_refused`; `no_table_is_none`; `roadmap_table_is_name_checked` (`spec check`, `spec tree` on `roadmap-a`; `[roadmapp]` an unknown key).
- [x] RD-2 -- typed keys. **M1** `estimate: 3` read as `frontmatter-type` -> `estimate_integer_reads` (core `tests/front_matter.rs`, with `estimate: "2-4"` read, `estimate: [1]`, `release: [v1]`, `priority: {}` `frontmatter-type`). **M2** `INDEX_FORMAT` kept at 11 -> `roadmap_items_pin_format_12_with_one_new_history_line` (store `tests/format.rs`). `format_dump` and `the_fixture_dumps_exercise_every_serialised_field` read `roadmap-a`; `norm_recipe.rs` te2 lists the three, `NORM_RECIPE` unchanged; eval `layout_cli.rs` at 30 with the new arm.
- [x] RD-3 -- order; core `tests/roadmap.rs`. **M3** ties by ID alone -> `ties_break_by_priority_then_id`. **M4** a cycle's members dropped or doubled -> `cycle_members_stay_listed_with_a_note`. `releases_follow_config_order`, `chain_beats_priority`, `cycle_dependent_follows_the_cycle`, `edges_across_groups_never_reorder`, `waits_on_lists_open_predecessors_only`, `off_scale_priority_ranks_last_with_a_note`, `duplicate_id_reads_the_first_by_path`.
- [x] RD-4 -- sums. **M5** an unestimated item as 0 -> `unestimated_items_counted_apart`. `done_items_leave_the_sums`, `cut_sums_releases_through_mvp_cut`, `no_cut_without_mvp_cut`, `current_is_the_first_release_with_open_work`, `bad_estimate_is_unestimated_with_a_note`, `estimate_bounds`.
- [x] RD-5 -- groups and exits; CLI `tests/roadmap.rs`: `item_without_release_is_pool_only`, `unknown_release_is_unplaced`, `release_flag_lists_one_group`, `pool_flag_lists_pool_and_unplaced`, `release_flag_naming_nothing_exits_1`, `no_roadmap_table_exits_2`, `bad_roadmap_table_exits_2_naming_its_line`, `release_with_pool_is_usage`.
- [x] RD-6 -- genre. **M6** a hard-coded `"mvp"` -> `roadmap_sources_name_no_vocabulary` (core `tests/roadmap_genre.rs`, both fixtures and this repository's `[roadmap]`).
- [x] RD-7 -- **M7** walk or insertion order -> `roadmap_is_byte_identical_across_roots_and_order` (red on the `expected.txt` comparison: the CLI's input always arrives path-sorted; core `plan_ignores_input_order` covers order). **M8** the cut without its tail -> `roadmap_cut_names_its_narrowing`. `roadmap_writes_nothing_under_the_root`.
- [x] RD-8 -- iteration 2, decided (checks, not counted as mutations): (a) a wrongly typed `priority` gives `<ID>: priority is not a string` -> `mistyped_priority_ranks_last_with_a_note`; (b) the tail and `truncated` only when k > 0 -> `an_empty_group_past_the_cap_is_no_truncation`; (c) a done first file and an open duplicate give the `also defined` note -> `a_done_item_read_with_an_open_duplicate_left_out_has_a_note`; each red when its change is dropped; (d) the four source citations (core `roadmap.rs`, `roadmap_toml.rs`, CLI `roadmap.rs`, store `lib.rs`) cite `spec-roadmap.md` "Plan", "Config", "Command", "Items", `doc_pointers.rs` green.

test-engineer:

- [x] TE-1 -- `fixtures/roadmap-a` (RM width 4, dir `plan`, cut `beta`, `days`; 21 items), `roadmap-b` (WORK width 3, dir `work`, no cut, no unit, Greek and Japanese titles; 22 items): `roadmap_a_matches_expected`, `roadmap_b_matches_expected` (the whole output, each `--release`, `--pool`).
- [x] TE-2 -- churn: `fixtures/spec-a/expected.json` loses `unknown-key:9`; http `daemon_keys.rs`'s debt entry takes `playtest`; CLI `bundle_config.rs` `ac16` pins 12; store `rows.rs`'s fixture oracle green with `roadmap-a`; the root config's ripples: store `parity_config` (5 live shards, 7 outputs), `check_parity`, CLI `shards_repo`, `parity`, the genre lists, core `dogfood` (scheme {ADR, RM}); every other suite green.

spec-writer (bytes before -> after; caps: Tier 2 12 288, Tier 1 and index 10 240, Tier 0 16 384, ADR 1 536):

- [x] SW-1 -- ADR-0052 (1 240), `architecture.md` `#storage`, `#universal` (11 428 -> 11 759); the slice drafts; `docs/features/roadmap.md` abandoned as split; 08 s2 Phase 6, s4.1.
- [x] SW-2 -- new `docs/canon/spec-roadmap.md` (8 903; "View", "Position", "Next", "Surfaces" to come, 3a's "Screen" in its own file if it does not fit), its line in `docs/index-canon-spec.md`.
- [x] SW-3 -- READMEs: core 10 124 -> 10 228, store 10 067 -> 10 118, CLI 10 038 -> 10 130, import 9 994 -> 10 030, model 8 917 -> 9 032; `node-revision.md` "Never counted" 12 282 -> 12 286 (a stale `INDEX_FORMAT` 8 pointer trimmed); `import-layout.md` "YAML"; `symbol-index.md` "Tables" format 12, byte-neutral.
- [x] SW-4 -- root `specengine.toml` (`[ids] RM`, `[roadmap]`, the spec class's keys, the shard); `docs/README.md` "Generated" six shards (10 185 -> 10 198); `spec-check-graph.md` s11.6 "less two of its shards" (12 273 -> 12 272).
- [x] SW-5 -- 29 items seeded (N8; no shipped history, N13): `mvp` 1, `v1` 10, `v2` 7, the pool 11 (the four "Later" sections, Phase 5 "Optional", the parked and conditional slices); 08 s2 :41, Phase 6; 05 :70; 07 s2; `CLAUDE.md` State.

## Risks

- **Mentions**: with `RM` in `[ids]`, an inline `RM-<digits>` in a live document is a mention, dangling unless the item exists; examples stay in fenced blocks.
- **Version skew**: `spec-roadmap.md` "Config".
- **Typed keys in other corpora**: one using `priority:`, `release:` or `estimate:` loses its `unknown-key`; a list, a mapping, or a number or boolean under `release`/`priority` (a float or boolean under `estimate`) becomes `frontmatter-type`, an error that nulls the document's `norm_hash`: a locked document of that shape then cannot be verified, and `rev-not-raised` skips it. No corpus here or in the pilots uses the keys (review, 2026-10-20).

## Decided

Q-1..Q-7, N1..N13: 2026-10-20 under the owner's pre-approval, review open, as the analyst recommended; the alternatives and costs: the analyst's breakdown and `docs/features/roadmap.md` before its split.

| Q | Answer |
|---|---|
| Q-1, N1 | one `spec` file per item, found by `[roadmap] prefix`, the kind from `[ids]`; tasks execute it; position computed (ADR-0052) |
| Q-2 | after 8a-2 (the owner) |
| Q-3 | a flat ordered `releases` list and `mvp_cut`; goal documents later |
| Q-4 | `N` or `N-M` in `unit`, an integer read as N-N; unestimated never 0; no dates |
| Q-5 | advisory: no transition or gate reads the plan |
| Q-6, N4 | the owner (here the pipeline) fills the pool; agents propose `create` at `dir` |
| Q-7 | seed at 1a; at 1b the view replaces 08 s2's list and "Total" |
| N2, N3, N11 | `RM` width 4, `docs/roadmap/`, live shard; `mvp`, `v1`, `v2` |
| N5 | the view an extra output of the index entry (1b) |
| N6 | existing codes plus notes; unknown release unplaced; off-scale priority last, bad estimate unestimated, each with a note |
| N7 | a task executes the best-placed item among its targets (2) |
| N8, N13 | parked lists not harvested; seeded once; no back-fill |
| N9 | a seventh UI section (3a) |
| N10 | typed keys, `estimate` `TextOrInteger`, never counted; `INDEX_FORMAT` 12 |
| N12 | before 3b, an ADR letting an apply commit carry rendered outputs; 3b parked |

Readings of iteration 1, accepted by the review and recorded in canon: pool and unplaced always listed; a self-edge is a one-item cycle; a mistyped release unplaced (`release mistyped`), a mistyped estimate noted as its compact JSON; `007` reads 7, `3-3` prints `3`, the unit verbatim; `n` counts every listed item. The review's nits, decided for iteration 2 (RD-8): a mistyped priority noted; no tail when k = 0; an open duplicate noted.

## Out of scope

The committed view (1b); the queue, `next`, MCP, HTTP (2); the screen (3a); moves (3b, parked); dates, velocity, capacity, assignees (ADR-0017); a gate on the chain (ADR-0012); release goal documents; `spec new`, `spec edit`; tracker sync, cross-project plans, agent write tools (ADR-0004).

## Implementation

| Module | What it does |
|---|---|
| core `src/roadmap_toml.rs` (new) | `RoadmapConfig` (the six, `scheme`, `paths`), `from_toml`, `roadmap_from_toml`, `MAX_UNIT_BYTES`; every value judged, the first error by line |
| core `src/roadmap.rs` (new) | `plan` and its types (`Plan`, `PlanGroup`, `PlanItem`, `Sum`, `CutSum`, `Estimate`, `GroupKind`), `parse_estimate`, `ESTIMATE_MAX`: items, edges via `Resolver`, Tarjan order, sums, notes |
| core `src/front_matter.rs` | `KeyType::TextOrInteger`, `text_or_integer`, three reader arms, `TYPED_KEYS` 30 |
| core `src/project_toml.rs`, `src/lib.rs` | `roadmap` name-checked; modules, re-exports |
| core `src/check/mod.rs` | `pub(crate) use text::FileText`, outside the planned list: the plan rebuilds a reference's written form as the check does |
| model `src/node.rs` | `Fields.release`, `priority`, `estimate` (`Option<String>`) |
| import `src/layout/typed.rs` | `Release`, `Priority` (`Text`), `Estimate` (`CoreType::TextOrInteger`): a canonical integer plain, else quoted |
| store `src/lib.rs` | `INDEX_FORMAT` 12 |
| CLI `src/roadmap.rs` (new) | `roadmap`, `RoadmapRequest`, `RoadmapOutcome` (`requested`, `omitted`): text, JSON, the cut, notes on stderr |
| CLI `src/main.rs`, `src/lib.rs` | `Roadmap {--release (conflicts with --pool), --pool}`; `Outcome::Roadmap` (exit 1 with `reason`) |
| tests, fixtures | core `tests/roadmap{,_toml,_genre}.rs` (new), `front_matter.rs`, `norm_recipe.rs`; store `format.rs`, `format_history.txt`; eval `layout_cli.rs`; CLI `roadmap.rs` (new), `bundle_config.rs`, `shards_repo.rs`, `parity.rs`, `genre.rs`; store `parity_config`, `check_parity`; core `dogfood.rs`, `check_genre.rs`; http `daemon_keys.rs`; `fixtures/roadmap-{a,b}` (new), `spec-a/expected.json` |

No new dependency (`petgraph`, `serde_json`, `toml` already in core). Deviations: `RoadmapConfig`'s `scheme` and `paths` and the `FileText` export ("Data"); the readings and iteration 2 ("Decided"); after the tests a micro pass rewrapped two citations and a doc paragraph (comments only).

Tests: nextest core, code, CLI, MCP, HTTP, store, model, import 3 107 run, green; eval `build_graph`, `doc_pointers`, `anonymity`, `layout_cli`, `import_cli` 150 of 152 (the two fixture-status guards turn green once the fixtures are committed); clippy, fmt clean; `check` over the root config 285 documents, 0 errors, 0 warnings, clean; `export index` all seven outputs unchanged. Mutations 8 of 8 killed by their named tests; RD-8's three checks red without their change. The fixtures carry `expected.json` and `expected.txt`.
