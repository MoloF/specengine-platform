---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 08. Implementation plan, importing existing corpora, acceptance criteria, risks

## 1. SpecEngine repository layout

```
specengine-platform/
  Cargo.toml                       # workspace, edition 2024
  crates/
    specengine-model/              # shared types: IDs, nodes, links, proposals, tasks, events (like code-system-graph `-model`)
    specengine-core/               # md+yaml parser, refs, graph, checks, budget, bundle, proposals, tasks
    specengine-code/               # tree-sitter (rust), own RON lexer, markers, module resolver, AST hash, Bevy detector, lock
    specengine-store/              # SQLite: index + operational state, schema migrations
    specengine-mcp/                # rmcp server: tools, resources, prompts, MRTR/elicitation
    specengine-http/               # axum: REST, SSE, /mcp, embedded UI
    specengine-import/             # importers: one adapter per pilot project's convention
    specengine-ra/                 # layer C: rust-analyzer as a library; outside default-members (ra_ap_* never in the core graph)
    specengine-eval/               # permanent measurement harness over pilot corpora (paths at run time, read-only, JSON aggregates)
    specengine-cli/                # `spec` binary (alias `specengine`)
  ui/                              # Vite + React 19 + TS (Phase 4)
  plugin/                          # Claude Code plugin: plugin.json, .mcp.json, hooks, skills, agents
  fixtures/                        # mini-repositories of pilot projects for integration tests (Bevy-like code + docs)
  docs/                            # this research, later the SpecEngine canon (SpecEngine is managed by SpecEngine)
```

Built crates and `fixtures/`: `CLAUDE.md` "Layout"; the rest is planned.

## 2. Phases

Estimates are rough, for one developer with agents. **MVP = Phase 0-2 on CLI + MCP**, no web UI. That is enough to close the main requirement: reading tasks, sending them back for elaboration, approval, the discrepancy queue.

### Phase 0. Decisions and spikes — done 2026-09-29

- Decisions ADR-0001…ADR-0025 (`docs/decisions/`), each with its canon diff.
- Spikes (`docs/features/phase-0-spikes.md`, shipped): six measurement groups on two pilot corpora, delivered as production increments of `specengine-code`, `-mcp`, `-import`, `-ra` and the permanent harness `specengine-eval`. **12 verdicts: 9 confirmed, 3 refuted.** The hash walk is confirmed after amendment: recipe v1 was refuted under rustfmt, v2 adds four normalisations (05 §5.2). Refuted: `qpath` uniqueness (→ a target discriminator, Phase 1), `tree-sitter-ron` 0.2.0 (→ own RON lexer, 05 §9, 04 §6), the MCP output-cap claim (→ a character-based cap, 04 §4, 07 §1.1). No new ADR: ADR-0020 and ADR-0021 stand.
- Not part of the spikes, carried to the start of Phase 1: the pre-code reading (tracey sources and a tracey run on one pilot crate — input for ADR-0019; limpet `anchor.rs`, sem, cgr docs, fiberplane/drift and amiss, `/speckit.converge`).
- **Hold W** (`docs/canon/documentation-system.md` §1) is a standing rule: each task extracts its slice of 04-08 into `docs/features/<slug>.md` and moves the truth into canon on shipping; an exhausted section of 04-08 is shortened, an exhausted document gets `status: shipped`. Worst W on 2026-09-29 ≈ 118 KB against ≤ 40 KB, driven by 05, 04 and 08 (`cargo xtask docs budget`).

### Phase 1. Reading core — ~2-3 weeks

