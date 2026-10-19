---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 08. Roadmap: phases, import, criteria, risks

## 1. SpecEngine repository layout

Built: `CLAUDE.md` "Layout", each subtree's README; planned crate roles: 05 §1. Still to come: `specengine-http`'s other writes, `spec mcp` and `/mcp` (on a trigger), alias `specengine`.

## 2. Phases

Estimates are rough, for one developer with agents. **MVP = Phase 0-2 on CLI + MCP**, no web UI. Enough for the main requirement: reading tasks, sending them back for elaboration, approval, the discrepancy queue.

### Phase 0. Decisions and spikes — done 2026-09-29

- ADR-0001…ADR-0025 with canon diffs; spikes: `docs/features/phase-0-spikes.md`.
- Carried to Phase 1: pre-code reading (tracey sources and a run on one pilot crate, input for ADR-0019; limpet `anchor.rs`, sem, cgr docs, fiberplane/drift, amiss, `/speckit.converge`).
- **Hold W** (`docs/canon/documentation-system.md` §1) is a standing rule: each task extracts its slice of 04-08 into `docs/features/<slug>.md` and moves the truth into canon on shipping; an exhausted section of 04-08 is shortened, an exhausted document gets `status: shipped`. Worst W (`spec check`'s summary): ~108 KB against <= 40 KB, driven by 05 and 04.

### Phase 1. Reading core — done 2026-10-05

- Shipped (open questions: crate READMEs, `docs/canon/*`): the parser, the SQLite/FTS5 index, CLI passes 1, 3 and 4 (`bundle`), check increments 1-4 (`check` to `--changed`, the gate, process rules ADR-0031), index compaction, shards (ADR-0028, ADR-0030), MCP stdio reads, layer A identity, pilot schemes (§3 AC-10), import records, gaps and layout (§3 AC-6, §4.3), token calibration, pilot W (§3 AC-1, `docs/canon/w-measurement.md`).
- **Pilot projects** (ADR-0008): `specengine.toml` and an importer for each, dry-run import, "before / after / hashes" reports; W on every task document.
- **Migration order** (owner, 2026-10-05; no ADR, `docs/features/pilot-w.md` AC-12): pilot A first, then B, after an ADR on hyphenless codes superseding ADR-0009 (decision table there).

### Phase 2. Queue and tasks — in progress since 2026-10-05, ~2 weeks  ⟵ value for the owner

- Slices: `proposal-apply` (1: `apply_proposal` for `update`), `queue-export` (2: backup, restore), `agent-intake` (3: MCP intake), `queue-path-targets` (4: path targets), `plugin-skills` (`.mcp.json`, skills; owner's check open) shipped 2026-10-05; `decision-apply` (5: answers as decision records), `daemon-read` (`specengine-http`) shipped 2026-10-06; `proposal-kinds` (6: `create`), `task-package` (7, ADR-0027) shipped 2026-10-07 (UI: Phase 4); `decision-staging` (ADR-0035), `plugin-gate`, `round` (ADR-0038), `ui-round` (Phase 4), `plugin-roles`, `spec-serve` 2026-10-08; `kind-{interpretation,decision,amendment}`, `health-left` (`docs/canon/health.md`) 2026-10-09; left: the live check; `spec mcp`, the bridge, on a trigger.
- MCP lever (Phase 0 spike): `_meta["anthropic/maxResultSizeChars"]` (500 000) declared; open: does it act, the maxima (`docs/canon/mcp-read.md` "Owner's check": 48-60 k on 2.1.288).
- MCP: `get_task`, `claim_task`, `submit_plan`, `report_run`, `complete_task` shipped; no agent tool stages or decides (ADR-0035); the staging forms `review_proposal`, `approve_task`: Phase 5 (Phase 0's is a demo, `crates/specengine-mcp/README.md`).
- Daemon `spec serve` shipped (the UI inside since Phase 4 `ui-embed`): `specengine-http` per data directory; `spec mcp` on a trigger (05 §1 (6)); no gate needs it (ADR-0039).
- **Plugin** (06 §8; `docs/canon/plugin.md`): the gate hook, skills, stack-neutral roles, `/specengine:feature` shipped; other hooks (07 §4) later; stack roles from a profile plugin or the project (ADR-0027).
- Live check: 2-3 real tasks of **each** pilot project go through the full cycle (ADR-0008); stack neutrality done 2026-10-08: `fixtures/spec-c` (07 s1.2 P2-5).

### Phase 3. Code and drift — ~2-3 weeks

- Slices in order: `code-refs` (shipped 2026-10-10), `norm-hash` (shipped 2026-10-10, ADR-0040), `spec-lock` (shipped 2026-10-12, ADR-0041), `spec-verify` (4a, shipped 2026-10-13, ADR-0042), `rev-discipline` (4b-1: the raise at apply, `approve --editorial`, shipped 2026-10-14, ADR-0043), `rev-rule` (4b-2: `rev-not-raised`, `spec bump`, `verify --staged`, shipped 2026-10-15, ADR-0043), `drift-queue` (4c-1: `spec propose drift`, the approve running accept, shipped 2026-10-15, ADR-0045), `drift-follow-up` (4c-2: `spec reject --follow-up`, shipped 2026-10-15, ADR-0045), `hash-levels` (5a-1: levels, fingerprints, entry points, shipped 2026-10-16), `hash-levels-ron` (5a-1b: the RON value hash, shipped 2026-10-16), `hash-levels-eval` (5a-2: `ast-hash`, `ron`, pilot figures, shipped 2026-10-16, TE-1 the owner's pilot run open; split 2026-10-13), `symbol-discriminator` (6a-1: in-file twins discriminated, the display signature, shipped 2026-10-16), `symbol-index` (6a-2: the code index, `spec index`, `refs` from it, AC-10, shipped 2026-10-16; split 2026-10-13; 6a after 5a-2, before 5b), `symbol-reads` (6a-3: lookup tiers, `spec symbols`, `spec unmapped`, shipped 2026-10-16, TE-4 the owner's pilot run open; split 2026-10-15), `code-tools` (6b-1: MCP `find_symbols`, `refs`, `unmapped`, `read-spec`, plugin 0.9.0, `CODE_INDEX_FORMAT` 2, shipped 2026-10-16, TE-3 the owner's pilot run open; split 2026-10-16), `lock-levels` (5b-1: lock format 2, its writers, level-aware verify, draft, ADR-0050; after 5a-1, 5a-1b, 6a-2, 6b-1 and 5a-2's whole TE-1, the owner's gate; split 2026-10-16), `lock-rebase` (5b-2: `spec lock rebase`, draft, ADR-0050; right after 5b-1), `anchor-states` (5c, draft), `verify-node` (6b-2: `spec verify --node`, MCP `check_binding`, `implement`, the next plugin MINOR (2026-10-19), draft; after 5c; split from `code-tools` 2026-10-16), `code-index-speed` (6c, if AC-10 fails), `impact-bundle` (7a `bundle-bindings`, 7b `impact`, draft, ADR-0046; 7a after 5b), `verify-tests` (8a-1: `spec verify --tests`, the `[verify] tests` template, the `verify.tests` event, shipped 2026-10-16, ADR-0044, TE-9 the owner's pilot run open) and `verify-task` (8a-2: plain `spec verify --task`, draft; after 5c; split 2026-10-16), `tested-evidence` (8b: `[[tested]]` evidence in `spec.lock`, format 3, draft; after 4a, 8a, 5b), `health-code` (9a-1: rows 4-5 in `spec health` and its route over the code index, `[[health.bound]]`, shipped 2026-10-19, ADR-0047, ADR-0051, TE-7 the owner's pilot run open) and `health-code-ui` (9a-2: Health's parts for them, shipped 2026-10-19, ADR-0047, split 2026-10-16), `health-ui-follow-up` (9a-3: Health re-reads on `proposal.approved` and `.apply_failed` with one trailing read, 9a-2's review nits; UI only, shipped 2026-10-19, no ADR) and `health-drift` (9b: row 6, after 4a and 7a, refreshed after 9a-3; split 2026-10-10), `bevy-roles` (10a-1: derive, `impl`, call and `add_message` roles in the code index, `[code] frameworks`, `spec index` counts, `INDEX_FORMAT` 10, `CODE_INDEX_FORMAT` 3, shipped 2026-10-16, TE-7 the owner's pilot run open), `bevy-roles-reads` (10a-2: roles in `spec symbols` and `find_symbols`, plugin 0.10.0, `CODE_INDEX_FORMAT` 4, shipped 2026-10-16, TE-8 the owner's pilot run open; split 2026-10-16) and `bevy-dump` (10b: the project's schedule dump as annotation, draft, ADR-0048; after 10a-2, parked 2026-10-16: slice 12 no longer reads the dump; split 2026-10-10), `layer-c` (11a-1: the `specengine-ra` executable, protocol v1, shipped 2026-10-19, ADR-0049), `layer-c-resolve` (11a-2: `spec resolve`, the advisory snapshot, `scip` retired, shipped 2026-10-19, ADR-0049, TE-6 the owner's pilot run open) and `layer-c-reads` (11a-3: monikers in `spec symbols` and `find_symbols`, the next plugin MINOR; after the owner's TE-6; draft, ADR-0049; split 2026-10-19; 11b `layer-c-macros` when > 1 % of a pilot's items sit under attribute macros; 11c `layer-c-registration` after 10b (parked; resumes with it), for what the dump cannot give, gated by `[code] frameworks`; split 2026-10-10), `machine-bindings` (12a `constant-census`: `specengine-eval constants`, a report of string constants carrying record codes, shipped 2026-10-19, no ADR, TE-4 the owner's pilot run open; 12b `machine-bindings`, bindings without a marker under a decision amending ADR-0020, and 12c `constant-reach`, not drafted until a user exists: this repository's dogfooding if the owner wants it, or pilot B after the ADR superseding ADR-0009 and its migration; `docs/features/constant-census.md` "Later"; split 2026-10-10). Build order from 2026-10-16 (decided under the owner's pre-approval, review open): 6b-1, 10a-1 `bevy-roles`, 10a-2 `bevy-roles-reads`, 8a-1 `verify-tests`, 9a-1 `health-code` (shipped) -> 9a-2 `health-code-ui` (shipped) -> 12a `constant-census` (shipped) -> (11a-1 `layer-c` (shipped) -> 11a-2 `layer-c-resolve` (shipped) -> the owner's TE-6 -> 11a-3 `layer-c-reads`, only if 5a-2's TE-1 is still outstanding; split 2026-10-19); 9a-3 `health-ui-follow-up` (shipped, while 5a-2's TE-1 and 11a-2's TE-6 are the owner's); 5b-1 takes over at the next slice boundary once the owner's 5a-2 TE-1 lands, then 5b-2 -> 5c -> 8a-2 `verify-task` -> 6b-2 -> (6c) -> 7a -> 7b -> 8b -> 9b (refreshed after 9a-3); 10b parked (its consumer is gone). Until 5b-1 ships, no slice edits `specengine-code`'s `levels.rs`, `hash.rs`, `ron/` or `Cargo.lock`'s tree-sitter entries: TE-1's figures describe the bytes 5b-1 freezes.
- Symbol index (layers A tree-sitter, B the Bevy `schedule_data` dump, C `ra_ap_ide`, ADR-0020; generic instances need B or C, 05 §5.1), Bevy detector on resolved types, signatures in the bundle.
- Layer C: `spec resolve` stores rust-analyzer's monikers as an advisory snapshot (shipped 2026-10-19: `docs/canon/layer-c.md`); next the owner's TE-6 on both pilots, then 11a-3 shows it in the reads. 11b `layer-c-macros`: items under attribute proc macros lose monikers with the proc-macro server on; map them through the expansion (`crates/specengine-ra/README.md`).
- Markers `@implements/@verifies/@configures/@assumes` (revision `@N`), `spec.lock`, `lock accept [--editorial]`, `lock regen` (shipped: `docs/canon/spec-lock.md`), drift cascade, `spec verify [--changed | --staged]` (shipped: `docs/canon/spec-verify.md`), `spec verify --tests` (shipped: `docs/canon/spec-verify-tests.md`; `--task` alone: 8a-2), `spec propose drift` (shipped: `docs/canon/proposal-drift.md`), `spec reject --follow-up` (shipped: `docs/canon/proposal-completion.md` "Reject"), `spec bump` (shipped: `docs/canon/node-revision.md` "Raising"), per-level hashes and fingerprints (shipped: `docs/canon/code-hash.md`, figures `docs/canon/code-hash-eval.md`; recorded from 5b-1), `refs` (shipped: `docs/canon/code-bindings.md`), `unmapped`, `get_impact --since`.
- ID citations in code become **weak** `mentions` (shipped: `spec refs` lists them); markers only in new or touched code (ADR-0016).
- Machine bindings `check → const → test` (slice 12): free string constants whose whole value is a record's code are reported as candidates (12a), bind a node without a marker only under a decision amending ADR-0020 (12b), and what reaches them is listed, never evidence (12c); nothing writes code (ADR-0016).

### Phase 4. Web UI — ~2-3 weeks

Screens and SSE per 07 s3, begun on mocks (ADR-0033, `ui/README.md`): `ui-shell` shipped 2026-10-05, `ui-tree-node`, `ui-graph`, `ui-tasks`, `ui-home` and `ui-markdown` (ADR-0036) 2026-10-06, `ui-health`, `ui-live`, `ui-live-tasks` 2026-10-07, `ui-round` 2026-10-08, `ui-embed` (the UI inside `spec serve`) 2026-10-10; left: the drift screens, after Phase 3.

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
| AC-2 | **AST isolation**: `cargo fmt`, edits to comments and neighbouring functions do not change a symbol's hash | property test over all symbols of the pilot projects. **Phase 0 baseline met for fmt and comments**: 100 % on both pilots (`specengine-eval ast-hash`, 05 §5.2); the property test is Phase 3 (5a-1 fixtures, 5a-2 pilots) |
| AC-3 | **Single door**: no agent-facing MCP tool modifies spec files | call every tool on a temp repo + `git status` is empty |
| AC-4 | **Task approval**: without an approved task an agent cannot write a file in a gated zone (`selective`), no daemon needed; in `observe` it passes, a finding recorded | hook integration test. **Met 2026-10-08** (`plugin-gate`); in Claude Code: the owner's check |
| AC-4b | **Nothing is blocked by a discrepancy** (ADR-0012): an open proposal does not change the task status, several proposals can be open on one node, applying the second one rebases | test |
| AC-5 | **No repeated questions**: `ask_question` on an already answered question returns the decision instead of opening a new one | test: a repeated call yields `decision`, the question count does not grow. **Met 2026-10-05** (`agent-intake` AC-04); a decided item returns its record (`decision-apply` AC-11, 2026-10-06) |
| AC-6 | **Lossless import**: all records of the corpus are imported, verbatim-text hashes match; **the project's full test suite is green** after migration | importer report + the project's `cargo test`. **Dry run met 2026-10-04** (`specengine-eval layout`): A 2 242 of 2 249 definitions matched (7 headers core rejects), B 251 of 251; none unexplained, enforce clean; B one residue, a corpus fact. Suite: at migration |
| AC-7 | **Bundle determinism**: one state → one `bundle_hash` | test. **Re-confirmed on both pilots** 2026-10-05 (`w`: every bundle twice, `nondeterministic` 0) |
| AC-8 | **Machine-verified**: `verified` is set only after SpecEngine itself runs the `@verifies` tests | test. **Run half met 2026-10-16** (`verify-tests`, 8a-1: `spec verify --tests` on the owner's terminal, `docs/canon/spec-verify-tests.md`; CLI `tests/verify_tests_*.rs`); `verified` from its evidence: 8b `tested-evidence` |
| AC-9 | **Staleness**: editing a node from the `spec_snapshot` of a `ready` task makes it `stale`, the bundle shows the diff | test |
| AC-10 | **Performance**: full symbol hash ≤ 2 s per 300 kLOC, full index ≤ 10 s for hundreds of md files; per-file increment ≤ 200 ms. The `ra_ap_ide` layer is measured separately | benchmark on the pilot projects. **`ra_ap_ide` layer measured in Phase 0** (`specengine-eval ra`, 05 §5.1): cold 33–65 s, warm pass ≈ 0.2 s, group peak ≤ 3.76 GiB. Index (`specengine-eval index`, 2026-10-03, dev profile): `full_ms` A 3 637, B 1 373; `one_file_ms` A 184, B 76. Code index (`specengine-eval code-index`, 6a-2: `docs/canon/symbol-index.md` "AC-10 measurement"): release, `hash_ms` × 300 / `kloc` ≤ 2 000, `one_file_ms` ≤ 200, `noop_ms` ≤ 200; pilot figures: the owner's TE-8 |
| AC-11 | **Homoglyphs**: IDs with mixed scripts are rejected with an auto-fix | test on IDs where a Latin letter is swapped for its Cyrillic look-alike (U+0420 for `P`, U+0415 for `E`). **Rejection met** (`spec check`, `check_ids.rs`); applying the fix: Phase 2 (`apply_proposal`) |
| AC-12 | **Responsiveness**: an MCP event is visible in the UI ≤ 1 s | e2e |
| AC-13 | **Meaning change without revision fails**: pre-commit fails if a locked node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries | test on a temp repo. **Met 2026-10-15** (`rev-rule`, 4b-2: `rev-not-raised` in `spec check --staged`, `docs/canon/node-revision.md` "Raising"; CLI `tests/rev_rule_hook.rs`, a real hook on temp repos); the raise at apply `rev-discipline` (4b-1, 2026-10-14: `docs/canon/proposal-revisions.md`); the pilots' run is the owner's |
| AC-14 | **"Could not verify" ≠ "fresh"**: an unparseable file yields `cannot_verify` and exit 2, not a green run; two different unparseable items do **not** get the same hash | test (regression for the `is_extra()` trap on ERROR). **Hash half met in Phase 0** (`specengine-code` `tests/hash_traps.rs`); **exit 2 met 2026-10-13** (`spec verify`, every mode: `docs/canon/spec-verify.md` "Verdict and exit", CLI `tests/verify_cannot.rs`) |
| AC-15 | **Rename is not drift, twin is not a trap**: a renamed function with a non-trivial body is tracked (`MOVED`), a trivial one (< 124 B buffer) yields `LOST`/`STALE` rather than false tracking; two copies with the same skeleton yield `AMBIGUOUS` | test |

## 4. Importing existing corpora

One core for all projects (ADR-0008); corpus specifics live only in `specengine.toml` and the importer, an adapter translating the project's convention (§4.1) into SpecEngine nodes, records and links.

### 4.1. What the importer carries over

| Source in the corpus | → | Notes |
|---|---|---|
| Documents with front-matter (requirements, decisions, features) | nodes and records, header normalization | `canon:` on decisions is kept; feature criteria → `{#AC-..}` sections (ADR-0026) |
| Records as table rows (assumptions, questions, terms), also rows without an ID | one record per row, `immutable_text`; missing IDs issued (agent proposes, owner confirms) | status prefixes in cells ("closed …", "implemented in code …") → status + events; text verbatim |
| Table-style headers and journals (registry, decisions, trade-offs, blockers, milestones) | registry -> computed; journal -> `DEC-NNN` with `cost`; blockers -> high-severity questions; milestones -> tasks | decisions without `cost` -> debt baseline |
| Principles and check codes | `principle`, `check`; constant bindings; candidates reported (`specengine-eval constants`), no code written (ADR-0016) | old designations -> aliases (ADR-0009) |
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
| The queue grows faster than the owner processes it | medium | nothing is blocked (ADR-0012), sorting by `severity` and number of dependent `@assumes` (not built; `spec health` row 4 lists them); batched rounds; metric "decided differently from the working answer" |
| Agents do not add markers | medium | rules in plugin roles; `unbound` on the panel (`spec health` row 5, shipped); `SubagentStop` reminder; check in `spec verify` |
| The tool eats time from the main projects (scope creep) | high | MVP = CLI + MCP; live UI only after live-task validation; "enforcement first, the bot never first"; multi-user server mode out of plan (ADR-0017) |
| The core silently bends toward one project | medium | pilots of different nature connected from Phase 1 (ADR-0008); specifics only in `specengine.toml` and the importer; core tests on fixtures of all pilots |
| Bevy 0.20 changes the schedule API | medium | tree-sitter detector covered by `fixtures/bevy-mini`; observers behind a Bevy-version gate (05 §5.1); the `schedule_data` dump schema changes across minors: re-run `specengine-eval bevy-detector --dump` per Bevy upgrade |
| The MCP protocol changes again | medium | rmcp + both eras; logic in core, MCP a thin adapter; Claude Code verified on 2.1.283 (MCP README), re-check on upgrade; Streamable HTTP without GET → 405 unmeasured (stdio-only build) |
| rust-analyzer memory near the threshold | medium | whole-group peak 3.76 of 4 GiB (≈ 6 % headroom) on the heavier pilot; re-measure (`specengine-eval ra`) when a pilot grows or `ra_ap` is bumped; fallback: no proc-macro server (≤ 3.2 GiB) |
| HTTP hook is open when the daemon is down | known | `command` hook with exit 2 + managed-deny |
| Specs do not reduce the cost of change (O(n) cross-cutting edits) | fact | do not promise it; show the blast radius via `get_impact` |
