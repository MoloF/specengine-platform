---
class: canon
tier: 2
scope: [crates/specengine-eval]
owner: owner
reviewed: 2026-11-12
---

# Scale eval: read cost on a seeded synthetic corpus

`specengine-eval scale` (eval `src/scale/`) takes each `--nodes` size in turn and:
- generates a corpus;
- indexes it cold and checks it;
- times today's warm reads through the CLI library, as MCP's `get_node`, `search` and `get_context_bundle` call it (mcp `read.rs`), five whole reads, of `get_flow`, `get_trace` and `get_tree`, `similar`, a create's probes, with their recall (`proposal-similar.md` "Measured"), and `collisions` with its planted twins (`collision-pass.md` "Measured");
- replays each step of the first four alone;
- computes W_after by `w`'s formula (`docs/canon/w-measurement.md` "W_after").

It measures and never judges. RM-0054's bars, for the first four: warm reads under 1 s at 100 000 nodes; under 2x growth from 10 000, a median under 10 ms counted as 10 ms (`growth_trusted`); W_after's median at 10 000 within 10 % of 1 000.

No test asserts a time (ADR-0012). A synthetic corpus proves load, not usefulness.

## CLI

```
specengine-eval scale --out <scratch-dir> [--label <name>] [--nodes <n>[,<n>...]]
    [--seed <u64>] [--budget <n>] [--timeout <s>] [--corpus-only]
```

Defaults and bounds: `--nodes` `1000,10000,100000`, each 500 to 200 000, strictly ascending; `--seed` 1; `--budget` 10 000, 1 to 4294967295, as `w`; `--timeout` 600 s over the whole run (the heavy run gives 3600); `--label` `synthetic`, one plain path component. `--corpus-only` writes the trees and `size.json` only: no index, check or timing.

The corpus is generated, never read: no `--pilot`, `--config` or `--scheme`.

Exit codes:
- **0** measured; an overrun prints `"timeout"`, finished sizes keeping their `size.json`.
- **2** refused, nothing written: a flag out of range; `--nodes` empty, unsorted or repeated; `--out` and this repository (`CARGO_MANIFEST_DIR/../..`, canonicalised; absent: only the link checks) nested either way; a symbolic link at `<out>/scale` or `<out>/scale/<label>`; clap's usage errors.
- **1** an I/O error, or a read answering an error or a `reason`. stderr names the step and the size.

## Corpus

`generate(seed, nodes)` builds every file in memory, `write` writes them: two xorshift64\* streams seeded through splitmix64, so seed 0 works. The READMEs, then the stage words, come from the seed alone, so they are byte-equal at every size. The part files and the samples come from the seed and the size. Nothing else goes in: no clock, no hash-map order, no environment.

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
- IDs run from `P-000001` in file order; the last file takes what remains: N part nodes, N + 17 in all.

**Tier 3:** one part document in five is `shipped`, with `shipped: 2026-01-01`.

**Flow:** a directory `areas/aNN/gMMMM/` is one chain: its file of least k the head, every other part document naming it as `parent:` and following file k - 16. A part document's `stage:` is word `floor(k / 16) mod 8` of eight two-syllable words, distinct, redrawn on a repeat; a stage crosses every directory. Tier 3 heads and steps, left out of a flow read, break chains: Top at 1k is 27 nodes, 11 edges.

**Hubs:** the first four part files. Files 5 to 8 name one each, then one file in ten a random one; a hub's in-degree is uncapped, about files / 40.

**Links**, drawn: a document's `depends_on` holds 0..=2 earlier non-hub documents, so no cycle; a section 0..=2 inline mentions. A non-hub node takes at most 16 of them, a draw on a full node redrawn 8 times, then dropped. `corpus.links` and `degree_max` count these and the hub names, never `follows` or `parent:`.

**Text:**
- A syllable is one of `j k q v x z` followed by one of `a e i o u y`. Common words are 1 to 3 syllables.
- Titles 2 to 4 words; paragraphs 20 to 60 words for a document, 15 to 50 for a section; sentences 6 to 14.
- Section j holds the 5-syllable rare word `floor(j / 8)`, in base 36, so a rare word is in at most 8 sections at any size.
- The READMEs hold one paragraph each (Tier 0 also lists the 16 areas), no ID and nothing that grows with N.

