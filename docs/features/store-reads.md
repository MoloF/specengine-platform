---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-eval]
ref: RM-0079; Phase 7 step 12d, split from RM-0054 on 2026-11-03; T3, T5, Q7 and D1-D11 await the owner's review
adrs: []
---

# store-reads: `show`, `--links` and `bundle` read a subset from the index (RM-0079)

## Why

To answer for one node, the reads took the whole corpus: at 100 000 nodes `get_node` cost 705 ms, `get_node_links` 1.64 s and `get_context_bundle` 1.61 s, with reparse, hash, decode and graph dominating (`docs/canon/scale-eval.md` "Baseline"). RM-0078 had stored core's resolution in the index, unread. Here `spec show`, `show --links` and `spec bundle`, with their MCP and daemon twins, read a subset: the rows of the files around the asked nodes, plus those files' bytes. Whole-corpus reads stay O(N) but stop re-parsing a file whose bytes match its row (Q7). Two of the step's invariants hold from here on:
- **T3.** One index snapshot per read.
- **T5.** Every printed byte matches the BLAKE3 of the row it came from. On a mismatch the read updates that path and retries; if that is not enough, it walks.

T1, T2 and T4 belong to RM-0054 (12e). Decided here:
- **D1** A subset read resolves no link: rows give ends, state, reason and label, the citing file's bytes give written forms and lines. Only a REF is resolved, over its closure.
- **D2** The subset per read: each file decoded once and read once.
- **D3** T3 is one deferred read transaction per attempt, closed before any write.
- **D4** T5 covers every subset file, printed or not: `update_paths` of exactly the mismatched paths, one retry, then the walk.
- **D5** The walk is the whole read, the fallback and the differential's oracle.
- **D6** The freshness step is unchanged.
- **D7** Code-layer reads stay whole, with Q7.
- **D8** Q7 is here: T5's rule in `read_bytes`, on the store's projection contract.
- **D9** The store API is read-only; `INDEX_FORMAT` stays 16.
- **D10** `pick` and `pick_reference` lose `written`; core's `resolve*` keep theirs.
- **D11** No ADR: no rule of `docs/canon/architecture.md` changes (files stay the truth, the index a rebuildable projection with no new rows, nothing blocked). What a read may trust without walking is RM-0054's ADR (Q6).

## Description and interactions

How it works now: `docs/canon/spec-read-subset.md`. Also changed: `spec-index-edges.md`, `spec-cli-{graph,bundle,read}.md`, `scale-eval.md`, the CLI, store and core READMEs. No MCP tool, route, JSON key, flag or config key changes.

## Data

The canon page's "The subset", "The walk" and "API".

### Store API

`snapshot() -> IndexSnapshot`, `Decoded`, `EDGE`, `PARENT`; `Stored*` are core's rows.

### Core API

