---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-14
---

# `spec lock accept`: the owner's lock commit

`spec lock accept REF [--editorial]` writes REF's entries of `spec.lock` (`docs/canon/spec-lock.md`: format, entries, lockability) from committed bytes and commits the lock alone with provenance, on the owner's terminal (ADR-0041). One library function, `lock_accept`, is the accept, the editorial refresh and a gone node's drop; it takes the owner's consent closure and extra trailers, so a `drift` item's approve (ADR-0045) runs the same accept with its `Proposal:`.

## Steps

1. Not a terminal: exit 2, before anything is read.
2. The project, its root `specengine.toml`, `[code]` (none: exit 2, `spec refs`' text); REF as `spec show` reads it, a written `@rev` dropped with a `note:`.
3. Git: a worktree, HEAD born, on a branch, no operation in progress; the lock read. REF naming entries while no node has that canonical ID: "A gone node".
4. The code walk finds the node's markers. **Inputs**, tracked and equal to HEAD, else each named with its state (`modified`, `staged`, `untracked`, `deleted`, `not tracked`), exit 1 `…: commit them first; nothing written`: the node's file, its markers' files, `specengine.toml`, `spec.lock`; every `.rs`/`.ron` the walk reads that is modified, staged, deleted or untracked; every `Cargo.toml` under a code root or in a directory at or above one, up to the project root (its manifest whenever it exists). Names, `qpath`s, ambiguity depend on them all: one commit, one lock.
5. Each marker file bound, its bound items hashed (`hash_bound_items`), from its HEAD blob's raw bytes (`blob_at`, no filters: `*.rs ident`, `$Id$` stay unexpanded); OIDs from HEAD's tree; the node's revision and `norm_hash` from its file's HEAD blob.
6. **Merge** (`merge_accept`): a lockable binding `new`/`refreshed`, or `unchanged` when equal on `rev`, `spec_hash`, `ast_hash`, `file_blob` (byte-identical, `commit` and `verified_at` too); a binding with a key not lockable now keeps its entry (`kept`); a vanished key is `dropped`. An old entry at R with another `spec_hash` refuses, exit 1 (a meaning change without a raise: "raise its `rev` by hand, commit, then accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`)"; an approve raises it itself, `proposal-revisions.md`). Other nodes' entries keep their bytes.
7. Nothing to change: no question; exit 0 `nothing to change` if some binding is locked as it is, else exit 1 saying why none can be.
8. Before asking: the committer identity, a first lock git ignores, the data directory. Rows on stdout in key order, `<change> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ | <state>]`, change `new`, `refreshed`, `unchanged`, `kept`, `dropped`, `not locked` (keyless: kind `unanchored`/`cannot_verify`, symbol its category); stderr `lock R-12 rev 3 (accept: 2 new, 1 refreshed, 1 kept, 1 dropped) at <commit> on <branch> in <worktree>? [y/N]`, all four counts always; only `y`, `yes`; no `--yes`.
9. HEAD, the branch, the lock's bytes read again; a change: exit 1 `spec.lock changed while being accepted; nothing written`.
10. "The lock commit".

Each accept walks the code tree, hashing the node's items only.

## The lock commit

Written atomically (`replace_file`; a first lock created with an intent-to-add entry); `git commit --only spec.lock`, hooks run, never `--no-verify`: `spec: lock R-12`, an empty line, `Lock: R-12`, `Lock-mode: accept|editorial`, `Decided-by: <committer_ident>`, a caller's trailers; other staged paths stay staged. The commit is the tip of `refs/heads/<branch>`, verified by four checks: its parents exactly the old HEAD; `spec.lock` alone changed; a `Lock:` trailer naming the node; its raw `spec.lock` blob the bytes the owner confirmed (a commit deleting `spec.lock` passes the second, fails this one: "it has no <path>", `<path>` the lock's path from the repository top). **Exit 0 only for a verified commit**, stdout ending `committed <oid>`; history is never rewritten; a refusal prints no `committed` line.

