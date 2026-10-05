---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-05
---

# Queue backup and restore

The queue (`proposals`, `events` in `<slug>.db`, `docs/canon/proposal-queue.md` "Store") is the one state git cannot rebuild. `spec export state` dumps it to a JSONL file outside the repository; `spec import-state` restores a dump into an empty queue (ADR-0003; one write door: ADR-0004, ADR-0005; single user: ADR-0012; no re-targeting: ADR-0032). No automatic backup or retention (the daemon's), no `specengine.toml` key. Code: CLI `state.rs` (both commands), `state_file.rs` (the format), `main.rs` (the terminal check); store `queue/state.rs`, `WorktreeGit::{top_if_repository, worktrees}`.

## Commands

- `spec export state [--out PATH]`: the root slug's whole queue, every repository of the slug, unreadable rows included; no consent, no index refresh. Library `export_state(&Env, &Globals, &ExportStateRequest {out, now, git})` → `ExportStateOutcome {path, proposals, events, messages}`. stdout `wrote <D>: <p> proposal(s), <e> event(s)`, `--json` `{path, proposals, events}`, never the dump. `<D>`: `--out` joined to the current directory as given (not canonicalised), else `<data dir>/backups/<slug>-<YYYYMMDDTHHMMSSZ>.jsonl` (UTC, the injected clock; a second default export within one second is refused as existing).
- `spec import-state FILE`: into the slug's empty queue, after one question on a terminal. Library `import_state(&Env, &Globals, &ImportStateRequest {file}, Consent)` → `ImportStateOutcome {db, proposals, events, refusal, messages}`. stdout `restored <p> proposal(s), <e> event(s) into <db>`, `--json` `{db, proposals, events}`.
- Both read only the root's own `specengine.toml` (another `--config` → exit 2, the queue's `drop --config` message) and check the data directory as the queue does (inside the root → exit 2) without creating it. Text, errors and the prompt are escaped and one-lined as the queue's (`proposal-queue.md` "Terminal and git safety").
- Exit 0 done; 1 the import declined; 2 every refusal below, nothing written (export) or changed (import).

## Format

`STATE_FORMAT` = 1 (CLI): UTF-8 compact JSON, one object per LF-ended line.

1. The header `{"format":1,"queue_schema":1,"project":"<slug>","proposals":<p>,"events":<e>}`, keys in this order: `STATE_FORMAT`, the store's `QUEUE_SCHEMA_VERSION`, the root's slug, the row counts of the same snapshot.
2. Every `proposals` row by ID number (`ORDER BY length(id), id`), then every `events` row by `seq`, each `{"<table>":{…}}` with every column in table order (`PROPOSAL_COLUMNS`, `EVENT_COLUMNS`): TEXT a string (`serde_json` escapes), NULL `null`, `seq` a number; `author`, `diagnostics`, `payload` stay the strings stored.

No export time, host or rowid inside: equal queues give byte-identical dumps, whatever their insertion order. Test oracle: `SqliteQueue::dump()`. Backing up another table (tasks, runs) extends the format in its own slice.

```
{"format":1,"queue_schema":1,"project":"lantern-keep","proposals":1,"events":1}
{"proposals":{"id":"PR-0001","project":"lantern-keep","kind":"update",…,"updated_at":"2026-10-05T12:00:00Z"}}
{"events":{"seq":1,"project":"lantern-keep","type":"proposal.created","payload":"{\"id\":\"PR-0001\"}","at":"2026-10-05T12:00:00Z"}}
```

## Export

In order, the first refusal exit 2; nothing is written before step 5.

