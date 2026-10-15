---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-15
---

# Proposal apply: consent and steps

`spec approve` is the one write door into spec files (ADR-0004, ADR-0005), bound to where the proposal was raised (ADR-0032, `architecture.md#apply`). Commands, states, store ops, escaping, git safety: `proposal-queue.md`; completion, reject: `proposal-completion.md`; revisions, `--editorial`: `proposal-revisions.md`. An apply writes the target file (or a new file: a record, `decision-record.md`; a create's, an amendment's, `proposal-create-apply.md`; `--editorial` also `spec.lock`; a `drift` item `spec.lock` alone, by its node's lock accept: `proposal-drift.md` "Approve") and one commit in the recorded worktree, the queue and the data directory; nothing else. Code: CLI `apply.rs` (approve, reject, steps 7-10, completion), `preflight.rs` (steps 2-6, the trailer lookup); store `WorktreeGit` (`merge_file`, `commit_only{,_paths}`, git reads), `replace_file`, `update_file`.

## Consent

`spec approve`, `spec reject`, `spec import-state` (`queue-backup.md`) and `spec round answer` (`round.md`) run only when stdin is a terminal; else exit 2 before anything is read or logged: ``spec: `spec approve` asks the owner for consent on a terminal, and stdin is not one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed``. No `--yes`. The question goes to stderr, one line is read, only `y` or `yes` (lower case) consents; else exit 1 `` `PR` not applied: the answer was not `y`; nothing changed `` (`not completed`, `not rejected`), no event. Questions, escaped:

- `apply PR-0001 to <path from the worktree top> on <branch> in <worktree> (applies|rebases[; raises R-12 3 -> 4])? [y/N]`, after steps 2-6 and the identity;
- `complete PR-0001 by its commit <sha> on <branch> in <worktree>? [y/N]` (an `open` proposal's own commit; `in` the current project root when the lookup read the current repository);
- `reject [approved ]PR-0001 (<target_id> in <path> on <branch> in <worktree>)? [y/N]`.

A lookup git could not make is printed above the question (`note: cannot tell whether …`) and in the outcome's notes. A pseudo-terminal passes (ADR-0034). A stage it confirms is printed above, the question marked `staged` (`decision-staging.md` "Terminal").

## Apply steps

`approve_with(…, &ApproveRequest, &ApproveFlags, consent)` (a decision flag on a kind that does not decide: exit 2, `flag_on_apply`, a drift item's naming ``spec approve PR runs the lock accept for <ID>``): `applied` -> exit 1 `` `PR` is already applied: commit <sha> ``; `rejected` -> exit 1. Then the trailer lookup (`proposal-completion.md` "Completion"); its own commit completes it. A drift item then takes `run_approve`'s drift branch (`drift_apply`): apply's steps 1, 2, 7-10 around its node's `lock_accept`. Else, in the recorded worktree only:

1. Consent (above).
2. **Place**: the worktree exists, top and common dir as recorded, `HEAD` on the recorded branch with a commit, no merge, rebase, cherry-pick, revert, bisect or sequencer state; else exit 2 (`a <operation> is in progress in <w>: finish or abort it first`, …).
3. **File**: the recorded root's own `specengine.toml`, its slug the queue's (else exit 2); the target listed by its walk, read by `WorkingTree` of `<worktree>/<root_rel>` with no symlink component, UTF-8, not generated nor immutable, tracked, `git status --porcelain=v1 -z --untracked-files=all` of it empty; else exit 1. `--editorial`: `spec.lock` too.
4. **Resolve**: one holder in the recorded root's refreshed index (data directory), at `target_path`, located in a fresh parse of the bytes just read, never by stored offsets; a path target: that parse's document, no holder lookup (one ≠ `target_path` refused; completion alike).
5. **Text**: span hash = `base_hash` -> the new text (`applies`); else `git merge-file -p -L current -L base -L proposed` over scratch files: clean -> the merge (`rebases`), conflict -> exit 1, its text on stdout. Each side of a section gets a final LF, a clean merge loses one: a line added after its last line merges with an edit of the line before. The patched file equal to the file as read -> exit 1, already in place (its own commit: completed; another `Proposal:` commit named with why). A text to write and a `Proposal:` commit that applied it on its parent without completing it -> exit 1 naming it, never merged again on top.
6. **Structure**: creation's check 3 on this file; the raise. Then the identity, `git var GIT_COMMITTER_IDENT` minus the date in the worktree; none -> exit 2 (logged as step 7), before the prompt.
7. **Approved**: `approve_from` on the state read; another run's change since -> exit 1, nothing written.
8. **Write**: the place re-checked (branch, `HEAD` at step 2's commit, no operation; exit 1 here), the file re-read and equal to step 3's bytes (else exit 1 `` `<path>` changed while being applied; nothing written ``), then replaced atomically: a sibling `.<name>.specengine-<pid>-<n>.tmp` (a dot-name the walk skips) with its mode, synced, renamed over it; `--editorial`: then `spec.lock`.
9. **Commit**: `git commit --only --cleanup=verbatim -F <scratch> -- :(literal)<path>` (`--editorial`: `spec.lock` too): only the path, whatever else is staged or modified; hooks run, never `--no-verify`. On failure, when the branch's (or `HEAD`'s) new tip carries this ID's `Proposal:` trailer, git made it anyway: a `warning:`, step 10 judges; else the bytes restored, reopened, exit 1 ``the commit of `<path>` failed, its bytes restored: <git's error>`` (+ ``; `<branch>` moved to <sha>, which has no `Proposal: PR` trailer (not this apply's commit)``).
10. **Verify**: the branch's new commit has one parent, the old `HEAD`, changes only the path (`--editorial`: and `spec.lock`), its `Proposal:` is the ID -> `applied_with` (task-bound: `applied_refreshing` refreshes its task's snapshot, `tasks.md`; also when its hold was reopened meanwhile; recorded by another run: done, a note), then the index's `update_paths` (failure: a `warning:`). Else it stays `approved`, exit 1 naming the commit (the branch unmoved: `HEAD`'s) and the hint.

**Runs.** No lock until the daemon: runs meet in the queue. A run holds nothing before its own step 7, so a refusal there (steps 2-6, identity, step 7's compare-and-set) only logs (`log_failure`): `open` stays open, another run's `approved` stays approved. At steps 8-9 it reopens (`reopen_from`) only while the proposal is in the state it wrote; step 10 never reopens, a commit exists. One `proposal.apply_failed` per refusal at steps 2-10; none for the terminal check, a decline, another repository, a bad ID, an applied or rejected one.

**Commit** (core `commit_message`; an update's commit is its record):

```
spec: apply PR-0001

<rationale, verbatim>

Proposal: PR-0001
Decided-by: Ann Owner <ann@example.org>
Proposed-by: agent role=spec-writer model=claude-opus-5-5 run=unknown
Base-commit: <40 or 64 hex>
```

`Decided-by`: the step-7 identity. `Proposed-by`: `<agent|human> role=<r> model=<m> run=<run>`, an absent field `unknown`. An interpretation adds a consent line, a step-4 check of its decision, two paragraphs, `Interprets: <ID>` last: `proposal-interpretation.md` "Apply". `Revision:`, `Lock:`, `Lock-mode:` follow `Base-commit:`.

## Known limits

None blocks; completion's and reject's: `proposal-completion.md` "Known limits"; the inbox's: `proposal-review.md`.

- Racing terminals meet git's `index.lock` (reported); hooks or signing needing stdin fail (bytes restored); revisions: `proposal-revisions.md`.
- A path target follows no rename (step 3 refuses; no re-targeting).
