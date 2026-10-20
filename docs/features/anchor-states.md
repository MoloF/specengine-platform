---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5c (WA-1..WA-8; Q1-Q16 2026-10-12, 2026-10-16; Q17-Q23, S1-S20, the R(b) rule, M1-M8 2026-10-20; the readings of 2026-10-20 at shipping); all decided under the owner's pre-approval, review open; the alternatives and the full plan in this file at ab856cc; 05 s5.2, s5.3; 08 s2, AC-15; developer 2 iterations, review 1 accepted
shipped: 2026-10-20
adrs: []
---

# anchor-states (5c): a renamed or moved binding judged against its old lock entry

## Why

Before 5c a function renamed or moved with its marker gave two rows: the old key `broken_binding symbol_gone|file_gone` (Fix), the new key `unlocked` (Check). Under `enforce-introduced` a pure rename exited 1, on disk and in the `--staged` hook alike; `spec propose drift` raised a `high` item; the agent got a Fix ("broken by the change", ADR-0042) for a refactor that broke nothing; the owner could not tell a rename from a deletion plus new code. A manifest change renames qpaths with no code edit (`code-identity.md` "Known limits"), and so does an in-file twin (`symbol-names.md` "Frozen names").

Delivered: a read-only pairing inside verify's one judge, `judge_side`, on every side (Q7). A lock entry whose key no marker gives any more is paired by fingerprints, one to one, with a marker binding of its group that has no entry; the two are one row at the new key, judged against the old entry by 5b-1b's levels: a pure rename `ok`, a rename with edits `code_ahead code_changed <levels>` or `conflict`, a `[path]` binding `code_changed path`. The anchor answers "where is it", `sync` "does it still match" (05 s5.2). No writer, no new `state`, no lock byte, nothing stored but a drift item's evidence, no ADR: ADR-0020 (only marker bindings are followed), ADR-0042 (nothing unverifiable is followed or `ok`), ADR-0046 hold; ADR-0041, ADR-0050 stand (accept re-keys); ADR-0012 is helped. Following code with no marker would amend ADR-0020 (12b).

## Description and interactions

As built, in canon: `docs/canon/spec-verify-anchors.md` (the pairing, anchors per row, the merged row, drift evidence, output); `spec-verify.md` "Rows", "Cascade", "Known limits"; `spec-verify-output.md` "Text", "Hints", "JSON"; `proposal-drift-item.md` "The item" (place 18); `proposal-drift.md` "Known limits"; `code-hash.md` "Fingerprints"; `spec-lock.md` "Entry"; `symbol-names.md` "Frozen names"; `code-identity.md` "Known limits"; `architecture-code.md#verify`; the core and CLI READMEs. Unchanged: the code crate, store, MCP (drift rows pass untyped), HTTP, UI (a drift card prints each row as JSON), plugin, the gate, `spec check`, the index, the queue's schema, `spec.lock`'s bytes, `specengine.toml`, every earlier fixture, `Cargo.toml`.

Cited from outside: **Q13** core API, an opaque group key and digest records, no node or relation type, so 7a (`bundle-bindings.md`: `anchor`, `moved_from`) and 7b (`impact.md`) call core `anchor::pair` as is; **Q14** 5c fills `delta.bindings.moved` only, `symbols.renamed` stays 7b's on Q13's function. 6b-2 (`verify-node.md`, its Q34) inherits the pairing through `judge_side`, before its filter.

## Data

Pointers kept for the citations into this spec.

### Anchors per row

`docs/canon/spec-verify-anchors.md` "Anchors per row" (Q3, Q20: `lost` for every entry row with no binding that is not moved or ambiguous; null without an entry or on `cannot_verify`).

### Output

`docs/canon/spec-verify-anchors.md` "Output"; the forms in `spec-verify-output.md` "Text", "Hints", "JSON" (Q5, Q16, Q21).

### Readings

Decided at shipping under the owner's pre-approval, review open, the developer's readings as the review judged them: `exact` covers an entry and a marker at one key in any state but `cannot_verify`, a gone node's dangling marker sharing its entry's row too (that row is `bad_marker`); only code-side lockability selects B, so spec-side states (`spec_ahead marker_behind`, `predated`) stay on the merged row; an `ok` moved row's `levels.changed` is `[]` (`compared` `ast`); `delta.bindings.moved` covers rows not Pre-existing, as `stale` does, so a committed moved `code_ahead` row is absent under `--changed` while an `ok` move is listed every run (Q21); the moved hint has no backticks. Iteration 2, decided 2026-10-20 under the same pre-approval: (1) E prints with `body_bytes` under T are left out of B's selection, so a trivial entry selects and hashes no binding (`pair` returns `Lost` for it either way; the review's minor); (2) a binding in a file whose tree has any `ERROR` or `MISSING` node is never a candidate (ADR-0042: its place is the parser's recovery), so an unclosed `impl X {` re-parenting a locked fn reads `broken_binding symbol_gone` `lost` plus `unlocked` at the new key. Spec gaps the tester found: the lagging-marker scenario raises the node's `rev` after locking (accept refuses a lagging marker); the manifest-only rename adds `src/lib.rs` (the CLI reads only `[package] name`); a fixture corpus needs ≥ 5 `.md` files (the store's own-text oracle); `drift_raise.rs` `without_levels` drops `anchor` too.

