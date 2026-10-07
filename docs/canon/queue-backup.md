---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-06
---

# Queue backup and restore

The queue (`proposals`, `tasks`, `runs`, `events` in `<slug>.db`, `proposal-queue.md` "Store") is the one state git cannot rebuild. `spec export state` dumps it to a JSONL file outside the repository; `spec import-state` restores a dump into an empty queue (ADR-0003; one write door: ADR-0004, ADR-0005; single user: ADR-0012; no re-targeting: ADR-0032). No automatic backup or retention (the daemon's), no `specengine.toml` key. Code: CLI `state.rs` (commands), `state_file.rs` (format), `main.rs` (terminal); store `queue/state.rs`, `WorktreeGit::{top_if_repository, worktrees}`.

## Commands

- `spec export state [--out PATH]`: the root slug's whole queue, every repository of the slug, unreadable rows included; no consent, no index refresh. Library `export_state(&Env, &Globals, &ExportStateRequest {out, now, git})` -> `ExportStateOutcome {path, <C>, messages}`, `<C>` = `proposals, tasks, runs, events`. stdout `wrote <D>: <counts>` (`tasks.md` "Backup"), `--json` `{path, <C>}`, never the dump. `<D>`: `--out` joined to the current directory as given (not canonicalised), else `<data dir>/backups/<slug>-<YYYYMMDDTHHMMSSZ>.jsonl` (UTC, the injected clock; a second default export within a second is refused as existing).
- `spec import-state FILE`: into the slug's empty queue, after one question on a terminal. Library `import_state(&Env, &Globals, &ImportStateRequest {file}, Consent)` -> `ImportStateOutcome {db, <C>, refusal, messages}`. stdout `restored <counts> into <db>`, `--json` `{db, <C>}`.
- Both read only the root's own `specengine.toml` (another `--config` -> exit 2) and check the data directory as the queue does (inside the root -> exit 2) without creating it. Text, errors and the prompt are escaped and one-lined as the queue's (`proposal-queue.md` "Terminal and git safety").
- Exit 0 done; 1 the import declined; 2 every refusal below, nothing written (export) or changed (import).

## Format

`STATE_FORMAT` = 2 (CLI; tasks: `tasks.md` "Backup"): UTF-8 compact JSON, one object per LF-ended line.

1. The header `{"format":2,"queue_schema":5,"project":"<slug>","proposals":<p>,"tasks":<t>,"runs":<r>,"events":<e>}`, keys in this order: `STATE_FORMAT`, the store's `QUEUE_SCHEMA_VERSION`, the root's slug, the row counts of the same snapshot.
2. Every `proposals` row by ID number (`ORDER BY length(id), id`), the `tasks`, `runs` rows, then every `events` row by `seq`, each `{"<table>":{…}}` with every column in table order (`PROPOSAL_COLUMNS`, 43; `EVENT_COLUMNS`): TEXT a string, NULL `null`, `run`, `seq` numbers; JSON columns stay the strings stored.

No export time, host or rowid: equal queues give byte-identical dumps, in any insertion order. Test oracle: `SqliteQueue::dump()`. A format-1 dump (five header keys, `queue_schema` 1-3: 24, 35, 40 proposal columns) or a format-2 one of schema 4 (41) restores, the later columns NULL; a schema 1-4 DB exports as format 2, schema 5, unmigrated.

```
{"format":2,"queue_schema":5,"project":"lantern-keep","proposals":1,"tasks":0,"runs":0,"events":1}
{"proposals":{"id":"PR-0001","project":"lantern-keep","kind":"update",…,"updated_at":"2026-10-05T12:00:00Z"}}
{"events":{"seq":1,"project":"lantern-keep","type":"proposal.created","payload":"{\"id\":\"PR-0001\"}","at":"2026-10-05T12:00:00Z"}}
```

## Export

In order, the first refusal exit 2; nothing is written before step 5.

1. **Bounds**, the directories a dump never goes inside: the root's worktree top (`rev-parse --show-toplevel`); the main worktree, the common dir's parent when the common dir is named `.git` and git's main entry is not bare; every entry of `git worktree list --porcelain -z` (git ≥ 2.36; main first, pruned ones kept; a bare main entry, or one that is the common dir (`--separate-git-dir`), is its git directory); the common dir. Git finding no repository from the root (exit 128, `fatal: not a git repository (or any ` under `LC_ALL=C`): the root alone. Any other git failure (dubious ownership, a broken `.git` file, the root inside a git directory, no `git`, an older git) -> ``cannot tell whether the destination <D> lies inside the repository of the project root <R>: <git error>; nothing written``.
2. `<D>` resolved (nearest existing ancestor canonical, the rest as written) is **inside** a bound when it starts with the bound's canonical path, or when one of its existing ancestors has the bound's device and inode (a macOS firmlink, a case-folded name) -> ``the destination <D> lies inside <the worktree X of the project root | the worktree X of the project root's repository | the git directory X of the project root's repository>: a dump holds proposal texts, paths and identities and never goes into the repository; nothing written``.
3. `<D>` exists (a file, a directory, a symlink, a dangling one too) -> ``the destination <D> exists: a dump never replaces a file; name a new one; nothing written``; `--out`'s parent no existing directory -> ``the destination's directory <P> does not exist: `--out` names a new file in an existing directory; nothing written``.
4. Both tables read raw in one read transaction (`stored_rows`, never `list_readable`); no DB file or no queue tables: an empty dump, `0, 0`, nothing created. A row of another `project` -> ``proposal `PR-…` in <db> is of the project `<p>`, not `<slug>`'s: the dump holds one project's queue; nothing written`` (`event <seq> …`; `of no project (NULL)`).
5. **Placement**: the default's missing directories made (`backups/` 0700, the data directory as `create_dir_all` makes it); `<D>.partial` created new (0600), written, `fsync`ed, then hard-linked to `<D>` (`link` never replaces a name: a `<D>` appearing meanwhile is step 3's refusal, its bytes untouched; a file system without hard links, exFAT or some network shares: a rename after the check); the partial removed, the directory `fsync`ed. Any failure removes the partial and the directories this run made (innermost first, only while empty; one that stays is named). A stale partial -> ``cannot create <D>.partial: … (left by an export that stopped: remove it); nothing written``, never removed. A partial left or a directory not synced after the dump is whole: exit 0 with a `warning:`.

