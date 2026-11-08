---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-eval]
ref: RM-0077; Phase 7 step 12b (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split and Q2, Q8 awaiting the owner's review; one iteration
adrs: []
---

# scale-eval: a seeded synthetic corpus and today's read cost at 1 000, 10 000 and 100 000 nodes (RM-0077)

## Why

RM-0054's bars are numbers at 100 000 nodes, and nobody knew what today's reads cost there. Every warm read walks and hashes every file and decodes the whole index. With links, and in a bundle, it also re-reads and re-parses every file and resolves every link. 12c-12e each claim a speed-up, and each needs a number from before it, split by step, so the claim lands on the step it changes. This slice measures and changes no product code.

Decided here, each awaiting the owner's review:
- **S1, location:** the eval crate, with no new crate and no new dependency (an in-module xorshift64\*, and store `b3_hash` for BLAKE3).
- **S2, a pure generator:** the READMEs are seeded from the seed alone.
- **S3, `--corpus-only`:** fast structural tests, and the corpus 12c and 12d need.
- **S4, flags of its own:** no `--pilot`, `--config` or `--scheme`.
- **S5, timing:** the library calls MCP makes, best of three after one warm-up call.
- **S6, the per-step split without a product hook:** the steps are replayed through the public API, the leftover printed. No ADR.
- **S7, W_after:** `w`'s formula.
- **S8, reused guards:** `w`'s and `layout`'s.
- **S9, measure only:** no time is ever asserted.
- **S10, canon:** a new page.

## Description and interactions

How it works now: `docs/canon/scale-eval.md` (the CLI, the corpus, the reads and steps, the output, isolation, the baseline, the known limits). The eval README lists it; `docs/canon/w-measurement.md` "W_after" points to it. No store, core, CLI, MCP or http file changed.

## Acceptance criteria

M names the mutations that must turn each red. Every named mutation turned red but one, noted at SE-2.

- [x] SE-1 [default, eval] Determinism.
  - Two runs of `--corpus-only --nodes 1000,2000 --seed 7` give equal trees and digests. `--seed 8` gives another digest.
  - The READMEs are byte-equal across sizes, and the digest is the documented hash.
  - M: the clock or pid as the seed; `HashMap` order; the READMEs drawn from the size's stream.
- [x] SE-2 [default, eval] Shape of the 1k tree.
  - The config is byte for byte as documented, with 1 000 part nodes and 1 017 in all.
  - 2..=6 sections per file, a mean of 4.5 to 5.5 nodes, at most 100 files per group.
  - Non-hub in-degree is at most 16, and every `depends_on` names a smaller ID. Each hub has at least 1 incoming link. A rare word is in at most 8 sections.
  - M: a `depends_on` to a later ID was killed.
  - M: dropping the cap **survives**. The cap never binds at seed 1: uncapped, the maximum in-degree is 6, 12 and 15 at 1k, 10k and 100k.
- [x] SE-3 [default, eval] Clean: `specengine_cli::check` on the 1k tree gives 0 errors and 0 warnings. M: a dangling mention; a `depends_on` cycle; a Tier 1 README without `owner`.
- [x] SE-4 [default, eval] Every token is in the closed vocabulary, and no pilot corpus name occurs when `SPECENGINE_PILOT_A` or `_B` is set. M: an English word in a title; a planted pilot name.
- [x] SE-5 [default, eval] One debug run at 1k.
  - The envelope holds every documented key, with numbers only except two leaves. stdout and stderr hold no ID, path or query word.
  - The index and the corpus both have 1 017 nodes; the check is clean; `update_parsed` is 0 and `nondeterministic` 0.
  - Search hits are 1 to 16, and `growth` is all `null`.
  - The test recomputes each target's W_after from the files.
  - M: an ID printed; the cold build timed as a warm read; Tier 1 counted twice.
- [x] SE-6 [default, eval] Refusals write nothing.
  - These exit 2 and leave `--out` as it was: `--out` inside the repository, a symbolic link at `<out>/scale` or at its label, bad `--nodes`, `--budget 0`, `--label ..`, `--pilot`.
  - The bounds themselves are accepted. `--timeout 0` gives `"timeout"`, exit 0.
  - M: the repository guard dropped, or run late.
- [x] SE-7 [default, eval] The child's empty `HOME` stays empty, and the database lies under the size's `home` directory.
  - M: `Env::from_process` (killed at run time); `process_git` (killed by the static scan of the sources only).
- [x] SE-8 [default, eval] Each `other_us` is recomputed from the printed numbers. M: a step dropped from a read's sum; `walk_us` added on top of `update_us`.
- [x] SE-9 [manual, release] `scale_baseline` (`#[ignore]`) ran once: the three sizes, `--timeout 3600`, 131 s. It made SE-5's structural checks at each size and asserted no time. The figures: "Baseline" below.
- [x] F1 — form.
  - Clippy and fmt are clean, and 4 924 of 4 924 workspace tests pass. `build_graph.rs` and `doc_pointers.rs` pass.
  - No `docs/`, fixture or `Cargo.toml` change.
  - `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

Out of scope here:
- Product changes: RM-0078, RM-0079, RM-0054.
- Every bar, the stat walk's own bar, and a hub's trusted path: RM-0054.
- Code, flow, tree, graph and impact reads.
- Cold reads after a start, concurrent readers, and MCP's JSON rendering.
- Queries with unbounded hits, and pilots as corpora.

## Implementation

Review accepted it on the first iteration, with two minors and some nits. All of them went into the canon page's "Known limits".

The developer took 15 readings of the spec, all accepted. Those that shape behaviour:
- With the repository absent, only the link refusals run (`refuse_links`).
- `--out` is created before the timer starts, and `<out>/scale/<label>/` is emptied inside the timed run.
- The steps share one warm handle and one `WorkingTree`. Decode, resolver, reparse and graph are timed in one loop of three, each step keeping its minimum.
- `update_parsed` is the maximum over the three timed updates.
- The `read_bytes` mirror drops the CLI's warnings.
- `black_box` keeps each result past its timer.
- The hub's reads are timed like a target's.
- stderr prints one summary line per size.

At 100 000 nodes the reads take 705, 1 638, 424 and 1 613 ms (`get_node`, with links, `search`, the bundle), about 11x their 10 000-node figures. A bundle is 49 % reparse, 26 % update (hashing), 15 % decode and 8 % graph. W_after's median is 2 210, 2 182 and 2 239 B for 1 000, 10 000 and 100 000.

| Module | What it does |
|---|---|
| eval `src/scale/generate.rs` | `generate` (`:187`), `write` (`:167`), `Synthetic::digest` (`:153`); the constants (`:26-94`); `Rng` and `splitmix64` (`:243`, `:283`); `Links` with `draw_capped` (`:339`, `:437`); `Samples` (`:455`); `rare_word` and the paragraph, title and syllable helpers |
| eval `src/scale/mod.rs` | `prepare` (`:103`: the label, `parse_nodes` `:149`, the budget, `layout::check_out` against `harness::repository_root`, `refuse_links` `:180`); `run` (`:358`); `measure_size` (`:382`); `Session` (`:484`, the scratch `Env` and `GitEnv`); `Measuring` (`:618`) with `index`, `check`, `reads`, `steps` (`:833`) and `w_after` (`:940`); `read_bytes` (`:1036`); `other_us` (`:1076`); `growth` (`:1094`) |
| eval `src/main.rs` | `Measurement::Scale`, `ScaleArgs`; `run_measured`, the timeout and the envelope factored out of `measure_with` |
| eval `src/harness.rs` | `repository_dir`, which `fixture_dir` now uses, and `repository_root`, canonicalised; `is_single_component` becomes `pub(crate)` |
| eval `src/w.rs` | `nearest_rank` becomes `pub(crate)` |
| eval `tests/scale_cli.rs` | SE-1 to SE-8 in 15 tests; `scale_baseline` `#[ignore]` (SE-9) |

### Baseline

The numbers before 12c-12e, on the corpus with no flow shape (moved here from the canon page by `flow-scope`, whose run replaced them there): 2026-11-04, the owner's laptop, release, seed 1, budget 10 000, `c2847ca` plus the slice, `wall_ms` 130 786.

| Read, median ms (max) | 1k | 10k | 100k | growth 10k to 100k |
|---|---|---|---|---|
| `get_node` | 7.06 (7.07) | 64.2 (64.5) | 705 (708) | 10.99 |
| `get_node_links` | 15.7 (15.7) | 150 (150) | 1 638 (1 649) | 10.93 |
| `search` | 5.23 (5.35) | 38.6 (38.8) | 424 (426) | 10.98 |
| `get_context_bundle` | 15.6 (15.7) | 149 (149) | 1 613 (1 615) | 10.83 |

| Step, ms | 1k | 10k | 100k |
|---|---|---|---|
| open | 0.36 | 0.37 | 0.38 |
| walk | 0.56 | 1.60 | 12.4 |
| hash | 3.54 | 34.1 | 393 |
| update | 4.32 | 36.7 | 421 |
| decode | 1.88 | 22.4 | 249 |
| reparse | 7.80 | 75.6 | 798 |
| resolver | 0.11 | 1.42 | 22.0 |
| graph | 0.71 | 9.02 | 128 |
| fts | 0.44 | 1.03 | 6.08 |
| other: node, links, search, bundle | 0.38, 0.51, 0.11, 0.54 | 3.28, 4.36, 0.50, 4.81 | 13.4, 20.3, 0, 17.6 |

At 100k: reads linear, about 11x from 10k; a bundle's reparse 49 %, update (hashing) 26 %, decode 15 %, graph 8 %; 20 015 files, 22.9 MB, 101 746 links, 3 973 Tier 3, in-degree at most 15; build 9.43 s, check 0.92 s, 0 findings; 5 to 8 hits per query. W_after's median is 2 210, 2 182 and 2 239 B at 1k, 10k and 100k, within 10 % already; at 100k the p90 2 695, the max 2 936, Tier 0 1 021 B, the largest Tier 1 700 B. The hub at 100k: in-degree 533, 441 links shown, `get_node_links` 1.64 s, bundle 1.62 s, W_after 2 646 B.

12d's before and after: `docs/features/store-reads.md` SR-4. **After 12e** (`read-trust`; base `7c0ca35`, 12d's head, and head in one session), 100k, trusted: `get_node` 0.55, links 0.85, `search` 6.5, bundle 2.4 ms, hub 23.7 and 124; walking 43-50; `update_us` 41.9, none hashed; growth 1.00 (`read-trust` AC-13).
