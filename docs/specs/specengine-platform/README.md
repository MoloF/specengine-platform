---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
adrs: [ADR-0001, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0008, ADR-0009, ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0014, ADR-0015, ADR-0016, ADR-0017, ADR-0018, ADR-0019, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, ADR-0025, ADR-0026, ADR-0027]
---

# SpecEngine — research and platform concept

> Compiled 2026-09-28 from the initial platform spec, an analysis of the pilot projects' existing documentation corpora, and external research (tools, MCP 2026-07-28, Claude Code 2.1.x, crates.io).

## Why

1. **Task context grows with corpus size.** The more documents there are, the more an agent or a human reads before the first line of code; without enforcement the working set of a single task balloons several-fold. SpecEngine assembles the context bundle by tree and links, not by folders, and keeps its size constant.
2. **Documentation drifts from code.** A document can lie about the code for weeks: there is no binding to symbols and no signal on change. `@implements ID@rev` markers, `spec.lock` and AST hashes turn drift into a visible, addressable discrepancy.
3. **Agents hallucinate and re-ask.** The same question is asked again, the answer is lost in chat, "green" is reported before the run. A proposal queue, question deduplication, decisions with `cost`, and test runs by the engine itself turn this into the project's machine memory.

## In short

1. **Half of SpecEngine already exists** in the pilot projects' corpora: the documentation convention (classes, tiers, budgets, promotion rule — now `docs/canon/documentation-system.md`), "documents as data", "single write door", the context bundle, xtask checks. They are ported and finished, not reinvented.
2. **The initial spec was turned around** from a multi-user SaaS into a local engine for one developer with agents: specs in git next to the code, three status axes, a change proposal as a separate entity, plus tasks, links, questions, decisions and a glossary. CRDT and Docker leave the MVP (`03`).
3. **There is no ready product; the closest analogue in Rust is tracey**: the same "daemon + CLI/MCP/LSP bridges" topology and requirement versions in code markers (pre-commit fails if meaning changed without a version bump), but no tree, node types, code drift or reconciliation. fiberplane/drift and amiss confirm lock file + AST fingerprint; OpenFastTrace and Doorstop confirm coverage invalidation by revision. Nobody does spec graph + bindings + reconciliation queue (`04`).
4. **The main scenario** is discrepancy → reconciliation → development: the agent calls `report_discrepancy` with evidence and costed options and **keeps working** on the working answer; discrepancies block nothing, the owner processes the queue in the CLI, the UI or a Claude Code form. Only a task approved by the owner goes into development (`06` §3, ADR-0012).
5. **MVP = CLI + MCP in ~5-6 weeks**, full scope in ~11-14 weeks (`08`). The core knows no genres; pilot projects (at least two of different nature) are connected from the first phase, each with its own tree and `specengine.toml` (ADR-0008).
6. **Code is indexed in three layers** (`05` §5.1): (A) tree-sitter + markers + normalized AST hash `path/sig/body/deps`, always and in fractions of a second; (B) for Bevy projects, the `schedule_data` schedule dump as the truth about systems; (C) rust-analyzer as a library, only if needed. MCP runs over stdio: on stateless HTTP Claude Code gets 405. No vector search (`04` §1.7, §2.1).
7. **Decisions are made**: ADR-0001…ADR-0027 in `docs/decisions/`, each with its canon diff. Phase 0 is done (2026-09-29, `docs/features/phase-0-spikes.md`); next is Phase 1 (`08` §2).

## Reading order

| Document | About | When to read |
|---|---|---|
| [03-critique-of-initial-spec](03-critique-of-initial-spec.md) | what to keep, change and add in the initial spec | before the architecture |
| [04-prior-art-and-stack](04-prior-art-and-stack.md) | analogues, MCP 2026-07-28, Claude Code capabilities, crate versions | when choosing technologies |
| [05-architecture](05-architecture.md) | topology, file layout, data model, lock, drift, context bundle | **the main document for implementation** |
| [06-workflows](06-workflows.md) | owner and agent scenarios, `/feature` | to validate the process |
| [07-interfaces](07-interfaces.md) | MCP tools, CLI, HTTP, hooks, `specengine.toml` | when implementing adapters |
| [08-roadmap](08-roadmap.md) | phases, acceptance criteria, importing existing corpora, risks | for planning |

Decisions — `docs/decisions/ADR-NNNN.md` (≤ 1.5 KB, with a cost and a `canon:` pointer to the section). The rules that must not be broken — `docs/canon/architecture.md`; the documentation convention — `docs/canon/documentation-system.md`.