**Vocabulary:** a token is a run of `[A-Za-z0-9_-]` with its edge `-` stripped. Every token is one of:
- a syllable word;
- `P-\d{6}`, `a\d{2}`, `g\d{4}`, `2026-01-01` or digits;
- a front-matter or config key or value.

**Samples:**
- 20 W targets: live, non-hub part nodes, the first 3 timed.
- 3 queries: rare words held by a live section.
- The hub: `P-000001`. The flow root: `areas/a00/g0000/`'s head. The stage: the first word.

## Reads and steps

Each read is warmed up once, then timed three times, the best kept. A call includes `render_text`. The tree waits 2.1 s after it is written (`RACY_WAIT`), so its build stores keys a warm update trusts (`spec-read-freshness.md`). The four reads and the hub's two run again in a `trusted` scope whose `IndexTrust` the warm-up walked.

| Key | Call | Its steps |
|---|---|---|
| `get_node` | `show`, no links | open, update |
| `get_node_links` | `show`, `links: true` | open, update |
| `search` | `search`, one word, limit 20 | open, update, fts |
| `get_context_bundle` | `bundle`, one ID, `--budget` | open, update, then fitting |

Since 12d `show` and `bundle` read a subset after the update (`spec-read-subset.md`); the other steps, still replayed, are what a whole read costs (`reparse_us` without Q7).

The steps, each the best of three on the warm index:

| Key | What runs |
|---|---|
| `open_us` | `discover` + `open_index` |
| `walk_us` | `WorkingTree::new` + `Source::list` |
| `hash_us` | `Source::read` + `b3_hash` of every listed file |
| `update_us` | `IndexWriter::update`, the stat walk; `update_parsed`, `update_hashed` its max `parsed`, `hashed`, 0 when warm |
| `decode_us` | `SpecIndex::indexed_input` |
| `reparse_us` | the CLI's private `read_bytes` replayed: `Source::read`, core `parse`, `CheckFile::parsed` |
| `resolver_us` | `Resolver::new` |
| `graph_us` | `SpecGraph::new` over the re-parsed input |
| `fts_us` | `SpecIndex::search`, the first query |

`other_us.<read>` is `max(0, median_us - the sum of its steps)`: locating, assembly and fitting, rendering, noise.

