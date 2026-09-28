---
class: spec
status: shipped
shipped: 2026-09-28
scope: [specengine]
ref: research-2026-09-28
---

# 03. Critique of the initial specification

> A review of `system_architecture_specification_specengine_platform.md` from the standpoint of real consumer projects.
> Verdict: the idea is right, but the document designs a **multi-user SaaS**. What is needed is a **local engine for one
> developer working with agents**, centred on an approval queue rather than cursor synchronisation.

## 1. What to keep unchanged

- Separation of **WHAT/WHY** (spec) from **HOW/WHERE** (code); spec text contains no file paths.
- Atomic nodes instead of monolithic documents.
- Binding to **AST symbols**, not lines or files; hash of the canonical subtree without comments and whitespace.
- 3 sub-agent phases: Discovery → Planning → Execution + Binding. The "historical memory" of bindings lets discovery be skipped.
- Tree resource **without node bodies** (token economy), `get_context_bundle`, proposing an edit instead of writing directly.
- Drift matrix (spec changed / code changed).
- Requirement: edge-case tests before the "implemented" status.

## 2. What to change, by importance

### 2.1. Source of truth: git, not the DB  ⟵ the main decision

In the original the DB is the truth and `.spec/` in git is an optional mirror ("Model B"). Real projects need the opposite:

| Argument | Fact |
|---|---|
| Already adopted by consumers | principle "no second store for documents, no databases of our own" |
| The spec must travel with the code | agents work in **git worktrees on branches** (`.claude/worktrees/…`); a spec in the DB is shared by all, code is per branch. Drift "on branch X" cannot be expressed in the DB model |
| Promotion rule | decision and canon diff go **in one commit** with the code (documentation convention §5) |
| Review | spec changes are visible in `git diff`/PR next to the code |
| CI and hooks | the pre-commit hook and GitHub Actions already run the docs checks |
| Resilience | an agent can read specs without a running service; history is free (`node_revisions` = `git log`) |

**Decision:** spec files live in the project repository (Markdown + YAML front-matter). SpecEngine is an **indexer + server + CLI + MCP + UI**. SQLite is a rebuildable index plus **operational state** (proposal queue, tasks, agent runs). The outcome of every proposal always lands in git: as a spec diff and as a decision record (ADR-0001).

### 2.2. Status is overloaded → three independent axes

One `status` field mixes text approval (`Draft/Pending/Approved/Rejected`), implementation (`Implemented`) and synchronisation (`Verified/Drift`). A node can be "accepted v3" and "code matches v2" at the same time. Also, `Conflict` from the matrix in §5.2 and `Failed` from `report_implementation_status` are missing from the DDL CHECK constraint.

| Axis | Values | Stored / computed |
|---|---|---|
| `spec_status` | draft → review → accepted → superseded / rejected | stored (front-matter) |
| `impl_status` | none → planned → in_progress → implemented → verified | stored (lock file / DB) |
| `sync` | ok · spec_ahead · code_ahead · conflict · unbound · broken_binding | **computed** from hashes, never written by hand |
| `acceptance` (features) | pending → accepted | stored (front-matter) |

### 2.3. `propose_node_update` must not touch the node

In the original a proposal overwrites `content` and puts the node into `Pending_User_Approval`. The accepted version disappears, and agents and the owner start reading unapproved text. A separate **Proposal** entity is needed:

- the node stays `accepted` with its own text while the proposal is open;
- the proposal stores `base_hash`, the hash of the version it was made from. If the node changed, the proposal must be rebased;
- while the proposal is not applied the node carries the `has_open_proposal` flag, and the context bundle shows it;
- only `apply_proposal`, triggered by a human action, writes to spec files: the "one door" (ADR-0004).

### 2.4. Missing entities that a real corpus lives on

The original has only `SpecNode` and `CodeBinding`; a task is "backlog = Approved nodes", the only link is parent-child. In real corpora the most numerous objects are different, and none of them exist in the original:

- customer requirement (immutable + revision chain);
- assumption (a decision in place of an answer, with "why");
- question to the owner (+ working answer, blocking or not, cost of a different answer);
- decision (with cost, `canon:`);
- acceptance criterion (kind, named mutation);
- principle / check code;
- glossary term (term → identifier in code);
- development **task**;
- **links** between nodes (`ref:`, `adrs:`, `depends_on`, `amends`, `[[…]]`).

Data model: `05-architecture.md` §3.

### 2.5. Node version: revision counter and a hash pair instead of semver

Semver for a node has no owner: nobody knows who bumps the minor version and by what rule. More important, the drift matrix is undefined. "The spec hash changed" — relative to what? We must store **which pair of hashes the code was verified against**:

```
binding: node_id, symbol, verified_spec_hash, verified_ast_hash, verified_at_commit
sync = f(current_spec_hash ≠ verified_spec_hash, current_ast_hash ≠ verified_ast_hash)
```

The spec hash is computed over **normalised** text: whitespace, line breaks and front-matter key order do not affect it. A typo does change it, so an edit has an `editorial` flag: an editorial edit updates `spec_hash` in the lock immediately and does not produce `spec_ahead`. This is essentially OpenFastTrace's "revision in the ID" (`dsn~name~2` invalidates coverage), without the manual bump.

### 2.6. Bindings: markers in code are the truth, the lock file is the verified state

The original has two sources: the `@implements` marker in code and the `bind_code_symbol` record in the DB. They will diverge. Proposed instead:

- **the marker in code is the only source of the link**. It moves with the symbol when the symbol is moved to another file, lives on the branch and is visible in review. Works in `.rs` and `.ron` alike, since RON supports `//` comments;
- **`spec.lock` in the repository** stores the verified hash pair (§2.5). Semantically it is a Cargo.lock for spec-code conformance;
- `bind_code_symbol` in MCP does not write to the DB: it checks that the marker is present and the symbol resolves, and returns the hash.

### 2.7. Symbol identity in Rust: tree-sitter does not resolve modules

`game::systems::stamina::regen_system` cannot be derived from a single file. The module path comes from the `mod` tree, from `mod.rs` or `file.rs`, from `#[path]` and the crate root (Cargo metadata). A custom module resolver on top of tree-sitter is needed (architecture §5); rust-analyzer/SCIP is an option for later. Methods: `Type::method`; trait impls: `<Type as Trait>::method`.

The `symbol_type` list is incomplete. Add: `trait`, `const`, `static`, `type_alias`, `macro`, `mod`, **`test`**, `message`, `observer`, `plugin`, `resource`, plus **data records and fields** (RON). In data-driven projects a hard-coded tunable value is an implementation bug.

### 2.8. The AST-hash limitation must be stated out loud

A symbol's subtree hash cannot see that behaviour changed in a called helper or in a constant. What to do:

- bind a node to several symbols (system + pure rule function);
- optionally fold hashes of called functions from the same crate into the symbol hash, depth 1;
- bind a node to **tests** (`@verifies`): the best detector of behaviour change;
- bind to data (RON record/field).

### 2.9. Bevy 0.19: the "Events" layer is obsolete

Since Bevy 0.17 buffered events are `Message` + `MessageReader`/`MessageWriter`. `Event` now belongs to observers (`On<E>`, `EntityEvent`). The layer matrix in §8 must be updated. A node's layer should be **derived from the AST** (`#[derive(Component)]`, `Resource`, `Message`, registration in `add_systems`/`add_observer`), not set by hand.

Besides the 4 implementation layers, **business-logic layers** are needed, or "describing the game at the top level" is impossible: Game → Domain → Mechanic → Rule / Invariant / Edge case, plus Content and Network where the project has them.

### 2.10. Multi-user, CRDT, WebSocket < 100 ms: out of the MVP

There is one user. Real contention is between the human and several agents in worktrees, and it is solved by `base_hash` on the proposal plus optimistic locking. SSE is enough for live UI updates. Multi-tenant, Yjs/Automerge and PostgreSQL can be postponed at no cost.

### 2.11. Docker and the MCP transport

- "Port 3000 — MCP (stdio/SSE)" is wrong: stdio runs over process pipes, not a port. The HTTP+SSE transport was replaced by Streamable HTTP back in revision 2025-03-26 and is formally deprecated in the current revision **2026-07-28**. Revision 2026-07-28 also made the protocol **stateless**: no sessions or `initialize`, user requests go through MRTR, tasks moved to an extension (details in `04-prior-art-and-stack.md` §3).
- Docker on macOS with a 500+ kLOC bind-mount is slow, and file-watching through Docker Desktop is unreliable. The primary mode is a **native binary** (`cargo install`); Docker is optional.
- The HTTP server must listen on 127.0.0.1 only, check `Origin` (DNS-rebinding protection) and require a local token.

### 2.12. The "< 2,000 tokens" bundle budget is unrealistic for a feature

On a real corpus the W of a typical feature is an order of magnitude larger even after optimisation. Budgets are needed per level: **node ≤ 2 k, task ≤ 8–12 k**, configurable. Count with a language-aware estimator: Cyrillic costs 40–75 % more tokens per character.

### 2.13. The agent's self-report on tests is insufficient

`report_implementation_status(test_passed: true)` contradicts the principle "the agent does not judge its own work as accepted". SpecEngine must run the tests bound via `@verifies` itself (a `cargo nextest` filter), or at least record the command, the commit and the output hash.

### 2.14. Vector search (pgvector): remove

Before two months under enforcement it is harmful: it masks rot. The 2026 data (04 §2.1) is against it on the merits too: embedders fail at "find the code this requirement is about", and Cursor switched its semantic indexing off. For thousands of nodes FTS5 + graph + resolved symbols is enough. Duplicate questions are caught by text through FTS and by node overlap in the graph.

## 3. What was missing and must be added

1. **Owner queue (inbox)** and **task approval by the owner**: discrepancies and questions are visible and handled as they arrive, blocking nothing; only an approved task goes into development (ADR-0012). This is the owner's main requirement.
2. **Tasks** as an entity: goals, nodes, criteria, blockers, the snapshot of spec revisions the task was prepared on (if the spec changed, the task is stale).
3. **Typed links** and **impact analysis**: what a node edit affects, which tasks and bindings.
4. **Classes, tiers and budgets** of documents per the convention (ADR-0022); without them W starts growing again.
5. **Glossary** "term → identifier in code", checked against the code. The main tool against hallucinated names.
6. **Provenance**: author (human, model, run id), cost, context.
7. **Importers** for the existing corpora of consumer projects and an **acknowledged debt** mode (baseline) for migration.
8. **Homoglyph check in IDs** (Cyrillic look-alikes of Latin K, P, B, C) and registry-issued IDs (ADR-0009).
9. Product acceptance criteria that test real pains (see `08-roadmap.md` §3).
