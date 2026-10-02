---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-store]
ref: 08 §2 Phase 1, CLI pass 4 of 5; 05 §6; 07 §1, §2, §5; owner's answers Q1-Q8, 2026-10-01
shipped: 2026-10-02
---

# spec bundle and bundle_hash

## Why

An agent assembles a task's context from repeated `spec search`, `show`, `graph` calls, and the steps grow with the corpus while W must stay O(1) (`docs/canon/documentation-system.md` §1). The bundle (05 §6) is one call that returns the needed context within a fixed budget and names the rest by ID for follow-up reads (04 §2). MCP `get_context_bundle` (07 §1.2, next pass) wraps the same library function; the Phase 2 task package refers to it by `bundle_hash` (ADR-0027). So it is kind-agnostic, within budget, deterministic (08 AC-7), never over either output cap.

No ADR: Q1–Q8 decide inside ADR-0008 (link types and `[ids]` scope, never kinds; budgets from config), ADR-0027 and the decisions pass 3 applies. How it works now: `docs/canon/spec-cli-bundle.md`.

## Acceptance criteria

CLI tests on scratch copies of `fixtures/spec-a`, `-b`, own `HOME`; no git writes. M: the mutation that must turn it red.

- [x] AC-01 spec-a `spec bundle MEC-STAMINA --budget 10000 --json`: targets MEC-STAMINA, text byte-equal to `spec show`'s; ancestors DOM-MOVEMENT, DOM-GAME; decisions DEC-0023 only (DEC-0007 is Tier 3); neighbours A-101, R-12, MEC-SPRINT, each once, MEC-SPRINT `via` `constrains in`, `depends_on out`; terms TERM-exhausted; the other layers `[]`. M: nested sections' links ignored; no live filter; no de-duplication.
- [x] AC-02 spec-b: REQ-001 → open questions QN-07, working answer ASM-01; MOD-CLI → decisions ADR-0001 only (the superseded one is Tier 3); REQ-002 → criteria CRIT-01; REQ-001's Russian text byte-equal to `spec show`'s. M: weak links not followed in layer 2; Tier 3 decisions admitted.
- [x] AC-03 a copy without DEC-0023's `answers:` → MEC-STAMINA's open questions Q-031 (`status open`, working answer A-101); with it → none. `spec bundle DEC-0023` → Q-032 with `status answered` verbatim (its `working_answer:` is DEC-0023, nothing answers it). M: "open" read from `status:`.
- [x] AC-04 R-12's criteria: AC-07, not RULE-STAM-REGEN (it mentions R-12 too); a scratch record with `links: {verifies: [MEC-STAMINA]}` enters MEC-STAMINA's. M: any node mentioning a target a criterion; criteria from `verifies` only.
- [x] AC-05 every `kind` in spec-a's `[ids]` renamed → MEC-STAMINA, R-12, EDGE-SPRINT-EMPTY give the same names per layer, same order; no string literal in the bundle sources equals a fixture kind (whole-literal match). M: a `kind == "…"` in a layer rule.
- [x] AC-06 both fixtures, each of {the minimum, 100, 300, 1 000, 2 000, 10 000} not below the minimum: `tokens` == `tokens_est` of the printed body ≤ `budget`; `chars` ≤ 40 000; every candidate in exactly one place (item, tail line or `more`); the minimum − 1 → exit 2 naming it. M: the budget measured on item estimates alone.
- [x] AC-07 a scratch target ≥ 3 000 estimated tokens with three child sections, `--budget 2000`: `tokens` ≤ 2 000; the target `form: outline`, header marked ` | outline`; the sections in the tail; none of its padding text in the body. M: layer 1 exempt from the budget; a mid-text cut.
- [x] AC-08 thirty neighbours with long summaries, `--budget 100000`: `chars` ≤ 40 000, no `[truncated` line; BLAKE3 of the printed body == `bundle_hash` == the JSON's; JSON `body` == the text's body. M: the CLI cap cutting the bundle; hashing a structure.
- [x] AC-09 1 000 unrelated scratch documents → the same bundle and hash; 1 000 open questions referring to MEC-STAMINA → within budget, the tail at most 20 lines (20 when the room allows), `more` exact. M: an uncapped tail.
- [x] AC-10 every tail entry's tokens == `spec show <name> --json` `tokens_est`. M: the tail priced by the summary.
- [x] AC-11 copies in opposite file orders, at different roots and `HOME`s → byte-identical text and JSON, the same hash; editing DEC-0023's summary changes MEC-STAMINA's hash, a same-line-count edit inside EDGE-SPRINT-EMPTY does not. M: the absolute root or a `HashMap` order in the body; token counts in item headers.
- [x] AC-12 `MEC-NOPE` → exit 1, JSON `reason`; `MEC-STAMINA MEC-NOPE` → exit 1, no bundle; a look-alike ID → exit 2 naming the Latin fix; `project:` → exit 2; `--budget 0`, `x` → exit 2, no JSON; `QST-031` → `Q-031`'s hash; `docs/features/stamina-tuning.md` accepted. M: an unresolvable REF silently dropped.
- [x] AC-13 `[budgets] bundle_node = 300` → `budget` 300; absent → 2 000; `--budget 500` → 500 over it; `bundle_node = 0` → exit 2 at its line; a broken `[classes]` → exit 0, as `spec show`. M: a hard-coded default; the budget read through the whole check config.
- [x] AC-14 both fixtures give the same JSON key sets (top level, `layers`, item, tail entry), every key present, `task: null`; `show`, `search`, `tree`, `graph` key sets unchanged. M: a key omitted when empty.
- [x] AC-15 after a call the copies are byte- and path-identical, new files only under `HOME`, DB tables unchanged; an edit shows in the next bundle without `spec index`. M: `update` skipped; a log table.
- [x] AC-16 `INDEX_FORMAT` 6; store `tests/format_history.txt` unchanged. M: estimator weights changed, the stamp kept.
- [ ] AC-17 Deferred until Q6's counts arrive (canon "Open"): the calibration test un-ignored and green.
- [x] AC-18 eval `build_graph.rs` green: no new dependency edge (the hash is `specengine_store::b3_hash`), no tokenizer crate, no `rusqlite_migration`; a synthetic non-Rust corpus yields a bundle with no P2-3 word as a whole word. M: a "run `cargo nextest`" hint in the bundle.
- [x] AC-19 the gate clean; `tree.rs` green over the committed config and its slug; at shipping worst W ≤ 114 377 B, every Tier 1 README and the root index ≤ 10 240 B, the new canon ≤ 12 288 B. M: the bundle canon appended to the CLI README.