## Import

In order; nothing is written before step 5.

1. `main`: stdin not a terminal -> exit 2 before `FILE` is opened, approve's message naming `spec import-state` (`proposal-apply.md` "Consent"). No `--yes`.
2. `FILE` (current-directory relative, symlinks followed) a regular file, checked before the open and on the opened handle, else ``cannot read <FILE>: not a regular file (a dump is one file)``, unread. Read whole and checked; the first defect -> ``<FILE>:<line>: <defect>``, never quoting the line or an unknown name: not UTF-8, not JSON, not a JSON object, an empty line or file, no final LF; no header, a header without exactly its format's keys (1: five; 2: seven) or with a value of the wrong type; `format` or `queue_schema` above the build's (`… upgrade SpecEngine`), `format` 0, `queue_schema` not 1-3 (format 1) or 4-5 (2) (``... (it holds queue schemas 4 to 5)``); `project` not the root's slug (both named); a row of another table (format 2 adds `tasks`, `runs`: `tasks.md` "Backup"); a column missing, extra or repeated (any repeated key); TEXT not a string or `null`; `seq` not an integer ≥ 1 (`12.0` too); `id` not `PR-` and 4 or more digits as the queue writes it, numbered 1 to 2⁶⁴−1; `project` not the header's; an `id` or `seq` repeating an earlier line's (named). Then rows read ≠ header counts -> ``<FILE>: header counts <p>, <e>; found <p'>, <e'>`` (format 2: four each). Rows in any order, inserted in file order; CRLF and whitespace inside a line accepted.
3. The data directory checked, not created; any table holding a row (any project) -> ``the queue of `<slug>` in <db> holds <counts>: import-state restores only into an empty queue (a fresh data directory, or <db> moved aside); nothing changed``, no question.
4. stderr ``restore <p> proposal(s), <t> task(s), <r> run(s) and <e> event(s) of <slug> from <FILE> into <db>? [y/N]``, one line read, only `y` or `yes`; else exit 1, ``spec: not restored: the answer was not `y`; nothing changed``, stdout empty (`--json` counts 0).
5. The data directory and DB made when absent (schema steps), then `restore`: one `Immediate` transaction, step 3 again under the write lock (filled meanwhile: step 3's refusal, nothing inserted), every row inserted as given, commit. No event of its own: the next ID and `seq` are the highest restored + 1.

**As stored**: no path rewritten. A proposal whose recorded common dir is gone is an orphan (`proposal-queue.md` "Place, IDs, repositories"), taken only by reject; same machine: move the repository back, `git worktree repair`.

## One door

Neither command writes a spec file, commits, runs apply or completion, or runs git in a worktree (export: only `rev-parse` and `worktree list` from the root; import: no git); an `approved` row stays `approved` until `spec approve` completes it. Export writes only its dump (and the default's directories), import only the queue tables (and an absent data directory and DB), on a read-only project too; neither touches index rows nor replaces the DB file.

## Store

| Op | Does |
|---|---|
| `SqliteQueue::open_existing(db, project)` | `None` without the file; creates nothing, runs no schema step; `user_version` above the build's -> `SchemaTooNew`; no queue tables: reads empty |
| `stored_rows() -> StoredQueue` | every row of the four tables, every project's, raw, one read transaction, in dump order (a schema 1-4 DB: the later columns `None`, 1-3 no tasks; tables at another `user_version` -> `Invalid`); a TEXT value that is no UTF-8 text (or a number, a BLOB) fails naming row and column |
| `counts() -> QueueCounts {<C>}` | every table, every project's, one snapshot (`is_empty()`) |
| `restore(&StoredQueue) -> Restore {Restored, Occupied(QueueCounts)}` | schema steps, then one `Immediate` transaction: all empty or `Occupied`, plain `INSERT`s, commit; a row of another project than the handle's -> `Invalid`, nothing written |

`StoredQueue {proposals, tasks, runs, events}` (`counts()`); `StoredProposal {columns: [Option<String>; 43]}` (`id()`, `project()`), `StoredTask`, `StoredRun` (`tasks.md`); `StoredEvent {seq: i64, columns: [Option<String>; 4]}` (`project()`). `PROPOSAL_COLUMNS` (43; `proposal_columns(schema)`: schema 1's first 24, 2's 35, 3's 40, 4's 41), `TASK_COLUMNS` (17), `RUN_COLUMNS` (11), `EVENT_COLUMNS` (5): the columns in table order, pinned to `PRAGMA table_info` by a store test. `WorktreeGit::top_if_repository() -> Option<PathBuf>` (`None` only on git's not-a-repository message, else `Err`), `worktrees() -> Vec<ListedWorktree {path, bare}>` (path canonical where it exists, else as printed).

## Known limits

Accepted at shipping (2026-10-05); none blocks (ADR-0017).

- **Stale backup** (R1): a dump older than the queue loses what came after, whose IDs are issued again; one whose old `Proposal:` commit is on a branch blocks reject. A DB lost before its first export is lost; deleting the data directory deletes `backups/` (copy dumps out).
- One TEXT value that is no UTF-8 text (written elsewhere) fails every export until fixed.
- Bounds: git seeing no repository from the root leaves the root as the only bound; another name for a directory inside a worktree (a bind mount) is not recognised (firmlinks are); the common dir's parent counts only when the common dir is named `.git`, so `--separate-git-dir` run from a linked worktree leaves the main checkout unprotected; a submodule root's superproject is not a bound; the data directory is judged against the root by path only.
- Placement: a crash between the link and the partial's removal leaves `<D>` whole beside its partial; without hard links, a file appearing between the check and the rename is replaced.
- Import: a FIFO swapped in between the check and the open still blocks; the file is read whole, no size cap; no `[ids]` `PR` clash check; SQLite may leave 0-byte `-wal`, `-shm` files when its open fails (pre-existing).
