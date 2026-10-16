---
class: spec
status: shipped
scope: [crates/specengine-eval, fixtures]
ref: Phase 3 slice 5a-2, split from hash-levels 2026-10-13 (its Q10; Q1-Q4, E1-E6 decided under the owner's pre-approval, review open); 08 AC-2, AC-10, AC-15; Rust 1 iteration, review accepted, tests 1 round; TE-1 the owner's
shipped: 2026-10-16
adrs: []
---

# hash-levels-eval: level and fingerprint figures on fixtures and pilots

## Why

5a-1 (`hash-levels.md`) fixes the bytes of the levels and fingerprints, 5a-1b (`hash-levels-ron.md`) the RON value's; 5b writes them into every lock, after which a change costs a `LEVEL_RECIPE` bump and a rebase of every lock (ADR-0050). Before that the owner needs figures from real code: do `sig`, `body`, `deps` and `skeleton` hold under rustfmt, comments and a neighbour's edit (08 AC-2), how many items are trivial or twins (AC-15's 124 B), does a renamed item stay followable, what do levels cost per kLOC (AC-10). Split from 5a so the eval cannot use up the library's three iterations (`hash-levels.md` Q10). Order (08 s2): after 5a-1 and 5a-1b, before 6a-1, 6a-2 and 5b. Implemented and reviewed on fixtures (RD-1, TE-2); TE-1, the owner's pilot run, gates 5b alone (a figure below 100.0 % is a 5a-1 recipe change first, ADR-0050) and AC-15's 124 before 5c; `cost.levels_ms_per_kloc` above 6.67 plans 6c at once (`docs/canon/symbol-index.md` "AC-10 measurement") (Q4). No ADR.

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
- `ast-hash` re-parses each file three more times (two neighbour passes, the rename); `ron` lexes and walks each file three more times (`comment_anchors`, `value_hashes`, the CRLF copy's `value_hashes`), plus two for a file with a bound marker (the stripped copy).

## Acceptance criteria

rust-developer:

- [x] RD-1 -- "`ast-hash`: `result.levels`" and "`ron`: `result.value_hash`", every field present on the fixtures. M: a neighbour pass skipped -> `levels.neighbour` in `fixtures/ast-hash/expected.json`; a constant hash -> `edited_changed` below `edited`; a marker's comment stripped -> `value_hash.bound` in `fixtures/ron/expected.json`; the sweep off by one threshold -> `levels.trivial_at` in `fixtures/ast-hash/expected.json`; markers only -> `value_hash.anchored` in `fixtures/ron/expected.json`; `0` for an absent rustfmt -> `levels_fmt_rows_are_null_without_rustfmt` (a rustfmt stub, as `ast_hash_units.rs`). The code-reviewer: `ast_hash/`, `ron.rs` hold no tree-sitter cursor, no `blake3` call, no 124.

test-engineer:

- [ ] TE-1 -- pilots A and B, manual, run by the owner (`#[ignore]`, release, foreground, `--timeout`, a scratch HOME, the child given `PATH`, the label's variables and `SPECENGINE_RUSTFMT`, `SPECENGINE_CARGO` as absolute toolchain paths: `rustup which rustfmt`, `rustup which cargo`, since rustup's proxies find no toolchain under a scratch HOME) (Q3). A run counts only with `detail.rustfmt.available` true. Recorded in "Implementation", counts only: `levels.stable_pct` per level and perturbation (100.0 %, or named exceptions the owner accepts), `module_stable_pct`, `neighbour`, `trivial`, `trivial_at`, `body_groups`, `skeleton_groups_in_file`, `quote_separates`, `rename`, `cost` against 6.67 ms per kLOC, `value_hash`'s `anchored`, `hashed`, `cannot_verify`, `crlf_stable_pct`, `wall_ms`; read-only proof (`git --no-optional-locks status`, a (path, kind, size, mtime) listing, before and after). Zero `levels-unstable.json` rows, the count printed: "100.0" alone hides up to 0.05 % (E4). Run in two steps (E6): `cargo nextest run --release -p specengine-eval --test hash_levels_eval --no-run` outside the alarm, then the run under `perl -e 'alarm 7800; exec @ARGV'` (`code-hash-eval.md` "Runs and cost"). Then the spec-writer records the figures here and in `code-hash-eval.md` "Measured", 08 AC-2 met (or the accepted exceptions), AC-10's levels figure, AC-15's threshold as the owner keeps or moves it (from SW-2).
- [x] TE-2 -- `ast-hash` and `ron` without `--pilot` give `expected.json`'s counts exactly, `edited_changed` = `edited`, zero `levels-unstable.json` rows, and leave `git status -- fixtures/` empty. M: a count off by one.

spec-writer:

- [x] SW-1 -- this spec (split from `hash-levels.md`, 2026-10-13; refreshed 2026-10-15).
- [x] SW-2 -- at shipping: `code-hash.md` had 667 B left, so a Tier 2 `docs/canon/code-hash-eval.md` holds the fields as built, the readings, the method and the fixture and this repository's figures, pointing at `code-identity.md` "Eval outputs" and that section pointing back; the eval README `ast-hash`, `ron` rows name it (10 195 -> 10 188 B); 08 s2's slice list; `CLAUDE.md` State. The TE-1 parts moved to TE-1.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

The library (5a-1, 5a-1b); a marker in a pilot; per-branch ratios (Phase 4); pilot runs in CI; 6a's index timings (`docs/canon/symbol-index.md` "AC-10 measurement").

## Open

Decided 2026-10-15 under the owner's pre-approval, review open: Q1 RON values measured over every `Path`-anchored comment, marked or not; Q2 the sweep at 64, 96, 124, 160, 256; Q3 a scratch HOME, rustfmt and cargo by absolute path, a run counting only with rustfmt; Q4 TE-1 gates only 5b and 5c, not 6a nor this slice (flagged for the owner). `hash-levels.md` Q10 and WA-13 hold; "SW refinement" (2026-10-13): `modules`, `cost`. E1-E6: "Implementation".

## Implementation

| Module (eval `src/`) | What it does |
|---|---|
| `ast_hash/levels.rs` | `result.levels`: `originals` (one parse per file: levels, module levels, edit points, `cost`), `Passes::keyed` (fmt, comments, by v2's key), `neighbour`, `rename`, `result` (groups, sweep), `levels-unstable.json` rows, stderr summary |
| `ast_hash/mod.rs` | `Analyzed.levels`, `.module` off the one tree; step 4b, the neighbour and rename passes; `AstHashResult.levels`; the new `--out` file |
| `ron.rs` | `RonResult.value_hash`: `value_hashes` beside `comment_anchors`, `crlf_copy`, `strip_unmarked` |

Deviations, accepted in review: edit points found by `descendant_for_byte_range` plus `parent()`, a node walk of the eval's own against "never a walk", item nodes being private to `specengine-code` (a library accessor may replace it); `followable` also needs the `body` unchanged by the rename; the Rules' `ron` cost line corrected; the rename identifier `specengine_renamed_<index in file>`; `detail.hash_ms` unchanged, the levels passes after it.

Owner's answers (pre-approved): E1 `cost.*` `null` on a corpus with no `.rs` newline; E2 `--out` also writes `levels-unstable.json`, per-item rows like `unstable.json` (path, line, kind, label, perturbation, the changed judged fields), never on stdout; E3 a marker losing its value hash in the stripped copy counts neither as bound nor as unstable; E4 "100.0" means no item changed, so TE-1 needs zero `levels-unstable.json` rows; E5 neighbour passes editing nothing still count their items as checked; E6 TE-1 in two steps (TE-1).

Tester readings, adopted: L1 indexes over every item, broken ones edited and renamed, counts over items with levels; L2 one decimal over the items compared, 100.0 when none; L3 a `fmt_*` row itself `null`; L4 `trivial_at` keyed by the threshold's text, `cost` under `levels`; L5 `quote_separates` over both group kinds; L6 `renamed` = named items with levels holding none, a quote naming a renamed item in `before` or `after` not kept; L7 `bound` = markers with a value hash in both copies; L8 `git status -- fixtures/` equal before and after; L9 TE-1 judges every figure but `quote`, `module_stable_pct`, groups, sweep, `cost`; L10 the child gets only the corpus variable and toolchain paths; L11 the read-only listing walks as the harness; L12 totals = v2's `items`, `files`; L13 a stripped comment becomes one space, the edit a byte insert; L14 in-file skeleton groups include trivial items.

Figures: fixtures as `expected.json` (66 + 3 items; neighbour 66, 66, 52; trivial 63; skeleton groups 3 of 10 items; 59 renamed, 3 followable; `ron` 15 anchored, 8 bound); this repository, release, 14 534 items: judged figures printed 100.0, `cost` 3.89 ms/kLOC (v2 1.72, limit 6.67), 48 s; pilots: TE-1.

Tests, 1 round: 652 of 654 pass (`import_cli`, `layout_cli` red until the fixture pins are committed); 9 mutations killed, M4b (`<` to `<=` in the sweep) by `trivial_at_puts_an_item_at_a_threshold_above_it`; E1, E4, E6 applied (`hash_levels_eval.rs`: 7, 2 ignored). fmt, clippy clean.
