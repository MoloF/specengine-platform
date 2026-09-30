---
class: generated
generator: cargo xtask docs index --write
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `cargo xtask docs index --write`. Manual edits are overwritten on rebuild, and `cargo xtask docs check` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.

## Canon

- [CLAUDE.md](../CLAUDE.md) SpecEngine · root · tier 0
- [README.md](../README.md) SpecEngine · root · tier 2
- [crates/specengine-cli/README.md](../crates/specengine-cli/README.md) specengine-cli — the spec binary · crates/specengine-cli · tier 1
- [crates/specengine-code/README.md](../crates/specengine-code/README.md) specengine-code — layer A: Rust and RON parsing, AST hash, markers, Bevy detector · crates/specengine-code · tier 1
- [crates/specengine-core/README.md](../crates/specengine-core/README.md) specengine-core — the spec parser and the check · crates/specengine-core · tier 1
- [crates/specengine-eval/README.md](../crates/specengine-eval/README.md) specengine-eval — the permanent measurement harness · crates/specengine-eval · tier 1
- [crates/specengine-import/README.md](../crates/specengine-import/README.md) specengine-import — importers of existing spec corpora · crates/specengine-import · tier 1
- [crates/specengine-mcp/README.md](../crates/specengine-mcp/README.md) specengine-mcp — the MCP server over stdio · crates/specengine-mcp · tier 1
- [crates/specengine-model/README.md](../crates/specengine-model/README.md) specengine-model — the corpus model and the reference grammar · crates/specengine-model · tier 1
- [crates/specengine-ra/README.md](../crates/specengine-ra/README.md) specengine-ra — layer C: rust-analyzer as a library · crates/specengine-ra · tier 1
- [crates/specengine-store/README.md](../crates/specengine-store/README.md) specengine-store — the spec index · crates/specengine-store · tier 1
- [docs/README.md](README.md) Documentation: how the convention is applied · docs · tier 1
- [docs/canon/architecture.md](canon/architecture.md) SpecEngine architecture rules · architecture · tier 2
- [docs/canon/documentation-system.md](canon/documentation-system.md) Documentation System: Constant Cost at Corpus Growth · docs · tier 2
- [docs/canon/spec-check-cli.md](canon/spec-check-cli.md) The spec CLI: check and export index · crates/specengine-cli, crates/specengine-store · tier 2
- [docs/canon/spec-check-graph.md](canon/spec-check-graph.md) spec check: index render, generators, graph warnings · crates/specengine-core, xtask · tier 2
- [docs/canon/spec-check-links.md](canon/spec-check-links.md) spec check: feature scopes and links · crates/specengine-core, crates/specengine-model · tier 2
- [docs/canon/spec-check.md](canon/spec-check.md) spec check: what the documentation check enforces · crates/specengine-core, crates/specengine-store, crates/specengine-eval, xtask · tier 2
- [xtask/README.md](../xtask/README.md) xtask — enforcement of the documentation convention · xtask · tier 1

## Decisions

