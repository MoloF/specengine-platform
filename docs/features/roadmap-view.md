---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-store, fixtures]
ref: Phase 6 slice 1b of `roadmap-items.md` (its Q-7, N5); the requirement-analyst's breakdown of 2026-10-20 and its refresh against 1a as built (725f338, Q-A..Q-E), decided under the owner's pre-approval, review open; 08 s2; developer iterations 1 and 2, the review accepted both
shipped: 2026-10-20
adrs: [ADR-0052]
---

# roadmap-view (Phase 6, 1b): the committed, checked roadmap view

## Why

After 1a the path was computed, but 08 s2 still kept it by hand ("~N weeks", a summed "Total") and drifted. A generated view committed beside the documents makes the plan readable in any diff and on GitHub, and the check fails a hand edit as it fails a hand-edited index. 08 s2's open-slice lists and "Total" became a pointer to it (Q-7, Q-C).

Delivered: the index entry's optional `roadmap = "<path>"`, one more output rendered from git alone, written by `spec export index`, compared by `spec check`; here `docs/generated/roadmap.md`. No ADR: it applies ADR-0052, ADR-0022 (§7, §11 items 5-6), ADR-0030, ADR-0013, ADR-0012, ADR-0039, ADR-0008. How it works now: `docs/canon/spec-roadmap.md` "View", "Config"; `spec-check-graph.md` "API", §11.6; `spec-check-cli.md` "spec export index"; `proposal-kinds.md` "Propose" step 2; `docs/README.md` "Generated".

## Decided

2026-10-20 under the owner's pre-approval, review open.

