---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b-2, split from lock-levels 2026-10-16 (WA-1..WA-12; Q1-Q41, decided under the owner's pre-approval, review open; refreshed 2026-10-20 against d45dfce, R1-R14); 05 s3.5; 07 s2; 08 s2; developer 1 iteration, review 1 accepted, tests 1 round
shipped: 2026-10-20
adrs: [ADR-0050]
---

# lock-rebase (5b-2): `spec lock rebase`

## Why

Nothing could carry a lock across a change of format or recipe (WA-1). Since 5b-1a (`lock-levels.md`) every `Some` reader exits 2 on a format-1 lock or another `code_recipe` with core's `REBASE_STEP` (`run spec lock rebase on a terminal`): verify in every mode (cause 6), accept on all three paths, `spec propose drift`, MCP `report_drift`, a drift approve, an editorial approve that would refresh an entry; regen's `UNREBASED` sends a format-1 stage to `git merge --abort, spec lock rebase on the lagging branch`. The command did not exist; a hand edit is forbidden (ADR-0041); a grammar bump stranded every lock.

Delivered: `spec lock rebase`, the first migration writer (ADR-0050; `architecture-code.md#spec-lock`), re-deriving each entry from its own `file_blob`; and the base rule that lets a project's verify hook pass its commit (Q14). Order (Q9, Q21; `lock-levels.md` Q22, Q23): right after 5b-1a, closing its window; before 5b-1b (`lock-levels-verify.md`), 5c, 6b-2, 7a, 7b, 8b (which rebases its `[[tested]]` blocks on this: `tested-evidence.md` "Rebase").

## Description and interactions

As built, in canon: `docs/canon/spec-lock-rebase.md` (the steps, location, levels, drops, output, the commit, the base rule, known limits); `spec-lock.md` (the writer, the model), `spec-lock-accept.md` "The lock commit" (its subject), `spec-verify.md` "Base" and cause (7), `architecture-code.md#spec-lock`, the CLI README. The owner runs it on a terminal; an agent never rebases (no MCP tool or plugin command). The code crate is unchanged (Q36).

## Data

Pointers kept for the citations into this spec.

### `spec lock rebase`

`spec-lock-rebase.md` "Steps", "Location, levels, drops", "Output and the commit". **The plan** (R9): core `plan_rebase` takes the old `Lock` and per key a `Rederived` (a `LevelCode` or a `DropReason`) and gives a `RebasePlan` (format 2 at the build's recipes, a `RebaseRow` per entry); a kept entry takes a `LevelCode`, so no format-1 code reaches `render_lock`.

### The base

`spec-lock-rebase.md` "The base" (Q8, Q14, Q33, Q34): `HEAD`'s lock of format 1 or another `code_recipe` is judged with the current one under `--changed` and `--staged`, with a note. Decided at shipping (owner pre-approved): a judged lock not this build's gives cause (6) alone, as before (#1); an absent judged lock is an empty one of this build's, so `HEAD`'s old rows read `unlocked` against it (#2).

### Fixtures

`fixtures/spec-lock/code-a.rebased.lock`: `code-a.format-1.lock` rebased at its commit on `fixtures/code-a/`, 9 `rebased`, 2 `dropped bad_levels`; it is `code-a.lock` with each entry's `commit` from `code-a.format-1.lock` (5 lines differ). Never hand-written: `SPECENGINE_WRITE_REBASED_GOLDEN=1 cargo nextest run -p specengine-cli --test lock_rebase rebased_golden_is_accepts_with_format_1_commits` writes it, after `code-a.lock`.

## Rules and edge cases

- WHEN a rebase runs, the system SHALL read code only from each entry's `file_blob` by OID and SHALL drop, never guess, an entry it cannot re-derive (Q1, Q15).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, rebase SHALL give byte-identical locks and output.
- WHEN a rebase commit lands, no `drift` item SHALL be completed by it.
- **The pin** (Q16, R11): `lock_rebase.rs` joins `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`, 7 files) with `item_levels(`, `module_levels(`, `hash_bound_items(`; no `hash_item(`, `hash_node(`, `HashState`, nor a `.state` the pin does not allow.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red; M1-M6, 6 in all; CLI `tests/` unless named):

- [x] RD-1 -- the sources, `lock_rebase.rs`. **M1** an entry read from `HEAD`'s blob at its `file`, not its `file_blob` -> `rebase_uses_each_entrys_own_blob` (a later commit edits `counter.rs`). Scenarios `rebase_ignores_uncommitted_code`, `rebase_keeps_verified_at_and_commit` (an old date, a decayed `commit`).
- [x] RD-2 -- the golden, `lock_rebase.rs`: `rebase_of_the_format_1_golden_is_the_rebased_golden` (lock bytes, rows, the question, 9 rebased, 2 dropped), `rebased_golden_is_accepts_with_format_1_commits`. **M2** an `Invalid` list taken as `sig, body` -> `rebase_drops_invalid_level_entries` and the golden test.
- [x] RD-3 -- drops, `lock_rebase.rs`. **M3** a missing blob exits 2 instead of dropping -> `missing_blob_drops_its_entry`. **M4** the v2 cross-check skipped -> `v2_mismatch_drops_the_entry` (an item's `ast_hash` another well-formed digest). Scenarios `not_utf8`, `symbol_gone`, `ambiguous` (a module and an inline `mod` both marked: Q37), `cannot_verify <category>`, a `Foreign` OID as `blob_missing`.
- [x] RD-4 -- refusals, scenarios, `lock_rebase.rs`: `rebase_off_a_terminal_reads_nothing`; `other_norm_recipe_exits_2` (the `Some` text verbatim), `format = 3`, not UTF-8: exit 2; `rebase_refuses_an_operation_in_progress` (`Repo::open`'s text); a dirty `spec.lock` or `specengine.toml`, no lock: exit 1; a lock already this build's: exit 0 `nothing to change`, no question.
- [x] RD-5 -- the commit, `lock_rebase.rs`. **M5** check 3 passing a rebase commit without `Lock-mode: rebase` -> `rebase_commit_without_its_mode_trailer_fails_a_check` (a `commit-msg` hook drops it: exit 1, unverified). Scenario `rebase_commit_names_no_node` (no `Lock:`, no `Proposal:`). Guard, not a counted mutation (Q39): `drift_approve.rs`' rewritten-`Lock-mode` loop gains `Some("rebase")`. `lock_commit.rs`, `lock_exits.rs`, `drift_approve.rs` green after the subject refactor.
- [x] RD-6 -- "The base", `lock_rebase.rs`. **M6** `HEAD`'s old-format lock as cause (7) -> `head_lock_of_another_format_is_judged_with_the_current_lock`, under `--changed` (the rebased lock on disk) and `--staged` (staged): every row Pre-existing, no cause (7), the note on stderr and in `notes`. Decisions #1, #2: `other_head_locks_keep_their_causes`, `an_absent_judged_lock_judges_heads_rows_unlocked`.
- [x] RD-7 -- guards, no mutation: the pin (`code_refs_walk.rs`); eval `build_graph.rs` (`core_and_store_have_no_tree_sitter`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`); store `tests/verify_git.rs`: `read_blobs` by OID (bytes, missing, foreign).

test-engineer:

- [x] TE-1 -- `lock_rebase.rs`, temp repos from `fixtures/code-a/` and `code-a.format-1.lock`: a scenario per row and drop reason (`bad_levels` included), text and JSON pinned, `lock_rebase_json_keeps_its_keys`.
- [x] TE-2 -- the migration on a pseudo-terminal (`PtyRun`): the format-1 golden committed -> verify exit 2 naming the step -> rebase, `y` -> one commit passing the four checks -> verify's keys, states, categories as over accept's golden (rows carry `lock.commit`, which differs: `spec-verify-output.md` "JSON"; exit 2 throughout on code-a's Cannot rows) -> `spec lock accept` of `PIECE-COUNTER`, `LAW-BOUND`, `GOAL-01`: `nothing to change`; `cap-check/PASS-01`: exit 1.
- [x] TE-3 -- a simulated grammar bump: accept's golden committed as `Variant::OtherCode` -> verify exit 2 naming the step -> rebase, under the note, gives `code-a.lock` byte for byte.
- [x] TE-4 -- an entry's `file_blob` no object has: `dropped blob_missing`; after the commit verify shows the binding `unlocked`.
- [x] TE-5 -- new `lock_rebase_hooks.rs` on verify-ecs (Q41; `drift_approve.rs` `lock_commit_under_spec_verify_hooks`' corpus), its accepted lock committed as `Variant::OtherCode` beside a pre-existing Fix row (a stray `@implements` marker's `bad_marker dangling`; a code edit gives only a Check row, `code_ahead`): a project `pre-commit` hook running `spec verify --staged`, and one running `--changed`, under `enforce-introduced` passes the rebase commit, exit 0; under `enforce` the Fix row refuses it: exit 1, bytes restored.
- [x] TE-6 -- after every refusal and an `N`: `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs, index, `spec.lock` bytes unchanged.
- [x] TE-7 -- 4b-2's carry-over (`rev-rule.md` TE-3, Q18), `rev_rule_hook.rs`: the rebase commit through a real `spec check --staged --root .` hook under `[check] mode = "enforce"`: clean, the commit made. A scenario, no mutation (Q38).
- [x] TE-8 -- byte-identical lock, stdout, stderr, JSON: two roots, another cwd, `core.autocrlf=true`, the input's entries reversed (`lock-levels.md` TE-2's twin).

spec-writer:

- [x] SW-1 -- this spec: split 2026-10-16 (`lock-levels.md` Q21); refreshed 2026-10-20 against d45dfce (R1-R14 fixed in place, Q32-Q41, M1-M6).
- [x] SW-2 -- at shipping (2026-10-20), bytes before -> after: new `spec-lock-rebase.md` 4 607 (budget 4 608); `spec-lock.md` 9 979 -> 10 121 (2 167 left for 8b); `spec-verify.md` 11 743 -> 11 961; `spec-lock-accept.md` 11 728 -> 11 840; `architecture-code.md` 11 825 -> 11 916; CLI README 9 934 -> 9 957; `docs/index-canon-spec.md` 2 831 -> 3 033; `symbol-names.md` re-pointed; 05 s3.5, 07 s2, 08 s2, `CLAUDE.md` State; this spec compacted ("The base", "Out of scope" kept); `lock-levels-verify.md` (pointers, "Later"), `anchor-states.md`, `tested-evidence.md` re-pointed.
- [x] SW-3 -- no new ADR (ADR-0050 decides this writer; ADR-0012, ADR-0041, ADR-0042 hold); `export index`, `check` clean.

## Out of scope

Rebasing across `norm_recipe` or into an older format; regen across formats; an MCP tool, UI or plugin command; `--yes`; 5c's anchors (a rebase keeps keys: `anchor-states.md`); 8b's `[[tested]]` blocks under a rebase (`tested-evidence.md` "Rebase", built on this).

## Implementation

| Module | What it does |
|---|---|
| store `src/verify_git.rs`, `src/lib.rs` | `ObjectBlob` (`Bytes`, `Missing`, `Foreign`); `VerifyGit::read_blobs`: OIDs in one raw `cat-file --batch`, `GIT_NO_LAZY_FETCH`, no listing |
| core `src/lock.rs` | `DropReason` (`text`), `Rederived`, `RebaseRow`, `RebasePlan`, `plan_rebase` |
| CLI `src/lock_rebase.rs` (new) | the command: refusals, blob reads, location, levels, hashing, the cross-check, rows, JSON |
| CLI `src/lock.rs` | `LockSubject` (a node, the rebase) through `write_and_commit`, `check_lock_commit`, `changed_while`; `LockMode::Rebase` outside `TRAILERS`; `LockOutcome::of_rebase`; helpers `pub(crate)` |
| CLI `src/verify_rows.rs`, `src/verify.rs` | `head_lock`: `HEAD`'s lock in a rebase's order; the note under `--changed`, `--staged` |
| CLI `src/main.rs`, `src/lib.rs` | `LockCommand::Rebase` behind `require_terminal`; `Outcome::LockRebase` |
| tests | new CLI `lock_rebase.rs`, `lock_rebase_hooks.rs`, `lock_common/rebase.rs`; `lock_common/mod.rs`, `drift_approve.rs`, `code_refs_walk.rs`, `rev_rule_hook.rs`; core `lock_model.rs`; store `verify_git.rs` |
| fixtures | `spec-lock/code-a.rebased.lock` new |

Readings accepted by the review: HEAD's lock under cause (7) gives the strict reader's text (#3); a module's candidates are inline `mod`s only (#4); a marker on an item with a parse error binds no unit, so a tie may read `ambiguous`, not `cannot_verify` (#5); a RON entry is `ambiguous` on any `Ambiguous` anchor, several comments give the first's digest (#6); `hash_bound_items` giving no digest drops `hash_mismatch` (#7); the dirty refusal is accept's text (#8); the note also for an empty lock (#9). Also: an unmerged `spec.lock` with no operation in progress gets `Repo::open`'s merge text; a `file_blob` that is no OID drops `blob_missing`. Deferred to the carry-overs (`lock-levels-verify.md` "Carry-overs", RD-9): the review's minor (`catch_unwind` wraps more than the parse) and its nits. Q1-Q41 with their alternatives: this file at fc5ea38.

Tests: core, code, CLI, MCP, HTTP, store 2 839 passed, 12 skipped; eval `build_graph`, `doc_pointers`, `anonymity` 58 passed; clippy `--workspace --all-targets -D warnings` clean. Mutations 6 of 6 killed (M1-M6, the criteria). Unpinned: a parser panic or no tree, no committer identity, `GIT_NO_LAZY_FETCH` on a partial clone, a bare `cannot_verify` from a real blob (core's `DropReason::text` test only).
