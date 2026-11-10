---
class: canon
tier: 0
scope: [root]
owner: owner
reviewed: 2026-10-21
---

# SpecEngine

A local specification engine for projects built with AI agents; Rust first, including ECS-style game code. It keeps a business-logic tree and atomic records (requirements, assumptions, questions, decisions, criteria) in git next to the code, binds them to code symbols through markers and AST hashes, surfaces drift, and runs a proposal queue for the owner. Interfaces: CLI, MCP for Claude Code agents, a web UI. The goal: the context cost of a task does not grow with the size of the project.

State: shipped: Phases 0-1 (parser, index, reads, graph, bundle, check, shards, layer A, pilot W), Phase 2 slices 1-7, `daemon-read`, `decision-staging`, `plugin-*`, `round`, `spec-serve`, `kind-*`, `health-left`, Phase 3 `code-refs`, `norm-hash`, `spec-lock`, `spec-verify`, `rev-discipline`, `rev-rule`, `drift-queue`, `drift-follow-up`, `hash-levels`, `hash-levels-ron`, `hash-levels-eval`, `hash-levels-use-lists`, `lock-levels`, `lock-rebase`, `lock-levels-verify`, `anchor-states`, `symbol-discriminator`, `symbol-index`, `symbol-reads`, `code-tools`, `bevy-roles`, `bevy-roles-reads`, `verify-tests`, `verify-task`, `verify-node`, `check-binding`, `show-bindings`, `bundle-bindings`, `impact`, `impact-since`, `tested-evidence`, `tested-reads`, `health-code`, `health-code-ui`, `health-ui-follow-up`, `health-drift`, `health-drift-ui`, `citation-sweep`, `constant-census`, `layer-c`, `layer-c-resolve`, `layer-c-reads`, `hygiene-pass`, Phase 4 `ui-*` (RM-0011: `ui-node-bindings`, `ui-drift-evidence`, `ui-tree-drift`; RM-0033: `lock-diff`, `lock-diff-ui`), Phase 5 `review-form` (RM-0012, ADR-0054), `import-homes`, `import-lead-ins` (RM-0013), Phase 6 `roadmap-items`, `roadmap-view`, `roadmap-position`, `roadmap-surfaces`, `roadmap-screen`, Phase 7 `full-cycle-decisions`, `flow-links`, `flow-read`, `flow-canvas`, `card-store`, `board-ui`, `card-trace`, `trace-views`, `trace-truth`, `proposal-rehearsal`, `flow-authoring`, `screen-fit`, `hub-identity`, `hub-team`, `hub-confirm`, `apply-outputs`, `hub-git`, `hub-mcp`, `hub-deploy`, `hub-accounts`, `stream-life`, `flow-linear`, `scale-eval`, `edge-table`, `store-reads`, `read-trust`, `spec-ids`, `hub-ops`, `v4` `local-landing`, `workspaces`, `ui-languages`, `flow-scope`, `writer-studio`, `ui-languages-sweep`, `board-handoff`, `card-tasks`, `v5` `design-foundation`, `flow-scenario`, `doc-modal`, `departments`. Plan: `spec roadmap` (items in `docs/roadmap/`, the view `docs/generated/roadmap.md`), `docs/specs/specengine-platform/08-roadmap.md`. Planned next: Phase 7 `full-cycle` (flow, board, trace, then a hosted hub; shipped: steps 0-7, 7a `trace-truth`, 7b `proposal-rehearsal`, 7c `flow-authoring`, 7d `screen-fit`, step 8 `hub-identity`, step 9a `hub-team`, step 9b `hub-confirm`, step 10a `apply-outputs`, step 10b `hub-git`, step 11 `hub-mcp`, step 11b-1 `hub-deploy`, step 11c-1 `hub-accounts`, step 11c-2 `stream-life`, step 12a `flow-linear`, step 12b `scale-eval`, step 12c `edge-table`, step 12d `store-reads`, step 12e `read-trust`, RM-0073 `spec-ids`, step 11b-2 `hub-ops`, ADR-0055 to ADR-0071, `docs/canon/flow-links.md`, `docs/canon/cards.md`, `docs/canon/board-ui.md`, `docs/canon/spec-trace.md`, `docs/canon/hub.md`, `docs/canon/hub-team.md`, `docs/canon/hub-confirm.md`, `docs/canon/proposal-outputs.md`, `docs/canon/hub-git.md`, `docs/canon/hub-mcp.md`, `docs/canon/hub-deploy.md`, `docs/canon/hub-ops.md`, `docs/canon/spec-read-freshness.md`, `docs/canon/spec-ids.md`). `first-result` is closed, by a gate and not a slice: step 7e, the re-run of the trial with no code, passed 2026-10-31, so one owner on the local daemon has his AI write a real flow from an empty project and sees a whole cycle. Step 12 `scale-reads` is closed: its five slices, `flow-linear`, `scale-eval`, `edge-table`, `store-reads` (RM-0076 to RM-0079) and `read-trust` (RM-0054), shipped 2026-11-04. The owner scoped release `v3` to the hosted hub on 2026-11-04 (`08-roadmap.md` Phase 7, "Release scope"). Its code items, RM-0073 `spec-ids` and 11b-2 `hub-ops` (RM-0074), shipped 2026-11-04: the hub's code for `v3` is done. Next: the owner's gate 11b-3 on a VPS (`v0.29.1` tagged at 979b8f4, its image published; a fix it needs branches from the tag): VF4-VF9, HO1-HO17, H1-H17, G1-G10 and the claude.ai check, then `--push` and real people. `v4`, on `main`, re-scoped by the owner on 2026-11-08 (`08-roadmap.md` Phase 7, "v4 scope"): trade workspaces and the writer studio first, RM-0080 to RM-0090 (`local-landing`, `workspaces`, `ui-languages`, `flow-scope`, `writer-studio`, a language sweep, a studio trial, then board, dashboards, roadmap states, plain questions); then step 13 onward, with 10c `board-in-git`. Its step 0 `local-landing`, 1a `workspaces` (ADR-0073, `docs/canon/workspaces.md`), 1b `ui-languages` (ADR-0072, `docs/canon/ui-languages.md`), 2a `flow-scope`, 2b `writer-studio` (Flow in three panes, `docs/canon/spec-flow-place.md`), 2d `ui-languages-sweep` (every screen in the catalog, Russian a lazy chunk) and 3a `board-handoff` (ADR-0074, `docs/canon/card-flows.md`, `docs/canon/board-screen.md`) shipped 2026-11-09, and 3b `card-tasks` (ADR-0075, `docs/canon/card-tasks.md`) 2026-11-10 (plugin 0.32.0, the next tag `v0.32.0`), closing `v4`. `v5`, scoped by the owner on 2026-11-10 (`08-roadmap.md` Phase 7, "v5 scope"): a Jira-like UI over the pipeline, RM-0093 to RM-0099 with RM-0059 to RM-0061, RM-0086, RM-0088, RM-0089 and the optional RM-0100, a pipeline trial after its step 6 replacing the studio trial; v4's unstarted tail (10c, steps 13-16 and 20-21, RM-0070, RM-0087) is in the pool. Its step 1 `design-foundation` (two themes, Inter, the scale, primitives, the card as a work item: `docs/canon/ui-design.md`) and step 2 `flow-scenario` (`in_flow`, the served path, Flow top to bottom: `docs/canon/spec-flow-cuts.md`) shipped 2026-11-10, step 3 `doc-modal` (one document modal and page everywhere, served link-end and gap titles: `docs/canon/doc-view.md`) and step 4 `departments` (the owner's words, flow titles, `kinds` directions, a hand-off by department: ADR-0076, `docs/canon/directions.md`) 2026-11-11 (plugin 0.35.0, the next tag `v0.35.0`). Next: step 5 `card-questions` (RM-0097, its ADR `card-questions`).

