---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-core, crates/specengine-model]
owner: owner
reviewed: 2026-11-04
---

# spec index: core's resolved edges, kept incrementally

Since `INDEX_FORMAT` 16 (`docs/features/edge-table.md`, RM-0078) the index stores core's resolution of every file's links beside the file rows, under the store's hard property: after any sequence of edits the incremental index equals a rebuild, row for row (store README), these tables included. They are rebuildable rows outside the repository (ADR-0001, ADR-0003).

- **E1.** The tables are a pure function of the stored rows, the `[ids]` fingerprint and the `[paths]` keys resolution reads.
- **E2.** One core decides every end, state, reason and key (`check::resolve_files`); the store's SQL only matches key text and projects columns.
- **E3.** No statement orders by rowid: `ord` and `n` are core's orders, decoded subsets are sorted by path.
- **E4.** Nothing a read computes is stored: written text, lines, cycle breaks, children order, ancestors, walks, admission, `left_out`, bundle layers.

**Reads.** `show`, `--links` and `bundle` read a subset of them (`spec-read-subset.md`); `tree`, `graph`, `impact`, `flow` and `refs` still resolve per call over the whole input (`spec-cli-graph.md` "Input"); `spec index` prints as before.

## Tables

`STRICT`, keyed by the citing file, `file_id` cascading (store `schema.rs`); `worktrees` gains `paths_fp`. Indexes: `edges_by_holder (file_id, holder)`, `edges_by_end (end_path, end_ord)`, `edge_parents_by_end (end_path, end_ord)`, `edge_deps_by_key`, `edge_names_by_key` (on `key`).

- **`edges`**, key `(file_id, ord, n)`, one row per end node. `ord`: the position among the file's edges, core's order; `n`: among the written end's nodes. `holder`: `Edge.holder.ord`; the other end is always `Nodes([holder])` (`file_links`), unstored. `type`; `origin` `frontmatter | inline`; `names_source` 1 for `status: superseded-by X` (the written end is the source); `state`, `LinkState::as_str` of the written end; `reason`, `Edge::reason()` (why it dangles, else its note); `label`, the JSON of `EdgeLabel` or NULL; `at`, `Edge.offset`; `end_path`, `end_ord`, the n-th end as a text pair, never a surrogate key (a target's rows are re-inserted on each edit), NULL unless resolved: an unresolved edge is one row, `n` 0. No written text or line: those come from the citing file's bytes.
- **`edge_parents`**, key `(file_id, n)`: core's resolved `parent:` before cycle breaking. `n` 0 is the node it is listed under, 1.. `Parent::Node.others`; `state` `resolved | dangling | skipped`, `reason` the dangling one, `end_path`, `end_ord`. No row without a `parent:`, with a failed front-matter, or without a parse.
- **`edge_deps`** and **`edge_names`**, key `(file_id, key)`: what the file's resolution read, and what the file gives resolution.

`FORMAT_TABLES` lists the four before `files`; the dump (`dump.rs` `TABLES`) owns them by `(project, root, path)`, `file_id` skipped. Tier 3, generated, failed and unreadable files are stored like any other: standing is a read's filter (`spec-cli-graph.md` "Live sources"). A file without a parse gives `path:` as its only name and has no edges.

## Keys

`EdgeKey {Name(String), Path(String)}`, `Ord`; `text()` is `name:<text>` or `path:<root-relative path>`, `parse` its inverse. `given_keys(&CheckFile)`, `given_keys_of(path, Option<&ParsedFile>)` for the store's rows, is exactly what `Resolver` indexes a file by, sorted and unique: `path:<its path>`, and `name:` of its document `id:`, each `{#ID}` section and each `aliases:` entry; `edge_names` is written with the file rows. A resolution records each lookup it makes, hit or miss, as made: a defined-ID hit records nothing after it.

