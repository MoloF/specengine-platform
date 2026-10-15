---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4b-2 (4b split 2026-10-13, Q8; WA-6..WA-10, WA-13, WA-14); refresh analysis 2026-10-13; 05 s3.5; 07 s2; 08 s2, AC-13; refreshed 2026-10-15 against shipped 4c-1
adrs: [ADR-0043]
---

# rev-rule (4b-2): a locked node's meaning change without a raise fails the staged check

## Why

08 AC-13: a node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries must fail pre-commit. 4b-1 (`docs/features/rev-discipline.md`, shipped 2026-10-14; canon `proposal-revisions.md`) makes every approval declare one; nothing judges a commit that skips the queue. Delivered here, by ADR-0043 (`node-revision.md` "Raising"): the rule `rev-not-raised` in `spec check --staged` and `--changed`; `spec bump`, listing what to raise; `spec verify --staged`, judging what a commit records. The rule ships after 4b-1, else every meaning apply on a locked node fails its hook; `verify --staged` needs only 4a and may be pulled forward alone if a hook deadlocks first.

**Owner review: risk 3, "content never blocks".** The rule refuses a commit over a spec change, beside ADR-0012 and `docs/README.md` "Enforcement" ("content never does"). The reading taken: it judges whether a locked node's change was *declared* (a raise or an editorial refresh), never what the text says; ADR-0019 took it from tracey; `[check] mode` gates it (`observe` never blocks), debt covers it. Read as content, it becomes a warning and AC-13 is unmet. Consequences: the first committed `spec.lock` turns it on under the project's existing `[check] mode`, with no opt-in of its own (unlike `[verify] mode`, default `observe`); once committed, the same fact shows in verify as `spec_ahead same_revision`, a Check that never blocks (`spec-verify.md` "States", ADR-0042), consistent only under the declaration reading; no per-rule switch, the way out is one raise or `--editorial`. "Enforcement" gets the clause at shipping (SW-2).

## Description and interactions

- **A project** in `enforce` or `enforce-introduced` gets the rule with its first committed `spec.lock` (the pilots run `observe`: it only reports). Its hook runs `spec check --staged` (the rule inside) and, with `[code]`, `spec verify --staged` under `enforce-introduced` (documented, not shipped): `spec-verify.md` "A project's hook" switches from `--changed` at shipping. Both also run on every lock commit: `spec lock accept`'s and a drift item's approve (4c-1, canon `proposal-drift.md`) are one `git commit --only spec.lock` with hooks (CLI `lock.rs` `commit_only`, ~1741-1747), judged on git's temporary index. Agents keep `--changed` (`spec-check-git.md` "The changed check").
- **The owner** reads the line to write from `spec bump`, raises by hand and commits, or sends the change through the queue with `--editorial` (4b-1). A meaning change already committed (`--no-verify`, `observe`) reaches him as a drift item (`spec propose drift`, `spec_ahead same_revision`), settled by `spec approve PR --editorial` (4c-1).
- **This repository** (WA-10): no `spec.lock`, nothing in scope: output byte-identical, `CHECK_CODES` 35 -> 36; hook, CI unchanged; `verify --staged` not wired (no `[code]`).
- **Crates** (WA-13): core the rule (pure), bump's rows, `canonical_id` moved from CLI `propose.rs` (the rule runs in store's check, which cannot call the CLI; accept (`lock.rs`), verify (`verify_rows.rs`) and the drift raise (`drift.rs`, 4c-1) then call core's), 4b-1's vouching predicate reused; store the two lock blobs in check's git session, a `VerifyGit` constructor over the index (`Staged::read`; `open` stays `Staged::head_only`) giving its entries as a `CommitListing`; CLI `spec bump`, `VerifyRequest`'s `changed: bool` three states as check's `CheckedTree`, 4c-1's one-commit seam (`verify_rows.rs` `commit_config`, `read_commit`, `judge_commit`) fed the index's listing and given a base, `read_head` given the index side. `INDEX_FORMAT` and the queue schema stay 8. MCP, HTTP, UI, plugin: no code or text (`INSTRUCTIONS`, `ask-owner`, plugin 0.8.0, review keys, `daemon-keys.json` unchanged); no new crate or dependency.
- **Later slices**: 5b's rebase commit and format-1 `HEAD` lock (Q9; `lock-levels.md` Q8); 6b's `--node` refuses `--staged` (`code-tools.md` "spec verify --node"); 8b's `[[tested]]` blocks vouch like entries.
- **Overridden live-spec lines**, fixed at shipping: 05:103 (`bump` writing revisions, `bump --editorial`), 07:105 ("--staged 4b-2"), 07:106 (`spec bump [--editorial ID]`), 08:44 (`spec bump`), 08:82 (Q4).