- [ADR-0001](decisions/ADR-0001.md) Source of truth for specs is files in the project's git; SQLite is index and queue · storage · accepted
- [ADR-0003](decisions/ADR-0003.md) Operational database outside the repo, queue in git as a generated file · storage · accepted
- [ADR-0004](decisions/ADR-0004.md) An approved proposal is applied in the task branch's worktree · process · accepted
- [ADR-0005](decisions/ADR-0005.md) Applying is a separate commit with provenance · process · accepted
- [ADR-0006](decisions/ADR-0006.md) The gate is closed when the daemon is unavailable · process · accepted
- [ADR-0007](decisions/ADR-0007.md) EARS is a hint, not a requirement · docs · accepted
- [ADR-0008](decisions/ADR-0008.md) The core is universal; domain specifics live in the project config · architecture · accepted
- [ADR-0009](decisions/ADR-0009.md) IDs are Latin only; legacy IDs are aliases · ids · accepted
- [ADR-0010](decisions/ADR-0010.md) Code marker is an @implements comment · code · accepted
- [ADR-0011](decisions/ADR-0011.md) UI is React 19 + Vite + @xyflow/react · ui · accepted
- [ADR-0012](decisions/ADR-0012.md) Discrepancies block nothing; control is task approval · process · accepted
- [ADR-0013](decisions/ADR-0013.md) Project generators stay in the project; docs checks move to spec check · docs · accepted
- [ADR-0014](decisions/ADR-0014.md) The interface is in English · ui · accepted
- [ADR-0015](decisions/ADR-0015.md) Distribution: cargo install + Claude Code plugin · architecture · accepted
- [ADR-0016](decisions/ADR-0016.md) Legacy ID citations in code become weak mentions links · code · accepted
- [ADR-0017](decisions/ADR-0017.md) Single-user mode · architecture · accepted
- [ADR-0018](decisions/ADR-0018.md) Hybrid: revision in the marker + spec.lock for AST · code · accepted
- [ADR-0019](decisions/ADR-0019.md) Own implementation with tracey's topology and discipline · architecture · accepted
- [ADR-0020](decisions/ADR-0020.md) MVP: tree-sitter + markers; rust-analyzer as a precision layer in Phase 3 · code · accepted
- [ADR-0021](decisions/ADR-0021.md) Digest is a normalized tree-sitter AST · code · accepted
- [ADR-0022](decisions/ADR-0022.md) The documentation convention is mandatory here and in every project · docs · accepted
- [ADR-0023](decisions/ADR-0023.md) Development only through the /feature role pipeline · process · accepted
- [ADR-0024](decisions/ADR-0024.md) All repository content is in English · docs · accepted
- [ADR-0025](decisions/ADR-0025.md) The repository is licensed under MIT · root · accepted
- [ADR-0026](decisions/ADR-0026.md) Feature-scoped IDs are {#ID} sections of their feature document · storage · accepted

## Specs

- [docs/features/roadmap.md](features/roadmap.md) Roadmap and backlog (planned) · specengine · draft
- [docs/specs/specengine-platform/04-prior-art-and-stack.md](specs/specengine-platform/04-prior-art-and-stack.md) 04. Prior art, the MCP protocol, Claude Code, the stack · specengine · in-progress
- [docs/specs/specengine-platform/05-architecture.md](specs/specengine-platform/05-architecture.md) 05. Target architecture of SpecEngine · specengine · in-progress
- [docs/specs/specengine-platform/06-workflows.md](specs/specengine-platform/06-workflows.md) 06. Workflows · specengine · in-progress
- [docs/specs/specengine-platform/07-interfaces.md](specs/specengine-platform/07-interfaces.md) 07. Interfaces: MCP, CLI, HTTP, hooks, config · specengine · in-progress
- [docs/specs/specengine-platform/08-roadmap.md](specs/specengine-platform/08-roadmap.md) 08. Implementation plan, importing existing corpora, acceptance criteria, risks · specengine · in-progress
- [docs/specs/specengine-platform/README.md](specs/specengine-platform/README.md) SpecEngine — research and platform concept · specengine · in-progress

## Archive — Tier 3, by id only

- [docs/archive/initial-architecture-spec.md](archive/initial-architecture-spec.md) Technical Specification: SpecEngine Platform · specengine · abandoned
- [ADR-0002](decisions/ADR-0002.md) A record is a file, a node document is a file, rules are {#ID} sections · storage · superseded-by ADR-0026
- [docs/features/phase-0-spikes.md](features/phase-0-spikes.md) Phase 0 spikes: turn the engine's unverified claims into numbers on the two pilot corpora · spikes · shipped
- [docs/features/phase1-cleanup.md](features/phase1-cleanup.md) Phase 1 cleanup: the open minors of the parser, index and check · crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-eval · shipped
- [docs/features/spec-check-graph.md](features/spec-check-graph.md) spec check, increment 2 part 1: index, generators, graph warnings · crates/specengine-core, crates/specengine-store, crates/specengine-eval · shipped
- [docs/features/spec-check-links.md](features/spec-check-links.md) spec check, pass B: file links · crates/specengine-core, crates/specengine-store · shipped
- [docs/features/spec-check-scopes.md](features/spec-check-scopes.md) spec check, increment 2 part 2, pass A: feature scopes · crates/specengine-core, crates/specengine-model, crates/specengine-store · shipped
- [docs/features/spec-check.md](features/spec-check.md) spec check, increment 1: a config-driven check engine at xtask parity · crates/specengine-core, crates/specengine-model, crates/specengine-store, crates/specengine-eval, xtask · shipped
- [docs/features/spec-cli-check.md](features/spec-cli-check.md) The spec CLI, pass 2a.1: check and export index · crates/specengine-cli, crates/specengine-core, crates/specengine-store, crates/specengine-eval · shipped
- [docs/features/spec-cli.md](features/spec-cli.md) The spec CLI, pass 1: the agent read loop · crates/specengine-cli, crates/specengine-core, crates/specengine-store · shipped
- [docs/features/spec-index.md](features/spec-index.md) Spec index: SQLite + FTS5, incremental · crates/specengine-store · shipped
- [docs/features/spec-parser.md](features/spec-parser.md) Spec parser: the corpus model and the single reference grammar · crates/specengine-model, crates/specengine-core, crates/specengine-eval · shipped
- [docs/specs/specengine-platform/03-critique-of-initial-spec.md](specs/specengine-platform/03-critique-of-initial-spec.md) 03. Critique of the initial specification · specengine · shipped