| Lookup (core) | Key recorded |
|---|---|
| anywhere (`holders_in`): the defined ID, the `aliases:` entry (the ID, then `bare`), the `aliases_from` target; each name-fallback candidate the same | `name:<text>` |
| `slug/ID`: its feature document | `path:<features>/<slug>.md` |
| in one file (a slug's document, the citing feature): the ID when `bare` differs (whether `bare` is tried hangs on any file's `aliases:` holding the ID) | `name:<id>` |
| a bare feature-scoped ID that dangles: the definers its reason lists | `name:` of the ID, `bare`, the target |
| a Markdown link's candidates (`resolve_link_path`), each tried | `path:<candidate>` |
| a path-form `canon:` | `path:<path>` |
| the citing file; anchors, sections, `#Y`, a canon's class, positions in a file named above | none |

Recording is off in the check and every read (`Reads::off`, one branch per lookup), on only in `resolve_files`; passed as `&mut Reads`, it keeps `Resolver` and `SpecGraph` `Sync`.

## Parse-time capture

Resolution reads the parse alone; bytes give only the text a finding quotes. `ParentRef` keeps, beside `id` and `span`, the parsed reference's `alias_of`, `project`, `scope`, `section` and `written_id`, escaped values included (a section's containment parent: none); `Reference.written_id` is the ID as written when it differs from `id` (model README). Each is `Option`, skipped when `None`, so a plain parent's JSON is unchanged. Core's `parent_reference` builds from the fields, its script from `written_id`, else `id`; `bare`, the text tried as an `aliases:` entry, is `written_id`, else `id` (CLI `show`'s `bare_text` alike); a `status: superseded-by` target is tried by its ID alone. So `indexed_input`, with empty `bytes`, resolves the check's graph. This closed the open question of an escaped `parent:` at the cost it listed, a model and `INDEX_FORMAT` change: `parent: "feat\/AC-01"` resolves, its message keeping scope and section, and a wiki or escaped look-alike resolves through an `aliases:` entry spelled alike.

## Passes

`update`, `update_paths` and `rebuild` keep the tables in their one `Immediate` transaction, after the file rows (store `write.rs`, `edges.rs`): a reader sees one commit's file and edge rows; a no-op opens no transaction (`spec-read-freshness.md` "No-op updates"). Nothing is fatal (ADR-0012): an encode error is the README's `Sqlite` minor.

**Full pass**, when the stamp was recreated, the worktree is new, `scheme_fp` or `paths_fp` moved, or on a rebuild: the worktree's `edges`, `edge_parents` and `edge_deps` go, every stored file is decoded in the transaction (`read.rs` `decode_files`, `indexed_input`'s), and `resolve_files(&input, scheme, source.paths(), None)` is inserted. `resolved` is the file count, `resolved_all` true. A `paths_fp` move alone re-parses nothing.

**Incremental pass**, otherwise, over the paths whose rows the transaction deleted or wrote; none: no edge row is read or written, `resolved` 0.
1. N, the changed names: those paths' `edge_names`, read before deletion, and the written rows'.
2. A, by path (file ids change on insertion): the written paths and the files whose `edge_deps` hold a key in N, minus the deleted.
3. A's `edge_deps` are read, then A's rows deleted.
4. S is A plus the holders (`edge_names`) of those keys. Each round decodes S's new members, runs `resolve_files(&S, …, Some(&A))` and adds the holders of every key read; once S stops growing, that round's rows for A are inserted. `resolved` is |A|.

**Why it equals a rebuild (R4).** A file's resolution over a subset equals the whole worktree's when every key it read has every holder in the subset; the fixpoint stops only then. A file outside A read no key in N, so each lookup it made answers as before and its rows stand. Rounds are unbounded: each grows S or stops.

## `[paths]` fingerprint and probes

`paths_fp` is BLAKE3 of `Paths::resolution_json()`, `{"features":…,"link_base":…,"roots":[…],"exclude":[…]}` in that order (`link_base` `null` when absent, `roots` with defaults applied): `spec`, `records`, `archive`, `generated` count only through the default `roots`; `tier0`, `tier1_name`, `index` not at all. A move recomputes every edge and escalates `update_paths` to a walk, as the stamp and `scheme_fp` do; the worktree row then takes both fingerprints.

`Source::paths()` (required) gives the writer the `Paths` it walks by, so no `IndexWriter` signature changed. `update_paths` classifies its named paths in one `Source::probe_paths(&[&str]) -> Vec<Probed>`: `Dir` by `is_dir`, else `Listed` by `probe`, else `Unlisted`; a `Dir` or an unclean path walks. A second call probes the stored files under a vanished directory; one now a directory counts as unlisted. The default probes path by path; `WorkingTree` keeps a listing memo per call, so a directory is listed at most once a call, the answers unchanged. No escalation by count.

