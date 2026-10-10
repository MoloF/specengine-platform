---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin]
ref: Phase 3 plan 2026-10-10, slice 2 (WA-1..WA-15, RD/TE/UI/SW criteria; review points decided under the owner's pre-approval, review open); 05 s3.3, s3.5; 08 s2 Phase 3; Rust 2 iterations, UI 1, review accepted
shipped: 2026-10-10
adrs: [ADR-0040]
---

# norm-hash: a node's meaning hash and revision

## Why

Slice 3 (`spec-lock`) writes each binding's `spec_hash` = the node's `norm_hash` into `spec.lock` (05 s3.5), slice 4 compares it, and a marker's `@N` (slice 1, `code-refs`) meets the node's revision. Neither contract existed: 05 s3.5 named what goes into the hash, not its bytes; a section's revision was the core README's open Q3, and the parser accepted `rev=0` / `rev: 0` and took the first of two `rev=`. Once a lock is committed the recipe cannot change without re-verifying every binding, so both were fixed before anything writes them. The slice writes no spec, lock or file under a project root and blocks nothing (ADR-0012); ADR-0040 records the revision rule (`docs/canon/architecture.md#layout`).

## Description and interactions

Canon: `docs/canon/node-revision.md` ("Revision", "The recipe", "Null", "Where it appears", "Known limits"); the core, model, store and CLI READMEs; `mcp-read.md` (`get_node`), `plugin-files.md` (0.7.0), `spec-cli-bundle.md` (targets). Unchanged: `spec check` (no new code; `bad-rev` also covers `0` and a repeat), `bundle` text and `bundle_hash`, `tree`, `search`, the parse JSON, the reference grammar, `fixtures/daemon-keys.json`.

## Data

### Revision

`docs/canon/node-revision.md` "Revision" (WA-1..WA-3).

### The recipe

`docs/canon/node-revision.md` "The recipe", "Null" (WA-4..WA-8, WA-13): `specengine-norm/v1`, frozen; deviations 1 and 2 below are part of it.

### Index

`nodes.norm_hash` after `own_text`, `INDEX_FORMAT` 7 -> 8, `format_history.txt` `8 91ccc1a3…`; a 7-stamped index rebuilds, the queue kept (WA-10; `crates/specengine-store/README.md`).

### spec show and mirrors

` | norm b3:<hex>` right before ` | span`, JSON `norm_hash` between `sections` and `span_hash`; MCP mirror, HTTP pass-through, UI `ShownNode.norm_hash: string | null`; plugin 0.7.0 (WA-11, WA-12; `docs/canon/node-revision.md` "Where it appears").

## Rules and edge cases

`docs/canon/node-revision.md`: the same bytes under the same `[ids]` give the same hash from any root, cwd or order (WA-14); text moved from a section into its parent changes both, a nested edit leaves the parent; a `norm_hash` sent as a proposal's `base` meets the existing stale-base refusal; linear cost; project keys never counted in v1.

## Acceptance criteria

Core `tests/norm_recipe.rs` (17), `node_revision.rs` (12); store `norm_index.rs` (3), `format.rs` +2; CLI `norm_show.rs` (7); MCP `mcp_read.rs` +1; http `norm.rs` (1); eval `norm_pilot.rs` (2, `#[ignore]`). Every named mutation red. Workspace 2306 passed, 22 skipped; Vitest 1715; fmt, clippy, `spec check` clean.

rust-developer:

- [x] RD-1 -- core `norm.rs`; no Cargo change; `build_graph.rs` `model_and_core_sources_do_no_file_io` green, red under a `std::fs` call.
- [x] RD-2 -- `revision.rs`, `section_rev`, front-matter `rev`: `rev_zero_is_bad_rev`, `repeated_rev_is_bad_rev`.
- [x] RD-3 -- the column and format 8; `show` text and JSON; `mirror.rs`; `plugin.json` 0.7.0 + `PINS`.
- [x] RD-4 -- `DiagnosticCode::ALL` 13, `CHECK_CODES` 35 (the existing `check_output` table); nothing written under a root; `spec check` 0 warnings.

test-engineer:

- [x] TE-1 -- `te1_golden_inputs_of_spec_a_and_spec_b` (five nodes), CLI `te1_golden_hashes_are_cores_stores_and_shows`, store `the_golden_hashes_are_stored`; M final LF, links unsorted, big-endian.
- [x] TE-2 -- `te2_invariance_rows_each_with_its_mutation` (65 rows), `te2_the_never_counted_keys_are_the_typed_keys_minus_the_counted_six`, `te2_a_renamed_h1_of_any_form_keeps_the_input`, `te2_the_h1_cut_joins_the_text_around_it_with_an_empty_line`, `te2_a_lone_cr_inside_text_is_lf` (CRs inside text only), `te2_nested_sections_never_reach_the_parent`, `te2_an_ids_kind_or_width_change_keeps_every_input`.
- [x] TE-3 -- `te3_sensitivity`.
- [x] TE-4 -- `te4_the_five_codes_null_the_document_and_only_it`, `te4_other_findings_null_nothing`, `te4_not_utf8_and_foreign_bytes_are_null`, CLI `te4_not_utf8_and_a_broken_front_matter_print_no_norm`; M text alone on a broken front-matter.
- [x] TE-5 -- `node_revision.rs`, `r1_at_0_parses`, `bad_rev_stays_a_warning`; M 0 accepted (both forms), first `rev` wins, unreadable as 1, a section inheriting, `@0` made bad.
- [x] TE-6 -- `norm_index.rs`; `norm_hash_pins_format_8_with_one_new_history_line`, `a_seven_stamped_database_rebuilds_with_its_queue_intact`; M stamp kept at 7, a queue table dropped.
- [x] TE-7 -- `te7_exact_headers_norm_right_before_span`, `te7_json_keys_in_order`, `te7_show_store_and_core_agree_over_every_id`, `te7_bundle_text_and_hash_are_unchanged` (44 bundles of spec-a, -b, -c against the pre-change build); M `norm` in `header()`.
- [x] TE-8 -- `te8_get_node_norm_hash_parity_and_schema`, http `te8_nodes_carry_norm_hash_as_spec_show_does`; `fixtures/daemon-keys.json` unchanged; `PINS` 0.7.0; M the key at NodeView's top.
- [x] TE-9 -- `te9_the_path_never_enters_the_input`, CLI `te9_two_roots_another_cwd_reverse_order_one_hash`.
- [x] TE-10 -- `te10_spec_c_is_hashed_like_any_corpus`, `te10_the_recipe_source_holds_no_domain_literal`, `te10_the_scan_sees_what_it_must`; M a `"mechanic"` literal.
- [x] TE-11 -- `te11_fifty_thousand_sections_within_the_own_text_bound` (~150 ms), `te11_twice_the_sections_at_most_three_times_the_time` (2.04x); M a quadratic join, whole-file UTF-8 per node.
- [x] TE-12 -- `norm_pilot.rs`, run in debug, in process (not release, not the binary): figures below.

ui-developer:

- [x] UI-1 -- `provisional.ts`, `provisional.test.ts` (KEYS, CITED, source order), `mocks/corpus.ts`, `test/builders.ts`, `corpus.test.ts` +4; M the key missing, after `span_hash`.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- ADR-0040 and its `architecture.md#layout` diff.
- [x] SW-3 -- `docs/canon/node-revision.md` (new); core, model, store, CLI READMEs; `spec-cli-bundle.md`, `mcp-read.md`, `plugin-files.md`, `spec-check-links.md` (Q-I); 05 s3.3, s3.5; 08 s2, AC-13; `CLAUDE.md`.
- [x] SW-4 -- `export index`, `check` clean.

## Out of scope

`spec.lock`, `spec lock`, `spec bump`, `--editorial`, 08 AC-13's revision check (`rev-discipline`); a finding for a meaning change without a bump; a `specengine.toml` list of counted keys; section attributes other than `rev`; NFC or reflow-insensitive text; a UI screen; `tree`, `search`, HTTP routes; writing `rev=`.

## Open

The owner reviews WA-1..WA-15 and the answers taken: `rev: 0` / `rev=0` is `bad-rev` and unknown, `@0` stays legal; a project's own front-matter keys never count in v1; a document's hash covers its own text only; reflow and comment edits count.

## Implementation

| Module | What it does |
|---|---|
| core `norm.rs` (new) | `NORM_RECIPE`, `norm_input`: own pieces, the H1 cut, declared links, normalisation, framing |
| core `revision.rs` (new) | `Revision`, `number()`, `revision`; `REV_KEY`, `MAX_REVISION` |
| core `lib.rs`, `front_matter.rs`, `markdown.rs` | `section_rev` once, 1-999 999 999; front-matter `rev` 0, out of range or repeated -> `bad-rev`, kept in `extra`; `first_h1`; exports |
| model `node.rs`, `diagnostic.rs` | doc comments |
| store `schema.rs`, `rows.rs`, `write.rs`, `lib.rs` | `nodes.norm_hash`, `INDEX_FORMAT` 8 |
| CLI `show.rs`, `cap.rs`, `bundle.rs` | `run_show` hashes the printed nodes; `shown_header` alone prints it; bundle leaves it `None` |
| MCP `mirror.rs`; `plugin.json` | `ShowNode.norm_hash`; 0.7.0 |
| UI `api/provisional.ts`, `mocks/corpus.ts`, `test/builders.ts` | the type; deterministic fake `b3:` hashes, null for a non-UTF-8 file |

Pins rewritten: store `format.rs`, `format_history.txt`, `tier3.rs` (nodes dump 11 -> 12 columns); CLI `bundle_config.rs`, `common/mod.rs`, `bundle.rs`, `show.rs`, `show_tail.rs` (spec-b's cut moved); MCP `plugin_{files,roles,skills}.rs`; UI `provisional.test.ts`.

Deviations, accepted in review: 1 line endings made LF per piece, then pieces joined with LF (a piece ending in a lone CR stays a line apart); 2 the H1 cut leaves the text before and after it as two pieces (`Intro\n# Title\nBody` = `Intro\n\nBody`); 3 a declared link also needs no `src_span` (drops `status: superseded-by` naming the document itself); 4 extra nulls: a span outside `bytes`, a piece over 4 GiB; `revision` unreadable for no node or a stored rev out of range; 5 `bad-rev` messages reworded. Iteration 2: TE-11 caught `norm_input` re-validating the whole file per node (50 000 sections 1.81 s, quadratic); now only own pieces are read (~0.15 s) and `show` hashes only printed nodes.

TE-12, two consumer corpora read-only: A 1 442 files, 2 474 nodes, all hashed, no revision written, 0.78 s; B 150 files and nodes, 143 hashed, 7 null and 7 revisions unknown (the same `frontmatter-yaml` files), 0 `bad-rev`, 0.51 s.

Known limits (`node-revision.md`): a lone CR ending a heading line or front-matter fence changes the parse; a YAML-escaped self `superseded-by` counts as a link. Untested: a `norm_hash` sent as `base`; TE-12 in release.
