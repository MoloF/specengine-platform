---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, docs]
ref: roadmap Q-8 (`docs/features/roadmap.md`); owner's answers Q1-Q6, 2026-10-01
shipped: 2026-10-01
adrs: [ADR-0030]
---

# Index shards

## Why

The index was one file, one line per document, counted whole in W: the one W term growing with n, against §3. At 7947d5d it was 8 792 of 10 240 B, a pass added ≈ 186 B (cap ≈ 7 passes away), the Archive section 1 403 B. A pilot's records are canon, never Tier 3: ~500 lines, 3–6 times the cap. ADR-0028 left the fix to Q-8; ADR-0030 takes it.

Durable means: (1) bounded read: the index step reads the root plus at most one live shard, each ≤ `index_bytes`; (2) growth off the read path: a shipped spec or a superseded decision adds bytes to the archive shard only, a live document to exactly one file; (3) relief by configuration: at a cap, a new or narrower claim, never code or a raised cap.

## Acceptance criteria

M: the mutation that must turn the criterion red. Core tests unless named.

- [x] AC-01 No shard, no change: every existing test target passes, edited only by `None` at `worst_w` calls and `shards: Vec::new()` in `Generator` literals, except the index entry's extra-`writes` case, which the new rule rejects; `--stdout`, `--json` byte-identical. M: the root always ends with `## Shards`; `"shards":[]` always emitted.
- [x] AC-02 Archive shard, root `a/index.md`, shard `a/b/arch.md`: the root has no Tier 3 line and one pointer linked `b/arch.md`; the shard holds exactly the Tier 3 documents as compact lines in path byte order, linked from `a/b/`, under the header naming the registered command and gate, its back link `../index.md`. M: shard links from the root's directory; Tier 3 decided by folder.
- [x] AC-03 Once: over AC-02's and AC-04's corpora every walked document but `class: generated` is on exactly one line across the outputs; no output listed, one with unclosed front-matter and one with `class: canon` included. M: a Tier 3 line kept in the root; outputs excluded by class only.
- [x] AC-04 Claims, on scratch copies of `fixtures/spec-a` and `fixtures/spec-b`, the config written into each copy: claimed live lines in the shard, the rest plus the pointers in the root; overlapping claims → the first shard; a Tier 3 document a claim matches → the archive shard, or with none the claiming shard's Archive section. M: the last claim wins; claims tried in path order; Tier 3 placed after claims.
- [x] AC-05 Drift per output (and store `check_verdict.rs`): a byte edited on line k of a shard → one `index-drift` on its path at k, the root clean; a deleted shard → `index-missing` on its path; a root edit → drift on the root only; a `walk_gap` → no comparison. M: only `[paths] index` compared; the first drifting output stops the rest.
- [x] AC-06 Registry (`check_config.rs`): every registry error at its line, one case each; `shards = []` loads as no shard; the §11.6 example read from the canon loads. M: the `writes` membership check removed; an extra index `writes` path accepted; two `tier3` accepted.
- [x] AC-07 Budgets (`check_budget.rs`): the root and each live shard at `index_bytes + 1` B → one `budget` (subject `index`) each on its path; an archive shard of `index_bytes + 1` B → none; all at `index_bytes` → none. M: live shards uncapped; the archive shard capped.
- [x] AC-08 W (`check_worst_w.rs`, the store's in-test oracle, CLI `worst_w.rs`): live shards of 3 000 and 5 000 B and an archive shard of 20 000 B → W = Tier 0 + largest Tier 1 + root + 5 000 + the 3 largest pool files. M: the archive shard counted; live shards summed; the term dropped.
- [x] AC-09 Export (CLI, scratch `HOME`, sharded copies of spec-a, spec-b): every output == `render_index_set`; `spec check` then finds no drift or missing output; a second run → every output `unchanged`, every mtime kept; one output edited → only it rewritten; text, JSON and `--stdout` as in the CLI canon, outputs in config order, `--stdout` writing nothing. M: equal outputs rewritten; `--stdout` without `==>` lines.
- [x] AC-10 Refusals write nothing: a symlink on the way to a shard, a missing shard parent, a shard path that is a directory → exit 2, empty stdout, no output created or modified (the stale root's bytes and mtime kept). M: the root written before the shard is checked.
- [x] AC-11 Genre and determinism: core `check_genre.rs` `PROJECT_NAMES` and CLI `genre.rs` gain the shard names of this repository and the fixtures; reversed input → identical bytes for every output. M: a default shard name in `render.rs` or `export.rs`.
- [x] AC-12 This repository after step 2 (store read-only; CLI on a scratch copy): the committed root and `docs/index-archive.md` == the render; the shard lists exactly the walked Tier 3 documents, the root none; both deleted on the copy → export recreates both byte-identical; `git status --porcelain` unchanged by the runs. M: the shard not written; Tier 3 kept in the root.
- [x] AC-13 Shipping costs the root nothing (CLI, scratch copy of this repository): a live spec set to `shipped` → the root loses exactly its line, the shard gains exactly one; a new shipped spec changes the shard only. M: a document count in the pointer.
- [x] AC-14 Docs at shipping (command checks): the gate clean; `docs/index.md` without a Tier 3 line; ADR-0030 ≤ 1 536 B and its `canon:` and ADR-0028's resolve; every Tier 2 canon ≤ 12 288 B; `docs/canon/documentation-system.md` byte-identical to 7947d5d; W ≤ 115 743 − 1 403 (Archive at 7947d5d) + P (the root's `## Shards` section) + L (live root lines added since, ADR-0030's included); net growth since 7947d5d ≤ 0 for `CLAUDE.md`, 04, 05, 06, 07, 08 and the largest Tier 1; no "Next" names index sharding. M: +1 B in `CLAUDE.md`.
- [x] AC-15 `cargo nextest run --workspace` once at the end, green (978 passed, 15 skipped); clippy and fmt clean; every new CLI test uses a scratch `HOME`; no test writes git state or runs `spec export index` in this repository.

## Implementation

Canon: `docs/canon/spec-check-graph.md#index-shards` (render, placement, §11.5 per output, §11.6 shard errors and this repository's block), `docs/canon/spec-check.md` (budget, W), `docs/canon/spec-check-cli.md` (export, W-5). Iteration 1: the mechanism. Iteration 2: inspection opens a differing output for writing (device and inode), two outputs that are one file refused, claims and shard paths Markdown-safe (no control character or backtick), the `tier3` message, `ARCHIVE_TITLE` shared by section and label.

| Module | What it does |
|---|---|
| core `check/config.rs` | `Generator.shards`, `Shard`, `ShardKind`, `Shard::is_archive`; `shards_from` checks each shard; the index entry writes only the root and its shards |
| core `check/render.rs` | `render_index_set`, `IndexOutput`: placement, the `## Shards` pointers, the shard header; `render_index` = the root |
| core `check/generated.rs`, `engine.rs`, `working_set.rs` | §11.5 per output; `budget` on the root and each live shard; `worst_w(.., Option<&Generator>)` adds the largest live shard |
| CLI `src/export.rs`, `lib.rs` | two phases: `inspect`, `one_file_each`, then `write`; `ShardOutcome`, JSON `shards`, `==>` blocks |
| `specengine.toml` | the archive shard `docs/index-archive.md` |

Tests: core `check_shards.rs`, `check_config.rs`, `check_budget.rs`, `check_worst_w.rs`; store `check_verdict.rs`, `check_parity.rs`; CLI `export_shards.rs`, `shards_repo.rs`, `worst_w.rs`. At shipping, repository-pinned tests moved to the sharded layout: `check_parity.rs` (two new seeds: the archive shard hand-edited, deleted), CLI `parity.rs`, `shards_repo.rs`, `check_config.rs` `the_registry_example_loads`. Sizes: root 9 015 → 7 556 B, shard 1 898 B, W 115 966 → 114 506 B (AC-14's bound 114 507: P 87 B, L 80 B).

Deviations: `IndexOutput.bytes` is a `String`; `shards`, even `[]`, is rejected on a non-index entry; shard checks report the first error only; paths equal ignoring case refused for every pair, existing or not (CLI W-5). A2 ("no shard → today's file") has one explicit exception: the index entry's `writes` membership rule applies with or without shards, so a config with an extra index `writes` path is now rejected. A read-only root whose bytes differ is now a refusal at inspection (`cannot open … for writing`, nothing written), no longer a failed write.

Open: an absent NFC/NFD alias pair and an absent shard in a non-writable directory stay failed writes after earlier outputs (no normalisation dependency); an editor's atomic save between inspection and write (negligible).

Owner, with the shipping commit: `.claude/agents/spec-writer.md:31`, now ``- **generated** — `docs/index.md`. Never touch it by hand.``, becomes ``- **generated** — `docs/index.md` and its shards (the index generator's `writes` in `specengine.toml`). Never touch them by hand.``