- Parser — shipped 2026-09-29: `crates/specengine-{model,core}/README.md`; owner questions Q1–Q5 open there.
- SQLite index + FTS5, incremental by BLAKE3 — shipped 2026-09-29: `crates/specengine-store/README.md`; owner questions Q1–Q7 open there.
- `spec check` increments 1 and 2 part 1 — shipped 2026-09-29: `docs/canon/spec-check.md`, `docs/canon/spec-check-graph.md`. Next: 2 part 2 `spec-check-links` (owner Q-D); 3 with the CLI, `xtask` retired; 4 `spec-check-process`.
- CLI: `init`, `index`, `tree`, `show`, `search`, `graph --format mermaid|dot`, `bundle`, `check`.
- MCP stdio: `get_tree`, `get_node`, `search`, `get_context_bundle` + resources.
- **Pilot projects** (ADR-0008): `specengine.toml` and an importer for each, dry-run import, "before / after / hashes" reports; W measured on 10 tasks per project. The order of full migration (§4) is chosen at the end of Phase 1 from the reports.
- **From the Phase 0 spikes**:
  - `qpath` gains a target discriminator: `src/bin`, `examples` and `tests` targets share an empty root module path (16.5–32.2 % of pilot items ambiguous; 05 §5.1 "Module resolver").
  - The marker parser parses the `[tiers]` list of the canon grammar `// @implements ID@rev [tiers]` (`docs/canon/architecture.md#markers`, 05 §5.3); the Phase 0 parser keeps everything after `ID[@rev]` as a free-text note. Duplicate RON paths are flagged as ambiguous, never merged.
  - MCP: test the per-tool `_meta["anthropic/maxResultSizeChars"]` lever against the character-based output cap (04 §4, 07 §1.1).
  - The census findings of §4.3 become per-corpus importer settings and rules.

### Phase 2. Queue and tasks — ~2 weeks  ⟵ value for the owner

- Proposals (all kinds), questions with deduplication, decisions with `cost` and `canon:`, `apply_proposal` (patch by section, optional commit with provenance).
- Tasks: states, brief, `spec_snapshot`, `stale`, `changes_requested`.
- CLI: `inbox`, `review`, `approve/reject`, `task …`, `round new/answer`.
- MCP: `get_task`, `claim_task`, `submit_plan`, `report_discrepancy`, `ask_question`, `propose_change`, `get_proposal`, `report_run`; `review_proposal`/`approve_task` with `requiresUserInteraction` and the consent-tool requirements of 07 §1.2 (the Phase 0 `review_proposal` is a demo skeleton, `crates/specengine-mcp/README.md`).
- Daemon `spec serve` (no UI): HTTP API + SSE; MCP for agents is the `spec mcp` stdio bridge to the daemon (MCP HTTP transport after MVP, 07 §1.1).
- **Plugin**: `.mcp.json`, hooks (`gate` fail-closed, session-start, touched, subagent-stop), prompts, updated roles `requirement-analyst`/`rust-developer`/`spec-writer`/`code-reviewer`/`test-engineer` and the `/feature` command (06 §8).
- Live check: 2-3 real tasks of **each** pilot project go through the full cycle (ADR-0008).

### Phase 3. Code and drift — ~2-3 weeks

- Symbol index (layer A: tree-sitter; layer B: the Bevy `schedule_data` dump; layer C: `ra_ap_ide` per ADR-0020 — B or C is required for generic system instances, 05 §5.1), Bevy detector on resolved types, signatures in the bundle, `find_symbols`, `get_impact`.
- Layer C: monikers for items under attribute proc macros (`#[tokio::main]`-style), which lose theirs when the proc-macro server runs — the scan must map an item through its attribute expansion (`crates/specengine-ra/README.md`).
- Markers `@implements/@verifies/@configures/@assumes` (with revision `@N`), `spec.lock` with `file_blob`, drift cascade (blob OID → normalized text → AST), `spec verify [--tests] [--changed]` with Fix/Check/Pre-existing verdicts and exit codes 0/1/2, `spec bump`, `lock accept [--editorial]`, `refs`, `unmapped`, `get_impact --since`.
- Existing ID citations in code automatically become **weak** `mentions` links. Markers appear in new work and when code is touched; they are not added in bulk (ADR-0016).
- Check codes lifted into constants are the first source of machine bindings `check → const → test`.

### Phase 4. Web UI — ~2-3 weeks

Screens per 07 §3: Tree, Node, Graph, Queue (with diff and in-place editing), Tasks, Health, Round. Live updates over SSE.

### Phase 5. Maturity — ~2 weeks

- Full migration of the remaining pilot projects (the first one migrates in Phase 2-3, order per Phase 1 results).
- A `shared` project (common library) linked from several projects.
- Metrics and compaction, notifications, URL-mode elicitation onto UI cards.
- Optional: queue export to Beads/Task Master, spec digest into `AGENTS.md`/`CLAUDE.md` for external reviewers. No vector search (04 §2.1).