## Data

### The rule `rev-not-raised`

WA-6, WA-7, WA-14, Q2, Q9. One error code (`CHECK_CODES` 36), in check, not verify (WA-14), under `--staged` and `--changed` only; plain never reads the lock.

- **Candidate**: a node whose canonical ID is at `HEAD` and in the checked tree, its checked `norm_hash` non-null and different from `HEAD`'s. None: neither lock is parsed. A lock-only commit (accept, editorial accept, regen's merge, 4c-1's drift approve, 5b's rebase) changes no `norm_hash`, so the rule stays silent whatever the lock holds.
- **In scope**: a candidate whose `HEAD` revision is readable and `HEAD` `norm_hash` non-null, `HEAD`'s `spec.lock` holding a vouching entry for it (at that revision, `spec_hash` that `norm_hash`; with 8b, or a `[[tested]]` block: `tested-evidence.md` Q3).
- **Finding**: the checked revision unreadable or not above `HEAD`'s, and the checked lock (absent: none) not carrying the new hash on each vouching entry (same key, or the node's block, same `rev`).
- **Locks** (Q9): `HEAD:spec.lock` and the checked one, requested inside check's one git session before it finishes (store `check.rs`); the checked one under `--staged` the index's blob (`GIT_INDEX_FILE`: the `--only` index of apply's, accept's and a drift approve's commits too), under `--changed` the disk. Parsed only when a candidate exists, for `node`, `rev`, `spec_hash` (with 8b, its blocks) alone, in any format this build reads or rebases from (format 1; after 5b, 1 and 2); `code_recipe` and code fields ignored. None at `HEAD`: nothing in scope, the checked lock unparsed. Unparseable, another `norm_recipe` or a newer format: cannot check, exit 2, the cause naming `HEAD:spec.lock` or `spec.lock`. Unmerged under `--staged`: check's refusal of any unmerged path (`spec-check-git.md` "The staged check"), no new code.
- Always introduced; `[check] mode` decides; debt covers it.

```
error  docs/spec/combat.md:14: rev-not-raised: `R-12` changed at rev 3, which spec.lock vouches for: raise it (rev=4), or have the owner approve it through the queue with --editorial
```

Line: the heading or the `id:` line; subject the ID; `fix` the raised line as 4b-1's apply writes it. Unlocked nodes, marker-only bindings, `@N` pins: no finding (bump's `suggested`).

### spec bump

WA-8. `spec bump [--staged] [--json]` (+ `--root`, `--config`), read-only: no file, index entry, git object, queue row or event. The disk (as `check --changed`) or `--staged` the index, against `HEAD`. A row per node with an ID at `HEAD` whose `norm_hash` changed and revision is not above `HEAD`'s, by path and line; `required` when the rule finds it, else `suggested`; `to` = `HEAD`'s + 1:

```
required | R-12 | docs/spec/combat.md:14 | rev 3 -> 4 | 2 lock entries | ## Cost {#R-12 rev=4}
suggested | R-13 | docs/spec/combat.md:30 | rev 1 -> 2 | 0 lock entries | ## Range {#R-13 rev=2}
hint: suggested: raising it makes markers naming it lag
```

`hint: required:` gives the finding's ways out. JSON `{rows: [{need, id, path, line, rev, to, lock_entries, write}]}`; an unreadable revision `rev ?`, `rev` and `write` null. None: `nothing to raise`. Exit 0 when run; 2 on check's run causes or the rule's lock causes; `--editorial` a usage error. Verify's `spec_ahead same_revision` hint (CLI `verify.rs` `SAME_REVISION_STEP`, ~597) gains "(`spec bump` prints the line while the edit is uncommitted)" after "raise its `rev` by hand"; accept's refusal, plain or a drift approve's, is unchanged: it judges `HEAD` with the node's file clean, where bump lists nothing.

### spec verify --staged

