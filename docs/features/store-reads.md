---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-eval]
ref: RM-0079; Phase 7 step 12d (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03; T3, T5 and Q7 are the orchestrator's, D1-D11 await the owner's review
adrs: []
---

# store-reads: `show`, `--links` and `bundle` read a subset from the index (RM-0079)

## Why

To answer for one node, today's reads take the whole corpus. At 100 000 nodes `get_node` costs 705 ms, `get_node_links` 1.64 s and `get_context_bundle` 1.61 s; reparse (798 ms), hash (393), decode (249) and graph (128) dominate (`docs/canon/scale-eval.md` "Baseline"). Each of the three reads decodes every row (`indexed_input`, store `src/read.rs:271-273`). Plain `show` then builds a resolver over the whole input (CLI `src/show.rs:216-220`). `--links` and `bundle` also re-read and re-parse every parsed file (`corpus.rs:37-50`, `:69-101`) and build a whole `SpecGraph` (`show.rs:247-249`, `bundle.rs:398-400`). RM-0078 stored core's resolution in the index (`docs/canon/spec-index-edges.md`), and nothing reads it yet.

Here `spec show`, `show --links` and `spec bundle`, with their MCP and daemon twins, read a subset: the rows of the files around the asked nodes, plus those files' bytes. Whole-corpus reads stay O(N), but they stop re-parsing a file whose bytes match its row (Q7). Two of the step's invariants hold from here on:
- **T3.** One index snapshot per read.
- **T5.** Every printed byte matches the BLAKE3 of the row it came from. On a mismatch the read updates that path and retries; if that is not enough, it walks.

T1, T2 and T4 belong to RM-0054 (12e).

Decided here, each awaiting the owner's review:
- **D1 Rows and bytes, no resolution of links.** A subset read resolves no link. Each edge's ends, state, reason and label are its stored row (E2: core's). Its written form and line come from the citing file's bytes, through the same per-file routine `file_links` uses (core `check/spec_graph.rs:780-901`). Only a REF is resolved, by core's detached resolution over that REF's closed subset ("Data").
- **D2 The subset per read** is the table in "Data". Every file in it is decoded once from the snapshot and read once from the tree.
- **D3 T3 is one deferred read transaction per attempt** (`IndexSnapshot`). Every row of an attempt comes from it, and it is closed before any write.
- **D4 T5 covers every file of the subset**, printed or not: a file whose row is used is a file whose bytes are checked. On a mismatch, `update_paths` runs on exactly the mismatched paths and the read retries once. A second mismatch walks.
- **D5 The walk is today's read.** Today's code path stays as a function: it is the fallback and the differential's oracle.
- **D6 The freshness step is unchanged.** Every read still runs `refresh` -> `update` first (`refresh.rs:116-150`). "Description and interactions" sets 12d's freshness apart from 12e's.
- **D7 Code-layer reads keep the whole input.** These are `show --bindings` and `bundle` when `[code]` is present or does not read (`CodeConfig::from_toml` other than `Ok(None)`: `show.rs:211-219`, `bundle.rs:484`), and `spec task show`'s bundle under the same condition (`package.rs:964`). The judge builds `refs::Corpus` over every file (`bindings.rs:219`). They gain Q7 only.
- **D8 Q7 belongs in this slice.** It is one function (`read_bytes`, `corpus.rs:69-101`), and it applies T5's rule (bytes against the row's BLAKE3) to the same snapshot hashes the subset read needs. Its equality rests on a contract the store already tests: a stored row decodes to exactly `parse(bytes)` (store `tests/projection.rs`, spec-a and spec-b), which SR-3 widens to every fixture. It also shortens the reads D7 keeps whole, and the walk itself. Its risk is breadth: it reaches every `with_bytes` caller, not only `tree`, `graph`, `impact` and `flow`. These callers are `flow.rs:153`, `graph.rs:183`, `tree.rs:160`, `impact.rs:368`, `roadmap.rs:193`, `verify.rs:479`, `verify_node.rs:81`, `lock_diff.rs:205`, `health_code.rs:447`, `refs.rs:222`, `bundle.rs:627` and `package.rs:342`. SR-3 bounds that risk with a parse counter and the projection contract. Splitting it out would cost a second pass over the same function and the same goldens, for no smaller risk.
- **D9 The store API is read-only.** No table, column, index, `INDEX_FORMAT` (stays 16) or dump changes. Two keyed statements are added.
- **D10 `pick` and `pick_reference` lose `written`** (`show.rs:604-609`, `:613-662`, its `let _ = written;` at `:645`; callers `:311`, `:788`, `refs.rs:465`). Core's `resolve`, `resolve_mention`, `resolve_detached` and `holders_of` keep theirs (`check/resolve.rs:274-364`): dropping them is a cross-crate churn that changes no behaviour, so it is left out.
- **D11 No ADR.** No rule of `docs/canon/architecture.md` changes:
  - files stay the truth, and T5 makes each printed byte the file's own;
  - the index stays a rebuildable projection with no new rows;
  - nothing is blocked.

  What a read may trust without walking is RM-0054's "read freshness" ADR (Q6), and 12d decides none of it (D6). On a tree no one edits, every answer is byte-equal to today's.

