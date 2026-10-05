---
class: generated
generator: cargo run -q -p specengine-cli -- export index
source: front-matter of the repository's documents
---

# Documentation index: `docs/decisions/*.md`

<!-- Built by `cargo run -q -p specengine-cli -- export index`. Manual edits are overwritten on rebuild, and `cargo run -q -p specengine-cli -- check` rejects them. -->

A shard of [docs/index.md](index.md), the index's one entry point.

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
- [ADR-0027](decisions/ADR-0027.md) Tasks are stack-neutral packages; skills render them · architecture · accepted
- [ADR-0028](decisions/ADR-0028.md) Tier 3 index lines carry only id and status · docs · accepted
- [ADR-0029](decisions/ADR-0029.md) One pipeline; role write areas for the gate · process · accepted
- [ADR-0030](decisions/ADR-0030.md) Configured index shards · docs · accepted
- [ADR-0031](decisions/ADR-0031.md) Kinds and process rules are project config · architecture · accepted
- [ADR-0032](decisions/ADR-0032.md) A proposal applies where it was raised · architecture · accepted
- [ADR-0033](decisions/ADR-0033.md) UI starts before the daemon · ui · accepted
