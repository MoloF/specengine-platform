---
class: canon
tier: 2
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-11-04
---

# Scale eval: read cost on a seeded synthetic corpus

`specengine-eval scale` (eval `src/scale/`) works through each `--nodes` size in turn. For each it:
- generates a corpus;
- indexes it cold and checks it;
- times today's warm reads through the CLI library, as MCP's `get_node`, `search` and `get_context_bundle` call it (`crates/specengine-mcp/src/read.rs`);
- replays each step of those reads alone;
- computes W_after by `w`'s formula (`docs/canon/w-measurement.md` "W_after").

It measures and never judges. The bars are RM-0054's:
- warm reads under 1 s at 100 000 nodes;
- under 2x growth from 10 000;
- W_after's median at 10 000 within 10 % of 1 000.

No test asserts a time (ADR-0012). A synthetic corpus proves load, not usefulness.

## CLI

```
specengine-eval scale --out <scratch-dir> [--label <name>] [--nodes <n>[,<n>...]]
    [--seed <u64>] [--budget <n>] [--timeout <s>] [--corpus-only]
```

The flags:
- `--nodes`: default `1000,10000,100000`. Each value is 500 to 200 000, strictly ascending.
- `--seed`: default 1.
- `--budget`: default 10 000, 1 to 4294967295, as `w`.
- `--timeout`: default 600 s, over the whole run. The heavy run gives 3600.
- `--label`: default `synthetic`, one plain path component.
- `--corpus-only`: write the trees and `size.json` only, with no index, check or timing.

The corpus is generated, never read, so there is no `--pilot`, `--config` or `--scheme`.

Exit codes:
- **0** measured. An overrun prints `"timeout"`, and finished sizes keep their `size.json`.
- **2** refused, with nothing written:
  - a flag out of range;
  - `--nodes` empty, unsorted or repeated;
  - `--out` and this repository nested either way. The repository is `CARGO_MANIFEST_DIR/../..`, canonicalised; when it is absent, only the link checks run;
  - a symbolic link at `<out>/scale` or at `<out>/scale/<label>`;
  - clap's usage errors.
- **1** an I/O error, or a read answering an error or a `reason`. stderr names the step and the size.

## Corpus

`generate(seed, nodes)` builds every file in memory, and `write` writes them. It uses two xorshift64\* streams seeded through splitmix64, so seed 0 works. The READMEs come from the seed alone, so they are byte-equal at every size. The part files and the samples come from the seed and the size. Nothing else goes in: no clock, no hash-map order, no environment.

`n<N>/tree/` holds `specengine.toml`, `README.md` (Tier 0), `areas/a00/README.md` to `areas/a15/README.md` (Tier 1) and the part files `areas/aNN/gMMMM/P-XXXXXX.md`. The config:

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

**Layout:**
- Part file k lies in area `k mod 16` and group `floor(k / 16) / 100`, so a group holds at most 100 files.
- Each file is one `class: spec` document with 2..=6 `{#P-...}` sections, about five nodes per file.
- IDs run from `P-000001` in file order. The last file takes what remains, so there are N part nodes and N + 17 nodes in all.

**Tier 3:** one part document in five is `shipped`, with `shipped: 2026-01-01`.

**Hubs:**
- The first four part files are the hubs. Part files 5 to 8 name one hub each.
- After that, one file in ten names a random hub.
- Hub in-degree is uncapped, about files / 40.

**Links:**
- A document's `depends_on` holds 0..=2 earlier non-hub documents, so there is no cycle.
- Each section holds 0..=2 inline mentions.
- A non-hub node takes at most 16 incoming links. A draw on a full node is redrawn 8 times, then dropped.

**Text:**
- A syllable is one of `j k q v x z` followed by one of `a e i o u y`. Common words are 1 to 3 syllables.
- Titles are 2 to 4 words; paragraphs are 20 to 60 words for a document and 15 to 50 for a section; sentences run 6 to 14 words.
- Section j holds the 5-syllable rare word `floor(j / 8)`, in base 36. A rare word is therefore in at most 8 sections at any size.
- The READMEs hold one paragraph each (Tier 0 also lists the 16 areas). They have no ID and nothing that grows with N.