## Description and interactions

**One subset read**, for `show REF [--links [--archive]]` and `bundle REF…` when D7 does not apply:
1. Classify every REF, as today (`show.rs:203`, `bundle.rs:376-394`). Its notes and exits are unchanged.
2. Run the freshness step, unchanged (D6). Its warnings print once.
3. **Attempt.** Open the snapshot. Compute the subset ("Data"). Decode its files with their stored BLAKE3, then close the snapshot.
4. Read each subset file through the same `WorkingTree` the freshness step walked. Compare it against its row (R3).
5. If every file matches, build the subset graph (`SpecGraph::from_rows`) over the decoded parses with those bytes attached, and run today's code on it unchanged: `holders`, `node_links`, `bundle_layers`, `assemble`. For a path REF, `show` is found by its row. Plain `show` keeps its holder loop, which takes a fresh parse of the bytes it prints (`show.rs:273-322`).
6. If some files mismatch, the attempt's messages are dropped and `update_paths(M)` runs on the mismatched paths. Its report is dropped and it prints nothing. Then step 3 runs once more.
7. A second mismatch, `NotIndexed` from a snapshot, or a store error from `update_paths` runs **the walk**. The walk is today's whole read, Q7 included, rerun from step 2 with only step 1's messages kept. The attempts' messages and the first freshness step's warnings are dropped, so no warning prints twice.

**Twins.** The twins call the CLI's functions, so none of them changes:
- MCP `get_node` (`mcp/src/read.rs:709-728`) and `get_context_bundle` (`:765-779`);
- the resource `spec://<slug>/node/<REF>` (`resources.rs:187`);
- the hub's MCP, through the same `read` (`hub.rs:53`);
- the daemon's `nodes` and `bundle` routes (`http/src/app.rs:541-560`, `:585-597`);
- `spec task show`'s bundle hash (`package.rs:964`).

**12d's freshness, set apart from 12e's.** In 12d, every read still runs today's whole `update`: it walks, hashes every file and takes the write transaction. So a file edited before the read is caught by that step, as today. The read then trusts the snapshot for nothing it has not read. Every file it uses is read now and compared with the row's BLAKE3, and a mismatch is repaired by `update_paths` of exactly those paths. 12d has:
- no stat key (Q4);
- no trusted or hub mode, no mode flag through http or mcp, and no change to the MCP texts that promise a refresh (`mcp/src/hub.rs:262`, `:339`);
- no per-process dirty mark (T4), and no reconcile (Q3);
- no replayed warnings (T2), since the freshness step prints its own;
- no T1, since there is no trusted read to compare.

12d's walk on doubt is local to one read. 12e replaces the freshness step and issues the ADR; 12d's T3 and T5 are what that ADR may then rely on.

**What a subset read cannot see:** an edit made after the freshness step to a file outside its subset. Say a file starts citing the node: today `--links` would list it (every file is re-parsed), and 12d answers from the snapshot. That window is 12e's.

## Data

No `specengine.toml` key, MCP tool, route, JSON key or CLI flag changes. The tables and lookups are `docs/canon/spec-index-edges.md` "Tables", "Keys" and "API".

### The subset

