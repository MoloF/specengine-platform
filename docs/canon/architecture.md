---
class: canon
tier: 2
scope: [architecture]
owner: owner
reviewed: 2026-11-12
---

# SpecEngine architecture rules

The rules all code is written by, each with the ADRs that introduced it and hold its reasons; a rule changes only through a new decision in the same change. Its detail is one step away, in the document its section names (the code layer's: `architecture-code.md`); the design: `docs/specs/specengine-platform/`.

<a id="storage"></a>
## Storage

- The source of truth for specs and bindings is files in the project's git repository (Markdown + YAML front-matter) and `spec.lock`. SQLite is a rebuildable index plus the operational queue (proposals, tasks, runs). ADR-0001.
- The operational database lives outside the repository, in the data directory beside the daemon's files (`serve.md`). Git will receive the generated `docs/generated/queue.md`, never the queue's dump (`spec export state`, `queue-backup.md`). ADR-0003.
- A plan item is a `spec`-class file whose `id:` has the prefix `[roadmap]` names; tasks execute it from the queue; its position is computed per read, never stored, its `depends_on` chain ordering work, never gating it (`spec-roadmap.md`). ADR-0052.

<a id="layout"></a>
## Spec layout in a project

A record of a project-scoped prefix (R, A, Q, DEC, TERM) is a separate file, for atomic diffs, conflict-free merges and cheap agent writes; a node document (mechanic, feature) is one file, read whole, its rules `{#ID}` sections hashed one by one, so editing a neighbour gives no false drift. An ID whose prefix has `scope = "feature"` in `[ids]` (criteria) is only a `{#ID}` section of a feature document `<slug>.md` directly under `[paths] features`, cited `<slug>/ID` from other files, bare inside its own (`spec-check-links.md`). ADR-0026.

A node's revision is its own (a document's `rev:`, a section's `rev=N`; none written = 1). ADR-0040. Approving a change of its `norm_hash` raises it, never lowering it; `spec approve --editorial` keeps it (`#apply`); `spec check --staged` and `--changed` flag a locked node changed with neither. How: `node-revision.md`. ADR-0043.

Directories come from `specengine.toml` `[paths]`, so a consumer's existing `docs/` stays intact; each defaults to `docs/<role>` (`crates/specengine-core/README.md` "`[paths]`"): `spec/`, the business-logic tree (canon: a root document, `<domain>/README.md` Tier 1, `<domain>/<mechanic>.md`); `records/<PREFIX>/<ID>.md`; `features/<slug>.md`, change specs (consumable); `generated/`, SpecEngine's output; `archive/`, Tier 3 by front-matter, not by folder. Beside `specengine.toml`, the committed `spec.lock`: verified node ↔ symbol hash pairs, Phase 3, its writers in `#apply`.

<a id="apply"></a>
## Applying proposals

An approved proposal is applied only in the worktree, branch and base commit it was raised on (a task's: its worktree, so the spec travels with the code) and committed at once as its own commit `spec: apply PR-…` with provenance (`proposal-apply.md`). Spec files are written only by `apply_proposal` once a person confirms: on a terminal, one `[y/N]` each, or on a hub alone under a login `[team]` entitles, one click per item; a hub's `--push` sync merges others' commits, `spec.lock` too, never rebasing and authoring neither (`hub-git.md`); else the UI and the owner's MCP form only stage, and no tool takes a choice from an agent (`decision-staging.md`, `hub-confirm.md`, `review-form.md`, `round.md`). ADR-0004, ADR-0005, ADR-0032, ADR-0035, ADR-0038, ADR-0054, ADR-0061, ADR-0064.

A `generated` document is no spec file: only its registered generator writes it (`spec export` the queue; SpecEngine the `index = true` entry's outputs, by `export index` or in a commit it makes, never a spec file: `proposal-outputs.md`; a project's generator its own). ADR-0003, ADR-0013, ADR-0022, ADR-0062, ADR-0064; owner, Q3, 2026-09-30.

`spec.lock` is neither: only the owner's terminal `[y/N]` writes it, by the writers of `architecture-code.md#spec-lock`; a lock of another format or recipe is carried over only by a rebase from the blobs the owner accepted, never the current tree; never otherwise `apply_proposal`, nor an MCP tool, a role, a check or a hand edit. ADR-0041, ADR-0043, ADR-0044, ADR-0045, ADR-0050.

<a id="control"></a>
## Control and no blocking

- Discrepancies, questions and proposals block nothing: a node may have several open proposals, rebased on `base_hash` when applied; an agent keeps working on a working answer, marked `// @assumes PR-…`, and a decision unlike it makes a follow-up task. ADR-0012.
- The only control point is the owner approving a task on a terminal (`spec task approve`, no MCP tool; `claim_task` takes only `ready`). The `PreToolUse` hook passes a gated write only for an `in_progress` task claimed in its worktree and branch, never consulting the daemon (`gate.md`). ADR-0012, ADR-0039.
- `spec verify` shows drift between `spec.lock` and the tree and stops nothing unless a project's own `[verify] mode` says so; a binding it cannot verify is never `ok`. How: `architecture-code.md#verify`. ADR-0042, ADR-0045, ADR-0050.
- Reads of nodes (bundle, `show --bindings`, tree, task package, impact) show each binding's `sync`, verify's plain state computed per read, never stored, nothing depending on it. How: `architecture-code.md#sync-in-reads`. ADR-0046.
- `spec health` lists, read-only, suspect markers, unbound nodes and drift. A row it cannot measure shows its causes, never 0; no exit 1, check finding, status, claim, approval, task or gate depends on them. How: `architecture-code.md#health`. ADR-0047, ADR-0051.

<a id="tasks"></a>
## Tasks and stack skills

Tasks live per project, in its queue and dump (`#storage`), never in SpecEngine's repository (`tasks.md`). A task goes out as a stack-neutral, versioned package, the core never branching on a `profile` (`task-package.md` "Genre"); the code layer stays Rust-first. Rendering and execution belong to the project's `.claude/` or a stack-profile plugin outside SpecEngine's repository, the project's layer first, adding no control point; a tracker ticket is rendered one way, storing nothing back. ADR-0027. A project's tests run only through its own command on the owner's terminal, `verified` never from an agent's report (`architecture-code.md#tests`). ADR-0044.

Any trade's work is a card, `C-NNNN`, a queue row beside `T-`, never read by the gate (`cards.md`); the requesting side accepts it. ADR-0056. No card write creates, moves or refuses another; a split makes a task and subtasks in one act (`card-split.md`); a mandate only frames the page (`card-mandate.md`). ADR-0074, ADR-0078, ADR-0079.

<a id="rules-format"></a>
## Rule form

EARS ("WHEN … the system SHALL …") is a suggested rule template, not a checked requirement. ADR-0007.

<a id="universal"></a>
## Universality

The core knows no subject domain. Node kinds are each project's vocabulary, declared per prefix in `[ids]`, never validated in a document; link types are the model's shared table. Domains, prefixes, kinds, zones, budgets, process rules (`[[check.rules]]`: `spec-check-process.md`), which nodes must be bound (`[[health.bound]]`: `architecture-code.md#health`), what starts and ends a flow (`[flow]`) and the plan's releases, priority scale and unit (`[roadmap]`) live in each project's `specengine.toml` and importer. From Phase 1 the core is exercised on at least two pilot projects of different nature; the structure of one is never carried into another. ADR-0008, ADR-0031, ADR-0047, ADR-0052.

Flow takes three shared types, `follows` (cycles allowed), `uses`, `supplies`; `depends_on` keeps its meaning and cycle warning (`flow-links.md`). ADR-0055.

<a id="ids"></a>
## Identifiers

All IDs are Latin; mixing scripts inside an ID is forbidden. Legacy IDs of an imported corpus (including non-Latin ones) live as `aliases` and resolve in text and git history; the mapping of legacy prefixes to new ones is set in the project's `specengine.toml`. Record text is not translated. ADR-0009.

<a id="markers"></a>
## Code markers

Code is linked to the spec by a comment `// @implements ID@rev [tiers]` (also `@verifies`, `@configures`, `@assumes`) in `.rs` and `.ron`; `spec.lock` keeps the AST hashes of the verified state. ADR-0010, ADR-0018. Existing ID citations in code become weak `mentions`, never mass-converted. ADR-0016. How: `architecture-code.md#markers`.

<a id="code-identity"></a>
## Symbol identity and digest

The marker is a link's identity; the digest is a normalized tree-sitter AST. MVP: tree-sitter and markers; rust-analyzer as a library is a Phase 3 precision layer. Layers: `architecture-code.md#code-identity`. ADR-0020, ADR-0021.

A layer B fact, one only a build or run of the project knows, is a file the project writes and names in `[code]`; SpecEngine never builds or runs the project for it. Its facts annotate symbols, never bind, lock, judge or block; on what is registered where they outrank the syntactic detector, both shown. How: `architecture-code.md#code-identity`. ADR-0048.

Layer C, rust-analyzer, is linked only into the separate executable `specengine-ra`, spawned only by `spec resolve`, never by a read, MCP, the daemon, a hook, `check` or `verify`; it runs no project build script or proc macro. Its per-item moniker is advisory index data in the data directory, stale after any change: never a binding, `spec.lock` key, verdict or finding. How: `architecture-code.md#code-identity`. ADR-0049.

<a id="ui"></a>
## Interface

Web UI: React 19 + Vite + TanStack Query + `@xyflow/react` on strict TypeScript, in `ui/`, a standalone pnpm project. Its chrome is English or Russian per viewer and spec content as written (`ui-languages.md`), so search and token estimates handle any script. ADR-0011, ADR-0072.

The UI reads, stages or confirms decisions (`#apply`) and writes cards and their questions and decisions only through one client interface, the daemon's HTTP client or a typed mock flagged "Mock data"; a card write takes a typed name or, on a hub, the proxy's login. It computes no core logic and renders what the daemon returns; types generated from Rust replace its provisional ones; dependencies are the owner's allowlist at exact versions; spec prose renders as markdown only through `ui/src/markdown/`. How: `ui/README.md`. ADR-0033, ADR-0036, ADR-0057, ADR-0059, ADR-0077, ADR-0080.

<a id="checks-migration"></a>
## Documentation checks in projects

A project's own generators (schemas, registries, references derived from code) stay in the project; their output is class generated. Documentation checks move into `spec check` once it reaches parity with the project's checks. ADR-0013.

<a id="documentation-convention"></a>
## Documentation convention — mandatory for every project

The convention `docs/canon/documentation-system.md` is a built-in part of SpecEngine, not an option: it applies to every project under SpecEngine and to this repository, enforced here by `spec check` through roles, the pre-commit hook and CI (`docs/README.md` "Enforcement"). ADR-0022.

<a id="language"></a>
## Repository language

All content of this repository — documentation, decisions, role prompts, code comments, messages — is in English but the UI's Russian catalog. ADR-0024, ADR-0072.

<a id="distribution"></a>
## Distribution and mode

`cargo install` + a Claude Code plugin (MCP, hooks, roles and prompts in one version; stack roles: `#tasks`); a hub, a container image (`hub-deploy.md`). ADR-0015, ADR-0027, ADR-0066. A daemon is local, for one person, or a hub, for people behind a proxy (`hub.md`), only its `--push` online (`hub-git.md`). ADR-0058, ADR-0064.

No authentication inside SpecEngine, ever, on any surface: no token, key, account, login session, WebAuthn or Touch ID; hosting or a shared machine needs infrastructure per service; a hub trusts its proxy's login header, no agent on its host. Threats: `serve.md#trust`, `hub.md#trust`. ADR-0034, ADR-0059, ADR-0065, ADR-0067, ADR-0068.

<a id="implementation-base"></a>
## Own implementation

SpecEngine is written from scratch. From tracey it takes the topology "daemon + CLI/MCP/LSP bridges over a socket, a bridge starts the daemon itself" and the revision discipline (`#layout`, ADR-0043). ADR-0019.
