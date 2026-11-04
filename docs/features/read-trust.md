---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, crates/specengine-eval, scripts/deploy]
ref: RM-0054; Phase 7 step 12e, the last of step 12 `scale-reads` (08 s2 "Step 12 in five"); Q1-Q6 of 2026-11-03, D1-D16 and the decisions taken during implementation are the orchestrator's working answers, awaiting the owner's review
adrs: [ADR-0069]
---

# read-trust: a freshness step keyed by stat, and a hub's reads on the index its writers keep (RM-0054)

## Why

Since `store-reads` (12d) a read cost its freshness step: at 100 000 nodes 372-380 ms, 349 of it hashing 20 015 files to learn that nothing changed, then a write lock to commit nothing. A hub paid the same, though only its confirms and merges write its tree. This slice keys the step on each file's stat, lets a hub's reads answer from the index its writers keep, sets the bars at 10 000 and 100 000 nodes, and closes step 12. Beside T3 and T5 (`docs/canon/spec-read-subset.md`):
- **T1.** A trusted read equals the walking read, text, JSON and stderr, byte for byte, whenever a walk would change nothing.
- **T2.** A trusted read prints the last walk's warnings, replayed from the index.
- **T4.** Any doubt means a walk: a dirty mark per project, per process.

Decided, awaiting the owner (now: `docs/canon/spec-read-freshness.md`):
- **D1** One slice: both halves share format 17, one ADR, one walk.
- **D2** The stat key (Q4): size, mtime and ctime in ns, inode, device; a 2 s racy window, no opt-out; `update_paths` and `--full` read every file.
- **D3** `file_stats` and `walk_notes`, two tables outside the dump.
- **D4** A no-op update takes no write lock.
- **D5** The code index stays on content (Q5).
- **D6** Trust is a hub's alone: `--public-origin` is the switch.
- **D7** The mode: a per-call thread-local scope, no signature changed.
- **D8** One `IndexTrust` per served project: a walk at start, on a doubt, and 300 s after the last (a lazy reconcile).
- **D9** Doubt: a failed post-commit update, merge or fast-forward, a trusted read's T5 or Q7 mismatch, a panic or exit 2, a refused confirm.
- **D10** Writers keep the index in the turn: a confirm's paths, a merge's `git diff`.
- **D11** `walk_notes` holds the listing's three facts, rendered by one function.
- **D12** The hub's MCP texts say what a hub's read trusts; stdio's keep "refresh".
- **D13** The bars (Q1, Q2): trusted reads under 1 s at 100k, growth under 2x from 10k with a 10 ms floor; the warm update reads no file; W_after within 10 %.
- **D14** Fan-out measured, no bar; a plan item above 1 s.
- **D15** `PARENTED` kept, its comment fixed. **D16** One ADR, ADR-0069 `read-freshness`; `architecture.md` unchanged.
- **During implementation:** only reads trust; a route or tool that writes, authorizes a write or is not classified walks, and trace data walks wherever it is shown.

## Description and interactions

How it works now: `docs/canon/spec-read-freshness.md`. Also changed: `spec-read-subset.md`, `spec-index-edges.md`, `scale-eval.md`, `hub{,-git,-mcp,-deploy,-confirm}.md`, five READMEs, the guide. Outputs, routes and tools are unchanged.

## Data

The canon page's "The stat key", "Walk notes and the format" and "API".

### The key and the racy rule

R1-R4, R7; `RACY_NS` 2 s.

### Store API

`StatKey`, `Source::stat`, `WalkNotes`, `trusted_notes`, `UpdateReport.{hashed, committed}`, `ChangedPaths`.

### CLI API

`IndexTrust`, `RECONCILE_EVERY`, `trusted`, `walking`, `trusting`, `doubt_in_scope`.

### The daemon

`Project.trust`; `answer::run` walks, `run_read` trusts, `run_as` lets the request decide; a panic or exit 2 doubts.

### The hub's MCP texts

`HUB_INSTRUCTIONS` and the tails of five trusting reads (mcp `hub.rs`, `read.rs`); `HUB_READ_TOOLS`, `reads_trust`.

### Eval

`scale-eval.md` "Output"; `RACY_WAIT`.

## Rules and edge cases

R1-R16 hold as the canon page states them: R1, R2, R3, R4, R7 the key; R5, R6 no-op updates; R8, R9 the trust and its walks; R10 (commits), R11 (merges), R12 (fast-forwards), R13 (reads) doubt; R14 apply's step 4 and `spec index` walk; R15 no status, check or gate consults `IndexTrust` (the `check` and `health` routes are display reads that trust); R16 no scope off a hub.

## Acceptance criteria

Scratch copies, non-ASCII as escapes; M: the mutations to turn red, 25 named. Mutations: all 25 named killed. Three first survived on test gaps, each closed by a new test: the merged config ignored (the read after the merge must not walk), the dirty mark dropped (killed at the mark itself), an MCP write tool classed as a read (a card write). Not observable: one doubt per read, with no doubt counter. Not tested apart: a fast-forward killed at the deadline (R12); only a failed `merge --ff-only` is.

