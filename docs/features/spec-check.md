---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-model, crates/specengine-store, crates/specengine-eval, xtask]
ref: 08-roadmap.md Phase 1, third bullet (`spec check`), increment 1 of 4
shipped: 2026-09-29
adrs: []
---

# spec check, increment 1: a config-driven check engine at xtask parity

## Why

Each project checks the convention with its own script (here `xtask`, literals compiled in); ADR-0013 moves them into `spec check` at parity (`docs/canon/architecture.md#checks-migration`). Pilots carry debt (08 §4.3): a strict day-one gate stays red or is switched off. Increment 1: one core check driven only by `specengine.toml`, at `xtask`'s §11.1–4, plus the ADR-0009 ID checks and an expiring debt baseline: a way off `xtask` without a mass-fix PR.

## Acceptance criteria

Tests in `crates/specengine-core/tests/` unless named; scratch corpora in temp dirs.

- [x] AC-01 Layering (`crates/specengine-eval/tests/build_graph.rs`): the parser's AC-02 no-file-access scan covers the check module; no `rusqlite` under `-core`; `git diff main -- Cargo.toml` adds no `[workspace.dependencies]` entry. Mutation: `std::fs::read` in the check module → red.
- [x] AC-02 Anchors (`anchors.rs`): a `slug` per heading (`-1`, `-2` on repeats, Cyrillic kept), `attr` for a non-ID `{#…}`, `html` for `<a id>`, `<a name>`, each with origin and span; none from code blocks or HTML comments. Mutations: raw-line scan → red; no repeat suffix → red.
- [x] AC-03 Here (`dogfood.rs`): every parsing ADR's `canon:` anchor is among its target's; ADR-0023 via `html`, ADR-0025 (`README.md#license`) via `slug`. Mutations: no `html` → ADR-0023 red; no slugs → ADR-0025 red.
- [x] AC-04 Format (`crates/specengine-store/tests/format.rs`): `INDEX_FORMAT` 2; `format_history.txt` keeps line 1, adds one for 2; both `expected.json` regenerated; index AC-05, AC-07, AC-10 green. Mutation: new dump stamped 1 → red.
- [x] AC-05 Config (`check_config.rs`; re-parse half in store `tests/check_config.rs`): the parity config loads; unknown key or class, wrong type, `tier0_bytes = 0`, `mode = "strict"` fail as `specengine.toml:<line>: message`; editing only new tables or keys → `update` reports `parsed: 0`. Mutation: unknown keys accepted → red.
- [x] AC-06 Contracts (`check_classes.rs`): default: canon without `reviewed` → `key-missing`, an untyped extra key → only `unknown-key`; `closed = true` → `key-extra`; no `class:` (with a scheme `id:`, without, no front-matter) → `class-missing`, no contract finding; a YAML failure in a decision → one finding. Mutations: default closed → red; class checks after a YAML failure → red; class-less `id:` files skipped → red.
- [x] AC-07 IDs (`check_ids.rs`): `id: ADR-001` (width 4) → `id-width`; a definition and a reference with U+0420 for `P`, U+0415 for `E` → error `homoglyph` with Latin text and span; bytes unchanged. Mutations: a warning → red; fix applied → red.
- [x] AC-08 Duplicates (`check_ids.rs`): an ID in two files → `id-taken` on the later, naming the first; feature-scoped → none. Mutation: per-file uniqueness → red.
- [x] AC-09 File names (`check_ids.rs`): fixture records clean; `Q-32.md` holding `Q-031`, `ADR-00011.md` holding `ADR-0001` → `file-name`. Mutation: bare `starts_with` → red.
- [x] AC-10 References (`check_refs.rs`): spec-a `refs: [R-12, QST-031]`, spec-b `superseded-by ADR-0001`, `adrs: [ADR-0002]`, `cli.md`'s U+0422 U+0420 U+0411 `-002` resolve; spec-b U+0422 U+0420 U+0411 `-003` (`dry-run.md`), U+0412 U+041E U+041F `-6` (`QN-07.md`) → `ref-dangling`; scratch `parent: QST-031` resolves; an undefined inline mention → nothing. Mutations: aliases ignored → red; `MOD-CLI#CMD-NOPE` passes → red.
- [x] AC-11 `canon:` (`check_refs.rs`): spec-b ADR-0001's resolves via a Cyrillic slug, ADR-0002's `MOD-CLI#CMD-SYNC` as a reference; scratch: no `#`, missing file, a spec, missing anchor → `canon-form`, `canon-file` ×2, `canon-anchor`, also class-less. Mutation: no `#` accepted → red.
- [x] AC-12 Budgets (`check_budget.rs`): whole-file bytes (BOM, front-matter); cap passes, cap + 1 → `budget`; UTF-8 bytes; `index` capped whatever its class. Mutations: body only → red; `>=` → red.
- [x] AC-13 Baseline (`check_baseline.rs`): a match is debt, kept when a line is inserted above; past `expires` an error blocks, counted `expired`, a warning stays one; unmatched → `Report.stale`, counted `stale`, not in warnings; no `reason` or `expires`, bad TOML → cannot check. Mutations: match by line → red; `expires` ignored → red; expired warning made an error → red.
- [x] AC-14 Verdict (store `tests/check_verdict.rs`): a blocking finding → `observe` 0, `enforce` 1; missing root or written root, mode-000 file or directory, invalid config → 2 in both; spec-a (default `docs/archive` missing) → not 2. Mutation: missing root clean → red.
- [x] AC-15 Output (`check_output.rs`): only debt and warnings → 1 line; k blocking → k + 1; `observe` → 1; `detail` lists the rest. Mutation: warnings listed without `detail` → red.
- [x] AC-16 Parser codes (`check_output.rs`): every `DiagnosticCode::ALL` code reaches the report via the one table; parser errors block. Mutation: parser warnings dropped → red.
- [x] AC-17 Determinism (`check_output.rs`): reversed input → byte-identical lines and JSON. Mutation: `HashMap` order in output → red.
- [x] AC-18 Genre (`check_genre.rs`): blocking findings exactly — spec-a: `frontmatter-type` (`docs/features/stamina-tuning.md`); spec-b: `homoglyph` (`docs/spec/cli.md`), `frontmatter-yaml` + `homoglyph` (`docs/records/QN/QN-08.md`), AC-10's two `ref-dangling`; so no `class-missing`; the check source names no prefix, path or file of a corpus. Mutations: an `"ADR-"` special case → red; `class:` dropped from `Q-031.md` → red.
- [x] AC-19 Parity, clean (store `tests/check_parity.rs`): the parity config (roots: top-level `.md` files and directories not `.`-named nor in `xtask`'s `SKIP_DIRS`; `exclude` + `**/<name>/**` per `SKIP_DIRS`) walks exactly `cargo xtask docs budget`'s documents; `enforce` + A4 baseline → `clean`, debt = A4's seven. Mutation: `_*.md` exclude dropped → red.
- [x] AC-20 Parity, differential (`check_parity.rs`): on a scratch copy, per seed, the files with a blocking finding from `cargo xtask docs check --root` (§11.5–6 aside) equal the new check's, among files whose front-matter parses (A4 aside), and the seeded file blocks with the seed's code. Seeds: `CLAUDE.md` 16 385 B, a README 10 241 B, an ADR 1 537 B; front-matter removed, `class: memo`, `owner` removed, `foo: bar`, `reviewed: 2026-9-1`, `tier: 1` on `docs/canon/architecture.md`; `shipped:` removed from `docs/features/spec-index.md`, accepted ADR without `canon:`; `canon:` without `#`, to a missing file, to `docs/features/spec-index.md#why`, to a missing anchor; `ADR-0099` via `superseded-by`, `supersedes`, `adrs`; ADR-0002 as `id: ADR-0001`, `ADR-0003.md` renamed `3.md`, `id: ADR-001`. Mutation: any check off → its seed red.
- [x] AC-21 Read-only: `git status --porcelain -- fixtures/` equal before and after; scratch files keep bytes and mtimes. Mutation: a fix written → red.
- [x] AC-22 Eval (`crates/specengine-eval/tests/check_cli.rs`): on `fixtures/spec-a` one JSON envelope: counts per code and severity, both verdicts, `wall_ms`; no path or ID on stdout; `--out` under the corpus → exit 2, nothing written. Mutation: a path on stdout → red. Pilot runs: owner-run `#[ignore]`, skipped until the pilot schemes gain `[paths]`, `[budgets]`, `[check]`.
- [x] AC-23 Docs: `cargo xtask docs index --write && cargo xtask docs check` green; `anonymity.rs` green; worst W (`cargo xtask docs budget`) ≤ 118 068 B (its start value): 118 025 B at the last iteration, 117 883 B after shipping.
- [x] AC-24 Regression: `cargo nextest run -p specengine-model -p specengine-core -p specengine-store -p specengine-eval` green, incl. parser AC-19, index AC-05–AC-10.

## Implementation

Two iterations; `cargo nextest run --workspace` 476 passed, 0 failed, 15 skipped; clippy, fmt, docs check clean; review accepted in both (no blocker or major). Iteration 1 built all below. The owner then answered Q-4 (`class:` mandatory); iteration 2 removed the skip of class-less files with an `id:`, dropped `Counts.without_class` (`class-missing` counts them), kept an expired matched warning a warning, and made a stale baseline entry no finding (`debt-stale` only labels `Report.stale` in detail lines). The test-engineer strengthened AC-20: a symmetric comparison over files whose front-matter parses plus a per-seed expected code (a file-level comparison cannot tell `id-width` / `id-taken` from `file-name`); all 24 seeds agree with `xtask`. This repository under the parity config and the A4 baseline: 52 documents (53 with `docs/canon/spec-check.md`), `enforce` → `clean`, 7 debt.

| Module | What it does |
|---|---|
| model `node.rs`, `parsed.rs` | `Anchor {name, origin, level?, span}`, `AnchorOrigin` `slug` / `attr` / `html` |
| core `markdown.rs` | heading slugs (github-slugger, repeats `-1`, `-2`); `<a id>`, `<a name>` from HTML events, comments skipped |
| core `paths_toml.rs` | `[paths]` `tier0`, `tier1_name`, `index`, `roots_written` |
| core `check/config.rs`, `baseline.rs` | `[budgets]`, `[classes]`, `[check]` → `CheckConfig`; `.spec-debt.toml` → `Baseline`; errors `file:line: message` |
| core `check/input.rs`, `text.rs` | `CheckInput`, `CheckFile`, `Problem`; written keys and lines, date shapes |
| core `check/engine.rs` | `run`: `PARSER_SEVERITY`, class contracts, tiers, budgets, IDs, references, `canon:`, baseline matching |
| core `check/report.rs` | `Finding`, `Counts`, `Verdict`, sort, `lines`, `to_json`, exit codes |
| store `check.rs`, `lib.rs` | `check_input`, `check_worktree`, `today_utc`, `BASELINE_FILE`; `INDEX_FORMAT` 2 |
| eval `check.rs`, `main.rs` | `specengine-eval check` |
| xtask `main.rs` | `docs <command> --root DIR` |
| tests, fixtures | core `anchors.rs`, `check_*.rs`; store `check_{config,verdict,parity}.rs`; eval `check_cli.rs`; every spec-a / spec-b document given its class and minima |

Accepted divergences from `xtask`: a file whose front-matter fails is not judged; `file-name` needs `X.` or `X-`, not `starts_with`; slugs come from inline text, not the raw line. Truth: `docs/canon/spec-check.md` (new: rules, tables, baseline, output, owner questions Q-1…Q-8, open minors), the READMEs of core, model, store, eval, `xtask`, `docs/README.md`; 05 §4, 07 §5, 08 Phase 1 (increments 2–4) and AC-11. Open for the developer: `crates/specengine-store/src/check.rs` and `crates/specengine-eval/src/check.rs` cite this spec's sections "Description and interactions" and "API", removed by compaction: repoint them to `docs/canon/spec-check.md`.
