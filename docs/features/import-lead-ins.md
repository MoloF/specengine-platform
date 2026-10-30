---
class: spec
status: shipped
scope: [crates/specengine-import, crates/specengine-eval, fixtures]
ref: RM-0013 slice 2; analysis 2026-10-22, decisions owner pre-approved, review open; 08 §2 Phase 5, §3 AC-6
shipped: 2026-10-30
adrs: [ADR-0008, ADR-0009, ADR-0012, ADR-0026]
---

# import-lead-ins: wrapped lead-ins, lead-ins not taken, the walk kept (RM-0013, slice 2)

## Why

After slice 1 (`docs/features/import-homes.md`), pilot A's migration still needed two hand fixes: V1, a strong lead-in wrapped onto the item's next line gives no record (17 joined in the source), unreported, as a clean hash result counts only what was recognised; B4, the emitted `specengine.toml` took `roots` from first path components and wrote `exclude = []`. This slice reads wrapped lead-ins, counts ID-led ones not taken, and keeps the before walk. Nothing is written into the pilot.

## Data

No config key, CLI flag, MCP, UI, hook, CI, core change or ADR (ADR-0008). Now canon: `docs/canon/import-lead-ins.md`; output in `docs/canon/import-layout-verifier.md` "Output" and the import README "Before report".

### Emitted `[paths]`

The before walk, kept: `docs/canon/import-lead-ins.md` "Emitted `[paths]`". Existing fixtures: `import-layout/one`, `three` unchanged; `two`, `pilot-w/two` `roots = ["log", "pages", "ledger", "topics"]` (was sorted).

### Exclude and index outputs

Copying the before `exclude` verbatim hid the layout's own index outputs and moved `pilot-w/one`'s figures (iteration 1). So a pattern matching an index output the emitted scheme writes is dropped and named by a comment right above `exclude` (owner). `pilot-w/one` emits:

```toml
# exclude dropped: "book/index*.md" matches an index output this scheme writes
exclude = []
```

### Fixture `fixtures/import-layout/four`

A kiln log. Census roots `kiln/log`, `kiln/rules`; the scheme adds `kiln/GUIDE.md`, which the tree lacks; both exclude `kiln/log/old/**`; `records` `kiln/atoms` lies outside the roots, `features` `kiln/rules/glazes` inside. Below, `firings` is `kiln/log/firings.md`, `limits` `kiln/rules/limits.md`, `celadon` `kiln/rules/glazes/celadon.md`. Each `text` is the closing line after `**: `; KF-21's adds `\n  The hold is logged.`

| ID | Path, line | Title | Extent |
|---|---|---|---|
| KF-21 | firings 3 | `Cone ten reached` | 3-5 |
| KF-22 | firings 6 | `A soak that spans three lines`, box `true` | 6-8 |
| KF-23 | firings 9 | `Parent rule` | 9-9 |
| KF-24 | firings 10 | `Nested and wrapped` | 10-11 |
| KF-28 | firings 12 | `Lazy wrap` | 12-13 |
| KF-27 | firings 19 | none | 19-19 |
| KF-12 | limits 3 | none | 3-3 |
| GZ-01 | celadon 3 | `Even sheen` | 3-4 |

`not_taken.json`: firings 14 `**KF-25: Never closed`; firings 18 `**KF-26: Cut by the next item`; limits 4 `**KF-17a.**`; limits 5 `**KF-12 (part)**`. The emitted `tree/specengine.toml` `[paths]`, exactly:

```toml
exclude = ["kiln/log/old/**"]
features = "kiln/rules/glazes"
records = "kiln/atoms"
roots = ["kiln/log", "kiln/rules", "kiln/atoms"]
```

## Rules and edge cases

Canon: `docs/canon/import-lead-ins.md` "Wrapped spans" (the lines ending the search), "Titled lead-ins", "List item text", "Not taken".

## Acceptance criteria

Mutations M1-M8, plus M9 by the owner's decision of iteration 1. Phase B: all nine killed ("Implementation").