## Implementation

Canon: `docs/canon/spec-cli-bundle.md` (command, layers, fitting, output, `bundle_hash`, exits, API, "Not yet", "Open"); the `bundle_node` bound in `docs/canon/spec-check.md`; pointers in the CLI, core and store READMEs; 05 §6 cut to a pointer and the undelivered parts. Two iterations; the reviewer accepted both; 873 of 873 tests, clippy and fmt clean, every named mutation red.

| Module | What it does |
|---|---|
| core `check/bundle.rs` (new), `mod.rs` | `bundle_layers`, `BundleLayer`, `BUNDLE_LINK_TYPES`, `BundleCandidate`, `BundleLayers` |
| core `check/config.rs` | `bundle_node_from_toml`, `BundleNode`; the check bounds `bundle_node` to 1..=`u32::MAX` |
| core `check/spec_graph.rs` | `SpecGraph::file`, `SpecGraph::scheme` |
| store `lib.rs` | `b3_hash` |
| CLI `bundle.rs` (new) | `bundle`: budget, REFs, fitting, text, JSON, hash |
| CLI `lib.rs`, `main.rs`, `project.rs`, `show.rs`, `cap.rs` | `Outcome::Bundle`, the subcommand, `discover_with_text`, `shown` and `header` shared |
| `specengine.toml` | `[project] slug = "specengine-platform"` (Q8) |

Tests: CLI `bundle.rs`, `bundle_fit.rs`, `bundle_config.rs`, `common/bundle.rs` (new), `common/graph.rs` (`repository_copy` keeps the committed config), `common/mod.rs`; core `bundle_layers.rs` (new), `check_config.rs`.

Deviations from the draft, now canon: the hash lives in the store, so the CLI gains no dependency edge (the draft named `blake3` as its one new edge); layer keys and headings live in the CLI (core's genre scan bans layer 6's word); a header-only target is marked ` | outline` (JSON `header`); no reserve when nothing is left out; only an `answers` written in a live file closes a question; ancestors are filtered one by one; the tail gets the room the items left (at most 20 lines); a merged section follows its outer target's form; with `--budget` the key is not read; JSON `working_answer.path:line` is where it is written in every state (iteration 2); `spec check` bounds `bundle_node` as the bundle reads it (iteration 2).
