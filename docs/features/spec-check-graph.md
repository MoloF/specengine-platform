---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-eval]
ref: 08-roadmap.md Phase 1, spec check increment 2, part 1 of 2
shipped: 2026-09-29
adrs: []
---

# spec check, increment 2 part 1: index, generators, graph warnings

## Why

Only `xtask` gated §11.5 (index drift) and §11.6 (generated documents): retiring it (increment 3) would lose two of six checks (ADR-0013, `docs/canon/architecture.md#checks-migration`). The check resolved only front-matter: a prose citation of a missing ID or of a superseded decision, and a `depends_on` loop, stayed invisible (§10: "retrieval cites dead decisions"). Goal: see that drift without a noisy gate — errors where the convention says "fail", warnings where recognition is heuristic (ADR-0012, `#control`). Owner, 2026-09-29: Q-2 the four invalid YAML scalars quoted, no baseline; Q-A the three graph rules are warnings; Q-C `petgraph =0.8.3` approved. Q-D open (`docs/canon/spec-check-graph.md`, "Next"). The truth now lives in that canon document.

## Acceptance criteria

Tests in `crates/specengine-core/tests/` unless named; each names the mutation that turns it red.

- [x] AC-01 Layering (eval `build_graph.rs`): no file I/O in the renderer and rules; exactly one added workspace key, `petgraph` `=0.8.3`, a normal dependency of `specengine-core` only, default features off (none needed); new to the default members: `fixedbitset` 0.5.7, `hashbrown` 0.15.5, `foldhash` 0.1.5. Red: `std::fs` in the renderer, a second key, `"0.8"`.
- [x] AC-02 Renderer parity (store `check_parity.rs`): on this repository `render_index` under the parity config + registry equals `cargo xtask docs index --root` stdout and the committed `docs/index.md`, byte for byte. Red: a shipped spec under `Specs`; links from the root.
- [x] AC-03 Renderer (`check_index.rs`): links from `index.md` and `a/b/index.md`; the index and other generated documents unlisted; section order, empty ones omitted; Archive and live cases by status; `No class — fix` (none, `class: memo`, failed front-matter); `tier ?`, `?`, empty scope; header `command` twice and `gate` (default `spec check`); reversed input → same bytes; no corpus literal. Red: a `"../"` literal; a `docs/` strip.
- [x] AC-04 Drift (`check_index.rs`): committed = render → nothing; a hand edit (line of the first differing byte), an unrendered document or status, a trailing space, CRLF → `index-drift`; not walked → `index-missing`; no `index = true` entry → neither; bytes not supplied → cannot check; a `read_error`, an `UnreadableDir` or a written `MissingRoot` → no comparison; a skipped non-UTF-8 name → still compared. Red: whitespace-insensitive comparison; drift without an index entry.
- [x] AC-05 Registry (`check_config.rs`, `check_generated.rs`; store `check_config.rs`): every invalid table of the canon list (both `index` cross-checks, a `[generators]` table, a blank `gate`, `command`/`gate` not a plain scalar or not reading back as itself, `.inf`, `-->`) → `specengine.toml:<line>: message`, cannot check; a code-built `index = true` entry without `[paths] index` → a cause. No table: `generator: foo` → nothing; with it: `foo` or no `generator:` → `generator-unknown`, a path outside `writes` → `generator-path`. Editing only the table → `parsed: 0`. Red: an unknown generator accepted; the table in the fingerprint.
- [x] AC-06 Differential (`check_parity.rs`, scratch copies, registry on): per seed, `cargo xtask docs check --root` and the new check block the same files, the seeded one with its code; the 24 old seeds agree; new seeds: hand-edited index, an unrendered canon document, an unrendered `superseded-by` → `index-drift`; index deleted → `index-missing`; `generator: foo` → `generator-unknown`; the index command on another path → `generator-path`. Red: any new rule off.
- [x] AC-07 Parity, clean: parity config + registry, `enforce`, no baseline → `clean`, 0 debt, 0 stale, `xtask` blocks nothing; exactly one new finding: `mention-dangling` `docs/canon/spec-check.md:50` (the `file-name` example). Red: drift on a clean index; Tier 3 sources scanned.
- [x] AC-08 Mentions (`check_mentions.rs`): defined ID, `aliases:`, `aliases_from`, existing `#section` → nothing; undefined ID or `#MISSING` → `mention-dangling`, its line, subject as written; `slug/`, `project:`, generated and Tier 3 sources → nothing; a failed front-matter source checked; spec-a none, spec-b one (`REQ-003` with a Cyrillic letter). Red: error severity; generated sources checked.
- [x] AC-09 Greedy names (`check_mentions.rs`): `MEC-STAMINA-based` → nothing; `MEC-NOPE-based`, `MEC-X` → `mention-dangling`; `refs: [MEC-STAMINA-based]` → `ref-dangling`. Red: no fallback; fallback on front-matter.
- [x] AC-10 Cycles (`check_graph.rs`): spec-a → one `depends-cycle` `MEC-SPRINT, MEC-STAMINA`; a self-loop, a 3-cycle, two cycles sharing a node → one each; a DAG, an unresolved target, a Tier 3 or generated source → none; reversed input → same. Red: one finding per edge; the DAG flagged; subject in input order.
- [x] AC-11 Superseded (`check_graph.rs`): `refs`, `adrs`, `links.*`, `parent`, inline → one `ref-superseded` each, naming X; `supersedes:`, Y's `status:`, X's own citations, Tier 3 and generated sources → none; spec-a, spec-b none. Red: X's citations not exempt (spec-b's `ADR-0001` has `adrs: [ADR-0002]`).
- [x] AC-12 Output (`check_output.rs`): `CHECK_CODES` 19 + 7; warnings only in `lines(true)`, counted; a baseline entry makes a `depends-cycle` or `mention-dangling` debt; reversed input → same lines and JSON. Red: `HashMap` order.
- [x] AC-13 Genre (`check_genre.rs`): spec-a, spec-b block as in increment 1 (no registry); new warnings: spec-a one `depends-cycle`, spec-b one `mention-dangling`; no corpus prefix, path or file in the sources. Red: an `"ADR-"` special case.
- [x] AC-14 Eval (`check_cli.rs`): spec-a envelope counts `depends-cycle` 1 (warning), no path or ID on stdout. Red: the subject printed.
- [x] AC-15 Read-only: `git status --porcelain` unchanged by the test run. Red: the render written to `[paths] index`.
- [x] AC-16 Format (store `format.rs`): `INDEX_FORMAT` 3, `format_history.txt` and both `expected.json` unchanged. Red: the fallback in the parser.
- [x] AC-17 The Q-2 edit: ADR-0015, ADR-0018, ADR-0020 `title:` and `docs/features/phase-0-spikes.md` `ref:` quoted; `dogfood.rs` `INVALID_YAML` gone, every ADR `id` = its stem; the A4 baseline gone from `check_parity.rs`. Red: one title unquoted.
- [x] AC-18 Documents: `cargo xtask docs index --write && cargo xtask docs check` green; both `docs/canon/spec-check*.md` ≤ 12 288 B; worst W 118 107 → 117 863 B (≤ 117 874; Tier 0 5 630, Tier 1 9 510, index 8 088, 05 41 891, 04 34 905, 08 17 839); §11.5–6 and the warnings answerable from `docs/canon/` alone.
- [x] AC-19 Regression: `cargo nextest run --workspace` 592 passed, 0 failed, 15 skipped; clippy, fmt clean.