**Vocabulary:** a token is a run of `[A-Za-z0-9_-]` with its edge `-` stripped. Every token is one of:
- a syllable word;
- `P-\d{6}`, `a\d{2}`, `g\d{4}`, `2026-01-01` or digits;
- a front-matter or config key or value.

**Samples:**
- 20 W targets: live, non-hub part nodes. The first 3 are timed.
- 3 queries: rare words held by a live section.
- The hub: `P-000001`.

## Reads and steps

Each read is called once to warm up, then timed three times, and the best is kept. A call includes `render_text`.

| Key | Call | Its steps |
|---|---|---|
| `get_node` | `show`, no links | open, update |
| `get_node_links` | `show`, `links: true` | open, update |
| `search` | `search`, one word, limit 20 | open, update, fts |
| `get_context_bundle` | `bundle`, one ID, `--budget` | open, update, then fitting |

Since 12d `show` and `bundle` read a subset after the update (`spec-read-subset.md`); the other steps, still replayed, are the whole reads' cost (`reparse_us` without Q7).

The steps, each the best of three on the warm index:

| Key | What runs |
|---|---|
| `open_us` | `discover` + `open_index` |
| `walk_us` | `WorkingTree::new` + `Source::list` |
| `hash_us` | `Source::read` + `b3_hash` of every listed file |
| `update_us` | `IndexWriter::update`; it holds walk and hash. `update_parsed` is its max `parsed`, 0 when warm |
| `decode_us` | `SpecIndex::indexed_input` |
| `reparse_us` | the CLI's private `read_bytes` replayed: `Source::read`, core `parse`, `CheckFile::parsed` |
| `resolver_us` | `Resolver::new` |
| `graph_us` | `SpecGraph::new` over the re-parsed input |
| `fts_us` | `SpecIndex::search`, the first query |

`other_us.<read>` is `max(0, median_us - the sum of its steps)`. It holds locating, assembly and fitting, rendering, and noise.

## Output

stdout is the envelope `{measurement: "scale", label, versions, wall_ms, result}`. `result` holds `seed`, `budget {tokens, source}`, `corpus_only` and `sizes[]`. Each size holds:
- `nodes`;
- `corpus {files, nodes, bytes, links, tier3_files, degree_max, hubs, hub_degree {min, max}}`;
- `index {files, nodes, build_us}`;
- `check {errors, warnings, us}`;
- `reads.<read> {median_us, max_us}`, with `search` adding `hits_min` and `hits_max`;
- `growth.<read>`: the median over the previous size's, to two decimals, `null` first;
- `steps`;
- `other_us`;
- `w_after {median, p90, max, tier0, tier1_max}`, by nearest rank;
- `hub {degree, links_shown, get_node_links_us, get_context_bundle_us, w_after}`;
- `nondeterministic`: (read, target) pairs whose four answers differ.

With `--corpus-only`, a size holds `nodes` and `corpus` only. stdout holds numbers only, never an ID, word or path. stderr holds one summary line per size.

The detail goes under `<out>/scale/<label>/`, emptied first. Each size gets a `n<N>/` directory with `tree/`, `home/` (the data directory, so the database lives there) and `size.json`. `size.json` is written when the size ends and holds:
- the size's entry;
- `digest`: `b3:` over each file's path, NUL, bytes, NUL, in path order;
- `targets`, `queries` and `hubs`;
- `calls`: per read and target, the timings and the answers' BLAKE3;
- `w_after_bytes {tier1, bundle, total}`.

## Isolation

Every call takes:
- `Globals {root: tree}`;
- an `Env` whose `cwd` is the tree and whose home is the size's `home` directory, with `xdg_data_home` `None`;
- `GitEnv::new(tree, [PATH, HOME])`, `HOME` being that same `home` directory.

It never calls `Env::from_process` or `process_git`, so nothing is read or written under the process's `HOME` or the repository. The run deletes nothing outside `<out>/scale/<label>/`.

## Baseline