## API

- Core `check`: `resolve_files<'a>(&'a CheckInput, &'a IdScheme, &'a Paths, only: Option<&BTreeSet<&str>>) -> ResolvedFiles {paths, files: [FileEdges {file, parent: Option<Parent>, edges, reads}]}`, in path order, `None` every file; `SpecGraph::build` runs the same per-file `file_links`, recording off.
- Store: `UpdateReport.resolved`, `resolved_all`, `#[serde(skip)]`; `SqliteIndex::stored_edges() -> StoredEdges {edges, parents}`, one snapshot, for tests: `StoredEdge` (core's `EdgeRow`: the columns, `origin: LinkOrigin`, `state: LinkState`, `label: Option<EdgeLabel>`, `ends: [(path, ord)]`) by `(path, ord)`, `StoredParent {path, state, reason, ends}` (`ParentRow`) by path.
- `edges.rs`'s `pub(crate)` lookups, `IndexSnapshot`'s but `PARENTED` and `DEPENDENTS` (`spec-read-subset.md` "API"), by `files.wt`, ordered `(path, ord, n)`, each a `SEARCH`: `HELD` (core's `held`: worktree, path, position, `INDEXED BY edges_by_holder`), `OUTGOING` (`from`: held with `names_source` 0, UNION ALL ending with 1), `INCOMING` (`into`: ending with 0, UNION ALL held with 1), each one row per end, grouped by `(path, ord)` by the reader; `PARENTED`; `DEPENDENTS` and `HOLDERS`, one key per statement (a `json_each` or `carray` list plans as a `SCAN` or needs a rusqlite feature).

## Example

`R = { kind = "requirement", width = 2 }`, `AC = { kind = "criterion", width = 2, scope = "feature" }`, `roots = ["docs"]`, `features = "docs/features"`; a comment names a file, offsets count from its `---`:

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

- `edges` of `docs/a.md`: `0 0 0 depends_on frontmatter 0 resolved NULL NULL 35 docs/b.md 0`; `1 0 0 depends_on frontmatter 0 dangling "resolves to no ID and no alias" NULL 41 NULL NULL`; `2 0 0 mentions inline 0 resolved NULL NULL 62 docs/b.md 0`; `3 0 0 mentions inline 0 resolved NULL NULL 83 docs/b.md 0` (anchor `intro`, no ID section: the document).
- `edge_parents` of `docs/b.md`: `0 resolved NULL docs/features/stamina.md 1`.
- `edge_deps`: `docs/a.md` `name:R-02`, `name:R-09`, `path:docs/b.md`; `docs/b.md` `path:docs/features/stamina.md`.
- `edge_names`: `name:R-01`, `path:docs/a.md`; `name:OLD-2`, `name:R-02`, `path:docs/b.md`; `name:AC-01` and its path.

Deleting `stamina.md`: N is `name:AC-01` and its path, A is `docs/b.md` (`resolved` 1), its parent ``dangling "resolves to no feature document `docs/features/stamina.md`"``. Editing `docs/a.md`'s prose: A is that file alone.

## Known limits

- `resolve_files` runs without `catch_unwind`: a core panic on stored input would fail every update, where it failed one read. Resolution is trusted not to panic, as `spec check` is (awaiting the owner).
- **Hub fan-out.** A file citing every document, such as a generated `index*.md`, depends on every `path:` key: any edit puts it in A and its fan-out in S. At 100 000 nodes `update_paths` of one edited leaf took 409 ms beside such a file and 519 ms of that file, against 7.0 ms without one (`read-trust` AC-14, release, best of 3): under 1 s, no plan item.
- The full pass holds the write lock while it resolves, about 0.4 s at 100 000 nodes (decode 249 ms, graph 128 ms, `scale-eval.md` "Baseline") plus the inserts.
- The `probe_paths` memo is keyed by spelling: on case-insensitive APFS a directory may be listed once per spelling, the answers unchanged.
- `project:` stays `Skipped`.
