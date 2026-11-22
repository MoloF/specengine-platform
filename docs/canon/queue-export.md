---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-11-22
---

# Queue export and import: the steps

What `spec export state` does between its arguments and the dump (ADR-0003: a dump never goes into the repository), and `spec import-state`'s steps. `<D>`, the destination, the command and its outcome: `queue-backup.md` "Commands"; the dump: "Format" there. Code: CLI `state.rs`; store `stored_rows`, `WorktreeGit::{top_if_repository, worktrees}` (`queue-backup.md` "Store").

## Steps

In order, the first refusal exit 2; nothing is written before step 5.

1. **Bounds**, the directories a dump never goes inside: the root's worktree top (`rev-parse --show-toplevel`); the main worktree, the common dir's parent when the common dir is named `.git` and git's main entry is not bare; every entry of `git worktree list --porcelain -z` (git ≥ 2.36; main first, pruned ones kept; a bare main entry, or one that is the common dir (`--separate-git-dir`), is its git directory); the common dir. Git finding no repository from the root (exit 128, `fatal: not a git repository (or any ` under `LC_ALL=C`): the root alone. Any other git failure (dubious ownership, a broken `.git` file, the root inside a git directory, no `git`, an older git) -> ``cannot tell whether the destination <D> lies inside the repository of the project root <R>: <git error>; nothing written``.
2. `<D>` resolved (nearest existing ancestor canonical, the rest as written) is **inside** a bound when it starts with the bound's canonical path, or when one of its existing ancestors has the bound's device and inode (a macOS firmlink, a case-folded name) -> ``the destination <D> lies inside <the worktree X of the project root | the worktree X of the project root's repository | the git directory X of the project root's repository>: a dump holds proposal texts, paths and identities and never goes into the repository; nothing written``.
3. `<D>` exists (a file, a directory, a symlink, a dangling one too) -> ``the destination <D> exists: a dump never replaces a file; name a new one; nothing written``; `--out`'s parent no existing directory -> ``the destination's directory <P> does not exist: `--out` names a new file in an existing directory; nothing written``.
4. Every table read raw in one read transaction (`stored_rows_with_comments`, never `list_readable`); no DB file or no queue tables: an empty dump, counts 0, nothing created. A row of another `project` -> ``proposal `PR-…` in <db> is of the project `<p>`, not `<slug>`'s: the dump holds one project's queue; nothing written`` (`event <seq> …`; `of no project (NULL)`); a comment's ``comment `CM-…` in <db> is not `<slug>`'s: …`` (``version <n> of comment `CM-…` …``).
5. **Placement**: the default's missing directories made (`backups/` 0700, the data directory as `create_dir_all` makes it); `<D>.partial` created new (0600), written, `fsync`ed, then hard-linked to `<D>` (`link` never replaces a name: a `<D>` appearing meanwhile is step 3's refusal, its bytes untouched; a file system without hard links, exFAT or some network shares: a rename after the check); the partial removed, the directory `fsync`ed. Any failure removes the partial and the directories this run made (innermost first, only while empty; one that stays is named). A stale partial -> ``cannot create <D>.partial: … (left by an export that stopped: remove it); nothing written``, never removed. A partial left or a directory not synced after the dump is whole: exit 0 with a `warning:`.

## Import

In order; nothing is written before step 5.

1. `main`: stdin not a terminal -> exit 2 before `FILE` is opened, approve's message naming `spec import-state` (`proposal-apply.md` "Consent"). No `--yes`.
2. `FILE` (current-directory relative, symlinks followed) a regular file, checked before the open and on the opened handle, else ``cannot read <FILE>: not a regular file (a dump is one file)``, unread. Read whole and checked; the first defect -> ``<FILE>:<line>: <defect>``, never quoting the line or an unknown name: not UTF-8, not JSON, not a JSON object, an empty line or file, no final LF; no header, a header without exactly its format's keys (`queue-backup.md` "Format") or with a value of the wrong type; `format` or `queue_schema` above the build's (`… upgrade SpecEngine`), `format` 0, `queue_schema` not its format's (``... (it holds queue schemas 4 to 8)``); `project` not the root's slug (both named); a row of a table its format lacks; a column missing, extra or repeated (any repeated key); TEXT not a string or `null`; `seq` not an integer ≥ 1 (`12.0` too); `id` not `PR-` (`T-`, `C-`, `CM-`) and 4 or more digits as the queue writes it, numbered 1 to 2⁶⁴−1; `project` not the header's; an `id` or `seq` repeating an earlier line's (named). Then rows read ≠ header counts -> ``<FILE>: header counts <p>, <e>; found <p'>, <e'>`` (the format's counts each). Rows in any order, inserted in file order; CRLF and whitespace inside a line accepted.
3. The data directory checked, not created; any table holding a row (any project) -> ``the queue of `<slug>` in <db> holds <counts>: import-state restores only into an empty queue (a fresh data directory, or <db> moved aside); nothing changed``, no question.
4. stderr ``restore <p> proposal(s), <t> task(s), <r> run(s), <c> card(s) and <e> event(s) of <slug> from <FILE> into <db>? [y/N]``, one line read, only `y` or `yes`; else exit 1, ``spec: not restored: the answer was not `y`; nothing changed``, stdout empty (`--json` counts 0).
5. The data directory and DB made when absent (schema steps), then `restore_with_comments`: one `Immediate` transaction, step 3 again under the write lock (filled meanwhile: refused, nothing inserted), the project's collision ledger emptied, every row inserted as given, commit. No event of its own: the next ID and `seq` are the highest restored + 1, the ID never below a stored floor (`queue-backup.md` "ID floor").

**As stored**: no path rewritten. A proposal whose recorded common dir is gone is an orphan (`proposal-queue.md` "Place, IDs, repositories"), taken only by reject; same machine: move the repository back, `git worktree repair`.

## Known limits

None blocks a local daemon (ADR-0058); a hub rests on its dumps (`hub-ops.md`); the backup's own, the import's among them: `queue-backup.md` "Known limits".

- Bounds: git seeing no repository from the root leaves the root as the only bound; another name for a directory inside a worktree (a bind mount) is not recognised (firmlinks are); the common dir's parent counts only when the common dir is named `.git`, so `--separate-git-dir` run from a linked worktree leaves the main checkout unprotected; a submodule root's superproject is not a bound; the data directory is judged against the root by path only.
- Placement: a crash between the link and the partial's removal leaves `<D>` whole beside its partial; without hard links, a file appearing between the check and the rename is replaced.
