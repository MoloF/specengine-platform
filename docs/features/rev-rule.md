---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4b-2 (4b split 2026-10-13, Q8; WA-6..WA-10, WA-13, WA-14); refreshed 2026-10-13, 2026-10-15 against shipped 4c-1 (Q14-Q20; decided under the owner's pre-approval, review open, "Open"); 05 s3.5; 07 s2; 08 s2, AC-13; Rust 2 iterations, review accepted with minors, tests 2 rounds
shipped: 2026-10-15
adrs: [ADR-0043]
---

# rev-rule (4b-2): a locked node's meaning change without a raise fails the staged check

## Why

08 AC-13: a node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries must fail pre-commit. 4b-1 (`rev-discipline.md`; canon `proposal-revisions.md`) made every approval declare one; nothing judged a commit that skips the queue. Delivered, by ADR-0043: the rule `rev-not-raised` in `spec check --staged` and `--changed`; `spec bump`, listing what to raise; `spec verify --staged`, judging what a commit records.

**Owner review: risk 3, "content never blocks".** The rule refuses a commit over a spec change, beside ADR-0012 and `docs/README.md` "Enforcement". The reading taken: it judges whether a locked node's change was *declared* (a raise or an editorial refresh), never what the text says; ADR-0019 took it from tracey; `[check] mode` gates it (`observe` never blocks), debt covers it. Read as content, it becomes a warning and AC-13 is unmet. Consequences: the first committed `spec.lock` turns it on under the project's `[check] mode`, no opt-in of its own (unlike `[verify] mode`); once committed, the same fact shows in verify as `spec_ahead same_revision`, a Check that never blocks (ADR-0042); no per-rule switch, the way out is one raise or `--editorial`. "Enforcement" carries the clause.

## Description and interactions

Canon: `docs/canon/node-revision.md` "Raising" (the rule, its finding, `spec bump`), `spec-check-git.md` "The rule's locks", `spec-verify.md` "Base" and "A project's hook" (`--staged`), `spec-lock-accept.md` "The lock commit" (its hooks); the rest: SW-2.

- **A project** in `enforce` or `enforce-introduced` gets the rule with its first committed `spec.lock` (the pilots run `observe`). Its hook runs `spec check --staged` and, with `[code]`, `spec verify --staged` under `enforce-introduced` (documented, no script shipped); the rule is silent on a lock commit (accept's, a drift approve's): it changes no `norm_hash`. Agents keep `--changed`.
- **The owner** writes the line `spec bump` prints and commits, or sends the change through the queue and approves it `--editorial`; a committed escape reaches him as a drift item (4c-1).
- **This repository**: no `spec.lock`, so output byte-identical, `CHECK_CODES` 35 -> 36; hook, CI unchanged; `verify --staged` not wired (no `[code]`).
- **Unchanged**: MCP, HTTP, UI, plugin (no code or text, Q20), `INDEX_FORMAT` and the queue schema 8, accept's refusal, `check_parity.rs`; no new crate or dependency. Live-spec lines fixed: 05:103, 07:105-106, 08:41, 08:44, 08:82 (AC-13 met, Q4).

## Data

As built in the canon; the headings stay for the pointers in code and tests.

### The rule `rev-not-raised`

`docs/canon/node-revision.md` "Raising" (WA-6, WA-7, WA-14, Q2): scope, pairing, the finding's text and `fix`; its locks `docs/canon/spec-check-git.md` "The rule's locks" (Q9).

### spec bump

`docs/canon/node-revision.md` "Raising" (WA-8): rows, text, JSON, exits; verify's `same_revision` hint `docs/canon/spec-verify-output.md` "Hints" (Q16).

### spec verify --staged

`docs/canon/spec-verify.md` "Base" (WA-9, Q13-Q15, Q17, Q18), "A project's hook", "Known limits"; its git `architecture-code.md#sync-in-reads`.

## Rules and edge cases

In the canon: findings and rows byte-identical from any root, cwd or creation order, genre-neutral (canonical IDs and the lock only); a commit changing only `spec.lock` parses neither lock; a hook's `spec verify --staged` on a lock commit judges `HEAD` plus the staged lock, a dirty spec edit not counted; escapes (`--no-verify`, `observe`, no hook, CI's plain check, regen dropping one of two refreshes) show as `spec_ahead same_revision`, queued by `spec propose drift`, settled by `spec approve PR --editorial`. A format-1 `HEAD` lock beside a format-2 one: 5b (Q9).

