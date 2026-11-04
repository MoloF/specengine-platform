---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-store, crates/specengine-core, crates/specengine-model]
ref: RM-0078; Phase 7 step 12c (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split and D1-D10 awaiting the owner's review; two iterations
adrs: []
---

# edge-table: core's resolved edges kept in the index, incrementally (RM-0078)

## Why

Every read parses the whole corpus and resolves every link to answer for one node: at 100 000 nodes, reparse, decode, graph and resolver take 1.2 s of a 1.6 s bundle (`docs/canon/scale-eval.md` "Baseline"). RM-0079 can read a subset from the index only if the edges are already there. This slice stores core's resolution in the index, kept equal to a rebuild row for row, with `INDEX_FORMAT` 16. Reads do not change.

Decided here, each awaiting the owner's review:
- **D1:** core resolves (`check::resolve_files`, `SpecGraph::build`'s per-file routine), and the store only stores.
- **D2:** four tables keyed by the citing file, with ends as `(path, ord)` text and one row per end node.
- **D3:** no written text or line is stored; RM-0079 reads them from the bytes.
- **D4:** dependency keys are the lookups resolution made, closed by a fixpoint (R4).
- **D5:** `ParentRef`'s qualifiers and `Reference.written_id` are captured at parse time. This closes the open question "An escaped `parent:` loses its qualifiers" in `docs/canon/spec-check-links.md` at the cost it listed: a model and `INDEX_FORMAT` change.
- **D6:** `paths_fp` covers `features`, `link_base`, `roots` and `exclude`.
- **D7:** `Source::paths()` supplies the paths, so no `IndexWriter` signature changes.
- **D8:** batched probes with no count threshold.
- **D9:** the reports are unchanged, and the new counts are `#[serde(skip)]`.
- **D10:** no ADR. These are rebuildable index rows (ADR-0001, ADR-0003), no rule in `docs/canon/architecture.md` changes, and D5 takes the answer the open question had already priced.

## Description and interactions

How it works now is in `docs/canon/spec-index-edges.md`. Also changed:
- the store, core and model READMEs;
- `docs/canon/spec-check-links.md`, `spec-check.md` "Rules", `spec-cli-graph.md` "Input" and `scale-eval.md` "Tests";
- the format number in `symbol-index.md` and `layer-c.md`.

No read, `spec index` output, MCP tool or route changes.

## Data

- **Tables:** `edges`, `edge_parents`, `edge_deps` and `edge_names`, five indexes, and `worktrees.paths_fp`. `INDEX_FORMAT` is 16, and `tests/format_history.txt` has a `16` line. The columns, keys and worked example are in the canon page: "Tables", "Keys" and "Example".
- **Model JSON:** `ParentRef` gains `alias_of`, `project`, `scope`, `section` and `written_id`. `Reference` gains `written_id`, placed after `alias_of`. All are skipped when `None`.
- **`specengine.toml`:** no new key. Four `[paths]` keys enter `paths_fp`: `features`, `link_base`, `roots` and `exclude`.

## Rules and edge cases

R1-R12 hold as specified; the canon page states them as they stand now. R11 held exactly. `spec check` changed only in two cases:
- an escaped `parent:` no longer gives a false `ref-dangling`;
- a wiki-form or escaped look-alike resolves through an `aliases:` entry spelled the same.

## Acceptance criteria

Store tests run on scratch copies under `std::env::temp_dir()`, with non-ASCII text written as escapes. M names the mutations each criterion must turn red. All 23 were killed, including a revert of the iteration-2 fix.

- [x] AC-1 [default, store] Seeded edits equal a rebuild (`tests/edges.rs`).
  - 40 seeds of 25 steps run over trees of 12-20 files. The config has a project-scoped `R`, a feature-scoped `AC`, a name-shaped `TERM`, a legacy prefix and `link_base`.
  - The steps add, delete and rename files. They cover IDs, aliases (Latin and look-alike), every mention, path link, `canon:` and `parent:` form, broken and unreadable files, and `[paths]` and `[ids]` edits.
  - After each step, `dump_worktree` equals a fresh DB's, whether the step ran through `update` or `update_paths`.
  - A tally shows all 58 named cases.
  - M: dependents found by new names only; the fixpoint cut to one round; `paths_fp` ignored; a `features` edit leaving its edges.
- [x] AC-2 [default, store; eval] Core's edges.
  - On the 19 configured fixtures and after every AC-1 step, `stored_edges` and the parents equal `SpecGraph::new` on every stored column. This holds over `indexed_input` and over the bytes.
  - The same holds in eval `tests/scale_edges.rs` on the 1k `scale` corpus, before and after 20 seeded edits.
  - M: `state` taken from a `names_source` edge's target; `n` reversed; `label` dropped; `ParentRef` without `scope`; `bare` from `written`.
- [x] AC-3 [default, store] The format stamp.
  - `INDEX_FORMAT` is 16, and the `16` history line equals the `DUMPED` fixtures' dump, which holds rows from all four tables.
  - M: the stamp left at 15; a table missing from `TABLES` or from `FORMAT_TABLES`.
- [x] ET-1 [default, core] Keys are complete (`tests/edge_keys.rs`), over 1 200 seeded inputs and the fixtures.
  - For every file G, `resolve_files` over {G} plus the holders of G's reads equals the whole input's.
  - `SpecGraph::new`'s edges equal `resolve_files(None)`'s.
  - M: any of these left unrecorded: the alias lookup, the fallback candidates, the slug's path, the `link_base` candidate; `given_keys` without section IDs.
- [x] ET-2 [default, store] Incremental work on a 40-file tree.
  - Editing F's prose gives `resolved` 3, and deleting F gives 2. No change gives 0.
  - A `link_base` edit gives `resolved_all` with `parsed` 0.
  - An `[ids]` edit gives `reparsed_all`.
  - After a `[paths]` edit, `update_paths` walks.
  - The report's JSON keys are unchanged.
  - M: a full pass on every update.
- [x] ET-3 [default, core] Parse-time capture.
  - Scoped and sectioned `parent:` values parse with their qualifiers.
  - An escaped `parent:` resolves, with no `ref-dangling`.
  - A plain parent's JSON is unchanged.
  - A `\u{0422}ERM-x` mention resolves through its alias, from bytes and from the index alike.
  - M: the parser dropping `scope`.
- [x] ET-4 [default, store unit] Batched probes. Naming 2 000 + 50 files lists each directory at most twice. M: `WorkingTree` without the override.
- [x] AC-12 [default, store unit] The lookups.
  - The six statements each plan as a `SEARCH` with no table `SCAN`.
  - Each costs at most 3x from 10 000 rows to 100 000.
  - HELD, OUTGOING and INCOMING equal core's `held`, `from` and `into` on every fixture node.
  - M: `edges_by_end` dropped; `edge_deps_by_key` dropped; OUTGOING ignoring `names_source`.
- [x] F1 — form. Clippy and fmt are clean, there is no new dependency, and the code touched no `docs/`. `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

- Reads on the tables (RM-0079).
- Trust and hub writers (RM-0054).
- Storing what a read computes.
- Resolution in SQL.
- `project:` resolution.
- A count threshold for `update_paths`, and a file watcher.
- New `spec index` output.

## Implementation

Review gave REWORK on iteration 1. The major finding: a one-file lookup of a look-alike written ID recorded no `name:<id>`, so a Latin-twin alias added elsewhere left an edge stale. Iteration 2 got ACCEPT with two nits, one left for RM-0079: `show`'s ignored `written` parameter.

- **Choices:** the developer's open choices are in `docs/canon/spec-index-edges.md`. Two are not:
  - `written_id` follows `alias_of` in JSON order;
  - the ET-4 counter is `cfg(test)`.
- **Clippy:** `Reference` grew by 24 B. The enums that hold it now allow `clippy::large_enum_variant`, and `Plan::Write` is boxed.
- **Limits awaiting the owner:** listed in the canon page's "Known limits":
  - no `catch_unwind` around resolution;
  - the hub fan-out;
  - a probe memo keyed by spelling.

  The release `scale` run was not made.
- **Tests:** the new model fields are in `format.rs`'s `SERIALISED` list and in a new dumped corpus `fixtures/parent-ref`.
- **Mutations:** 23 of 23 killed.
