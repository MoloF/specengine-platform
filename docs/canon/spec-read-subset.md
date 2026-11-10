---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core]
owner: owner
reviewed: 2026-11-10
---

# spec show, show --links, spec bundle: a subset read from the index

Since `store-reads` (`docs/features/store-reads.md`, RM-0079), `spec show REF`, `show REF --links [--archive]` and `spec bundle REF...` answer from the index rows of the files around the asked nodes and from those files' bytes, never from the whole corpus. Their twins call the same functions: MCP `get_node`, `get_context_bundle` and `spec://<slug>/node/<REF>`, a hub's MCP, the daemon's `nodes` and `bundle` routes, and `spec task show`'s bundle hash. On a tree no one edits, every answer is byte-equal to the whole read's. The index stays a rebuildable projection (ADR-0001, ADR-0003): the subset read adds no table or column (`INDEX_FORMAT` 17's two tables are the freshness step's), and no rule of `architecture.md` moved, so it has no ADR. Code: CLI `subset.rs`, `show.rs`, `bundle.rs`, `corpus.rs`. The tables and their lookups: `spec-index-edges.md`.

## The read

1. Every REF is classified as before. Its notes and exits are unchanged.
2. **The freshness step** (`freshness::step`, `spec-read-freshness.md`): the stat-keyed walk, or a hub's trusted index, through the one `WorkingTree` the read later reads its files with. Its warnings print once.
3. **An attempt (T3).** One snapshot, a deferred read transaction, computes the subset and decodes each of its files once with its stored BLAKE3. Every row of the attempt comes from it, and it ends before any file is read.
4. **The check (T5).** Every subset file, printed or not, is read once and compared with its row. It matches when its bytes hash to the row's BLAKE3 (`row_matches`), or when the row has no hash and the read fails now too. Anything else is a mismatch: another hash, a read error on a hashed row, or a readable file on a row with no hash. Every file is read before the read decides.
5. **All match.** The answer is built over the subset: `SpecGraph::from_rows` over the decoded parses, each given the bytes just read. The rest of `show` and `bundle` runs on it unchanged (holders, `node_links`, `bundle_layers`, assembly). Plain `show` still prints each holder from a parse of the very bytes it prints (`spec-cli-read.md` "Resolution").
6. **Some mismatch.** The attempt is dropped with its messages. `update_paths` runs on exactly the mismatched paths, re-parsing them and re-resolving their dependents in one transaction, and its report is dropped. Then one more attempt runs.
7. Anything else is **the walk** (below).

Nothing in an attempt prints: an answer is built only once its files matched.

## The subset

- **W(n)**: a node and the ID sections within it.
- **HELD(m)**: every row of every edge m holds (`HELD`).
- **ENDING(m)**: the edges with a row ending on m, in both `names_source` values (the by-end halves of `OUTGOING` and `INCOMING`), each completed with all its rows by `EDGE`.
- **C(REF)**, a REF's closure, grows in rounds from the empty set. Each round decodes the set, runs `Resolver::resolve_detached_reading` over it and adds the `HOLDERS` of every key read; it stops when no file joins. By R4 (`spec-index-edges.md` "Passes"), its resolution equals the whole input's. A path REF's closure is its path, when stored.

| Read | Rows | Files |
|---|---|---|
| `show REF` | each shown node's chain of `PARENT` rows, file by file, to the row's first end, until none or one repeats: its `ancestors` (`spec-cli-read.md` "Output") | C(REF); every chain file |
| `show REF --links` | `show REF`'s; for each shown node n and each m in W(n): HELD(m), ENDING(m) | C(REF); each chain file; each edge's citing file and ends |
| `bundle REF...` | `--links`' rows for each kept target (`bundle_linked`); for each linked candidate c, its held `working_answer` edges and, for m in W(c), its ending `answers` edges; each kept target's chain of `PARENT` rows, file by file, to the row's first end, until none or one repeats | the C(REF)s; every citing, end and chain file of those rows |

Generated and Tier 3 citing files are decoded too, for their standing (left-out counts, admission). Rows a read does not use stay out: a candidate's other edges, `PARENTED` and `DEPENDENTS`. So a hub candidate pulls only its `answers` citers, and a generated index citing the node joins once, without its fan-out.

## The walk

The walk is the whole read, each command's function from before the subset, kept as the fallback and as the tests' oracle (D5). It runs from its own freshness step, keeping only step 1's messages, and it never retries. It always builds the whole graph (`SpecGraph::new`), plain `show` too, for `ancestors`. Its causes:
- a second mismatch;
- `NotIndexed` from a snapshot: a stamp or worktree row gone after the freshness step (another build recreated the tables, or a row was deleted);
- any other store error in an attempt or from `update_paths`;
- **`covers`** fails: a row whose holder or resolved end is not a node of the subset's files. The plan decodes every file a row names, so only a corrupted row fails it, and the walk resolves again instead of printing a row no file holds.

**The walk's own change, beside the race cases below.** `show --links`' whole read printed each holder from a second read of its file, so a same-size edit between the two reads could pair one version's structure with the other's text: the AC-7 flipper found 3 mixed answers in 150 runs. It now prints a holder from the bytes `read_bytes` attached to its input (`indexed_read`), read once. Output at rest is unchanged.

## Race cases

