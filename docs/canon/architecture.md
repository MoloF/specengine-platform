---
class: canon
tier: 2
scope: [architecture]
owner: owner
reviewed: 2026-10-06
---

# SpecEngine architecture rules

The rules all code is written by. Each rule was introduced by a decision (ADR) and changes only through a new decision in the same change (promotion rule). Reasons are in the ADRs; the detailed design is in the spec `docs/specs/specengine-platform/`.

<a id="storage"></a>
## Storage

- The source of truth for specs and bindings is files in the project's git repository (Markdown + YAML front-matter) and `spec.lock`. SQLite is a rebuildable index plus the operational queue (proposals, tasks, runs). ADR-0001.
- The operational database lives outside the repository: `~/Library/Application Support/specengine/`. Git will receive the generated `docs/generated/queue.md`; the queue's full dump, `spec export state` (ADR-0003's `export --state`), goes to a backup directory, never into the repository, and `spec import-state` restores it into an empty queue (`docs/canon/queue-backup.md`). ADR-0003.

<a id="layout"></a>
## Spec layout in a project

A record of a project-scoped prefix (R, A, Q, DEC, TERM) is a separate file. A node document (mechanic, feature) is one file; its rules are sections with `{#ID}`. An ID whose prefix has `scope = "feature"` in `[ids]` (criteria) is defined only as a `{#ID}` section of a feature document: a file `<slug>.md` directly under `[paths] features`, slug = the file stem, `[a-z][a-z0-9-]*`. It is unique within that file, has no record file, and is cited `<slug>/ID` from other files, bare inside its own. ADR-0026.

Granularity: a record file gives atomic diffs, conflict-free merges and cheap agent writes; a mechanic or feature reads best whole, and its sections are hashed one by one, so editing a neighbour gives no false drift. Directories come from `specengine.toml` `[paths]`, so a consumer's existing `docs/` stays intact; the defaults (`crates/specengine-core/README.md` "`[paths]`"):

