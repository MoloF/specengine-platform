---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.

## Canon

- [CLAUDE.md](../CLAUDE.md) SpecEngine · root · tier 0
- [README.md](../README.md) SpecEngine · root · tier 2
- [docs/README.md](README.md) Documentation: how the convention is applied · docs · tier 1
- [ui/README.md](../ui/README.md) ui -- the web UI · ui · tier 1

## Specs

- [docs/features/bevy-dump.md](features/bevy-dump.md) bevy-dump: the project's schedule dump annotates symbols (layer B) · crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-eval, crates/specengine-mcp, fixtures · draft
- [docs/features/bundle-bindings.md](features/bundle-bindings.md) bundle-bindings: an agent's reads carry its nodes' code bindings and sync · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, ui, fixtures · draft
- [docs/features/health-drift.md](features/health-drift.md) health-drift: Health shows drift between spec.lock and the tree · crates/specengine-cli, crates/specengine-http, ui, fixtures · draft
- [docs/features/impact.md](features/impact.md) impact: the blast radius of a node, a symbol or a revision · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, fixtures · draft
- [docs/features/layer-c-reads.md](features/layer-c-reads.md) layer-c-reads (11a-3): monikers in spec symbols and find_symbols, the next plugin MINOR · crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-mcp, plugin · draft
- [docs/features/roadmap-position.md](features/roadmap-position.md) roadmap-position (Phase 6, 2): live progress, next by the plan, MCP and HTTP reads · crates/specengine-core, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, plugin · draft
- [docs/features/roadmap-screen.md](features/roadmap-screen.md) roadmap-screen (Phase 6, 3a): the Roadmap screen, read-only · ui · draft
- [docs/features/tested-evidence.md](features/tested-evidence.md) tested-evidence: a passing test run becomes self-staling evidence in spec.lock · crates/specengine-core, crates/specengine-cli, fixtures · draft
- [docs/features/verify-node.md](features/verify-node.md) verify-node (6b-2): spec verify --node, MCP check_binding · crates/specengine-cli, crates/specengine-mcp, plugin · draft
- [docs/specs/specengine-platform/04-prior-art-and-stack.md](specs/specengine-platform/04-prior-art-and-stack.md) 04. Prior art, MCP, Claude Code, the stack · specengine · in-progress
- [docs/specs/specengine-platform/05-architecture.md](specs/specengine-platform/05-architecture.md) 05. Target architecture of SpecEngine · specengine · in-progress
- [docs/specs/specengine-platform/06-workflows.md](specs/specengine-platform/06-workflows.md) 06. Workflows · specengine · in-progress
- [docs/specs/specengine-platform/07-interfaces.md](specs/specengine-platform/07-interfaces.md) 07. Interfaces: MCP, CLI, HTTP, hooks, config · specengine · in-progress
- [docs/specs/specengine-platform/08-roadmap.md](specs/specengine-platform/08-roadmap.md) 08. Roadmap: phases, import, criteria, risks · specengine · in-progress
- [docs/specs/specengine-platform/README.md](specs/specengine-platform/README.md) SpecEngine — research and platform concept · specengine · in-progress

## Shards

- [docs/index-archive.md](index-archive.md) Archive — Tier 3, by id only
- [docs/index-canon-spec.md](index-canon-spec.md) `docs/canon/spec-*.md`
- [docs/index-canon.md](index-canon.md) `docs/canon/*.md`
- [docs/index-decisions.md](index-decisions.md) `docs/decisions/*.md`
- [docs/index-crates.md](index-crates.md) `crates/*/README.md`
- [docs/index-roadmap.md](index-roadmap.md) `docs/roadmap/*.md`