**Total**: MVP in ~5-6 weeks, full scope in ~11-14 weeks.

## 3. Product acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| AC-1 | **Task W**: median on the pilot projects ≤ 40 KB or the project's own target | `bundles` log on 10 real tasks per pilot project |
| AC-2 | **AST isolation**: `cargo fmt`, edits to comments and neighbouring functions do not change a symbol's hash | property test over all symbols of the pilot projects. **Phase 0 baseline met for fmt and comments**: 100 % on both pilots (`specengine-eval ast-hash`, 05 §5.2); the property test is Phase 3 |
| AC-3 | **Single door**: no agent-facing MCP tool modifies spec files | call every tool on a temp repo + `git status` is empty |
| AC-4 | **Task approval**: without an approved task an agent cannot write a file in `zones.code` (`selective` mode); same with the daemon stopped; in `observe` mode the write passes and a finding is recorded | hook integration test |
| AC-4b | **Nothing is blocked by a discrepancy** (ADR-0012): an open proposal does not change the task status, several proposals can be open on one node, applying the second one rebases | test |
| AC-5 | **No repeated questions**: `ask_question` on an already answered question returns the decision instead of opening a new one | test: a repeated call yields `decision`, the question count does not grow |
| AC-6 | **Lossless import**: all records of the corpus are imported, verbatim-text hashes match; **the project's full test suite is green** after migration | importer report + the project's `cargo test` |
| AC-7 | **Bundle determinism**: one state → one `bundle_hash` | test |
| AC-8 | **Machine-verified**: `verified` is set only after SpecEngine itself runs the `@verifies` tests | test |
| AC-9 | **Staleness**: editing a node from the `spec_snapshot` of a `ready` task makes it `stale`, the bundle shows the diff | test |
| AC-10 | **Performance**: full symbol hash ≤ 2 s per 300 kLOC, full index ≤ 10 s for hundreds of md files; per-file increment ≤ 200 ms. The `ra_ap_ide` layer is measured separately | benchmark on the pilot projects. **`ra_ap_ide` layer measured in Phase 0** (`specengine-eval ra`, 05 §5.1): cold 33–65 s, warm pass ≈ 0.2 s, group peak ≤ 3.76 GiB. Index: `specengine-eval index`, pilot run pending |
| AC-11 | **Homoglyphs**: IDs with mixed scripts are rejected with an auto-fix | test on IDs where a Latin letter is swapped for its Cyrillic look-alike (U+0420 for `P`, U+0415 for `E`). **Rejection met** (`spec check`, `check_ids.rs`); applying the fix: Phase 2 (`apply_proposal`) |
| AC-12 | **Responsiveness**: an MCP event is visible in the UI ≤ 1 s | e2e |
| AC-13 | **Meaning change without revision fails**: pre-commit fails if a node's `norm_hash` changed without a `rev` bump and without `--editorial` | test on a temp repo |
| AC-14 | **"Could not verify" ≠ "fresh"**: an unparseable file yields `cannot_verify` and exit 2, not a green run; two different unparseable items do **not** get the same hash | test (regression for the `is_extra()` trap on ERROR). **Hash half met in Phase 0** (`specengine-code` `tests/hash_traps.rs`); exit 2 of `spec verify` is Phase 3 |
| AC-15 | **Rename is not drift, twin is not a trap**: a renamed function with a non-trivial body is tracked (`MOVED`), a trivial one (< 124 B buffer) yields `LOST`/`STALE` rather than false tracking; two copies with the same skeleton yield `AMBIGUOUS` | test |

## 4. Importing existing corpora

One core for all projects (ADR-0008); corpus specifics live only in `specengine.toml` and the importer. The importer is an adapter for the project's convention: documents with front-matter, records as table rows, table-style headers and journals — it translates all of them into SpecEngine nodes, records and links.

### 4.1. What the importer carries over

