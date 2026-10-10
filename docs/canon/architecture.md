---
class: canon
tier: 2
scope: [architecture]
owner: owner
reviewed: 2026-10-10
---

# SpecEngine architecture rules

The rules all code is written by, each with the ADRs that introduced it and hold its reasons; a rule changes only through a new decision in the same change (promotion rule). Its detail is one step away, in the document its section names (the code layer's: `docs/canon/architecture-code.md`); the design: `docs/specs/specengine-platform/`.

<a id="storage"></a>
## Storage

- The source of truth for specs and bindings is files in the project's git repository (Markdown + YAML front-matter) and `spec.lock`. SQLite is a rebuildable index plus the operational queue (proposals, tasks, runs). ADR-0001.
- The operational database lives outside the repository, in `~/Library/Application Support/specengine/` beside the daemon's files (`docs/canon/serve.md`). Git will receive the generated `docs/generated/queue.md`, never the queue's dump (`spec export state`, `docs/canon/queue-backup.md`). ADR-0003.

<a id="layout"></a>
## Spec layout in a project

A record of a project-scoped prefix (R, A, Q, DEC, TERM) is a separate file, for atomic diffs, conflict-free merges and cheap agent writes; a node document (mechanic, feature) is one file, read whole, its rules `{#ID}` sections hashed one by one, so editing a neighbour gives no false drift. An ID whose prefix has `scope = "feature"` in `[ids]` (criteria) is only a `{#ID}` section of a feature document `<slug>.md` directly under `[paths] features`, cited `<slug>/ID` from other files, bare inside its own (`docs/canon/spec-check-links.md`). ADR-0026.

A node's revision is its own (a document's `rev:`, a section's `rev=N`; none written = 1). ADR-0040. Approving a change of its `norm_hash` raises it, never lowering it; `spec approve --editorial` keeps it (`#apply`); `spec check --staged` and `--changed` flag a locked node changed with neither. How: `docs/canon/node-revision.md`. ADR-0043.

Directories come from `specengine.toml` `[paths]`, so a consumer's existing `docs/` stays intact; each defaults to `docs/<role>` (`crates/specengine-core/README.md` "`[paths]`"): `spec/`, the business-logic tree (canon: a root document, `<domain>/README.md` Tier 1, `<domain>/<mechanic>.md`); `records/<PREFIX>/<ID>.md`; `features/<slug>.md`, change specs (consumable); `generated/`, SpecEngine's output; `archive/`, Tier 3 by front-matter, not by folder. Beside `specengine.toml`, the committed `spec.lock`: verified node ↔ symbol hash pairs, Phase 3, its writers in `#apply`.

<a id="apply"></a>
## Applying proposals

An approved proposal is applied only in the worktree, branch and base commit it was raised on (a task's: its worktree, so the spec travels with the code) and committed at once as its own commit `spec: apply PR-…` with provenance (`docs/canon/proposal-apply.md`). Spec files are written only by `apply_proposal` on a decision the owner confirms on a terminal, one `[y/N]` each; the UI, later an MCP form, only stages (`docs/canon/decision-staging.md`, `round.md`). ADR-0004, ADR-0005, ADR-0032, ADR-0035, ADR-0038.

A `generated` document is no spec file: only its registered generator writes it (`spec export` the queue, a project's generator its output). ADR-0003, ADR-0013, ADR-0022; owner, Q3, 2026-09-30.

`spec.lock` is neither: only the owner's terminal `[y/N]` writes it, by the writers of `docs/canon/architecture-code.md#spec-lock`; never otherwise `apply_proposal`, nor an MCP tool, a role, a check or a hand edit. ADR-0041, ADR-0043, ADR-0044, ADR-0045.

<a id="control"></a>
## Control and no blocking

- Discrepancies, questions and proposals block nothing: a node may have several open proposals, rebased on `base_hash` when applied; an agent keeps working on a working answer, marked `// @assumes PR-…`, and a decision unlike it makes a follow-up task. ADR-0012.
- The only control point is the owner approving a task on a terminal (`spec task approve`, no MCP tool; `claim_task` takes only `ready`). The `PreToolUse` hook passes a gated write only for an `in_progress` task claimed in its worktree and branch, never consulting the daemon (`docs/canon/gate.md`). ADR-0012, ADR-0039.
- `spec verify` shows drift between `spec.lock` and the tree and stops nothing unless a project's own `[verify] mode` says so; `spec propose drift` queues items that stop nothing; a binding it cannot verify is never `ok`. How: `docs/canon/architecture-code.md#verify`. ADR-0042, ADR-0045.
- Reads of nodes (bundle, `show --bindings`, tree, task package, impact) show each binding's `sync`, verify's plain state computed per read, never stored, nothing depending on it; a code-layer failure leaves a read's spec part and exit as they were, `sync` unknown, never `ok`. How: `docs/canon/architecture-code.md#sync-in-reads`. ADR-0046.

<a id="tasks"></a>
## Tasks and stack skills

Tasks live per project, in its queue and dump (`#storage`), never in SpecEngine's repository (`docs/canon/tasks.md`). A task goes out as a stack-neutral, versioned package, the core never branching on a `profile` (`docs/canon/task-package.md` "Genre"); the code layer stays Rust-first. Rendering and execution belong to the project's `.claude/` or a stack-profile plugin outside SpecEngine's repository, the project's layer first, adding no control point; a tracker ticket is rendered one way, storing nothing back. ADR-0027. A project's tests run only through its own command on the owner's terminal, `verified` never from an agent's report (`docs/canon/architecture-code.md#tests`). ADR-0044.

<a id="rules-format"></a>
## Rule form

EARS ("WHEN … the system SHALL …") is a suggested template for rules, not a checked requirement. ADR-0007.

<a id="universal"></a>
## Universality

The core knows no subject domain. Node kinds are each project's vocabulary, declared per prefix in `[ids]`, never validated in a document; link types are the model's shared table. Domains, prefixes, kinds, zones, budgets and process rules (`[[check.rules]]`: `docs/canon/spec-check-process.md`) live in each project's `specengine.toml` and importer. From Phase 1 the core is exercised on at least two pilot projects of different nature; the structure of one is never carried into another. ADR-0008, ADR-0031.

<a id="ids"></a>
## Identifiers

All IDs are Latin; mixing scripts inside an ID is forbidden. Legacy IDs of an imported corpus (including non-Latin ones) live as `aliases` and resolve in text and git history; the mapping of legacy prefixes to new ones is set in the project's `specengine.toml`. Record text is not translated. ADR-0009.

<a id="markers"></a>
## Code markers

Code is linked to the spec by a comment `// @implements ID@rev [tiers]` (also `@verifies`, `@configures`, `@assumes`) in `.rs` and `.ron`; `spec.lock` keeps the AST hashes of the verified state. ADR-0010, ADR-0018. Existing ID citations in code become weak `mentions`, never mass-converted. ADR-0016. How: `docs/canon/architecture-code.md#markers`.

<a id="code-identity"></a>
## Symbol identity and digest

The marker is a link's identity; the digest is a normalized tree-sitter AST. MVP: tree-sitter and markers; rust-analyzer as a library is a Phase 3 precision layer. Layers: `docs/canon/architecture-code.md#code-identity`. ADR-0020, ADR-0021.

<a id="ui"></a>
## Interface

Web UI: React 19 + Vite + TanStack Query + `@xyflow/react` on strict TypeScript, in `ui/`, a standalone pnpm project. The interface is in English; spec content is shown in the project's language, so search and token estimates must handle non-Latin scripts. ADR-0011, ADR-0014.

The UI reads and stages decisions only through one client interface, the daemon's HTTP client by default or a typed mock flagged "Mock data" on every screen; a decision is only staged, confirmed on a terminal (`#apply`). It computes no core logic (diffs, rebases, checks) and renders what the daemon returns; types generated from Rust replace its provisional ones; dependencies are the owner's allowlist at exact versions; spec prose renders as markdown only through `ui/src/markdown/`. How: `ui/README.md`. ADR-0033, ADR-0036.

<a id="checks-migration"></a>
## Documentation checks in projects

A project's own generators (schemas, registries, references derived from code) stay in the project; their output is class generated. Documentation checks move into `spec check` once it reaches parity with the project's checks. ADR-0013.

<a id="documentation-convention"></a>
## Documentation convention — mandatory for every project

The convention `docs/canon/documentation-system.md` is a built-in part of SpecEngine, not an option: it applies to every project under SpecEngine and to this repository, enforced here by `spec check` itself through roles, the pre-commit hook and CI (`docs/README.md` "Enforcement"). ADR-0022.

<a id="language"></a>
## Repository language

All content of this repository — documentation, decisions, role prompts, code comments, messages — is in English. ADR-0024.

<a id="distribution"></a>
## Distribution and mode

`cargo install` + a Claude Code plugin (MCP, hooks, stack-neutral roles and prompts in one version; stack roles: `#tasks`). Single-user mode. A multi-user server mode (initial spec §6.1 Model A) is out of plan, but storage access goes through a trait and changes go through the `events` log. ADR-0015, ADR-0017, ADR-0027.

No authentication inside SpecEngine, ever, on any surface: no token, key, account, session, WebAuthn or Touch ID; hosting or a shared machine needs infrastructure in front, per service. Threat model: `docs/canon/serve.md#trust`. ADR-0034.

<a id="implementation-base"></a>
## Own implementation

SpecEngine is written from scratch. From tracey it takes the topology "daemon + CLI/MCP/LSP bridges over a socket, a bridge starts the daemon itself" and the revision discipline (`#layout`, ADR-0043). ADR-0019.
