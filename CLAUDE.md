---
class: canon
tier: 0
scope: [root]
owner: owner
reviewed: 2026-10-01
---

# SpecEngine

A local specification engine for projects built together with AI agents; Rust first, including ECS-style game code. It keeps a business-logic tree and atomic records (requirements, assumptions, questions, decisions, criteria) in git next to the code, binds them to code symbols through markers and AST hashes, surfaces drift, and runs a proposal queue for the owner. Interfaces: CLI, MCP for Claude Code agents, later a web UI. The goal: the context cost of a task does not grow with the size of the project.

State: Phase 0 done; **Phase 1** (reading core) in progress: parser, index, CLI reads and graph, `spec check` 1–3 (`--staged`, `--changed`), sharded `export index` shipped. Plan: `docs/specs/specengine-platform/08-roadmap.md`.

## How to read

`CLAUDE.md` → `docs/index.md` → at most three documents. Needing a third step is a defect of the index or the canon: fix it. The archive is read by id only. Long files: headings first, then the section you need. Details: `docs/README.md`.

## Rules that must not be broken

Full list with reasons: `docs/canon/architecture.md`; every rule changes only through a new ADR.

- The source of truth is files in the project's git; SQLite is an index and a queue, outside the repository (ADR-0001, ADR-0003).
- Spec files are written only by `apply_proposal` on the owner's action, in the task branch's worktree, as a separate commit with provenance (ADR-0004, ADR-0005).
- **Nothing is blocked by a discrepancy.** The only control point is the owner approving a task; the hook is closed when the daemon is unavailable (ADR-0006, ADR-0012).
- The core knows no subject domain: project specifics live in its `specengine.toml` and importer (ADR-0008).
- IDs are Latin-only, no mixed scripts; legacy IDs are aliases (ADR-0009).
- Markers `// @implements ID@rev [tiers]`; legacy citations become `mentions` (ADR-0010, ADR-0016, ADR-0018).
- MVP: tree-sitter + markers; rust-analyzer in Phase 3 (ADR-0020, ADR-0021).
- UI: React 19 + Vite + `@xyflow/react`, English interface (ADR-0011, ADR-0014).
- The documentation convention is mandatory here and in every project under SpecEngine (ADR-0022).
- All repository content is in English (ADR-0024).

<a id="process"></a>
## Process: all development goes through roles

Code and documentation change **only through the pipeline** `/feature <requirement>`. The main session is an orchestrator: it hands stages to roles, passes results along, sums up, and writes no code, tests or specs itself. Exception: a direct instruction from the owner in the current session. ADR-0023, ADR-0029.

| Role | Does | Writes only to |
|---|---|---|
| `requirement-analyst` | requirement analysis, assumptions, questions, criteria | — (nothing) |
| `spec-writer` | task specs, ADR + canon diff, index | `docs/`, `CLAUDE.md`, `*/README.md`, `README.md`, `specengine.toml`, `.spec-debt.toml` |
| `rust-developer` | Rust code | `Cargo.toml`, `crates/*/src`, `plugin/`, `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/` |
| `ui-developer` | web UI (Phase 4+) | `ui/**`, except generated types |
| `test-engineer` | tests and running checks | `crates/*/tests`, `fixtures/`, `#[cfg(test)]` |
| `code-reviewer` | review against the spec and the rules | — (no Write/Edit) |

The developer must not touch `docs/`: a discrepancy between spec and code must be visible, not smoothed over. At most three implementation iterations; if they run out, the task is not marked implemented. Roles do not commit: the owner commits. `.claude/**` is the owner's: a role puts its text in the spec; the owner applies it. Role models live in front-matter (Opus 5.5).

## Documentation

Classes: canon, decision, spec, generated; tiers: `docs/README.md`. Caps: Tier 0 16 KB, Tier 1 and index 10 KB, Tier 2 canon 12 KB, ADR 1.5 KB. An accepted decision changes the canon in the same change; its `canon:` points at the section. `docs/index*.md` are generated only. Before handing in any documentation change:

```bash
cargo run -q -p specengine-cli -- export index && cargo run -q -p specengine-cli -- check
```

The pre-commit hook and CI reject a red check (`docs/README.md` "Enforcement").

## Layout

- `docs/` — `canon/`, `decisions/`, specs (`features/`, `specs/`), index, archive.
- `crates/specengine-{model,core,store,cli,code,eval,import,mcp,ra}` — the corpus model and reference grammar, the spec parser and check, the spec index (SQLite + FTS5), the `spec` binary, layer A parsing and hashing, the measurement harness, the corpus census, the stdio MCP server, layer C (outside `default-members`); each has a Tier 1 `README.md`.
- `fixtures/` — test corpora with `expected.json`; `bevy-mini` and `ra-mini` are workspace-excluded.
- `.claude/` — pipeline roles and commands.
- Planned: `crates/specengine-http`, `ui/`, `plugin/`.

## Owner's machine

A macOS laptop, not a build server. Build directory: `target.noindex` (`.cargo/config.toml`); do not create a second one. Tests: `cargo nextest run -p <crate> --test <file> <filter>`; the full run once at the end. Stress load only in self-terminating form `perl -e 'alarm 120; exec "yes"'`; never leave background processes without a timeout. Consumer projects used to exercise SpecEngine are read-only.
