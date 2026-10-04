---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, fixtures, docs]
ref: analysis 2026-10-04, owner's Q1-Q5 and counts; spec-parser AC-15, spec-cli-bundle AC-17
shipped: 2026-10-04
adrs: []
---

# Token calibration: estimates in real tokens

## Why

`tokens_est` weights were a guess: a "2 000-token" bundle was ≈ 2 670 real tokens of English, 3 120 of code, 3 860 of a table. Calibrated, budgets mean real tokens.

## Data

Canon: core README "Token estimator". `fixtures/token-calibration/reference.json`: `claude-opus-5-5`, 2026-10-04, `count_tokens`, overhead 8 (baseline 9), counts en/ru/mixed/code/table 72/86/58/142/110. Fit: least worst relative error, multiples of 50, WHITESPACE 150 held (Q2), sum ≥ reference, CYRILLIC > ASCII_ALNUM; one optimum.

## Acceptance criteria

- [x] AC-01 `reference.json` as above; asserted: `counts + overhead = input_tokens`, `overhead = baseline − 1`, five equal names. M: table 111.
- [x] AC-02 The calibration test runs un-ignored, green. M: old weights; `chars / 4`.
- [x] AC-03 Each estimate in `r*85/100..=r*115/100` for r = reference − 1, +0, +1; sum ≥ reference sum. M: CYRILLIC 500 (mixed 66); ASCII_ALNUM 330 (sum 464).
- [x] AC-04 AC-16 tests unchanged, green. M: WHITESPACE 0; CYRILLIC ≤ ASCII_ALNUM.
- [x] AC-05 Six constants, the fitted values, a reason each; module doc names model, date, `reference.json`.
- [x] AC-06 `INDEX_FORMAT` 7, `7:` note; history: six lines verbatim + `7 <hash>`; pins, comment say 7. M: stamp 6; an old line edited.
- [x] AC-07 Test and fixture diff: the files below only; `cargo nextest run --workspace` green once, calibration not skipped.
- [x] AC-08 Core README: weights, model, date, no Q4; store README 7; `spec-cli-bundle.md`, `spec-cli-graph.md` rewritten; root README, 08 "Next": `pilot-w`; spec-parser AC-15, spec-cli-bundle AC-17 ticked; `rg -i "uncalibrated|calibration itself is pending|counts pending|stays (6|six)" -g '!docs/features/**'` empty.
- [x] AC-09 Gate clean, worst W ≤ 109 484 B; canon ≤ 12 288 B; core README ≤ 9 980 B; spec ≤ 3 KB; clippy, fmt.

## Implementation

One iteration, accepted. Core `tokens.rs`: ASCII_ALNUM 350, ASCII_OTHER 1400, CYRILLIC 450 (were 270, 500, 500), reasons, module doc; store `lib.rs`: `INDEX_FORMAT` 7. Estimates en/ru/mixed/code/table 72/85/64/155/99 (0, −1.2, +10.3, +9.2, −10.0 %), sum 475 vs 468.

Tests: `reference.json` filled; core `tokens.rs`; store `format.rs`, `format_history.txt` (+ `7 af07f567…`); CLI `bundle_config.rs`, `bundle_fit.rs`, `bundle.rs`; MCP `mcp_index.rs`. Workspace 1 424/1 426; eval `import_cli`, `layout_cli` (clean `fixtures/`) green after commit.

Deviation (AC-07 widened): two more tests pinned estimates. `bundle_fit.rs` AC-09: filler 0..300 (~1 900 tokens), `--budget` 1500 (was 1000), comment; asserts kept. `bundle.rs` AC-05: kinds renamed to their ROT13 (same lengths and classes: weight-proof), mutation red. Not fitted: OTHER_LETTER, REST; ID-dense text ≈ 10 % under; less text per budget (×1.33 English, ×1.70 code).