## How to read

`CLAUDE.md` -> `docs/index.md` -> at most three documents. The root lists Tier 0-1 canon and live specs; `docs/canon/*`, decisions, crate READMEs and plan items are in the live shards it links, one step each. A third step is an index or canon defect: fix it. The archive is read by id only. Long files: headings first, then the needed section. Details: `docs/README.md`.

## Rules that must not be broken

Full list with reasons: `docs/canon/architecture.md`; every rule changes only through a new ADR.

- The source of truth is files in the project's git; SQLite is an index and a queue, outside the repository (ADR-0001, ADR-0003).
- Spec files are written only by `apply_proposal` once a person confirms, where the proposal was raised, as a separate commit with provenance: on a terminal, or on a hub alone under a login `[team]` entitles, one click per item; a hub's merge of others' commits authors none (ADR-0004, ADR-0005, ADR-0032, ADR-0035, ADR-0061, ADR-0064).
- **Nothing is blocked by a discrepancy.** The only control point is the owner approving a task; the daemon-free hook is closed with no verdict (ADR-0012, ADR-0039).
- The core knows no subject domain: project specifics live in its `specengine.toml` and importer (ADR-0008).
- IDs are Latin-only, no mixed scripts; legacy IDs are aliases (ADR-0009).
- Markers `// @implements ID@rev [tiers]`; legacy citations become `mentions` (ADR-0010, ADR-0016, ADR-0018).
- MVP: tree-sitter + markers; rust-analyzer in Phase 3 (ADR-0020, ADR-0021).
- UI: React 19 + Vite + `@xyflow/react`; chrome English or Russian per viewer (ADR-0011, ADR-0072).
- The documentation convention is mandatory here and in every project under SpecEngine (ADR-0022).
- All repository content is in English but the UI's Russian catalog (ADR-0024, ADR-0072).

