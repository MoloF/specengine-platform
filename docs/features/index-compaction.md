---
class: spec
status: shipped
scope: [xtask, crates/specengine-core]
ref: 08 §2 Phase 1, before CLI pass 2b; owner's answers Q1-Q5 (2026-09-30)
shipped: 2026-10-01
adrs: [ADR-0028]
---

# Index compaction: Tier 3 lines

## Why

Every documentation commit must pass `cargo xtask docs check`, and caps never move (§4). The index stood at 9 842 of 10 240 B; 27 % of it (2 650 B, 14 lines) listed Tier 3 documents with full title and scope, although Tier 3 is reachable by id only (§3). Each `/feature` run adds such a line, and 2b alone would have used up the remaining 398 B. Both renderers now write a Tier 3 line as `- [<label>](<link>) <status>`, the status whole (`superseded-by X` names the successor, Q3): nothing deleted or moved, every document keeps one line and its citations. Canon: `docs/canon/spec-check-graph.md` "Index render (§9)", `docs/README.md` "Tiers" (ADR-0028, Q1); `docs/canon/documentation-system.md` quotes the convention verbatim and stays unchanged. Done before 2b, not inside it: the two renderers still checked each other byte for byte on the only format change, and 2b's commit does not mix a convention change with the hook and CI switch.

This slows growth, it does not stop it: live lines stay O(n) (~110 B per ADR, ~150 B per canon document), a shipped spec still adds ~76 B, and pilots gain nothing until they register `spec export index`. Q2: Tier 3 stays in the index until sharding decides where it lives. Q5: the durable fix, sharding (`docs/features/roadmap.md` Q-8) or a terser live line, comes before Phase 2 and before the first pilot registers `spec export index` (08 §2 Phase 1).

## Acceptance criteria

Each named mutation turns its criterion red; all were run red by the test-engineer. AC-06 and AC-07 are command checks at shipping, not tests: a byte cap or a diff against a fixed commit would break with every later document.

- [x] AC-01 Core grammar: in `crates/specengine-core/tests/check_index.rs`, `the_render_is_the_convention_s_index`'s CORPUS renders its `## Archive` section as exactly the four lines below; everything before `## Archive` equals the previous `expected()` bytes. Mutations: a Tier 3 line keeps `title · scope ·`; a Specs line compacted.

```
- [ADR-0002](decisions/ADR-0002.md) rejected
- [ADR-0003](decisions/ADR-0003.md) superseded-by ADR-0001
- [docs/features/f-abandoned.md](features/f-abandoned.md) abandoned
- [docs/features/f-shipped.md](features/f-shipped.md) shipped
```

- [x] AC-02 By status, not folder (`check_index.rs`): a `draft` spec at `docs/archive/x.md` gets a full line under `## Specs`; a `shipped` spec at `notes/y.md` the compact form under `## Archive`; an accepted decision and one without `status:` full lines under `## Decisions`. Mutation: Tier 3 chosen by a path test.
- [x] AC-03 Parity, unmodified tests: `crates/specengine-store/tests/check_parity.rs` `the_core_render_is_xtask_s_index_and_the_committed_one`, `the_parity_config_walks_the_budget_documents_and_is_clean`; `crates/specengine-cli/tests/parity.rs` `export_index_recreates_this_repository_s_index`, `dogfood_check_of_this_repository_is_clean_and_changes_nothing` pass with no edit to those tests. Mutations: only the core compacts; the index not regenerated.
- [x] AC-04 Seeded parity (a new test in `check_parity.rs`): on a scratch copy of the documents with an added `status: rejected` decision and an `abandoned` spec without `scope:`, `render_index` equals the stdout of `xtask docs index --root <copy>` byte for byte, both seeds as compact `## Archive` lines. Mutation: `xtask`'s Tier 3 branch keyed on `superseded-by` only.
- [x] AC-05 Every document listed once (a new read-only store test over this repository): the link targets of `docs/index.md`, resolved against `docs/`, equal the walked documents minus `class: generated`, each exactly once; no line under `## Archive` contains ` · `. Mutations: the Archive section dropped; the title kept on Tier 3.
- [x] AC-06 At shipping: `wc -c docs/index.md` ≤ 8 700; `git diff -U0 e6426a7 -- docs/index.md` changes only lines under `## Archive`, plus the added ADR-0028 line and this spec's own line. Mutation: the ` · ` separator changed (live lines in the diff). Result: 8 427 B; the hunks cover the Archive lines and the ADR-0028 line only.
- [x] AC-07 Hold W: `cargo xtask docs budget`'s worst W ≤ 117 061 − (9 842 − the index's bytes). Mutation: +1 B in `CLAUDE.md`. Result: 115 643 ≤ 117 061 − (9 842 − 8 427) = 115 646.
- [x] AC-08 Genre and determinism: the genre test of `check_index.rs` and `reversed_input_renders_identical_bytes` pass. Mutation: a `"docs/"` literal in `render.rs`.
- [x] AC-09 Docs: `cargo xtask docs index --write && cargo xtask docs check` green; ADR-0028 ≤ 1 536 B, its `canon:` resolving; every Tier 2 canon ≤ 12 288 B; `docs/canon/documentation-system.md` byte-identical to `e6426a7`. Mutation: an unresolvable `canon:`.
- [x] AC-10 Run: `cargo nextest run --workspace` once at the end, green; clippy and fmt clean; any new CLI test uses a scratch `HOME`; no test writes git state in this repository.

## Implementation

One iteration; review accepted with no code findings; the full workspace run: 866 passed, 0 failed, 15 skipped. The index went 9 842 (10 089 once this spec and ADR-0028 were listed) → 8 427 B, W 117 308 → 115 643 B. Q4: once the renderer changed, the committed index drifted; the orchestrator ran `cargo xtask docs index --write` right after the implementer's report, so the registered generator wrote it and no role's write area changed.

| Module | What it does |
|---|---|
| xtask `src/docs/index.rs` | `line()` returns early for `Doc::is_archived` (`src/docs/mod.rs`) → `- [<label>](<link>) <status>`; title and scope computed for live lines only; module doc |
| core `check/render.rs` | `line()` returns early when `fields.is_some_and(is_tier3)`, the predicate `section_of` uses, so section and line always agree; a failed front-matter stays live under `No class — fix`, never compacted; module doc |
| core `tests/check_index.rs` | AC-01's `expected()` Archive lines; `tier3_is_chosen_by_status_not_by_folder` (AC-02), `a_tier3_line_reads_neither_title_nor_scope` |
| store `tests/check_parity.rs` | `seeded_tier3_cases_render_as_xtask_does` (AC-04: catches a predicate narrowed to this repository's statuses, which AC-03 alone misses); `every_document_of_this_repository_is_listed_once` (AC-05: over the committed index, the core render and `xtask`'s stdout) |

Both predicates are exact string matches: a spec `shipped` or `abandoned`, a decision whose status is anything but `accepted`; canon and generated documents are never Tier 3. The Tier 3 line reads neither title nor scope, so the title divergences of "Index render (§9)" no longer reach it. AC-03's four parity tests and every CLI test pass unedited (the CLI compares against the library render).

Open, pre-existing, out of scope: an `id:` present but not a valid ID renders differently in the two renderers (the core drops or normalises it, `xtask` prints it raw), on live lines too; it ends when 2b retires `xtask`.
