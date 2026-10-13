---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4a (WA-1..WA-13; Q1-Q9 decided under the owner's pre-approval, review open, "Open"); 05 s5.3; 07 s2; 08 s2, AC-14; Rust 2 iterations, review accepted
shipped: 2026-10-13
adrs: [ADR-0042]
---

# spec-verify: drift between spec.lock and the tree

## Why

`spec.lock` (slice 3, canon `spec-lock.md`, `spec-lock-accept.md`) recorded what was verified at a commit, but nothing read it back: drift (05 s5.3) stayed invisible, slices 5, 7, 8, 9 lacked a judge, and AC-14's exit 2 was open.

Delivered: `spec verify [--changed] [--json]`, read-only: a state per lock key and marker binding, findings Fix, Check or Pre-existing, exit 0/1/2 under `[verify] mode`; ADR-0042 bounds what a finding may stop (`architecture.md#control`, `architecture-code.md#verify`). Slice 4 split (Q1): 4b `rev-discipline`, 4c `drift-queue`.

## Description and interactions

Canon: `docs/canon/spec-verify.md`, `docs/canon/spec-verify-output.md`; `architecture-code.md#verify`; `spec-lock.md`, `spec-lock-accept.md` "Known limits"; `spec-check.md` "Not checked yet"; the core, store, CLI READMEs. Crates (WA-11): core the pure `[verify]` reader and judge; store `VerifyGit` on the check's base reader, under the caller's `GitEnv`; CLI the command over one walk, accept's key builder (`lock::Target`), `read_lock`. Unchanged: `spec check` (`CHECK_CODES` 35), this repository's hook, CI, `INDEX_FORMAT`, MCP, HTTP, UI, plugin, the gate; no new crate or dependency. Seams (RD-10) for 4b, 4c, 7a, 7b, 9b: (a) core `judge_row` over per-key facts from any source; (b) store `list_commit`, `read_files` for any commit; (c) CLI `pub(crate) judge_walk` over a walked tree and a `read_head` base, never calling `read_tree`. Live-spec lines fixed: 05:160, 05:173, 05:188, 07:104, 08:41, 08:44, 08 AC-14.

## Data

As built in the canon; the headings stay for the pointers in code and neighbouring specs.

### The verify table

`docs/canon/spec-verify.md` "The `[verify]` table" (WA-1).

### Command and rows

`docs/canon/spec-verify.md`, its first paragraph and "Rows" (WA-2, WA-3, Q6).

### States

`docs/canon/spec-verify.md` "States" (WA-4, Q8).

### Cascade and spec side

`docs/canon/spec-verify.md` "Cascade"; R and H in "States" (WA-5, WA-8, Q4, ADR-0040).

### Base

`docs/canon/spec-verify.md` "Base" (WA-7, Q5).

### Verdict and exit

`docs/canon/spec-verify.md` "Verdict and exit" (WA-6, Q2, ADR-0042).

### Output

`docs/canon/spec-verify-output.md` "Text", "Hints", "JSON" (WA-9).

## Rules and edge cases

`docs/canon/spec-verify.md`: what cannot be verified is never `ok`, exit 2 in every mode (AC-14), two broken items never compared; a bound error-free item beside a broken one judged as in a clean file; byte-identical output from any root, cwd or creation order; read-only; one walk per call; "A project's hook" (Q7); "Known limits" (Q9's autocrlf, `ident`/LFS, walk and layer-A limits). No `spec.lock` is no error (WA-13); no stack, kind or prefix in states or hints.

## Acceptance criteria

Tests: core `tests/verify_judge.rs` (11); store `tests/verify_git.rs` (1); CLI `tests/verify_states.rs` (31), `verify_modes.rs` (17), `verify_cannot.rs` (6), `verify_cascade.rs` (6), `verify_output.rs` (8), `lock_deleted.rs` (1), `verify_pilot.rs` (1, `#[ignore]`), `verify_rows.rs` `#[cfg(test)]` (1); `verify_common/` asserts after every run that the worktree and the queue are untouched; fixture `fixtures/verify-ecs/`. Named mutations red: 35 of 36 in iteration 1 (`is_extra()` equivalent, TE-3), 34 of 34 in iteration 2.

rust-developer:

- [x] RD-1 -- `verify_mode_defaults_to_observe`, `verify_table_is_name_checked`, `verify_unknown_key_is_refused`.
- [x] RD-2 -- `cannot_verify_is_never_ok`, `raised_marker_with_both_changed_is_code_ahead`, `lagging_marker_is_spec_ahead`, eval `model_and_core_sources_do_no_file_io`, `queue_ids_are_skipped`.
- [x] RD-3 -- AC-14: `unparseable_bound_item_exits_2_in_every_mode`, `symbol_hidden_by_a_parse_error_cannot_verify`, `unreadable_bound_file_cannot_verify`, `lock_of_another_recipe_exits_2`.
- [x] RD-4 -- the cascade: `blob_equal_entries_are_not_hashed`, `blob_only_binding_change_is_code_ahead`, `comment_edit_is_ok`.
- [x] RD-5 -- the base: `drift_at_head_is_pre_existing`, `pre_existing_fix_passes_enforce_introduced`, `head_stricter_verify_mode_wins`.
- [x] RD-6 -- exits: `observe_never_exits_1`, `check_findings_never_block`, `verify_without_code_table_is_exit_2`.
- [x] RD-7 -- output: `verify_json_has_no_timing`, `verify_is_byte_identical_across_roots_and_order`, `verify_json_every_key_present`.
- [x] RD-8 -- read-only: `verify_never_writes_the_lock`, `verify_leaves_the_worktree_untouched`, `verify_writes_no_queue_row`.
- [x] RD-9 -- no new crate or dependency (the CLI pin in `build_graph.rs`); `check_codes_stay_35`.
- [x] RD-10 -- the seams: `verify_judges_a_given_walk`, `judge_is_source_neutral`, `golden_lock_verifies_ok_but_bad_levels` (`fixtures/spec-lock/code-a.lock`, 11 entries, over `fixtures/code-a/`: 9 `ok`, 2 `bad_marker bad_levels`; Q8).
- [x] RD-11 -- slice 3's carried items (1)-(7), code-reviewer checked (`features/spec-lock.md` "Implementation"): "changed meanwhile" names the index entry left; an unreadable branch "unverified, left as written"; "not restored (see the warning)"; a commit deleting `spec.lock` fails a check (TE-11); step 6's doc; "every read" gets `GIT_OPTIONAL_LOCKS=0`; pointers retargeted to the two lock canons.

test-engineer:

- [x] TE-1 -- a scenario per "States" row and category over `fixtures/code-a/` and slice 3's golden lock, text and JSON (`verify_states.rs`).
- [x] TE-2 -- TE-1 with CRLF files.
- [x] TE-3 -- AC-14, exit 2 in every mode: an unparseable bound `fn`, two broken items, an earlier unclosed item, a broken unbound neighbour (bound item judged), a non-UTF-8 file, a recipe mismatch. The `is_extra()` mutation is equivalent at verify level (seven broken shapes, identical rows); layer A's `hash_traps` pins it.
- [x] TE-4 -- {three modes} x {plain, `--changed`} x {clean, Check only, Fix introduced, Fix pre-existing, `cannot_verify`}; unborn `HEAD`; a merge in progress; an unmerged `spec.lock`; Q7: `hooked_verify_changed_lets_one_conflict_be_accepted`.
- [x] TE-5 -- AC-2 at verify level: rustfmt, a comment or neighbour edit `ok`, a body edit `code_ahead`, counters pinned; Q9: `autocrlf_items_same_module_and_ron_code_ahead`, `autocrlf_items_verify_as_lf_but_the_literal`.
- [x] TE-6 -- two roots, another cwd, reverse creation: byte-identical stdout, stderr, JSON.
- [x] TE-7 -- after every run `git status`, `HEAD`, `spec.lock` bytes, queue rows unchanged (`verify_common`).
- [x] TE-8 -- `fixtures/spec-c` exits 2 naming `[code]`; `verify_sources_hold_no_project_words` (M: a "mechanic" literal).
- [x] TE-9 -- `spec_check_is_unchanged_by_drift`, `repository_hook_runs_check_only`.
- [ ] TE-10 -- pilots, manual (`verify_pilot.rs`, `#[ignore]`, release, scratch clones): counts by state, wall time as AC-10's baseline, read-only proof. Owner, not run.
- [x] TE-11 -- `lock_commit_deleting_the_lock_fails_a_check` (M: absent read as unreadable).

spec-writer:

- [x] SW-1 -- this spec. SW-2 -- ADR-0042, `architecture.md#control`. SW-3 -- 08 s2's slice list; `spec-lock.md`'s "Out of scope", Q3's amendment.
- [x] SW-4 -- `docs/canon/spec-verify.md`, `spec-verify-output.md`; the overridden lines; 08 AC-14 met; CLI, core, store READMEs; `CLAUDE.md` State. SW-5 -- `export index`, `check` clean.

## Risks

Noise until 5b: RON and modules by blob (Q9), `[sig]` hashed whole, every binding `unlocked` on adoption; a grammar bump or an unparseable bound item holds exit 2 (`docs/canon/spec-verify.md` "Known limits"). Vocabulary: `blocked` beside ADR-0012; `unlocked` beside accept's `not locked`.

## Out of scope

- **4b `rev-discipline`** (ADR-0043, AC-13): the rule in `check --staged`, `--changed`; apply raising `rev`; an editorial approve; `spec bump`; `verify --staged`.
- **4c `drift-queue`** (ADR-0045): `spec propose drift` (MCP `report_drift`) queues `HEAD`'s rows as `drift` items, the backlog; approve runs accept; verify gains the drift hint and stays read-only.
- 5a-5c levels, lock format 2, `spec lock rebase`, `MOVED`/`AMBIGUOUS`/`LOST`; 6b `--node REF`; 7a `tests_reaching`, `sync` in reads; 7b `delta.symbols`; 8a `--tests`, `--task`; 8b; 9a; 9b: plain findings as Health row 6, never the verdict (ADR-0047). Not planned: a PATH, a debt baseline, a watcher, `spec check --base REF`.

## Open

| Point | Answer |
|---|---|
| Q1 slice 4 | split: 4a this (ADR-0042), 4b `rev-discipline` (ADR-0043), 4c `drift-queue` (ADR-0045) |
| Q2 which `cannot_verify` stops | any row, locked or not, Pre-existing or not: exit 2 (AC-14) |
| Q3 orphans | `spec lock accept <gone ID>` drops its entries; gone only when `HEAD` agrees |
| Q4 hashing needs a tree; the walk keeps none | re-parse only bound files whose blob changed; 9b's RD-13 counts `read_tree` calls |
| Q5 unmerged `spec.lock` | stages `:1:`-`:3:` in the base `cat-file --batch`, inside the canon's git list |
| Q6 a key `key_problem` refuses | `unlocked`, detail `key_problem`, the hint naming the field |
| Q7 hooks vs accept's lock commit | a project's hook runs `--changed` under `enforce-introduced` (4b: `--staged`); no ADR |
| Q8 the golden lock's invalid levels (`counter.rs:77`, `bounds.rs:14`) | accept locks them until 5b (`lock-levels.md` RD-3); verify reads them `bad_marker bad_levels` |
| Q9 `core.autocrlf`, `eol` | `hash-object --no-filters` stays (`architecture-code.md#sync-in-reads`); `module`, `ron` rows `code_ahead` by blob, a named limit until 5b's digests; items verify |

Q1-Q3 decided 2026-10-10, Q4-Q8 2026-10-12, Q9 2026-10-13, under the owner's pre-approval, review open.

## Implementation

| Module | What it does |
|---|---|
| core `verify.rs` (new), `lib.rs`, `project_toml.rs` | `VerifyConfig`, `verify_from_toml` (closed `[verify]`); `RowKey`, `RowBinding`; `RowFacts` (`NodeFact`, `MarkerFact`, `CodeFact`, `Comparison`, `Unbound`); `judge_row` -> `Judgement` (`State`, `Class`, `Compared`); `verdict`, `is_pre_existing`; `RawFile` name-checks `verify` |
| store `verify_git.rs` (new), `git.rs`, `lib.rs` | `VerifyGit::{open, is_unmerged, hash_files, list_commit, read_files, finish}`, `CommitListing`, `CommitEntry(Kind)`, `CommitFile` on `Staged::head_only`'s session: requests by name (`:1:`-`:3:`), `hash-object --no-filters --stdin-paths`, `rev-parse`, `ls-tree`; `read_files` moves each blob out, copying only a shared OID |
| CLI `verify.rs` (new) | `verify` -> `VerifyOutcome`: run causes in order, `judged_mode`, rows shown, hints, `render_text`, JSON with `delta` |
| CLI `verify_rows.rs` (new) | `judge_walk` (rows, facts, cascade), `read_head` (the base), `RunCause`, `Walked`, `HeadSide` |
| CLI `code_walk.rs` | `tree_of_blobs`: a commit's tree, equal files' bindings reused; `ManifestSeen`, `not_utf8` |
| CLI `lock.rs`, `lock_regen.rs`, `lib.rs`, `main.rs`; core `lock.rs`; store `worktree.rs`; code `lib.rs` | `Target::{of_rust, of_ron, key}` shared with `classify_found`; `read_lock`, `build_recipes` `pub(crate)`; RD-11 (1)-(7); `Command::Verify`, `Outcome::Verify` |

Deviations, accepted in review, in the canon as built: RD-10's golden test renamed (Q8); the drift hint left to 4c; a hint per (state, category); stages `:1:`-`:3:`; a run cause judges no row; a RON entry without its marker `marker_gone`; a walk gap for an unreadable walked file. Iteration 2 (the review's minors): the walk beside any cause, the first `cannot:` line first in order; the `conflict` hint split by R against `rev_L`; unresolved IDs keyed without `@rev` (a node renamed away: `bad_marker dangling`, then `node_gone`; owner's pre-approval); the mode shown the verdict's. TE-5 red: Q9.

Runs (iteration 2): fmt, clippy clean; nextest core, store, CLI 1 646 passed, 2 skipped (pilots). Gaps: Rust `nesting_too_deep` (`Comparison::CannotVerify`) pinned in core only; TE-10's figures not taken.

Carried to 4b's developer (`rev-discipline.md` RD-13), code only: CLI `verify.rs:640-646`'s "raise the marker to its revision" also serves R < `rev_L`: "set the marker to its revision", with `spec-verify-output.md` "Hints"' quote changed in the same change; store `git.rs` `Staged::blob` doc and `verify_git.rs:16` "each OID once": "once per `read_files` call"; pointers to this spec's headings retargeted to the two canons (core `lib.rs:63`, `verify.rs:2`; store `git.rs:51`, `verify_git.rs:1`, `lib.rs:41`; CLI `lib.rs:32`, `code_walk.rs:39`, `verify.rs:1`, `verify_rows.rs:2,757`, `lock.rs:603`, as "canon `spec-verify`"): "The verify table" -> "The `[verify]` table", "Command and rows" -> "Rows", "Cascade and spec side" -> "Cascade", "Output" -> `spec-verify-output.md`. Accepted: `tree_of_blobs` clones reused files (the current side borrows the walk).