<a id="process"></a>
## Process: all development goes through roles

Code and documentation change **only through the pipeline** `/feature <requirement>`. The main session is an orchestrator: it hands stages to roles, passes results along, sums up, and writes no code, tests or specs itself. Exception: a direct instruction from the owner in the current session. ADR-0023, ADR-0029.

| Role | Does | Writes only to |
|---|---|---|
| `requirement-analyst` | requirement analysis, assumptions, questions, criteria | -- (nothing) |
| `spec-writer` | task specs, ADR + canon diff, index | `docs/`, `CLAUDE.md`, `*/README.md`, `README.md`, `specengine.toml`, `.spec-debt.toml` |
| `rust-developer` | Rust code | `Cargo.toml`, `crates/*/src`, `plugin/`, `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/` |
| `ui-developer` | web UI | `ui/**`, except generated types |
| `test-engineer` | tests and running checks | `crates/*/tests`, `fixtures/`, `#[cfg(test)]` |
| `code-reviewer` | review against the spec and the rules | -- (no Write/Edit) |

The developer must not touch `docs/`: a discrepancy between spec and code must be visible, not smoothed over. At most three implementation iterations; if they run out, the task is not marked implemented. Roles do not commit: the owner commits. `.claude/**` is the owner's: a role puts its text in the spec; the owner applies it. Role models live in front-matter (Opus 5.5).

## Documentation

Classes: canon, decision, spec, generated; tiers: `docs/README.md`. Caps: Tier 0 16 KB, Tier 1 and index 10 KB, Tier 2 canon 12 KB, ADR 1.5 KB. An accepted decision changes the canon in the same change; its `canon:` points at the section. `docs/index*.md` and `docs/generated/` are generated only. Before handing in any documentation change:

```bash
cargo run -q -p specengine-cli -- export index && cargo run -q -p specengine-cli -- check
```

The pre-commit hook and CI reject a red check (`docs/README.md` "Enforcement").

## Layout

- `docs/` -- `canon/`, `decisions/`, specs (`features/`, `specs/`), plan items (`roadmap/`), index and the roadmap view (`generated/`), archive.
- `crates/specengine-{model,core,store,cli,code,eval,import,mcp,http,ra}` -- the corpus model and reference grammar, the parser and check, the index (SQLite + FTS5), the `spec` binary, layer A parsing and hashing, the measurement harness, the corpus census, the stdio MCP server, the daemon, layer C (outside `default-members`); each has a Tier 1 `README.md`.
- `fixtures/` -- test corpora with `expected.json`; `bevy-mini`, `ra-mini` workspace-excluded.
- `.claude/` -- pipeline roles and commands.
- `plugin/` -- the Claude Code plugin (root `README.md`); `ui/` -- the web UI.
- `scripts/` -- the hooks installer, `scripts/deploy/`, a hub's compose deployment and image (its `README.md` the operator's guide), and `scripts/presets/`, the packs a project copies.

## Owner's machine

A macOS laptop, not a build server. Build directory: `target.noindex` (`.cargo/config.toml`); do not create a second one. Tests: `cargo nextest run -p <crate> --test <file> <filter>`; the full run once at the end. Stress load only in self-terminating form `perl -e 'alarm 120; exec "yes"'`; never leave background processes without a timeout. Consumer projects used to exercise SpecEngine are read-only.
