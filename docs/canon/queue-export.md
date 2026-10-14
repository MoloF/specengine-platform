---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-14
---

# Queue export: bounds and placement

What `spec export state` does between its arguments and the dump (ADR-0003: a dump never goes into the repository). `<D>`, the destination, the command and its outcome: `queue-backup.md` "Commands"; the dump: "Format" there. Code: CLI `state.rs`; store `stored_rows`, `WorktreeGit::{top_if_repository, worktrees}` (`queue-backup.md` "Store").

## Steps

In order, the first refusal exit 2; nothing is written before step 5.

1. **Bounds**, the directories a dump never goes inside: the root's worktree top (`rev-parse --show-toplevel`); the main worktree, the common dir's parent when the common dir is named `.git` and git's main entry is not bare; every entry of `git worktree list --porcelain -z` (git ≥ 2.36; main first, pruned ones kept; a bare main entry, or one that is the common dir (`--separate-git-dir`), is its git directory); the common dir. Git finding no repository from the root (exit 128, `fatal: not a git repository (or any ` under `LC_ALL=C`): the root alone. Any other git failure (dubious ownership, a broken `.git` file, the root inside a git directory, no `git`, an older git) -> ``cannot tell whether the destination <D> lies inside the repository of the project root <R>: <git error>; nothing written``.
2. `<D>` resolved (nearest existing ancestor canonical, the rest as written) is **inside** a bound when it starts with the bound's canonical path, or when one of its existing ancestors has the bound's device and inode (a macOS firmlink, a case-folded name) -> ``the destination <D> lies inside <the worktree X of the project root | the worktree X of the project root's repository | the git directory X of the project root's repository>: a dump holds proposal texts, paths and identities and never goes into the repository; nothing written``.
3. `<D>` exists (a file, a directory, a symlink, a dangling one too) -> ``the destination <D> exists: a dump never replaces a file; name a new one; nothing written``; `--out`'s parent no existing directory -> ``the destination's directory <P> does not exist: `--out` names a new file in an existing directory; nothing written``.
4. Both tables read raw in one read transaction (`stored_rows`, never `list_readable`); no DB file or no queue tables: an empty dump, `0, 0`, nothing created. A row of another `project` -> ``proposal `PR-…` in <db> is of the project `<p>`, not `<slug>`'s: the dump holds one project's queue; nothing written`` (`event <seq> …`; `of no project (NULL)`).
5. **Placement**: the default's missing directories made (`backups/` 0700, the data directory as `create_dir_all` makes it); `<D>.partial` created new (0600), written, `fsync`ed, then hard-linked to `<D>` (`link` never replaces a name: a `<D>` appearing meanwhile is step 3's refusal, its bytes untouched; a file system without hard links, exFAT or some network shares: a rename after the check); the partial removed, the directory `fsync`ed. Any failure removes the partial and the directories this run made (innermost first, only while empty; one that stays is named). A stale partial -> ``cannot create <D>.partial: … (left by an export that stopped: remove it); nothing written``, never removed. A partial left or a directory not synced after the dump is whole: exit 0 with a `warning:`.

## Known limits

None blocks (ADR-0017); the backup's own: `queue-backup.md` "Known limits".

- Bounds: git seeing no repository from the root leaves the root as the only bound; another name for a directory inside a worktree (a bind mount) is not recognised (firmlinks are); the common dir's parent counts only when the common dir is named `.git`, so `--separate-git-dir` run from a linked worktree leaves the main checkout unprotected; a submodule root's superproject is not a bound; the data directory is judged against the root by path only.
- Placement: a crash between the link and the partial's removal leaves `<D>` whole beside its partial; without hard links, a file appearing between the check and the rename is replaced.
