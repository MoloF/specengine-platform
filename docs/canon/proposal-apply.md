---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-07
---

# Proposal apply: consent, steps, completion, reject

`spec approve` is the one write door into spec files (ADR-0004, ADR-0005), bound to where the proposal was raised (ADR-0032, `docs/canon/architecture.md#apply`). Commands, states, store ops, escaping, git safety: `docs/canon/proposal-queue.md`. An apply writes the target file (or a new file: a decision record, `decision-record.md`; a create's, `proposal-kinds.md`) and one commit in the recorded worktree, the queue and the data directory; nothing else. Code: CLI `apply.rs` (approve, reject, steps 7–10, completion), `preflight.rs` (steps 2–6, the trailer lookup); store `WorktreeGit` (`merge_file`, `commit_only`, git reads), `replace_file`, `update_file`.

## Consent

`spec approve`, `spec reject` and `spec import-state` (`queue-backup.md`) run only when stdin is a terminal (`main.rs`, `IsTerminal`); else exit 2 before anything is read or logged: ``spec: `spec approve` asks the owner for consent on a terminal, and stdin is not one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed``. Claude Code's Bash tool has none, nor presumably the owner's `!` commands: decide in another terminal. No `--yes`. The question goes to stderr, one line is read, only `y` or `yes` (lower case) consents; else exit 1 `` `PR` not applied: the answer was not `y`; nothing changed `` (`not completed`, `not rejected`), no event. Questions, escaped:

- `apply PR-0001 to <path from the worktree top> on <branch> in <worktree> (applies|rebases)? [y/N]`, after steps 2–6 and the identity;
- `complete PR-0001 by its commit <sha> on <branch> in <worktree>? [y/N]` (an `open` proposal's own commit; `in` the current project root when the lookup read the current repository);
- `reject [approved ]PR-0001 (<target_id> in <path> on <branch> in <worktree>)? [y/N]`.

A lookup git could not make is printed above the question (`note: cannot tell whether …`) and again among the outcome's notes. A pseudo-terminal passes: a speed bump, not proof of presence (ADR-0034). Staged choices: `decision-staging.md`.

## Apply steps

`approve_with(&Env, &Globals, &ApproveRequest {id, note, now, git}, &ApproveFlags, consent)` (a decision flag on an update: exit 2): `applied` → exit 1 `` `PR` is already applied: commit <sha> ``; `rejected` → exit 1. Then the trailer lookup ("Completion"); its own commit completes it. Else, in the recorded worktree only:

1. Consent (above).
2. **Place**: the worktree exists, top and common dir as recorded, `HEAD` on the recorded branch with a commit, no merge, rebase, cherry-pick, revert, bisect or sequencer state; else exit 2 (`the proposal's worktree <w> no longer exists`, `a <operation> is in progress in <w>: finish or abort it first`, …).
3. **File**: the recorded root's own `specengine.toml`, its slug the queue's (else exit 2); the target listed by its walk, read by `WorkingTree` of `<worktree>/<root_rel>` with no symlink component, UTF-8, not generated nor immutable, tracked, `git status --porcelain=v1 -z --untracked-files=all` of it empty; else exit 1.
4. **Resolve**: one holder in the recorded root's refreshed index (data directory), at `target_path`, located in a fresh parse of the bytes just read, never by stored offsets; a path target: that parse's document, no holder lookup (one ≠ `target_path` refused; completion alike).
5. **Text**: span hash = `base_hash` → the new text (`applies`); else `git merge-file -p -L current -L base -L proposed` over scratch files: clean → the merge (`rebases`), conflict → exit 1, its text on stdout. Each side of a section gets a final LF, a clean merge loses one: a line added after its last line merges with an edit of the line before. The patched file equal to the file as read → exit 1, already in place (its own commit: completed; another `Proposal:` commit named with why). A text to write and a `Proposal:` commit that applied it on its parent without completing it → exit 1 naming it, never merged again on top.
6. **Structure**: creation's check 3 on this file. Then the identity, `git var GIT_COMMITTER_IDENT` minus the date in the worktree; none → exit 2 (logged as step 7), before the prompt.
7. **Approved**: `approve_from` on the state read; another run's change since → exit 1, nothing written.
8. **Write**: the place re-checked (branch, `HEAD` at step 2's commit, no operation; exit 1 here), the file re-read and equal to step 3's bytes (else exit 1 `` `<path>` changed while being applied; nothing written ``), then replaced atomically: a sibling `.<name>.specengine-<pid>-<n>.tmp` (a dot-name the walk skips) with its mode, synced, renamed over it.
9. **Commit**: `git commit --only --cleanup=verbatim -F <scratch> -- :(literal)<path>`: only the path, whatever else is staged or modified; hooks run, never `--no-verify`. On failure, when the branch's (or `HEAD`'s) new tip carries this ID's `Proposal:` trailer, git made it anyway: a `warning:`, step 10 judges; else the bytes restored, reopened, exit 1 ``the commit of `<path>` failed, its bytes restored: <git's error>`` (+ ``; `<branch>` moved to <sha>, which has no `Proposal: PR` trailer (not this apply's commit)``).
10. **Verify**: the branch's new commit has one parent, the old `HEAD`, changes only the path, its `Proposal:` is the ID → `applied_with` (also when its hold was reopened meanwhile; recorded by another run: done, a note), then the index's `update_paths` (failure: a `warning:`). Else it stays `approved`, exit 1 naming the commit (the branch unmoved: `HEAD`'s) and the hint.

**Runs.** No lock until the daemon: runs meet in the queue. A run holds nothing before its own step 7, so a refusal there (steps 2–6, identity, step 7's compare-and-set) only logs (`log_failure`): `open` stays open, another run's `approved` stays approved. At steps 8–9 it reopens (`reopen_from`) only while the proposal is in the state it wrote; step 10 never reopens, a commit exists. One `proposal.apply_failed` per refusal at steps 2–10; none for the terminal check, a decline, another repository, a bad ID, an applied or rejected one.

**Commit** (core `commit_message`; an update's commit is its record):

```
spec: apply PR-0001

<rationale, verbatim>

Proposal: PR-0001
Decided-by: Ann Owner <ann@example.org>
Proposed-by: agent role=spec-writer model=claude-opus-5-5 run=unknown
Base-commit: <40 or 64 hex>
```

`Decided-by`: the step-7 identity. `Proposed-by`: `<agent|human> role=<r> model=<m> run=<run>`, an absent field `unknown`.

## Completion

A proposal's own commit on its branch completes it, no new commit: an interrupted apply, a cherry-pick, a split. **Where**: the recorded worktree when it is a directory of the recorded repository; else the current repository when it is the recorded one (worktree removed); else refused (`the worktree <w> is not there and its repository <dir> is not the current one`, `<w> is no longer a git worktree, and …`, `the worktree <w> belongs to another repository (<dir>), and …`). **Which**: commits with `Proposal: <id>` in `base_commit..refs/heads/<branch>`, newest first; the base commit not in the repository (pruned after a rebase): the branch's whole history. A gone branch fails the lookup: ``the branch `t1` no longer exists; recreate it at its last commit (`git branch t1 <commit>`), then `spec approve PR` or `spec reject PR` ``; base gone too: ``the branch `t1` no longer exists, and the proposal's base commit <sha> is not in the repository; recreate the branch at its last commit (…), then …``. **Completes**: one parent, only the target's path changed, its span equal to step 5's result on the first parent's blob (what an apply wrote on top of it) or on its own blob (the new text; a split or cherry-picked merge), each blob raw (`cat-file blob`) parsed under the `<root_rel>/specengine.toml` of that commit's tree, else the one read now (the recorded root's; the current project's for a current-repository read). Else why: `changes a, b`, `has N parents`, `does not carry the proposal's text`, `cannot be read: …`.

Looked up by approve before step 2 and at step 5, by review, by reject before and after its prompt. Completing: `approved → applied`, no prompt; `open → applied` after the `complete` question, the identity of where it was looked up (none → exit 2, logged as step 7); note `` `PR` completed by its commit <sha> on `<branch>`; no new commit ``; writes the queue and the data directory's index only (failed reindex: a `warning:`). A failed lookup is a note for approve and review (appended to an exit-2 reason at steps 2–6) and refuses reject. Hint (steps 5, 10, reject): ``a commit on `<branch>` with the trailer `Proposal: PR`, one parent, changing only `<path>` to the proposal's text completes it (`spec approve PR`)``.

## Reject

`reject(&Env, &Globals, &RejectRequest {id, reason, now, git}, consent)`: `open` or `approved` only (a question or discrepancy: `decision-record.md` "Reject"; else exit 1 `` `PR` is applied: only an open or approved proposal is rejected (commit <sha>) ``), never with a `Proposal:` commit on its branch, checked before and after the prompt; refused: nothing written, no event:

- its completing commit → exit 1 `` `PR` has its commit <sha> on `<branch>`: a proposal whose commit is in history is never rejected; `spec approve PR` completes it ``;
- another → the newest named, ` (<k> older one(s) too)`, why it does not complete, the hint;
- a failed lookup → exit 1 `` cannot tell whether `PR` has its commit in history: <why, with the way out> ``.

Then `reject_from` on the state read (changed since → exit 1); `decision_note` the reason, `proposal.rejected`. `decided_by`: the committer where the history was read, else the current repository's; none → exit 2.

**Orphans** (`proposal-queue.md`): rejected `open` or `approved`, with the note `` `PR` belongs to the repository <dir>, which no longer exists ``. The lookup runs in the current repository on the branch of its name: skipped when neither branch nor base commit is there (another repository); the branch alone missing refuses with the way out ending ``then `spec reject PR` ``; a commit found refuses, suffixed ``; it was recorded in the repository <dir>, now gone or moved: move the repository back to <dir> (`git worktree repair`), then `spec approve PR` `` (`<dir>` the recorded common dir, `…/.git`).

## Known limits

Accepted at shipping (2026-10-05); none blocks (ADR-0012).

- A pseudo-terminal passes consent; racing terminals meet git's `index.lock` (reported); hooks or signing needing stdin fail (bytes restored); Phase 3's rev rule (08 AC-13) will refuse apply commits without a rev bump.
- A reject running while a live apply commits between reject's second check and its write leaves it rejected with its commit in history (the apply's step 10 exits 1, `rejected`): needs a cross-process lock (daemon).
- An end-of-line or clean/smudge filter on spec files (`core.autocrlf`, `eol=crlf`): completion compares raw blobs with worktree bytes, so an apply interrupted at step 10 can be neither completed nor rejected (later: `cat-file --filters`).
- The trailer names no project: with several SpecEngine roots in a repository, or a reset data directory, another proposal's same-ID commit on the branch blocks reject; with the base pruned the whole-branch read (its cost grows with history) finds older ones too; approve refuses only when that commit applied the text.
- A reverted apply commit: approve refused at step 5, reject refused; way out: a completing commit (the hint).
- A path target follows no rename (step 3 refuses; no re-targeting).
- An orphan whose branch was renamed and base pruned is rejected without the lookup, even with its commit on the renamed branch.
- An orphan rejected from a clone that never had its branch refuses (branch missing): `git branch <branch> HEAD`, `spec reject`, delete the branch; the texts' "unless its commit is in history" then misleads.
- A transient failure of only step 5's lookup is dropped (no note); consent still precedes the write.
- `inbox --all`'s gone note lists gone applied and rejected IDs too.
