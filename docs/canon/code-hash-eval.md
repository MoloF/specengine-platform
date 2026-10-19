---
class: canon
tier: 2
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-10-20
---

# Code hash eval: level, fingerprint and RON value figures

What `specengine-eval ast-hash` and `ron` report about v3 (`code-hash.md`) and the RON value hash (`code-hash-ron.md`), since 5a-2; v2, `qpath`, target and marker fields: `code-identity.md` "Eval outputs". The eval calls the library only (`analyze_tree`, `item_levels`, `module_levels`, `hash_bound_items`, `LevelHashes::is_trivial`, `TRIVIAL_BODY_BYTES`, `attached_attributes`, `name_node`; `ron::comment_anchors`, `value_hashes`) over each file parsed once: no hash, threshold or cursor of its own. stdout: counts, percentages, times; names only under `--out`.

## `ast-hash`: `result.levels`

- `hashed`, `cannot_verify`: items; `modules.{hashed, cannot_verify}`: files; the totals equal v2's `items`, `files`.
- `stable_pct.<p>.<f>`, `p` in `fmt_default`, `fmt_contrast`, `comments` (these match items by v2's key) and `neighbour` (by index); `f` in `sig`, `body`, `deps`, `skeleton`, `body_bytes`, `quote`. `module_stable_pct.<p>.{sig, body, deps}`, the first three `p`, over hashed files. Without rustfmt (`detail.rustfmt.available` false) a `fmt_*` row is itself `null`. Judged: every `f` but `quote` (textual by design, reported), and every `module_stable_pct` field (5a-1c). Comment stripping turns a comment into one space.
- **Percentages**: one decimal, half up, over the items compared (hashed originals; `checked` for `neighbour`); 100.0 when none is compared. **"100.0" means no item changed only with zero `levels-unstable.json` rows**: rounding hides up to 0.05 % (7 of 14.5k items).
- `neighbour.{edited, edited_changed, checked}`: two passes per file over every item in analysis order, broken ones included, one re-parse each; pass 1 edits odd indexes and checks even ones, pass 2 swaps. The edit inserts `#[cfg(all())]` and an LF at the item's first attached attribute, else the item. Checked: the other parity, neither holding nor inside an edited item. `edited_changed`: edited items whose `sig` changed, equal to `edited`. A pass editing nothing (a file of 0-1 items) still re-parses, and its items count as checked.
- `trivial.{items, pct}` (`is_trivial`, over `hashed`); `body_groups.{groups, items}`: 2+ non-trivial items of one kind with one `body` across the corpus; `skeleton_groups_in_file.{groups, items}`: 2+ items of one kind and file with one `skeleton`, trivial ones included; `quote_separates`: groups of both kinds whose members' quotes all differ.
- `trivial_at."<t>".{items, body_twins}`, `t` 64, 96, `TRIVIAL_BODY_BYTES` (124), 160, 256, in numeric order: items with `body_bytes < t`; items at or above `t` sharing a `body` with another of their kind.
- `rename.{renamed, kept.<f>, followable}`: every named item with levels holding no named item gets its name node replaced by `specengine_renamed_<index in file>`, one re-parse per file. A `quote` whose `before` or `after` line holds a renamed name is not kept. `followable`: renamed, non-trivial, its `body` unique among the corpus's items of its kind and unchanged by the rename, matched by index.
- `cost.{levels_ms_per_kloc, v2_ms_per_kloc}`: the least of 3 passes over the parsed trees, parse excluded: `item_levels` over every item plus `module_levels`; `hash_bound_items` over every item; per thousand `.rs` newlines, two decimals, `null` without a newline. A release figure; AC-10's limit 6.67 ms per kLOC (2 s per 300 kLOC), above it 6c is planned at once (`symbol-index.md` "AC-10 measurement").
- `detail.hash_ms` keeps v2's meaning (parse plus `analyze_tree`); the levels passes run after it.

**Edit points.** Item nodes are private to `specengine-code`: the eval takes `descendant_for_byte_range` over an item's bytes and climbs `parent()` to the node of its kind and range, then `attached_attributes` and `name_node`. A point off a character boundary skips that file's pass with a stderr note.

**`--out` `levels-unstable.json`**, like `unstable.json`: `{path, line, kind, label, perturbation, changed}` per item, or file module (`kind` `source_file`), whose judged field changed (`changed` names them, never `quote`) or that is `cannot_verify` or `unmatched` after; `perturbation` a `stable_pct` row or `rename`.

## `ron`: `result.value_hash`

`{anchored, hashed, cannot_verify, crlf_stable_pct, bound, bound_stable_pct}`. Over every comment with a `Path` anchor, marked or not: `anchored` = `hashed` + `cannot_verify` (the values of a file `RonAnalysis` rejects, the whole file refused); `crlf_stable_pct` over `hashed`, against an in-memory CRLF copy, matched by comment order. Over markers: `bound`, those whose comment has a value hash in the original and in a copy with every comment holding no marker turned into one space, paired in marker order; `bound_stable_pct` over `bound`, `null` iff `bound` is 0. A marker losing its value hash in that copy counts neither as bound nor as unstable: it shows only as a lower `bound`.

## Runs and cost

- A repeated fixture run gives the same JSON but its times (`wall_ms`, `detail.hash_ms`, `levels.cost`); `fixtures/ast-hash/expected.json` and `fixtures/ron/expected.json` pin every count.
- `ast-hash` re-parses each file 3 more times (two neighbour passes, the rename). `ron` lexes and walks each file 3 more times (`comment_anchors`, `value_hashes`, the CRLF copy's `value_hashes`), plus 2 for a file with a bound marker (the stripped copy).
- **Pilot run** (`tests/hash_levels_eval.rs` `te1_pilot_a`, `_b`, `#[ignore]`, release asserted): the test makes a scratch HOME (the child's cwd) and gives the child only `PATH`, `SPECENGINE_PILOT_A` / `_B`, and `SPECENGINE_RUSTFMT`, `SPECENGINE_CARGO` as absolute paths (from the parent, else `rustup which` at `/`: rustup's proxies find no toolchain under a scratch HOME), with `--timeout 1800`; `ast-hash`, then `ron`. It counts only with rustfmt available and `levels.hashed` ≥ 1000, needs zero `levels-unstable.json` rows (the count printed) and every judged figure at 100.0 (`stable_pct` but `quote`, `module_stable_pct`, `rename.kept` = `renamed` but `quote`, `crlf_stable_pct`, a non-null `bound_stable_pct`), prints the rest, and proves the pilot read-only (`git --no-optional-locks status`, a (path, kind, size, mtime) listing, before and after). Build first with `--no-run`, then run under `perl -e 'alarm 7800; exec @ARGV'` (4 children × 1800 s plus a margin).

## Measured

- Fixtures (pinned): 66 items hashed, 3 `cannot_verify`; modules 6 and 2; every judged figure 100.0 in all four rows; `neighbour` 66 edited, 66 changed, 52 checked; trivial 63 (95.5 %); body groups 0; in-file skeleton groups 3 (10 items); 59 renamed, 3 followable. `ron`: 15 anchored, 15 hashed, 8 bound, both percentages 100.0.
- This repository (developer run, release; 607 files, 14 534 items; unstable rows not counted): every judged figure printed 100.0; `neighbour` 14 524 edited, all changed, 12 416 checked; trivial 45.2 %; 147 body groups (379 items); `cost` levels 3.89 ms/kLOC, v2 1.72, within 6.67.
- v3 cost, a developer run (release, this repository, 325 kLOC): `bind_tree` plus every item's and module's levels ≈ 4.9 ms/kLOC (1.61 s; v2's `analyze_tree` 0.56 s).
- Pilots: TE-1, the owner's run (`hash-levels-eval.md` "Implementation"); it gated 5b-1 (`lock-levels.md`) and gates AC-15's 124 before 5c. A judged figure below 100.0 there is a 5a-1 recipe change first (ADR-0050). First run (2026-10-19): pilot A's `module_stable_pct.body` 79.6 under both rustfmt passes, 80 rows (40 file modules × 2); pilot B not run. Cause: rustfmt sorts `use` list members; routed to 5a-1c, N6 (`hash-levels-use-lists.md`). Rerun (2026-10-20, `e07c352`, after N6, release, rustfmt 1.9.0; A / B): 11 937 / 7 599 items, 196 / 190 files and modules, `cannot_verify` 0, 0 rows, every judged figure 100.0, `module_stable_pct` included; `quote` (`fmt_default`, `fmt_contrast`, `comments`, `neighbour`) 88.6, 39.7, 4.8, 92.8 / 100.0, 34.6, 23.3, 62.1; `neighbour` edited = changed 11 937 / 7 599, checked 10 194 / 6 198; trivial 5 843 (48.9 %) / 3 124 (41.1 %); `trivial_at` (items, body twins) at 64: 4 057, 1 574 / 2 156, 573; 96: 5 152, 1 217 / 2 769, 444; 124: 5 843, 986 / 3 124, 407; 160: 6 547, 775 / 3 614, 280; 256: 7 827, 357 / 4 387, 127; body groups 246 / 107 (986 / 407 items), in-file skeleton groups 611 / 288 (2 381 / 1 009 items), `quote_separates` 757 / 357; renamed 11 273 / 7 131, followable 4 666 / 3 717, `kept` = renamed but `quote` 11 033 / 5 233; `cost` levels 4.09 / 4.17 ms/kLOC, v2 1.95 / 1.88, within 6.67; `ron` 6 301 / 2 098 anchored, 6 234 / 2 098 hashed, 67 / 0 `cannot_verify`, `bound` 0, `crlf_stable_pct` 100.0; `wall_ms` `ast-hash` 32 977 / 22 284, `ron` 106 / 44; read-only proof equal (0 / 1 `git status` lines, B's pre-existing untracked file; 4 037 / 1 037 listed entries; the scratch HOME left empty). TE-1 passed: 5b-1 opened; the sweep recorded, 124 unchanged.

## Known limits

- Neither pilot holds a marker in `.rs` or `.ron`: `bound` is fixture evidence only.
- The vacuous neighbour passes above widen `checked`, the rounding's denominator.
- The edit-point lookup is a small node walk of the eval's own; a library accessor may replace it.