## Rules and edge cases

- WHEN an entry's key is given by no marker, the system SHALL follow it only to a marker binding of its group by a mutual unique match, and SHALL never follow a trivial body (either side), RON, `marker_gone`, `node_gone`, a cannot-verify fact or a binding in a file with a parse error (05 s5.2: "a missed follow heals as STALE, a false follow lies forever").
- WHEN verify judges a side, it SHALL pair on that side alone inside `judge_side`, so a committed pure rename reads the same under `--changed`, `--staged` and a sweep (Q7); `is_pre_existing` (key, state, category) is unchanged.
- WHEN a pair is MOVED, verify SHALL give one row at the binding's key judged against the entry, and SHALL write nothing: no lock byte, index row, queue row or file (ADR-0046).
- WHEN only `path` changed on a moved row, its drift `code` SHALL be null, the move carried by place 18 alone (Q8).
- WHEN a row cannot be verified, its anchor SHALL be null and it SHALL never be `ok` (AC-14).
- WHEN a side has no non-trivial E, verify SHALL hash no binding beyond the cascade's.
- WHEN run from two roots, another cwd, `core.autocrlf=true` or over reversed entries, rows, anchors and output SHALL be byte-identical.
- The pin `nothing_outside_the_binder_reads_an_items_state` (`code_refs_walk.rs`) stays green unedited; no stack, kind or prefix in the pairing, rows or hints (ADR-0008); core gains no tree-sitter; CLI sources cite canon by name, never `docs/` (`genre.rs`).

## Acceptance criteria

rust-developer (M1-M8: the mutation that must turn the named test red; tests by the test-engineer):