- [x] AC-01 — V1 on `four`: `import` `records.json` holds the table (8 `list-item` records, 6 titled), none at firings 14, 18; `layout` `hashes.matched` = `before.definitions` = 8, `titles.matched` 6, `prose.mismatched`, `extents.residue`, `reasons.unexplained` 0; celadon's residue holds `## Even sheen {#GZ-01}`. `tests/import_lead_ins.rs`: `__`; a fence, heading, table row, block quote ends the search; a comment line continues it; blank content on the item's line is no lead-in; a hyphenless match at a wrapped lead-in defines. M1 the span closes on the item's line only; M2 a list-item line does not end the search (KF-26 a record); M8 the nested-record test reads one line (KF-23's extent 9-11).
- [x] AC-02 — not taken: `import` on `four` `records.not_taken` 4, `not_taken.json` the four entries above; `layout` `before.not_taken` 4, the same file; their lines stay in the residue byte for byte; a copy with `lead_in = false` gives 0. M3 the count always 0.
- [x] AC-03 — B4, `layout` on `four`: `tree/specengine.toml` `[paths]` is the block above and loads; observe `observed`, enforce `clean`, `stale`, `emitter_findings` 0; `index.files` = `tree.documents` = 10 (7 record files, 3 residues). M4 roots from first path components (the old code); M5 `exclude = []`; M6 records outside the roots not added (`index.files` 3); M7 a root the tree lacks kept (`kiln/GUIDE.md`: the tree check names a missing root).
- [x] AC-04 — `import-one`, `import-two` `expected.json` add only `"not_taken": 0` under `records`; `import-layout/{one,two,three}` only `"not_taken": 0` under `before`; `pilot-w/{one,two}`, `corpus-mini` unchanged (W and layout figures equal the base commit's); the emitted `[paths]` above; whitelists 51 → 52. The guards `census_and_import_outputs_are_unchanged` and `homes_ac04_without_the_keys_output_is_as_before` are red until the owner commits, as in slice 1.
- [x] AC-05 — "Exclude and index outputs", `layout_cli::lead_ins_an_exclude_matching_an_index_output_is_dropped_with_a_comment`: a pattern matching an index output is dropped, its comment exact, directly above `exclude`; a non-matching one is kept; `pilot-w/one` emits the block above. M9 a pattern matching an index output kept.
- [x] CR — iteration 1 accepted: the genre scan forbids `four`'s config strings and run prefixes; `four` shows `not_taken` non-zero; output additive only; no pilot literal.
- [x] Gates, proven — fmt and clippy exit 0; `anonymity`, `doc_pointers`, `build_graph` green inside the run below; no `Cargo.toml` or `Cargo.lock` diff.
- [ ] Gates, open — `cargo nextest run -p specengine-import -p specengine-eval`: 613 run, 611 passed, 34 skipped, 2 failed: the two guards of AC-04, green once the owner commits the fixtures.
- [x] SW — this spec; `docs/canon/import-lead-ins.md`; `import.md`, `import-layout.md` "Emitted configs", `import-layout-verifier.md` "Output"; the import README; RM-0013, 08 §2, `CLAUDE.md` State.
- [ ] OC-1 — the owner's, open: in scratch `git clone`s of pilot A (no hash in the repository), the script's census config plus `home`, `inherit`: before the source fixes, each wrapped lead-in V1 names is a `list-item` record and `records.not_taken` = `before.not_taken` = 2; before the migration, the emitted and the committed scheme walk equal file sets of `tree/`; scratch `HOME` empty; no pilot-side code.

## Risks

- A kept exclude can hide a directory the scheme itself adds as a root (`exclude = ["docs/**"]`, `records = "docs/records"`): `index.files` < `tree.documents` shows it, `hashes.missing` does not, the verifier also reading the files the walk misses; not dropped automatically (owner).
- A setext `===` underline is crossed as paragraph text, though CommonMark reads a heading; HEAD's `item_continuation` has the same gap; very rare (owner). An HTML tag line or an ordered marker other than `1.` ends a span conservatively; an ID-led item there lands in `not_taken`.
- Wrapped bold prose opening with an ID becomes a record (in `records.json`); a wrapped title is not the source's bytes, its lines lie in the extent.

## Out of scope

Paragraph lead-ins; wrapped headings or rows; a dropped root's diagnostic; the census; writes into pilot A; check parity (RM-0039, RM-0040); markers; pilot B.

## Implementation

| Module | What it does |
|---|---|
| import `src/import/lead_in.rs` | `list_span`, `ListSpan` (closing line, joined text, `title`), `crossed_text`, `opened_lead` |
| import `src/import/mod.rs` | `list_items`, `list_lead_in`, the nested-record test by line index, `text_tokens`; `ListItem::NotTaken` |
| import `src/import/ids.rs` | `like_at`: the `not_taken` test, unbounded after the match |
| import `src/import/model.rs` | `NotTaken`, `Import.not_taken` |
| import `src/layout/scheme.rs` | `Scheme::parse(text, roots, exclude, hides_index_output)`; `emit_scheme` |
| import `src/layout/toml_out.rs` | `write_commented`, `Comment`; `basic_string` shared |
| eval `src/import.rs` | `Records.not_taken`, `not_taken.json`, stderr |
| eval `src/layout.rs` | `Before.not_taken`, `not_taken.json`, the walk and `hides_index_output` into `Scheme`, `cut_wrapped` in `extent_residue` |

Tests: import `tests/import_lead_ins.rs`; eval `layout_cli.rs`, `import_cli.rs`, `import_genre.rs`.

Phase B: M1-M9 all killed, each by at least 2 tests beyond the two commit guards; M4 and M5 only by the exact `[paths]` pins, as no figure of `four/expected.json` moves under them. Gates: fmt and clippy exit 0; `cargo nextest run -p specengine-import -p specengine-eval` 613 run, 611 passed, 34 skipped, the 2 failures being the guards `census_and_import_outputs_are_unchanged` and `homes_ac04_without_the_keys_output_is_as_before`, green once the fixtures are committed; `anonymity`, `doc_pointers`, `build_graph` green inside that run; no `Cargo.toml` or `Cargo.lock` diff.

Iteration 1, review accepted. A span closed on its line reads exactly as at HEAD, so existing corpora read as before; CRLF and BOM are safe. The V1 terminators are shared by records, the nested-record test, the hyphenless position and `not_taken`, all through `list_span` and `crossed_text`. An ID crossing a break is dropped. The drop rule uses core's `WalkScope::is_excluded` on a one-glob copy; the import crate has no core dependency, a predicate is passed in. The comment is well-formed TOML through `basic_string`. `cut_wrapped` is bounded.

Deviations from the draft, all in the canon: exclude patterns matching an index output (owner, M9); no duplicate root; `records`, `features` tested against tree paths with index outputs; the `like` test unbounded after the match; setext underlines open nothing. Open: OC-1 of this slice and of `import-homes`, the owner's.