The whole reads, timed alike, have no steps, `other_us`, trusted twin or bar (a hub's flow and trace walk): `get_flow_top`, `flow` with no ROOT, `depth: 0`; `get_flow_root`, of the flow root, `depth: 1`; `get_flow_stage`, of the stage; each `trace: true`; `get_trace_summary`, `summary: true`; `get_tree`, no ROOT, `depth: 1`. So have `similar` and `collisions`.

## Output

stdout is the envelope `{measurement: "scale", label, versions, wall_ms, result}`. `result` holds `seed`, `budget {tokens, source}`, `corpus_only` and `sizes[]`. Each size holds:
- `nodes`, `corpus {files, nodes, bytes, links, tier3_files, degree_max, hubs, hub_degree {min, max}}`, `index {files, nodes, build_us}`, `check {errors, warnings, us}`;
- `reads.<read> {median_us, max_us}`, `search` adding `hits_min`, `hits_max`, a whole read's of one target;
- `growth.<read>`: the median over the previous size's, to two decimals, `null` first;
- `reads_trusted.<read> {median_us, max_us}` and `growth_trusted.<read>` (each median at least 10 000 us), the trusted reads'; `trusted_differs`: (read, target) pairs whose trusted answer's BLAKE3 is not the walking one's;
- `steps`, `other_us`, `w_after {median, p90, max, tier0, tier1_max}` by nearest rank;
- `hub {degree, links_shown, get_node_links_us, get_context_bundle_us, w_after, trusted {get_node_links_us, get_context_bundle_us}}`;
- `similar_recall {planted, found, variants}`, each of the four `{planted, found}`; `collision_planted {planted, found}`;
- `nondeterministic`: (read, target) pairs whose four answers differ.

With `--corpus-only`, a size holds `nodes` and `corpus` only. stdout holds numbers only, never an ID, word or path. stderr holds one summary line per size, the trusted medians too.

The detail goes under `<out>/scale/<label>/`, emptied first. Each size gets a `n<N>/` directory with `tree/`, `home/` (the data directory) and `size.json`, written when the size ends, holding:
- the size's entry;
- `digest`: `b3:` over each file's path, NUL, bytes, NUL, in path order;
- `targets`, `queries` and `hubs`;
- `calls`: per read and target, the timings and the answers' BLAKE3; a whole read's target is `all`, the head's ID (`get_flow_root`) or the stage word (`get_flow_stage`);
- `w_after_bytes {tier1, bundle, total}`.

## Isolation

Every call takes `Globals {root: tree}`, an `Env` with `cwd` the tree and home the size's `home` directory (`xdg_data_home` `None`), and `GitEnv::new(tree, [PATH, HOME])`, `HOME` that directory. It never calls `Env::from_process` or `process_git`: nothing is read or written under the process's `HOME` or the repository, and the run deletes nothing outside `<out>/scale/<label>/`.

## Baseline

2026-11-08, `flow-scope`'s head, a quiet machine, release, seed 1, budget 10 000, `wall_ms` 54 679. A later slice re-runs its base and head on one machine.

| Read, median ms | 1k | 10k | 100k |
|---|---|---|---|
| `get_node` | 1.56 | 5.06 | 54.4 |
| `get_node_links` | 1.82 | 5.23 | 55.7 |
| `search` | 1.93 | 6.17 | 52.5 |
| `get_context_bundle` | 1.98 | 5.40 | 57.5 |
| `get_flow_top` | 9.27 | 90.3 | 1 016 |
| `get_flow_root` | 9.00 | 86.9 | 980 |
| `get_flow_stage` | 9.82 | 92.0 | 1 041 |
| `get_trace_summary` | 5.33 | 46.9 | 544 |
| `get_tree` | 8.49 | 82.6 | 956 |

At 100k: 20 015 files, 23.9 MB, 101 746 drawn links, 3 973 Tier 3; build 11.3 s, check 1.16 s, 0 findings; 8.5-11.6x from 10k; reparse 918 ms, decode 288, graph 156, update 44.9. RM-0054's bars hold: trusted 0.61-6.31 ms, `growth_trusted` 1.00, W_after's median 2 555, 2 391, 2 552 B; 0 nondeterministic, 0 trusted differ.

## Tests

`tests/scale_cli.rs`: **SE-1** one seed, one tree, the digest the documented hash, no clock, pid or environment read; **SE-2** the shape, the flow's too; **SE-3** a clean check; **SE-4** the closed vocabulary, no pilot name; **SE-5, SE-7, SE-8** one debug run at 1k, SE-7 also a static scan of the sources; **SE-6** the refusals, the bounds themselves accepted, `--timeout 0`.

`tests/scale_edges.rs` checks the index's edge tables (`spec-index-edges.md`) on the `--corpus-only` 1k tree: they equal core's resolution before and after 20 seeded edits, and the worktree dumps as a fresh index's.

`scale_baseline` is `#[ignore]` and runs the baseline: `cargo nextest run -p specengine-eval --release --test scale_cli --run-ignored ignored-only scale_baseline --no-capture`.

## Known limits

- **`links_shown` counts the hub's links before the 40 000-character cap.** It overstates what is printed above about 180 000 nodes.
- **The steps run low and `other_us` high:** they share one warm connection and one `WorkingTree`, a read opens its own; the residual never hides time.
- **Timing noise can push `other_us` to 0;** when every residual is 0, SE-8 says it cannot tell a dropped step apart.
- **A failure's stderr quotes the engine's reason,** which may hold a synthetic ID.
- **`layout::check_out`'s reused refusal calls the repository "the corpus".**
- **The in-degree cap never binds at seed 1:** `degree_max` 6, 12, 15 at 1k, 10k, 100k; dropping it survives the tests.
- **`process_git` in place of the scratch `GitEnv` is caught only by SE-7's static scan.**
- **The `read_bytes` mirror drops the CLI's warnings.** A file that cannot be read keeps its decoded parse.
- **Flow, trace and tree stay whole O(N) reads,** about 1 s at 100k, `--stage` bounding only the payload: a screen asking one per scope change pays it (RM-0084).
- **Not timed:** code reads, `--bindings`, graph and impact; cold reads after a start; concurrent readers; the JSON that MCP renders.
