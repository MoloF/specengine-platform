---
class: spec
status: draft
scope: [crates/specengine-eval]
ref: RM-0077; Phase 7 step 12b (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split and Q2, Q8 awaiting the owner's review
adrs: []
---

# scale-eval: a seeded synthetic corpus and today's read cost at 1 000, 10 000 and 100 000 nodes (RM-0077)

## Why

RM-0054's bars are numbers at 100 000 nodes, and nobody knows what today's reads cost there. Every warm read walks and hashes every file (`crates/specengine-cli/src/refresh.rs:116-128`, store `src/write.rs:126-159`) and decodes the whole index (`corpus.rs:37-50`). With links, and in a bundle, it also re-reads and re-parses every file (`corpus.rs:69-101`) and resolves every link (`show.rs:244-246`, `bundle.rs:398-400`). The figures we have are small or one-off: MCP's "Latency" at 83 documents and 3 000 candidates (`docs/canon/mcp-read.md`), and `impact_cost.rs`, which times a 10 000-node radius by spawning a process. 12c-12e each claim a speed-up, and each needs a number from before it to beat, split by step so the claim lands on the step it changes.

This slice measures; it changes no product code. Decided here, each awaiting the owner's review:
- **S1 Home.** It lives in the eval crate: `src/scale/mod.rs` holds the command, `src/scale/generate.rs` the generator. There is no new crate and no new dependency. The PRNG is an in-module xorshift64\*, the pattern of core `tests/check_rules.rs:1943` and CLI `tests/code_index_steps.rs:44`. BLAKE3 comes from `specengine_store::b3_hash` (store `src/lib.rs:274`).
- **S2 A pure generator.** `generate(seed, nodes) -> Synthetic` builds every file in memory, and `write(&Synthetic, tree)` writes them. There is no clock, no hash-map order and no environment, so one seed and one size give one byte sequence. The READMEs are drawn from a stream of their own, seeded by `seed` alone, so for one seed they are byte-equal at every size: W_after compares like with like.
- **S3 `--corpus-only`.** It writes the trees and nothing else. This keeps the structural tests fast. It also gives 12c and 12d "RM-0077's corpus" (RM-0078, RM-0079 "Done") without a timing run.
- **S4 Its own flags.** `ScaleArgs` carries no `--pilot`, `--config` or `--scheme`: the corpus is generated, never read.
- **S5 The library calls MCP makes.** It times `show`, `search` and `bundle` as `crates/specengine-mcp/src/read.rs:709-781` calls them, each answer rendered by `render_text`, as `w` does (`w.rs:924-984`). Each read gets one unrecorded warm call, then three timed calls, and keeps the best (Q2). Times are integer microseconds.
- **S6 The split needs no product hook.** Each step is replayed through the public API the read itself calls, on the same warm index: `discover`, `open_index`, `WorkingTree` and `Source`, `IndexWriter::update`, `SpecIndex::indexed_input`, core `parse`, `Resolver::new`, `SpecGraph::new`, `SpecIndex::search`. The one mirror is `corpus.rs`'s private `read_bytes`, a 30-line loop. What the steps do not cover is printed as a residual, so the split cannot hide time. No ADR.
- **S7 W_after by `w`'s formula.** It is Tier 0 + Tier 1 + `Bundle.bytes` at `--budget` (`docs/canon/w-measurement.md` "W_after"), over 20 targets per size. The statistics use `w.rs`'s nearest rank (`:1886-1891`).
- **S8 Reused guards.** These are `layout::check_out` (`layout.rs:229-248`, with the repository root in the corpus's place), `layout::clear` (`:597-607`), `harness::run_with_timeout` (`harness.rs:314-327`) and the measurement envelope (`main.rs:467-473`).
- **S9 It measures and never judges.** No test asserts a time; the bars are RM-0054's, and nothing is blocked by a number (ADR-0012). The heavy run is a named `#[ignore]` test in release.
- **S10 Canon at ship is a new page,** `docs/canon/scale-eval.md`. The contract is about 5 KB, and `w-measurement.md` has 1 766 B free.

## Description and interactions

`specengine-eval scale` runs these steps, for each `--nodes` size N in ascending order, inside one `--timeout`:
1. Generate the corpus and write `tree/`.
2. Build its index cold, through `specengine_cli::index` (`IndexRequest {full: false}`), and time the build.
3. Check it with `specengine_cli::check` (`CheckedTree::WorkingTree`, no baseline) and time the check.
4. Time the four reads on three targets, plus the hub's two reads.
5. Time each step of the split.
6. Bundle the 20 W targets and the hub for W_after.
7. Write `size.json`.

The run then prints the envelope. `--corpus-only` stops after step 1.

Every call takes `Globals {root: Some(tree), config: None}` and `Env {cwd: tree, home: Some(<n>/home), xdg_data_home: None}`, as `w` does (`w.rs:661-669`). It never calls `Env::from_process`. Git's environment is `GitEnv::new(tree, [PATH, HOME = <n>/home])`, not `process_git`, so no child process sees the real `HOME`.

| Key | Call (as MCP) | Steps it takes today |
|---|---|---|
| `get_node` | `show`; `links`, `bindings` and `archive` false (`read.rs:709-728`) | open, update, decode, resolver (`show.rs:213-217`) |
| `get_node_links` | the same with `links: true` | open, update, decode, reparse, resolver, graph (`show.rs:244-246`) |
| `search` | `search`; `terms: [word]`, no kinds, default limit 20, no archive (`read.rs:738-755`) | open, update, fts (`search.rs:134-146`) |
| `get_context_bundle` | `bundle`; `references: [ID]`, `budget: Some(--budget)` (`read.rs:765-781`) | open, update, decode, reparse, graph (`bundle.rs:398-400`), then fitting |

The steps, each the best of three, are timed with the same `Env` after the reads:

| Key | What runs |
|---|---|
| `open_us` | `specengine_cli::discover` + `open_index` (`location.rs:79-85`) |
| `walk_us` | `WorkingTree::new` + `Source::list` (store `src/source.rs:53-63`): the listing and `lstat`s alone |
| `hash_us` | every listed file read by `Source::read` and hashed by `b3_hash` |
| `update_us` | `IndexWriter::update` on the warm index: walk, read and hash, the `Immediate` transaction (store `src/lib.rs:559-575`). `update_parsed` is its `parsed`, which is 0 when the index is warm |
| `decode_us` | `SpecIndex::indexed_input` (store `src/lib.rs:539-553`) |
| `reparse_us` | every file with a parse, read by `Source::read` and parsed by core `parse` (`src/lib.rs:191`) into `CheckFile::parsed` (`check/input.rs:42`): `read_bytes` replayed |
| `resolver_us` | `Resolver::new` (`check/resolve.rs:90`) over the decoded input |
| `graph_us` | `SpecGraph::new` (`check/spec_graph.rs:277`) over the re-parsed input |
| `fts_us` | `SpecIndex::search` for the first query: limit 20, no kinds, no archive |

`other_us.<read>` is `max(0, reads.<read>.median_us - Σ its steps' _us)`, summed over the table's third column (`update_us` covers `walk_us` and `hash_us`, which are not added again). It holds discovery's share, locating, assembly and fitting, rendering, and noise.

For 12c-12e: each re-runs `scale` at its base commit and at its result on one machine, and compares the two runs. The 12b figures in "Implementation" are a reference only, because a laptop's numbers from different days do not compare.

## Data

### CLI

```
specengine-eval scale --out <scratch-dir> [--label <name>] [--nodes <n>[,<n>...]]
    [--seed <u64>] [--budget <n>] [--timeout <s>] [--corpus-only]
```

- `--nodes`: default `1000,10000,100000`. Each value is 500 to 200 000, and the values are strictly ascending.
- `--seed`: default 1.
- `--budget`: as `w`, 1 to 4294967295, default 10 000.
- `--timeout`: default 600 s, and it bounds the whole run, generation included. The heavy run passes 3600.
- `--label`: default `synthetic`, one plain path component (`harness.rs:199`).

Exit codes:
- **0**: measured. On an overrun the result is `"timeout"`, and finished sizes keep their `size.json`; the process exits, which ends the worker (`harness.rs:311-313`).
- **2**: refused, and nothing is written. The causes:
  - a flag outside its range, `--nodes` empty, unsorted or repeated;
  - `--out` and the repository nested either way. The repository is `CARGO_MANIFEST_DIR/../..` canonicalised, as `fixture_dir` locates it; when that path is absent, nothing is compared;
  - a symbolic link at `<out>/scale` or at `<out>/scale/<label>`;
  - clap's usage errors (`--pilot`).
- **1**: an I/O error, or a read answering an error or a `reason`. stderr names the read, the size and the target's ordinal.

### The generated tree

`<out>/scale/<label>/n<N>/tree/`:
- `specengine.toml`;
- `README.md` (Tier 0);
- `areas/a00/README.md` to `areas/a15/README.md` (Tier 1, 16 areas);
- part files at `areas/aNN/gMMMM/P-XXXXXX.md`.

`specengine.toml`, byte for byte:

```toml
[project]
slug = "synthetic"

[paths]
roots = ["README.md", "areas"]
tier0 = "README.md"
tier1_name = "README.md"

[ids]
P = { kind = "node", width = 6 }
```

A part file:

```markdown
---
class: spec
id: P-000101
status: draft
scope: [a03]
links:
  depends_on: [P-000006, P-000031]
---

# Kexo qavi zoju

Vaxe joqi kuzo xeva P-000042 jiky. Qo zavexu kiko vyqe.

## Qazo vixe {#P-000102}

Zoki vaqexojuky jexa P-000310 kuvo qi. Xavo zejy.
```

Every count below is drawn from the size's stream.

**Layout:**
- Part file k (0, 1, ...) lies in area `k mod 16` and group `floor(k / 16) / 100`, so a group directory holds at most 100 files.
- A file is one document plus s sections, s uniform in 2..=6, which makes about five nodes per file.
- IDs run from `P-000001` in file order: the document's, then its sections'.
- The last file takes only the count that remains, so there are exactly N part nodes, and the corpus has N + 17 nodes.

**Status:** one part document in five is `status: shipped` with `shipped: 2026-01-01` (Tier 3), never a hub. The rest are `draft`.

**Hubs:** the documents of the first four part files. Part files 5 to 8 each name one hub in turn, so every hub has at least one incoming link. After that, one part file in ten names one hub, chosen at random, in its document paragraph. Hub in-degree is not capped, so it grows with N, about files / 40.

**Links:**
- `depends_on` holds 0..=2 IDs of earlier non-hub part documents, so the graph has no cycle.
- Each section holds 0..=2 inline mentions of any non-hub part node other than itself. The text is laid out first, so a mention may point forward.
- Every non-hub node has at most **16** incoming links, `depends_on` and mentions together. A draw that lands on a full node is redrawn up to 8 times, then dropped.

**Text:**
- A title is 2 to 4 words, the first capitalised.
- A document paragraph is 20 to 60 words, and a section paragraph 15 to 50.
- Sentences run 6 to 14 words, and each ends with `.`.
- A syllable is one of `j k q v x z` followed by one of `a e i o u y`.
- A common word is 1 to 3 syllables.
- A **rare word** is exactly 5 syllables, the bijective base-36 code of its number. Section j, counted over all part sections in ID order, holds rare word `floor(j / 8)`. So a rare word is in at most 8 sections at any N, and since common words are shorter, no common word contains one.

**READMEs** are drawn first, from the seed's own stream:
- Tier 0 has `class: canon`, `tier: 0`, `owner: owner` and `reviewed: 2026-01-01`. Its body is an H1, a paragraph of 40 to 80 words, and 16 items `- [Title](areas/aNN/README.md)`.
- Tier 1 adds `scope: [aNN]` and has `tier: 1`. Its body is an H1 and a paragraph of 60 to 120 words.
- Neither holds an ID or anything that grows with N.

**The closed vocabulary.** A token is a maximal run of `[A-Za-z0-9_-]`, with its edge `-` stripped. Every token of a tree is one of:
- a syllable word, case-insensitive `^(?:[jkqvxz][aeiouy])+$`;
- an ID, `^P-\d{6}$`;
- an area or group name, `^a\d{2}$` or `^g\d{4}$`;
- the date `2026-01-01`, or a digit run;
- one of `project slug synthetic paths roots README md areas tier0 tier1_name ids P kind node width class canon spec tier owner reviewed scope id status draft shipped links depends_on`.

### Samples

The size's stream draws:
- **W targets:** 20 live, non-hub part nodes (documents or sections), without replacement. The first 3 are the **timed targets**.
- **Queries:** 3 rare words, each held by at least one live section.
- **The hub:** `P-000001`, measured apart.

### stdout

The envelope is `{measurement: "scale", label, versions, wall_ms, result}` (`main.rs:467-473`), with `result` exactly as below. It holds numbers, `null`, `budget.source` (`flag` | `default`) and `corpus_only`, and never an ID, word or path.

```json
{"seed":1,"budget":{"tokens":10000,"source":"default"},"corpus_only":false,
"sizes":[{"nodes":1000,
 "corpus":{"files":0,"nodes":1017,"bytes":0,"links":0,"tier3_files":0,"degree_max":0,"hubs":4,"hub_degree":{"min":0,"max":0}},
 "index":{"files":0,"nodes":1017,"build_us":0},
 "check":{"errors":0,"warnings":0,"us":0},
 "reads":{"get_node":{"median_us":0,"max_us":0},"get_node_links":{"median_us":0,"max_us":0},
  "search":{"median_us":0,"max_us":0,"hits_min":0,"hits_max":0},"get_context_bundle":{"median_us":0,"max_us":0}},
 "growth":{"get_node":null,"get_node_links":null,"search":null,"get_context_bundle":null},
 "steps":{"open_us":0,"walk_us":0,"hash_us":0,"update_us":0,"update_parsed":0,"decode_us":0,"reparse_us":0,"resolver_us":0,"graph_us":0,"fts_us":0},
 "other_us":{"get_node":0,"get_node_links":0,"search":0,"get_context_bundle":0},
 "w_after":{"median":0,"p90":0,"max":0,"tier0":0,"tier1_max":0},
 "hub":{"degree":0,"links_shown":0,"get_node_links_us":0,"get_context_bundle_us":0,"w_after":0},
 "nondeterministic":0}]}
```

The fields:
- `reads.*.median_us` and `max_us` run over the timed targets (or the queries), each taken at its best of three.
- `hits_*` are the queries' hit counts.
- `growth.<read>` is `median_us` over the previous size's, to two decimals. It is `null` at the first size or over a 0.
- `nondeterministic` counts (read, target) pairs whose four answers rendered differently.
- `hub.links_shown` is the number of links its `get_node_links` answer lists.

With `--corpus-only`, a size holds `nodes` and `corpus` only.

### Detail under `<out>/scale/<label>/`

The directory is emptied first, then each size gets `n<N>/`:
- `tree/`;
- `home/`, the index database under the data directory `location.rs` derives (absent with `--corpus-only`);
- `size.json`, written when the size ends.

`size.json` holds the size's entry plus:
- `digest`: `b3:` over each file's path, NUL, bytes, NUL, in path byte order;
- `targets`, `queries` and `hubs`;
- `calls`: per read and target, the three timings and the four answers' BLAKE3;
- `w_after_bytes`: per target, `{tier1, bundle, total}`.

## Rules and edge cases

- WHEN the same `--seed` and N are given, the tree SHALL be byte-identical. WHEN the seed differs, `digest` SHALL differ.
- WHEN a size's warm `update` parses any file, `update_parsed` SHALL report it. The run goes on: the number is a finding.
- WHEN a read answers an error or a `reason`, the run SHALL fail (exit 1). A synthetic corpus answers every read, so a failure is a generator bug or an engine bug.
- WHEN the check finds anything, the size SHALL report its counts and go on.
- The run SHALL write only under `<out>/scale/<label>/`, and SHALL read and write nothing under `HOME` or the repository.
- Figures are taken in release. A debug run checks shape only.
- The steps replay the reads and do not account for them: noise can make `other_us` 0.

## Acceptance criteria

The tests live in eval `tests/scale_cli.rs` and run the binary (`CARGO_BIN_EXE_specengine-eval`) under scratch directories in `std::env::temp_dir()`. M names the mutations that must turn each red.

- [ ] SE-1 [default, eval] Determinism.
  - `scale --corpus-only --nodes 1000,2000 --seed 7` is run twice into two `--out`s.
  - Both `n1000/tree/` directories are equal path for path and byte for byte, and so are their `digest`s.
  - `--seed 8` gives another `digest`.
  - Within one run, `README.md` and every `areas/*/README.md` are byte-equal between `n1000` and `n2000`.
  - M: the seed from the clock or the process id; `HashMap` order in generation; the READMEs drawn from the size's stream.
- [ ] SE-2 [default, eval] Shape, scanned by the test on SE-1's `n1000` tree.
  - `specengine.toml` is as in "Data", byte for byte.
  - There are exactly 1 000 part nodes (documents and `{#P-...}` sections) and 1 017 in all.
  - Every part file but the last has 2..=6 sections, and the mean is 4.5 to 5.5 nodes per file.
  - No group directory holds more than 100 files.
  - Every non-hub ID has at most 16 incoming links (`depends_on` items and inline mentions), and every `depends_on` names a smaller ID.
  - Each hub has at least 1 incoming link.
  - Each rare word appears in at most 8 sections.
  - M: the in-degree cap dropped; `depends_on` to a later ID.
- [ ] SE-3 [default, eval] Clean. The test calls `specengine_cli::check` on SE-1's `n1000` tree with a scratch `HOME`, and gets 0 errors and 0 warnings. M: a mention of an unissued ID (`mention-dangling`); a back edge in `depends_on` (`depends-cycle`); a Tier 1 README without `owner` (`key-missing`).
- [ ] SE-4 [default, eval] No domain or pilot word.
  - Every token of the `n1000` tree is in the closed vocabulary.
  - With `SPECENGINE_PILOT_A` or `_B` set, neither corpus directory's name occurs, case-insensitively, in any file. Unset, the test says so and passes that half (`anonymity.rs:171-199`).
  - M: one English word in a title template; a pilot name planted, its variable set.
- [ ] SE-5 [default, eval] One debug run at 1k: `scale --nodes 1000`, exit 0.
  - stdout is one envelope, `measurement` `"scale"`, holding every key of "Data". Every leaf of `result` is a number or `null`, except `budget.source` and `corpus_only`.
  - Neither stdout nor stderr holds a `P-` ID, a `/`, or any word from `size.json`'s `queries`.
  - `index.nodes` equals `corpus.nodes`, which is 1 017.
  - `check` is 0 and 0, `update_parsed` is 0 and `nondeterministic` is 0.
  - `hits_min` is at least 1 and `hits_max` at most 16.
  - `growth` is all `null`.
  - `size.json` lists 20 targets, 3 queries and 4 hubs. The test reads the files: each target's `total` equals the bytes of `README.md` plus its area's README plus its `bundle`.
  - M: a target ID printed; the cold build timed as a warm read (`update_parsed` > 0); Tier 1 counted twice.
- [ ] SE-6 [default, eval] Refusals, nothing written.
  - Each of these exits 2 and leaves `--out` absent or as it was:
    - `--out <repository>/target.noindex/scale-refused`;
    - a symbolic link at `<out>/scale`;
    - `--nodes 499`, `200001`, `2000,1000`, `1000,1000` or `""`;
    - `--budget 0`;
    - `--label ..`;
    - `--pilot x`.
  - `--timeout 0` exits 0, with `result` `"timeout"`.
  - M: the repository guard dropped, or run after the first write.
- [ ] SE-7 [default, eval] `HOME` untouched. SE-5's child runs with `HOME` set to an empty scratch directory, which stays empty. The database lies under `n1000/home/`. M: `Env::from_process`; `process_git` in place of the scratch `GitEnv`.
- [ ] SE-8 [default, eval] The split adds up. From SE-5's numbers, the test recomputes each `other_us.<read>` per the "Description" table and gets the printed value. M: `decode_us` left out of `get_node`'s sum; `walk_us` added on top of `update_us`.
- [ ] SE-9 [manual, release] The baseline, `scale_baseline` (`#[ignore]`):
  - The command is `cargo nextest run -p specengine-eval --release --test scale_cli --run-ignored ignored-only scale_baseline --no-capture`.
  - It runs the default sizes with `--timeout 3600`, and the result must not be `"timeout"`.
  - Each size passes SE-5's structural checks, and `degree_max` is at most 16.
  - It prints the envelope. "Implementation" records reads, growth, steps, `other_us`, W_after, hub, build and check per size, with the commit.
  - It asserts no time.
- [ ] F1 — form.
  - `cargo clippy -p specengine-eval --all-targets` and fmt are clean.
  - Every existing eval test passes unchanged, the factored envelope included. `build_graph.rs` and `doc_pointers.rs` are green.
  - The implementation touches no `docs/`, fixture or `Cargo.toml`.
  - `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

- Any product change: the store, core, the CLI, MCP and http (RM-0078, RM-0079, RM-0054).
- Every bar (RM-0054). That includes the local stat walk's own bar and a hub's trusted path: `scale` times the local read only.
- Code index reads, `--bindings`, flow, tree, graph and impact (the analysis's step 3; 12d's).
- Cold reads after a start, concurrent readers, and the JSON half MCP renders.
- Queries with unbounded hits, realism of the text, and pilots as corpora (`scale` reads none).
- 12c's and 12d's equality tests on this corpus: they run `--corpus-only`, and their specs decide the rest.

## Implementation

To fill in after implementation. Code comments cite this spec's "Acceptance criteria" (it survives compaction) or `w-measurement.md` "W_after"; the new canon page exists only from the ship commit, so code does not cite it before then (`doc_pointers.rs`).

### Files in scope

| Module | What it changes |
|---|---|
| eval `src/main.rs` | `Measurement::Scale(ScaleArgs)`; `ScaleArgs` (S4); the envelope printing factored out of `measure_with` (`:505-575`) so `scale` prints the same envelope with no change to the others |
| eval `src/scale/mod.rs` (new) | flags and guards (`layout::check_out` against the repository root, `layout::clear`); per size: write, index, check, reads, steps, W_after, hub, `size.json` (`census::write_json`, `:276`); the aggregates |
| eval `src/scale/generate.rs` (new) | xorshift64\* seeded through splitmix64 (so seed 0 works); syllables; layout; capped links; hubs; rare words; `generate`, `write` |
| eval `src/harness.rs`, `src/w.rs` | `is_single_component` and `nearest_rank` become `pub(crate)`; a shared repository-root helper beside `fixture_dir` |
| eval `tests/scale_cli.rs` (new) | SE-1 to SE-8; SE-9 `#[ignore]` |

The rust-developer writes `src/`, and the test-engineer writes `tests/scale_cli.rs`. Both work against "Data", so they can run side by side. No `Cargo.toml` changes. Release runs grow `target.noindex`: clean it at the slice's end. At 100 000 nodes the tree is tens of MB and its index, with trigram FTS, hundreds; the run deletes nothing outside `<out>/scale/<label>/`.

### Canon at ship

Nothing below is written before the ship commit. Free bytes are as of this spec, and no cap is raised.

- **New `docs/canon/scale-eval.md`** (Tier 2, scope `crates/specengine-eval`, 12 288 B cap). It covers the CLI, the corpus, the timing, the split, W_after, stdout and detail, the guards, and the known limits. Its index line, about 140 B, goes to `docs/index-canon.md`, which has 218 B free. If that does not fit, add a new live shard (`docs/README.md` "Compaction").
- **The eval README** (195 B free):
  - The CLI contract names `scale` and `--nodes`, `--seed`, `--corpus-only`.
  - One row: `scale`, read cost on a seeded synthetic corpus per size (the new page); its fixture "none: generated"; its detail `n<N>/{tree,home}/`, `size.json`.
  - The tests line names `scale_cli.rs`.
  - To fit, the `ast-hash` row's field list ("parse errors, `cannot_verify`, `qpath` ambiguity, units, target sources") becomes a pointer to `docs/canon/code-identity.md` "Eval outputs", which already holds those fields.
- **`docs/canon/w-measurement.md` "W_after"** (1 766 B free) gains one sentence: `scale` applies this formula to a synthetic corpus, with a link to the new page.
- **Plan:** RM-0077 shipped and compressed; 08's row 12b; the state line in `CLAUDE.md`; this file compressed to "Why", the criteria and the baseline.
- **Unchanged:** `architecture.md`, every ADR, and the product crates' READMEs.