The numbers before 12c-12e: 2026-11-04, the owner's laptop, release, seed 1, budget 10 000, `c2847ca` plus the slice, `wall_ms` 130 786. A later slice compares re-runs at its base and head on one machine.

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

What the 100k run shows:
- **Reads are linear,** about 11x from 10k.
- **Three steps dominate a bundle.** Reparse is 49 % of it, update (hashing) 26 % and decode 15 %; graph is 8 %.
- **The corpus at 100k:** 20 015 files, 22.9 MB and 101 746 links. 3 973 files are Tier 3. The maximum in-degree is 15.
- **Index and check:** the build 9.43 s, the check 0.92 s, 0 findings.
- **Search hits:** 5 to 8 per query.
- **W_after:** the median is 2 210, 2 182 and 2 239 B at 1k, 10k and 100k, already within 10 %. At 100k the p90 is 2 695 and the max 2 936. Tier 0 is 1 021 B, and the largest Tier 1 is 700 B.
- **The hub at 100k:** in-degree 533, 441 links shown, `get_node_links` 1.64 s, bundle 1.62 s, W_after 2 646 B.

**After 12d** (`store-reads`), release, seed 1, base `2220d05` and head in one session:

| Median ms, base->head | 1k | 10k | 100k |
|---|---|---|---|
| `get_node` | 6.55 -> 4.21 | 59.1 -> 32.1 | 665 -> 372 |
| `get_node_links` | 14.6 -> 4.37 | 140 -> 32.5 | 1 557 -> 373 |
| `get_context_bundle` | 14.2 -> 4.54 | 137 -> 32.7 | 1 534 -> 376 |
| `update_us` | 3.80 -> 3.62 | 32.1 -> 31.7 | 376 -> 376 |

At 100k: hub links 1 567 -> 398, hub bundle 1 542 -> 494, `search` 382 -> 380, build 10.21 -> 9.96 s; `other_us` 0 for `get_node` and links, 0.34 for bundle. Answers equal at every size, `nondeterministic` 0. A read now costs about `update_us` (hashing 349): 12e's.

## Tests

The tests are in `tests/scale_cli.rs`:
- **SE-1:** one seed gives one tree. The digest is the documented hash, and the generator reads no clock, pid or environment.
- **SE-2:** the shape.
- **SE-3:** a clean check.
- **SE-4:** the closed vocabulary and no pilot name.
- **SE-5, SE-7, SE-8:** one debug run at 1k. SE-7 also scans the sources statically.
- **SE-6:** the refusals, the bounds themselves accepted, and `--timeout 0`.

`tests/scale_edges.rs` checks the index's edge tables (`spec-index-edges.md`) on the `--corpus-only` 1k tree: they equal core's resolution before and after 20 seeded edits, and the worktree dumps as a fresh index's.

`scale_baseline` is `#[ignore]` and runs the baseline: `cargo nextest run -p specengine-eval --release --test scale_cli --run-ignored ignored-only scale_baseline --no-capture`. Release runs grow `target.noindex`.

## Known limits

- **`links_shown` counts the hub's links before the 40 000-character cap.** It overstates what is printed above about 180 000 nodes.
- **The steps run low and `other_us` high.** The steps share one warm connection and one `WorkingTree`, while a read opens its own. The bias points the honest way: the residual never hides time.
- **Timing noise on a busy laptop can push `other_us` to 0.** At 100k, `search`'s steps summed past the read, so its `other_us` came out 0. When every residual is 0, SE-8 says it cannot tell a dropped step apart.
- **A failure's stderr quotes the engine's reason,** which may hold a synthetic ID.
- **The reused refusal (`layout::check_out`) calls the repository "the corpus".**
- **The in-degree cap never binds at seed 1.** Uncapped, the maximum is 6, 12 and 15 at 1k, 10k and 100k, so the cap is an unexercised guard and dropping it survives the tests.
- **`process_git` in place of the scratch `GitEnv` is caught only by SE-7's static scan.**
- **The `read_bytes` mirror drops the CLI's warnings.** A file that cannot be read keeps its decoded parse.
- **Not timed:** code reads, `--bindings`, flow, tree, graph and impact; cold reads after a start; concurrent readers; the JSON that MCP renders.
