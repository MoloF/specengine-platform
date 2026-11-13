---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp]
owner: owner
reviewed: 2026-11-12
---

# Read freshness: the stat key, and a hub's trusted index

ADR-0069, built by `read-trust` (`docs/features/read-trust.md`, RM-0054, step 12e). How a read's first step chooses what to read, and when a hub's read skips it. Files stay the truth, the index a rebuildable projection (ADR-0001, ADR-0003); nothing here blocks (ADR-0012). The subset read, T3 (one snapshot per read), T5 (every printed byte matches its row's BLAKE3): `spec-read-subset.md`. Code: store `write.rs`; CLI `freshness.rs`; http `answer.rs`; mcp `hub.rs`.

## The freshness step

`freshness::step`, every corpus read's first step (CLI `corpus.rs`, `search.rs`, `subset.rs`), either:
- **walks**: `refresh_in` -> the store's `update` through the read's one `WorkingTree`: list, `lstat` each listed file, read only those whose key is not trusted; warnings print once;
- or **trusts**, on a hub alone when its trust is not due: the last walk's notes and both fingerprints in one deferred transaction (`trusted_notes`), that walk's warnings printed (`refresh::walk_warnings`, shared with the walk: same lines, same order), nothing updated; `None` or an error walks.

The CLI, stdio MCP, a local daemon and eval's walking reads always walk: an editor writes a laptop's tree. `spec index` (`--full`: `rebuild`) and apply's step 4 walk in any scope (R14): a commit is judged against the files. Outputs and exits are unchanged; no key, flag or variable sets a mode, the window or the interval.

## The stat key

`Source::stat(path) -> Option<StatKey {size, mtime_ns, ctime_ns, ino, dev}>`, one `lstat` before the read: ns from the Unix epoch (`i64`), inode and device compared for equality only. `WorkingTree` on `cfg(unix)`: `None` for an unclean path, an error, a symlink, a non-regular file; `None` on other targets and for `GitIndex` (the default); wrappers forward it (CLI `subset::Listed`). An update reads the clock once, `taken_ns`, before its first `stat`.
- **R1.** `update` does not read a file whose row has a BLAKE3 and a stored key equal to its key now, taken `RACY_NS` (2 s) or more after that key's `max(mtime, ctime)`, unless it re-parses all: `unchanged`.
- **R2.** Any other file is read and hashed; `update_paths` and `rebuild` read every file they name or list (they follow a known change or a T5 mismatch, evidence a key lied).
- **R3.** An unchanged hash keeps the rows (`parsed` 0); the key is stored when it differs, none is stored, or it ends the stored key's raciness, never rewritten while racy. A changed hash re-parses.
- **R4.** No key (a failed `stat`, a source with none): read, no `file_stats` row.
- **R7.** `spec index --full` reads every file and stores every key: it repairs a lying key.

`UpdateReport.hashed` counts the files read. Printed files are still BLAKE3-checked (T5, Q7): not the size heuristic `spec-cli-graph` dropped. A wrong "moved" costs one read, a wrong "unchanged" misses an edit. Read once: a new inode (`cp -p`, `ditto`, an APFS clone, an atomic save, Docker's copy-up), the files `git checkout` or `restore` wrote, `touch` (`parsed` 0), a same-size edit with mtime restored (`touch -r`, `rsync --inplace`: ctime moves), a remount's device or renumbered inodes (FUSE, SMB). A write within one coarse tick (HFS+ 1 s, FAT 2 s) is racy; so is a clock set back or a file dated ahead, read every update. A clock set forward misses a same-tick write on a coarse file system.

The code index keys on content, never a stat (`symbol-index.md` "Cache key and cross-file pass").

## No-op updates

- **R5.** `update` and `update_paths` return before their `Immediate` transaction, `UpdateReport.committed` false, when the stamp is current, the worktree row exists, no fingerprint moved, it is no rebuild, every plan is unchanged with no key to store, no stored path is gone and a listing's notes equal the stored ones. Warm reads never take turns on the write lock, nor compete with a hub's queue writes.
- **R6.** A hashless row that fails again with the same `read_error` and `size` stands, counted `unreadable`.

`update_paths` also walks, beside the stamp and fingerprint cases (`spec-index-edges.md` "`[paths]` fingerprint and probes"), when a root may start or stop being missing (`roots_may_move`: a listed path under a root with no stored file, or every stored file under one unlisted): only a walk's listing records it.

## Walk notes and the format

`INDEX_FORMAT` 17 adds two `STRICT` tables (`FORMAT_TABLES`, children first): `file_stats (file_id -> files ON DELETE CASCADE, size, mtime_ns, ctime_ns, ino, dev, taken_ns)`, `ino` and `dev` as `i64` bits; `walk_notes (wt -> worktrees ON DELETE CASCADE, missing_roots, unreadable_dirs, skipped_names)`, the last listing, JSON arrays (`missing_roots` in listing order, `unreadable_dirs` sorted, `""` the root), written by the update's own transaction when changed. They hold what the file system said, not what the bytes are, so the canonical dump (`dump.rs`) leaves both out: two copies of a tree dump equal, and the incremental index still equals a rebuild. The dump differs from 16's only in `index_meta`; a format-16 database is recreated by its first write.

## A hub's trusted index

`--public-origin` is the switch (`App::new`): a hub's clone is written only by its confirms, merges and the operator's hand.
- **`IndexTrust`**, one per served project for the process's life (`Project.trust`; `None` on a local daemon): a canonical root, `{walked, dirty, since, walks}`. **R8.** A read walks when its trust has not walked since start, is dirty, or began its last successful walk `RECONCILE_EVERY` (300 s) or more ago; else it trusts. The reconcile is lazy, the first read due running it in its turn: no timer, no watcher.
- **Scopes**: a thread-local, restored on return or unwind; a call runs whole on one blocking-pool thread. `trusted(&[&IndexTrust], run)`: reads may trust. `walking(..)`: every step walks, doubts and walks still reaching the trusts. `trusting(run)`: the current scope's trusts, reads trusting. Only a trust whose root is the project's (canonical) is used.
- **R9.** A walk under a matching trust clears the mark and counts itself as it begins; success sets `walked` and `since` (its start), failure or a panic (`Walk`'s drop) the mark, so a doubt raised meanwhile survives.
- **Doubt (T4)**, the dirty mark: `IndexTrust::doubt`, CLI `freshness::doubt(root)`, `doubt_in_scope()` (every trust in scope); outside a scope, nothing. Raised by a failed post-commit `update_paths` (`post_commit_update`: apply, completion, create, decide; R10); a merge's failed diff, discovery, open or update, skipped non-UTF-8 names, a root with no path from the top (R11); a failed fast-forward, every served root (R12); a trusted read's T5 or Q7 mismatch, `covers` failure or attempt store error, once per read (R13); a refused confirm (http `confirm.rs`: it may have written); a call that panicked or could not run (`answer.rs`, `HubSeam::turn`, MCP's `call` and resources).
- **Writers keep the index in the turn.** A confirm's commit updates its paths. A sync's fast-forward, every served project's turn held, in their `walking` scope, runs `reindex`: `SyncGit::changed(L, target) -> ChangedPaths {paths, skipped_names}` (`diff --name-only -z --no-renames`, under the deadline); per served root, the paths under its path from the top, prefix stripped, through `update_paths` on a `WorkingTree` of the merged `[paths]` and scheme, the project discovered again (a moved `[paths]` or `[ids]` walks). `Contained`, `Same`: no update.
- **R15.** A trust never refuses a read; no status, check or gate consults `IndexTrust` (the `check` and `health` routes are display reads that trust). **R16.** The CLI, stdio MCP, a local daemon and eval's walking reads enter no scope.

## Routes and tools

**Only reads trust.** A route or tool that writes or authorizes a write walks, so nothing is judged from an index that missed a hand edit; so does one not classified, and trace data wherever shown (the rollups and marks a card is accepted on).
- **http** (`answer.rs`). `run_read` trusts: `tree`, `nodes`, `search`, `bundle`, `graph`, `map`, `flow` without `trace`, `check`, `health`, `lock-diff`, `roadmap`, `delivery`, `inbox`, `proposals/<id>`, `tasks`, `tasks/<id>`, `cards` (GET) without `marks`, `cards/<id>` (its `upstream` mark line may lag a hand edit on the host up to 300 s, as the walking `cards?marks=true` row flags: `card-upstream.md`); `run_as` lets `flow`, `cards` decide by the request. `run` walks the rest: `trace`, `catalog`, `rings`, the round's sheet and answers, the stage, card writes, the trace's clear, a confirm's read and write.
- **MCP** (`hub.rs`). A call runs in the turn's `walking` scope; `reads_trust(name, arguments)` opts into `trusting` for `HUB_READ_TOOLS` (7): `get_tree`, `get_node`, `search`, `get_context_bundle`, `get_proposal`, `list_cards` without `marks`, `get_roadmap`; resource lists and reads trust. `get_flow`, `get_trace`, `get_card`, queue and card writes, probes walk: `get_flow` even without `trace`: its stdio text (2 045 of 2 048 B) has no room for a hub's tail.
- **Texts**: `HUB_INSTRUCTIONS` and five trusting reads' tails say what a hub's read trusts (`hub-mcp.md` "Tools"); stdio's keep "refresh".

## Invariants and the window

- **T1.** A trusted read equals the walking read, text, JSON and stderr, byte for byte, whenever a walk would change nothing. **T2.** It prints the last walk's warnings from `walk_notes`. **T4.** Any doubt means a walk: one mark per project, per process. T3, T5 hold in both modes.
- **The window.** A subset read cannot see an edit made after its step to a file outside its subset (`spec-read-subset.md` "Known limits"); locally as short as the stat walk. On a hub every SpecEngine write is indexed before its turn ends, so only hand edits on the host remain: one in a printed file shows at once (T5), any other after a reconcile (≤ 300 s) or `docker compose exec specengine spec index --root /projects/<name>` (`scripts/deploy/README.md`). Writes and trace data see the disk.

## API

Store `StatKey`, `Source::stat`, `WalkNotes`, `SqliteIndex::trusted_notes(&IdScheme, &Paths) -> Option<WalkNotes>` (never writes; `Some` when the stamp is current, the worktree row's fingerprints match and its notes read), `ChangedPaths`; `IndexWriter` unchanged. CLI `IndexTrust` (`#[doc(hidden)]` `with_every`, `walks`, `is_dirty`), `RECONCILE_EVERY`, `trusted`, `walking`, `trusting`, `doubt_in_scope`. http crate-private `answer::{run_read, run_as}`. mcp `HUB_READ_TOOLS`, `HUB_PROBES` (`probe_panic`, `probes` builds only).

## Known limits

- **A driver whose ctime does not move on a write** (some FUSE, FAT-family), with a same-size edit and mtime restored: missed until the key moves or `--full`.
- **A network client's attribute cache** (NFS 3-60 s by default; SMB) lags another machine's write by its life; this machine's own show at once. No network file system is detected (no `statfs` in `std`, no libc).
- **A stale writer**, local and multi-process only: a row written from a parse older than another process's commit stands until an update reads that file, since a no-op update no longer re-checks it in an `Immediate` transaction. T5, Q7 still check printed files; a hub's writes take the turn.
- **`roots_may_move` misses a missing root created holding only non-`.md` files:** trusted reads warn it missing until the next walk.
- **Doubt does not cross processes:** a terminal's `spec approve` or `spec index` on a hub's host writes rows the daemon reads as written; a failure of theirs waits for the reconcile.
- **`HubSeam::turn` sees only a panic or a slug that cannot run;** an exit 2 inside an MCP call is doubted by the mcp crate (`doubt_in_scope`).
- **Fan-out** per update: `spec-index-edges.md` "Known limits".

## Tests

Store `tests/stat_key.rs` (raw connections: the one exception to `tests/api.rs`' `rusqlite` guard); CLI `tests/read_trust.rs`; http `tests/hub_git.rs`, `tests/hub_trust.rs`; mcp `tests/mcp_hub_trust.rs`; eval `tests/read_trust_scale.rs` (`#[ignore]`); `src/read_trust_tests.rs` in CLI and http. Criteria: the spec.
