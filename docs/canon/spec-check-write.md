---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp]
owner: owner
reviewed: 2026-11-20
---

# spec check inside a write: the walked rows, the affected set, the memos

A write's introduced findings are those of a propose (`propose.rs`, `create.rs`, `amend.rs`, `interpret.rs`, `intake.rs`), of a deciding kind's approve (decide step 6), and of a hub confirm's read and write. They come from store `check_rows::introduced_from_rows`, equal to 0.47.0's whole disk check `introduced_findings` finding for finding and in its order. Nothing refuses (ADR-0012): a parser panic gives no findings. Built by `write-cost` (RM-0125, 0.47.1).

<a id="trusted-rows"></a>
## The walked rows (ADR-0083)

After the call's walk, the check takes each file as its row has it, trusting the stat key as the freshness step does (`spec-read-freshness.md` "The freshness step", ADR-0069). Steps (`check_rows.rs`):
1. **One snapshot.** One deferred read transaction (`IndexSnapshot`) gives the walk-state digest and every row read.
2. **The memo** in scope ("Memos"): D's findings when held; C's as-is findings of the set's paths it holds.
3. **The cold path**, 0.47.0's whole disk check:
   - no index handle, or a store error;
   - rows of another `scheme_fp` or `paths_fp` than the setup's;
   - in a daemon or stdio server, the first call per root and after a config change.

   A row recording a parser panic (`PARSER_PANICKED`) on a file that reads now takes the cold path, nothing doubted.
4. **The set** ("The affected set"), decoded until no file joins.
5. **The region**, when a cycle pass reruns.
6. **The hash net.** Every set file, and every index output a render compares, is read from disk and its BLAKE3 compared with its row's. A mismatch runs the whole disk check, empties D, and the caller doubts the trust (T4).
7. **The cycle line.** An introduced cycle whose first member was not read is read through the same net and the patched side judged again, so its line is that file's.

From disk the check reads only the target, `specengine.toml` and the baseline, a deciding kind's template and ID floor, and step 6's files. Apply still re-reads the file it writes, so written bytes never come from a row. Reads keep ADR-0069.

