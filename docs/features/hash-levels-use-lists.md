---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-eval, fixtures]
ref: Phase 3 slice 5a-1c, from 5a-2's first TE-1 (the owner's run, 2026-10-19); Q1-Q4 decided 2026-10-19 under the owner's pre-approval (Q2 flagged for the owner); 05 s5.2; 08 s2, AC-2, AC-10, AC-14; developer 2 iterations, review 1 accepted, iteration 2 re-reviewed accepted; OW-1 the owner's
shipped: 2026-10-20
adrs: []
---

# hash-levels-use-lists (5a-1c): use-list members in canonical order (N6)

## Why

5a-2's first TE-1 (the owner's run, 2026-10-19) failed on pilot A: every item figure 100.0, but `module_stable_pct.body` 79.6 under both rustfmt passes, 80 `levels-unstable.json` rows (40 files × 2), each a file module whose `body` alone changed. rustfmt sorts the members inside `use a::{..}` braces; `walk3` sorted runs of `use` items (N4) but walked a `use_list`'s members in source order, inside items too (latent on pilot A). Member order has no meaning in Rust. Before 5b-1 records v3 digests the fix costs a recipe change and one re-derivation of the code index; after it, a `LEVEL_RECIPE` bump and a rebase of every lock (ADR-0050). The route was written (`code-hash-eval.md` "Measured"): after 5a-2, before 5b-1, the one exception to the `levels.rs` freeze (08 s2); then the owner's TE-1 rerun (OW-1), which gates 5b-1. No ADR: ADR-0050's rule covers recorded digests and none records v3 yet; N5 shipped the same way. As built: `docs/canon/code-hash.md` "Levels" (N6) and "Known limits".

## Acceptance criteria

rust-developer:

- [x] RD-1 -- N6 in `crates/specengine-code/src/levels.rs` only; `git diff main -- crates/specengine-code/src/hash.rs crates/specengine-code/src/ron Cargo.lock` empty (the code-reviewer). M: N6 in `hash.rs` -> TE-5; N6 applied to a brace list other than a `use_list` (a `field_declaration_list`, a `token_tree`) -> `hash_levels_units.rs`' golden digests.
- [x] RD-2 -- `CODE_INDEX_FORMAT` 5, its doc naming the in-place amendment of the level recipe. M: kept at 4 -> TE-9.

test-engineer:

- [x] TE-1 -- every permutation of a list's members at every depth gives equal `sig`, `body`, `deps`, `skeleton`, `body_bytes`, item and module, in three places (file level, a function body, an inline `mod`) and five shapes (flat; mixed `self`, `*`, `as`, paths; nested; a comment inside the braces; a trailing comma or none): `hash_levels_props.rs` `te1_use_list_permutations_keep_every_level_at_every_depth`, 4 050 renderings (5 shapes × 5 layouts × 3 places). M1 N6 removed; M2 only depth-1 lists sorted.
- [x] TE-2 -- `fixtures/hash-levels/src/use_lists.rs` and its `fmt-default`, `fmt-60` variants: `te2_levels_hold_under_the_committed_rustfmt_variants` green, every file module counted, N6 occurring at four named items; the TE-6 relation of `hash-levels.md` reads "N5 or N6" (`levels_common::has_n6_list`); eval `te6_v2_changes_exactly_with_sig_or_body_over_the_pairs` requires both `"v2": "n5"` and `"n6"` pairs. M1.
- [x] TE-3 -- `body` differs, item and module, on a member removed, renamed, its alias changed, a glob swapped for a name, `self` removed, a member moved between nested lists, a duplicate added: `te3_use_list_edits_change_the_body`, 11 edits. M3 duplicates removed after sorting.
- [x] TE-4 -- `skeleton` equal for `x::{a::{p, q}, b::{r}}` and `x::{z::{p, q}, b::{r}}`: `te4_use_list_skeleton_sorts_by_the_masked_walks`. M4 the masked stream sorted by the normal walks.
- [x] TE-5 -- v2 untouched: `hash_item` tells `{b, c}` from `{c, b}`, two v2 digests pinned, equal on `main` (`hash_v2.rs` `use_list_member_order_still_moves_the_v2_hash`). M5 N6 added to `hash.rs`.
- [x] TE-6 -- `hash_levels_depth.rs`: `nested_use_lists_count_against_the_cap` (64 deep hashed; 65, 66 and 2 000 refused `nesting_too_deep`; N4 and N6 runs counted together; single-member lists not counted), `hundred_thousand_nested_use_lists_are_refused_in_time`, `hundred_thousand_single_member_lists_are_hashed_in_time` (item and module). The bound, `DEEP_LIST_BOUND`: 5 s for the level walks, parse excluded, debug build, inside a 60 s deadline so a hang fails rather than stalls. M6 N6 runs not counted against the cap.
- [x] TE-7 -- module figures judged (Q3): `shortfalls()` in eval `tests/hash_levels_eval.rs` names `levels.module_stable_pct.<p>.<f>` below 100.0, over `MODULE_PERTURBATIONS` × `MODULE_FIELDS`: `shortfalls_name_the_module_figures_below_100`. M7 the module loop removed.
- [x] TE-8 -- the Q2 limit pinned, `te8_single_member_and_empty_lists_are_not_normalised`: `{b}`, `{*}`, `{b as c}`, `{b::{c}}` against their unwrapped forms, `{a, a}` against `{a}`, and `use a::{};` give a different `body`. M: a single-member unwrap (`a::{b}` walked as `a::b`).
- [x] TE-9 -- `code_format_history.txt` line `5 5ab64e6e779803b6708fd4f2750e701e11db3c92a1477710bc7ed9135973f640`; `code_format_history_keeps_lines_1_to_4_and_adds_5`. M: `CODE_INDEX_FORMAT` kept at 4.
- [x] TE-10 -- `#[ignore]`, needs rustfmt: `te10_hash_levels_sources_leave_no_unstable_row` (`ast-hash --pilot` over a scratch copy of `fixtures/hash-levels/src`) writes zero `levels-unstable.json` rows, module rows included. M1.

