---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-core, crates/specengine-model]
ref: RM-0078; Phase 7 step 12c (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split and D1-D10 awaiting the owner's review
adrs: []
---

# edge-table: core's resolved edges kept in the index, incrementally (RM-0078)

## Why

To know one node's incoming links, every read today parses the whole corpus and resolves every link. `SpecGraph::build` resolves them all (core `src/check/spec_graph.rs:289-338`), and with `--links` or in a bundle it runs over every file re-read and re-parsed. At 100 000 nodes that is reparse 798 ms, decode 249, graph 128 and resolver 22 of a 1.6 s bundle (`docs/canon/scale-eval.md` "Baseline"). RM-0079 (12d) wants `show`, `--links` and `bundle` to read a subset from the index, and it can only do that if the edges are already there.

This slice stores core's resolution in the index and keeps it current under the store's hard property: after any sequence of edits, the incremental index equals a rebuild, row for row (store README). `INDEX_FORMAT` becomes 16. Reads do not change: RM-0079 moves them onto the table. The invariants come from the step's analysis (RM-0078):
- **E1.** The table is a pure function of the stored rows, the `[ids]` fingerprint and the `[paths]` keys that resolution reads.
- **E2.** Stored edges equal core's resolution. One core produces them, and SQL holds no second rule set.
- **E3.** No read orders by rowid.
- **E4.** Nothing computed per read is stored.

Decided here, each awaiting the owner's review:
- **D1 Core resolves, the store stores.** A new `check::resolve_files` runs the same per-file routine `SpecGraph::build` runs (`file_links`, `:703-832`). The store writes its output and matches key text. It never decides an end.
- **D2 Four tables.** `edges`, `edge_parents`, `edge_deps` and `edge_names`, keyed by the citing file. An end is a `(path, ord)` text pair, never a surrogate key, because a target file's rows are deleted and inserted again on every edit. There is one row per end node.
- **D3 Stored columns are byte-free.** The table holds no `written` text and no line: those come from the citing file's bytes, which RM-0079 reads and checks (T5).
- **D4 Dependency keys are the lookups resolution made.** Core's `Resolver` records them and the store keeps them per citing file (`edge_deps`). Each file's names (`edge_names`) are core's too. An update re-resolves the files whose recorded keys meet a changed file's old or new names, over a subset closed by a fixpoint ("Rules and edge cases" R4).
- **D5 Parse-time capture.** Two resolution inputs that today come from the bytes move into the parse. The first is a `parent:`'s qualifiers: `ParentRef` keeps only the ID and a span (model `src/node.rs:75-81`), and `parent_reference` re-reads the bytes (core `check/resolve.rs:602-639`). The second is the ID as written, which `bare` looks up in `aliases:` (`:518-525`). Without the capture, `indexed_input`, which has empty `bytes`, resolves a different graph from the check's. This closes the open question in `docs/canon/spec-check-links.md` "Open owner questions" ("An escaped `parent:` loses its qualifiers") with the answer and cost it lists: a model and `INDEX_FORMAT` change.
- **D6 A `[paths]` fingerprint.** `worktrees.paths_fp` covers the four keys resolution reads: `features`, `link_base`, `roots` and `exclude`. When it moves, every edge is recomputed and nothing is re-parsed, and `update_paths` walks. Today the fingerprint covers `[ids]` only (store `src/write.rs:97-103`), and `link_base` is documented as "outside the index fingerprint" (core `src/paths_toml.rs:72-76`).
- **D7 `[paths]` comes from the source.** `Source::paths()` gives the writer the `Paths` it walks by, so no `IndexWriter` signature changes (`src/lib.rs:559-589`) and no caller does.
- **D8 Batched probes, not a threshold.** `update_paths` probes its paths through `Source::probe_paths`. A working tree reads each directory's listing at most once per batch, which ends the `entry_kind` n×m (store `src/source.rs:564-582`, README "Open minors"). There is no escalation by count.
- **D9 Reports stay as they are.** `UpdateReport` gains `resolved` and `resolved_all`, marked `#[serde(skip)]`, so the text and JSON of `spec index` and eval's `index` do not change.
- **D10 No ADR.** The edges are rebuildable index rows outside the repository (ADR-0001, ADR-0003), and no rule in `docs/canon/architecture.md` changes. D5 takes the other answer an open question already priced, and is recorded here and in the canon at ship.

## Description and interactions

`update`, `update_paths` and `rebuild` keep the edge tables current inside their one `Immediate` transaction, after the file rows are applied (store `src/write.rs:159-262`). Nothing outside the store and core changes behaviour: every read still resolves per call, and `spec index` prints the same.

**Full pass.** It runs when the stamp was recreated, the worktree is new, `scheme_fp` or `paths_fp` moved, or this is a `rebuild`:
1. Delete the worktree's `edges`, `edge_parents` and `edge_deps`.
2. Decode every stored file with `indexed_input`'s decode, in this transaction.
3. Call `resolve_files(&input, scheme, source.paths(), None)`.
4. Insert its edges, parents and reads.

`resolved` is then the number of files and `resolved_all` is true. `edge_names` rows are written with each file's rows, because a file's names depend on that file alone.

**Incremental pass.** It runs otherwise. C is the set of paths whose rows this transaction deletes or writes, and nothing happens when C is empty:
1. **N, the changed names:** C's `edge_names` rows, read before their deletion, plus core's `given_keys` of C's new rows.
2. **A, the files to re-resolve:** the written paths, plus the paths whose `edge_deps` hold a key in N, minus deleted paths. A is tracked by path, because file ids change on insertion.
3. Read A's current `edge_deps`. Then delete A's `edges`, `edge_parents` and `edge_deps` (written files have none: their rows cascaded).
4. **S0** is A plus the holders of the deps just read. The holders of a key are the files whose `edge_names` hold it.
5. **Round k:**
   - decode S_k from this transaction's rows;
   - call `resolve_files(&S_k, scheme, paths, Some(&A))` with reads recorded, giving R_k;
   - S_{k+1} is S_k plus the holders of R_k.
6. Stop when S_{k+1} equals S_k, and insert the last round's edges, parents and reads for A.

`resolved` is |A|.

**Paths and probes.** When `update_paths` finds `paths_fp` moved, it escalates to a walk, as for the stamp and `scheme_fp` (`src/write.rs:87-93`). The README's "a `[paths]` change needs `update`" then becomes the store's own check. Named paths are classified by one `probe_paths` call: an unclean path or a `Dir` walks, as today. A second call probes the stored files under a vanished directory (`probe_targets`, `:329-349`).

**Measurement, optional.** Run `specengine-eval scale --nodes 1000,10000,100000 --seed 1 --budget 10000` in release at the base and at the head, with labels `base` and `head`. Report `index.build_ms`, `update_us` and the four read medians in "Implementation". Only the build should move, by the full pass. Reads are unchanged until RM-0079, and the bars stay with RM-0054. Release runs grow `target.noindex`: clean it at the slice's end.

## Data

### `specengine.toml`

There is no new key. The `[paths]` keys `features`, `link_base`, `roots` (defaults applied, as written, duplicates dropped) and `exclude` now enter `paths_fp`. Changing any of them recomputes every edge on the next `update`. `spec`, `records`, `archive` and `generated` count only through the default `roots`. `tier0`, `tier1_name` and `index` do not count.

### The model (stored JSON of `nodes.node` and `links.link`)

- `ParentRef` gains `alias_of`, `project`, `scope`, `section` and `written_id`. Each is `Option<String>` with `default` and `skip_serializing_if = "Option::is_none"`.
  - The front-matter parser fills them from the `Reference` it already parses (core `src/front_matter.rs:244-252`), escaped values included.
  - A section's containment parent (core `src/lib.rs:354-358`) leaves them `None`.
- `Reference` gains `written_id: Option<String>`, also skipped when `None`.
  - It is the ID's text as written: prefix and body, with no qualifier, section, revision or wiki brackets.
  - It is set only when that text differs from `id`: a look-alike normalised, or an alias's body normalised (model `src/grammar.rs:337-385`).

A plain parent's JSON is unchanged. A scoped and sectioned one gains keys:

```json
{"id":"R-02","span":[31,37]}
{"id":"AC-01","span":[31,50],"scope":"stamina","section":"AC-02"}
```

Core then reads only the parse:
- `parent_reference(parent)` builds its `Reference` from these fields, and the bytes are no longer read.
- `bare` is `written_id`, else `id`.
- `written` stays as a parameter of `resolve`, `resolve_mention`, `resolve_detached` and `holders_of`, and of the internal `resolve_named`, but it no longer decides a lookup.

### Tables (store `src/schema.rs`, after `diagnostics`)

```sql
-- worktrees gains: paths_fp TEXT NOT NULL
CREATE TABLE edges (
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE, -- Edge.file, the citing file
  ord INTEGER NOT NULL,          -- position among the file's edges, core's order
  n INTEGER NOT NULL,            -- position among the written end's nodes; 0 when it has none
  holder INTEGER NOT NULL,       -- Edge.holder.ord, in the citing file
  type TEXT NOT NULL,            -- Edge.link_type
  origin TEXT NOT NULL,          -- frontmatter | inline
  names_source INTEGER NOT NULL, -- 1: `status: superseded-by X`, the written end is the source
  state TEXT NOT NULL,           -- LinkState::as_str of the written end
  reason TEXT,                   -- Edge::reason(): why it dangles, else its note
  label TEXT,                    -- JSON of Edge.edge (EdgeLabel); NULL when none
  at INTEGER NOT NULL,           -- Edge.offset
  end_path TEXT,                 -- the written end's n-th node: its file's path; NULL unless resolved
  end_ord INTEGER,               -- and its position
  PRIMARY KEY (file_id, ord, n)
) STRICT;
CREATE TABLE edge_parents (
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE, -- the document's file
  n INTEGER NOT NULL,            -- 0: the node it is listed under; 1..: Parent::Node.others
  state TEXT NOT NULL,           -- resolved | dangling | skipped
  reason TEXT,                   -- Parent::Dangling.reason
  end_path TEXT,
  end_ord INTEGER,
  PRIMARY KEY (file_id, n)
) STRICT;
CREATE TABLE edge_deps (         -- what the file's resolution read
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  key TEXT NOT NULL,             -- name:<text> | path:<root-relative path>
  PRIMARY KEY (file_id, key)
) STRICT;
CREATE TABLE edge_names (        -- what the file gives resolution
  file_id INTEGER NOT NULL REFERENCES files ON DELETE CASCADE,
  key TEXT NOT NULL,
  PRIMARY KEY (file_id, key)
) STRICT;
CREATE INDEX edges_by_holder ON edges (file_id, holder);
CREATE INDEX edges_by_end ON edges (end_path, end_ord);
CREATE INDEX edge_parents_by_end ON edge_parents (end_path, end_ord);
CREATE INDEX edge_deps_by_key ON edge_deps (key);
CREATE INDEX edge_names_by_key ON edge_names (key);
```

Rules for the rows:
- The other end of an edge is always the holder, `Nodes([holder])`, which is how `file_links` builds it, so it is not stored.
- An unresolved edge is one row with `n` 0 and NULL ends.
- A dangling edge's `reason` is its dangling reason. Core gives a dangling edge no note, so `reason` loses nothing.
- `edge_parents` holds core's resolved `parent:` before cycle breaking. There is no row for `Parent::None` (no `parent:`, or a failed front-matter) or for a file with no parse.
- `FORMAT_TABLES` (`:276-297`) gains the four tables before `files`.
- The dump's `TABLES` (`src/dump.rs:42-185`) gains four entries with `FILE_OWNER`, skipping `file_id`.

**Keys.** `given_keys(file)` lists exactly what `Resolver::of_sorted` (`check/resolve.rs:98-145`) indexes the file by:
- `path:<its path>`;
- `name:<id>` for its document `id:` and each `{#ID}` section;
- `name:<entry>` for each `aliases:` entry.

The reads a resolution records, hit or miss:

| Lookup (core) | Key recorded |
|---|---|
| place "anywhere" (`holders_in`, `:338-362`): the defined ID, the `aliases:` text and `bare`, the `aliases_from` target, and each name-fallback candidate with its target | `name:<text looked up>` |
| `slug/ID`: the feature document by slug | `path:<features>/<slug>.md` |
| a bare feature-scoped ID that dangles: the definers its reason lists (`unresolved`, `:408-444`) | `name:` of the ID, `bare` and the target |
| a Markdown file link's candidates (`resolve_link_path`, `check/links.rs:143-180`) | `path:<candidate>`, each tried |
| a path-form `canon:` (`canon_path`, `spec_graph.rs:834-871`) | `path:<path>` |
| the citing file, or a check that a file already named by a key holds an ID | none |

Anchors, sections, `#Y`, a canon target's class and a holder's positions are read in a file that one of these keys already brings in.

### Example

The config holds `R = { kind = "requirement", width = 2 }` and `AC = { kind = "criterion", width = 2, scope = "feature" }`, with `roots = ["docs"]` and `features = "docs/features"`. The tree follows; each comment line names a file and is not part of it, so offsets count from its `---`:

```markdown
<!-- docs/a.md -->
---
id: R-01
links:
  depends_on: [R-02, R-09]
---
# A

Needs R-02 and [the intro](b.md#intro).
<!-- docs/b.md -->
---
id: R-02
parent: stamina/AC-01
aliases: [OLD-2]
---
# B

## Intro
<!-- docs/features/stamina.md -->
# Stamina

### Delay {#AC-01}
```

| Table | Path | Columns |
|---|---|---|
| `edges` | `docs/a.md` | `0 0 0 depends_on frontmatter 0 resolved NULL NULL 35 docs/b.md 0` |
| `edges` | `docs/a.md` | `1 0 0 depends_on frontmatter 0 dangling "resolves to no ID and no alias" NULL 41 NULL NULL` |
| `edges` | `docs/a.md` | `2 0 0 mentions inline 0 resolved NULL NULL 62 docs/b.md 0` |
| `edges` | `docs/a.md` | `3 0 0 mentions inline 0 resolved NULL NULL 83 docs/b.md 0` (anchor `intro`, no ID section: the document) |
| `edge_parents` | `docs/b.md` | `0 resolved NULL docs/features/stamina.md 1` |
| `edge_deps` | `docs/a.md` | `name:R-02`, `name:R-09`, `path:docs/b.md` |
| `edge_deps` | `docs/b.md` | `path:docs/features/stamina.md` |
| `edge_names` | each file | `docs/a.md`: `name:R-01`, `path:docs/a.md`; `docs/b.md`: `name:OLD-2`, `name:R-02`, `path:docs/b.md`; `stamina.md`: `name:AC-01`, its path |

One dump line: `edges\t["demo","<root>","docs/a.md",0,0,0,"depends_on","frontmatter",0,"resolved",null,null,35,"docs/b.md",0]`.

Two edits on this tree:
- **Deleting `docs/features/stamina.md`.** N is `name:AC-01` and its path, so A is `docs/b.md` and `resolved` is 1. Its row becomes ``dangling "resolves to no feature document `docs/features/stamina.md`"``.
- **Editing the prose of `docs/a.md`.** A is that file alone.

### Core API (`specengine_core::check`)

- `EdgeKey {Name(String), Path(String)}`, `Ord`; `text()` gives `name:…` or `path:…`, and `parse(&str)` is its inverse.
- `given_keys(&CheckFile) -> Vec<EdgeKey>`, sorted and unique.
- `resolve_files<'a>(&'a CheckInput, &'a IdScheme, &'a Paths, only: Option<&BTreeSet<&str>>) -> ResolvedFiles<'a>`. It returns `{paths: Vec<&'a str>}`, which `file` and every `NodeAt.file` index, and `files: Vec<FileEdges {file, parent: Option<Parent>, edges: Vec<Edge>, reads: Vec<EdgeKey>}>`, in path order. `None` means every file.
  - `SpecGraph::build` calls the same per-file routine, with recording off: recording costs `SpecGraph::new` nothing.
- `Paths::resolution_json() -> String` is the JSON `{"features":…,"link_base":…,"roots":[…],"exclude":[…]}` in that key order. `paths_fp` is the store's `hash_bytes` of it.

### Store API

- `Source::paths(&self) -> &Paths`, required. `WorkingTree` and `GitIndex` keep the `Paths` they were made with; the test `Reversed` delegates.
- `Source::probe_paths(&self, &[&str]) -> Vec<Probed>`, with `Probed {Listed, Unlisted, Dir}`. `Dir` means `is_dir`, else `Listed` means `probe`, else `Unlisted`. The default goes path by path, and `WorkingTree` overrides it with a listing memo scoped to the call.
- `UpdateReport.resolved: usize` and `resolved_all: bool`, both `#[serde(skip)]`.
- `SqliteIndex::stored_edges() -> StoredEdges {edges, parents}`, an inherent like `dump`, read in one deferred transaction:
  - `StoredEdge {path, ord, holder, link_type, origin, names_source, state: LinkState, reason, label: Option<EdgeLabel>, at, ends: Vec<(String, usize)>}`, by `(path, ord)`;
  - `StoredParent {path, state, reason, ends}`, by path.
  - It is for the tests and for RM-0079; no read calls it here.
- `INDEX_FORMAT = 16`. Its doc line names the four tables and their indexes, `worktrees.paths_fp`, `ParentRef`'s five fields and `Reference.written_id`, citing this spec, "Data".
- `tests/format_history.txt` gains `16 <hash>`.

### Lookups (store `src/edges.rs`, `pub(crate)` SQL for RM-0079)

Each is filtered to the worktree through `files.wt` and ordered by `(path, ord, n)`:

| Statement | Selects | Core's term |
|---|---|---|
| `HELD` | `file_id = ?, holder = ?` | `held` |
| `OUTGOING` | held with `names_source = 0`, UNION ALL ending with `names_source = 1` | `from` |
| `INCOMING` | ending with `names_source = 0`, UNION ALL held with `names_source = 1` | `into` (`index_edges`, `spec_graph.rs:1015-1029`) |
| `PARENTED` | `edge_parents` ending at `(path, ord)` | |
| `DEPENDENTS` | `edge_deps` whose `key` is in a list | maintenance |
| `HOLDERS` | `edge_names` whose `key` is in a list | maintenance |

## Rules and edge cases

- **R1.** WHEN any update commits, the worktree's four edge tables SHALL dump exactly as a rebuild of the same tree does. This is the store's hard property, extended to them.
- **R2 (E2).** Only `resolve_files` decides an end, a state, a reason or a key. The store's SQL SHALL only match key text and project columns. OUTGOING and INCOMING are `Edge::written_end` and `index_edges`, transcribed, and AC-12 compares them with core's.
- **R3 (E1).** WHEN the stamp is recreated, the worktree is new, `scheme_fp` or `paths_fp` moves, or a `rebuild` runs, every edge SHALL be recomputed. A `paths_fp` move alone SHALL re-parse nothing (`parsed` 0).
- **R4, the closure.** A resolution SHALL equal the resolution over the whole worktree when every key it read has every holder in its input. The fixpoint stops only when that holds. A file outside A read no key in N, so its stored rows still hold: every lookup it made answers as before. Rounds are unbounded, because each round adds files or stops.
- **R5.** WHEN no file's rows are written or deleted, no edge row SHALL be read or written, and `resolved` is 0.
- **R6.** The edge pass SHALL run inside the update's one `Immediate` transaction, after the file rows. A concurrent writer is serialised by it, and a reader sees file and edge rows of one commit. The full pass holds the write lock for its resolution: at 100 000 nodes about 0.4 s (decode 249 ms, graph 128 ms) plus the inserts. WAL readers are not blocked.
- **R7.** Tier 3, generated, failed-front-matter and unreadable files are stored like any other file: standing is a read's filter (`spec-cli-graph.md` "Live sources"). A file with no parse has `path:` as its only name and has no edges.
- **R8 (E3).** No statement SHALL order by rowid or rely on insertion order. `ord` and `n` are core's orders, and decoded subsets are sorted by path.
- **R9 (E4).** Nothing a read computes SHALL be stored: written text, lines, cycle breaks, children order, ancestors, walks, admission, `left_out`, bundle layers.
- **R10.** Nothing is fatal (ADR-0012): a resolution never fails an update. A label JSON encode error is `StoreError::Sqlite`, as the README's accepted minor.
- **R11.** D5 SHALL change `spec check` only where today's resolution read bytes the parse lost:
  - an escaped `parent:`, such as `parent: "feat\/AC-01"`, keeps its qualifiers, so its false `ref-dangling` goes;
  - a wiki-form or escaped reference written with look-alikes resolves through an `aliases:` entry spelled the same way.

  Every other finding and every check parity test stays as it is.
- **R12.** WHEN `update_paths` finds `paths_fp` moved, it SHALL walk. `probe_paths` SHALL list each directory at most once per call, and its results SHALL equal path-by-path probing on an unchanged tree.

## Acceptance criteria

Store tests run on scratch copies under `std::env::temp_dir()`. Any non-ASCII text in a test is written as escapes (`\u{0422}`) only. M names the mutations that must turn each criterion red.

- [ ] AC-1 [default, store] Seeded edits equal a rebuild. In `tests/edges.rs`, at least 40 seeds of 25 steps each run over a generated tree of 12-20 files (xorshift64\*, as core `tests/check_rules.rs:1943`).
  - **Config:** `R` project-scoped, `AC` feature-scoped, `TERM` `shape = "name"`, a legacy prefix with `aliases_from` R, and `link_base`.
  - **Steps cover:**
    - add, delete and rename files, also into and out of `features`, and an upper-case stem;
    - add and remove an ID section, an `id:` and a duplicate ID;
    - add and remove `aliases:` entries, Latin and look-alike;
    - mentions: bare, `slug/ID`, `slug/ID#Y`, bare feature-scoped inside and outside its feature, legacy, a name-fallback suffix, wiki, `project:`;
    - path links: relative, `/`-led, through `link_base`, `#anchor`, `%20`;
    - `canon:` with and without an anchor, its target canon and not;
    - `parent:`: plain, scoped, sectioned, `project:` and escaped;
    - `status: superseded-by`;
    - a failed front-matter, invalid UTF-8, and a `chmod 000` file;
    - edits of `[paths] features`, `link_base` and `exclude`, and an `[ids]` edit.
  - After each step, half through `update` and half through `update_paths` of the changed paths (a config edit walks), some through `Reversed`: `dump_worktree` equals a fresh DB's.
  - A tally SHALL show each listed case at least once in the run, and a failure prints its seed and step.
  - The existing dump tests (`incremental.rs`, `concurrency.rs`, `worktrees.rs`, `format.rs`) stay green unchanged.
  - M: dependents found by new names only, so a deleted definer keeps its edge; the fixpoint cut to one round; `paths_fp` ignored; a `features` edit leaves its edges.
- [ ] AC-2 [default, store; eval] Core's edges.
  - On each of the 18 fixtures with a root `specengine.toml`, and after every AC-1 step, `stored_edges` equals both of these on every stored column, grouped by file:
    - `SpecGraph::new(&indexed_input)`;
    - `SpecGraph::new(&check_input(&WorkingTree, &scheme))`, from the bytes.
  - Parents are compared too. A document whose `SpecGraph::parent` is not `Cycle` equals its stored row, and `Parent::None` means no row. A cycle's breaker has a `resolved` row whose first end is a member of its cycle.
  - The same holds in eval `tests/scale_edges.rs` on `scale --corpus-only --nodes 1000 --seed 1`'s tree, before and after an `update_paths` of 20 seeded part-file edits (RM-0078 "Done").
  - M: `state` taken from the target of a `names_source` edge; `n` in reverse; `label` dropped; `ParentRef` without `scope` (the byte comparison fails on a scoped parent); `bare` from `written` again (it fails on a look-alike alias).
- [ ] AC-3 [default, store] The format stamp.
  - `INDEX_FORMAT` is 16, and `format_history.txt` ends `16 <hash>`, which equals the hash of the five `DUMPED` fixtures' dump (`tests/format.rs:152-158`).
  - That dump holds `edges\t`, `edge_parents\t`, `edge_deps\t` and `edge_names\t` lines.
  - `a_rewritten_stamp_reparses_everything_and_equals_a_fresh_index` (`:72`) stays green, and its fresh comparison now includes the edges.
  - M: the stamp left at 15; a table missing from `TABLES` (no line); `edges` missing from `FORMAT_TABLES` (a rewritten stamp fails to recreate it).
- [ ] ET-1 [default, core] Keys are complete. In `tests/edge_keys.rs`, over at least 200 seeded `CheckInput`s with AC-1's coverage, and over every fixture:
  - for every file G, `resolve_files` over {G} plus the holders (by `given_keys`) of G's reads over the whole input equals the whole input's: edges with ends as `(path, ord)`, parent, and reads;
  - the edges of `SpecGraph::new` equal `resolve_files(None)`'s, concatenated in path order.
  - M: the `aliases:` lookup unrecorded; fallback candidates unrecorded; the slug's path unrecorded; the `link_base` candidate unrecorded; `given_keys` without section IDs.
- [ ] ET-2 [default, store] Incremental work. On a designed tree of 40 files, where F is cited by exactly G1 (by ID and by an alias) and G2 (by a path link with an anchor):
  - editing F's prose gives `resolved` 3, and deleting F gives 2;
  - an update with no change gives `resolved` 0 and `resolved_all` false;
  - a `link_base` edit then `update` gives `resolved_all` and `parsed` 0;
  - an `[ids]` edit gives `reparsed_all` and `resolved_all`;
  - `update_paths` of one file after a `[paths]` edit gives `walked` equal to every file;
  - `serde_json::to_value(&report)` has exactly today's keys (D9).
  - M: a full pass on every update (`resolved` 40); dependents found by new names only (deleting F gives 0).
- [ ] ET-3 [default, core] Parse-time capture.
  - A document with `parent: stamina/AC-01#AC-02` parses to a `ParentRef` with `scope` `stamina` and `section` `AC-02`.
  - `parent: "stamina\/AC-01"`, escaped, in a feature-scoped config resolves, and `spec check` gives no `ref-dangling`, where the base gives one.
  - The JSON of `parent: R-01` is `{"id":"R-01","span":[…,…]}` with no new key.
  - An inline and a wiki mention written `\u{0422}ERM-x` both resolve through `aliases: ["\u{0422}ERM-x"]`, with byte and indexed input alike.
  - M: the parser drops `scope`; `bare` from `written`.
- [ ] ET-4 [default, store unit] Batched probes. In `src/source.rs` `#[cfg(test)]`, a counter of directory listings shows that `update_paths` naming 2 000 files in one directory of 2 000, plus 50 in a nested one, lists each directory at most twice. The dump equals a walk's. M: `WorkingTree` without the override (path-by-path `entry_kind`).
- [ ] AC-12 [default, store unit; store half] Lookups. In `src/edges.rs` `#[cfg(test)]`, as `src/symbols.rs:181-195`:
  - `EXPLAIN QUERY PLAN` of each of the six statements shows a `SEARCH` and no `SCAN` of a table (`CONSTANT ROW` aside), on an empty DB;
  - each statement's cost at 100 000 SQL-filled `edges` rows (and as many key rows) is at most 3x its cost at 10 000, with a 2 ms floor, best of three of 20 rounds;
  - HELD, OUTGOING and INCOMING, run for every node of every fixture, return exactly the edges whose holder, source or target in `SpecGraph::new`'s `edges()` holds that node.
  - M: drop `edges_by_end` (INCOMING scans); drop `edge_deps_by_key`; OUTGOING ignoring `names_source`.
- [ ] F1 — form:
  - clippy and fmt are clean, and there is no new dependency;
  - the implementation touches no `docs/`;
  - `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

- Moving reads onto the table: `show`, `--links`, `bundle`, `tree`, `graph`, `impact`, `flow` and `refs` resolve per call as today (RM-0079).
- Trust, the stat key, the dirty mark and hub writers (RM-0054).
- Storing written text, lines, cycle breaks, children order, ancestors, walks or admission (E4).
- Resolution in SQL or SQL recursion (E2).
- `project:` resolution, which stays `Skipped`.
- An escalation of `update_paths` by count, and a file watcher.
- New `spec index` output, and the code tables.

## Implementation

To fill in after implementation. Code comments cite this spec's "Data" or "Acceptance criteria" headings: the new canon page exists only from the ship commit (`doc_pointers.rs`).

### Files in scope

| Module | What it changes |
|---|---|
| model `src/node.rs`, `src/reference.rs`, `src/grammar.rs` | `ParentRef`'s five fields; `Reference.written_id`, set from the written slice (`:337-385`); 9 `Reference` and 3 `ParentRef` literals across the crates |
| core `src/front_matter.rs` | `parent:` keeps the parsed qualifiers (`:244-252`) |
| core `src/check/resolve.rs` | `parent_reference` and `bare` read no bytes; the read recorder, off by default; `EdgeKey`, `given_keys` |
| core `src/check/spec_graph.rs`, `check/mod.rs`, `src/paths_toml.rs` | `resolve_files`, `FileEdges`, `ResolvedFiles`, with `build` on the same routine; exports; `Paths::resolution_json`, and the `link_base` comment |
| store `src/schema.rs`, `src/dump.rs`, `src/lib.rs` | the tables, indexes, `paths_fp`, `FORMAT_TABLES`, `TABLES`; `INDEX_FORMAT` 16 and its line; the `UpdateReport` fields; the module doc ("resolves nothing" goes) |
| store `src/edges.rs` (new), `src/write.rs`, `src/read.rs`, `src/rows.rs` | full and incremental passes, the fixpoint, the projection and `stored_edges`, the lookups; names with the file rows; old names before deletion; the escalation; the subset decode sharing `rebuild_parsed` (`rows.rs:227`) |
| store `src/source.rs` | `paths`, `probe_paths` and `Probed`; the listing memo; the ET-4 counter |
| tests | store `tests/edges.rs` (new), `tests/common/mod.rs` (`Reversed::paths`, the generator), `tests/format_history.txt`; core `tests/edge_keys.rs` (new) and ET-3; eval `tests/scale_edges.rs` (new) |

The rust-developer writes `src/` and the test-engineer writes the tests, both against "Data", so they can run side by side. `Reversed` needs `paths` before the store tests compile.

### Canon at ship

Nothing below is written before the ship commit. Free bytes are as of this spec, and no cap is raised.
- **New `docs/canon/spec-index-edges.md`** (Tier 2, scope `crates/specengine-store`): the tables, keys, passes, R1-R12 and the lookups. The `spec-*` glob puts its index line in `docs/index-canon-spec.md` (5 168 B), because `docs/index-canon.md` has 81 B free.
- **Store README** (51 B free): the intro (core's edges, incrementally), "Rows" (a pointer), "Cache key and format stamp" (`paths_fp`, 16), the API (`Source::paths`, `probe_paths`, `stored_edges`, the report fields), and the `entry_kind` minor gone. To fit, move "Rows"' column detail to the new page.
- **Core README** (3 B free) and **model README** (453 B free): one line each for `resolve_files`/`EdgeKey` and `written_id`/`ParentRef`, with the detail on the new page.
- **`docs/canon/spec-check-links.md`** (15 B free): "`parent:`" loses its byte re-read and its limit, and "Open owner questions" loses that bullet. The text shrinks.
- **`docs/canon/spec-check.md`** (3 B free): "Rules" changes "An alias `parent` is re-read through the scheme" to "`parent:` keeps the parser's qualifiers", at no greater length.
- **`docs/canon/spec-cli-graph.md`** "Input": `parent:` is read from the parse; no read uses the edges yet.
- **Plan:** RM-0078 shipped; 08's row 12c; `CLAUDE.md`'s state line; `docs/canon/scale-eval.md` "Tests" names `scale_edges.rs`.