1. **Bounds**, the directories a dump never goes inside (it holds proposal texts, paths, identities): the root's worktree top (`rev-parse --show-toplevel`); the main worktree, the common dir's parent when the common dir is named `.git` and git's main entry is not bare; every entry of `git worktree list --porcelain -z` (git ≥ 2.36; main first, pruned ones kept; a bare main entry, or one that is the common dir (`--separate-git-dir`), is its git directory); the common dir. Git finding no repository from the root (exit 128, `fatal: not a git repository (or any ` under `LC_ALL=C`): the root alone. Any other git failure (dubious ownership, a broken `.git` file, the root inside a git directory, no `git`, an older git) → ``cannot tell whether the destination <D> lies inside the repository of the project root <R>: <git error>; nothing written``.
2. `<D>` resolved (nearest existing ancestor canonical, the rest as written) is **inside** a bound when it starts with the bound's canonical path, or when one of its existing ancestors has the bound's device and inode (a macOS firmlink, a case-folded name) → ``the destination <D> lies inside <the worktree X of the project root | the worktree X of the project root's repository | the git directory X of the project root's repository>: a dump holds proposal texts, paths and identities and never goes into the repository; nothing written``.
3. `<D>` exists (a file, a directory, a symlink, a dangling one too) → ``the destination <D> exists: a dump never replaces a file; name a new one; nothing written``; `--out`'s parent no existing directory → ``the destination's directory <P> does not exist: `--out` names a new file in an existing directory; nothing written``.
4. Both tables read raw in one read transaction (`stored_rows`, never `list_readable`); no DB file or no queue tables: an empty dump, `0, 0`, nothing created. A row of another `project` → ``proposal `PR-…` in <db> is of the project `<p>`, not `<slug>`'s: the dump holds one project's queue; nothing written`` (`event <seq> …`; `of no project (NULL)`).
5. **Placement**: the default's missing directories made (`backups/` 0700, the data directory as `create_dir_all` makes it); `<D>.partial` created new (0600), written, `fsync`ed, then hard-linked to `<D>` (`link` never replaces a name: a `<D>` appearing meanwhile is step 3's refusal, its bytes untouched; a file system without hard links, exFAT or some network shares: a rename after the check); the partial removed, the directory `fsync`ed. Any failure removes the partial and the directories this run made (innermost first, only while empty; one that stays is named). A stale partial → ``cannot create <D>.partial: … (left by an export that stopped: remove it); nothing written``, never removed. A partial left or a directory not synced after the dump is whole: exit 0 with a `warning:`.

## Import

In order; nothing is written before step 5.