| Git said | The branch | Result |
|---|---|---|
| made | moved, all four hold | exit 0 |
| made | moved, a check fails | exit 1 naming the commit and the reasons, advice `git reset --soft <commit>^` |
| made | moved, a check unreadable | exit 1 "made but cannot be verified", same advice |
| made | unmoved, HEAD moved | exit 1, unverified, nothing restored |
| made | unmoved, HEAD too | bytes and the lock's index entry restored (`reset_path`; a first lock: file and entry removed), exit 1 |
| made | names no commit, unreadable | exit 1, unverified, a `; HEAD names <oid>` hint |
| failed | moved, all four hold | exit 0, a warning (this run's) |
| failed | moved, a check fails | not this run's: bytes restored, exit 1 |
| failed | moved, a check unreadable | exit 1, bytes left as written |
| failed | unmoved | bytes restored, exit 1 "the commit of spec.lock failed, its bytes restored: <error>" |
| failed | names no commit, unreadable | exit 1 "it is unverified and spec.lock is left as written" (the commit may have been made), nothing restored, the hint |

Restoring puts the old bytes back (a first lock: file and intent-to-add entry removed), only while the file still holds what the run wrote; else "spec.lock was changed meanwhile, so it is left as it is, not restored", naming the index entry the run made, left too: a first lock's ", and so is its intent-to-add entry in the index (`git rm --cached -- <path>` drops it)"; after "made, unmoved, HEAD too" ", and so is its index entry, which holds the bytes confirmed (`git reset -q -- <path>` sets it back to HEAD's)". A restore step that fails (the bytes, the index entry, a first lock's file or entry) is a warning naming the git command that does it (`git checkout -- <path>`, `git reset -q -- <path>`, `git rm --cached -- <path>`), and the refusal says "it is not restored (see the warning)", never "its bytes restored". After a failed commit git has rolled its index back itself.

## `--editorial`

Sets `spec_hash` to the current `norm_hash` on the node's entries at R, nothing else (code hashes, `file_blob`, `commit`, `verified_*` kept); `Lock-mode: editorial`. Inputs: the node's file, `specengine.toml`, `spec.lock`; no walk, dirty code ignored. No entry at R (after a raise), or a gone ID: exit 1; all match: exit 0 `nothing to change`. Rows `refreshed`, `unchanged`; asks `lock R-12 rev 3 (editorial: spec_hash of 3 entries) at … [y/N]`.

## A gone node

When the lock holds entries whose `node` is REF's bytes (a written `@rev`, 1-9 digits, stripped) and no node has that canonical ID (deleted, or renamed with the old ID now an alias), a plain accept drops exactly those: the exception to exit 1 "REF names nothing". **Gone only when HEAD agrees**: each `.md` under the root that differs from HEAD (a `git mv`'s old path too) is parsed at HEAD; one still holding the ID is a dirty input, exit 1; a parser panic on it: exit 2. Inputs `specengine.toml`, `spec.lock`; no walk, no hash: a drop writes no hash, so no false `ok`. Rows `dropped | …`; asks `lock R-12 (gone: 3 dropped) at … [y/N]`; the commit as accept's.

## Exits

One-line messages (CLI README "Exit codes and streams"); after a refusal HEAD, refs, index, status and files are as before.

| Exit | Causes |
|---|---|
| 0 | a verified commit; nothing to change |
| 1 | REF names nothing (no gone entries) or several; no marker names the node; nothing lockable; a dirty input; a gone ID held at HEAD; revision unreadable or `norm_hash` null; text changed at R; `--editorial` without an entry at R or on a gone ID; an ignored first lock; not `y`; changed while accepting; the commit failed, failing a check or unverified |
| 2 | not a terminal; usage; discovery, config, `--config` naming another file; no `[code]`; a walk gap; no worktree; unborn or detached HEAD; an operation in progress; an unreadable lock, other recipes; a HEAD blob not UTF-8, or giving other items than the working tree's (a filter); a parser panic at HEAD; no committer identity |

`--json`: one document mirroring the outcome (`reference`, `node`, `rev`, `mode`, `rows`, `commit`, `unchanged`, `refusal`); not pinned by a test.

## Known limits

- Git-ignored `.rs`/`.ron` the walk reads are no input: they can change a `qpath` or an ambiguity unnamed.
- A marker's `id_state` is judged on the working-tree spec corpus: an uncommitted alias or duplicate ID elsewhere changes which markers lock.
- Both leave orphans or unlocked rows at the commit, never a false `ok`.
- A clean filter on `spec.lock` (LFS) fails the fourth check: exit 1 after the commit is made; LFS on code hashes pointer bytes.
- Hooks run on the lock commit (a project's verify hook: `docs/canon/spec-verify.md` "A project's hook"); stdin signing fails. Friction: commit first, one prompt and commit per node.
- Unpinned: a parser panic at HEAD (no input found), "made, the branch unreadable" and "failed, moved, all four hold" (not stageable).