- `specengine.toml` (roots, ID prefixes, classes, budgets, zones); `spec.lock` (verified node ↔ symbol hash pairs, generated, committed; Phase 3);
- `docs/spec/`, the business-logic tree (canon): a root document (a game's vision, pillars, core loop), `<domain>/README.md` (Tier 1), `<domain>/<mechanic>.md` with rules, invariants and edge cases as `{#ID}` sections;
- `docs/records/<PREFIX>/<ID>.md` (`R/R-12.md`, `DEC/DEC-0023.md`); `docs/features/<slug>.md`, change specs (consumable);
- `docs/generated/` (index, registries, glossary, queue: SpecEngine's output); `docs/archive/` (Tier 3 by front-matter, not by folder).

<a id="apply"></a>
## Applying proposals

An approved proposal is applied in the task branch's worktree (the spec travels with the code) and committed immediately as a separate commit `spec: apply PR-…` with provenance: who decided, the author agent's model and run. A proposal is bound at creation to the worktree, branch and base commit it was raised on (a task's proposals: the task's worktree), and is applied only there, never in the caller's or the main tree; one whose repository is gone can only be rejected. Spec files are written only by `apply_proposal` on the owner's action. ADR-0004, ADR-0005, ADR-0032. How: `docs/canon/proposal-apply.md`. Class `generated` documents are not spec files: each is written only by its registered generator, never by `apply_proposal` or by hand, as `spec export` writes the queue (ADR-0003) and a project's generators their output (ADR-0013, ADR-0022); owner, Q3, 2026-09-30.

<a id="control"></a>
## Control and no blocking

- Discrepancies, questions and proposals block nothing. A node may have several open proposals; they are rebased on `base_hash` when applied. An agent keeps working on a working answer and marks `// @assumes PR-…`. If the decision differs from the working answer, a follow-up task is created. ADR-0012.
- The only control point is the owner approving a task: `claim_task` returns only `ready` tasks. The `PreToolUse` hook checks only that. When the daemon is unavailable the hook is closed (exit 2). `observe` mode turns this check off too. ADR-0006, ADR-0012.

<a id="tasks"></a>
## Tasks and stack skills

Tasks live per project: in its database and its exported queue (`#storage`), never in SpecEngine's repository. SpecEngine hands a task out as a stack-neutral package with a schema version: goal, targets, criteria, assumptions, open proposals, bindings, `spec_snapshot`, the bundle (07 §1.2). A new key keeps the version; removing or renaming one raises it. The engine writes no stack-specific text into the package or brief, its plugin has no stack role names, and the core never branches on the optional `profile` of `specengine.toml`. The code layer stays Rust-first (`#markers`, `#code-identity`). Rendering and execution belong to the project's `.claude/` skills and roles, or to a stack-profile plugin hosted outside SpecEngine's repository; the project's layer comes first. A profile adds no control point (`#control`); a tracker ticket is rendered one way and stores nothing back. ADR-0027.

<a id="rules-format"></a>
## Rule form

EARS ("WHEN … the system SHALL …") is a suggested template for rules, not a checked requirement. ADR-0007.

<a id="universal"></a>
## Universality

The core knows no subject domain. Node kinds are each project's vocabulary, declared per prefix in `[ids]`, never validated in a document; link types are the model's shared table. Domains, prefixes, kinds, zones, budgets and process rules (required keys, allowed values, labelled parts, own text; keyed by kind, class or path: `[[check.rules]]`, the project's own form findings) live in each project's `specengine.toml` and importer. From Phase 1 the core is exercised on at least two pilot projects of different nature; the structure of one project is never carried into another. ADR-0008, ADR-0031.

<a id="ids"></a>
## Identifiers

All IDs are Latin; mixing scripts inside an ID is forbidden. Legacy IDs of an imported corpus (including non-Latin ones) live as `aliases` and resolve in text and git history; the mapping of legacy prefixes to new ones is set in the project's `specengine.toml`. Record text is not translated. ADR-0009.

<a id="markers"></a>
## Code markers

- Code is linked to the spec by a comment `// @implements ID@rev [tiers]` (also `@verifies`, `@configures`, `@assumes`) in `.rs` and `.ron`. The revision in the marker is raised after the code is brought in line with the new text. AST hashes of the verified state live in `spec.lock`. ADR-0010, ADR-0018.
- Existing ID citations in code automatically become weak `mentions` links. Markers are placed only in new work and when code is touched; there is no mass conversion. ADR-0016.

<a id="code-identity"></a>
## Symbol identity and digest

MVP: tree-sitter and markers only; the marker provides the identity of a link. The digest is a normalized tree-sitter AST. rust-analyzer as a library is a precision layer in Phase 3. ADR-0020, ADR-0021.

<a id="ui"></a>
## Interface

Web UI: React 19 + Vite + TanStack Query + `@xyflow/react` on strict TypeScript, in `ui/`, a standalone pnpm project. The interface is in English; spec content is shown in the project's language, so search and token estimates must handle non-Latin scripts. ADR-0011, ADR-0014.

The UI reads and decides only through one client interface, implemented twice: by default an HTTP client of `specengine-http`, the daemon's read surface (`crates/specengine-http/README.md`; a decision there is refused, it stays on a terminal), and a typed mock (`?scenario=`), flagged "Mock data" on every screen. Its types are provisional hand-written ones; types generated from Rust replace them and win, nothing extends them. Dependencies are the owner's allowlist at exact versions; a new package or a version change is an owner decision. The UI computes no core logic (diffs, rebases, checks): it renders what the daemon returns. How: `ui/README.md`. ADR-0033.

<a id="checks-migration"></a>
## Documentation checks in projects

A project's own generators (schemas, registries, references derived from code) stay in the project; their output is class generated. Documentation checks move into `spec check` once it reaches parity with the project's checks. ADR-0013.

<a id="documentation-convention"></a>
## Documentation convention — mandatory for every project

The convention `docs/canon/documentation-system.md` is a built-in part of SpecEngine, not an option: four classes, tiers, budgets, the front-matter contract, the promotion rule, a generated index and the six checks of §11 apply to every project under SpecEngine and to this repository. Here they are enforced by `spec check` itself: roles, the pre-commit hook and CI (`docs/README.md` "Enforcement"). ADR-0022.

<a id="language"></a>
## Repository language

All content of this repository — documentation, decisions, role prompts, code comments, messages — is in English. ADR-0024.

<a id="distribution"></a>
## Distribution and mode

`cargo install` + a Claude Code plugin (MCP, hooks, stack-neutral roles and prompts in one version; stack roles: `#tasks`). Single-user mode. A multi-user server mode (initial spec §6.1 Model A) is out of plan, but storage access goes through a trait and changes go through the `events` log. ADR-0015, ADR-0017, ADR-0027.

<a id="implementation-base"></a>
## Own implementation

SpecEngine is written from scratch. From tracey it takes the topology "daemon + CLI/MCP/LSP bridges over a socket, a bridge starts the daemon itself" and the revision discipline (pre-commit fails on a semantic change without a `rev` bump). ADR-0019.