The notation:
- **W(n)** is a node and the ID sections within it (`nodes_within` over its decoded parse).
- **HELD(m)** is store `HELD` (`edges.rs:52-58`): every row of every edge m holds.
- **ENDING(m)** is the edges with a row whose end is m, in both `names_source` values: the by-end halves of `OUTGOING` and `INCOMING` (`:66-76`, `:84-94`), or one statement over `edges_by_end`. Each such edge is completed with all its rows by `EDGE`, since a by-end row is only the matching end (n), and `name` takes `nodes.first()` (`links.rs:93-95`).
- **C(REF)**, a REF's closure, is built in rounds. Start with the empty set. Each round decodes the set, runs `Resolver::resolve_detached_reading` over it, and adds `HOLDERS` of every key read. It stops when no file joins. A path REF's closure is its path, when it is stored. By R4 (`spec-index-edges.md` "Passes"), the resolution over C(REF) equals the whole input's.

| Read | Rows | Files |
|---|---|---|
| `show REF` | none | C(REF) |
| `show REF --links [--archive]` | for each shown node n and each m in W(n): HELD(m), ENDING(m) | C(REF); each edge's citing file; every end of every edge |
| `bundle REF…` | as `--links`, for each kept target. For each linked candidate c (live, within no target): c's HELD edges of type `working_answer`, and, for m in W(c), its ENDING edges of type `answers`. For each kept target, `PARENT` of each document on its chain: a section goes to its enclosing node, a document to its row's n = 0 end, until there is none or one repeats | the union of the C(REF)s; the files of `--links`; the citing and end files of the candidate edges; every chain file and every end of a parent row |

Files outside D7's whole reads count too, including generated and Tier 3 citing files: they are decoded for their standing (left-out counts, `admits`). Rows a read does not use stay out: other types of a candidate's edges, PARENTED and DEPENDENTS. So a hub candidate pulls only its `answers` citers, and a generated index that cites the node joins once, without its fan-out.

**Worked example** (`spec-index-edges.md` "Example"), `bundle R-02`:
- C(R-02): round 1 reads `name:R-02` and finds `docs/b.md`; round 2 adds nothing.
- ENDING(b.md, 0) gives `docs/a.md` edges 0 (`depends_on`), 2 and 3 (`mentions`), one row each. HELD(b.md, 0) is empty.
- The candidate `(a.md, 0)` has no `working_answer` or `answers` edges.
- PARENT(b.md) is `0 resolved NULL docs/features/stamina.md 1`. The chain runs `(stamina.md, 1)`, then `(stamina.md, 0)`, then nothing, giving layer 3.
- The subset is `{docs/a.md, docs/b.md, docs/features/stamina.md}`. Lines 4 and 8 come from a.md's bytes.

### Store API (`src/read.rs`, `src/edges.rs`; read-only)

```rust
impl SqliteIndex {
    /// One deferred read transaction over the handle's worktree (T3);
    /// `NotIndexed` as `with_worktree` (`read.rs:61-73`).
    pub fn snapshot(&self) -> Result<IndexSnapshot<'_>, StoreError>;
}
impl IndexSnapshot<'_> {
    pub fn holders(&self, key: &str) -> Result<Vec<String>, StoreError>;          // HOLDERS, `EdgeKey::text()`
    pub fn held(&self, path: &str, ord: usize) -> Result<Vec<StoredEdge>, StoreError>;   // by (path, ord), ends in n order
    pub fn ending(&self, path: &str, ord: usize) -> Result<Vec<StoredEdge>, StoreError>; // each edge whole (EDGE)
    pub fn parent(&self, path: &str) -> Result<Option<StoredParent>, StoreError>;        // PARENT
    pub fn decode(&self, paths: &[&str]) -> Result<Decoded, StoreError>;                 // `decode_some` (`read.rs:351-391`)
    pub fn decode_all(&self) -> Result<Decoded, StoreError>;                             // `decode_all` (`:292-347`), Q7
}
pub struct Decoded {
    pub input: CheckInput,                         // path order, `bytes` empty
    pub blake3: BTreeMap<String, Option<String>>,  // `files.blake3`: lower-case hex, NULL for a read error or a parser panic
}
```

The two new statements, each a `SEARCH` on a primary key:

```sql
-- EDGE: ?1 wt, ?2 path, ?3 ord
SELECT <edge_columns!()> FROM files f CROSS JOIN edges e ON e.file_id = f.file_id
 WHERE f.wt = ?1 AND f.path = ?2 AND e.ord = ?3 ORDER BY n;
-- PARENT: ?1 wt, ?2 path
SELECT p.n, p.state, p.reason, p.end_path, p.end_ord
  FROM files f CROSS JOIN edge_parents p ON p.file_id = f.file_id
 WHERE f.wt = ?1 AND f.path = ?2 ORDER BY p.n;
```

