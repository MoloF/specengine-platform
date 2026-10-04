---
class: spec
status: draft
scope: [crates/specengine-eval, fixtures, docs]
ref: pilot-w analysis 2026-10-05, owner's answers 1-10, iteration-1 review decisions; 08 §2 Phase 1 "Pilot projects", §3 AC-1, AC-7
adrs: []
---

# Pilot W: every task, before and after

## Why

08 §3 AC-1 wants task W (`docs/canon/documentation-system.md` §1, §3) on both pilots; Phase 1 closes when the owner picks the migration order (08 §2). `w` replays **every task document** (shipped included, owner's rule; figures lean to history) twice on the same targets: the pilot's reading protocol on the untouched corpus (**W_before**), the shipped `spec bundle` on the after-tree (**W_after**). The bundle caps itself and follows `mentions` only into layers 2 and 4 (`docs/canon/spec-cli-bundle.md`), so follow-up reads count (**W_after_followups**). No ADR (ADR-0008, ADR-0009, ADR-0012: 40 KB recorded, never enforced, ADR-0022, ADR-0026, ADR-0027).

## Description and interactions

`specengine-eval w`, read-only on the corpus: refusals; `layout::prepare`; `layout`'s pre-verifier step writes the after-tree to `<out>/w/<label>/tree/` (files, emitted `specengine.toml`, index outputs; no baseline), core reads it (`check_input`, `SpecGraph::new`); `Layout.emission` (`emission.json`, in memory) maps after paths to sources; per task the library's `specengine_cli::bundle` and `show`, as MCP. CLI and core change only per A6. Code cites ACs, never headings.

## Data

### Tasks config

Read only by `w`: `--tasks`, else `SPECENGINE_TASKS_A` / `_B` under `--label pilot-a` / `pilot-b`, else `tasks.toml` at the corpus root; `deny_unknown_fields`.

```toml
[tasks]
include = ["docs/features/**/*.md"] # required, >= 1; census globs over source paths
exclude = ["docs/features/drafts/**"] # default []
[tier1] # optional: no Tier 1 slot without it
key = "scope" # optional; front-matter key, read by core in the after-tree
default = "docs/README.md" # optional; corpus file
[tier1.paths] # needs `key`; value -> corpus file
"area-a" = "crates/a/README.md"
```

Globs: core's matcher (`WalkScope::is_excluded`). Exit 2 `<tasks>:<line>: message` (line 1 without a span), nothing written: unknown key, wrong type, empty `include`, an `include` or `exclude` glob matching no walked source document, `[tier1.paths]` without `key`, a path not clean-relative or not a corpus file.

**Recipe** (no pilot named in the repository): `include` = the documents the pilot hands agents as tasks; `exclude` = its draft or archive directories; an area key after the key map → `key`, per value the Tier 1 README its Tier 0 routes to; `default` = the Tier 1 README over the tasks. Re-run until no refusal; a zero `slots.tier0` or `.index` is a finding. Accepted: A no `default` (15 unparseable-header tasks: `slots.tier1.none`); B no `[tier1]` (its one area README, far over the Tier 1 cap, makes 40 KB unreachable).

### CLI

`specengine-eval w … [--tasks <toml>] [--budget <n>]`, other flags as `layout` (timeout after `prepare`), fixture `pilot-w/one`. `--budget` 1..=4294967295 (negative: clap's usage error), default 10 000 (`bundle_task`, 07 §5), as `BundleRequest.budget` (the emitted `bundle_node` unread).

Exit 0 measured (`"timeout"` on overrun); 2 refused, nothing written: the tasks config, `--budget`, `layout`'s refusals, `--out` and corpus nested, a symlinked `<out>/w[/<label>]`, a before scheme without `[project] slug`; 1: a CLI error but a frame refusal, I/O.

stdout envelope as `layout`, keys sorted; `result` exactly (a W statistic: bytes, or `"refused"` / `"failed"`):

```json
{"tasks":4,"budget":{"tokens":10000,"source":"default"},
"w_before":{"median":0,"p90":0,"max":0},"w_after":{…},"w_after_followups":{…},"docs_needed":{…},
"third_step":0,"worst":{"w_before":"task-3",…},
"slots":{"tier0":0,"index":0,"shard":0,"tier1":{"key":0,"default":0,"none":0},"unmapped":0},
"targets":{"refs":0,"text":0,"outline":0,"header":0,"not_included":0},
"citations":{"unresolved":0,"unchecked":0,"wiki_links":0},
"followups":{"refs":0,"bytes":0,"truncated":0,"incomplete":0},"refused":0,"failed":0,
"nondeterministic":0,"fill_percent":{"median":0.0,"max":0.0},"bytes_per_token":0.0,"bundle_ms":{"median":0,"p90":0}}
```

`source` `flag` | `default`; `slots.tier0`, `.index` corpus bytes (0: unset, missing); `.shard`, `.tier1.*` task counts. No path, ID, glob or key on stdout.

Detail in `<out>/w/<label>/` (emptied first): `tree/`, `home-1|2/`, `bundles/task-N.txt` (pass 1 body per answered task), `tasks.json`, per task: `task`, `source`, `after`, `refs`, `targets` (with `sources`), `citations` (`written`, `line`), wiki-link lines, `slots` (`slot`, `side`, `path`, `bytes`, `read`: corpus | tree), the figures, `bundle` (`Bundle`'s counts, `bundle_hash`, `forms`) or `message`, `followups`, `bundle_hash_2`, `{bundle,bundle_2,show}_ms`.

## Rules and edge cases

- **Tasks**: `Layout.emission.documents` entries whose source matches `include`, no `exclude`, their after file not `Standing::Generated` (no other filter): Tier 3 (shipped, abandoned) and a failed or absent front-matter are tasks, a Tier 3 D bundled ` | archived` (`docs/canon/spec-cli-bundle.md` "Input"); `task-1…n` in source-path byte order. An after file unwalked or without a document node → `failed`, with a message.
- **Targets N**: D; each record layout moved out of D (`emission.definitions` with D's source, another after file: the node at (after, ID)); the written end (`Edge::written_end`) of each resolved link written (`Edge::file`) in D's or those records' after files, of every kind. A file link dangling in the tree but resolving in the corpus from D's source (layout moved an end) → its `emission.documents` after path; dangling in both → `unresolved`; skipped or unchecked → `unchecked`; `[[…]]` pairs on one line of D's or its records' after files, fences included → `wiki_links`; none followed. REFs, once each, (path, ord) order: `<slug>/ID` for a feature-scoped ID in a feature document (bare, it names every feature's holder: `docs/canon/spec-check-links.md` "Resolution"), else `SpecGraph::name` (ID, else path); never the written form.
- **Sources** of a node: `emission.definitions` entries with its after path and ID (every holder, A5), else the `emission.documents` entry with that after path, else none (`unmapped`).
- **W_before** = corpus bytes of the distinct paths: Tier 0 (`[paths] tier0`, before scheme), Tier 1, index root (`[paths] index`), the first shard (path order) whose core parse links D's source (as core: `link_base`, percent-decoding), the sources of D and N; unset or missing: 0. Shards: the `index = true` generator's, else the other corpus-present `writes` of the generator writing `[paths] index` (A's). `docs_needed` = those sources outside the four slots; `third_step` = tasks with > 3.
- **Tier 1**: `key`'s value in D's after-tree front-matter (string, or each string of a list; `Node.fields` / `.extra`) in `[tier1.paths]`; several → the largest in corpus bytes (path order on a tie); none → `default`, else 0. One path, both sides.
- **W_after** = Tier 0 + Tier 1 + `Bundle.bytes` (`Globals{root: tree}`, REFs, budget), not post-processed; Tier 0/1 from the tree when it holds the path, else the corpus. No index, no de-duplication.
- **Outcomes**: `reason` (exit 1) → `failed`; exit 2 naming the minimum or the ceiling (Fitting step 1: `below this bundle's minimum`, `-character ceiling`) → `refused`; any other error ends the run.
- **Follow-ups**: `text`/`outline`/`header` = the targets layer by `form` (merged targets inside), every bundled task; follow-ups (all in `refs`) = outline and header items (the canonical REF at the item's (path, line, name), else its name), a refused task's every REF; `bytes` = Σ `render_text(show(REF))` bytes per distinct one (cap applied); `truncated` = per task, those answered with `[truncated: …]`; a `show` not answering → the task `failed` (message names the REF), forms and REFs still counted; `incomplete` = tasks with > 3 (05 §6). **W_after_followups** = W_after + them (refused: Tier 0 + Tier 1 + them).
- **Aggregates**: nearest rank over all tasks: ascending, p = v[⌈p·n/100⌉], median = p50, max = v[n]; refused and failed rank above every number (task order), a statistic landing there prints the word; `worst` = each W statistic's max task. `fill_percent` (tokens × 100 / budget), `bytes_per_token` (Σ bytes / Σ tokens): one decimal, bundled tasks; `bundle_ms` pass 1.
- **Determinism**: pass 2 recomputes every bundle in reverse order on a fresh `home-2/`; `nondeterministic` = tasks whose hash or outcome differs (08 AC-7). Sorted orders only; two runs: identical detail but `*_ms`.
- **Data directories**: `Env {cwd: tree, home: <out>/w/<label>/home-1|2, xdg_data_home: None}`, never `Env::from_process`; nothing under the corpus, `HOME` or the repository.

## Assumptions and risks

A1 thresholds > 3; A2 nearest rank, integer bytes; A3 budget 10 000, `--budget` re-runs; A4 after-side Tier 0/1 from the tree; A5 duplicates → every holder; A6 a bundle defect a pilot exposes: fixed with an invented-fixture regression, or recorded here.

Risks: (1) ~4.2 B per estimated token (Russian): a full body ≈ 42 KB; measure, don't tune. (2) Time (debug; each call re-parses the tree): ~0.6 s a bundle at A's scale (1 601 files), ~0.26 s a `show`: A ≈ 180 s both passes, ≤ ~680 s with follow-ups (cached per REF); pilots run debug, `--timeout 3600`. (3) Dangling mentions (A 4 193), wiki links (B 143) understate N both sides; counted. (4) `show` cuts at 40 000 characters (B up to ~129 KB): `truncated`. (5) Iteration cap: drop the shard slot, then Tier 1 to `default`. (6) Worst W's third Tier 2 term is 08 (15 786 B): growing it, the index root, or this spec past it counts.

## Acceptance criteria

`tests/w_cli.rs` over `fixtures/pilot-w/{one,two}`, two conventions: `census.toml` + `[layout]`, `specengine.toml`, `tasks.toml`, `expected.json` (= `result` but `bundle_ms`); no raw Cyrillic (`anonymity.rs`). `one`: shards (`index = true`), a Tier 1 table, a task whose records layout moves (one holding a link), a nested task, a shipped task, a generated document in the globs, multi-byte text; `two`: no index, `default` only, a `slug`-moved document a task links, a duplicate definition, an even task count.

- [ ] AC-01 A header core rejects, no front-matter, a shipped document (bundled ` | archived`): tasks; `class: generated`, outside the globs: not; `task-N` by source path. Each refusal → exit 2 at `<tasks>:<line>`, `--out` empty. M1: filtering on liveness or a parsed header → red. M2: a generated task → red. M3: `deny_unknown_fields` off → red.
- [ ] AC-02 Front-matter ID, inline ID, legacy alias, `<slug>/ID`, `../` file link, own section by a bare feature-scoped ID another feature defines too, a record moved out of D, a link inside it, a link to the `slug`-moved document → canonical `refs`, `failed` 0; corpus-dangling → `unresolved` 1, `[[…]]` → `wiki_links` 1, non-document link → `unchecked` 1, none a REF. M1: written forms as REFs → `failed` ≥ 1. M2: dangling uncounted → red. M3: a bare feature-scoped REF → red. M4: moved records or their links dropped → red. M5: that link `unresolved` → red.
- [ ] AC-03 Slots = `expected.json`: two targets in one source document count it once; the larger of two matched READMEs (a larger one unmatched); the second shard (the first does not link the task), also from an A-style generator (scratch copy: no `index = true`, the shards in `writes`); a duplicate → both sources; a moved record: D's source once, its body in `Bundle.bytes`; `third_step` 1. M1: per-target counting → red. M2: after-tree bytes before → red. M3: the table's largest README → red. M4: shards from `index = true` only → red.
- [ ] AC-04 The test's own `specengine_cli::bundle` (own `Env`, `tree/`) per task: `bytes`, `bundle_hash` = `tasks.json`, body = `bundles/task-N.txt`; `tree/` = `layout`'s but `.spec-debt.toml`. M1: chars or tokens for bytes → red. M2: not-included list stripped → red. M3: no index outputs → red.
- [ ] AC-05 Forms = the targets layer; `followups.bytes` = the test's `show` + `render_text`; `incomplete` 1; at a small `--budget` one task refused, in `tasks`, W_after max `"refused"`, its moved record's `show` in `followups.bytes`. M1: bundled tasks only → red. M2: refused → 0 → red. M3: follow-ups from `tokens_est` → red.
- [ ] AC-06 Aggregates on `two` (even n) = `expected.json`; stdout = the whitelist. M1: averaged median → red. M2: a path on stdout → red.
- [ ] AC-07 `nondeterministic` 0; two runs, different `--out`: `tasks.json` equal but `*_ms`. M: a `HashMap` for tasks or REFs → red.
- [ ] AC-08 Tree, data directories, databases under `<out>/w/<label>/`; scratch `HOME` empty; `git status --porcelain -- fixtures/` empty; nesting, a symlinked `<out>/w` → exit 2, nothing written or deleted. M: `Env::from_process` → red.
- [ ] AC-09 `import_genre.rs` scans `src/w.rs`, `tasks.toml` strings forbidden; `build_graph.rs`: eval's direct normal dependencies gain exactly `specengine-cli` (path, no features); the CLI's graph, CLI_FORBIDDEN unchanged. M1: a fixture glob or key in `w.rs` → red. M2: another dependency → red.

Owner's check, by an agent the owner instructs:

- [ ] AC-10 Tasks configs (the owner's pilot configs) per the recipe; `w --label pilot-a|pilot-b` (`#[ignore]`, one at a time; child: `PATH`, the label's variables and `SPECENGINE_TASKS_*`, empty `HOME`) exits 0, not `"timeout"`; read-only proofs equal, `anonymity` green, `nondeterministic` 0, `failed` 0 or each recorded (A6). Recorded, dated, here and in 08 §3 AC-1: the table, budget, profile, `bundle_ms`; 40 KB met or not (a finding); the Phase 2 re-measure (`bundles` log, `--task` at `bundle_task`, follow-ups, live-check tasks).

At shipping:

- [ ] AC-11 Rules in a new Tier 2 canon `docs/canon/w-measurement.md` (≤ 12 288 B); eval README `w` row (≤ 10 020 B, largest Tier 1); 08 §2 "10 tasks" → "every task document", the MCP lever → Phase 2, "Next" closed; §3 AC-1 filled, AC-7 "re-confirmed on both pilots"; check clean, worst W ≤ 109 484 B; nextest `-p specengine-eval`, clippy, fmt, mutations red; manifest and lock diff = the eval → cli edge; fixtures: only `pilot-w`.
- [ ] AC-12 Phase 1 closes when the owner names the order from the table: 08 §2 records it (no ADR), Phase 1 "done <date>"; `CLAUDE.md` "State", root README updated. `pilot-w` may ship before.

## Migration order: decision table (filled at AC-10)

| | A | B |
|---|---|---|
| W_before / W_after / W_after_followups: median (p90, max) | | |
| 40 KB on the W_after_followups median | | |
| third_step; incomplete, refused, failed | | |
| unresolved, wiki_links, truncated | | |
| AC-6 dry run (08 §3) | 2 242 / 2 249 (7 headers core rejects) | 251 / 251; 1 residue; hyphenless: an ADR superseding ADR-0009 |
| baseline debt (`import-layout.md`) | 5 336 | 340 |
| documents cited from code / citations (`import-records.md`) | 92 / 911 | 16 / 44 |
| **Owner's choice: migrates first** | | |

## Out of scope

Phase 2: `--task`, the `bundles` log, the follow-up signal in `runs`, MCP `maxResultSizeChars`. Not planned: bundle latency, the bundle following mentions, header references as typed links, the migration (08 §4), meeting 40 KB.

## Implementation

Iteration 1 (eval only, `layout`'s output unchanged): `w.rs`; `main.rs` `WArgs`; `harness.rs` `PILOT_TASKS`; `layout.rs` `pub(crate) write_tree` (`layout`'s staging), `check_out` per measurement, `Setup` fields `pub(crate)`; `Cargo.toml` + `specengine-cli` (path). Iteration 2 (open): Targets N, W_before shards, Follow-ups as amended.