## Implementation

Four iterations (the fourth owner-approved); review accepted in 1, 3 and 4. (1) renderer, §11.5–6, `[[generators]]`, the three warnings, `petgraph`, `Resolver`. (2) reviewer minors: no index comparison on an incomplete walk; `command`/`gate` plain scalars; blank `gate`; a cause for a code-built `index = true` entry without `[paths] index` — but a skipped non-UTF-8 name also stopped the comparison (a major). (3) that fixed, the unreachable `read_error` branch removed, `-->` rejected. (4) `command`/`gate` must read back as themselves through the crate's front-matter reader (`read_back`, replacing a hand-kept YAML 1.2 core-schema list); `.inf`, `-.inf`, `.nan` rejected. Deviations from the draft: none left undocumented; open nits in the canon document.

| Module | What it does |
|---|---|
| core `check/render.rs` | `render_index`; `is_tier3`, the live-source predicate shared with the graph rules |
| core `check/generated.rs` | `index-drift`, `index-missing` (`walk_incomplete` guard); `generator-unknown`, `generator-path` |
| core `check/graph.rs` | `mention-dangling`, `depends-cycle` (`tarjan_scc`), `ref-superseded` |
| core `check/resolve.rs` | public `Resolver`, `Resolution`, `resolve_mention` fallback; `declared_references` moved from the engine |
| core `check/config.rs` | `Generator`, `DEFAULT_GATE`, `index_generator()`; table validation, `not_plain_scalar`, `read_back` |
| core `check/engine.rs`, `check/mod.rs`, `paths_toml.rs` | rules wired in, `CHECK_CODES` 26, exports; `checked_path` shared |
| `Cargo.toml`, core `Cargo.toml`, `Cargo.lock` | `petgraph =0.8.3`, default features off |
| tests | core `check_{index,generated,mentions,graph}.rs` new, `check_{config,output,genre,classes}.rs`, `dogfood.rs`; store `check_{parity,config,verdict}.rs`, `format.rs`; eval `build_graph.rs`, `check_cli.rs` |