owner:

- [ ] OW-1 -- 5a-2's TE-1 rerun on pilots A and B by its method (`code-hash-eval.md` "Runs and cost"): zero `levels-unstable.json` rows on each, every judged figure 100.0, `module_stable_pct` included; `cost.levels_ms_per_kloc` within 6.67. A row left is a finding routed as this slice was. Closes `hash-levels-eval.md` TE-1; opens 5b-1.

spec-writer:

- [x] SW-1 -- the canon: `code-hash.md` N6 in the `walk3` bullet, "N5, N6" in "Levels" and "File module", the two "Known limits" clauses, its v3 cost line moved to `code-hash-eval.md` "Measured"; `code-hash-eval.md` judging `module_stable_pct` ("Judged", the pilot run) and the first TE-1 with its route ("Measured"); `symbol-index.md` "Analyzer stamp" 5; the code README; `hash-levels-eval.md` L9; 08 s2; `CLAUDE.md` State.
- [x] SW-2 -- this spec; 08 s2 (slice list, build order, the freeze's exception); the routing note in `hash-levels-eval.md` TE-1.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Implementation

| Module | What it does |
|---|---|
| code `src/levels.rs` | N6: walk3's `Open` frame keeps `list`, the node when a `use_list`; `list_run` makes its named non-comment children, two or more, a `Run` that `RunWalk` sorts, normal and masked buffers apart, counted against `MAX_USE_RUN_NESTING`. Fewer than two named children exit in O(1); any other list is read once through `list.walk()`, never a copy of the walk's cursor. `LEVEL_RECIPE`'s doc records the amendment |
| code `src/lib.rs` | `CODE_INDEX_FORMAT` 5 |
| code `tests/` | TE-1, TE-2 `hash_levels_props.rs`; TE-3, TE-4, TE-8 `hash_levels_units.rs`; TE-6 `hash_levels_depth.rs`; TE-5 `hash_v2.rs`; `levels_common::has_n6_list` |
| eval `tests/` | TE-7, TE-10 `hash_levels_eval.rs`; "N5 or N6" `hash_levels_fixture.rs` |
| CLI `tests/` | TE-9: `code_format_history.txt`, `code_index_cache.rs` |
| `fixtures/hash-levels/` | `use_lists.rs` and its variants (rustfmt 1.9.0 `--edition 2024 --emit stdout`, `fmt-60` adding `--config max_width=60`: the recipe reproduces the 8 older variants byte for byte); `expected.json` 17 `ul-*` pairs, 7 `"v2": "n6"` |

Decisions (the owner's pre-approval): Q1 `specengine-hash/v3` amended in place, `level_header()` unmoved, hence the stamp; **Q2, for the owner**: `a::{b}` against `a::b` and `a::{}` not normalised (neither occurs on either pilot; cheap before 5b-1, after it a v4 recipe and a rebase of every lock); Q3 module figures judged; Q4 v2 untouched.

Divergences: A1 was wrong. Default rustfmt 1.9.0 may drop a duplicated member (`{Write, Read, Read}` → `{Read, Write}`, `{b, b, a}` → `{a, b}`, `{*, *}` → `{*}`; `{b, b as c, b}` kept); N6 keeps it. A duplicate glob compiles (rustc, edition 2024, a warning), a duplicate name is E0252: a canon limit, not confined to broken code. Stamps: line 5 was planned equal to line 4 (`8a7cd52d…`), but the dump holds `code_fp`, so the stamp alone moved it.

Iteration 2 (review finding 1): iteration 1's `list_run` copied the walk's cursor per list, quadratic on a single-member chain (100 000 levels: 6 s release, 13.36 s in TE-6 against 5 s); now 72 ms, linear, digests byte-identical over 1.1 M items.

Figures, the orchestrator's read-only diagnostic eval, not OW-1: pilot A every judged figure 100.0, module included, 0 rows, `cost` 4.02 ms/kLOC (before N6: module `body` 79.6, 80 rows); pilot B all 100.0, 0 rows, 4.0. On `effcd01`, without N6: the N6 tests red, TE-10 14 rows.

Tests: code 327 of 327, eval and CLI 50 of 50 (3 skipped), TE-10 1 of 1; clippy (code, eval, CLI) clean. Mutations 7 of 7 killed: M1 N6 removed (te1, te2, te4, the cap pair, eval te1_pairs and te6, TE-10); M2 depth-1 only (te1: 2 700 of 4 050 renderings, te2, eval te1_pairs and te6, cap); M3 dedup after sort (te3, te8, eval `ul-fn-duplicate`, te6); M4 masked by normal walks (te4); M5 N6 in v2's `use_run` (TE-5, te1, te2, eval te6); M6 runs not counted against the cap (cap pair); M7 module loop out of `shortfalls()` (`shortfalls_name_the_module_figures_below_100`).
