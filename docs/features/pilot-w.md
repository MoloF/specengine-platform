---
class: spec
status: shipped
scope: [crates/specengine-eval, fixtures, docs]
ref: pilot-w analysis 2026-10-05, owner's answers 1-10, review decisions N1-N4, R1-R3; 08 §2 Phase 1 "Pilot projects", §3 AC-1, AC-7
shipped: 2026-10-05
adrs: []
---

# Pilot W: every task, before and after

## Why

08 §3 AC-1 wants task W (`docs/canon/documentation-system.md` §1, §3) on both pilots; Phase 1 closes when the owner picks the migration order (08 §2). `specengine-eval w` replays **every task document** (shipped included, owner's rule) twice on the same targets: the pilot's reading protocol on the untouched corpus (**W_before**), the shipped `spec bundle` on the after-tree (**W_after**), plus the follow-up reads the capped bundle leaves (**W_after_followups**). How it works now: `docs/canon/w-measurement.md`. No ADR (ADR-0008, ADR-0009, ADR-0012: 40 KB recorded, never enforced, ADR-0022, ADR-0026, ADR-0027).

## Acceptance criteria

`tests/w_cli.rs` over `fixtures/pilot-w/{one,two}`, two conventions: `census.toml` + `[layout]`, `specengine.toml`, `tasks.toml`, `expected.json` (= `result` but `bundle_ms`); no raw Cyrillic (`anonymity.rs`). `one`: shards (`index = true`), a Tier 1 table, a task whose records layout moves (one holding a link), a nested task, a shipped task, a generated document in the globs, multi-byte text; `two`: no index, `default` only, a `slug`-moved document a task links, a duplicate definition, an even task count.

- [x] AC-01 A header core rejects, no front-matter, a shipped document (bundled ` | archived`): tasks; `class: generated`, outside the globs: not; `task-N` by source path. Each refusal → exit 2 at `<tasks>:<line>`, `--out` empty. M1: filtering on liveness or a parsed header → red. M2: a generated task → red. M3: `deny_unknown_fields` off → red.
- [x] AC-02 Front-matter ID, inline ID, legacy alias, `<slug>/ID`, `../` file link, own section by a bare feature-scoped ID another feature defines too, a record moved out of D, a link inside it, a link to the `slug`-moved document → canonical `refs`, `failed` 0; corpus-dangling → `unresolved` 1, `[[…]]` → `wiki_links` 1 per holder, non-document link → `unchecked` 1, none a REF. M1: written forms as REFs → `failed` ≥ 1. M2: dangling uncounted → red. M3: a bare feature-scoped REF → red. M4: moved records or their links dropped → red. M5: that link `unresolved` → red.
- [x] AC-03 Slots = `expected.json`: two targets in one source document count it once; the larger of two matched READMEs (a larger one unmatched); the second shard (the first does not link the task), also from an A-style generator (scratch copy: no `index = true`, the shards in `writes`); a duplicate → both sources; a moved record: D's source once, its body in `Bundle.bytes`; `third_step` 1. M1: per-target counting → red. M2: after-tree bytes before → red. M3: the table's largest README → red. M4: shards from `index = true` only → red.
- [x] AC-04 The test's own `specengine_cli::bundle` (own `Env`, `tree/`) per task: `bytes`, `bundle_hash` = `tasks.json`, body = `bundles/task-N.txt`; `tree/` = `layout`'s but `.spec-debt.toml`. M1: chars or tokens for bytes → red. M2: not-included list stripped → red. M3: no index outputs → red.
- [x] AC-05 Forms = the targets layer; `followups.bytes` = the test's `show` + `render_text`; `incomplete` 1; at a small `--budget` one task refused, in `tasks`, W_after max `"refused"`, its moved record's `show` in `followups.bytes`. M1: bundled tasks only → red. M2: refused → 0 → red. M3: follow-ups from `tokens_est` → red.
- [x] AC-06 Aggregates on `two` (even n) = `expected.json`; stdout = the whitelist. M1: averaged median → red. M2: a path on stdout → red.
- [x] AC-07 `nondeterministic` 0; two runs, different `--out`: `tasks.json` equal but `*_ms`. M: a `HashMap` for tasks or REFs → red.
- [x] AC-08 Tree, data directories, databases under `<out>/w/<label>/`; scratch `HOME` empty; `git status --porcelain -- fixtures/` empty; nesting, a symlinked `<out>/w` → exit 2, nothing written or deleted. M: `Env::from_process` → red.
- [x] AC-09 `import_genre.rs` scans `src/w.rs`, `tasks.toml` strings forbidden; `build_graph.rs`: eval's direct normal dependencies gain exactly `specengine-cli` (path, no features); the CLI's graph, CLI_FORBIDDEN unchanged. M1: a fixture glob or key in `w.rs` → red. M2: another dependency → red.

Owner's check, by an agent the owner instructs:

- [x] AC-10 Tasks configs (the owner's pilot configs) per the recipe (canon "Tasks config"); `w --label pilot-a|pilot-b` (`#[ignore]`, one at a time; child: `PATH`, the label's variables and `SPECENGINE_TASKS_*`, empty `HOME`) exits 0, not `"timeout"`; read-only proofs equal, `anonymity` green, `nondeterministic` 0, `failed` 0 or each recorded (A6: a bundle defect a pilot exposes is fixed with an invented-fixture regression or recorded here). Recorded, dated, here and in 08 §3 AC-1: the table, budget, profile, `bundle_ms`; 40 KB met or not (a finding); the Phase 2 re-measure (`bundles` log, `--task` at `bundle_task`, follow-ups, live-check tasks).

At shipping:

- [x] AC-11 Rules in a new Tier 2 canon `docs/canon/w-measurement.md` (≤ 12 288 B); eval README `w` row (≤ 10 020 B, largest Tier 1); 08 §2 "10 tasks" → "every task document", the MCP lever → Phase 2, "Next" closed; §3 AC-1 filled, AC-7 "re-confirmed on both pilots"; check clean, worst W ≤ 109 484 B; nextest `-p specengine-eval`, clippy, fmt, mutations red; manifest and lock diff = the eval → cli edge; fixtures: only `pilot-w`.
- [ ] AC-12 Phase 1 closes when the owner names the order from the table: 08 §2 records it (no ADR), Phase 1 "done <date>"; `CLAUDE.md` "State", root README updated. `pilot-w` may ship before.

## Migration order: decision table (AC-10, 2026-10-05)

Facts only; the owner names the order (AC-12).

| | A | B |
|---|---|---|
| task documents | 70 | 51 |
| W_before median (p90, max) | 533 076 (743 566, 922 383) | 225 562 (396 166, 483 850) |
| W_after | 43 061 (45 048, 45 760) | 27 233 (39 154, 41 216) |
| W_after_followups | 101 031 (160 515, 200 347) | 76 516 (306 313, 417 480) |
| 40 KB on the median: W_after_followups; W_after | unmet; unmet | unmet; met |
| third_step; incomplete; truncated | 70; 67; 0 | 21; 15; 56 |
| refused, failed; unresolved, wiki_links | 0, 0; 100, 1 030 | 0, 0; 0, 2 216 |
| AC-6 dry-run residue (08 §3) | 0 (2 242 / 2 249 matched, 7 headers core rejects) | 1, a corpus fact (251 / 251); hyphenless codes need an ADR superseding ADR-0009 |
| baseline debt (`import-layout.md`) | 5 336 | 340 |
| documents cited from code / citations (`import-records.md`) | 92 / 911 | 16 / 44 |
| what limits W_after | the budget: fill 99.9 %, 3.0 B/token; Tier 0 + 1 ≈ 13.7 KB on a full bundle | document size: fill 60.9 %, 12 tasks without a text item; follow-ups are whole documents (median 47 KB, 56 cut at the `show` cap) |
| **Owner's choice: migrates first** | | |

## Implementation

| Module | What it does |
|---|---|
| eval `src/w.rs` (new) | the measurement: tasks config, refusals, tasks, targets and REFs, before slots (shards parsed by core), pass 1 bundles and follow-up `show`s, pass 2 in reverse, aggregates, `tasks.json`, `bundles/` |
| eval `src/main.rs` | `WArgs` (`layout`'s plus `--tasks`, `--budget`), `Measurement::W`, `check_out` per measurement |
| eval `src/harness.rs` | `PILOT_TASKS` (`SPECENGINE_TASKS_A` / `_B`) |
| eval `src/layout.rs` | `pub(crate) write_tree` extracted from `layout`'s staging (same parts and order, `layout`'s output unchanged); `Setup` fields `pub(crate)` |
| eval `Cargo.toml`, `Cargo.lock` | path dependency on `specengine-cli`, no features; one lock line |

Tests: `w_cli.rs` (33, two `#[ignore]` pilot runs) over `fixtures/pilot-w/{one,two}`; `import_genre.rs` scans `w.rs`; `build_graph.rs` pins eval's direct dependencies; mutations of iterations 1-3 red.

**AC-10, 2026-10-05** (debug, budget 10 000, iteration 3): both pilots exit 0 (A 659 s, B 79 s), `nondeterministic` 0, `failed` 0 (A6: no defect, CLI and core unchanged), read-only proofs equal, scratch `HOME` empty, `anonymity` green; figures in the table and 08 §3 AC-1; `bundle_ms` median A 2 035 (3.4× the planned 0.6 s), B 640. Phase 2 re-measures: the `bundles` log, `--task` at `bundle_task`, follow-ups, the live-check tasks.

**Deviations, all in the canon**: Targets N gained the records layout moves out of D and the links inside them, a corpus re-read of tree-dangling or unchecked links, anchors to moved records (path-form `canon:` included); A-style shards (`.md` only); a failing `show` keeps forms and counts its REF. B's wiki links are 2 216 lines, not the planned 143 (another unit). Code comments cite this spec's former headings "CLI", "Targets N", "W_before": now headings of `docs/canon/w-measurement.md`, to repoint.