Each case below replaces a mixed state with one snapshot:
- a subset file deleted or made unreadable after the freshness step: the update records it, and the read answers without it (the whole read warned `cannot read ...; left out`);
- a file outside the subset edited after the freshness step: answered from the snapshot;
- an unrelated file that vanishes: no warning;
- a file the parser panics on (its row has no hash): every subset read naming it walks, with the whole read's output.

## Code-layer reads stay whole (D7)

`show --bindings` and `bundle` when `[code]` is present or does not read (`CodeConfig::from_toml` other than `Ok(None)`), and `spec task show`'s bundle under the same condition, keep the whole read: the code judge builds `refs::Corpus` over every file, and `show --bindings` pays one whole-graph build more for `ancestors`, unmeasured. `show --bindings` without `[code]` reads a subset and adds its note.

## Whole reads keep a matching row's parse (Q7)

Every whole read that takes bytes still reads every parsed file: `tree`, `graph`, `impact`, `flow`, `roadmap`, `refs`, `verify`, `lock diff`, `health`'s code rows, the code-layer reads, `spec task show`'s package (`add_bytes`) and the walk. `read_bytes` takes each file's hash from the snapshot its input was decoded from (`decode_all`). A file whose bytes hash to its row keeps its decoded parse; any other is parsed again, with the warnings as before. This rests on the store's projection contract, a stored row decodes to exactly `parse(bytes)`, tested on every configured fixture and the 1k `scale` tree (store `tests/projection.rs`, eval `tests/store_reads_scale.rs`). A whole read never retries; these reads stay O(N).

## API

- **Store.** `SqliteIndex::snapshot() -> IndexSnapshot`: one deferred transaction over the handle's worktree, ended on drop, the handle unable to write until then; `NotIndexed` on an old stamp or no worktree row. Its methods: `holders(key) -> [path]`; `held(path, ord)` and `ending(path, ord)` -> `[StoredEdge]`; `parent(path) -> Option<StoredParent>`; `decode(&[&str])` and `decode_all()` -> `Decoded {input, blake3}`, `input` with empty `bytes`, `blake3` by path in lower-case hex, `None` for a read error or a parser panic. `StoredEdge` and `StoredParent` are core's `EdgeRow` and `ParentRow`, re-exported. The `pub(crate)` statements `EDGE`, `PARENT` and `FILE_BY_PATH` (the keyed decode's file row) each plan as a `SEARCH`.
- **Core `check`.** `SpecGraph::from_rows(&CheckInput, &IdScheme, &Paths, GraphRows {edges: [EdgeRow], parents: [ParentRow]})` resolves nothing. Edges are sorted by (path, `ord`), core's order; ends map by path onto the input's files, so `NodeAt` order agrees with the whole graph's; written forms and lines come from the citing file's bytes through `file_links`' per-file routine; then cycles, children and edge indexes as in `new`. A row naming no file of the input is dropped. It answers `links`, `ancestors`, `within`, `node`, `name`, `line`, `standing`, `document`, `file_of`, `locate` (given C(REF)) and `children` within one file (bundle's target forms); not `cycles`, `walk` or `documents`. `Resolver::resolve_detached_reading(&Reference) -> (Resolution, Vec<EdgeKey>)` adds the keys its lookups read, hit or miss, sorted and unique. `bundle_linked(&SpecGraph, targets) -> (kept, linked)`: the targets `bundle_layers` keeps, and the live nodes linked to them within no kept target.
- **CLI.** `subset::read(&Env, &ProjectRoot, Wanted::{Show, Bundle}) -> Subset::{Read(SubsetRead), Walk}`; `corpus::{row_matches, indexed_read, indexed_decoded}`. `show_walk` and `bundle_walk`, `#[doc(hidden)]` and public, run the walk alone for the tests (eval's 1k differential). The `#[cfg(test)]` seams in `subset::seams`: a thread-local hook at (a) after the freshness step, (b) after the snapshot ends, (c) between two file reads; counters of whole decodes, listings, whole resolvers and graphs, files decoded and read, `update_paths` calls, attempts, walks and `read_bytes` parses.
- `pick` and `pick_reference` take no `written` (D10); core's `resolve*` and `holders_of` keep theirs.

## Known limits

- **A subset read cannot see** an edit made after the freshness step to a file outside its subset: a file that starts citing the node then is not listed. On a hub it holds only hand edits on the host (`spec-read-freshness.md` "Invariants and the window"). The freshness step's cost: `scale-eval.md` "Baseline".
- **Corrupted edge rows** under intact file hashes are caught only by `covers`. Printed as stored until `spec index --full`: an `ord` with no site (`written` "" at line 1), a NULL reason on a dangling row, a flipped `names_source`, an end at another existing node, a wrong holder, deleted rows, missing `edge_names` keys.
- Statements are prepared per call: `prepare_cached` needs rusqlite's `cache` feature, and the workspace pins `default-features = false`.
- Left as perf nits: closure rounds clone the parses they resolve over, and the graph a bundle builds only to find its linked candidates computes every edge's written form.
- **Through a CLI REF the closure never grows past round 1:** look-alikes and mixed scripts are refused at classification, and an alias-prefix match keeps the run as written, so round 1 reads every key of the fallback chain. The later rounds and the bare form's key are untested guards; cutting either survives the tests.
- **Not tested:** subset exactness on the 1k `scale` targets (the counters are CLI `#[cfg(test)]`, the tree is eval's; all 21 configured fixtures are covered). The 10k differential is `#[ignore]`d and has not been run.
