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

The open work, its order and the time left (open items' estimates summed, the pool excluded; rough, for one developer with agents): the generated view `docs/generated/roadmap.md` or `spec roadmap`, over the plan items in `docs/roadmap/` (`docs/canon/spec-roadmap.md`). Releases, in `[roadmap]`'s order: `mvp` the rest of Phases 0-2, `v1` Phases 3, 4 and 6, `v2` Phase 5, `v3` Phase 7 through the hosted hub, `v4` the trade workspaces, the writer studio and the board's hand-off, `v5` a Jira-like UI over the pipeline (Phase 7, "Release scope", "v4 scope", "v5 scope"); v4's unstarted tail is in the pool. **MVP = Phase 0-2 on CLI + MCP**, no web UI. Enough for the main requirement: reading tasks, sending them back for elaboration, approval, the discrepancy queue.

### Phase 0. Decisions and spikes — done 2026-09-29

- ADR-0001…ADR-0025 with canon diffs; spikes: `docs/features/phase-0-spikes.md`.
- Carried to Phase 1: pre-code reading (tracey sources and a run on one pilot crate, input for ADR-0019; limpet `anchor.rs`, sem, cgr docs, fiberplane/drift, amiss, `/speckit.converge`).
- **Hold W** (`docs/canon/documentation-system.md` §1) is a standing rule: each task extracts its slice of 04-08 into `docs/features/<slug>.md` and moves the truth into canon on shipping; an exhausted section of 04-08 is shortened, an exhausted document gets `status: shipped`. Worst W (`spec check`'s summary): ~108 KB against <= 40 KB, driven by 05 and 04.

### Phase 1. Reading core — done 2026-10-05

- Shipped (open questions: crate READMEs, `docs/canon/*`): the parser, the SQLite/FTS5 index, CLI passes 1, 3 and 4 (`bundle`), check increments 1-4 (`check` to `--changed`, the gate, process rules ADR-0031), index compaction, shards (ADR-0028, ADR-0030), MCP stdio reads, layer A identity, pilot schemes (§3 AC-10), import records, gaps and layout (§3 AC-6, §4.3), token calibration, pilot W (§3 AC-1, `docs/canon/w-measurement.md`).
- **Pilot projects** (ADR-0008): `specengine.toml` and an importer for each, dry-run import, "before / after / hashes" reports; W on every task document.
- **Migration order** (owner, 2026-10-05; no ADR, `docs/features/pilot-w.md` AC-12): pilot A first, then B, after an ADR on hyphenless codes superseding ADR-0009 (decision table there).

### Phase 2. Queue and tasks — in progress since 2026-10-05  ⟵ value for the owner

- Slices: `proposal-apply` (1: `apply_proposal` for `update`), `queue-export` (2: backup, restore), `agent-intake` (3: MCP intake), `queue-path-targets` (4: path targets), `plugin-skills` (`.mcp.json`, skills; owner's check open) shipped 2026-10-05; `decision-apply` (5: answers as decision records), `daemon-read` (`specengine-http`) shipped 2026-10-06; `proposal-kinds` (6: `create`), `task-package` (7, ADR-0027) shipped 2026-10-07 (UI: Phase 4); `decision-staging` (ADR-0035), `plugin-gate`, `round` (ADR-0038), `ui-round` (Phase 4), `plugin-roles`, `spec-serve` 2026-10-08; `kind-{interpretation,decision,amendment}`, `health-left` (`docs/canon/health.md`) 2026-10-09.
- MCP lever (Phase 0 spike): `_meta["anthropic/maxResultSizeChars"]` (500 000) declared; open: does it act, the maxima (`docs/canon/mcp-read.md` "Owner's check": 48-60 k on 2.1.288).
- MCP: `get_task`, `claim_task`, `submit_plan`, `report_run`, `complete_task` shipped; no tool takes a choice from an agent or decides (ADR-0035, ADR-0054); the owner's staging forms: Phase 5.
- Daemon `spec serve` shipped (the UI inside since Phase 4 `ui-embed`): `specengine-http` per data directory; `spec mcp` on a trigger (05 §1 (6)); no gate needs it (ADR-0039).
- **Plugin** (06 §8; `docs/canon/plugin.md`): the gate hook, skills, stack-neutral roles, `/specengine:feature` shipped; other hooks (07 §4) later; stack roles from a profile plugin or the project (ADR-0027).
- Live check: 2-3 real tasks of **each** pilot project go through the full cycle (ADR-0008); stack neutrality done 2026-10-08: `fixtures/spec-c` (07 s1.2 P2-5).

### Phase 3. Code and drift

- Shipped (specs `docs/features/<slug>.md`, canon `docs/canon/`): 2026-10-10 `code-refs`, `norm-hash` (ADR-0040); 10-12 `spec-lock` (ADR-0041); 10-13 `spec-verify` (4a, ADR-0042); 10-14 `rev-discipline` (4b-1, ADR-0043); 10-15 `rev-rule` (4b-2), `drift-queue` (4c-1, ADR-0045), `drift-follow-up` (4c-2); 10-16 `hash-levels` (5a-1), `hash-levels-ron` (5a-1b), `hash-levels-eval` (5a-2), `symbol-discriminator` (6a-1), `symbol-index` (6a-2), `symbol-reads` (6a-3), `code-tools` (6b-1, plugin 0.9.0), `verify-tests` (8a-1, ADR-0044), `bevy-roles` (10a-1), `bevy-roles-reads` (10a-2, plugin 0.10.0); 10-19 `health-code` (9a-1, ADR-0047, ADR-0051), `health-code-ui` (9a-2), `health-ui-follow-up` (9a-3), `citation-sweep`, `layer-c` (11a-1, ADR-0049), `layer-c-resolve` (11a-2), `constant-census` (12a); 10-20 `hash-levels-use-lists` (5a-1c), `lock-levels` (5b-1a, ADR-0050), `lock-rebase` (5b-2), `lock-levels-verify` (5b-1b), `anchor-states` (5c), `verify-task` (8a-2); 10-21 `verify-node` (6b-2a), `check-binding` (6b-2b), `show-bindings` (7a-1), `bundle-bindings` (7a-2), `impact` (7b-1, plugin 0.15.0), `impact-since` (7b-2, plugin 0.16.0), `tested-evidence` (8b-1, plugin 0.17.0), `tested-reads` (8b-2, plugin 0.18.0), `health-drift` (9b-1), `health-drift-ui` (9b-2), `hygiene-pass` (RM-0029 from the pool: the review nits of 5b-1b, 5c, 8a-2).
- The owner's pilot runs: TE-1 (5a-2) passed 2026-10-20, rerun after 5a-1c; TE-6 (11a-2) passed 2026-10-22 (`docs/features/layer-c-resolve.md`); open: TE-3 (6b-1), TE-4 (6a-3, 12a), TE-7 (9a-1, 10a-1, 9b-1), TE-8 (10a-2), TE-9 (8a-1), TE-5 (6b-2a, 6b-2b), TE-6 (7a-1), TE-5 (7a-2).
- 5b-1a froze the bytes TE-1 measured: an edit of `specengine-code`'s `levels.rs`, `hash.rs`, `ron/` or `Cargo.lock`'s tree-sitter entries that moves a digest is a recipe bump and a rebase of every lock (ADR-0050); 5a-1c was the last change before it.
- Symbol index (layers A tree-sitter, B the Bevy `schedule_data` dump, C `ra_ap_ide`, ADR-0020; generic instances need B or C, 05 §5.1), Bevy detector on resolved types, signatures in the bundle.
- Layer C: `spec resolve` stores rust-analyzer's monikers as an advisory snapshot (shipped 2026-10-19: `docs/canon/layer-c.md`); `spec symbols` and `find_symbols` show it (11a-3, shipped 2026-10-22: `docs/canon/layer-c-reads.md`). 11b `layer-c-macros` (RM-0022) not triggered: with the proc-macro server on no item gains a moniker (A loses 1, 0.01 %; B 19, 0.25 %; cold 3.9 and 6.3 times), under its 1 % trigger; re-measured on a new pilot (`crates/specengine-ra/README.md`).
- Markers `@implements/@verifies/@configures/@assumes` (revision `@N`), `spec.lock`, `lock accept [--editorial]`, `lock regen`, `lock rebase` (shipped: `docs/canon/spec-lock.md`), drift cascade, `spec verify [--changed | --staged]` (shipped: `docs/canon/spec-verify.md`), `spec verify --tests` (shipped: `docs/canon/spec-verify-tests.md`), `spec verify --task` (shipped: `docs/canon/spec-verify-scope.md`), `spec propose drift` (shipped: `docs/canon/proposal-drift.md`), `spec reject --follow-up` (shipped: `docs/canon/proposal-completion.md` "Reject"), `spec bump` (shipped: `docs/canon/node-revision.md` "Raising"), per-level hashes and fingerprints (shipped: `docs/canon/code-hash.md`, figures `docs/canon/code-hash-eval.md`; recorded by lock format 2: `docs/canon/spec-lock.md`; judged by verify: `docs/canon/spec-verify.md` "Cascade"), `refs` (shipped: `docs/canon/code-bindings.md`), `unmapped`, `spec impact` and `get_impact` for a node, a symbol or the code since a revision (shipped: `docs/canon/spec-impact.md`).
- ID citations in code become **weak** `mentions` (shipped: `spec refs` lists them); markers only in new or touched code (ADR-0016).
- Machine bindings `check → const → test` (slice 12): free string constants whose whole value is a record's code are reported as candidates (12a), bind a node without a marker only under a decision amending ADR-0020 (12b), and what reaches them is listed, never evidence (12c); nothing writes code (ADR-0016).

### Phase 4. Web UI

Screens and SSE per 07 s3, begun on mocks (ADR-0033, `ui/README.md`): `ui-shell` shipped 2026-10-05, `ui-tree-node`, `ui-graph`, `ui-tasks`, `ui-home` and `ui-markdown` (ADR-0036) 2026-10-06, `ui-health`, `ui-live`, `ui-live-tasks` 2026-10-07, `ui-round` 2026-10-08, `ui-embed` (the UI inside `spec serve`) 2026-10-10. The drift screens (RM-0011), split 2026-10-21 into `ui-node-bindings`, `ui-drift-evidence` and `ui-tree-drift`, UI only, all shipped 2026-10-21 (`docs/canon/drift-screens.md`); the lock diff inspector moved to RM-0033 (ADR-0053): `lock-diff` (`spec lock diff`, the daemon's `lock-diff`, the UI's types) shipped 2026-10-21 (`docs/canon/spec-lock-diff.md`), then the Bindings tab's panel `lock-diff-ui` (`docs/canon/lock-diff-panel.md`), UI only, shipped 2026-10-21: RM-0033 done.

### Phase 5. Maturity

- Full migration of the pilot projects (order: Phase 1). A was migrated 2026-10-07 through a pilot-side wrapper (§3 AC-6); RM-0013 has SpecEngine replay it alone from config, nothing written into the pilot: `import-homes` (shipped 2026-10-22: `[definitions] home`, `[layout] inherit`), `import-lead-ins` (shipped 2026-10-30: wrapped lead-ins, a `not_taken` count, the emitted scheme keeping the before walk); the owner's replay stays open; check parity: RM-0039 (before 2026-12-31), RM-0040. Then B (RM-0014).
- A `shared` project (common library) linked from several projects.
- The owner's staging forms over MCP, form mode (ADR-0054): `review_proposal` (`review-form`, RM-0012), then `approve_task` (`task-stage`, RM-0038, its own ADR).
- Metrics and compaction, URL-mode elicitation onto UI cards (staging only, ADR-0035); notifications moved to Phase 7 (RM-0017).
- Optional: queue export to Beads/Task Master, spec digest into `AGENTS.md`/`CLAUDE.md` for external reviewers. No vector search (04 §2.1).

### Phase 6. Planning: roadmap and pool (moved before the rest of Phase 3 by the owner's decision, 2026-10-20)

Releases, priority, a `depends_on` chain, range estimates, a computed position, the pool; plan items are files in git (ADR-0052). Slices decided 2026-10-20 under the owner's pre-approval, review open, each scoped in its spec. Shipped 2026-10-20 (`docs/canon/spec-roadmap.md`): `roadmap-items` (1a; the open work seeded once as items in `docs/roadmap/`, read by `spec roadmap`), `roadmap-view` (1b; the generated view replacing §2's slice lists, its "~N weeks" and "Total"), `roadmap-position` (2a; this repository's open tasks on `spec roadmap`, read-only) and `roadmap-surfaces` (2b; `spec task show --next` by the plan, the package's `roadmap`, MCP `get_roadmap`, the daemon's uncut route, plugin 0.11.0). Shipped 2026-10-21: `roadmap-screen` (3a; the Roadmap screen, read-only: lanes, the chain canvas; `docs/canon/spec-roadmap-screen.md`), completing Phase 6's roadmap feature; 3b `roadmap-moves` (a move staged from the screen) was parked on N12 (`roadmap-items.md`), unparked 2026-11-03 by ADR-0062, no spec file. 6b-2 (RM-0005), split 2026-10-21 into 6b-2a `verify-node` (the CLI) and 6b-2b `check-binding` (MCP, plugin 0.12.0), shipped 2026-10-21. 7a (RM-0006), split 2026-10-21, both shipped 2026-10-21 (`docs/canon/code-reads.md`): 7a-1 `show-bindings` (`spec show --bindings`, `get_node` `with: ["bindings"]`, `delta.tests_reaching`, plugin 0.13.0), 7a-2 `bundle-bindings` (layers 5 and 9, the package's `bindings`, the Bundle tab's rows, plugin 0.14.0); tree counters deferred to RM-0034. 7b (RM-0007), split 2026-10-21, both shipped 2026-10-21 (`docs/canon/spec-impact.md`): 7b-1 `impact` (`spec impact REF | --symbol`, MCP `get_impact`, plugin 0.15.0), 7b-2 `impact-since` (`--since REV`, `verify --changed --json`'s `delta.symbols`, plugin 0.16.0); 8b, split 2026-10-21: 8b-1 `tested-evidence` (RM-0008), shipped 2026-10-21 (`docs/canon/spec-lock-tested.md`: format 3, the run's evidence commit, plain verify's and `check_binding`'s `tested`, plugin 0.17.0) and 8b-2 `tested-reads` (RM-0032), shipped 2026-10-21 (the reads' `tested`, `spec-lock-tested.md` "Reads", plugin 0.18.0). 9b (RM-0009), split 2026-10-21: 9b-1 `health-drift` (`docs/canon/health-drift.md`: `spec health`'s row 6, plain verify's judge in-process over the call's code index; the route, the daemon keys, the UI's types) and 9b-2 `health-drift-ui` (Health draws row 6), both shipped 2026-10-21, closing the 6b-2 to 9b chain; RM-0011, the drift screens, followed (Phase 4), then RM-0033's `lock-diff` and `lock-diff-ui`, both shipped 2026-10-21: RM-0033 done; then RM-0010 `layer-c-reads` (11a-3), shipped 2026-10-22 after the owner's TE-6. The draft `docs/features/roadmap.md` is abandoned as split.

### Phase 7. Full cycle: flow, board, trace, then a hub (planned 2026-10-30; first result closed 2026-10-31: steps 0-7 and 7a-7d shipped, the gate, step 7e, passed; step 8 `hub-identity` and step 9a `hub-team` shipped 2026-11-01, step 9b `hub-confirm`, step 10a `apply-outputs`, step 10b `hub-git`, step 11 `hub-mcp`, step 11b-1 `hub-deploy`, step 11c-1 `hub-accounts` and step 11c-2 `stream-life` 2026-11-03; step 12 `scale-reads` split in five 2026-11-03, 12a `flow-linear`, 12b `scale-eval`, 12c `edge-table`, 12d `store-reads` and 12e `read-trust` shipped 2026-11-04, closing step 12; release `v3` scoped to the hosted hub 2026-11-04, its code items RM-0073 `spec-ids` and 11b-2 `hub-ops` shipped 2026-11-04: next the owner's gate 11b-3, `v0.29.1` tagged 2026-11-06; 10c `board-in-git`, step 13 `zone-index` onward and RM-0070 deferred to `v4`; `v4` re-scoped 2026-11-08: the trade workspaces and the writer studio first, RM-0080 to RM-0090, then steps 13-21; its step 0 `local-landing`, 1a `workspaces`, 1b `ui-languages`, 2a `flow-scope`, 2b `writer-studio` and 2d `ui-languages-sweep` shipped 2026-11-09, 3a `board-handoff` 2026-11-09 and 3b `card-tasks` 2026-11-10, closing `v4`; `v5` scoped 2026-11-10, a Jira-like UI over the pipeline, RM-0093 to RM-0100, v4's unstarted tail to the pool; its step 1 `design-foundation` shipped 2026-11-10)

**Goal.** One tool for the whole development cycle of a game or any project. Writers, with an AI over MCP, build a flow diagram with branches, loops, reuse and collapsible groups. One plain kanban board carries the work of every trade. Cards and brief sections link both ways; accepted cards confirm the brief and fill a timeline; a changed brief marks its cards and records the consequences. Code binding is phase two and stays as it is: markers, `spec.lock`, verify, the gate, terminal approvals, local `T-` tasks and layers A-C.

**First result, closed 2026-10-31: steps 0-7e, 34.5-52 pipeline hours, local, no server.** The owner, alone on the local daemon, has his AI write a real flow from an empty project, sees it collapse, cuts cards and moves them; accepted cards confirm their nodes, a changed node marks its cards, and the timeline fills. Trace (`card-trace`, `trace-views`) belongs to this result. Steps 0-7 (27-43 hours) shipped 2026-10-31; a trial the same day showed the cycle works and costs too much, so the closing group, steps 7a-7e, 7.5-9 hours, stood before the hub. `trace-truth` (7a), `proposal-rehearsal` (7b), `flow-authoring` (7c, RM-0068) and `screen-fit` (7d) shipped 2026-10-31; the gate, step 7e, the re-run of the trial, passed the same day and closed the result (its record below). Step 8, `hub-identity`, shipped 2026-11-01 (ADR-0058, ADR-0059, `docs/canon/hub.md`), steps 9a to 11c-2 by 2026-11-03, step 12's five slices 2026-11-04, 10c deferred; the hub waits for RM-0073, 11b-2 and the gate ("Release scope" below); RM-0070 `trial-followups` keeps what the trials left and blocks nothing. The team result, steps 0-11c with `board-in-git`, `hub-deploy`, `hub-ops`, `hub-accounts` and `stream-life`, is 64-95 hours; everything is 96-147 hours. Hours are pipeline hours at the measured pace (15 items, 33-54 "days", 37 hours), with a 1.25-2x margin for undecided design. Items keep them in their body, never in `estimate:`, whose unit is human-paced days, so the view counts them unestimated.

**This reverses the single-user premise, at step 8.** The owner cancelled it on 2026-11-01: ADR-0058 `hub-mode`, issued by step 8, replaces the decision "One user, a local daemon; server and multi-user mode are out of plan". Nothing before step 8 depends on that reversal: steps 0-7e run on the local daemon, which stays as it is, and the work can stop after the gate with a whole local tool.

| Step | Slice | Item | Result | ADR issued | Hours | After |
|---|---|---|---|---|---|---|
| 0 | `full-cycle-decisions` | RM-0041 | Owner's round, two ADRs, this plan | ADR-0055 `flow-graph`, ADR-0056 `cards` | 3-5 | - |
| 1 | `flow-links` | RM-0042 | `follows`, `uses`, `supplies`, labelled edges, impact, bundle | - | 2-4 | 0 |
| 2 | `flow-read` | RM-0043 | `spec flow`, `get_flow`, lifted edges, gap rows | - | 3-5 | 1 |
| 3 | `flow-canvas` | RM-0044 | Collapsing groups, gap marks, text twin | - | 4-6 | 2 |
| 4 | `card-store` | RM-0045 | Cards, flows, events, CLI and MCP tools | - | 4-6 | 0 |
| 5 | `board-ui` | RM-0046 | Board, filters, card panel, write routes | ADR-0057 `card-accept-page` | 4-6 | 4 |
| 6 | `card-trace` | RM-0047 | Targets, suspect, clearing, re-estimate, corrections, rollup | - | 4-6 | 4 |
| 7 | `trace-views` | RM-0048 | Flow badges, `stage:`, timeline lanes, a node's cards | - | 3-5 | 3, 5, 6 |
| 7a | `trace-truth` | RM-0066 | A nested edit marks, counts stop at containers, `get_card` shows its mark | - | 2-2.5 | 7 |
| 7b | `proposal-rehearsal` | RM-0067 | `dry_run`, no false `ref-dangling`, `spec approve --open` | - | 1.5-2 | - |
| 7c | `flow-authoring` | RM-0068 | `write-flow` skill, remedies in gap rows, errors that teach, `spec init` comments | - | 2-2.5 | 7b |
| 7d | `screen-fit` | RM-0069 | A drop lands on an empty column, a clean flow takes the width | - | 1 | 7 |
| 7e | gate: the trial again | - | Done 2026-10-31: the re-run below, no code; closed the first result | - | 1 | 7a-7d |
| 8 | `hub-identity` | RM-0049 | Public origin, identity header, the login as the actor of five card writes; nothing that confirms | ADR-0058 `hub-mode`, ADR-0059 `hub-identity` | 3.5-4.5 | 0 |
| 9a | `hub-team` | RM-0071 | `[team]`, the fence's classes; accept, clear, stages and round answers under a login; no spec file changes | ADR-0060 `hub-team` | 2.5-3.5 | 8 |
| 9b | `hub-confirm` | RM-0050 | One click confirms one stage, a commit in the hub's checkout, nothing pushed; `Decided-by`, a person in `Author`; `door.rs` extended; plugin 0.29.0 | ADR-0061 `hub-confirm` | 3-4 | 9a |
| 10a | `apply-outputs` | RM-0072 | Shipped 2026-11-03: the index in every apply commit, rendered from the committed tree, terminal and hub, never refusing; a hub mints `PR-` from a stored floor, `PR-100001`; no network | ADR-0062 `apply-outputs`, ADR-0063 `hub-ids` | 2-3 | 9b |
| 10b | `hub-git` | RM-0051 | Shipped 2026-11-03: a `--push` hub fetches, merges never rebasing, pushes, apart from any confirm; the sole writer a convention; credentials git's; push status and "N not pushed" in the UI; `spec sparse-set` | ADR-0064 `hub-push` | 3-4.5 | 10a |
| 11 | `hub-mcp` | RM-0053 | Shipped 2026-11-03: `/mcp/<slug>` on a hub, both eras, in the project's turn; sessions bound to a login and capped; the login the author and the card's actor; 17 tools that decide nothing; `Authorization` refused elsewhere | ADR-0065 `hub-mcp` | 6-9 | 8 |
| 11b-1 | `hub-deploy` | RM-0064 | Shipped 2026-11-03: `scripts/deploy/`, the image workflow: Docker compose, Traefik TLS, Authelia with the step-up, a per-person MCP secret, the entrypoint, moving a project, `Author.login` checked on decode; no network in its tests | ADR-0066 `hub-image`, ADR-0067 `hub-door` | 3-4 | 8, 9b, 10b, 11 |
| 11c-1 | `hub-accounts` | RM-0065 | Shipped 2026-11-03: the deployment's `hub-admin` in a one-shot `admin` service with no network: add, disable, enable, reset, MCP secrets, list, check, an act log; the door re-reads a person on every request; `spec team`; no account code in a crate | ADR-0068 `hub-accounts`, the owner's confirmation open | 2.5-3.5 | 8, 11b-1 |
| 11c-2 | `stream-life` | RM-0075 | Shipped 2026-11-03: a hub's events tail and `/mcp` GET streams end 300 s after they open, between two events, their sessions kept; the client reconnects through the door with `Last-Event-ID`, losing nothing; a local daemon's never end; no UI change | ADR-0068 (no new ADR) | 1-1.5 | 11, 11c-1 |
| 12a | `flow-linear` | RM-0076 | Shipped 2026-11-04: a flow read linear in `parent:` depth, one parent map and one fold per read; bundle counts, running sums of characters and thousandths that make the concatenation's decision; byte-equal output, no store change; the `--trace` counts left to RM-0070 | - | 1-1.5 | - |
| 12b | `scale-eval` | RM-0077 | Shipped 2026-11-04: `specengine-eval scale`, a seeded domain-free corpus, warm reads timed as MCP calls them, each step replayed alone, W_after; at 100k the reads take 0.4-1.6 s, linear, reparse, hash and decode dominating (`docs/features/scale-eval.md` "Baseline") | - | 1.5-2 | - |
| 12c | `edge-table` | RM-0078 | Shipped 2026-11-04: core's resolved edges in four index tables, `INDEX_FORMAT` 16, kept by a full or an incremental pass over recorded lookups, equal to a rebuild and to core's resolution; `parent:` qualifiers from the parse, batched probes; reads unchanged (`docs/canon/spec-index-edges.md`) | - | 2-3 | 12b |
| 12d | `store-reads` | RM-0079 | Shipped 2026-11-04: `show`, `--links` and `bundle` over a subset from one index snapshot, every file used checked against its row's BLAKE3, else one targeted update and retry, then the whole read; whole-corpus reads reuse a matching row's parse; at 100k links and bundle 0.37 s, about the freshness step (`docs/canon/spec-read-subset.md`) | - | 1.5-2.5 | 12c |
| 12e | `read-trust` | RM-0054 | Shipped 2026-11-04: a read reads a file only when its stat key moved or is racy, a no-op update takes no write lock, `INDEX_FORMAT` 17; on a hub only reads trust the index its confirms and merges keep, walking after start, a doubt or 5 minutes; fan-out under 1 s at 100k (`docs/canon/spec-read-freshness.md`) | ADR-0069 `read-freshness` | 2-3 | 12d |
| - | `spec-ids` | RM-0073 | Shipped 2026-11-04: a hub mints record and created node IDs from 100001, a terminal below it, each counting its own side's; propose refuses the other side, apply never (ADR-0012); `id-width` passes a hub's number; no format changes; plugin 0.29.1 (`docs/canon/spec-ids.md`) | ADR-0070 `spec-ids` | - | 10b, 11, 12e |
| 11b-2 | `hub-ops` | RM-0074 | Shipped 2026-11-04: every 15 minutes two sidecars with no network stage the queues and the door's own exports on tmpfs, and restic sends them encrypted to the operator's backend (`secrets/restic_env`); runbooks for restore, upgrade and `--push`; `specengine-health`, the fence's loopback 403 and port 443; no crate change (`docs/canon/hub-ops.md`) | ADR-0071 `hub-ops` | 1-1.5 | 11b-1, RM-0073 |
| 11b-3 | gate: the hub on a VPS | - | The owner's session, no code: `v0.29.1`, tagged at 979b8f4, its image published 2026-11-06; VF4-VF9 and HO1-HO17 of `hub-ops`, H1-H17 of `hub-deploy`, G1-G10 of `hub-accounts` and the claude.ai client check; then `--push` and real people | - | - | RM-0073, 11b-1, 11b-2 |
| 10c | `board-in-git` | RM-0052 | Pool since 2026-11-10. The board's snapshot on a dedicated branch, checked by CI | `board-snapshot` | 2-4 | 10b |
| 13 | `zone-index` | RM-0055 | Pool since 2026-11-10. Index-free zone, `status: retired` | `zone-index` | 2-3 | 12e |
| 14 | `change-record` | RM-0056 | Pool since 2026-11-10. `CR-`, baseline tag, RM-0035, RM-0036 | `changeset` | 4-6 | 6, 10b |
| 15 | `draft-overlay` | RM-0057 | Pool since 2026-11-10. Revisable drafts in reads | - | 3-5 | 11 |
| 16 | `reuse-overrides` | RM-0058 | Pool since 2026-11-10. Overrides, variants, usage reads | - | 3-5 | 2 |
| 17 | `intake-similar` | RM-0059 | `v5` step 8. `similar`, `distinct_from` on create | - | 2-4 | 12e |
| 18 | `collision-pass` | RM-0060 | `v5` step 9. Candidates, pair ledger, skill | - | 3-5 | 17 |
| 19 | `pack` | RM-0061 | `v5` step 11. `spec init --pack`, config proposal | - | 3-5 | 4 |
| 20 | `writer-skills` | RM-0062 | Pool since 2026-11-10. Profile plugin or MCP prompts | - | 3-5 | 11 |
| 21 | `notify` | RM-0017 | Pool since 2026-11-10. Notifications | - | 1-2 | 8 |

**Release scope, 2026-11-04.** The owner's instruction: do only what the hub's release to a server needs, and defer everything after it to the next release. `v3` ends with the hosted hub: RM-0073 `spec-ids`, so a hub and a terminal never mint the same ID (shipped 2026-11-04), then 11b-2 `hub-ops` (shipped 2026-11-04), then the owner's gate 11b-3 on the VPS, then `--push` and real people (RM-0074's last bullet). The order is the owner's instruction; the scope is the orchestrator's reading of it, the owner's review open. A release `v4` was added to `[roadmap] releases` and takes, as `release: v4`, the rows marked `v4` above: 10c `board-in-git` (RM-0052), step 13 `zone-index` (RM-0055: its draft spec `docs/features/zone-index.md` stays committed, a partial implementation exists outside the tree), steps 14-21 (RM-0056 to RM-0062, RM-0017) with RM-0035 and RM-0036, which ship inside step 14; also RM-0070 `trial-followups` and RM-0063's load flakes. The other pool items keep no release; `mvp` (RM-0001) and `v2` are untouched, so `spec roadmap` still reads `current mvp`. Since 2026-11-10 those rows read the pool or `v5` ("v5 scope").

**v4 scope, 2026-11-08.** The owner's requirement: a full development tool for game writers and every other trade, simpler and clearer, each trade with the pages it needs; modules, each flow and the whole timeline, gaps filled, questions without walls of IDs and paths; Flow in three panes on one screen; a board with static columns and columns per discipline; roadmap states; an Overview that says something. His decisions: (1) four roles to start with: writer, developer, artist or level designer, and lead (the owner); (2) the UI in Russian and English, switchable: ADR-0072 `ui-languages`, issued with 1b's spec on 2026-11-09, replaces the English-only interface and amends decision 10 below; (3) Tasks leaves the nav, and an agent task shows inside its card, with the `T-` model and the gate kept (ADR-0056); (4) role research and the writer studio first: the slices below, then steps 13-21 as they stand, except what the hosted hub needs; the partial step 13 (`zone-index`, RM-0055) is stashed, its draft spec kept. He accepted the orchestrator's outline, which the table follows. The slices, order and working answers are the orchestrator's and the analyst's, the owner's review open. `v0.29.1` is tagged at 979b8f4, its image published 2026-11-06, so `v4` lands on `main`; a fix the hub's gate 11b-3 needs branches from the tag. The twelve items are `release: v4`, `priority: must`, so `spec roadmap` puts them before steps 13-21 (`should`, `could`): 51-78 pipeline hours, 24-37 up to the studio trial (2d re-estimated from its measured count, 2026-11-09; step 3 split into 3a and 3b the same day, 13-18 hours against 8-12). Closed 2026-11-10 with its eight shipped items; the rest moved by the v5 scope below.

| Step | Slice | Item | Result | ADR issued | Hours | After |
|---|---|---|---|---|---|---|
| 0 | `local-landing` | RM-0080 | Shipped 2026-11-09: the local fence admits a top-level `GET /` navigation from any site, as a hub's landing does, by one shared predicate; `Host` and `Origin` are still judged first | - | 0.5-1 | - |
| 1a | `workspaces` | RM-0081 | Shipped 2026-11-09: a workspace per trade from `[[workspace]]` (sections, landing, card flows, default module), a bad table the implicit one with a note; a Workspace select per viewer, changing no right; Tasks off the nav, still a route; `[[module]]` roots Tree and Flow; `spec workspace`, `GET .../workspaces` with no turn; `scripts/presets/gamedev.toml` (`docs/canon/workspaces.md`) | ADR-0073 `workspaces` | 4-6 | - |
| 1b | `ui-languages` | RM-0082 | Shipped 2026-11-09: the chrome in English and Russian, a Language select per browser; one catalog in `ui/src/i18n/` behind i18next 26.5.11 and react-i18next 17.1.9, English the key source, Russian typed against it; plurals and dates by `Intl`; letter hotkeys by `letterOf`; the shell, nav, palette, Home, `src/ui/`, the labels and 1a's strings; main chunk +99 kB; the owner's review of the Russian open (`docs/canon/ui-languages.md`) | ADR-0072 `ui-languages` | 3-5 | 1a |
| 2a | `flow-scope` | RM-0083 | Shipped 2026-11-09: Flow back to ADR-0055's three link types; `spec flow --stage S` in the CLI, MCP and the daemon, cutting the shown nodes after the fold; plugin 0.30.0; flow, trace and tree timed by `specengine-eval scale`, about 1 s at 100k, whole O(N) reads | - | 3.5-5 | parallel with 1 |
| 2b | `writer-studio` | RM-0084 | Shipped 2026-11-09, 2c included: Flow in three panes, places by module, scope and stage, one scope's canvas that holds still, a panel always shown; the place in the hash, a reload and Back landing in it; an edge click selects the edge; the List an outline of links; a gap handed to the AI as a card; its strings in 1b's catalog; main chunk +12 kB (`docs/canon/spec-flow-screen.md`, `docs/canon/spec-flow-place.md`) | - | 8-12 | 1a, 1b, 2a |
| 2d | `ui-languages-sweep` | RM-0090 | Shipped 2026-11-09, uncut: every screen 1b and 2b left joins the catalog (Inbox, Tasks, Board, Roadmap, Spec tree, Graph, Health, Questions, Flow's panel parts; English 551 values to 1 806); every `src/` directory but five scanned, words in code too; Russian a lazy chunk, the size gate first-load JS, 768.96 kB against a bar of 845; Flow's Up by title; English slips to RM-0091 (`docs/canon/ui-languages.md`) | - | 5-8 | 2b |
| gate | the studio trial | - | Never run: replaced 2026-11-10 by `v5`'s pipeline trial (Q13 of the v5 scope) | - | - | 2b, 2d |
| 3a | `board-handoff` | RM-0085 | Shipped 2026-11-09, uncut: columns that never move, a card modal at `#/<p>/board/<C>` and a page at `.../page`, swimlanes by assignee or parent; a hand-off in one act (`spec card new --reviewer`), a return into a flow's configured `return` column with a reason (`spec card move --note`), the parent's rollup, `spec card list --parent`; the preset's `sent-back` per flow; plugin 0.31.0; first-load JS 794.35 kB of 845; a real-mouse drop the owner's hand check (`docs/canon/card-flows.md`, `docs/canon/board-screen.md`) | ADR-0074 `card-handoff` | 9-12 | 1a, 2d |
| 3b | `card-tasks` | RM-0092 | Shipped 2026-11-10, uncut: a card links `T-` tasks on its own row (`spec card edit --task`, queue schema step 10, backup format 4, which still imports 3); `show`, `get_card` and the daemon serve each task's state read now, `null` with a note where it cannot be read; a Tasks block in the modal and page, the open card re-read on `task.*`; the link decides nothing; a hub shows the IDs without state; plugin 0.32.0; first-load JS 797.95 kB of 845 (`docs/canon/card-tasks.md`) | ADR-0075 `card-task-link` | 4-6 | 3a |
| 4 | `role-dashboards` | RM-0086 | Moved 2026-11-10 to `v5` step 12, re-scoped as `department-overview` | - | 4-6 | 1a, 3a |
| 5 | `roadmap-states` | RM-0087 | Pool since 2026-11-10 (Q12 of the v5 scope): the owner's states derived per read; views by state, release and timeline | `plan-states`, only if `status:` values are added | 3-5 | Q8 |
| 6 | `plain-questions` | RM-0088 | Moved 2026-11-10 to `v5` step 10, re-scoped: a question's gist, why it matters and 2-3 options; collisions as pairs | - | 6-10 | 1b |
| later | `hub-workspace` | RM-0089 | Moved 2026-11-10 to `v5` step 13, `hub-department`: a hub's per-login default, presentation only; `[team]` untouched | `hub-workspace` (supplements ADR-0060) | 1-2 | 11b-3, 1a |

**The studio trial**, the gate after 2b and 2d, no code, never run (replaced by `v5`'s pipeline trial, Q13): a person who did not write the project, given only the URL and one-line tasks, on a local daemon over a copy of a real corpus, unaided (1) finds a named step by module and stage, (2) reads it with no page scroll, (3) clicks an arrow and reads what it joins, (4) hands one gap to their AI as a card the AI then lists with `list_cards`, (5) reloads, presses Back and lands in the same place, (6) switches to Russian and back. Recorded as step 7e was: counts of actions, help requests and dead ends. `v4`'s own bar stays the writers' trial of at least a calendar week ("Only people can do" below).

**The owner's answers, 2026-11-09:** Q1 Russian text in the repository only in the UI's translation catalog; keys, comments, tests and docs stay English; ADR-0072 narrows ADR-0024 to the one file `ui/src/i18n/ru.ts`. Q2 "use any good and reliable library": i18next with react-i18next (MIT), the orchestrator's pick, at exact pins at least two weeks old. 1b no longer waits.

**The owner's answers on step 3, 2026-11-09:** Q1 on a return only the child card moves, into its flow's return column with a reason; the writer's card stays and shows the rollup. Q2 tasks inside cards are local only: a hub card shows linked IDs without state (3b). Q3 no board across trades now: tabs plus assignee lanes, revisited with RM-0086. The analyst's split of step 3 into 3a and 3b, and the rest of 3a's open points, are working answers (`docs/features/board-handoff.md` "Questions").

**Owner stops, each with its working answer:** Q8 new roadmap statuses in `status:`: no, states derived per read plus a `[roadmap]` vocabulary (5 waits). Q9 a gap to a `T-` task from the UI: no, a card only; a task stays a command to copy.

**Working answers taken, the owner's review open:** Q3 only the chrome is translated; daemon, CLI and MCP text stay English (ADR-0072). Q4 Flow back to ADR-0055's three link types (2a): in the owner's real corpus `depends_on` appears only in plan items and steps are ordered with `follows`, so no step order is lost. Q5 a module is a configured root document, or a kind selector, and its subtree; 1a builds the root form, the selector waiting for a route that reads the corpus (`docs/features/workspaces.md` D4). Q6 a hub login's default workspace later (RM-0089), presentation only, `[team]` untouched. Q7 development takes a hand-off by a move, a return is a move with a reason, the writer accepts the result (ADR-0074, 3a). Q10 the List made useful, not dropped. Q11 answered: `v0.29.1` tagged.

**Step 12 in five, 2026-11-03.** The orchestrator split `scale-reads` while the owner was away, 8-12 hours against the 6-10 planned; the split and each decision below await the owner's review. Q1: "under 1 s at 100k" and "under 2x from 10k" measure a read's own work, the trusted path on a hub and a local read after its freshness step; the local stat walk has its own bar, no file read when warm, its time reported. Q2: release build, best of three, about 20 000 files at about five nodes each, target degree and query hits bounded; highly linked nodes reported apart, with no bar. Q3: on a hub, writers update the index inside the turn; the first read after start walks, a dirty mark forces a walk, printed files are checked, a stat reconcile runs in the turn every 5 minutes, the operator's guide names `spec index`; no file watcher. Q4: the local stat key is size, mtime ns, ctime ns, inode and device, with a 2 s racy margin and no opt-out; `spec index --full` repairs. Q5: the code index stays on "never mtime". Q6: one working-answer ADR, `read-freshness`, issued by 12e. Q7: whole-corpus reads use the indexed parse when a file's BLAKE3 matches its row, and stay O(N) (12d). Q8: a new `specengine-eval scale`, a seeded generator, timings and W_after by `w`'s formula, under `--timeout` (12b). Q1 and Q3-Q5 are 12e's.

**The hub's gate, step 11b-3.** No code: the owner's session on a clean VM, then the VPS, with RM-0073 and the files of 11b-1 and 11b-2 at the tag `v0.29.1` (979b8f4, its image published 2026-11-06); a fix it needs branches from the tag. Its bar is VF4-VF9 and HO1-HO17 of `docs/features/hub-ops.md`, H1-H17 of `docs/features/hub-deploy.md`, G1-G10 of `docs/features/hub-accounts.md` and the claude.ai half of the client check (`docs/canon/hub-mcp.md` "Client check"), each finding recorded with the client's or image's version and the date. `--push` and real people come only after it.

**The gate, step 7e, passed 2026-10-31.** The trial of 2026-10-31 again: the same brief, a fresh AI, an empty project, built binaries at the commit of `flow-authoring`, only what a user's AI is given. No code. Its bar, all of: 0 rejected probes; no update made only to fix grammar; discovery under a third of calls; the flow collapses; cards move and accept; a nested edit marks; the timeline fills. One consent over a set of proposals was not in the group: ADR-0038 says "never one for a round", and whether the keys still hurt is the owner's call and a new ADR, kept open in RM-0070.

**The gate's record,** verified against the queue, the daemon and a browser. Counts: rejected probes 0 (6 in the first trial); the queue reads 17 proposals, all applied; updates made only to fix grammar 0; owner acts 7 commands against 33 (one config edit and commit, 2 `spec approve --open`, 2 `card assign`, 2 `card accept`) with 19 consent keys, the first flow's 16 proposals taking one walk; 16 nodes, 15 edges, ten rows measured at 0, `spec check` 0 errors 0 warnings; 6 cards, 2 trades; at 1440x900 an empty column of 227x480 took a real mouse drop at 80 % of its height. Seen, no number: the flow collapses and opens; cards moved and accepted; a text edit of a nested `{#ID}` section marked the card on its document; the stage rows filled; on a real daemon the canvas took the full width, the line `No gaps: every row reads None` came through its real React path, and the console stayed clean. Judgements: discovery, 22 calls of 85, 26 % (70 % in the first trial, about 30 of 43; the bar was under a third), is the AI's own sorting of its calls, where the rejected count is directly observed; that the text reads is a look.

**ADRs.** Each is written by the slice that builds it, so the canon never describes unbuilt machinery as true; only `flow-graph` and `cards` are issued, by step 0, shipped 2026-10-30. `flow-graph` is built by step 1, `flow-links`, shipped 2026-10-31 (`docs/canon/flow-links.md`); `cards` by step 4, `card-store`, shipped 2026-10-31 (`docs/canon/cards.md`); ADR-0057 `card-accept-page` by step 5; ADR-0058 `hub-mode` (replaces the single-user decision) and ADR-0059 `hub-identity` (supplements ADR-0034, ADR-0057: the daemon trusts one identity header) by step 8, `hub-identity`, shipped 2026-11-01 (`docs/canon/hub.md`); ADR-0060 `hub-team` (supplements ADR-0057, ADR-0059: what stayed local opens under `[team]`) by step 9a, `hub-team`, shipped 2026-11-01 (`docs/canon/hub-team.md`); ADR-0061 `hub-confirm` (narrows ADR-0035's "the only confirmation" and ADR-0038's "the UI only stages" on a hub alone, no `supersedes:` key) by step 9b, `hub-confirm`, shipped 2026-11-03 (`docs/canon/hub-confirm.md`); ADR-0062 `apply-outputs` (`roadmap-items.md`, N12; amends ADR-0005's one commit and the generated rule) and ADR-0063 `hub-ids` (a hub's own `PR-` range) by step 10a, `apply-outputs`, shipped 2026-11-03 (`docs/canon/proposal-outputs.md`, `docs/canon/queue-backup.md`); ADR-0064 `hub-push` (the first network verb; narrows ADR-0061's "nothing pushed") by step 10b, `hub-git`, shipped 2026-11-03 (`docs/canon/hub-git.md`); ADR-0065 `hub-mcp` (`/mcp` under each request's login) by step 11, `hub-mcp`, shipped 2026-11-03 (`docs/canon/hub-mcp.md`). ADR-0066 `hub-image` (supplements ADR-0015: a hub ships as a container image built on a version tag) and ADR-0067 `hub-door` (supplements ADR-0034, ADR-0059, ADR-0065: the compose file is the whole host, the proxy alone sets `Remote-User`, a per-person secret on `/mcp`) by step 11b-1, `hub-deploy`, shipped 2026-11-03 (`docs/canon/hub-deploy.md`). ADR-0068 `hub-accounts` (supplements ADR-0034, ADR-0059, ADR-0067: the account acts are the deployment's script, no crate holds account code; accepted by the orchestrator, the owner's confirmation open) by step 11c-1, `hub-accounts`, shipped 2026-11-03 (`docs/canon/hub-deploy.md#accounts`), its stream life built by 11c-2. ADR-0069 `read-freshness` (the local stat key, and only a hub's reads trust the index its writers keep; Q6 of step 12; accepted by the orchestrator, the owner's review open) by step 12e, `read-trust`, shipped 2026-11-04 (`docs/canon/spec-read-freshness.md`). ADR-0070 `spec-ids` (a hub's spec IDs from 100001, a terminal's below; replaces ADR-0063's last clause, ADR-0063 standing for `PR-`; accepted by the orchestrator, the owner's review open) by RM-0073, `spec-ids`, shipped 2026-11-04 (`docs/canon/spec-ids.md`). To come, each in the commit of the slice that builds it, as working answers, the owner's review open: `board-snapshot` (amends ADR-0003: a snapshot of the board enters git on a dedicated branch) by 10c; `zone-index` (amends ADR-0030, ADR-0022), `changeset` (amends ADR-0038: one confirm for a baseline), all three with their slices, in the pool since 2026-11-10. For `v4`'s new slices: ADR-0073 `workspaces` (a workspace is a lens, never an entitlement or a control point) by 1a, written with its spec on 2026-11-08 at the orchestrator's request, its canon `docs/canon/workspaces.md`, built by 1a, shipped 2026-11-09, the owner's review open; ADR-0072 `ui-languages` (replaces the English-only interface, narrows ADR-0024 to the Russian catalog file, amends decision 10; the owner's Q1 and Q2 of 2026-11-09, Q3 a working answer) by 1b, written with its spec on 2026-11-09, its canon `docs/canon/ui-languages.md`, shipped 2026-11-09, the owner's review of the Russian open; later `hub-workspace` (supplements ADR-0060) by RM-0089, maybe `card-handoff` by 3, and `plan-states` by 5 only if `status:` values are added; `local-landing` and `flow-scope` issued none, being canon changes and a return to ADR-0055 (shipped 2026-11-09). For `v5`: ADR-0076 `departments` (amends ADR-0073) by RM-0096, written with its spec on 2026-11-11, its canon `docs/canon/workspaces.md` "The lens", and `card-questions` by RM-0097, written with its slice's spec; RM-0093 issues none ("v5 scope"). Unchanged: ADR-0012, ADR-0032, ADR-0039, ADR-0052, ADR-0054.

**The owner's decisions, 2026-10-30, pre-approved, review open:**

1. Git is the store, a thin server is the door. Approved text, decisions and change records are files in git; the server pushes a snapshot of the board to a dedicated branch, so history on the main branch stays clean; CI validates each push; the board can be rebuilt from git, losing only the last minutes. The server gives the live board, MCP over the network and login through an external proxy.
2. The web UI may confirm under a named login, with a step-up at the proxy on confirm routes.
3. Zone leads confirm brief changes; the owner sets global baselines.
4. A rejected brief stops nothing (ADR-0012 holds).
5. The requesting side accepts a result, never the assignee.
6. The brief lives in the same repository as the game, with a sparse hub clone.
7. One named person operates the host.
8. A card (`C-NNNN`) is a separate lighter record; `T-` stays the code-work extension for phase two.
9. A brief is `canon`; `status: retired` is added.
10. English chrome, pack vocabulary in the project's language (amended 2026-11-08: the chrome in English and Russian, ADR-0072 `ui-languages`, built by v4 step 1b).
11. The hand-written canvas layout is tried on 200 nodes before any library is asked for.
12. One confirm for a baseline, with the impact shown.
13. Steps 0-7 run locally with no server, and trace belongs to the first result.

**Only people can do:** the host, buying the domain and pointing its DNS at the VPS, TLS, proxy, passkeys and backups, about a working day; the by-hand check that a writer's client attaches a remote MCP; judging how the canvas looks; a writers' trial of at least a calendar week. **Risks:** scope (§5 rates creep high, and this reverses a founding rule; the first result tests the idea before any hosting); a hub before `scale-reads` serialises the team behind O(n) reads; a forged identity header on the host passes as anyone, so only the proxy and an agent-free host stand behind it.

**Phase two** keeps the code layer exactly as it is, and the same repository puts brief nodes in every developer's walk. New work: a `card-task-link` and `.ron` binding by data ID (RM-0024); the layer items (RM-0021 to RM-0025) wait for it.

#### v5 scope, 2026-11-10

**The owner's request, 2026-11-10.** He wrote it in chat, in Russian; this is a translation.
- **Speed.** Development should be fast, with everything "on shelves" and grounded in documents.
- **Directions.** Documents and mechanics are simple and spread into directions the project configures (mechanics, models, scenes, ...), so one sees what an object consists of.
- **Flow.** Its left pane shows a path that goes deeper and deeper, not a scope.
- **The pipeline:**
  1. a writer creates an entity and writes it up in a document;
  2. its type is set: a mechanic, a model, a scene, an ambient sound, rules;
  3. it is compared with what exists: reused where it can be, and collisions from parallel work resolved with hints on what to do and why;
  4. a task with its dependencies is split to departments, and each decides on its own side;
  5. the task closes when every subtask is done, or resolved with a comment.

  Each department sees and works its own tasks.
- **The design**, "much more like Jira": the project is the workspace, and each department has its own statuses. Nothing is hard-coded: each project defines its settings, or extends a preset.
- **Local stays local.** Technical things live on each machine as a cache. The server holds documents, links and decisions, and nobody sees low-level things.
- **Questions and decisions.**
  - Writers describe, link and record decisions, so questions close.
  - Development sends a task back with a question, and the writers' answer becomes a decision record or a document that proves the work.
  - A developer who decides alone says so.
  - Who decided what, and why, stays visible.
- **The look.** It should be professional and consistent; today it "looks hand-made".

The same day, Flow v2 (RM-0094, RM-0095): the canvas is noisy, the documents in the right panel are unreadable, and "this is information for the AI".

**His answers, 2026-11-10**, given through the question dialog:
1. **Themes.** Light and dark. Light is the default, dark follows the system setting, and a toggle is remembered per viewer.
2. **Font.** Inter, bundled in the build under the OFL, with Cyrillic, and no font host.
3. **Parent tasks.** A parent task shows "Ready to accept" and a progress bar, and a person accepts it. Nothing closes it on its own (ADR-0074 stands).
4. **Words.** "Project" for the project, "Task" for a card, "Subtask" for a child card, "Agent task" for a `T-` task, and "Department" for a workspace lens. Their Russian lives only in the catalog (ADR-0072). Config keys stay as they are: `[[workspace]]` keeps its name, with no `[[department]]` alias, since a new top-level key makes older binaries refuse the file.
5. **Flow v2.** Records and ID-less documents leave the canvas, into a "Records for the AI" list and the Spec tree. One document modal serves every screen, with technical fields folded and a minimal panel. Now, with a mockup first.

**Working answers taken.** These are the analyst's recommendations, taken at the owner's call; his review is open.
- Q3: a department is focus only, never an entitlement (ADR-0073).
- Q5: people raise question and decision items from a task in the UI (the `card-questions` ADR, RM-0097).
- Q8: the design foundation comes before Flow v2's UI.
- Q9: journeys wait for the trial (RM-0100, optional).
- Q10: presets are copied and extended by `spec init --pack` (RM-0061).
- Q11: a decision record names no card; the card links back to it.
- Q12: RM-0087 `roadmap-states` leaves the direction.
- Q13: the pipeline trial replaces the studio trial.
- Q14: Flow walks top to bottom with stage dividers, until the owner's look.
- Flow v2's Q1-Q4 stand at their recommendations.

**Not taken**, because each would contradict a standing ADR: a parent closing itself (ADR-0074), a department hiding others' work (ADR-0073), and `spec.lock` out of git (ADR-0018, ADR-0041).

**The mapping.**
- **Project** is `[project]` and `#/<p>`.
- **Department** is an ADR-0073 workspace. Its statuses are its `[[card.flow]]` columns. Each column's status category comes from its position: the first column is to do, `accept.to` is done, `return` is returned, and the others are in progress. There is no `categories` key in `v5`.
- **Task** is a card (`C-`), **Subtask** a child card from a hand-off, and **Agent task** a `T-` task.
- **Entity type** is a node kind. **Direction** is a `[[module]]`: a root, or `kinds` from RM-0096.
- **Path** is the served `ancestors` list, the `parent:` chain from the top (RM-0094).
- **Proof** is a decision record that the card links back to.
- **Folded in the UI.** These are local technical details: the UI folds them and never moves them. They are IDs, paths, raw kinds, revisions and hashes, tokens, distance, gap codes, Bindings, Bundle, the lock diff and drift. `T-` tasks show as a Development summary.

The order is the one `spec roadmap` computes: `must` first, then `should`, then `could`, each after what it waits on. Steps 1-6 come to 56-83 pipeline hours (step 3 took RM-0101 in; step 4 re-estimated by its analysis) and steps 7-13 to 27-46.

| Step | Slice | Item | Result | ADR issued | Hours | After |
|---|---|---|---|---|---|---|
| 1 | `design-foundation` | RM-0093 | Shipped 2026-11-10: light and dark tokens as `light-dark()` roles (light by default, dark by the system, a Theme select per viewer); Inter 4.001 bundled as files; a rem type scale, spacing on 8 px, radii, a guard with 29 listed exceptions; Button, Lozenge, PropertyList, Breadcrumbs, IssueView, EmptyState, Disclosure; the card modal and page as the first work item; column heads by status category; a one-row header; first-load JS 806.03 kB of 845 (`docs/canon/ui-design.md`) | - | 12-17 | - |
| 2 | `flow-scenario` | RM-0094 | Shipped 2026-11-10: a domain-free `in_flow` on every flow node, folded, `types`, and an opt-in cut counting `not_in_flow`; the canvas the cut's nodes, top to bottom, stage dividers, 160 px boxes, a 0.92 fit floor, off-walk edges on selection; Gaps in the bar; Records for the AI on demand; a served `ancestors` path from the subset's `PARENT` rows, the places pane that path, Up gone; plugin 0.33.0; first-load JS 815.13 kB of 845; its follow-up RM-0101 `gap-titles` in the pool (`docs/canon/spec-flow-cuts.md`, `spec-flow-bar.md`) | - | 11-17 | 1 |
| 3 | `doc-modal` | RM-0095, RM-0091, RM-0101 | Shipped 2026-11-11: link ends' `title`, `end_kind` and gap items' `titles` (RM-0101) served; one document view, the `doc` hash pair's modal over any screen and a page, the path its breadcrumb, links by title, Inside on expansion, technical details folded; `DocLink` outside the Spec tree; Flow's panel minimal; RM-0091's thirteen plural fixes; plugin 0.34.0; first-load JS 824.13 kB of 845 (`docs/canon/doc-view.md`) | - | 10-16 | 1, 2 |
| 4 | `departments` | RM-0096 | The owner's words; `[[card.flow]] title`; `[[module]] kinds` directions (Tree and Inside; Flow rooted only, RM-0102 in the pool); hand-off by department, its `disciplines` served; preset flow titles; plugin 0.35.0 (`docs/features/departments.md`) | ADR-0076 `departments` (amends ADR-0073) | 10-14 | 1 |
| 5 | `card-questions` | RM-0097 | Intake items name `card_id` (queue schema 11, backup format 5); a decision's `why`; Activity; "Ask the writers" in Return; answers from the task | `card-questions` (supplements ADR-0056, ADR-0074, ADR-0075) | 10-14 | 1 |
| 6 | `handoff-progress` | RM-0098 | Ready to accept, a segmented bar, cancelled subtasks with reasons, "Relies on"; UI only | - | 3-5 | 1 |
| gate | the pipeline trial | - | No code: the bar below | - | - | 1-6 |
| 7 | `design-sweep` | RM-0099 | (a) Inbox, Questions, Spec tree, Overview; (b) Health, Graph, Roadmap, Tasks, on the foundation; the legacy tokens and step 1's exceptions gone | - | 8-14 | 1, gate |
| 8 | `intake-similar` | RM-0059 | `similar` and `distinct_from` on create | - | 2-4 | - |
| 9 | `collision-pass` | RM-0060 | Candidates, a pair ledger, a skill | - | 3-5 | 8 |
| 10 | `plain-questions` | RM-0088 | Re-scoped: a question's gist, why it matters and 2-3 options; collisions as pairs | - | 6-10 | 5, 8, 9 |
| 11 | `pack` | RM-0061 | `spec init --pack`, a config proposal, a non-game preset | - | 3-5 | 4 |
| 12 | `department-overview` | RM-0086 | Re-scoped: an Overview per department, counted by status category | - | 4-6 | 4, 5, 6 |
| 13 | `hub-department` | RM-0089 | A hub login's default department, presentation only | maybe `hub-workspace` | 1-2 | 4, 11b-3 |
| opt. | `journeys` | RM-0100 | One act hands a task to several departments, with a soft `after` | one, for the new key | 6-9 | 4, 6, gate |

**Lanes.** Two lanes run side by side.
- **The Rust lane** holds the cores of steps 2-5 (`in_flow` and `ancestors`, link ends' and gap members' titles, the department keys, the card's intake link), then steps 8 and 9.
- **The UI lane** starts with step 1, then takes the UIs of steps 2-5, then steps 6 and 7.

A slice's UI waits for its core and for step 1; no core waits for the UI lane. Steps 10-13 follow both lanes.

**The pipeline trial** is the gate after step 6. It needs no code and replaces the studio trial (Q13). A person who did not write the project gets only the URL and one-line tasks, on a local daemon over a copy of a real corpus, with two departments configured (writers and developers). Unaided, they:
1. find a named document by its path and read it in the modal;
2. make a task on it and hand one subtask to each department;
3. as the developer, send one subtask back with a question to the writers;
4. as a writer, answer with a decision, which then shows in the task's Activity with who decided and why;
5. as the developer, take the subtask again and move it on; the reviewer accepts both subtasks, the parent shows "Ready to accept" and its bar, and the reviewer accepts the parent;
6. switch the theme and the language, and switch them back.

It is recorded as step 7e was: counts of actions, help requests and dead ends, plus each screen's look in both themes. `v5`'s own bar stays v4's: a writers' trial of at least a calendar week ("Only people can do" above).

**Bookkeeping, 2026-11-10.**
- **`v4`** keeps its eight shipped items, steps 0-3b.
- **Into `v5`.** RM-0086 becomes step 12 and RM-0088 step 10, both re-scoped; RM-0089 becomes step 13. Steps 17-19 (RM-0059 to RM-0061) become steps 8, 9 and 11. RM-0091 ships inside step 3.
- **To the pool**, priorities kept: 10c (RM-0052), steps 13-16 (RM-0055 to RM-0058, with RM-0035 and RM-0036 inside step 14), steps 20 and 21 (RM-0062, RM-0017), RM-0070 and RM-0063. RM-0087 goes to the pool as `could` (Q12).
- `zone-index`'s draft spec stays committed.
- RM-0101 `gap-titles`, step 2's follow-up, joins the pool as `should`; the same day it rides in step 3's Rust lane, as `must` in `v5` (one plugin MINOR for both).
- RM-0102 `flow-directions`, Flow drawn through a kind direction, which step 4 leaves out, joins the pool as `could` (2026-11-11).
- `spec roadmap` still reads `current mvp`.

**Risks.**
- **Old binaries.** New config keys break older binaries (ADR-0073's rollout rule).
- **Size.** First-load JS is 797.95 kB of a bar of 845. Fonts are not JS. A slice that would pass the bar first moves Roadmap or Graph out of first load.
- **The queue.** Step 5 takes queue schema step 11.
- **Scale.** The path must come from the subset read's parent edges.
- **Accessibility.** Disclosures, focus, and contrast in two themes.
- **Both languages** for every new string.
- **Wording.** ADR-0012 and ADR-0074 hold: "Ready to accept" is a hint, never a gate.
- **Genre.** No kind, column, record or branch word enters the core.
- **Anonymity.**
- **Two stores on a hub.**
- **Test churn.** Each slice lists the tests it replaces, and why.

## 3. Product acceptance criteria

| # | Criterion | How it is checked |
|---|---|---|
| AC-1 | **Task W**: median on the pilot projects ≤ 40 KB or the project's own target | `bundles` log on every task document per pilot. **Measured 2026-10-05** (`specengine-eval w`, budget 10 000, debug; A / B): tasks 70 / 51; median (p90, max) W_before 533 076 (743 566, 922 383) / 225 562 (396 166, 483 850), W_after 43 061 (45 048, 45 760) / 27 233 (39 154, 41 216), W_after_followups 101 031 (160 515, 200 347) / 76 516 (306 313, 417 480); third_step 70 / 21, incomplete 67 / 15, refused 0, unresolved 100 / 0, wiki_links 1 030 / 2 216; 40 KB (not an AC) unmet on both W_after_followups medians, met on B's W_after; bundle ms median 2 035 / 640. Phase 2 re-measures: `bundles` log, `--task` at `bundle_task`, follow-ups, live-check tasks |
| AC-2 | **AST isolation**: `cargo fmt`, edits to comments and neighbouring functions do not change a symbol's hash | property test over all symbols of the pilot projects. **Phase 0 baseline met for fmt and comments**: 100 % on both pilots (`specengine-eval ast-hash`, 05 §5.2); the Phase 3 property test **met on both pilots** (5a-1 fixtures; 5a-2 TE-1, 2026-10-20: every judged level 100.0, 0 unstable rows; `docs/canon/code-hash-eval.md` "Measured"); verify honours it by declared level since 5b-1b (2026-10-20: rustfmt, a comment, a neighbour item, a sibling RON value read `ok`; `docs/canon/spec-verify.md` "Cascade", CLI `tests/verify_levels.rs`) |
| AC-3 | **Single door**: no agent-facing MCP tool modifies spec files | call every tool on a temp repo + `git status` is empty |
| AC-4 | **Task approval**: without an approved task an agent cannot write a file in a gated zone (`selective`), no daemon needed; in `observe` it passes, a finding recorded | hook integration test. **Met 2026-10-08** (`plugin-gate`); in Claude Code: the owner's check |
| AC-4b | **Nothing is blocked by a discrepancy** (ADR-0012): an open proposal does not change the task status, several proposals can be open on one node, applying the second one rebases | test |
| AC-5 | **No repeated questions**: `ask_question` on an already answered question returns the decision instead of opening a new one | test: a repeated call yields `decision`, the question count does not grow. **Met 2026-10-05** (`agent-intake` AC-04); a decided item returns its record (`decision-apply` AC-11, 2026-10-06) |
| AC-6 | **Lossless import**: all records of the corpus are imported, verbatim-text hashes match; **the project's full test suite is green** after migration | importer report + the project's `cargo test`. **Dry run met 2026-10-04** (`specengine-eval layout`): A 2 242 of 2 249 definitions matched (7 headers core rejects), B 251 of 251; none unexplained, enforce clean; B one residue, a corpus fact. **A's half met 2026-10-07**, through a pilot-side wrapper; the SpecEngine-only replay is RM-0013: 2 180 of 2 180 definitions matched, no prose, residue or emitter finding, a re-run byte-identical, the suite green (1 693 tests in both build configurations, 1 733 on the merged commit). B: at its migration (RM-0014) |
| AC-7 | **Bundle determinism**: one state → one `bundle_hash` | test. **Re-confirmed on both pilots** 2026-10-05 (`w`: every bundle twice, `nondeterministic` 0) |
| AC-8 | **Machine-verified**: `verified` is set only after SpecEngine itself runs the `@verifies` tests | test. **Met 2026-10-21**: the run (`verify-tests`, 8a-1, 2026-10-16: `spec verify --tests` on the owner's terminal, `docs/canon/spec-verify-tests.md`) and its evidence (`tested-evidence`, 8b-1: a `[[tested]]` block in `spec.lock`, `verified` computed on read, `docs/canon/spec-lock-tested.md`); CLI `tests/verify_tests_*.rs`, `tests/tested_*.rs`; in the reads (`tested-reads`, 8b-2, 2026-10-21): `tests/tested_reads.rs` |
| AC-9 | **Staleness**: editing a node from the `spec_snapshot` of a `ready` task makes it `stale`, the bundle shows the diff | test |
| AC-10 | **Performance**: full symbol hash ≤ 2 s per 300 kLOC, full index ≤ 10 s for hundreds of md files; per-file increment ≤ 200 ms. The `ra_ap_ide` layer is measured separately | benchmark on the pilot projects. **`ra_ap_ide` layer measured in Phase 0** (`specengine-eval ra`, 05 §5.1): cold 33–65 s, warm pass ≈ 0.2 s, group peak ≤ 3.76 GiB; `spec resolve` (TE-6, 2026-10-22, release; A / B) 9.1 / 10.7 s, peak 2.76 / 3.46 GiB. Index (`specengine-eval index`, 2026-10-03, dev profile): `full_ms` A 3 637, B 1 373; `one_file_ms` A 184, B 76. Code index (`specengine-eval code-index`, 6a-2: `docs/canon/symbol-index.md` "AC-10 measurement"): release, `hash_ms` × 300 / `kloc` ≤ 2 000, `one_file_ms` ≤ 200, `noop_ms` ≤ 200; pilot figures: the owner's TE-8. v3 levels (5a-2 TE-1, 2026-10-20, release; A / B): 4.09 / 4.17 ms/kLOC against 6.67 (`docs/canon/code-hash-eval.md` "Measured") |
| AC-11 | **Homoglyphs**: IDs with mixed scripts are rejected with an auto-fix | test on IDs where a Latin letter is swapped for its Cyrillic look-alike (U+0420 for `P`, U+0415 for `E`). **Rejection met** (`spec check`, `check_ids.rs`); applying the fix: Phase 2 (`apply_proposal`) |
| AC-12 | **Responsiveness**: an MCP event is visible in the UI ≤ 1 s | e2e |
| AC-13 | **Meaning change without revision fails**: pre-commit fails if a locked node's `norm_hash` changed with neither a revision raise nor an editorial refresh of its lock entries | test on a temp repo. **Met 2026-10-15** (`rev-rule`, 4b-2: `rev-not-raised` in `spec check --staged`, `docs/canon/node-revision.md` "Raising"; CLI `tests/rev_rule_hook.rs`, a real hook on temp repos); the raise at apply `rev-discipline` (4b-1, 2026-10-14: `docs/canon/proposal-revisions.md`); the pilots' run is the owner's |
| AC-14 | **"Could not verify" ≠ "fresh"**: an unparseable file yields `cannot_verify` and exit 2, not a green run; two different unparseable items do **not** get the same hash | test (regression for the `is_extra()` trap on ERROR). **Hash half met in Phase 0** (`specengine-code` `tests/hash_traps.rs`); **exit 2 met 2026-10-13** (`spec verify`, every mode: `docs/canon/spec-verify.md` "Verdict and exit", CLI `tests/verify_cannot.rs`), kept on lock format 2 2026-10-20 (`two_broken_golden_items_are_never_compared`), for `module` and `ron` bindings by 5b-1b (`module_file_error_elsewhere_cannot_verify`, `broken_ron_file_binding_cannot_verify`) |
| AC-15 | **Rename is not drift, twin is not a trap**: a renamed function with a non-trivial body is tracked (`MOVED`), a trivial one (< 124 B buffer) yields `LOST`/`STALE` rather than false tracking; copies the fingerprints cannot tell apart (one body or skeleton, one quote) yield `AMBIGUOUS` | test |

## 4. Importing existing corpora

One core for all projects (ADR-0008); corpus specifics live only in `specengine.toml` and the importer, an adapter translating the project's convention (§4.1) into SpecEngine nodes, records and links.

### 4.1. What the importer carries over

| Source in the corpus | → | Notes |
|---|---|---|
| Documents with front-matter (requirements, decisions, features) | nodes and records, header normalization | `canon:` on decisions is kept; feature criteria → `{#AC-..}` sections (ADR-0026) |
| Records as table rows (assumptions, questions, terms), also rows without an ID | one record per row, `immutable_text`; missing IDs issued (agent proposes, owner confirms) | status prefixes in cells ("closed …", "implemented in code …") → status + events; text verbatim |
| Table-style headers and journals (registry, decisions, trade-offs, blockers, milestones) | registry -> computed; journal -> `DEC-NNN` with `cost`; blockers -> high-severity questions; milestones -> plan items (ADR-0052) | decisions without `cost` -> debt baseline |
| Principles and check codes | `principle`, `check`; constant bindings; candidates reported (`specengine-eval constants`), no code written (ADR-0016) | old designations -> aliases (ADR-0009) |
| README files of code subtrees (Tier 1) | domain nodes | stay in place (locality), indexed as `domain` |
| Roadmap state and queue | plan items (`[roadmap]`, ADR-0052); work in flight -> tasks | the markdown becomes generated |
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
| The tool eats time from the main projects (scope creep) | high | MVP = CLI + MCP; live UI only after live-task validation; "enforcement first, the bot never first"; the hub only after the first result (ADR-0058) |
| The core silently bends toward one project | medium | pilots of different nature connected from Phase 1 (ADR-0008); specifics only in `specengine.toml` and the importer; core tests on fixtures of all pilots |
| Bevy 0.20 changes the schedule API | medium | tree-sitter detector covered by `fixtures/bevy-mini`; observers behind a Bevy-version gate (05 §5.1); the `schedule_data` dump schema changes across minors: re-run `specengine-eval bevy-detector --dump` per Bevy upgrade |
| The MCP protocol changes again | medium | rmcp + both eras; logic in core, MCP a thin adapter; Claude Code verified on 2.1.283 (MCP README), re-check on upgrade; Streamable HTTP without GET → 405 unmeasured (stdio-only build) |
| rust-analyzer memory near the threshold | medium | whole-group peak 3.76 of 4 GiB (≈ 6 % headroom) on the heavier pilot; re-measure (`specengine-eval ra`) when a pilot grows or `ra_ap` is bumped; fallback: no proc-macro server (≤ 3.2 GiB) |
| HTTP hook is open when the daemon is down | known | `command` hook with exit 2 + managed-deny |
| Specs do not reduce the cost of change (O(n) cross-cutting edits) | fact | do not promise it; show the blast radius via `spec impact`, `get_impact` (shipped: `docs/canon/spec-impact.md`) |
