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
- [docs/features/bevy-roles.md](features/bevy-roles.md) bevy-roles: framework roles of Rust items in the code index, opt-in [code] frameworks · crates/specengine-code, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, fixtures · draft
- [docs/features/bundle-bindings.md](features/bundle-bindings.md) bundle-bindings: an agent's reads carry its nodes' code bindings and sync · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, ui, fixtures · draft
- [docs/features/code-tools.md](features/code-tools.md) code-tools: MCP find_symbols, refs, unmapped, check_binding; spec verify --node · crates/specengine-mcp, crates/specengine-cli, plugin, fixtures · draft
- [docs/features/constant-census.md](features/constant-census.md) constant-census: which string constants carry a record's code (slice 12a) · crates/specengine-eval, crates/specengine-code, fixtures · draft
- [docs/features/drift-queue.md](features/drift-queue.md) drift-queue: drift reaches the owner's queue; approving it runs the lock accept · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures · draft
- [docs/features/hash-levels.md](features/hash-levels.md) hash-levels: per-level hashes and fingerprints of a bound item · crates/specengine-code, crates/specengine-eval, fixtures · draft
- [docs/features/health-code.md](features/health-code.md) health-code: Health shows code assuming a decision and nodes no code binds · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, fixtures · draft
- [docs/features/health-drift.md](features/health-drift.md) health-drift: Health shows drift between spec.lock and the tree · crates/specengine-cli, crates/specengine-http, ui, fixtures · draft
- [docs/features/impact.md](features/impact.md) impact: the blast radius of a node, a symbol or a revision · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, fixtures · draft
- [docs/features/layer-c.md](features/layer-c.md) layer-c: rust-analyzer in its own executable, monikers as advisory index data · crates/specengine-ra, crates/specengine-cli, crates/specengine-store, crates/specengine-code, crates/specengine-core, crates/specengine-eval · draft
- [docs/features/lock-levels.md](features/lock-levels.md) lock-levels: spec.lock format 2, level-aware verify, spec lock rebase · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-code, fixtures · draft
- [docs/features/rev-discipline.md](features/rev-discipline.md) rev-discipline: a changed meaning raises the revision, or the owner calls it editorial · crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures · draft
- [docs/features/roadmap.md](features/roadmap.md) Roadmap and backlog (planned) · specengine · draft
- [docs/features/spec-verify.md](features/spec-verify.md) spec-verify: drift between spec.lock and the tree · crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures · draft
- [docs/features/symbol-index.md](features/symbol-index.md) symbol-index: stored code rows, symbol lookup, coverage · crates/specengine-store, crates/specengine-code, crates/specengine-cli, crates/specengine-eval, fixtures · draft
- [docs/features/verify-tests.md](features/verify-tests.md) verify-tests: bound tests run by the project's own command · crates/specengine-core, crates/specengine-store, crates/specengine-cli, plugin, fixtures · draft
- [docs/specs/specengine-platform/04-prior-art-and-stack.md](specs/specengine-platform/04-prior-art-and-stack.md) 04. Prior art, MCP, Claude Code, the stack · specengine · in-progress
- [docs/specs/specengine-platform/05-architecture.md](specs/specengine-platform/05-architecture.md) 05. Target architecture of SpecEngine · specengine · in-progress
- [docs/specs/specengine-platform/06-workflows.md](specs/specengine-platform/06-workflows.md) 06. Workflows · specengine · in-progress
- [docs/specs/specengine-platform/07-interfaces.md](specs/specengine-platform/07-interfaces.md) 07. Interfaces: MCP, CLI, HTTP, hooks, config · specengine · in-progress
- [docs/specs/specengine-platform/08-roadmap.md](specs/specengine-platform/08-roadmap.md) 08. Roadmap: phases, import, criteria, risks · specengine · in-progress
- [docs/specs/specengine-platform/README.md](specs/specengine-platform/README.md) SpecEngine — research and platform concept · specengine · in-progress

## Shards

- [docs/index-archive.md](index-archive.md) Archive — Tier 3, by id only
- [docs/index-canon.md](index-canon.md) `docs/canon/*.md`
- [docs/index-decisions.md](index-decisions.md) `docs/decisions/*.md`
- [docs/index-crates.md](index-crates.md) `crates/*/README.md`