1. `main`: stdin not a terminal → exit 2 before `FILE` is opened, approve's message naming `spec import-state` (`proposal-apply.md` "Consent"). No `--yes`.
2. `FILE` (current-directory relative, symlinks followed) a regular file, checked before the open and on the opened handle, else ``cannot read <FILE>: not a regular file (a dump is one file)``, unread. Read whole and checked; the first defect → ``<FILE>:<line>: <defect>``, never quoting the line or an unknown name: not UTF-8, not JSON, not a JSON object, an empty line or file, no final LF; no header, a header without exactly the five keys or with a value of the wrong type; `format` or `queue_schema` above the build's (`… upgrade SpecEngine`) or below; `project` not the root's slug (both named); a row not `{"proposals"|"events":{…}}`; a column missing, extra or repeated (any repeated key); TEXT not a string or `null`; `seq` not an integer ≥ 1 (`12.0` too); `id` not `PR-` and 4 or more digits as the queue writes it, numbered 1 to 2⁶⁴−1; `project` not the header's; an `id` or `seq` repeating an earlier line's (named). Then rows read ≠ header counts → ``<FILE>: header counts <p>, <e>; found <p'>, <e'>``. Rows in any order, inserted in file order; CRLF and whitespace inside a line accepted.
3. The data directory checked, not created; either table holding a row (any project) → ``the queue of `<slug>` in <db> holds <p> proposal(s), <e> event(s): import-state restores only into an empty queue (a fresh data directory, or <db> moved aside); nothing changed``, no question.
4. stderr ``restore <p> proposal(s) and <e> event(s) of <slug> from <FILE> into <db>? [y/N]``, one line read, only `y` or `yes`; else exit 1, ``spec: not restored: the answer was not `y`; nothing changed``, stdout empty (`--json` `{db, proposals: 0, events: 0}`).
5. The data directory and DB made when absent (schema steps), then `restore`: one `Immediate` transaction, step 3 again under the write lock (filled meanwhile: step 3's refusal, nothing inserted), every row inserted as given, commit. No event of its own: the next ID and `seq` are the highest restored + 1.

**As stored**: no path rewritten. A proposal whose recorded common dir is gone is an orphan (`proposal-queue.md` "Place, IDs, repositories"), taken only by reject; same machine: move the repository back, `git worktree repair`.

## One door

Neither command writes a spec file, commits, runs apply or completion, or runs git in a worktree (export: only `rev-parse` and `worktree list` from the root; import: no git); an `approved` row stays `approved` until `spec approve` completes it. Export writes only its dump (and the default's directories), import only the two queue tables (and an absent data directory and DB), on a read-only project too; neither touches index rows nor replaces the DB file.

## Store

The store reads and inserts rows; the format is the CLI's.

| Op | Does |
|---|---|
| `SqliteQueue::open_existing(db, project)` | `None` without the file; creates nothing, runs no schema step; `user_version` above the build's → `SchemaTooNew`; no queue tables: reads empty |
| `stored_rows() -> StoredQueue` | every row of both tables, every project's, raw, one read transaction, by ID number and `seq`; a TEXT value that is no UTF-8 text (or a number, a BLOB) fails naming row and column |
| `counts() -> QueueCounts {proposals, events}` | both tables, every project's, one snapshot (`is_empty()`) |
| `restore(&StoredQueue) -> Restore {Restored, Occupied(QueueCounts)}` | schema steps, then one `Immediate` transaction: both tables empty or `Occupied`, plain `INSERT`s, commit; a row of another project than the handle's → `Invalid`, nothing written |

`StoredQueue {proposals: Vec<StoredProposal>, events: Vec<StoredEvent>}` (`counts()`); `StoredProposal {columns: [Option<String>; 24]}` (`id()`, `project()`); `StoredEvent {seq: i64, columns: [Option<String>; 4]}` (`project()`). `PROPOSAL_COLUMNS` (24), `EVENT_COLUMNS` (5): the columns in table order, pinned to `PRAGMA table_info` by a store test. `WorktreeGit::top_if_repository() -> Option<PathBuf>` (`None` only on git's not-a-repository message, else `Err`), `worktrees() -> Vec<ListedWorktree {path, bare}>` (path canonical where it exists, else as printed).

## Known limits

Accepted at shipping (2026-10-05); none blocks (ADR-0017).

- **Stale backup** (R1): a dump older than the queue loses what came after, whose IDs are issued again; one whose old `Proposal:` commit is on a branch blocks reject. A DB lost before its first export is lost; deleting the data directory deletes `backups/` (copy dumps out).
- One TEXT value that is no UTF-8 text (none in a `STRICT` table the queue wrote; a file written elsewhere may hold one) fails every export until fixed.
- Bounds: git seeing no repository from the root (`GIT_CEILING_DIRECTORIES` above it, an unreadable `.git`, a mount point between root and top) leaves the root as the only bound; another name for a directory inside a worktree (a Linux bind mount of a subdirectory) is not recognised (firmlinks are); the common dir's parent counts only when the common dir is named `.git`, so `--separate-git-dir` run from a linked worktree leaves the main checkout unprotected (git lists the git directory as the main worktree, named "the git directory …"); a submodule root's superproject is not a bound; git before 2.36 refuses every export in a repository; the data directory is judged against the root by path only.
- Placement: a crash between the link and the partial's removal leaves `<D>` whole beside its partial; without hard links, a file appearing between the check and the rename is replaced.
- Import: a FIFO swapped in between the check and the open still blocks; the file is read whole, no size cap; no `[ids]` `PR` clash check; a pseudo-terminal passes consent; SQLite may leave 0-byte `-wal`, `-shm` files when its open fails (pre-existing).