`#[allow(dead_code)]` comes off each lookup used; PARENTED keeps it. `stored_edges` stays for tests.

### Core API (`check`)

```rust
impl Resolver<'_> {
    /// `resolve_detached`, and every key its lookups read, hit or miss, sorted and unique,
    /// as `resolve_files` records them (`spec_graph.rs:275-300`). No `written` parameter.
    pub fn resolve_detached_reading(&self, reference: &Reference) -> (Resolution, Vec<EdgeKey>);
}
impl<'a> SpecGraph<'a> {
    /// The graph of a subset whose edges and parents are given, never resolved.
    pub fn from_rows(input: &'a CheckInput, scheme: &'a IdScheme, paths: &'a Paths, rows: GraphRows) -> Self;
}
pub struct GraphRows { pub edges: Vec<EdgeRow>, pub parents: Vec<ParentRow> }
// EdgeRow: StoredEdge's fields (`edges.rs:131-152`); ParentRow: StoredParent's (`:157-167`).
// The store maps its types one to one, or core owns them and the store re-exports.
```

`from_rows` works as follows:
- **Edges** are sorted by (citing path, `ord`), core's order (E3).
- **Ends** are mapped by path onto the subset's files, whose indexes follow path order, so `NodeAt` order agrees with the whole graph's.
- **`state`** gives the endpoint: `resolved` gives `Nodes(ends)`, `dangling` gives `Dangling(reason)`, `skipped` and `unchecked` give their own. The other end is `Nodes([holder])`, and `names_source` swaps the two.
- **`reason`** is the note of a resolved or unchecked edge.
- **Written forms and lines** come from one per-file routine that `file_links` also uses, without lookups: declared references (except `parent:`), then a path `canon:`, then inline links, through `resolve::written`, `reference_line`, `canon_written`, `links::written` and `links::line`.
- **A parent row** gives `Node {ends[0], others}`, `Dangling` or `Skipped`, its `written` from the bytes.
- **Last,** `break_cycles`, `link_children` and `index_edges` run as in `build` (`:348-365`).

It answers exactly for the nodes its rows cover: `links`, `ancestors`, `within`, `node`, `name`, `line`, `standing`, `document`, `file_of`, and `locate` when the subset holds C(REF). `edges()` holds only the given rows. `children`, `cycles`, `walk` and `documents` are not answered, and no subset read calls them.

### CLI

A subset module, its name the developer's, is called by `run_show` (`show.rs:192-360`) and `bundle_with` (`bundle.rs:331-436`). Today's paths stay as the walk. Q7 lives in `read_bytes`: one snapshot gives the input and the hashes (`decode_all`); a parsed file whose bytes hash to its row becomes `CheckFile::parsed(path, bytes, decoded)`; any other file is re-parsed as today, with today's warnings. A whole read never retries.

`#[cfg(test)]` seams for the tests: a thread-local hook called at three stages:
- (a) after the freshness step;
- (b) after the snapshot closes, before the first file read;
- (c) between two file reads.

Counters for whole decodes (`decode_all`, `indexed_input`), `Source::list` calls after the freshness step, files decoded, files read, `update_paths` calls and their paths, attempts, walks, and parses in `read_bytes`.

### Eval

`other_us` subtracts open and update only for `get_node`, `get_node_links` and `get_context_bundle` (`scale/mod.rs:1070-1092`). The other steps are still replayed and reported, since they are what the whole reads cost. SE-8's table follows (`tests/scale_cli.rs:108`).

## Rules and edge cases