**The hole** (ADR-0083's cost; `spec-read-freshness.md` "Known limits"): a file outside the set changes while its stat key equals a non-racy stored key. That happens with a same-size edit with mtime restored where ctime does not move (some FUSE, FAT), a clock set forward, an NFS or SMB attribute cache, or a stale local writer. On a hub (ext4 or overlayfs, ns timestamps) only a clock jump or a host terminal remains. A racy key (taken within 2 s, `RACY_NS`, of its own max(mtime, ctime)) is re-read on every walk. `spec index --full` repairs. A D hit returns before the hash net: D's key holds the walked digest, so this is the same hole.

## The affected set

Core `check::affected` (`Dependents`, `Affected`, `AsIs`), stated in model terms only (ADR-0008): keys a file gives or reads, never a directory, prefix, kind or class.
- **N**: the keys P, the changed or created file, gives before and after (`given_keys`): its path, its `id:`, `{#ID}` sections and `aliases:` entries.
- **The set**: P; every reader of a key in N (a file whose recorded resolution looked it up, hit or miss: a reference dangling today, a name-fallback candidate, a `slug/ID`'s feature path, a link candidate: `spec-index-edges.md` "Keys"); every holder of a key in N (`id-taken`). For a reader whose `status:` is `superseded-by`, the set also takes every reader of its own keys: the superseded-date exemption's second hop. Then it takes the files every set file's lookups read, on both sides, until no holder joins. A `summarises:` entry's file is a read of its page (core `summarised_reads`), so a write to a summarised page judges the summarising one again (`spec-guide.md`). A created P sorts in path order.
- **The global passes** (`Globals`):
  - `depends-cycle` reruns when a set file has a `depends_on` list on either side;
  - `parent-cycle` reruns when one has a `parent:`;
  - the index outputs rerun when an index entry is registered and P's rendered projection (fields, ID, title, front-matter state, given keys) or its path's role as an output changes. That render decodes every row.

  Otherwise each gives the same findings on both sides, so none is introduced.
- **The region.** From the set, every file reached forward over `depends_on` targets (`depends_targets`, `holders_of`) and `parent:` targets, on either side, each closed for its resolution by its keys' holders. Being forward closed, it holds each of its files' whole SCC or parent chain. A cycle finding landing outside it is dropped on both sides: such a component reads none of P's keys, is equal on both sides, and cancels in `judge`'s (code, path, subject) triple.

Both runs, as-is and patched, cover only the set (`engine::run_over`, `PatchCheck::introduced_over(&mut AsIs)`, `introduced_in`), each resolving over the whole snapshot. A file outside the set gives equal findings in both runs, so it is never introduced. The result is judged as `PatchCheck::introduced` judges and in its order; that function stays the oracle. Readers and holders come from the snapshot's `edge_deps` and `edge_names` (`spec-index-edges.md` "Passes", R4).

**Decode-free answers.**
- The ID preview is `SqliteIndex::highest_record_number`, two indexed range queries per prefix (`decision-record.md` "ID").
- An update's holder is `preflight::target_closure`, a key-closure decode.
- The whole decode stays only for review, interpretation and create (`StepRead.want_corpus`).

<a id="memos"></a>
## Memos

Store `check_memo`, in memory, per process, never stored.

**Keys.** These are computed, never stored, so there is no `INDEX_FORMAT` step.
- **C** is keyed by the root and the walk-state digest: the index stamp, the worktree's `scheme_fp` and `paths_fp`, every `files` row (path, blake3 or read error, size) in path order, the `walk_notes` row, the config's and the baseline's BLAKE3, and today.
- **D** is keyed by that digest, the root, the written path, the BLAKE3 of the new bytes, and the login (`-` off a hub). A confirm re-keys D under its `[team]` login (`check_memo::as_login`).

**What each holds.**
- **C** holds per-path as-is findings and the global passes' findings for one digest, and is replaced when the digest changes. The region's as-is cycles are never kept in C. C holds the corpus's own findings, no login's data.
- **D** holds one write's introduced `ProposalFinding` list.

No findings cross logins. The POST takes D's findings when its key equals its GET's. It still renders, asks and compares its own question byte for byte (`hub-confirm.md` "Rules"). A memo never holds a question, a record or an ID.

**Home.**
- A daemon has `Project.memo`, one per served project, a turned memo. Page routes, `/mcp` (after the login's line) and the delegated apply run their call inside the turn hold's `within`, and only there do `in_turn()` and `in_scope()` (thread-local) hold. A scope entered outside the turn panics in debug builds.
- The `--push` sync's merge holds the turn but enters no memo scope, and its reindex moves the digest.
- The stdio MCP server keeps one process-wide memo, with the root in every key.
- The terminal keeps none.
- The lock is a leaf: it is held for one lookup or insert, never while computing or waiting on a turn, SQLite or git.

**Budget.** `BUDGET` is 32 MiB of bytes for C and D together. D is evicted first, least recently used, then C. An entry over the budget is computed but not stored. A miss is slower, never wrong.

**Doubt** (T4) empties D. C is used again only under the digest the call's own walk gives. `DoubtOnPanic` runs after the scope is restored, so it leaves D alone, which is harmless because D is keyed by the digest.

**Probes.** `hits()`, `misses()`, `lookups_by_login()`; core `check::counts()` → `RunCounts` (engine runs, parses).

## Cost fields

Store `cost` (`scoped`, `charge`, `spawn`) counts per call on the calling thread. The innermost kind wins, so kinds never overlap and their sum stays within the call's time. Outside a scope every charge is a no-op. A kind that nothing charged is `-`.
- `check_ms` is the introduced findings' computation.
- `index_ms` covers walks, decodes, `update_paths` and the outputs.
- `git_ms` and `git` are git's processes, spawn to exit, a `cat-file --batch` session included.
- `write_ms` is the queue's write transactions (`WriteTx`), the record's and the floor's synced writes.

The line format: `serve-slow.md` "Lines".

## Git on the write path

- **G1.** `--local-env-vars` runs once per process per `PATH`, at most 8 values cached, and a failure is never cached.
- **G2.** `operation_in_progress` makes one `rev-parse` with seven `--git-path` arguments.
- **G3.** `place_step` uses `top_and_common_dir`.
- **G4.** The confirm opens its queue context once.
- **G5.** The trailer lookup and `place_step` share one handle.
- **One more.** The confirm's context reads the top and the common dir in one `rev-parse`, and `open_place` reuses them when the recorded worktree is that top and the git variables are equal.

A question's GET and POST now spawn 11 and 25 git processes, against 25 and 45 in 0.47.0; an update's spawn 15 and 25, against 29 and 46. Messages, exits and refusals are unchanged. In-process git reads were deferred, pending the owner's review (RM-0126).

## Known limits

- **The POST's write side**, about 1.1 s on the hub, is untouched: synced queue transactions, two git index writes, the post-commit update. Changing it means durability decisions, a later slice measured by `write_ms`.
- **The index render** decodes every row when P's rendered projection changes: about +107 ms from 1 000 to 4 000 nodes, against the asserted GET path's +31 ms.
- **A root file with many readers** costs O(affected): about +210 ms at the same sizes.
- **The trailer lookup** grows with history (22 ms per 400 commits on the hub).
- **The outputs' `HEAD` parse** under `index = true` is unchanged.

## Tests

- core `tests/introduced_oracle.rs`, `tests/common/write_cost.rs` (the catalogue, the cycle cases, the generator);
- store `tests/introduced_rows.rs`, `check_rows`' and `check_memo`'s unit tests, `tests/worktree_git.rs` (G1);
- cli `tests/write_cost_memo.rs`, `tests/write_cost_golden.rs`;
- http `tests/write_cost_golden.rs`, `tests/hub_concurrency.rs` (ten logins), `tests/write_cost_git.rs`, `tests/hub_confirm.rs` `wc5_*`, `tests/slow.rs`;
- mcp `tests/write_cost_golden.rs`;
- eval `tests/write_cost_scale.rs`, and `specengine-eval write-cost`, which uses no corpus but a scratch copy and prints counts and times only.

The goldens are `fixtures/write-cost-golden/`. The criteria are in `docs/features/write-cost.md`.
