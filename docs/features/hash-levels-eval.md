---
class: spec
status: draft
scope: [crates/specengine-eval, fixtures]
ref: Phase 3 slice 5a-2, split from hash-levels 2026-10-13 (its Q10, decided under the owner's pre-approval, review open); 08 AC-2, AC-10, AC-15
adrs: []
---

# hash-levels-eval: level and fingerprint figures on fixtures and pilots

## Why

5a-1 (`hash-levels.md`) fixes the bytes of the levels and fingerprints; 5b writes them into every lock, after which a change costs a `LEVEL_RECIPE` bump and a rebase of every lock (ADR-0050). Before that the owner needs figures from real code: do `sig`, `body`, `deps` and `skeleton` hold under rustfmt, comments and a neighbour's edit (08 AC-2), how many items are trivial or twins (AC-15's 124 B), does a renamed item stay followable, what do levels cost per kLOC (AC-10). Split from 5a so the eval cannot use up the library's three iterations (`hash-levels.md` Q10). Order: after 5a-1, before 5b. No ADR.

## Description and interactions

- **The owner** runs `specengine-eval ast-hash` and `ron` on the fixtures or a pilot (`--pilot`, `--label`) and reads `result.levels` and `result.value_hash`. No CLI, lock, MCP, UI or `specengine-code` change.
- **Crates**: `specengine-eval` (`ast-hash`, `ron`), calling 5a-1's entry points (`item_levels`, `module_levels`, `ron::value_hashes`: `hash-levels.md` "Entry points"), never a walk of its own; `fixtures/ast-hash/expected.json`, `fixtures/ron/expected.json`. No new dependency.
- **Output** (eval README "CLI contract"): counts, percentages and times only, never a file name, path, ID or item name.

## Data

### `ast-hash`: `result.levels`

WA-13 of the 5a analysis (2026-10-10).

- `hashed`, `cannot_verify`: items with and without levels; `modules.{hashed, cannot_verify}`: files (SW refinement).
- `stable_pct.<p>.<f>`: `p` in `fmt_default`, `fmt_contrast`, `comments`, `neighbour`; `f` in `sig`, `body`, `deps`, `skeleton`, `body_bytes`, `quote` (`quote` reported, never judged: textual by design, outside AC-2). `module_stable_pct.<p>.{sig, body, deps}` over files, the first three `p`.
- `neighbour.{edited, edited_changed, checked}`: two passes per file over its items in source order, one re-parse each; pass 1 edits every odd item and checks every even one, pass 2 swaps. The edit inserts `#[cfg(all())]` and an LF before the item's first attached attribute (else the item): its `sig` changes, every later byte and line shifts. Checked: items of the other parity neither holding nor inside an edited item. `edited_changed` = edited items whose `sig` changed, equal to `edited` (non-vacuity).
- `trivial.{items, pct}`; `body_groups.{groups, items}`: 2+ non-trivial items of one kind with one `body` across the corpus; `skeleton_groups_in_file.{groups, items}`: 2+ items of one kind and file with one `skeleton`; `quote_separates`: of these groups, those whose members' quotes all differ.
- `rename.{renamed, kept.<f>, followable}`: in memory, the name node of every named item holding no other named item replaced by a distinct fixed identifier, one re-parse per file; `kept` per `f` (all but `quote` equal to `renamed`); `followable` = renamed, non-trivial, with a `body` unique among the corpus's items of its kind.
- `cost.{levels_ms_per_kloc, v2_ms_per_kloc}` (SW refinement): the least of three passes, parse excluded, of every item's and file module's levels and fingerprints, and of every item's `hash_item`, per thousand `.rs` lines walked; AC-10's full symbol hash is 2 s per 300 kLOC (6.7 ms per kLOC).

### `ron`: `result.value_hash`

`{bound, hashed, cannot_verify, stable_pct}`: every comment holding no marker stripped (stripping a marker's comment drops its binding), values compared per marker in source order; `cannot_verify` counts the values of files `RonAnalysis` rejects (`hash-levels.md` "RON value").

`fixtures/ast-hash/expected.json` and `fixtures/ron/expected.json` pin the exact fixture counts; timings are never pinned.

## Rules and edge cases

- Pilots stay read-only: perturbations in memory or on scratch copies (eval README "Rules"); no marker added (ADR-0016); no pilot name or path committed.
- WHEN a fixture run is repeated, the system SHALL give the same JSON but `wall_ms` and `cost`.
- `ast-hash` re-parses each file three more times (two neighbour passes, the rename).

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "`ast-hash`: `result.levels`" and "`ron`: `result.value_hash`", every field present on the fixtures. M: a neighbour pass skipped -> `levels.neighbour` in `fixtures/ast-hash/expected.json`; a constant hash -> `edited_changed` below `edited`; a marker's comment stripped -> `value_hash.bound` in `fixtures/ron/expected.json`.

test-engineer:

- [ ] TE-1 -- pilots, manual, run by the owner (`#[ignore]`, release, foreground, `--timeout`, a scratch HOME): `levels` per level and perturbation (100.0 %, or named exceptions the owner accepts), `module_stable_pct`, `value_hash.stable_pct`, twin and triviality counts, rename counts, `cost` against AC-10, wall time; read-only proof (`git status`, a file listing).
- [ ] TE-2 -- `ast-hash` and `ron` without `--pilot` give `expected.json`'s counts exactly, `edited_changed` = `edited`, and leave `git status -- fixtures/` empty. M: a count off by one.

spec-writer:

- [x] SW-1 -- this spec (split from `hash-levels.md`, 2026-10-13).
- [ ] SW-2 -- at shipping: 08 AC-2 met with TE-1's figures (or the accepted exceptions), 08 s2's slice list; the eval README `ast-hash`, `ron` rows (10 185 of 10 240 B: the field list goes to `docs/canon/code-hash.md` "Eval outputs", the rows name it); `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- Neither pilot holds a marker in `.rs` or `.ron`: levels and anchors are proven on fixtures (5a-1); pilot evidence is AC-2 figures, twin counts, the rename simulation. limpet's 124 B is not verifiable locally.
- A figure below 100.0 % found here is a 5a-1 recipe change before 5b, not after (ADR-0050).

## Out of scope

The library (5a-1); a marker in a pilot; per-branch ratios (Phase 4); pilot runs in CI; 6a's index timings (`symbol-index.md` "AC-10 measurement").

## Open

No point of its own: `hash-levels.md` Q10 (the split) and WA-13 hold; "SW refinement" (2026-10-13): `modules`, `cost`.

## Implementation

Not started. Criteria from the split (2026-10-13): RD-1 <- `hash-levels.md` old RD-9; TE-1 <- its old TE-5; TE-2 new (the `expected.json` pins as a criterion); SW-2 <- its old SW-2's 08 AC-2 and eval README parts; SW-1, SW-3 new. To record: modules, TE-1's figures, deviations with reasons.
