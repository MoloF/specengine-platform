---
class: canon
tier: 0
scope: [root]
owner: owner
reviewed: 2026-11-13
---

# SpecEngine

A local specification engine for projects built with AI agents; Rust first, including ECS-style game code. It keeps a business-logic tree and atomic records (requirements, assumptions, questions, decisions, criteria) in git next to the code, binds them to code symbols through markers and AST hashes, surfaces drift, and runs a proposal queue for the owner. Interfaces: CLI, MCP for Claude Code agents, a web UI. The goal: the context cost of a task does not grow with the size of the project.

State: shipped: Phases 0-1 (parser, index, reads, graph, bundle, check, shards, layer A, pilot W); Phase 2 slices 1-7 with `daemon-read`, `decision-staging`, `plugin-*`, `round`, `spec-serve`, `kind-*`, `health-left`; Phase 3 (code refs, norm hashes, `spec.lock`, `spec verify`, revision discipline, drift, hash and lock levels, anchors, the symbol index and reads, Bevy roles, tested evidence, impact, code and drift health, layer C, the hygiene pass); Phase 4 `ui-*`; Phase 5 `review-form`, `import-homes`, `import-lead-ins`; Phase 6 `roadmap-*`; Phase 7 `full-cycle` steps 0-12 (flow, board, trace, the hosted hub, `scale-reads`), `spec-ids`, `hub-ops`. Each slice's spec is in `docs/features/`, its canon routed by `docs/index.md`. Plan: `spec roadmap` (items in `docs/roadmap/`, the view `docs/generated/roadmap.md`), `docs/specs/specengine-platform/08-roadmap.md` (Phase 7: each release's scope and the owner's words). `first-result` closed 2026-10-31 by its gate, step 7e. `v3`, the hosted hub, is code-complete; its owner's gate 11b-3 on a VPS is open (`v0.29.1` tagged at 979b8f4, its image published; a fix it needs branches from the tag): VF4-VF9, HO1-HO17, H1-H17, G1-G10 and the claude.ai check, then `--push` and real people. `v4` (trade workspaces, the UI in English and Russian, the writer studio, the Board's hand-off, a card's tasks) shipped 2026-11-09..10. `v5`, a Jira-like UI over the pipeline, shipped in full 2026-11-11..13 (`08-roadmap.md` Phase 7, "v5 scope"): the design foundation, the document modal, departments, a task's questions, split, mandate, progress, context and upstream signal, a node's history, intake similarity, collisions and plain questions, presets (`spec init --pack`), `spec delivery` with its timeline (`delivery-timeline`, 2026-11-13, the last), `spec catalog`, department overviews, a hub's department per login; the owner pushed `main` and `v0.45.0` (plugin 0.45.0) on 2026-11-13. Then `release-hardening` (the known flakes fixed in the tests, the scan's defects closed, `GATE_DEADLINE` 5 s) and RM-0111 `usage-orphans` (what uses a document, where, and which documents nothing links: `docs/canon/spec-usage.md`) shipped 2026-11-13 in one commit (plugin 0.46.0, the next tag `v0.46.0`); its clean full run, RH-G1, passed on that commit (6037 tests, 0 failures). The writer map closed with RM-0112 `corpus-map` (the whole corpus as one map, `spec graph --both` and `--kind`; ADR-0080, `docs/canon/spec-map.md`) and RM-0113 `scope-rings` (`[[map.ring]]` scope rings: `docs/canon/spec-rings.md`), shipped 2026-11-13 in one commit, no plugin change (it stays 0.46.0). v4's unstarted tail (10c, steps 13-16 and 20-21, RM-0070, RM-0087) is in the pool. Next, the owner's gates only (`08-roadmap.md` Phase 7, "Owner's word, 2026-11-13"): the hub's next upgrade, from 0.45.0 (queue schema 14, backup format 7, upgraded 2026-11-13) to 0.46.0, with no schema or format step (`docs/canon/hub-ops.md` "Upgrade", a dump first); a hub runs tagged images, so the map and rings reach it only with a tag at or after their commit; gate 11b-3; the pipeline trial.

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
- `scripts/` -- the hooks installer, `scripts/deploy/`, a hub's compose deployment and image (its `README.md` the operator's guide), and `scripts/presets/`, the packs `spec init --pack` installs or a project copies.

## Owner's machine

A macOS laptop, not a build server. Build directory: `target.noindex` (`.cargo/config.toml`); do not create a second one. Tests: `cargo nextest run -p <crate> --test <file> <filter>`; the full run once at the end. Stress load only in self-terminating form `perl -e 'alarm 120; exec "yes"'`; never leave background processes without a timeout. Consumer projects used to exercise SpecEngine are read-only.