- **R1 Subset.** WHEN `show` or `bundle` runs and D7 does not apply, the read SHALL decode exactly the subset of "Data", and no whole decode, `Source::list` beyond the freshness step, `Resolver::new` or `SpecGraph::new` over the whole input SHALL run.
- **R2 Snapshot (T3).** Every row of an attempt SHALL come from one deferred transaction, closed before `update_paths`. The answer SHALL come from one attempt, or from the walk.
- **R3 Match (T5).** A file matches when the bytes read now hash (store `hash_bytes`, `rows.rs:91-93`) to its row's `blake3`, or when the row has no hash and the read fails now too. Anything else is a mismatch: another hash, a read error on a hashed row, or a readable file on a row with no hash. The attempt reads every subset file before deciding, so one `update_paths` covers them all.
- **R4 Retry, then walk.** WHEN an attempt has mismatches, the read SHALL run `update_paths(M)` (`lib.rs:602-607`), which re-parses those files and re-resolves their dependents in one transaction (`spec-index-edges.md` "Passes"). It SHALL then attempt once more. A second mismatch, `NotIndexed` or an update error SHALL walk (D5), and the walk never retries.
- **R5 Messages.** The output SHALL hold step 1's messages, the freshness step's warnings once, and the final attempt's own messages. The walk's output is today's from its own freshness step. Byte-equality holds because the subset path prints no corpus-wide message. The only corpus-wide ones today are `read_bytes`' `cannot read …; its lines show as 1` and `the spec parser failed on …` (`corpus.rs:83-86`, `:96-98`), and on an unedited tree they never print: a parsed row was readable a moment before, and the same parse succeeded.
- **R6 Edge cases.**
  - A REF with several holders keeps its `has N holders` warning and its holder order (path order in both graphs).
  - A target in a Tier 3 or generated file is admitted as today.
  - A non-UTF-8 file has no node and no edge, so it gets no lookup.
  - A cycle on a chain is broken at its least member, as the whole graph breaks it (`spec_graph.rs:1042-1075`): every member lies on the chain.
  - Ties in the link sort (`links.rs:164-172`) and the first `working_answer` per holder (`check/bundle.rs:184-194`) follow edge order, which R1's (path, `ord`) order keeps.
- **R7 Missing or stale tables.**
  - An older `INDEX_FORMAT`, a worktree not yet indexed, or a moved `[ids]` or `[paths]` fingerprint is repaired by the freshness step (full pass) before the snapshot.
  - A stamp or worktree row that vanishes between the two gives `NotIndexed`, and the read walks: another build recreated the tables, or a row was deleted.
  - An edge row corrupted while its file rows are intact is not detected; the read prints what the row holds, and `spec index --full` repairs it ("Out of scope").
- **R8 Race-time divergences**, by design and recorded in the canon at ship. Each one replaces a mixed state with one snapshot:
  - a subset file deleted or made unreadable after the freshness step: today `cannot read …; left out`; now the update records it and the read answers without it;
  - a file outside the subset edited after the freshness step: answered from the snapshot (see "What a subset read cannot see");
  - an unrelated file that vanishes: no warning;
  - a file the parser panics on (its row has no hash): every subset read that names it walks, with today's output.
- **R9 Code layer.** WHEN D7 applies, the read SHALL be today's whole read, with Q7.

## Acceptance criteria

Tests run on scratch copies (the repository and fixtures stay unwritten), and non-ASCII text in sources is written as escapes. "M" lists the mutations each criterion must turn red. "The walk" is D5's function on the same index state.

Mapping to the step's analysis: its AC-7 "One state per read" is AC-7 here, in its 12d form ("both modes" and "trusted" are 12e's); its AC-12 "Store reads are sublinear" is AC-12's read half here (12c shipped the store half); the equality criterion is SR-0.

- [ ] **AC-7 [default, cli] One state per read.**
  - `crates/specengine-cli/tests/freshness.rs:285` (`a_same_size_edit_racing_the_read_is_never_mixed`, a whole `spec tree` under an atomic-rename flipper) stays green and unedited.
  - **The swap.** For `show REF`, `show REF --links` and `bundle REF`, two same-size versions of one subset file are prepared as that test's `sprint_moved` makes them: a different `parent:` and different links, and sections on other lines at the same byte offsets. The at-rest answer of each version is recorded first. Then the hook, at stage (b), between the snapshot and the byte read, swaps the file for the other version with the same size and the original's mtime restored to the nanosecond. The answer equals exactly one version's at-rest answer, never structure (spans, ends, parents, lines from rows) from one version with text from the other.
  - The same holds over the binary under a flipper like that test's, alternating `show --links` and `bundle` for 150 runs within 40 s, with zero mixed answers and both versions seen.
  - M: print bytes without comparing their BLAKE3; the attempt's rows kept with the swapped bytes attached.
