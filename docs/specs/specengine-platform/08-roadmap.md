---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 08. Roadmap: phases, import, criteria, risks

## 1. SpecEngine repository layout

Built: `CLAUDE.md` "Layout" and each subtree's README; the crates' planned roles: 05 §1. Still to come: `specengine-http`'s writes, `/mcp` and the embedded UI, the `spec` binary's alias `specengine`.

## 2. Phases

Estimates are rough, for one developer with agents. **MVP = Phase 0-2 on CLI + MCP**, no web UI. Enough for the main requirement: reading tasks, sending them back for elaboration, approval, the discrepancy queue.

### Phase 0. Decisions and spikes — done 2026-09-29

- ADR-0001…ADR-0025 with canon diffs; spikes: `docs/features/phase-0-spikes.md`.
- Carried to Phase 1: pre-code reading (tracey sources and a run on one pilot crate, input for ADR-0019; limpet `anchor.rs`, sem, cgr docs, fiberplane/drift, amiss, `/speckit.converge`).
- **Hold W** (`docs/canon/documentation-system.md` §1) is a standing rule: each task extracts its slice of 04-08 into `docs/features/<slug>.md` and moves the truth into canon on shipping; an exhausted section of 04-08 is shortened, an exhausted document gets `status: shipped`. Worst W (`spec check`'s summary): ≈ 109 KB against ≤ 40 KB, driven by 05 and 04.

### Phase 1. Reading core — done 2026-10-05

- Shipped (open questions: crate READMEs, `docs/canon/*`): the parser, the SQLite/FTS5 index, CLI passes 1, 3 and 4 (`bundle`), check increments 1–4 (`check` to `--changed`, the gate, process rules ADR-0031), index compaction, shards (ADR-0028, ADR-0030), MCP stdio reads, layer A identity, pilot schemes (§3 AC-10), import records, gaps and layout (§3 AC-6, §4.3), token calibration, pilot W (§3 AC-1, `docs/canon/w-measurement.md`).
- **Pilot projects** (ADR-0008): `specengine.toml` and an importer for each, dry-run import, "before / after / hashes" reports; W on every task document.
- **Migration order** (owner, 2026-10-05; no ADR, `docs/features/pilot-w.md` AC-12): pilot A first, then B, after an ADR on hyphenless codes superseding ADR-0009 (decision table there).

### Phase 2. Queue and tasks — in progress since 2026-10-05, ~2 weeks  ⟵ value for the owner

- Slices: `proposal-apply` (1: `apply_proposal` for `update`), `ui-shell` (Phase 4 on mocks, ADR-0033), `queue-export` (2: backup, restore), `agent-intake` (3: MCP intake), `queue-path-targets` (4: path targets for overlays), `plugin-skills` (`.mcp.json`, skills; the owner's check open) shipped 2026-10-05; `decision-apply` (5: answers as decision records), `daemon-read` (`specengine-http`) shipped 2026-10-06 (canon: the index); next, in order: `proposal-kinds` (`create`; then `interpretation`, `amendment`, `decision`), `task-package` (ADR-0027), `decision-staging` (ADR-0035), `ui-live`; UI-only, beside them: `ui-health`.
- Tasks, per project: states, the versioned stack-neutral package (ADR-0027), `spec_snapshot`, `stale`, `changes_requested`.
- CLI: `task …`, `round new/answer`.
- MCP lever (Phase 0 spike): `_meta["anthropic/maxResultSizeChars"]` (500 000) declared; open: does it act, the maxima (`docs/canon/mcp-read.md` "Owner's check": 48-60 k on 2.1.288).
- MCP: `get_task`, `claim_task`, `submit_plan`, `report_run`; no agent tool stages or decides (ADR-0035); the staging forms `review_proposal`, `approve_task`: Phase 5 (Phase 0's is a demo, `crates/specengine-mcp/README.md`).
- Daemon `spec serve` (no UI): HTTP API + SSE (reads: `specengine-http`); agents' MCP is the `spec mcp` stdio bridge to it (MCP HTTP after MVP, 07 s1.1).
- **Plugin** (06 §8; root `README.md`): hooks (07 §4, `gate` fail-closed), task prompts, stack-neutral roles, `/feature`; stack roles from a stack-profile plugin or the project (ADR-0027).
- Live check: 2-3 real tasks of **each** pilot project go through the full cycle (ADR-0008); stack neutrality: a synthetic non-Rust fixture in tests (07 §1.2, P2-5).

### Phase 3. Code and drift — ~2-3 weeks

- Symbol index (layer A: tree-sitter; layer B: the Bevy `schedule_data` dump; layer C: `ra_ap_ide` per ADR-0020 — B or C is required for generic system instances, 05 §5.1), Bevy detector on resolved types, signatures in the bundle, `find_symbols`, `get_impact`.
- Layer C: items under attribute proc macros (`#[tokio::main]`-style) lose their monikers with the proc-macro server on; the scan must map an item through its expansion (`crates/specengine-ra/README.md`).
- Markers `@implements/@verifies/@configures/@assumes` (with revision `@N`), `spec.lock` with `file_blob`, drift cascade (blob OID → normalized text → AST), `spec verify [--tests] [--changed]` with Fix/Check/Pre-existing verdicts and exit codes 0/1/2, `spec bump`, `lock accept [--editorial]`, `refs`, `unmapped`, `get_impact --since`.
- Existing ID citations in code automatically become **weak** `mentions` links. Markers appear in new work and when code is touched; they are not added in bulk (ADR-0016).
- Check codes lifted into constants are the first source of machine bindings `check → const → test`.

### Phase 4. Web UI — ~2-3 weeks

Screens and SSE per 07 §3, begun on mocks before the daemon (ADR-0033, `ui/README.md`): `ui-shell` shipped 2026-10-05, `ui-tree-node`, `ui-graph`, `ui-tasks`, `ui-home` and `ui-markdown` (ADR-0036) 2026-10-06; next `ui-health`, `ui-live`, `ui-round` (endpoints: their "Open").

### Phase 5. Maturity — ~2 weeks

- Full migration of the remaining pilot projects (A migrates first in Phase 2-3, then B; order: Phase 1).
- A `shared` project (common library) linked from several projects.
- Metrics and compaction, notifications, URL-mode elicitation onto UI cards (staging only, ADR-0035).
- Optional: queue export to Beads/Task Master, spec digest into `AGENTS.md`/`CLAUDE.md` for external reviewers. No vector search (04 §2.1).

### Phase 6. Planning: roadmap and pool — ~2-3 weeks (after the main work)

Releases, priority, a `depends_on` chain, range estimates, a computed position, the pool of unplaced work: `docs/features/roadmap.md` (draft). Needs the Phase 1 CLI, Phase 2 tasks and `apply_proposal`; the screen, Phase 4.

**Total**: MVP in ~5-6 weeks, full scope in ~11-14 weeks + Phase 6 ~2-3.

## 3. Product acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| AC-1 | **Task W**: median on the pilot projects ≤ 40 KB or the project's own target | `bundles` log on every task document per pilot. **Measured 2026-10-05** (`specengine-eval w`, budget 10 000, debug; A / B): tasks 70 / 51; median (p90, max) W_before 533 076 (743 566, 922 383) / 225 562 (396 166, 483 850), W_after 43 061 (45 048, 45 760) / 27 233 (39 154, 41 216), W_after_followups 101 031 (160 515, 200 347) / 76 516 (306 313, 417 480); third_step 70 / 21, incomplete 67 / 15, refused 0, unresolved 100 / 0, wiki_links 1 030 / 2 216; 40 KB (not an AC) unmet on both W_after_followups medians, met on B's W_after; bundle ms median 2 035 / 640. Phase 2 re-measures: `bundles` log, `--task` at `bundle_task`, follow-ups, live-check tasks |
| AC-2 | **AST isolation**: `cargo fmt`, edits to comments and neighbouring functions do not change a symbol's hash | property test over all symbols of the pilot projects. **Phase 0 baseline met for fmt and comments**: 100 % on both pilots (`specengine-eval ast-hash`, 05 §5.2); the property test is Phase 3 |
| AC-3 | **Single door**: no agent-facing MCP tool modifies spec files | call every tool on a temp repo + `git status` is empty |
| AC-4 | **Task approval**: without an approved task an agent cannot write a file in `zones.code` (`selective` mode); same with the daemon stopped; in `observe` mode the write passes and a finding is recorded | hook integration test |
| AC-4b | **Nothing is blocked by a discrepancy** (ADR-0012): an open proposal does not change the task status, several proposals can be open on one node, applying the second one rebases | test |
| AC-5 | **No repeated questions**: `ask_question` on an already answered question returns the decision instead of opening a new one | test: a repeated call yields `decision`, the question count does not grow. **Met 2026-10-05** (`agent-intake` AC-04); a decided item returns its record (`decision-apply` AC-11, 2026-10-06) |
| AC-6 | **Lossless import**: all records of the corpus are imported, verbatim-text hashes match; **the project's full test suite is green** after migration | importer report + the project's `cargo test`. **Dry run met 2026-10-04** (`specengine-eval layout`): A 2 242 of 2 249 definitions matched (7 headers core rejects), B 251 of 251; none unexplained, enforce clean; B one residue, a corpus fact. Suite: at migration |
| AC-7 | **Bundle determinism**: one state → one `bundle_hash` | test. **Re-confirmed on both pilots** 2026-10-05 (`w`: every bundle twice, `nondeterministic` 0) |
| AC-8 | **Machine-verified**: `verified` is set only after SpecEngine itself runs the `@verifies` tests | test |
| AC-9 | **Staleness**: editing a node from the `spec_snapshot` of a `ready` task makes it `stale`, the bundle shows the diff | test |
| AC-10 | **Performance**: full symbol hash ≤ 2 s per 300 kLOC, full index ≤ 10 s for hundreds of md files; per-file increment ≤ 200 ms. The `ra_ap_ide` layer is measured separately | benchmark on the pilot projects. **`ra_ap_ide` layer measured in Phase 0** (`specengine-eval ra`, 05 §5.1): cold 33–65 s, warm pass ≈ 0.2 s, group peak ≤ 3.76 GiB. Index (`specengine-eval index`, 2026-10-03, dev profile): `full_ms` A 3 637, B 1 373; `one_file_ms` A 184, B 76 |
| AC-11 | **Homoglyphs**: IDs with mixed scripts are rejected with an auto-fix | test on IDs where a Latin letter is swapped for its Cyrillic look-alike (U+0420 for `P`, U+0415 for `E`). **Rejection met** (`spec check`, `check_ids.rs`); applying the fix: Phase 2 (`apply_proposal`) |
| AC-12 | **Responsiveness**: an MCP event is visible in the UI ≤ 1 s | e2e |
| AC-13 | **Meaning change without revision fails**: pre-commit fails if a node's `norm_hash` changed without a `rev` bump and without `--editorial` | test on a temp repo |
| AC-14 | **"Could not verify" ≠ "fresh"**: an unparseable file yields `cannot_verify` and exit 2, not a green run; two different unparseable items do **not** get the same hash | test (regression for the `is_extra()` trap on ERROR). **Hash half met in Phase 0** (`specengine-code` `tests/hash_traps.rs`); exit 2 of `spec verify` is Phase 3 |
| AC-15 | **Rename is not drift, twin is not a trap**: a renamed function with a non-trivial body is tracked (`MOVED`), a trivial one (< 124 B buffer) yields `LOST`/`STALE` rather than false tracking; two copies with the same skeleton yield `AMBIGUOUS` | test |

## 4. Importing existing corpora

One core for all projects (ADR-0008); corpus specifics live only in `specengine.toml` and the importer, an adapter translating the project's convention (§4.1) into SpecEngine nodes, records and links.

### 4.1. What the importer carries over

| Source in the corpus | → | Notes |
|---|---|---|
| Documents with front-matter (requirements, decisions, features) | nodes and records, header normalization | `canon:` on decisions is kept; feature criteria → `{#AC-..}` sections (ADR-0026) |
| Records as table rows (assumptions, questions, terms), also rows without an ID | one record per row, `immutable_text`; missing IDs issued (agent proposes, owner confirms) | status prefixes in cells ("closed …", "implemented in code …") → status + events; text verbatim |
| Table-style headers and journals (registry, decisions, trade-offs, blockers, milestones) | registry → computed; journal → `DEC-NNN` with `cost`; blockers → high-severity questions; milestones → tasks | decisions without `cost` → debt baseline |
| Principles and check codes | `principle`, `check`; code constants → markers | old designations → aliases (ADR-0009) |
| README files of code subtrees (Tier 1) | domain nodes | stay in place (locality), indexed as `domain` |
| Roadmap state and queue | SpecEngine tasks | the markdown becomes generated |
| An existing tree of mechanics or domains | `mechanic` + rings → domains | no tree → built **through the queue**: the analyst proposes a clustering, the owner approves (SpecEngine's first test of its own cycle) |
| Homoglyphs, links to deleted files, numbers without files | fixes or baseline | importer report |
| Project generators (`gen` commands) | stay in the project | their output is the `generated` class |
| Project test harness that reads docs | moves to `spec show --json` / the new layout | adapter first, then file moves |

### 4.2. Preservation rules

1. The verbatim text of every record is kept with its hash; the importer prints a "before / after / hashes" report and supports dry-run without writing.
2. The project's test harness is adapted **before** files are moved: otherwise sentinel tests on verbatim text and layout break en masse, and an agent reports "green" before running them.
3. Corpus debt (decisions without `cost`, broken links, duplicates, homoglyphs) is recorded in a baseline and does not block the import.
4. All IDs become Latin; old ones remain as aliases (ADR-0009).
5. Project generators stay in the project (ADR-0013); the project's checks and `spec check` run in parallel until they converge.
6. Migration happens on a separate branch and is delivered as one PR.

### 4.3. Census findings (Phase 0, both pilots)

Each finding is a per-corpus import setting, not core code (ADR-0008; `crates/specengine-import/README.md`, `docs/canon/import.md`; counts: `docs/features/import-records.md` AC-10): (a) link base → `[links] base`, `[paths] link_base`; (b) wiki links to 8 targets that exist nowhere → the baseline (§4.2.3); (c) legacy prefixes → `[ids.legacy]` aliases (§4.2.4); (d) definition vs reference → `[definitions]`; (e) locally numbered tables → `tables.local_number` (candidates for §4.1's feature-scoped IDs); at import: (f) criteria as list items → `[lists]`; (g) field/value table headers → `header_table`, key and value maps; (h) hyphenless codes → `hyphenless`, counted only (ADR-0009 needs a superseding ADR before that pilot migrates).

## 5. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Migration breaks sentinel tests and verbatim corpus texts | high | harness adapter before the move; text hashes in the report; AC-6 |
| The queue grows faster than the owner processes it | medium | nothing is blocked (ADR-0012), sorting by `severity` and number of dependent `@assumes`; batched rounds; metric "decided differently from the working answer" |
| Agents do not add markers | medium | rules in plugin roles; `unbound` on the panel; `SubagentStop` reminder; check in `spec verify` |
| The tool eats time from the main projects (scope creep) | high | MVP = CLI + MCP; live UI only after live-task validation; "enforcement first, the bot never first"; multi-user server mode out of plan (ADR-0017) |
| The core silently bends toward one project | medium | pilots of different nature connected from Phase 1 (ADR-0008); specifics only in `specengine.toml` and the importer; core tests on fixtures of all pilots |
| Bevy 0.20 changes the schedule API | medium | tree-sitter detector covered by `fixtures/bevy-mini`; observers behind a Bevy-version gate (05 §5.1); the `schedule_data` dump schema changes across minors: re-run `specengine-eval bevy-detector --dump` per Bevy upgrade |
| The MCP protocol changes again | medium | rmcp + both eras; logic in core, MCP a thin adapter; Claude Code verified on 2.1.283 (MCP README), re-check on upgrade; Streamable HTTP without GET → 405 unmeasured (stdio-only build) |
| rust-analyzer memory near the threshold | medium | whole-group peak 3.76 of 4 GiB (≈ 6 % headroom) on the heavier pilot; re-measure (`specengine-eval ra`) when a pilot grows or `ra_ap` is bumped; fallback: no proc-macro server (≤ 3.2 GiB) |
| HTTP hook is open when the daemon is down | known | `command` hook with exit 2 + managed-deny |
| Specs do not reduce the cost of change (O(n) cross-cutting edits) | fact | do not promise it; show the blast radius via `get_impact` |