`from_rows`, `GraphRows`, `resolve_detached_reading`, `bundle_linked`; `children` answered within a file (bundle's `target_forms`).

### CLI

`subset.rs`; Q7 in `read_bytes`; the `#[cfg(test)]` stage hook (a), (b), (c) and counters.

### Eval

`other_us` subtracts only open and update from the three subset reads.

## Rules and edge cases

R1-R9 hold as the canon page states them (R2: T3; R3: T5's match; R8: race cases; R9: D7). R7, missing or stale tables, gained **`covers`**: a row whose holder or end is not a node of the subset walks. Other corrupted-row shapes under an intact BLAKE3 are R7's accepted gap.

## Acceptance criteria

Scratch copies, non-ASCII as escapes; M: the mutations to turn red. Mutations: 20 of 22 named mutations killed. The two survivors, "C(REF) cut to one round" and "the bare form's key dropped from the closure", change no answer a CLI REF produces: look-alikes and mixed scripts are refused at classification, and an alias-prefix match keeps the run as written, so round 1 already reads every key of the fallback chain and round 2 reads none.

- [x] **AC-7 [default, cli] One state per read.** `freshness.rs:285` stays green, unedited. **The swap**: a same-size version swapped in at stage (b), mtime restored to the nanosecond, gives one version's at-rest answer for `show`, `--links` and `bundle`. Through the binary: 150 alternating `show --links` and `bundle` runs under a flipper: zero mixed, both versions seen (it found the walk's second read). M: no BLAKE3 compare; the attempt's rows kept with swapped bytes.
- [x] **SR-0 [default, cli] Equal answers.** **The golden**: `tests/store_reads_before.json`, 2 322 rows over the 19 configured fixtures, recorded from 2220d05's binary, never again. **Every row replays equal**, plain and `--json`. The existing goldens and parity tests stay unedited and green. **Differential**: the subset read equals the walk for every fixture node, path and REF error, and on the 1k `scale` tree's 20 targets, 4 hubs and 200 seeded nodes; the 10k run is `#[ignore]`d, not run. No fixture had an ID with two holders or a scoped REF, so `sr0_differential_over_two_holder_ids` and `sr0_differential_over_alias_forms` add them. M: ENDING without `EDGE`; a scoped REF's `path:` key dropped; end files left out; C(REF) one round (survives); a chain section not climbing; candidate `answers` edges dropped; fetch order. Correction: the planned example for one round, "an alias declared in a second file", cannot grow a second round through a CLI REF.
- [x] **AC-12 [default, store unit; cli unit] The lookups serve the read.** `EDGE`, `PARENT`, `FILE_BY_PATH` plan as a `SEARCH`, no `SCAN`; `holders`, `held`, `ending`, `parent` and the keyed decode cost at most 3x from 10 000 to 100 000 rows. **On every fixture node** the files decoded and read are the oracle's subset, with no whole decode, listing, resolver or graph after the freshness step; every `from_rows` `Edge` equals the whole graph's, and an ID with two holders ends whole (store `ac12_ids_with_two_holders_end_whole`). `FILE_BY_PATH` is in both the plan and the cost test. Gap: the 1k targets. M: the resolver over `indexed_input`; the subset widened by citing files' closures; `EDGE` without `f.path = ?2` (the cost test); the keyed decode by `decode_all`.
- [x] **SR-1 [default, cli unit] T5 under concurrent edits.** At stages (a), (b), (c): a holder's text, a dropped link, a renamed end ID, a deleted or mode-000 subset file. Each answers as a second read with no hook, one `update_paths` of exactly the mismatched paths, no attempt-1 message. **A hook that edits at every attempt**: two attempts, then the walk, within 10 s. M: no compare; `update`; the old snapshot; unbounded retries; attempt 1's messages.
- [x] **SR-2 [default, cli unit] Corrupted, stale and missing rows.** At stage (a): another or a NULL `files.blake3` retries once; a stamp of 15, a deleted worktree row, a corrupted edge or parent row (`covers`) walk. Read through the binary, as the golden: stamp 15 over emptied edge tables, a worktree never indexed, a `git worktree add` copy read first, a citation added outside the subset. M: `NotIndexed` as exit 2; a NULL hash as a match; the subset read run before the freshness step.
- [x] **SR-3 [default, store; cli unit] Whole reads keep a matching row's parse (Q7).** The projection contract holds on every configured fixture and the 1k tree; warm whole reads parse 0 files, one edited at stage (a) alone. M: no compare; hashes by position; `rebuild_parsed` without `summary`.
- [x] **SR-4 [ignored, eval release] Before and after.** Seed 1, base `2220d05` and head in one session; answers equal at every size, `nondeterministic` 0; medians in `scale-eval.md` "Baseline". At 100k, ms: `get_node` 665 -> 372, links 1 557 -> 373, `search` 382 -> 380, bundle 1 534 -> 376, `update_us` 376 -> 376, hub links 1 567 -> 398, hub bundle 1 542 -> 494; build 10.21 -> 9.96 s. A read now costs about `update_us`.
- [x] **F1 [form].** `pick`, `pick_reference` without `written`; clippy and fmt clean, no dependency; no `docs/`, MCP or http `src/` change; `check` 0 errors, 0 warnings.

## Out of scope

RM-0054's T1, T2, T4, Q1-Q6 and `update_us` with a generated index; subset reads for the other whole reads and the code judge (D7); core's `written` (D10); `catch_unwind`; corrupted edge rows beyond `covers` (R7); PARENTED.

## Implementation

Review: ACCEPT on iterations 1 (`src/`) and 2 (tests; a row ending outside the subset walks); iteration 3 changed comments.

| Module | What it does |
|---|---|
| core `check/{spec_graph,mod,resolve}.rs` | `from_rows`, `GraphRows`, `{Edge,Parent}Row`, `resolve_detached_reading` |
| core `check/bundle.rs`* | `bundle_linked`; `bundle_layers` split into `kept_targets`, `linked_to`, `layers_of`, verbatim |
| store `src/{read,edges,lib}.rs` | `snapshot`, `IndexSnapshot`, `Decoded`, `FILE_BY_PATH`, `EDGE`, `PARENT`; `Stored*` re-exported |
| CLI `subset.rs` | the attempts, `covers`, seams |
| CLI `show.rs`, `bundle.rs`, `lib.rs` | subset answers; the walk kept (`show_walk`, `bundle_walk`); D7 |
| CLI `corpus.rs`, `refs.rs`, `package.rs`*, `refresh.rs`* | Q7 (`row_matches`, `indexed_read`, `indexed_decoded`, `add_bytes`); `pick_reference`; `refresh_in` |
| eval `src/scale/mod.rs` | `other_us` |
| tests | CLI `tests/store_reads.rs`, `src/store_reads_tests.rs`; store `tests/store_reads.rs`, `src/edges.rs`, `tests/projection.rs`; eval `tests/store_reads_scale.rs`, `tests/scale_cli.rs` |

\* Outside the planned files. Deviations: the walk prints holders from `read_bytes`' bytes; `covers`; `children` within a file; any attempt store error walks. Not taken: `prepare_cached` (rusqlite's `cache` feature) and the canon's perf nits.