| | Answer |
|---|---|
| Q-A | the view is `spec roadmap`'s lines as Markdown, the builders moved into core: one renderer, links on GitHub, no fence a title can break |
| Q-B | with a view registered, an absent or bad `[roadmap]` is a config error of the check tables; without the key a bad table fails `spec roadmap` alone |
| Q-C | 08 s2 lost "Total", the phase headings' "~N weeks", the open-slice lists and the build order to one pointer (the view, `spec roadmap`); the MVP line, the shipped history and the phase descriptions stay |
| Q-D | RM-0002 `shipped` in the shipping change, with the regenerated view |
| Q-E | export JSON: a top-level `roadmap` key, omitted without a view |
| iteration 1 | `RoadmapView` carries `line` (the key's); a bad `[ids]` or `[paths]` with a view registered is passed through with its own line and message, shown once; the view's notes go through the one-line rule; `spec propose create` refuses the view's path at propose and at apply (CLI `create.rs` `index_files`, a fourth refusal kind) |

Without a `roadmap` key the outputs and findings are 1a's; only the `writes` error's text now names the view (`` …is neither the `[paths] index` <root>, a shard nor the `roadmap` view… ``), the review of iteration 2.

## Acceptance criteria

rust-developer implements, test-engineer writes the tests; Mn is the mutation that must turn the named test red.

- [x] RD-1 -- registry; core `tests/check_config.rs`. **M1** the key accepted on an entry without `index = true` -> `roadmap_output_only_on_the_index_entry`. One cell per config error with its line and message; a valid key reads `RoadmapView` with its path, config and line; `bad_roadmap_without_a_view_fails_no_check`.
- [x] RD-2 -- comparison; core `tests/roadmap_view.rs`. **M2** the view skipped by the comparison -> `hand_edited_view_is_index_drift` (on the edited line). **M3** a missing view not reported -> `missing_view_is_index_missing`. A walk gap compares nothing; bytes not supplied are a cause; the view never in the index, broken front-matter too; reversed input, identical bytes.
- [x] RD-3 -- git only; CLI `tests/roadmap_view.rs`. **M4** the view computed through `spec roadmap`'s path (`corpus::indexed`, slug, data directory) -> `export_writes_the_view_without_a_data_directory`. `staged_view_follows_the_staged_items_and_config`; `view_ignores_the_queue`.
- [x] RD-4 -- render. **M5** links written root-relative -> `view_links_resolve_from_its_directory`. `view_lines_are_the_roadmap_lines` (both fixtures); `roadmap_{a,b}_matches_expected` green unchanged.
- [x] RD-5 -- export; CLI `tests/export.rs`: the view written last; `unchanged` when equal; `--stdout` with a view uses `==>` without shards; JSON `roadmap`, absent without a view; a missing directory: exit 2, nothing written.
- [x] RD-6 -- genre; core `tests/check_genre.rs` scans `src/roadmap.rs`. **M6** a hard-coded `"docs/generated/roadmap.md"` -> `the_check_source_names_no_prefix_path_or_file_of_a_project`.
- [x] TE-1 -- `fixtures/roadmap-{a,b}/expected-view.md`, each equal to what `export index` writes on a scratch copy with an `index = true` entry and `roadmap` in a subdirectory, byte-identical across two roots; `check` on the copy shows no `index-drift` or `index-missing`.
- [x] TE-2 -- churn: `roadmap: None` in the `Generator` literals; with the root config: store `parity_config` `INDEX_OUTPUTS` 8, the view last; store `check_parity`; CLI `parity`, `shards_repo`; CLI `proposal_kinds` n4's fourth row (the view); all suites green.

spec-writer (bytes after / cap):

- [x] SW-1 -- `docs/canon/spec-roadmap.md` 10 136 / 12 288: "View" (1 101 B), "Config" (Q-B, the view's path, version skew); 2 152 B left for slice 2.
- [x] SW-2 -- `docs/canon/spec-check-graph.md` 12 286 / 12 288: "API" (`render_index_set`'s order, `Generator.roadmap`), §11.6 (the rules by pointer, the writes rule), trimmed: two shipped "Next" items, "Open nits" wording.
- [x] SW-3 -- `docs/canon/spec-check-cli.md` 8 496: the outputs, no `HOME`/slug/queue, JSON `roadmap`, `--stdout` with exactly one output.
- [x] SW-4 -- `docs/README.md` 10 238 / 10 240: the generated row, "Generated"; trimmed "Classes", "Budgets", "Generated".
- [x] SW-5 -- `proposal-kinds.md` step 2 (9 999); CLI README 10 148 / 10 240; `07-interfaces.md` the `export index` line; `CLAUDE.md` 6 271 / 16 384 (State, "Documentation", "Layout"); 08 s2 (Q-C); RM-0002 shipped; `docs/generated/` created; the root `specengine.toml` key and `writes`.

## Risks

- **Version skew**: a 1a `spec` refuses the root config holding the key (`deny_unknown_fields`) in `check`, `export index` and approve's checks: install `spec`, `specengine-mcp`, `specengine-http` from one commit.
- **N12 wider**: a consumer project's item update changing release, priority, estimate, `depends_on`, title or status meets the view's `index-drift` under an `enforce` hook, as a `create` meets the index's; the N12 ADR is due before 3b.
- **Noise**: `spec review` lists the view's `index-drift` among an item proposal's introduced findings (never refusing); raw titles may render oddly on GitHub, as in the index.

## Out of scope

Task progress in the view (2); a second generator entry or `spec export roadmap`; new check codes (`CHECK_CODES` stays 36); dates; escaping titles; moving §11.6; the N12 ADR (3b).

## Implementation

| Module | What it does |
|---|---|
| core `src/check/config.rs` | `RawGenerator.roadmap`, `RoadmapView {path, config, line}`, `Generator.roadmap`, `roadmap_view_from` (the key's rules, `[ids]` via `scheme_from_toml`, `roadmap_from_toml`); the index entry's `writes` rule admits the view |
| core `src/check/render.rs` | `render_index_set` appends the view last; `is_output` excludes it; `header(generator, title, source)`, `relative_link` `pub(crate)` |
| core `src/check/generated.rs` | `Output {Root, Shard, View}`: the view's `index-drift`, `index-missing` messages |
| core `src/check/mod.rs` | re-exports `RoadmapView`; `generated_header`, `relative_link` crate-internal |
| core `src/roadmap.rs` | the line builders moved in: `Sum::text`, `Plan::summary`, `PlanGroup::header`, `PlanItem::line`, crate-internal `PlanItem::entry`, `render_view`; `one_line`, `or_dash` |
| core `src/roadmap_toml.rs`, `project_toml.rs`, `lib.rs` | doc comments |
| CLI `src/export.rs` | `Role::View`, the outside-the-walk warning, `ExportOutcome.roadmap`, the text line, `--stdout` blocks past one output, JSON `roadmap` |
| CLI `src/roadmap.rs` | calls the core builders; output byte for byte |
| CLI `src/create.rs` | `index_files` lists the view: refused at propose and at apply step 3 (`the index's roadmap view`) |
| CLI `src/main.rs` | `export index`'s help names the three outputs |
| tests, fixtures | core `tests/roadmap_view.rs` (new), `check_config.rs`, `check_genre.rs`; CLI `tests/roadmap_view.rs` (new), `export.rs`, `parity.rs`, `shards_repo.rs`, `proposal_kinds.rs`; store `tests/check_parity.rs`, `parity_config/mod.rs`; `fixtures/roadmap-{a,b}/expected-view.md` (new) |

No new dependency; `CHECK_CODES` 36, `INDEX_FORMAT` 12 unchanged. Deviations: none left; iteration 1's readings are "Decided", iteration 2 aligned the config messages with this spec's table and `export index`'s help.

Tests (the tester, on the final tree before the view existed): nextest core, code, model, import 1 102 of 1 102; store, MCP, HTTP 486 of 491 and CLI 1 535 of 1 539, the 9 reds the parity pins awaiting the shipping change, re-run in the final verification; eval 150 of 152 (the two fixture-status guards, green once committed); clippy `--all-targets`, fmt clean. New: core `roadmap_view.rs` (8), CLI `roadmap_view.rs` (7), CLI `export.rs` +5, `check_config.rs` a cell per config error and Q-B (8 bad tables), `proposal_kinds.rs` n4's view row, the two `expected-view.md` (scratch copies, fixture configs unchanged). Mutations 6 of 6 killed by their named tests (M5 in core `roadmap_view.rs`); iteration 1's message wording was caught by `invalid_views_fail_with_file_line_message`. `view_ignores_the_queue` guards slice 2. Shipping: `export index` wrote `docs/generated/roadmap.md` (3 952 B), every index output unchanged; `check` 286 documents, clean.