- [x] RD-1 -- the pairing, pure; core `tests/anchor_pairing.rs`. **M1** uniqueness checked one way -> `contested_candidate_moves_nothing`. **M2** `<=` for `<` -> `body_at_the_threshold_moves` (t), `body_under_the_threshold_is_never_followed` (t-1). **M3** the skeleton matched across files -> `skeleton_step_stays_in_the_entry_file`. **M4** a candidate taken while several remain after the quote -> `twins_with_equal_quotes_stay_ambiguous`. Unmutated: `body_match_wins_over_skeleton` (both sides, the R(b) rule), `twins_with_distinct_quotes_each_move`, `trivial_candidate_is_never_followed`, `candidates_share_the_group_key`, `pairing_ignores_input_order`.
- [x] RD-2 -- the merged row; CLI `tests/verify_anchors.rs`. **M5** e's row kept beside the merged one -> `moved_binding_is_one_row`. **M6** `path` not added -> `path_marker_reads_a_move_as_code_changed`. Unmutated: `exact_path_binding_stays_ok`, `renamed_and_edited_is_code_ahead`, `path_change_adds_no_code_fact` (null, never `""`), the never-followed cells (RON, `marker_gone`, `node_gone`, `hidden_by_error`).
- [x] RD-3 -- evidence and output. **M7** place 18 left null -> `moved_row_hash_carries_moved_from` (`drift_raise.rs`, the hash rebuilt by hand). Unmutated: `delta_lists_moved_pairs`, `delta_lists_are_disjoint`; text and JSON goldens in `verify_anchors.rs` (the sub-lines, ` (m moved)`, the hint's place, 17 keys).
- [x] RD-4 -- cost. **M8** the selection takes every unlocked binding -> `lone_entries_select_no_binding` (`#[cfg(test)]` in `verify_rows.rs`, over the pure B selection). Guards: one `.parse(` in `verify_rows.rs` outside `#[cfg(test)]` (a static scan); `verify_output.rs` `verify_never_writes_the_lock`, `verify_writes_no_queue_row` green.
- [x] RD-5 -- guards, no mutation: the pin green unedited; eval `build_graph.rs` `core_and_store_have_no_tree_sitter`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; core and CLI `genre.rs`; every other `verify_*` suite green but the churn ("Implementation").

test-engineer:

- [x] TE-1 -- `fixtures/anchor-states/` with `expected.json`, run by `verify_anchors.rs`: each scenario in a temp git repo copied from it, locked by `spec lock accept <ID>`, committed, then changed; no digest hand-written; the T-1 and T bodies asserted through `item_levels` against `specengine_code::TRIVIAL_BODY_BYTES`. Per scenario every row's state, class and anchor pinned (untouched bindings `ok` `exact`):

| Scenario | Expected rows |
|---|---|
| rename within the file | one `ok`, `moved` |
| move to another file, the old file kept | one `ok`, `moved` |
| rename plus a local renamed (skeleton step) | one `code_ahead code_changed body`, `moved` |
| move across files plus an edit | `broken_binding symbol_gone` `lost`; `unlocked` null |
| bodies of T-1 and T `body_bytes`, renamed | T-1: `broken_binding symbol_gone` `lost`, `unlocked` null; T: `ok` `moved` |
| twins, distinct quotes, both renamed | two `ok`, `moved` |
| twins, equal quotes, both renamed | two `broken_binding` `ambiguous`, 2 candidates each; two `unlocked` null |
| a contested candidate (two entries, one binding, no quote equal) | two `broken_binding` `ambiguous`, 1 candidate each; one `unlocked` null |
| a marker moved, the old item stays | `broken_binding marker_gone` `lost`; `unlocked` null |
| a locked impl gaining an unmarked in-file twin (`impl Tally` -> `impl Tally #1`) | one `ok`, `moved`, by `body` |
| an ordinal shift: an unmarked twin inserted above two locked twins of one node | never paired: the old `#1` `broken_binding marker_gone` `lost`; the old `#2` compared with the twin now there, `exact`; `#3` `unlocked` null |
| a RON path renamed | `broken_binding marker_gone` `lost`; `unlocked` null |
| a `//!` file renamed | `ok`, `moved`, kind `module` |
| a `[path]` binding moved | `code_ahead code_changed path`, `moved`, `levels.changed` `["path"]`, drift `code` null |
| a rename by the manifest only (a lib added) | every bound item of `main.rs` (bodies ≥ T) `ok`, `moved` |
| a lagging marker, renamed | `spec_ahead marker_behind`, `moved` |
| an ID renamed to an alias | `broken_binding node_gone` `lost` (Q20); `unlocked` null |
| a symbol gone from a file with an orphan parse error, its marker on a renamed copy | `cannot_verify hidden_by_error` null; `unlocked` null |
| an unclosed `impl X {` above a locked fn (iteration 2) | `broken_binding symbol_gone` `lost`; `unlocked` null at the new key |

- [x] TE-2 -- AC-15 end to end under `enforce-introduced`, every side: a pure rename uncommitted, `spec verify --changed` exits 0 (before 5c: 1); staged, a `--staged` hook exits 0; committed, `spec propose drift` stores nothing (MCP `report_drift` runs the same command); a committed rename plus an edit: the moved `code_ahead` row is Pre-existing under `--changed` and `--staged`.
- [x] TE-3 -- `verify_output.rs` `verify_is_byte_identical_across_roots_and_order` gains a moved and an ambiguous scenario: two roots, the lock's entries reversed, `core.autocrlf=true`; JSON and text byte-identical.
- [x] TE-4 -- read-only after every TE-1 scenario: `spec.lock` bytes, `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs unchanged.

spec-writer:

- [x] SW-1 -- this spec (2026-10-12), refreshed 2026-10-16 and 2026-10-20 (against 55709ac); `code-hash.md` "Triviality"'s pointer (Q18); 08 s2.
- [x] SW-2 -- at shipping (2026-10-20), bytes before -> after (caps: Tier 2 12 288, Tier 1 10 240, Tier 0 16 384): new `docs/canon/spec-verify-anchors.md` 3 584 (budget 3 584); `spec-verify.md` 11 958 -> 12 099 ("Rows", "Cascade" step 3 and its last sentence, "Known limits"); `spec-verify-output.md` 9 603 -> 10 474; `proposal-drift-item.md` 4 458 -> 4 594; `proposal-drift.md` 8 944 -> 9 022; `code-hash.md` 12 223 -> 12 218; `spec-lock.md` 10 121 -> 10 133 (2 155 left for 8b); `architecture-code.md` 11 956 -> 12 072; CLI README 9 957 -> 9 979; core README 10 139 -> 10 124; `symbol-names.md` 8 667 -> 8 663; `code-identity.md` 10 471 -> 10 464; `code-hash-eval.md` (tense); `CLAUDE.md` 6 022 -> 6 039; 05 29 054 -> 29 046 (s5.2 `MOVED`, `AMBIGUOUS`, the fingerprint paragraph, the reply's `moved`; s5.3 `broken_binding`); 08 s2, AC-15 reworded (Q17: "copies the fingerprints cannot tell apart (one body or skeleton, one quote) yield `AMBIGUOUS`"); `verify-task.md`, `verify-node.md`, `bundle-bindings.md` re-pointed to the canon; this spec compacted.
- [x] SW-3 -- no ADR ("Why"); `export index`, `check` clean.

## Out of scope

Following a lost marker (`marker_gone`) or code without a marker (12b); ID renames (`node_gone`: accept drops the gone node); RON; storing anchors but as evidence; the pairing in accept or rebase; `symbols.renamed` (7b's, Q14); `moved from` in `spec review`'s text and follow-up goals (Q22); evidence following a move (8b); `--deep`; UI.

## Later

A hygiene pass, not a scheduled slice; the review's nits: `judge_side` borrows `building`, so every row's `RowKey`, markers and entry are cloned on every side, the no-E path too (drain it by value); `select_bindings` builds `entry_key()` and runs `key_problem` before the cheap group test.

## Implementation

| Module | What it does |
|---|---|
| core `src/anchor.rs` (new), `src/lib.rs` | `Print`, `Follow`, `pair(entries, bindings, trivial)`: C0, the quote rule, R(b) with body precedence, over sets |
| core `src/drift.rs` | `HashedRow.moved_from` (`MovedFrom`), place 18 |
| CLI `src/verify_rows.rs` | `judge_side` pairs per side: `lone_print` (E), pure `select_bindings` over `group_of` (B, from non-trivial E only, none in a broken file: `Made.broken`, `in_broken_file`), B's units in `cascade`'s one `made()`, `binding_print`, `moved_cascade` (`path` first), `digest_fact` (null code fact), `Anchor`, `first_anchor`, `Row.anchor`; e's row dropped |
| CLI `src/verify.rs`, `src/lib.rs` | `VerifyAnchor`, `VerifyPlace` (exported), the 17th key, `place_line` sub-lines, ` (m moved)`, `MOVED_HINT`, `delta` `moved`, `ambiguous`, `lost` |
| CLI `src/drift.rs` | `hashed_row` fills `moved_from` |
| tests | new core `anchor_pairing.rs`, CLI `verify_anchors.rs`, `anchor_common/`, `fixtures/anchor-states/`; edited `verify_common/mod.rs`, `verify_output.rs`, `verify_staged.rs`, `verify_levels.rs`, `drift_raise.rs`, `drift_approve.rs`, `verify_rows.rs`' `#[cfg(test)]` |

**Churn**, corrected at shipping: `verify_common/mod.rs` `ROW_KEYS` 16 -> 17 and `row_json` gains `anchor` (it compares whole rows: 36 tests, the four rename tests of `verify_states.rs`, `drift_raise.rs`, `symbol_twins.rs` included, green through it unedited); `drift_approve.rs` `rows_of_a_later_build` (`anchor` now follows `levels` in a stored row); `verify_output.rs` `moved` null -> `[]`; the `HashedRow` literals of `verify_levels.rs`, `drift_raise.rs`.

Deviations: `unbound()` now runs before the cascade, rows and run causes unchanged; a merged row that would be `cannot_verify` keeps a null anchor (unreachable: b and e resolve to one node). Iteration 2 ("Readings"): the triviality filter on E, the broken-file rule on B; every CLI and core citation re-pointed from this spec to ``canon `spec-verify-anchors` `` and its five headings.

Tests: iteration 1, nextest core, code, CLI, MCP, HTTP, store 2 914 passed, 12 skipped; eval `build_graph`, `doc_pointers`, `anonymity` 58 passed; iteration 2's suites 279 passed, 2 skipped, `doc_pointers` 10; clippy clean on both. New: core `anchor_pairing.rs`; CLI `verify_anchors.rs` (a test per TE-1 row, TE-4 inside each, TE-2, RD-2, RD-3's goldens); `verify_rows.rs` `selection_tests`; `fixtures/anchor-states/` (a kite crate, a binary-only tool crate, a RON asset, 5 docs, 19 nodes with a never-accepted decoy; `expected.json` baseline + 18 scenarios). Mutations 8 of 8 killed by their named tests; M2 also by `threshold_bodies_follow_only_from_the_threshold`, M6 by `path_change_adds_no_code_fact` (the S7 `""` too), M7 core and CLI, M8 re-run red on iteration 2. Iteration 2's rules, not counted: the broken-file skip dropped turns `unclosed_impl_above_a_locked_fn_is_never_followed` red (`ok` `moved` again); the trivial-E filter dropped, `trivial_entries_select_no_binding` (a static scan: no observable output).