## Acceptance criteria

rust-developer (mutation -> the test that must fail):

- [x] RD-1 -- the rule, pure: every ID judged -> `unlocked_node_change_is_no_finding`; evidence ignored -> `refreshed_entries_pass_at_the_same_revision`; one of two refreshed -> `partial_refresh_is_a_finding`; a lower revision -> `lowered_revision_is_a_finding`; the heading hashed -> `heading_rename_is_no_finding`; by path -> `moved_section_is_no_finding`; a non-vouching entry -> `pre_existing_lock_drift_is_out_of_scope`; a bare-ID key -> `feature_scoped_node_is_judged_by_its_lock_key`; `std::fs` -> `model_and_core_sources_do_no_file_io`. A block judged as an entry -> `tested_block_vouches_like_an_entry`: 8b's.
- [x] RD-2 -- wiring: runs plain -> `plain_check_has_no_rev_finding`; the default index -> `rule_reads_the_lock_through_git_index_file`; `observe` blocks -> `observe_never_blocks_a_rev_finding`; `code_recipe` read -> `other_code_recipe_lock_still_checks`; an invalid lock with a candidate passes -> `invalid_lock_is_cannot_check`; `norm_recipe` ignored -> `other_norm_recipe_lock_is_cannot_check`; a lock parsed with no candidate -> `lock_only_commit_never_parses_the_lock`. `CHECK_CODES` 36; `check_parity.rs` unchanged. Format 1 refused -> `format_1_head_lock_is_read`: 5b's.
- [x] RD-3 -- 4b-1's raise removed -> `apply_commits_pass_the_revision_rule` (a real hook, a lock); 4c-1's `lock_commit_passes_spec_check_staged` green unedited, the rule inside.
- [x] RD-4 -- bump: any write (status, `HEAD`, index, queue) -> `bump_writes_nothing`; `bump_marks_vouched_nodes_required`, `bump_has_no_editorial_flag`, `same_revision_hint_names_bump_and_editorial` (verify's hint; accept's refusal, plain and drift, keeps its pins). 4c-1's hint pin in `verify_states.rs` churned, as expected.
- [x] RD-5 -- `verify --staged` (4c-1's seam): a disk read -> `verify_staged_judges_the_index_not_the_disk`, `verify_staged_ignores_a_disk_walk_gap`; `hash-object` -> `verify_staged_runs_no_hash_object`; `.git/index` for `GIT_INDEX_FILE` -> `verify_staged_reads_git_index_file`; the disk config -> `verify_staged_reads_the_staged_config`; an intent-to-add `spec.lock` read as a lock -> `verify_staged_drops_an_intent_to_add_lock`; with `--changed` -> `staged_and_changed_exclude_each_other`; `read_head` wanting shared blobs -> `verify_staged_reads_each_object_once`. Code-reviewer: one one-side reader (`read_commit`, `judge_side`), 4c-1's seam tests green unedited.
- [x] RD-6 -- a kind literal -> the genre scans; a new dependency -> the CLI pin in `build_graph.rs`.

test-engineer:

- [x] TE-1 -- AC-13 (`rev_rule_hook.rs`, code-a with the golden lock and verify-ecs, a real hook running `spec check --staged`): an unraised locked edit refused, naming it and both ways out; raised, whitespace-only, unlocked pass; `observe` passes, the finding in `--json`; `enforce-introduced` refuses.
- [x] TE-2 -- `--changed` = `--staged` on equal trees (`changed_equals_staged_on_equal_trees`); plain unchanged.
- [x] TE-3 -- an invalid lock with a candidate exits 2; an unparseable staged lock with no spec change: clean; another `code_recipe` judged; unmerged (`unmerged_lock_is_refused_under_staged`); two refreshes through `regen` (`two_refreshes_through_regen_meet_the_rule`). A rebase commit under `enforce`: 5b's.
- [x] TE-4 -- `bump` rows, read-only (`rev_bump.rs`).
- [x] TE-5 -- `verify --staged`, index unlike disk; a staged config judged; `core.autocrlf` reads `ok` (`verify_staged_reads_clean_blobs_under_autocrlf`); lock commits under a `--staged` and a `--changed` verify hook, accept and drift approve (`lock_commit_under_spec_verify_hooks`): `enforce-introduced` with a dirty deletion passes under `--staged`, `--changed` refuses; `enforce` with a pre-existing Fix row refused, restored, reopened; an intent-to-add `spec.lock` never stops the next commit.
- [x] TE-6 -- here: an ADR edit staged, no finding; the hook untouched. TE-7 -- `fixtures/spec-c`, no finding without a lock, one by ID with one; the source scan. TE-8 -- two roots, cwd, creation order (`rev_rule_corpora.rs`).
- [ ] TE-9 -- pilots, manual (`rev_rule_pilot.rs`, `#[ignore]`, written, not run: the owner's), scratch clones: silent without a lock; accepted in the test's own clone (`SPECENGINE_PILOT_ACCEPT=1`), the in-scope count and timing; no pilot name committed.

spec-writer:

- [x] SW-1 -- this spec (2026-10-13, refreshed 2026-10-15); `export index`, `check` clean.
- [x] SW-2 -- at shipping, 2026-10-15: `node-revision.md` "Raising"; `spec-check.md`, `-process.md`, `-graph.md` (36 codes), `spec-check-git.md` (the Git bullets, the detector's `spec.lock` trigger, "The rule's locks"); `spec-lock.md`; `spec-verify.md` "Base", "A project's hook", "Known limits"; `spec-verify-output.md` "Hints"; `spec-lock-accept.md` "The lock commit", `proposal-drift.md` "Approve" (a pointer); `architecture-code.md#sync-in-reads` (Q13); `docs/README.md` "Enforcement" (risk 3); 05:103, 07:105-106, 08:41, 08:44, 08 AC-13 (Q4); CLI, core, store READMEs; `CLAUDE.md`; `lock-levels.md`'s byte note.
- [x] SW-3 -- iteration 2, 2026-10-15: `node-revision.md` "Raising" (the three no-raise cases, bump's `rev N` alone), core README's `judge`, "Implementation".

## Out of scope

4b-1's parts (`rev-discipline.md`); 4c-2 `drift-follow-up`; slice 5 levels, lock format 2; 8a, 8b. A staged lock writer (Q5); a commit-msg hook; a CI range check; `@rev` pins; raising markers; Health rows; a shipped hook script; a per-rule switch; a lock here.

## Open

Owner pre-approved, review open (Q2, Q4 2026-10-10; Q8, Q9, Q13, `--staged` 2026-10-13; Q14-Q20 2026-10-15; Q16, Q18 flagged for the owner, nothing waits); the other answer's cost:

- Q2 the scope: nodes a `HEAD` lock entry vouches for. Q4 AC-13 reworded (08). Q8 split under ADR-0043, no new ADR.
- Q9 both locks in check's one session, parsed only with a candidate, `code_recipe` ignored, format 1 (2 after 5b): else the rule and 5b's rebase deadlock.
- Q13 check's index reader reused, `architecture-code.md`'s git list amended (no ADR). `--staged` and `--changed` both kept, exclusive.
- Q14 the index side through 4c-1's seam: two one-side readers kept byte-identical. Q15 the index's config whole unless `--config`: the disk's judges a staged config change under the old one.
- **Q16** `spec bump` named in verify's hint only, "while the edit is uncommitted": in accept's refusal it is false (bump lists nothing at a clean `HEAD`).
- Q17 index causes (5) per unmerged path, (3) an index blob: (7) reads as a partial base.
- **Q18** the intent-to-add detector widened to the root `spec.lock`: an empty-blob lock is exit 2 in every mode, every commit refused until `git rm --cached spec.lock`.
- Q19 the rule's "approve it through the queue with --editorial" and 4c-1's `DRIFT_HINT` both kept: naming the drift route invites `--no-verify`, changing `DRIFT_HINT` churns 4c-1's pin. Q20 no agent-facing text (MCP `INSTRUCTIONS`, `ask-owner` near their caps).
- Q1, Q3, Q5-Q7, Q10-Q12: `rev-discipline.md` "Open".

## Implementation

| Module | What it does |
|---|---|
| core `check/rev_rule.rs` (new) | rule, bump rows: `changed_nodes`, `rule_lock`, `judge(changed, head_lock, checked_lock, scheme, classes, rows)`; pure, hash a closure |
| core `check/{mod,engine,report,resolve}.rs`, `raise.rs` | `CHECK_CODES` 36; `add_findings`; `sort_findings`; `canonical_id` (CLI `{propose,drift,lock,interpret,create}.rs`), `feature_scoped`, out of `propose.rs`; `raised_line`, `closed_against_rev` |
| store `check.rs`, `base.rs` | `with_rule`, both checks (`judge` gets `[classes]`, `rows.is_some()`); `bump_{staged,changed}` -> `BumpCheck`; `BaseRun` (both sides' files, uncopied) |
| store `git.rs`, `verify_git.rs` | detector's `spec.lock` (Q18); `Staged::read_index`; `VerifyGit::staged`, `IndexRefusal` |
| CLI `bump.rs` (new), `main.rs`, `lib.rs` | `spec bump`; `verify --changed`, `--staged`, exclusive |
| CLI `verify.rs`, `verify_rows.rs` | `VerifiedTree`, `verify_staged`, `judge_index`; `SAME_REVISION_STEP` (D8); `CommitConfig::of_index`, `IndexShare` (manifest bytes: `Cargo.toml`, `…/Cargo.toml` only), `read_head(.., index)`, `judge_commit(.., base)` |

Working answers W1, W2, W4-W7, W9, W13-W16: canon. W8 `fix` the whole line sans ending, an insert an empty span; W10 bump drops notes, check's default baseline; W11 disk config only with `--config`, mode `null` if git fails or a path is unmerged without it; W12 a shared non-UTF-8 code file: a 1-byte stand-in, skipped as `HEAD`'s.

Deviations, accepted: 1 lock read whole by `parse_lock(text, None)` (`spec-check-git.md`), not `node`, `rev`, `spec_hash` alone: a bad code field: cannot check (exit 2) given a candidate. 2 store `CommitListing::commit()` -> `label()`, `is_index()`, causes byte-identical.

Iteration 2 (review 1's minors, nits; pre-approved): W14 changed, W15, W16: canon "Raising" (`closed_against_rev`, `raised_line` `None`); at a readable `HEAD` revision `fix`, `to`, `write` all or none set. Line, raise only for `required` or asked rows, one `FileText` per file; `Changed<'a>` lost `line`, `fix`, `to()`.

Tests: CLI `rev_rule*.rs`, `rev_bump.rs`, `verify_staged.rs`; pins moved: core `check_output.rs`, `verify_judge.rs`, CLI `drift_approve.rs`, `verify_states.rs`, `rev_editorial.rs`. Round 2 (+ `closed_class_*`, `rev_max_*`, `unwritable_raise_*`, `…_lookalike_path`; minors `verify_staged_reads_the_staged_config`, `bump_line`): 36/36 new, core+store+CLI 1 847, MCP+HTTP 215/215, 8/8 mutations red; `unwritable_raise_finding_names_only_editorial` fixed: sets `enforce-introduced` first.

Not covered: which side's `[classes]` W14 reads (code: checked, `setup.config`; tests commit them at `HEAD`); an unreadable index blob's cause (3).

Known limits: `--staged` parses all index code per hook, like `--changed` (TE-9 times it); intent-to-add code: `spec-check-git.md` "Unlike plain".

Follow-ups: `git.rs`, `bump.rs` comment nits (4c-2); `REQUIRED_HINT` names both ways out though all `required` rows show `-` (canon): 5b; `key-missing` on flow-style front-matter keys (pre-existing; triage).