- [x] **AC-8 [default, store] The stat key** (`tests/stat_key.rs`, raw connections). (a) Warm, 50 files: no file read, in CLI `search` and `show` too. (b) A same-size edit, mtime restored, is read. (c) A racy key reads every time. (d) `touch`: `parsed` 0, `hashed` 1, then nothing. (e) Two copies dump equal. (f) A no-op returns under another's `BEGIN IMMEDIATE`, `committed` false, `data_version` unmoved. (g) The notes' three facts; `trusted_notes` `None` on a stale stamp, fingerprint or row. M: no key, `Listed` without `stat`, no ctime, no or mtime-only raciness, no key stored when unchanged, the tables dumped, the unconditional `Immediate`, rewrites, `paths_fp` ignored.
- [x] **AC-9 [default] Existing guarantees** stay green, `freshness.rs`, `incremental.rs`, `concurrency.rs`, `projection.rs` and the goldens unedited; the format pins in `store_reads.rs`, `edges.rs`, `bundle_config.rs`, `format.rs` moved 16 -> 17 mechanically, `format_history.txt` gaining `17`.
- [x] **AC-14 [ignored, eval release] One file's increment** (`tests/read_trust_scale.rs`), `--corpus-only` 100k, seed 1, best of 3. Base `7c0ca35` (crates as `ceb16ca`): plain 7.0 ms, bar 200 ms; fan-out, no bar, a leaf 409 ms, the index file 519 ms: under 1 s, no plan item. Head: plain 10.2 ms, passed; leaf 422 ms, index file 536 ms. M: a full pass on every change.
- [x] **AC-4 [default, cli] Two modes, one answer (T1, T2).** `store_reads_before.json`'s 2 322 rows replay equal trusted, plain, `--json`, stderr; `search`, `tree`, `flow` on every fixture; replayed warnings in order. M: no replayed warnings.
- [x] **AC-5 [default, cli] Trust skips the walk:** a deleted file's node is still found trusted, `walks()` unchanged, and gone with no scope. M: the step calling `refresh_in`; a root that never matches.
- [x] **AC-6 [default] Invalidation.** A hub shows (a) a confirm, (b) a fast-forward, (c) a merge commit, (d) a new `[paths]` root, (e) a restart's stale index; (f) a failed `post_commit_update` warns and doubts; (g) a hand edit shows after `with_every`'s interval, in `show` at once (T5). M: no merge update; the old config; no mark; no reconcile; no T5; a trust that starts walked.
- [x] **AC-7 [default, cli] One state per read:** `freshness.rs:285` unedited, also trusted; the flipper 0 mixed trusted, capped at 150 runs or 8 s, at least 40. M: bytes printed without their BLAKE3 compare.
- [x] **AC-13 [ignored, eval release] The bars:** `scale_baseline`, seed 1: trusted reads under 1 s at 100k, `growth_trusted` under 2, `update_hashed` 0, W_after within 10 %, `nondeterministic`, `trusted_differs` 0: all met. Medians, ms, at 10k and 100k, 12d's head -> trusted: `get_node` 32.1, 372 -> 0.584, 0.552; links 32.5, 373 -> 0.721, 0.851; `search` 33.8, 380 -> 1.556, 6.496; bundle 32.7, 376 -> 0.833, 2.447; `update_us` 31.7, 376 -> 4.16, 41.9; walking now 4.90-5.93, 43.3-50.0; hub trusted links, bundle 2.99, 10.0; 23.7, 124. At every size `update_hashed` 0, both differ counts 0; `growth_trusted` 1.00 at 10k and 100k; W_after 2 210, 2 182 (-1.3 %), 2 239; trusted max at 100k 6.8 ms. M: the step calling `refresh_in` (about 11x).
- [x] **AC-15 [default] Fast default run:** each heavy test `#[ignore = "<command>"]`, no new default test over about 10 s.
- [x] **RT-1 [default, mcp] The hub's texts:** `initialize` and five hub reads hold the new sentences, no `Reads refresh`; stdio's unchanged. Widened at review iteration 3 to the four shared reads, `hub_mcp.rs:1766` updated. M: a text left as before.
- [x] **RT-2 [docs] The guide** names `docker compose exec specengine spec index --root /projects/<name>` in "Accounts" and "Limits".
- [x] **F1 [form]** clippy, fmt clean, no dependency; `PARENTED` kept; `IndexWriter` and the code index unchanged; `check` 0 errors, 0 warnings.

## Out of scope

A watcher, a timer, trust on a local daemon, a flag or key for any of it; doubt across processes; network file system detection; the fan-out's fix; subset `tree`, `graph`, `flow`, `impact`; `catch_unwind`; step 13 (RM-0055).

## Implementation

Review: ACCEPT with minors on iteration 1; review iteration 2 ACCEPT (more doubts, `roots_may_move`, only reads trust); review iteration 3 ACCEPT, the last (trace data walks, four hub tails).

| Module | What it does |
|---|---|
| store `source.rs`, `write.rs`, `read.rs`, `schema.rs`, `dump.rs`, `lib.rs` | the key, R5, R6, `roots_may_move`, the notes, format 17 |
| store `sync.rs`*, `edges.rs` | `SyncGit::changed`; `PARENTED`'s comment |
| CLI `freshness.rs`, `refresh.rs`, `corpus.rs`, `search.rs`, `subset.rs` | scopes, `step`, doubt, `walk_warnings`, `Listed::stat` |
| CLI `apply.rs`, `create.rs`, `decide.rs`, `sync.rs` | `post_commit_update`; the merge's `reindex` |
| http `start.rs`, `app.rs`, `answer.rs`, `mcp.rs`, `sync.rs`, `confirm.rs` | `Project.trust`, the scopes, doubt |
| mcp `hub.rs`, `read.rs`, `resources.rs`, `server.rs`, `probes.rs` | `reads_trust`, the texts, `doubt_in_scope`, `probe_panic` |
| eval `src/scale/mod.rs` | trusted reads, keys, `RACY_WAIT`* |

\* Outside the planned files. Deviations: `is_dirty` `#[doc(hidden)]`; no clock injection (tests sleep 2.1 s); `answer::run_as`; `get_flow` walks even without `trace` (no room for a hub tail); `api.rs`' guard admits `stat_key.rs`. Spec defects: RT-1's "each hub read tool"; AC-9's "no edit"; `HubSeam::turn` blind to an exit 2 inside `command.run`.