- [ ] **SR-0 [default, cli] Equal answers.**
  - **The golden.** `tests/store_reads_before.json` is recorded with the parent commit's `spec` binary before any `src/` change (flow-linear's L6), and never again. It covers the 19 configured fixtures (`fixtures/*/specengine.toml`):
    - for every node ID the index holds (repeats once) and every indexed document path: `show REF`, `show REF --links`, `--links --archive` where a fixture has Tier 3 files, and `bundle REF` at the default budget and at `4294967295`;
    - per fixture, an unknown ID (exit 1), a look-alike (exit 2), `project:`, a missing `.md` path, and `ID@3`.

    Each row is run plain and with `--json`, in a fresh `HOME` over a fresh copy, and keeps BLAKE3 digests of text, JSON and stderr, and the exits as `"<plain> <json>"`. Every row replays equal.
  - **Existing goldens.** These stay unedited and green: `flow_linear_before.json` (476 bundle rows), `trace_views_before.json`, `bundle_bindings_before.json` (spec-a, -b, -c and code-a without `[code]`; `[code]` bundles), CLI `parity.rs`, http `parity.rs` (`nodes/<every tree ID and path>`, `bundle?node_ids=`), and MCP `mcp_read`.
  - **Differential.** The subset read's outcome, text and JSON, equals the walk's for every fixture node and path, and on the `scale --corpus-only` 1k tree for its 20 W targets, the 4 hubs and 200 seeded nodes. At 10k it runs under `#[ignore]`.
  - M: ENDING not completed by `EDGE`; end files left out of the subset; C(REF) cut to one round (an alias declared in a second file); a section on a chain not going to its enclosing node; candidate `answers` edges left out (an answered question placed in layer 2); edges in fetch order instead of (path, `ord`).
