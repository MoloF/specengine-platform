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
- [docs/canon/agent-intake.md](canon/agent-intake.md) Agent intake: queue tools, questions, discrepancies · crates/specengine-mcp, crates/specengine-cli, crates/specengine-store, crates/specengine-core · tier 2
- [docs/canon/architecture.md](canon/architecture.md) SpecEngine architecture rules · architecture · tier 2
- [docs/canon/code-identity.md](canon/code-identity.md) Layer A identity: target units, markers, RON paths · crates/specengine-code, crates/specengine-eval · tier 2
- [docs/canon/decision-record.md](canon/decision-record.md) Decision records: a question or discrepancy approved · crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-mcp · tier 2
- [docs/canon/decision-staging.md](canon/decision-staging.md) Decision staging: UI prepares, terminal confirms · crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui · tier 2
- [docs/canon/documentation-system.md](canon/documentation-system.md) Documentation System: Constant Cost at Corpus Growth · docs · tier 2
- [docs/canon/gate.md](canon/gate.md) Task gate: the write hook · crates/specengine-cli, plugin · tier 2
- [docs/canon/import-layout-verifier.md](canon/import-layout-verifier.md) Import layout: verifier, attribution, output · crates/specengine-eval · tier 2
- [docs/canon/import-layout.md](canon/import-layout.md) Import layout: the emitter and the after-tree · crates/specengine-import, crates/specengine-eval · tier 2
- [docs/canon/import.md](canon/import.md) Import: record model and rules · crates/specengine-import · tier 2
- [docs/canon/mcp-read.md](canon/mcp-read.md) MCP read tools and resources · crates/specengine-mcp, crates/specengine-cli · tier 2
- [docs/canon/proposal-apply.md](canon/proposal-apply.md) Proposal apply: consent, steps, completion, reject · crates/specengine-cli, crates/specengine-store · tier 2
- [docs/canon/proposal-kinds.md](canon/proposal-kinds.md) Proposal kinds: create · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin · tier 2
- [docs/canon/proposal-queue.md](canon/proposal-queue.md) Proposal queue: commands, states, store · crates/specengine-cli, crates/specengine-store · tier 2
- [docs/canon/queue-backup.md](canon/queue-backup.md) Queue backup and restore · crates/specengine-cli, crates/specengine-store · tier 2
- [docs/canon/spec-check-cli.md](canon/spec-check-cli.md) spec check and export index · crates/specengine-cli · tier 2
- [docs/canon/spec-check-git.md](canon/spec-check-git.md) spec check against HEAD · crates/specengine-store, crates/specengine-cli · tier 2
- [docs/canon/spec-check-graph.md](canon/spec-check-graph.md) spec check: index render, generators, graph warnings · crates/specengine-core · tier 2
- [docs/canon/spec-check-links.md](canon/spec-check-links.md) spec check: feature scopes and links · crates/specengine-core, crates/specengine-model · tier 2
- [docs/canon/spec-check-process.md](canon/spec-check-process.md) spec check: process rules · crates/specengine-core · tier 2
- [docs/canon/spec-check.md](canon/spec-check.md) spec check: what it enforces · crates/specengine-core, crates/specengine-store, crates/specengine-eval · tier 2
- [docs/canon/spec-cli-bundle.md](canon/spec-cli-bundle.md) spec bundle and bundle_hash · crates/specengine-cli, crates/specengine-core, crates/specengine-store · tier 2
- [docs/canon/spec-cli-graph.md](canon/spec-cli-graph.md) spec tree, spec graph, show --links · crates/specengine-cli, crates/specengine-core, crates/specengine-model · tier 2
- [docs/canon/task-package.md](canon/task-package.md) Task package: what an agent is given · crates/specengine-cli, crates/specengine-mcp, crates/specengine-model · tier 2
- [docs/canon/tasks.md](canon/tasks.md) Tasks: commands, transitions, place, store · crates/specengine-store, crates/specengine-cli, crates/specengine-core · tier 2
- [docs/canon/w-measurement.md](canon/w-measurement.md) W measurement: every task, before and after · crates/specengine-eval · tier 2
- [ui/README.md](../ui/README.md) ui -- the web UI · ui · tier 1

## Specs

- [docs/features/plugin-gate.md](features/plugin-gate.md) Plugin gate · plugin, crates/specengine-cli · draft
- [docs/features/roadmap.md](features/roadmap.md) Roadmap and backlog (planned) · specengine · draft
- [docs/specs/specengine-platform/04-prior-art-and-stack.md](specs/specengine-platform/04-prior-art-and-stack.md) 04. Prior art, MCP, Claude Code, the stack · specengine · in-progress
- [docs/specs/specengine-platform/05-architecture.md](specs/specengine-platform/05-architecture.md) 05. Target architecture of SpecEngine · specengine · in-progress
- [docs/specs/specengine-platform/06-workflows.md](specs/specengine-platform/06-workflows.md) 06. Workflows · specengine · in-progress
- [docs/specs/specengine-platform/07-interfaces.md](specs/specengine-platform/07-interfaces.md) 07. Interfaces: MCP, CLI, HTTP, hooks, config · specengine · in-progress
- [docs/specs/specengine-platform/08-roadmap.md](specs/specengine-platform/08-roadmap.md) 08. Roadmap: phases, import, criteria, risks · specengine · in-progress
- [docs/specs/specengine-platform/README.md](specs/specengine-platform/README.md) SpecEngine — research and platform concept · specengine · in-progress

## Shards

- [docs/index-archive.md](index-archive.md) Archive — Tier 3, by id only
- [docs/index-decisions.md](index-decisions.md) `docs/decisions/*.md`
- [docs/index-crates.md](index-crates.md) `crates/*/README.md`