| Source in the corpus | → | Notes |
|---|---|---|
| Documents with front-matter (requirements, decisions, features) | nodes and records, header normalization | `canon:` on decisions is kept; feature criteria → `{#AC-..}` sections; locally unique IDs get a scope: `<slug>/AC-07` |
| Records as table rows (assumptions, questions, terms), including rows without an ID | one record per row, `immutable_text`; missing IDs are issued (agent proposes, owner confirms) | status prefixes in cells ("closed …", "implemented in code …") → status + events; text verbatim |
| Table-style headers and journals (registry, decisions, trade-offs, blockers, milestones) | registry → computed; journal → `DEC-NNN` with `cost`; blockers → blocking questions; milestones → tasks | decisions without `cost` → debt baseline |
| Principles and check codes | `principle`, `check`; code constants → markers | old designations → aliases (ADR-0009) |
| README files of code subtrees (Tier 1) | domain nodes | stay in place (locality), indexed as `domain` |
| Roadmap state and queue | SpecEngine tasks | the markdown becomes generated |
| An existing tree of mechanics or domains | `mechanic` + rings → domains | if there is no tree, it is built **through the queue itself**: the analyst proposes a clustering, the owner approves (SpecEngine's first test on its own cycle) |
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

A census config alone described each pilot corpus (ADR-0008 confirmed; schema and counts: `crates/specengine-import/README.md`); it took three capabilities beyond the first sample — headerless `|` blocks, an ID-column header regex for record tables, wiki links. Each finding below is a per-corpus importer setting or rule, not core code:

- (a) **Link base**: 57 of one pilot's 58 "broken" links point to existing files, written relative to the docs root rather than to the linking file → a per-corpus link-base setting.
- (b) **Genuine debt**: the other pilot's broken wiki links target 8 names that exist nowhere → the debt baseline (§4.2 item 3).
- (c) **Legacy prefixes**: both pilots keep non-Latin prefixes (20 / 287 IDs; one pilot's whole decision register) → ADR-0009 aliases at import (§4.2 item 4).
- (d) **Definition vs reference**: IDs recur across index and reference tables (1 407 / 110 duplicates) → a per-corpus rule telling a record's definition from a reference to it.
- (e) **Locally numbered tables**: a numero-sign (U+2116) column accounts for most of one pilot's 185 rows without an ID → a per-corpus setting naming local-number columns (candidates for the scoped IDs of §4.1).

## 5. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Migration breaks sentinel tests and verbatim corpus texts | high | harness adapter before the move; text hashes in the report; AC-6 |
| The queue grows faster than the owner processes it | medium | nothing is blocked (ADR-0012), sorting by `severity` and number of dependent `@assumes`; batched rounds; metric "decided differently from the working answer" |
| Agents do not add markers | medium | rules in plugin roles; `unbound` on the panel; `SubagentStop` reminder; check in `spec verify` |
| The tool eats time from the main projects (scope creep) | high | MVP = CLI + MCP; UI only after validation on live tasks; "enforcement first, the bot never first"; multi-user server mode out of plan (ADR-0017) |
| The core silently bends toward one project | medium | pilot projects of different nature are connected from Phase 1 (ADR-0008); specifics only in `specengine.toml` and the importer; core tests on fixtures of all pilots |
| Outdated RON grammar | resolved | Phase 0: `tree-sitter-ron` 0.2.0 replaced by the own RON lexer of `specengine-code` (05 §9) |
| Bevy 0.20 changes the schedule API | medium | tree-sitter detector covered by `fixtures/bevy-mini`; observers behind a Bevy-version gate (05 §5.1); the `schedule_data` dump schema is unstable across minors — re-run `specengine-eval bevy-detector --dump` on every Bevy upgrade |
| The MCP protocol changes again | medium | rmcp + both eras; logic in core, MCP is a thin adapter; Claude Code behaviour verified on 2.1.283 (04 §4), re-run the checks on upgrade; Streamable HTTP without GET → 405 not measured (stdio-only build) |
| rust-analyzer memory near the threshold | medium | whole-group peak 3.76 GiB against 4 GiB (≈ 6 % headroom) on the heavier pilot; re-measure with `specengine-eval ra` when a pilot grows or `ra_ap` is bumped; fallback: load without the proc-macro server (≤ 3.2 GiB) |
| HTTP hook is open when the daemon is down | known | `command` hook with exit 2 + managed-deny |
| Specs do not reduce the cost of change (O(n) cross-cutting edits) | fact | do not promise it; show the blast radius via `get_impact` |