- [ ] **AC-12 [default, store unit; cli unit] The lookups serve the read** (the read half of RM-0078's AC-12).
  - `EDGE` and `PARENT` plan as a `SEARCH` with no table `SCAN`, beside RM-0078's six.
  - **Sublinear on SQL-filled rows**, as 12c's AC-12. The rows are made by SQL at 10 000 and 100 000 nodes, every column the schema requires filled from `PRAGMA table_info` (precedent store `src/symbols.rs:182-191`, `:490-502`). `EDGE`, `PARENT` and the lookups that build a subset (`HOLDERS`, `HELD`, the by-end rows, the keyed decode with its BLAKE3) each cost at most 3x more at 100 000 than at 10 000, best of three, with that test's 2 ms floor.
  - On every fixture node and the 1k targets, the files decoded and read are exactly the subset the oracle computes from the whole graph by "Data". The whole-decode, `Source::list`, whole-resolver and whole-graph counters stay 0 after the freshness step.
  - Every `Edge` that `from_rows` holds equals the whole graph's edge at that (path, `ord`) in `link_type`, `origin`, `holder`, `source`, `target`, `names_source`, `written`, `line`, `offset`, `reason()`, `edge` and `state()`.
  - M: the resolver over `indexed_input`; the subset widened by the citing files' own closures (the generated `fixtures/impact/docs/spec/departures.md` pulling its corpus); `EDGE` without `f.path = ?2` (a scan: about 10x); the keyed decode served by `decode_all`.
- [ ] **SR-1 [default, cli unit] T5 under concurrent edits.** At each hook stage (a), (b) and (c), the hook makes one of these edits:
  - a holder's text changed;
  - a citing file dropping its link;
  - an end's ID renamed;
  - a subset file deleted;
  - a subset file made unreadable (mode 000).

  In each case, the answer equals a second read with no hook. No stale text, ID or link prints. `update_paths` runs once, with exactly the mismatched paths, and the first attempt's messages are absent. A hook that edits at every attempt gives two attempts, then the walk, whose output equals the walk's on the tree it saw.
  - M: no compare; `update` in place of `update_paths` (the counter); a retry on the old snapshot; unbounded retries (the editing hook never stops: a 10 s test deadline); messages kept from attempt 1.
- [ ] **SR-2 [default, cli unit] Corrupted, stale and missing rows.** At stage (a), SQL on the scratch DB makes one of these changes:
  - a subset file's `files.blake3` set to another hash: one `update_paths`, then the golden's answer;
  - `blake3` set to NULL with a `read_error`: the same;
  - the format stamp set to 15, or the worktree row deleted: `NotIndexed`, the walk, the golden's answer.

  Through the binary, the golden's answer also comes:
  - from an index whose stamp is 15 and whose edge tables were emptied before the read;
  - from a worktree never indexed;
  - from a `git worktree add` copy read first.
  - M: `NotIndexed` surfaced as exit 2; a NULL hash taken as a match; the subset read run before the freshness step.
- [ ] **SR-3 [default, store; cli unit] Whole reads keep a matching row's parse (Q7).**
  - Store `projection.rs` widens to every configured fixture and the 1k tree: each stored file's decoded parse equals `parse(path, bytes, scheme)`.
  - A warm `tree`, `graph`, `flow`, `impact` and `verify` over an unchanged copy parses 0 files in `read_bytes`. One file edited at stage (a) is re-parsed alone, and its fresh parse is used.
  - The flow and trace rows of `flow_linear_before.json` and `trace_views_before.json` stay equal, and `freshness.rs:285` (AC-7) is Q7's race guard: its own mutation, "a same-size file kept with its indexed parse", is this one.
  - M: the decoded parse kept without a compare (the edit's stale parse printed); hashes matched by position, not path; `rebuild_parsed` dropping `summary`.
- [ ] **SR-4 [ignored, eval release] Before and after at 1k, 10k and 100k.** `specengine-eval scale --nodes 1000,10000,100000 --seed 1` runs in release at the parent commit and with the slice, on one machine in one session (best of three per read, as `scale` times).
  - Each size's `size.json` `calls` answers equal across the two runs, and `nondeterministic` is empty in both.
  - The medians of the four reads, `update_us` and the hub's two reads are recorded in "Implementation" and in `scale-eval.md` "Baseline", rewritten in place within its cap, with no bar (RM-0054).
  - Expected but not asserted: the three reads fall to about `update_us` plus a few ms, and the freshness step's hashing (393 ms at 100k) becomes their cost, which is 12e's to remove.
- [ ] **F1 [form].**
  - `pick` and `pick_reference` take no `written`.
  - Clippy and fmt are clean, and no dependency is added.
  - The code touches no `docs/`, and MCP and http `src/` are unchanged.
  - `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

- For RM-0054 (12e):
  - T1, T2 and T4, the dirty mark included;
  - the stat key (Q4), the code index's "never mtime" (Q5) and the hub's reconcile and in-turn writers (Q3);
  - the mode flag and the MCP refresh texts;
  - the "read freshness" ADR (Q6);
  - the bars (Q1, Q2), including `update_us` with a generated index (`spec-index-edges.md` "Known limits": a retry on a file such an index cites pays its fan-out).
- Subset reads for `tree`, `graph`, `impact`, `flow`, `trace`, `refs` and `roadmap`: they stay O(N), with Q7.
- The code-layer judge over a subset (D7).
- Core's ignored `written` parameters (D10), and `catch_unwind` around resolution.
- Detecting corrupted edge rows; `children` and `cycles` from PARENTED.

## Implementation

To be filled after implementation. Files in scope, verified against `fd7c095`:

| Module | Change |
|---|---|
| core `check/resolve.rs` | `resolve_detached_reading` (beside `:294-304`; `Reads` stays `pub(crate)`, `:98`) |
| core `check/spec_graph.rs`, `check/mod.rs` | `from_rows`, `GraphRows`; the per-file text routine shared with `file_links` (`:780-901`) |
| store `src/read.rs`, `src/edges.rs`, `src/lib.rs` | `IndexSnapshot`, `Decoded`; `EDGE`, `PARENT`; HELD, OUTGOING, INCOMING and HOLDERS used |
| CLI subset module, `show.rs`, `bundle.rs` | the read of "Description and interactions"; today's paths kept as the walk |
| CLI `corpus.rs`, `refs.rs:465` | Q7 in `read_bytes`; `pick_reference` without `written` |
| eval `src/scale/mod.rs` | `other_us` |
| tests | CLI `tests/store_reads.rs` and `store_reads_before.json`, unit tests behind the seams; store `projection.rs` and a plan test; eval `scale_cli.rs` `READ_STEPS` |

**Canon at ship.** Most of the canon this slice touches is at or near its cap:
- a new Tier 2 page `docs/canon/spec-read-subset.md`: the subset, T3, T5, the walk, Q7 and R8;
- `spec-index-edges.md` (12 198 of 12 288 B): "No read uses them yet", and "Known limits" (`pick`);
- `spec-cli-graph.md` "Input" (12 261 B) and `spec-cli-bundle.md` "Command" (12 272 B): pointers only;
- `spec-cli-read.md` "Resolution";
- `scale-eval.md` "Reads and steps" and "Baseline" (11 588 B);
- the CLI README (10 045 B) and the store and core READMEs (10 238 and 10 237 B): API lines replaced by pointers, never added.