WA-9, Q13, Q14. `spec verify --staged [--json]` (+ `--root`, `--config`): 4a's judge over what the commit records, through 4c-1's one-commit seam (CLI `verify_rows.rs`). **Current side**: the index git names (`GIT_INDEX_FILE`: `-a`, `--only`, apply's, accept's and a drift approve's commits), listed as check `--staged` lists it (`Staged::read`: `ls-files -s -z`, the intent-to-add detector, its trigger widened to `spec.lock`; an unmerged path refused as (5), `spec.lock`'s text 4a's), as a `CommitListing` named `the index` in causes; the config is the index's `specengine.toml`, whole (scheme, `[paths]`, `[code]`, `[verify]`), unless `--config` (disk); then `read_commit` as built (`CommitListing::documents`, `read_files`, `tree_of_blobs` over `CodeTree::empty()`, the lock against this build's recipes, (6)); an index blob that cannot be read or parsed is (3), never (7). Level 1 by the index's OIDs against `file_blob` (`Files::Commit`). No disk walk (`read_tree`), no `hash-object`, no data-directory refresh. **Base** `HEAD`: `read_head` given the index side, a (path, OID) the index shares neither read nor parsed again; rows by `judge_commit` given that base, Pre-existing as `judge_walk` marks it. Mode the stricter of the index config's and `HEAD`'s; exits: `spec-verify.md` "Base", "Verdict and exit". With `--changed`: usage, exit 2. Index OIDs are clean blobs: 4a's autocrlf limit does not apply. Git: check's index reader joins `architecture-code.md#sync-in-reads` at shipping (Q13).

## Rules and edge cases

- WHEN one commit is checked from two roots, another cwd or creation order, findings and rows SHALL be byte-identical. Genre-neutral: canonical IDs and the lock only.
- WHEN a commit changes only `spec.lock`, the rule SHALL parse neither lock and find nothing (canon `docs/canon/proposal-drift.md` "Approve", a drift approve's commit, relies on it).
- WHEN `HEAD`'s lock is format 1 and the checked one format 2 (5b's rebase, an approval after it), the rule SHALL read both (Q9; `lock-levels.md` Q8).
- WHEN a hook runs `spec verify --staged` on accept's or a drift approve's lock commit, a spec edit dirty on disk SHALL not count: only `HEAD` plus the staged lock is judged.
- Escapes (`--no-verify`, `observe`, no hook, CI's plain check) and regen dropping one of two refreshes show in verify as `spec_ahead same_revision`; 4c-1 queues them (`spec propose drift`), and `spec approve PR --editorial` settles them.

## Acceptance criteria

rust-developer (mutation -> the test that must fail):

- [ ] RD-1 -- the rule, pure: every ID judged -> `unlocked_node_change_is_no_finding`; evidence ignored -> `refreshed_entries_pass_at_the_same_revision`; one of two refreshed -> `partial_refresh_is_a_finding`; a lower revision -> `lowered_revision_is_a_finding`; the heading hashed -> `heading_rename_is_no_finding`; by path -> `moved_section_is_no_finding`; a non-vouching entry -> `pre_existing_lock_drift_is_out_of_scope`; a bare-ID key -> `feature_scoped_node_is_judged_by_its_lock_key`; `std::fs` -> `model_and_core_sources_do_no_file_io`; a block judged as an entry -> `tested_block_vouches_like_an_entry` (the later of 4b-2 and 8b).
- [ ] RD-2 -- wiring: runs plain -> `plain_check_has_no_rev_finding`; the default index -> `rule_reads_the_lock_through_git_index_file`; `observe` blocks -> `observe_never_blocks_a_rev_finding`; `code_recipe` read -> `other_code_recipe_lock_still_checks`; an invalid lock with a candidate passes -> `invalid_lock_is_cannot_check`; `norm_recipe` ignored -> `other_norm_recipe_lock_is_cannot_check`; a lock parsed with no candidate -> `lock_only_commit_never_parses_the_lock`; format 1 refused -> `format_1_head_lock_is_read` (the later of 4b-2 and 5b). `CHECK_CODES` 36; `crates/specengine-store/tests/check_parity.rs` unchanged.
- [ ] RD-3 -- 4b-1's raise removed -> `apply_commits_pass_the_revision_rule` (a real hook, a lock); 4c-1's `lock_commit_passes_spec_check_staged` (CLI `tests/drift_approve.rs`) green unedited, the rule inside.
- [ ] RD-4 -- bump: any write (status, `HEAD`, index, queue) -> `bump_writes_nothing`; `bump_marks_vouched_nodes_required`, `bump_has_no_editorial_flag`, `same_revision_hint_names_bump_and_editorial` (covers verify's hint; accept's refusal, plain and drift, keeps its pins). It may churn 4c-1's hint pin in `verify_states.rs` (4c-1's RD-18): expected.
- [ ] RD-5 -- `verify --staged` (4c-1's seam): a disk read -> `verify_staged_judges_the_index_not_the_disk`, `verify_staged_ignores_a_disk_walk_gap`; `hash-object` -> `verify_staged_runs_no_hash_object`; `.git/index` in place of `GIT_INDEX_FILE` -> `verify_staged_reads_git_index_file`; the disk config -> `verify_staged_reads_the_staged_config` (a staged `[verify] mode` and `[code] roots`); an intent-to-add `spec.lock` read as a lock -> `verify_staged_drops_an_intent_to_add_lock`; with `--changed` -> `staged_and_changed_exclude_each_other`. The code-reviewer: the index side read by `read_commit` and judged by `judge_side`, no second one-side reader (`verify_judges_a_given_walk`, `judge_is_source_neutral` and 4c-1's RD-3 tests green, unedited); `read_head`'s `wanted` holds no (path, OID) the index shares.
- [ ] RD-6 -- a kind literal -> the genre scan; a new dependency -> the CLI pin in `build_graph.rs`.

test-engineer:

- [ ] TE-1 -- AC-13: temp repos from `fixtures/code-a/` with the golden lock `fixtures/spec-lock/code-a.lock` (or CLI `tests/verify_common/`'s four accepted `verify-ecs` nodes), a hook running `spec check --staged --root .` under `enforce`: a locked node edited without a raise refused, naming it and both ways out; raised by hand, whitespace-only, an unlocked node pass; `observe` passes, the finding in `--json`; `enforce-introduced` refuses. M: the rule off. May reuse 4c-1's CLI `tests/drift_common/` (`drifted`, `ecs_drifted`, the `same_revision` edit).
- [ ] TE-2 -- `--changed` = `--staged` on equal trees; plain unchanged.
- [ ] TE-3 -- an invalid lock with a candidate exits 2; an unparseable staged lock with no spec change: clean; another `code_recipe` judged; unmerged; two refreshes through `regen`; a rebase commit through `check --staged` under `enforce` passes (the later of 4b-2 and 5b).
- [ ] TE-4 -- `bump` rows, read-only.
- [ ] TE-5 -- `verify --staged`, index unlike disk; a staged config change judged; under `core.autocrlf`, `--staged` reads `ok`. Lock commits under a `pre-commit` hook running `spec verify --staged`, through `spec lock accept` and through a drift item's approve, in one test (4c-1's `lock_commit_under_spec_verify_hooks`, `tests/drift_approve.rs`, gains the `--staged` hook beside `--changed`): `enforce-introduced` with another node's section deleted on disk (`bad_marker dangling`, a Fix row `HEAD` lacks): passes, the item `applied`, the same hook with `--changed` refuses; `enforce` with a pre-existing Fix row: refused, restored, reopened. The state 4c-1's `lock_restore_commands_quote_the_path` leaves, an intent-to-add `spec.lock`: the next commit under the hook is not exit 2 on `spec.lock`.
- [ ] TE-6 -- here: an ADR edit staged, no finding; the hook untouched. TE-7 -- `fixtures/spec-c`, no finding; the source scan. TE-8 -- two roots, cwd, creation order.
- [ ] TE-9 -- pilots, manual, scratch clones: silent without a lock; accepted in the test's own clone (`SPECENGINE_PILOT_ACCEPT=1`, as `verify_pilot.rs`), the in-scope count and timing; no pilot name committed.

spec-writer:

- [x] SW-1 -- this spec (2026-10-13); `export index`, `check` clean.
- [ ] SW-2 -- at shipping: "Raising"'s rule and bump bullets as built; `spec-check*.md` (the code, the lock read); `spec-lock.md` (plain check never reads the lock, `--staged` and `--changed` do); `spec-verify.md` "Base", "A project's hook" (`--staged`), "Known limits"; `spec-verify-output.md` "Hints" (`spec bump`); `architecture-code.md#sync-in-reads` (Q13); `docs/README.md` "Enforcement" (risk 3); the overridden lines; 08 AC-13 met (Q4); CLI, core, store READMEs; `CLAUDE.md`; `proposal-drift.md` (its lock commit under the `--staged` hook, the rule silent on it); `spec-verify.md` "Base" (the index side through the seam); `spec-check-git.md` "Unlike plain" (the detector's `spec.lock` trigger). Not `drift-queue.md` (shipped, archive-bound): this spec takes over its TE-8 `--staged` half.

## Risks

- **Content never blocks** (risk 3): "Why". Order with 5b: without Q9 the rule and the rebase deadlock.
- `verify --staged` parses changed code blobs (AC-10); the rule parses `HEAD`'s and the checked lock once per candidate run.
- `--staged` binds the whole index tree from blobs (`read_commit`, nothing reused): a hook pays a full code parse, as `--changed`'s walk does (TE-9 times it). An intent-to-add `spec.lock` read as a lock is exit 2 in every mode, refusing every commit (Q18).

## Out of scope

4b-1's parts (`rev-discipline.md`); 4c-2 `drift-follow-up`; slice 5 levels, lock format 2; 8a, 8b. A staged lock writer (Q5); a commit-msg hook; a CI range check; `@rev` pins; raising markers; Health rows; a shipped hook script; a per-rule switch; a lock here.

## Open

- Q2 the rule's scope: nodes a `HEAD` lock entry vouches for; none here.
- Q4 AC-13 becomes "a locked node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries" (08, at shipping).
- Q8 split under ADR-0043, no new ADR: this file 4b-2, `rev-discipline.md` 4b-1 (`rev-discipline.md` "Open").
- Q9 the rule reads `HEAD:spec.lock` and the index's copy inside check's one session; parses them only when a candidate exists (a node with an ID at `HEAD` whose `norm_hash` changed); reads only `node`, `rev`, `spec_hash` (later 8b's blocks) in any format this build reads or rebases from; ignores `code_recipe`; another `norm_recipe`, a newer format or an unparseable lock: cannot check, exit 2. A lock-only commit (accept, editorial accept, regen merge, 4c-1's drift approve, 5b rebase) changes no `norm_hash`, so the rule stays silent.
- Q13 `verify --staged` reuses check's index reader (`ls-files -s -z`, the intent-to-add detector widened to `spec.lock`) through 4c-1's one-commit seam; `architecture-code.md`'s git list amended at shipping (read-only, no ADR).
- `--staged` and `--changed` both kept for verify, mutually exclusive (usage error, exit 2); `--changed` stays the agents' mode.
- Q14 the index side through 4c-1's seam, not a new reader ("spec verify --staged", RD-5). Other answer: two one-side readers kept byte-identical, against the seam's purpose (`verify_rows.rs` module doc).
- Q15 the config under `--staged`: the index's whole `specengine.toml` unless `--config`, as check `--staged`. Other answer: the disk config judges a staged config change under the old one; refusing a dirty config, as the drift raise does, exits 2 on every commit staging a config change.
- **Q16 (owner)** the `spec bump` pointer: not in accept's refusal (it judges `HEAD` with the node's file clean, where bump lists nothing); in verify's hint, worded "while the edit is uncommitted" ("spec bump"). Other answer: a hint false on every accept or drift refusal; or re-worded to `rev=<R+1>` in `spec-lock-accept.md`, `proposal-drift.md` and their pins.
- Q17 causes on the index side: an unmerged path (5), per path; an unreadable index blob or a parser failure (3). Other answer: (7) reads as a partial base while `HEAD` is fine.
- **Q18 (owner)** an intent-to-add `spec.lock`: store `git.rs` `may_be_intent_to_add` widened to `spec.lock`; code files stay a documented limit (`spec-check-git.md` "Unlike plain"). Other answer: an empty-blob lock, which `parse_lock` most likely refuses (inferred, not run), is exit 2 in every mode, and the hook refuses every commit until `git rm --cached spec.lock`.
- Q19 the rule's "approve it through the queue with --editorial" and 4c-1's `DRIFT_HINT` "commit first" (refused by the rule under `enforce`): both texts kept; the pre-commit route is `spec propose update` + `--editorial`, which bump's `hint: required:` names. Other answer: naming the drift route invites `--no-verify`; changing `DRIFT_HINT` churns 4c-1's RD-18 pin.
- Q20 agent-facing text: none (MCP `INSTRUCTIONS` 5 B under the probes build's 2 048 B assert, `ask-owner` 6 B under 4 096 B; roles already forbid hand edits of spec and lock files; the finding and bump's hint carry their way out). If wanted: a role file or `propose-spec-change`, plugin 0.8.1 and a new PINS hash, never `INSTRUCTIONS` or `ask-owner`.
- Q1, Q3, Q5-Q7, Q10-Q12: `rev-discipline.md` "Open".

Q2, Q4 decided 2026-10-10, Q8, Q9, Q13 and `--staged` 2026-10-13, Q14-Q20 2026-10-15 (the refresh against shipped 4c-1), under the owner's pre-approval, review open; Q16 and Q18 flagged for the owner, nothing waits on them. WA-n: the analysis (2026-10-10).

## Implementation

Not started. To record: modules, TE-9's figures, deviations with reasons. Criteria from `rev-discipline.md` before the split (2026-10-13), old -> new: RD-1 -> 1, RD-2 -> 2, RD-5 -> 3, RD-10 -> 4, RD-11 -> 5, RD-12 -> 6 (also 4b-1's RD-7); TE-1 -> 1, TE-4 -> 2, TE-5 -> 3, TE-7 -> 4, TE-8 -> 5, TE-9 -> 6, TE-10 -> 7, TE-11 -> 8, TE-12 -> 9; SW-4's rule part -> SW-2.
