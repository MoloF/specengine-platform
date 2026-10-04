---
class: canon
tier: 2
scope: [root]
owner: owner
reviewed: 2026-10-04
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

**Phase 0 done** (2026-09-29): architectural decisions are made and the engine's key claims are measured on two pilot corpora. **Phase 1**, the reading core, in progress: the spec parser (`specengine-model`, `specengine-core`), the SQLite index (`specengine-store`), `spec check` and the `spec` CLI (`specengine-cli`: `init`, `index`, `search`, `show`, `tree`, `graph`, `bundle`, `check`, `export index`) and the stdio MCP server's read tools and resources (`specengine-mcp`) are shipped; `spec check` is this repository's documentation gate. Also built: layer A parsing and hashing (`specengine-code`), a corpus census, importer and layout round trip (`specengine-import`), rust-analyzer loading (`specengine-ra`) and the measurement harness (`specengine-eval`). Next: token calibration, then the pilot projects ([`08-roadmap`](docs/specs/specengine-platform/08-roadmap.md)).

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

All development goes through the Claude Code role pipeline (`.claude/agents/`, command `/feature`): analyst → spec writer → developer ⇄ reviewer and test engineer → documentation update.

```bash
./scripts/hooks-install.sh                        # pre-commit: documentation check
cargo run -q -p specengine-cli -- export index    # regenerate docs/index.md
cargo run -q -p specengine-cli -- check           # the documentation check; its summary shows the worst W
```

Requires Rust ≥ 1.90 (≥ 1.98 for `specengine-eval --features ra`) and `cargo-nextest`.

## License

[MIT](LICENSE), copyright (c) 2026 MoloF. Free to use, modify and distribute; copies must keep the copyright notice and the license text (ADR-0025).
