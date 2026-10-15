---
class: spec
status: draft
scope: [crates/specengine-eval, fixtures]
ref: Phase 3 slice 5a-2, split from hash-levels 2026-10-13 (its Q10, decided under the owner's pre-approval, review open); 08 AC-2, AC-10, AC-15; refreshed 2026-10-15 against shipped 4b-1, 4c-1
adrs: []
---

# hash-levels-eval: level and fingerprint figures on fixtures and pilots

## Why

5a-1 (`hash-levels.md`) fixes the bytes of the levels and fingerprints, 5a-1b (`hash-levels-ron.md`) the RON value's; 5b writes them into every lock, after which a change costs a `LEVEL_RECIPE` bump and a rebase of every lock (ADR-0050). Before that the owner needs figures from real code: do `sig`, `body`, `deps` and `skeleton` hold under rustfmt, comments and a neighbour's edit (08 AC-2), how many items are trivial or twins (AC-15's 124 B), does a renamed item stay followable, what do levels cost per kLOC (AC-10). Split from 5a so the eval cannot use up the library's three iterations (`hash-levels.md` Q10). Order (08 s2): after 5a-1 and 5a-1b, before 6a-1, 6a-2 and 5b. Implemented and reviewed on fixtures (RD-1, TE-2); TE-1, the owner's pilot run, gates 5b alone (a figure below 100.0 % is a 5a-1 recipe change first, ADR-0050) and AC-15's 124 before 5c; `cost.levels_ms_per_kloc` above 6.67 plans 6c at once (`symbol-index.md` "AC-10 measurement") (Q4). No ADR.

## Description and interactions

- **The owner** runs `specengine-eval ast-hash` and `ron` on the fixtures or a pilot (`--pilot`, `--label`) and reads `result.levels` and `result.value_hash`. No CLI, lock, MCP, UI or `specengine-code` change.
- **Crates**: `specengine-eval` (`ast-hash`, `ron`), calling 5a-1's entry points over each file parsed once: `analyze_tree`'s item indexes into `item_levels` and `hash_bound_items`, `module_levels`, `LevelHashes::is_trivial` (`hash-levels.md` "Entry points"); 5a-1b's `ron::value_hashes` beside `ron::comment_anchors` (`hash-levels-ron.md` "Entry point"); never a walk, hash or threshold of its own; `fixtures/ast-hash/expected.json`, `fixtures/ron/expected.json`. No new dependency.
- **Output** (eval README "CLI contract"): counts, percentages and times only, never a file name, path, ID or item name.

## Data

### `ast-hash`: `result.levels`

WA-13 of the 5a analysis (2026-10-10).

- `hashed`, `cannot_verify`: items with and without levels; `modules.{hashed, cannot_verify}`: files (SW refinement).
- `stable_pct.<p>.<f>`: `p` in `fmt_default`, `fmt_contrast`, `comments`, `neighbour`; `f` in `sig`, `body`, `deps`, `skeleton`, `body_bytes`, `quote` (`quote` reported, never judged: textual by design, outside AC-2). `module_stable_pct.<p>.{sig, body, deps}` over files, the first three `p`. The `fmt_*` rows are `null` when no rustfmt was found, as v2's `stable_pct` (`detail.rustfmt.available`).
- `neighbour.{edited, edited_changed, checked}`: two passes per file over its items in source order, one re-parse each; pass 1 edits every odd item and checks every even one, pass 2 swaps. The edit inserts `#[cfg(all())]` and an LF before the item's first attached attribute (else the item): its `sig` changes, every later byte and line shifts. Checked: items of the other parity neither holding nor inside an edited item. `edited_changed` = edited items whose `sig` changed, equal to `edited` (non-vacuity), matched to the original by item index.
- `trivial.{items, pct}`; `body_groups.{groups, items}`: 2+ non-trivial items of one kind with one `body` across the corpus; `skeleton_groups_in_file.{groups, items}`: 2+ items of one kind and file with one `skeleton`; `quote_separates`: of these groups, those whose members' quotes all differ.
- `trivial_at.<t>.{items, body_twins}` for `t` in 64, 96, 124, 160, 256 (Q2): items with `body_bytes < t`; items at or above `t` sharing one `body` with another of their kind. AC-15's 124 moves only on these.
- `rename.{renamed, kept.<f>, followable}`: in memory, the name node of every named item holding no other named item replaced by a distinct fixed identifier, one re-parse per file; `kept` per `f` (all but `quote` equal to `renamed`); `followable` = renamed, non-trivial, with a `body` unique among the corpus's items of its kind, matched by item index (a rename keeps the order).
- `cost.{levels_ms_per_kloc, v2_ms_per_kloc}` (SW refinement): the least of three passes, parse excluded, over the same parsed trees, of `item_levels` over every item plus `module_levels`, and of `hash_bound_items` over every item, in release, per thousand `.rs` newlines (6a-2's `kloc`); AC-10's full symbol hash is 2 s per 300 kLOC (6.67 ms per kLOC).

### `ron`: `result.value_hash`

`{anchored, hashed, cannot_verify, crlf_stable_pct, bound, bound_stable_pct}` (Q1). Over every comment with a `Path` anchor, marked or not (`comment_anchors`; no pilot `.ron` holds a marker): `anchored`; `hashed`; `cannot_verify`, the values of files `RonAnalysis` rejects (`hash-levels-ron.md` "RON value"); `crlf_stable_pct` against an in-memory CRLF copy, matched by comment order. Over markers: `bound`, and `bound_stable_pct` with every comment holding no marker stripped (stripping a marker's comment drops its binding), compared per marker in source order; `null` without a marker.

`fixtures/ast-hash/expected.json` and `fixtures/ron/expected.json` pin the exact fixture counts; timings are never pinned.

## Rules and edge cases

- Pilots stay read-only: perturbations in memory or on scratch copies (eval README "Rules"); no marker added (ADR-0016); no pilot name or path committed.
- WHEN a fixture run is repeated, the system SHALL give the same JSON but its times (`wall_ms`, `detail.hash_ms`, `cost`).
- `ast-hash` re-parses each file three more times (two neighbour passes, the rename); `ron` lexes each file once more (the CRLF copy).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "`ast-hash`: `result.levels`" and "`ron`: `result.value_hash`", every field present on the fixtures. M: a neighbour pass skipped -> `levels.neighbour` in `fixtures/ast-hash/expected.json`; a constant hash -> `edited_changed` below `edited`; a marker's comment stripped -> `value_hash.bound` in `fixtures/ron/expected.json`; the sweep off by one threshold -> `levels.trivial_at` in `fixtures/ast-hash/expected.json`; markers only -> `value_hash.anchored` in `fixtures/ron/expected.json`; `0` for an absent rustfmt -> `levels_fmt_rows_are_null_without_rustfmt` (a rustfmt stub, as `ast_hash_units.rs`). The code-reviewer: `ast_hash/`, `ron.rs` hold no tree-sitter cursor, no `blake3` call, no 124.

test-engineer:

- [ ] TE-1 -- pilots A and B, manual, run by the owner (`#[ignore]`, release, foreground, `--timeout`, a scratch HOME, the child given `PATH`, the label's variables and `SPECENGINE_RUSTFMT`, `SPECENGINE_CARGO` as absolute toolchain paths: `rustup which rustfmt`, `rustup which cargo`, since rustup's proxies find no toolchain under a scratch HOME) (Q3). A run counts only with `detail.rustfmt.available` true. Recorded in "Implementation", counts only: `levels.stable_pct` per level and perturbation (100.0 %, or named exceptions the owner accepts), `module_stable_pct`, `neighbour`, `trivial`, `trivial_at`, `body_groups`, `skeleton_groups_in_file`, `quote_separates`, `rename`, `cost` against 6.67 ms per kLOC, `value_hash`'s `anchored`, `hashed`, `cannot_verify`, `crlf_stable_pct`, `wall_ms`; read-only proof (`git --no-optional-locks status`, a (path, kind, size, mtime) listing, before and after).
- [ ] TE-2 -- `ast-hash` and `ron` without `--pilot` give `expected.json`'s counts exactly, `edited_changed` = `edited`, and leave `git status -- fixtures/` empty. M: a count off by one.

spec-writer:

- [x] SW-1 -- this spec (split from `hash-levels.md`, 2026-10-13; refreshed 2026-10-15).
- [ ] SW-2 -- at shipping, after TE-1: 08 AC-2 met with TE-1's figures (or the accepted exceptions), AC-10's levels figure, AC-15's threshold as the owner keeps or moves it; 08 s2's slice list; the fields and the pilot figures in `docs/canon/code-hash.md` "Eval outputs", "Measured" when 5a-1 and 5a-1b left room, else a Tier 2 `docs/canon/code-hash-eval.md` (≤ 12 288 B; `docs/index-canon.md` re-measured), pointing at `code-identity.md` "Eval outputs" for the v2, `qpath` and marker fields, and that section pointing back; the eval README `ast-hash`, `ron` rows naming that section (10 185 of 10 240 B on 2026-10-15, re-measure, net ≤ 0); `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- Neither pilot holds a marker in `.rs` or `.ron`: levels and anchors are proven on fixtures (5a-1, 5a-1b); pilot evidence is AC-2 figures, twin counts and their threshold sweep, the rename simulation, RON coverage and refusal over unmarked anchors. limpet's 124 B is not verifiable locally.
- A figure below 100.0 % found here is a 5a-1 recipe change before 5b, not after (ADR-0050).
- A scratch HOME hides rustup's toolchains: without TE-1's absolute paths the `fmt_*` rows are `null` and AC-2's main figure is missing.

## Out of scope

The library (5a-1, 5a-1b); a marker in a pilot; per-branch ratios (Phase 4); pilot runs in CI; 6a's index timings (`symbol-index.md` "AC-10 measurement").

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 RON values on the pilots | measured over every `Path`-anchored comment, marked or not: `anchored`, `hashed`, `cannot_verify`, CRLF stability | `bound` 0 and `stable_pct` `null` on both pilots: no RON evidence before 5b, `hash-levels-ron.md` Q7's whole-file refusal unmeasured |
| Q2 a threshold sweep | `trivial_at` at 64, 96, 124, 160, 256 | "calibrates 124" stays unverifiable; the owner can only keep 124 |
| Q3 the pilot environment | a scratch HOME, rustfmt and cargo by absolute path; a run counts only with rustfmt available | under the owner's HOME the read-only proof must cover HOME too; else the `fmt_*` rows are `null` and AC-2 is unmet |
| Q4 what waits on TE-1 (owner, critical) | only 5b (the frozen bytes) and 5c (the threshold); 6a-1, 6a-2 and this slice's implementation do not | 5b on fixture evidence alone: a pilot shortfall found later costs a v4 recipe, a rebase of every lock, every rejected drift item raised again |

Q1-Q4 decided 2026-10-15 under the owner's pre-approval, review open; Q4 flagged for the owner, nothing waits on it; `hash-levels.md` Q10 and WA-13 hold; "SW refinement" (2026-10-13): `modules`, `cost`.

## Implementation

Not started. Criteria from the split (2026-10-13): RD-1 <- `hash-levels.md` old RD-9; TE-1 <- its old TE-5; TE-2 new (the `expected.json` pins as a criterion); SW-2 <- its old SW-2's 08 AC-2 and eval README parts; SW-1, SW-3 new. Refreshed 2026-10-15, no renumbering: RD-1 gains three mutations, TE-1 its environment and figure list. To record: modules, TE-1's figures, deviations with reasons.
