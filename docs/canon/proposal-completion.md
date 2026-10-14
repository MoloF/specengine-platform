---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-14
---

# Proposal completion and reject

How a proposal closes without a new apply: its own commit completes it, or `spec reject` closes it unapplied (ADR-0004, ADR-0005, ADR-0032). Consent, the questions and the apply steps named here: `proposal-apply.md`; a record's, a create's: `decision-record.md`, `proposal-create-apply.md`; revisions: `proposal-revisions.md` "Completion and reject". Code: CLI `apply.rs` (reject, completion), `preflight.rs` (the trailer lookup); store `WorktreeGit::{branch_commits_with_trailer, name_status}`.

## Completion

A proposal's own commit on its branch completes it, no new commit: an interrupted apply, a cherry-pick, a split. **Where**: the recorded worktree when it is a directory of the recorded repository; else the current repository when it is the recorded one (worktree removed); else refused, naming why (the worktree gone and its repository not the current one, no longer a git worktree, another repository's). **Which**: commits with `Proposal: <id>` in `base_commit..refs/heads/<branch>`, newest first; the base commit not in the repository (pruned after a rebase): the branch's whole history. A gone branch fails the lookup: ``the branch `t1` no longer exists; recreate it at its last commit (`git branch t1 <commit>`), then `spec approve PR` or `spec reject PR` ``; base gone too, adding that its base commit <sha> is not in the repository. **Completes**: one parent, only the target's path changed (or with `spec.lock`, `Lock-mode: editorial`), its span equal to step 5's result, raised or not, on the first parent's blob (what an apply wrote on top of it) or on its own blob (the new text; a split or cherry-picked merge), each blob raw (`cat-file blob`) parsed under the `<root_rel>/specengine.toml` of that commit's tree, else the one read now (the recorded root's; the current project's for a current-repository read). Else why: `changes a, b`, `has N parents`, `does not carry the proposal's text`, `cannot be read: …`.

Looked up by approve before step 2 and at step 5, by review, by reject before and after its prompt. Completing (a task's snapshot refreshed as at step 10): `approved -> applied`, no prompt; `open -> applied` after the `complete` question, the identity of where it was looked up (none -> exit 2, logged as step 7); note `` `PR` completed by its commit <sha> on `<branch>`; no new commit ``; writes the queue and the data directory's index only (failed reindex: a `warning:`). A failed lookup is a note for approve and review (appended to an exit-2 reason at steps 2-6) and refuses reject. Hint (steps 5, 10, reject): ``a commit on `<branch>` with the trailer `Proposal: PR`, one parent, changing only `<path>` to the proposal's text completes it (`spec approve PR`)``.

## Reject

`reject(…, &RejectRequest {id, reason?, …}, consent)`: `open` or `approved` only (a deciding kind: `decision-record.md` "Reject"; else exit 1 `` `PR` is applied: only an open or approved proposal is rejected (commit <sha>) ``), never with a `Proposal:` commit on its branch, checked before and after the prompt; refused: nothing written, no event:

- its completing commit -> exit 1 `` `PR` has its commit <sha> on `<branch>`: a proposal whose commit is in history is never rejected; `spec approve PR` completes it ``;
- another -> the newest named, ` (<k> older one(s) too)`, why it does not complete, the hint;
- a failed lookup -> exit 1 `` cannot tell whether `PR` has its commit in history: <why, with the way out> ``.

Then `reject_from` on the state read (changed since -> exit 1); `decision_note` the reason, `proposal.rejected`. `decided_by`: the committer where the history was read, else the current repository's; none -> exit 2.

**Orphans** (`proposal-queue.md` "Place, IDs, repositories"): rejected `open` or `approved`, with the note `` `PR` belongs to the repository <dir>, which no longer exists ``. The lookup runs in the current repository on the branch of its name: skipped when neither branch nor base commit is there (another repository); the branch alone missing refuses with the way out ending ``then `spec reject PR` ``; a commit found refuses, suffixed ``; it was recorded in the repository <dir>, now gone or moved: move the repository back to <dir> (`git worktree repair`), then `spec approve PR` `` (`<dir>`: the recorded common dir).

## Known limits

None blocks; the apply's own: `proposal-apply.md` "Known limits".

- A reject running while a live apply commits between reject's second check and its write leaves it rejected with its commit in history (the apply's step 10 exits 1, `rejected`): needs a cross-process lock.
- An end-of-line or clean/smudge filter on spec files (`core.autocrlf`, `eol=crlf`): completion compares raw blobs with worktree bytes: an apply interrupted at step 10 is neither completed nor rejected (later: `cat-file --filters`).
- The trailer names no project: with several SpecEngine roots in a repository, or a reset data directory, another proposal's same-ID commit on the branch blocks reject; with the base pruned the whole-branch read finds older ones too; approve refuses only when that commit applied the text.
- A reverted apply commit: approve refused at step 5, reject refused; way out: a completing commit (the hint).
- An orphan whose branch was renamed and base pruned is rejected without the lookup, even with its commit on the renamed branch.
- An orphan rejected from a clone that never had its branch refuses (branch missing): `git branch <branch> HEAD`, `spec reject`, delete the branch.
- A transient failure of only step 5's lookup is dropped (no note); consent still precedes the write.
