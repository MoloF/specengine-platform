---
class: canon
tier: 2
scope: [root]
owner: owner
reviewed: 2026-09-28
---

# SpecEngine

A local specification engine for projects built together with AI agents.

As a project grows, its documentation grows with it. An agent spends more and more context gathering what it needs, makes more mistakes, and relies on documents that have already drifted from the code. SpecEngine keeps the context cost of a task constant and makes drift visible:

- **business-logic tree and atomic records** — requirements, assumptions, questions, decisions, criteria — live in git next to the code, one file per record;
- **code bindings** — `// @implements ID@rev` markers and normalized AST hashes of symbols show where spec and code have diverged;
- **task context bundle** — the minimal set of nodes, signatures and open questions within a token budget, instead of reading the whole corpus;
- **owner queue** — an agent reports a discrepancy with evidence and options and keeps working on a working answer; nothing is blocked, and only tasks approved by the owner go into development;
- **interfaces** — CLI, an MCP server for Claude Code agents, later a web UI.

Rust comes first (including Bevy ECS and RON data); other languages via tree-sitter grammars.

## Status

**Phase 0.** Architectural decisions are made; the engine code is not written yet. Only the documentation-convention enforcement (`xtask`) works today.

## Where to start

| What | Where |
|---|---|
| Rules for humans and agents | [`CLAUDE.md`](CLAUDE.md) |
| Documentation index | [`docs/index.md`](docs/index.md) |
| Architecture rules | [`docs/canon/architecture.md`](docs/canon/architecture.md) |
| Decisions | [`docs/decisions/`](docs/decisions/) |
| Design and plan | [`docs/specs/specengine-platform/`](docs/specs/specengine-platform/README.md) |
| Documentation convention | [`docs/canon/documentation-system.md`](docs/canon/documentation-system.md), applied in [`docs/README.md`](docs/README.md) |

## Development

All development goes through the Claude Code role pipeline (`.claude/agents/`, commands `/feature` and `/feature-saving`): analyst → spec writer → developer ⇄ reviewer and test engineer → documentation update.

```bash
./scripts/hooks-install.sh                                  # pre-commit: documentation check
cargo xtask docs index --write && cargo xtask docs check    # before handing in any documentation change
cargo xtask docs budget                                     # document sizes and the working set W
```

Requires Rust ≥ 1.90 and `cargo-nextest`.

## License

[MIT](LICENSE), copyright (c) 2026 MoloF. Free to use, modify and distribute; copies must keep the copyright notice and the license text (ADR-0025).
