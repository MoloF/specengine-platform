---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, crates/specengine-eval, scripts/deploy]
ref: RM-0054; Phase 7 step 12e, the last of step 12 `scale-reads` (08 s2 "Step 12 in five"); Q1-Q6 of 2026-11-03 and D1-D16 below are the orchestrator's working answers, each awaiting the owner's review; the draft ADR `read-freshness` awaits the owner
---

# read-trust: a freshness step keyed by stat, and a hub's reads on the index its writers keep (RM-0054)

## Why

Since `store-reads` (12d) a read costs its freshness step. At 100 000 nodes `get_node`, `--links`, `search` and `get_context_bundle` take 372-380 ms, and the step alone 376 ms: 349 of it reads and hashes all 20 015 files to learn that nothing changed, and then it takes the index's write lock to commit nothing (`docs/canon/scale-eval.md` "Baseline"). A hub pays the same, though only its own confirms and merges write its tree. This slice:
- keys the freshness step on each file's stat, so a warm step lists, `lstat`s, reads no file and commits nothing;
- lets a hub's reads skip the step and answer from the index its writers keep inside the turn, walking when in doubt;
- sets the bars at 10 000 and 100 000 nodes, and closes step 12.

T3 (one index snapshot per read) and T5 (every printed byte matches its row's BLAKE3) hold since 12d (`docs/canon/spec-read-subset.md`). Added here:
- **T1.** A trusted read equals the walking read, text, JSON and stderr, byte for byte, whenever a walk would change nothing.
- **T2.** A trusted read prints the last walk's warnings, replayed from the index.
- **T4.** Any doubt means a walk: a dirty mark per project, per process.

Decided here, each the orchestrator's working answer awaiting the owner:
- **D1. One slice, not 12e-1 (the stat key) and 12e-2 (hub trust).** The halves share one schema bump (`INDEX_FORMAT` 17 carries both tables of D3), one decision (Q6: one ADR) and one mechanism: a hub's walk is the local stat walk, and T2 replays the notes that walk stores. Split, 12e-1 would ship a table nothing reads (as 12c did) and either an ADR a slice ahead of its second half or two ADRs. The size is at the precedents' upper end: 25 named mutations against 12c's 23 and 12d's 22, five crates against 12d's four. The work order puts the store half first, so a review can accept AC-8, AC-9 and AC-14 apart from the hub's criteria.
- **D2. The stat key (Q4):** size, mtime and ctime in nanoseconds, inode and device, from one `lstat` taken before the read. `update` skips a file whose key is unchanged and was taken 2 s or more after the file's last change (the racy window, no opt-out). `update_paths` reads every path it is named: it runs after a known change or a T5 mismatch, which is evidence a key lied. `spec index --full` reads every file, so it repairs a lying key. Rules R1-R7. This is not the size heuristic `spec-cli-graph` dropped (71 of 200 same-size races mixed): a printed file is still compared by BLAKE3 (T5, Q7), and the key moves on every write except one within the same timestamp tick, which the window reads.
- **D3. Two new tables outside the dump, not columns; `INDEX_FORMAT` 17.** `file_stats` holds a file's key, `walk_notes` a worktree's last listing: what the file system said, not what the bytes are. So the dump leaves both out: two copies of one tree dump equal (AC-8e), the dump stays every column of every dumped table, and the store's hard property (the incremental index equals a rebuild, row for row) keeps its meaning. A format-16 database is recreated by its first write, one full parse per worktree, as at 16.
- **D4. A no-op update takes no write lock.** Today `update` always opens its `Immediate` transaction (store `write.rs:211`), so two warm reads take turns on the write lock, and on a hub a read competes with the queue's writes in the same database. An update whose plan changes nothing returns before it (R5); an unreadable file that fails again the same way keeps its rows (R6).
- **D5. The code index stays on content (Q5):** `refresh_code` keys on BLAKE3, never on a stat (`docs/canon/symbol-index.md` "Key").
- **D6. Trust is a hub's alone.** `--public-origin` is the switch, with no flag, key or variable of its own, as for a hub's MCP mounts and stream life (`crates/specengine-http/src/app.rs:113-118`). On a laptop an editor writes the tree outside SpecEngine every second. A hub's tree is its own clone, written only by its confirms and merges and by the operator's hand. The CLI, the stdio MCP server, a local daemon and eval's walking reads keep the freshness step, now stat-keyed.
- **D7. The mode is a per-call scope,** `freshness::trusted(&[&IndexTrust], run)`: a thread-local for the call's thread, restored on return or unwind. The daemon enters it at its three turn sites (`answer::run`, the hub's `HubSeam::turn`, the sync's merge), so the hub's MCP calls take it through `turn` and the mcp crate gains no parameter. No read's signature changes: a field on `Env` or `Globals` would edit 125 and 57 struct literals, most of them other roles' tests. A call runs whole on one blocking-pool thread, and no read hands the index to another thread.
- **D8. The trust state, and how it meets `App` and the turn:** one `IndexTrust` per served project for the process's life, held by `Project`, set by `App::new` on a hub, changed only inside the project's turn but for `doubt`, which only sets the mark. A read walks when the trust has not walked since start, is dirty, or began its last walk 300 s ago or more (`RECONCILE_EVERY`); else it trusts (R8, R9). The reconcile is lazy: the first read due runs it in its own turn, with no timer task and no file watcher. A walk clears the mark as it begins and sets it again if it fails, so a doubt raised during a walk survives it.
- **D9. Doubt (T4)** comes from a failed post-commit update, a merge's failed fast-forward, diff or root update, a trusted read's T5 or Q7 mismatch, `covers` failure or attempt store error, and a call that panicked or could not run (R10-R13). Outside a scope it does nothing.
- **D10. Writers keep the index inside the turn.** A confirm already updates its paths after the commit (`crates/specengine-cli/src/apply.rs:1139`). The merge gains its update, over the paths `git diff` names between the old `HEAD` and the new, per served root, with the merged config, so a `[paths]` or `[ids]` change walks (R11; `docs/canon/spec-index-edges.md` "`[paths]` fingerprint and probes"). Apply's step 4 and `spec index` always walk: a commit is judged against the files (R14).
- **D11. What T2's replay stores and where.** `walk_notes` holds the last listing's three facts, not its messages: `missing_roots`, `unreadable_dirs` and `skipped_names`. The update's own transaction writes them when they changed. One function renders them for both paths, so a trusted read and a walk print the same lines in the same order. The messages still depend on the config, which is read per call (`paths_written`).
- **D12. The hub's MCP texts** that promise a refresh now say what a hub's read trusts ("Data"). Stdio's texts keep "refresh".
- **D13. The bars (Q1, Q2),** from `specengine-eval scale`, release, seed 1, best of three:
  - trusted warm `get_node` (with and without links), `search` and `get_context_bundle` take under 1 s at 100 000 nodes and grow under 2x from 10 000, with a median under 10 ms counted as 10 ms;
  - locally the warm update reads no file; its time is reported, with no bar;
  - W_after's median at 10 000 stays within 10 % of 1 000.

  The floor exists because a trusted read costs about a millisecond. `search`'s FTS step alone went from 1.02 to 6.02 ms between 10k and 100k, a 6x that matters to no one. The hub's most linked node is reported with no bar.
- **D14. Fan-out.** RM-0078 estimated it at 0.25-0.4 s per update at 100 000 nodes. It is measured first, in AC-14, on the 100 000 tree plus one file that cites every document by path, like a generated index. It gets no bar: a project's config creates it, and every confirm on a hub commits its generated index. Its number replaces the estimate in `spec-index-edges.md` "Known limits". If it is above 1 s, the plan gets an item.
- **D15. `PARENTED`** (store `edges.rs:95`): the comment is fixed, and the lookup is kept with its index `edge_parents_by_end`. No read uses it now, but a subset `tree` or `flow` would; once dropped, it would take another format bump to bring back.
- **D16. One ADR, `read-freshness` (Q6),** issued at ship as a working answer ("Draft ADR"). Its canon is a new Tier 2 page, `docs/canon/spec-read-freshness.md`. `docs/canon/architecture.md` is unchanged: files stay the truth and the index a rebuildable projection (ADR-0001, ADR-0003), and what a read trusts is a rule of the index, kept on that page.

## Description and interactions

**A local read** (the CLI, stdio MCP, a local daemon). The freshness step lists the tree as before and `lstat`s each listed file. A file whose key is trusted keeps its row unread. Any other file is read and hashed, and re-parsed only if its hash moved. When nothing moved, no write transaction opens. The read then runs as in 12d, and the step's warnings print as before.

**A hub's read.** The daemon holds an `IndexTrust` per project. Inside the project's turn, the read's freshness step does one of two things:
- it walks: the first read after start, after a doubt, or 5 minutes after the last walk;
- or it trusts: it reads the last listing's notes and both fingerprints from the index in one deferred transaction, prints the notes' warnings, and goes on with no update.

Show, links and bundle still check every subset file against its row (T5), and whole reads with bytes still re-parse any file whose bytes do not match (Q7). A mismatch also marks the project dirty. A confirm's commit updates its paths, a merge updates the paths it moved, and a failure marks the project dirty.

**The window.** `spec-read-subset.md` "Known limits" says a subset read cannot see an edit made after its freshness step to a file outside its subset. Locally the window is unchanged, and now as short as the stat walk. On a hub, every SpecEngine write is in the index before its turn ends, so the window holds only hand edits on the host. One in a file a read prints shows at once (T5); any other shows after a reconcile (at most 5 minutes of reads) or the operator's `spec index`.

**The operator.** The guide (`scripts/deploy/README.md`) says that after changing a project's files on the host by hand (a `git pull`, an edit, a restore after a stopped fast-forward), they run `docker compose exec specengine spec index --root /projects/<name>`. Otherwise the hub sees the change within 5 minutes of reads.

**Unchanged:**
- every command's output, exit code and JSON;
- `spec index`'s report: the new counts are `#[serde(skip)]`;
- the code index, and the walk rules (store README "Walk");
- 12d's subset, T3, T5 and the walk fallback;
- a local daemon, and the hub's routes, tools, schemas and fence;
- `specengine.toml`.

## Data

### Tables

Store `schema.rs`, after `files` and `worktrees`:

```sql
CREATE TABLE file_stats (
  file_id INTEGER PRIMARY KEY REFERENCES files ON DELETE CASCADE,
  size INTEGER NOT NULL,       -- st_size
  mtime_ns INTEGER NOT NULL,   -- st_mtime * 10^9 + st_mtime_nsec
  ctime_ns INTEGER NOT NULL,   -- st_ctime * 10^9 + st_ctime_nsec
  ino INTEGER NOT NULL,        -- st_ino, its u64 bits as i64
  dev INTEGER NOT NULL,        -- st_dev, its u64 bits as i64
  taken_ns INTEGER NOT NULL    -- the update's clock, read once before its first lstat
) STRICT;
CREATE TABLE walk_notes (
  wt INTEGER PRIMARY KEY REFERENCES worktrees ON DELETE CASCADE,
  missing_roots TEXT NOT NULL,    -- JSON array of strings, in the listing's order
  unreadable_dirs TEXT NOT NULL,  -- JSON array of strings, sorted ("" is the root)
  skipped_names INTEGER NOT NULL
) STRICT;
```

- `FORMAT_TABLES` (`schema.rs:318`) gains `file_stats` before `files` and `walk_notes` before `worktrees`, children first.
- The dump's `TABLES` (`dump.rs`) names neither table, and its module documentation says why: they record what the file system said, not what the bytes are.
- `INDEX_FORMAT` becomes 17 (`lib.rs:284`). Its documentation gains "17: `file_stats` and `walk_notes`, the freshness step's tables, outside the dump (`docs/features/read-trust.md` "Data")". `tests/format_history.txt` gains a `17 <hash>` line; the dump differs from 16's only in `index_meta`.
- Nanoseconds count from the Unix epoch, negative before it, and an `i64` holds them until 2262. Inode and device are compared for equality only.

Example, after `spec index` of a tree with `[paths] roots = ["docs", "notes"]`, `notes` missing and `docs/private` mode 000:

```text
file_stats  file_id 1, size 1834, mtime_ns 1793750400123456789, ctime_ns 1793750400123456789, ino 48213377, dev 16777230, taken_ns 1793750473000000000
file_stats  file_id 2, size 412, mtime_ns 1793750471900000000, ctime_ns 1793750471900000000, ino 48213380, dev 16777230, taken_ns 1793750473000000000
walk_notes  wt 1, missing_roots ["notes"], unreadable_dirs ["docs/private"], skipped_names 0
```

The next `update` trusts file 1, whose key was taken 72.9 s after its change. File 2 is racy, its key taken 1.1 s after its change: the next `update` reads it again, and stores its new `taken_ns` once that is 2 s past the change.

### The key and the racy rule

Store `write.rs`:
1. An update reads the wall clock once, as `taken_ns`, before its first `stat`.
2. For each file it lists or is named, it calls `key = source.stat(path)` before it calls `source.read(path)`, if it reads at all.
3. `update` trusts a file when all of these hold:
   - this is no `reparse_all`;
   - the stored row has a BLAKE3 and a `file_stats` row;
   - `key` is `Some` and equals the stored five fields;
   - `stored.taken_ns - max(stored.mtime_ns, stored.ctime_ns) >= RACY_NS`, where `RACY_NS` is 2 000 000 000.

   A trusted file is `Unchanged(stored hash)`, counted `unchanged`, and never read. `update_paths` and `rebuild` trust no key.
4. Any other file is read and hashed, as today.
   - If the hash is equal, the rows stand. `file_stats` is rewritten only when the key differs from the stored one, when none is stored, or when the stored key was racy and `taken_ns - max(key.mtime_ns, key.ctime_ns) >= RACY_NS`. A racy row read again with the same key is not rewritten.
   - If the hash differs, the file is re-parsed and the new rows get the key.
5. A file read with `key` `None` keeps no `file_stats` row; any row it had is removed.
6. `rebuild`, and every `reparse_all`, read every file and store every key.
7. `UpdateReport.hashed` counts the files read. `UpdateReport.committed` is true when a write transaction committed. Both are `#[serde(skip)]`.

### Store API

`specengine-store`:
- `pub struct StatKey { pub size: u64, pub mtime_ns: i64, pub ctime_ns: i64, pub ino: u64, pub dev: u64 }`, deriving `Debug, Clone, Copy, PartialEq, Eq`.
- `Source::stat(&self, path: &str) -> Option<StatKey>`, defaulting to `None`.
  - `WorkingTree` on `cfg(unix)` (`std::os::unix::fs::MetadataExt`) takes `fs::symlink_metadata(root.join(path))`. It gives `None` for an unclean path, an error, a symlink, or anything that is not a regular file. On other targets it gives `None`.
  - `GitIndex` keeps the default.
  - Every wrapper forwards `stat`: CLI `subset::Listed` (`subset.rs:534`, the freshness step's source) and the store tests' `Reversed` (`tests/common/mod.rs:469`).
- `pub struct WalkNotes { pub missing_roots: Vec<String>, pub skipped_names: usize, pub unreadable_dirs: Vec<String> }`, deriving `Debug, Clone, Default, PartialEq, Eq`, with `From<&UpdateReport>`.
- `SqliteIndex::trusted_notes(&self, scheme: &IdScheme, paths: &Paths) -> Result<Option<WalkNotes>, StoreError>` runs one deferred transaction and never writes. It gives `Some` when all of these hold, else `None`:
  - the stamp is this build's;
  - the handle's worktree row exists;
  - its `scheme_fp` and `paths_fp` equal `fingerprint(scheme)` and `paths_fingerprint(paths)`;
  - its `walk_notes` row exists.
- `UpdateReport.hashed: usize` and `UpdateReport.committed: bool`.
- `IndexWriter` keeps its three methods and their signatures.

### CLI API

A new module, `crates/specengine-cli/src/freshness.rs`, re-exported from `lib.rs`:

```rust
pub const RECONCILE_EVERY: Duration = Duration::from_secs(300);

/// A served project's trust in its index; a hub's daemon keeps one per
/// project for the process's life. Debug, Send, Sync.
pub struct IndexTrust {
    // root: PathBuf (canonical), every: Duration,
    // Mutex<{ walked: bool, dirty: bool, since: Option<Instant>, walks: u64 }>
}

impl IndexTrust {
    /// Not walked yet; `every` = RECONCILE_EVERY.
    pub fn new(root: PathBuf) -> Self;
    #[doc(hidden)]
    pub fn with_every(root: PathBuf, every: Duration) -> Self;
    /// The dirty mark (T4).
    pub fn doubt(&self);
    /// Walks begun inside a scope.
    #[doc(hidden)]
    pub fn walks(&self) -> u64;
}

/// `run` with `trusts` in scope on this thread, restored on return or unwind.
pub fn trusted<T>(trusts: &[&IndexTrust], run: impl FnOnce() -> T) -> T;
```

Crate-private:
- `freshness::step(index, project, tree) -> Result<Vec<Message>, CliError>` is the reads' freshness step. It sits at `corpus.rs:49`, `:65`, `:82`, `search.rs:137` (which builds its `WorkingTree` as `refresh` did) and `subset.rs:99`. With no scope, or no trust whose root equals `project.root` (both canonical), it is `refresh_in(.., false)`. With a matching trust it follows R8.
- `refresh_in` (`refresh.rs:128`) always walks. Inside a scope whose trust matches, it also keeps R9's books. `refresh` and `spec index` stay walking.
- `freshness::doubt(root: &Path)` marks the scope's trust for `root`; outside a scope it does nothing.
- `refresh::walk_warnings(project, &WalkNotes) -> Vec<Message>` holds today's three loops of `refresh_in` (`refresh.rs:147-166`), for both paths.
- `post_commit_update` is one helper for the four post-commit `update_paths` calls (R10).
- `sync_once` (`sync.rs:229`) gains `env: &Env` after `repository`, for the merge's update (R11).

### The daemon

`specengine-http`:
- `Project` (`start.rs:19`) gains `trust: Option<Arc<IndexTrust>>`. `App::new` (`app.rs:105`) sets it to `IndexTrust::new(root)` when `hub` is `Some`, and to `None` otherwise.
- `answer::run` (`answer.rs:205`): inside the blocking call, holding the turn, the command runs in `freshness::trusted(&[trust])` when the project has a trust. A panic or an `Answered::CannotRun` calls `trust.doubt()`.
- `HubSeam::turn` (`mcp.rs:118`) does the same around `command.run`; a panic or `cannot_run` doubts.
- The sync's merge (`sync.rs:110-117`), holding every turn of its projects, runs `merge` in `freshness::trusted` of their trusts, which `RepoSync` keeps beside `turns`.
- `answer::read`, which runs outside the turn, is unchanged.

### The hub's MCP texts

`crates/specengine-mcp/src/hub.rs`:
- The instructions line at `:262`, `- Reads refresh SpecEngine's index outside the project root; no tool writes under it. Spec files change only when a person confirms.`, becomes:
  `- Reads answer from the hub's index outside the project root: each confirm and merge updates it, a read first checks it against the files when the last check is 5 minutes old, and a file a read prints is checked as it is now. No tool writes under the root. Spec files change only when a person confirms.`
- `hub_read_tail!` at `:339`, `Reads the files as they are now: refreshes SpecEngine's index in its data directory, writes nothing under the project root.`, becomes:
  `Reads the hub's index, which each confirm and merge updates and a read checks against the files when the last check is 5 minutes old; a file it prints is checked as it is now. Writes nothing under the project root.`
- Each description stays within `DESCRIPTION_LIMIT` (2048). Stdio's texts (`read.rs`) keep theirs.

### Eval

`specengine-eval scale` (`docs/canon/scale-eval.md` "Output"). Each size gains:
- `reads_trusted.<read> {median_us, max_us}`: the four reads, run in `freshness::trusted` of an `IndexTrust` that the warm-up call walked;
- `growth_trusted.<read>`: `max(median, 10 000) / max(previous median, 10 000)`, in microseconds, to two decimals, `null` first;
- `trusted_differs`: the (read, target) pairs whose trusted answer's BLAKE3 differs from the walking answer's;
- `steps.update_hashed`: the most files any warm `update` read, 0 when warm. `update_us` is now the stat walk, and `hash_us` still replays reading and hashing every file;
- `hub.trusted {get_node_links_us, get_context_bundle_us}`.

The stderr summary line adds the trusted medians. AC-14 is its own `#[ignore]`d test, which prints `{plain_us, fan_out_leaf_us, fan_out_index_us}`, each the best of three.

### Config

No `specengine.toml` key. `RACY_NS` (store) and `RECONCILE_EVERY` (CLI) are constants, with no flag, key or variable.

## Rules and edge cases

- **R1.** WHEN `update` meets a file whose stored row has a BLAKE3 and a key equal to the file's now, taken 2 s or more after that key's `max(mtime, ctime)`, it SHALL NOT read the file.
- **R2.** Otherwise `update` SHALL read and hash the file. `update_paths` and `rebuild` SHALL read every file they name or list, whatever its key.
- **R3.** WHEN a read file's hash is unchanged, its rows SHALL stand (`parsed` 0). Its key SHALL be stored when it differs from the stored one, when none is stored, or when it ends the stored key's raciness. A racy key read again unchanged SHALL NOT be rewritten.
- **R4.** WHEN `stat` fails, or the source has none, the file SHALL be read, and SHALL keep no key.
- **R5.** WHEN all of these hold, `update` and `update_paths` SHALL return without opening a write transaction, with `committed` false:
  - the stamp is current and the worktree row exists;
  - both fingerprints are unchanged, and this is no rebuild;
  - every plan is unchanged, with no key to store;
  - no stored path is gone;
  - a listing's notes equal the stored notes.
- **R6.** WHEN a file stored without a hash fails to read again with the same `read_error` and `size`, its rows SHALL stand, counted `unreadable` as today.
- **R7.** `spec index --full` SHALL read every file and store every key, whatever is stored.
- **R8.** Inside a scope, WHEN the trust has not walked since start, is dirty, or began its last walk `RECONCILE_EVERY` or more ago, the reads' step SHALL walk. Otherwise it SHALL call `trusted_notes`:
  - on `Some`, it SHALL print `walk_warnings` of the notes and update nothing;
  - on `None` or an error, it SHALL walk.
- **R9.** WHEN `refresh_in` begins inside a scope whose trust matches, it SHALL clear the mark and count the walk. On success it SHALL set `walked`, and set `since` to the walk's start; on failure it SHALL set the mark.
- **R10.** WHEN a post-commit `update_paths` fails (`apply.rs:1139-1146`, `reindex` at `apply.rs:1825-1835`, `create.rs:1655-1662`, `decide.rs:738-745`), the warning SHALL print as today and the root SHALL be doubted. All four go through `post_commit_update`.
- **R11.** WHEN a merge's fast-forward (CLI `sync.rs:412`, in `merge`, `:377-421`) succeeds, the library SHALL, inside the turns:
  - take `git diff --name-only -z --no-renames <L> <target> --` under the deadline;
  - for each served root of the repository, keep the paths under the root's path from the top (`""` or `<dir>/`, as `MergedRoot.prefix`), with that prefix stripped;
  - where any path is kept: discover the root's project again (the merged config), open its index, and run `update_paths` on them through a `WorkingTree` of the merged `[paths]` with the merged scheme. The store walks when a fingerprint moved.

  A failed diff SHALL doubt every root; a failed discovery or update, its own root. `Contained` and `Same` move nothing and update nothing.
- **R12.** WHEN a fast-forward fails or is killed at the deadline, every served root of the repository SHALL be doubted: its files may be half moved (`docs/canon/hub-git.md` "Known limits").
- **R13.** WHEN a trusted read finds a T5 or Q7 mismatch, a `covers` failure or a store error in an attempt, it SHALL doubt its root, then go on as 12d does: `update_paths` and a retry, a re-parse, or the walk.
- **R14.** Apply's step 4 (`preflight.rs:297`, `decide.rs:304`, `create.rs:1137`) and `spec index` SHALL walk in any scope.
- **R15.** A trust SHALL never refuse a read, and no status, check or gate SHALL read it (ADR-0012). A doubt outside a scope SHALL do nothing.
- **R16.** A local daemon, the CLI, stdio MCP and eval's walking reads SHALL enter no scope.

**When the key lies.** A wrong "moved" costs one read; a wrong "unchanged" misses an edit.

| Case | What the key does | Result |
|---|---|---|
| `cp`, `cp -p`, `ditto`, an APFS clone | a new inode and ctime | the file is read once |
| `git checkout`, `switch`, `stash`, `restore` | git writes a new file: a new inode, mtime now; a file git leaves keeps its key | the written files are read, the rest trusted |
| An editor's atomic save (write a copy, rename it over) | a new inode | read |
| An in-place same-size edit with mtime restored (`touch -r`, `File::set_times`, `rsync --inplace`) | ctime moves | read |
| `touch` | mtime and ctime move | read once, `parsed` 0 (R3) |
| A write within the same coarse tick (HFS+ 1 s, FAT 2 s) | may not move | read: the racy window (R1) |
| A driver whose ctime does not move on a write (some FUSE and FAT-family drivers), with an in-place same-size edit and mtime restored | nothing moves | missed until the key moves or `--full`; a known limit |
| A new device number after a remount, or inodes renumbered (FUSE, SMB) | moves | every file read once |
| A network client's attribute cache (NFS keeps 3-60 s by default; SMB) | lags another machine's write by the cache's life | seen once the cache expires; this machine's own writes at once; a known limit |
| The wall clock set back, or a file dated in the future | `taken - change` is negative | racy: read every update, never rewritten (step 4) |
| The wall clock set forward | a recent change looks old | only a same-tick write on a coarse file system is missed |
| Docker's overlay copy-up | a new inode | read once |

There is no network file system detection: `std` has no `statfs`, and the store links no libc (`hub-git.md` "Sync", the deadline).

## Acceptance criteria

Tests run on scratch copies under `std::env::temp_dir()`, with non-ASCII written as escapes. M names the mutations that must turn each criterion red: 25 in all. A test that waits out the racy window sleeps 2.1 s, no more.

- [ ] **AC-8 [default, store] The stat key** (store `tests/stat_key.rs`; a `Source` wrapper that counts `read` calls and forwards `stat`).
  - (a) Over a 50-file tree last changed 2.1 s or more before the first `update`, a second `update` reads no file: `hashed` 0, the wrapper's count 0, `unchanged` 50. Through the CLI, a warm `search` and a warm `show` read no file in their freshness step (`subset::seams`' counters, `#[cfg(test)]`). M: the key ignored, so every file is read; `Listed` without `stat`.
  - (b) On a file indexed 2.1 s after it was written, a same-size edit followed by `File::set_times` back to the old mtime, to the nanosecond: the next `update` reads the file and stores its new rows. M: ctime left out of the key.
  - (c) A source whose `stat` returns one fixed key, its mtime an hour back and its ctime the current time at each call, while its bytes change between updates at one size: every update reads the file and sees the new bytes. With the ctime an hour back too, the second update reads nothing. M: no racy rule; raciness judged by mtime alone.
  - (d) `touch` (or `set_times` to now), wait 2.1 s, then `update`: `parsed` 0 and `hashed` 1, and the next `update` reads nothing. The same holds when the first update is `update_paths` of the path. M: the key not stored on an unchanged hash; `update_paths` storing no key.
  - (e) Two copies of one tree, made 2 s apart at two roots and each `update`d: `dump_worktree` is equal (root masked), while their `file_stats` differ. M: `file_stats` or `walk_notes` in the dump.
  - (f) A warm no-op takes no write lock and commits nothing:
    - (f1) while a second connection holds `BEGIN IMMEDIATE`, `update`, and `update_paths` of an unchanged path, return `Ok` within 1 s (`busy_timeout` is 5 s) with `committed` false;
    - (f2) without that lock, the second connection's `PRAGMA data_version` is equal before and after.

    Both run again with one mode-000 file in the tree, skipped when the test runs as root. M: the unconditional `Immediate` transaction (`write.rs:211`) kept; keys or notes rewritten on every update; an unreadable row rewritten on every update.
  - (g) After `update`, `walk_notes` hold the listing's three facts: a missing written root, a mode-000 directory, and a non-UTF-8 name where the file system allows one. An `update_paths` that probed leaves them. `trusted_notes` gives them, and gives `None` for an old stamp, another `[ids]` or `[paths]` fingerprint, or no worktree row. M: `trusted_notes` ignoring `paths_fp`.
- [ ] **AC-9 [default] Existing guarantees.** These stay green with no edit: CLI `tests/freshness.rs` (AC-08, AC-17, `:285`); store `tests/incremental.rs`, `tests/edges.rs` (AC-1's seeds), `tests/concurrency.rs` and `tests/projection.rs`; CLI `tests/store_reads.rs` with its golden; and every `spec index` text and JSON golden. `tests/format.rs` passes with its one new `17` line. M: none; this criterion guards against a test bent to fit.
- [ ] **AC-14 [ignored, eval release] One file's increment.** On the `--corpus-only` 100 000 tree, indexed and warm, `update_paths` of one edited file (its body changed, its IDs kept), edges included, takes at most 200 ms, best of three (08 s3 AC-10, "per-file increment <= 200 ms"). Also measured, with no bar (D14): the same tree plus one file citing every document by path, editing (i) a leaf file and (ii) that file. The test runs first at the slice's base, `ceb16ca`, then at its head; both sets of numbers go in "Implementation". M: every edge recomputed on every change (a full pass).
- [ ] **AC-4 [default, cli] Two modes, one answer (T1, T2).** With an `IndexTrust` whose first read walked:
  - every row of `tests/store_reads_before.json` (2 322 over the 19 configured fixtures) replays equal inside `trusted`, plain and `--json`, stderr included, through the library calls the binary makes;
  - `search`, `tree` and `flow` on each configured fixture give the walking read's answer when trusted;
  - on a scratch tree with a written `[paths]` root missing and a mode-000 directory, a trusted `search` and a trusted `show` print the walk's two warnings, in order.

  M: the trusted step printing no warnings (none replayed).
- [ ] **AC-5 [default, cli] Trust skips the walk.** After a walking read, a file is deleted on disk. A trusted `search` still finds its node, with `walks()` unchanged; a read with no scope does not find it. M: the trusted step calling `refresh_in`; a trust whose root never matches, so every read walks.
- [ ] **AC-6 [default] Invalidation.** After each event the next read shows the disk, as a walk would.
  - Through http, a hub over `hub_git.rs`' bare remotes (`--public-origin`, `--push`):
    - (a) a confirm: `search` finds the new text;
    - (b) a fast-forward and (c) a merge commit from the remote: `search` finds a node added there;
    - (d) a merge that changes `[paths] roots` to add a directory holding a file: that file's node is found;
    - (e) a hub restarted over an index made stale while it was down: the first `search` finds the edit.
  - Through the CLI library:
    - (f) `post_commit_update` given a source whose root is not the index's (an injected store error): the warning prints as today, the trust is dirty, and the next `search` walks;
    - (g) a hand edit: a trusted `search` does not see it until `with_every`'s interval passes, then finds it, with `walks()` up by one. `show` of a node in that file sees the edit at once (T5) and marks the trust dirty.

  M: the merge's `update_paths` dropped; `specengine.toml` ignored, the update run with the config as read before the merge; the dirty mark dropped; the reconcile dropped; T5 dropped from trusted reads; a trust that starts walked.
- [ ] **AC-7 [default, cli] One state per read, in both modes.** `tests/freshness.rs:285` passes unedited, and passes again inside `trusted`. 12d's flipper (`store_reads.rs` AC-7's swap, mtime restored to the nanosecond) gives zero mixed answers in trusted mode. M: bytes printed without their BLAKE3 compare.
- [ ] **AC-13 [ignored, eval release] The bars.** Run `cargo nextest run -p specengine-eval --release --test scale_cli --run-ignored ignored-only scale_baseline --no-capture`, at seed 1, for 1k, 10k and 100k, in a scratch `--out`:
  - trusted `get_node`, `get_node_links`, `search` and `get_context_bundle` each take under 1 s at 100k, with `growth_trusted` under 2 at 100k;
  - `steps.update_hashed` is 0 at every size, and `update_us` is reported;
  - W_after's median at 10k is within 10 % of 1k's;
  - `nondeterministic` and `trusted_differs` are both 0;
  - the run is bounded by its `--timeout`, writes nothing under the repository or `HOME`, and prints counts and times only (eval README).

  The numbers, base and head from one session, go in `scale-eval.md` "Baseline". M: the trusted step calling `refresh_in`, which gives about 11x growth.
- [ ] **AC-15 [default] The default run stays fast.** No new default-run test takes more than about 10 s, and each heavy test is `#[ignore = "<command>"]`.
- [ ] **RT-1 [default, mcp] The hub's texts.** A hub's `initialize` instructions and each hub read tool's description hold the new sentences of "Data" byte for byte, and no longer hold `Reads refresh` or `Reads the files as they are now`. Stdio's texts are unchanged, and their tests unedited. M: a text left as before.
- [ ] **RT-2 [docs, at ship] The guide.** `scripts/deploy/README.md` names `docker compose exec specengine spec index --root /projects/<name>` in both "Accounts" and "Limits"; `grep` finds both lines.
- [ ] **F1 [form].**
  - `cargo clippy` and `cargo fmt --check` are clean, with no new dependency.
  - `PARENTED`'s comment is fixed, and the lookup and its index are kept.
  - The code index's write path (CLI `code_index.rs`, store `code.rs`) and `IndexWriter`'s signatures are unchanged.
  - No developer changes `docs/`.
  - `check` gives 0 errors and 0 warnings.

## Out of scope

- A file watcher; a timer task for the reconcile; trust on a local daemon; a flag, key or variable for either mode, the racy window or the interval.
- Doubt across processes: a terminal's `spec approve` or `spec index` on a hub's host writes rows the daemon then reads as written, but a failure of theirs waits for the reconcile.
- Network file system detection; the code index's key (Q5).
- The fan-out's fix (D14).
- Subset `tree`, `graph`, `flow` and `impact` reads; `catch_unwind` around resolution.
- Step 13, `zone-index` (RM-0055).

## Implementation

To be filled in after implementation. The plan and the ship's canon follow.

### Files in scope

| Crate | Files | What changes |
|---|---|---|
| store | `schema.rs`, `lib.rs`, `source.rs`, `write.rs`, `read.rs`, `dump.rs`, `edges.rs:95` | the two tables, `INDEX_FORMAT` 17, `StatKey`, `Source::stat`, the key and the racy rule, R5, R6, `walk_notes`, `trusted_notes`, the dump's documentation, `PARENTED`'s comment |
| CLI | `freshness.rs` (new), `lib.rs`, `refresh.rs`, `corpus.rs`, `search.rs`, `subset.rs`, `apply.rs`, `create.rs`, `decide.rs`, `sync.rs` | the scope, `step`, `walk_warnings`, doubt, `post_commit_update`, `Listed::stat`, the merge's update |
| http | `start.rs`, `app.rs`, `answer.rs`, `mcp.rs`, `sync.rs` | `Project.trust`, the three scope sites, doubt on a panic or exit 2 |
| mcp | `hub.rs` | the two texts |
| eval | `src/scale/mod.rs` | the trusted reads and the new keys |
| tests (test-engineer) | store `tests/stat_key.rs`, `tests/common/mod.rs` (`Reversed::stat`), `tests/format_history.txt`; CLI `tests/read_trust.rs` and `#[cfg(test)]`; http hub tests beside `hub_git.rs`; mcp hub text tests; eval `tests/read_trust_scale.rs`, `tests/scale_cli.rs` | AC-4 to AC-9, AC-13, AC-14, RT-1 |

### Work order

1. AC-14's test at the base, `ceb16ca`, plain and with fan-out; record the numbers.
2. Store: the tables, the key, the racy rule, R5, R6, the notes, format 17; AC-8, AC-9.
3. CLI: the scope, `step`, `walk_warnings`, doubt, `post_commit_update`, `Listed::stat`; AC-4, AC-5, AC-6 (f) and (g), AC-7.
4. The merge's update (R11, R12) and the http wiring; AC-6 (a) to (e).
5. The MCP texts; RT-1.
6. Eval's keys; AC-13, and AC-14 at the head.

### Before

12d's head, from `store-reads`' SR-4 session of 2026-11-04 (release, seed 1). Times are medians in ms:

| | 1k | 10k | 100k |
|---|---|---|---|
| files | 220 | 2 022 | 20 015 |
| `update_us` | 3.62 | 31.7 | 376 |
| `hash_us` | 2.96 | 28.8 | 349 |
| `walk_us` | 0.50 | 1.46 | 11.7 |
| `fts_us` | 0.41 | 1.02 | 6.02 |
| `get_node` | 4.21 | 32.1 | 372 |
| `get_node_links` | 4.37 | 32.5 | 373 |
| `search` | 4.59 | 33.8 | 380 |
| `get_context_bundle` | 4.54 | 32.7 | 376 |
| hub links, hub bundle | 4.91, 5.53 | 35.0, 41.6 | 398, 494 |
| W_after median, B | 2 210 | 2 182 | 2 239 |

AC-14 was not measured then; work order step 1 measures it.

### Canon at ship

Free bytes are given as of this spec.
- **New `docs/canon/spec-read-freshness.md`** (Tier 2, claimed by `docs/index-canon-spec.md`, which has 4.6 KB free; scope `[crates/specengine-store, crates/specengine-cli, crates/specengine-http]`). Its sections:
  - an intro: ADR `read-freshness`, built by `read-trust`, the working answers;
  - "The freshness step", the ADR's anchor;
  - "The stat key", with R1-R7 and the table of lying keys;
  - "No-op updates"; "Walk notes";
  - "A hub's trusted index", with D6-D11 and R8-R16;
  - "The window"; "API"; "Known limits"; "Tests".
- **Store README** (9 B free):
  - "Cache key and format stamp": read and hash iff the key moved or is racy, pointing to the page;
  - "Rows": the two tables, which the dump leaves out;
  - "Writes": a no-op update opens no write transaction.
  - Room comes from the README's own sentences that the page now holds.
- **CLI README** (4 B free): the "Freshness" bullet names the reads' step (`freshness.rs`, the page). **http README** (137 B free): `Project.trust` and the three scope sites. **mcp README** (177 B free): the hub texts.
- **`spec-read-subset.md`** (1.5 KB free): "The read" step 2, the window in "Known limits", and the intro's "(16)".
- **`spec-index-edges.md`** (55 B free): "Known limits" "Hub fan-out" gets AC-14's number, and "Passes" says a no-op opens no transaction.
- **`scale-eval.md`** (2 B free): "Output" gets the new keys; "Baseline" gets "After 12e" and whether each bar was met. Room comes from folding "After 12d" into the new table.
- **`hub-git.md`** (2 B free): "Sync" step 3 updates each root's index in the turns, and the first bullet of "Known limits" says the projects are marked dirty. **`hub-confirm.md`** (1.6 KB free): "Known limits" says a failed post-commit update marks the project dirty. **`hub-mcp.md`** (2 B free): "Turn" gets the scope, "Tools" the two texts.
- **`symbol-index.md`, `layer-c.md`:** `INDEX_FORMAT` 16 becomes 17.
- **`scripts/deploy/README.md`** (709 B free), for RT-2:
  - "Accounts", after the hand `git pull`: "then `docker compose exec specengine spec index --root /projects/<name>`";
  - a "Limits" bullet, **Hand changes**: the hub reads its index, which its confirms and merges keep current; a change made on the host by hand shows after that command, or within 5 minutes of reads.
- **`CLAUDE.md`:** the state line names `read-trust`, step 12 closed, the ADR and the page.
- **Plan:** RM-0054 shipped and compressed; 08's row 12e shipped, with the ADR's number; 08's "ADRs" paragraph.
- **Unchanged:** `architecture.md` (4 B free; D16), ADR-0001, ADR-0003 and ADR-0012.

### Draft ADR

This is a working answer, awaiting the owner. It takes the next free number and its date at ship. About 1.44 KB with its front-matter:

```markdown
---
class: decision
id: ADR-0069
title: A read trusts a file's stat key outside a 2 s racy window, and a hub's reads trust the index its writers keep
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/spec-read-freshness.md#the-freshness-step
ref: read-trust (RM-0054, step 12e); the orchestrator's Q1-Q6 of 2026-11-03 and D1-D16, awaiting the owner's review
---

# ADR-0069

**Decision.** Accepted by the orchestrator in the owner's absence, awaiting the owner's review. A read's freshness step reads a file only when its stat key (size, mtime and ctime in ns, inode, device) moved or was taken within 2 s of the file's last change; no opt-out, `spec index --full` repairs, the code index stays on content. On a hub alone, a read answers from the index its confirms and merges update in the turn, replaying the last walk's warnings; it walks first after start, after a doubt, and 5 minutes after the last walk, and a file it prints is checked against its row.

**Why.** At 100 000 nodes the step was the read: 376 ms, 349 of it hashing files to learn that nothing changed. A hub's tree has no editor of its own.

**Cost.** A same-size edit that moves neither mtime nor ctime (no real ctime, a network client's attribute cache) waits for a key move or `--full`. On a hub, a hand edit on the host shows only in a printed file, after a reconcile or after `spec index`; another process cannot raise the dirty mark.
```
