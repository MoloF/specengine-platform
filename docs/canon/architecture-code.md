---
class: canon
tier: 2
scope: [architecture, crates/specengine-code, crates/specengine-cli]
owner: owner
reviewed: 2026-10-10
---

# SpecEngine architecture rules: the code layer

The detail of the code layer's rules: markers, symbol identity, `spec.lock`, drift, sync in reads and test evidence. `docs/canon/architecture.md` states each rule in a sentence or two with its ADRs and points at its section here; both change only through a new decision, in the same change (promotion rule). Design: 05 §3.5, §5. Layer A as built: `docs/canon/code-identity.md`; markers bound to code and `spec refs`: `docs/canon/code-bindings.md`; revisions and `norm_hash`: `docs/canon/node-revision.md`.

<a id="markers"></a>
## Markers

Rule: `architecture.md#markers`. ADR-0010, ADR-0016, ADR-0018.

- A marker is a comment in `.rs` or `.ron`: `// @implements ID@rev [tiers]`, also `@verifies`, `@configures`, `@assumes`. Grammar, levels, RON field paths: `code-identity.md` "Marker grammar", "RON binding"; the Rust item it binds, its ID and revision states: `code-bindings.md` "Rust binding", "ID state", "Revision state".
- The revision in a marker is raised after the code is brought in line with the node's new text; the AST hashes of the verified state live in `spec.lock` (`#spec-lock`).
- Existing ID citations in code comments automatically become weak `mentions` links, never bindings (`code-bindings.md` "Mentions", listed by `spec refs`). Markers are placed only in new work and when code is touched; there is no mass conversion.

<a id="code-identity"></a>
## Symbol identity and digest

Rule: `architecture.md#code-identity`. ADR-0020, ADR-0021.

- The marker provides a link's identity; a symbol's `qpath` is only its name. The digest is a normalized tree-sitter AST.
- Layers (05 §5.1): A, tree-sitter and markers, the MVP, built (`code-identity.md`, bindings `code-bindings.md`); C, rust-analyzer as a library, the Phase 3 precision layer (`crates/specengine-ra`, outside `default-members`); B, the Bevy `schedule_data` dump, Phase 3. What A cannot enumerate, a generic system's instances, needs B or C.

<a id="spec-lock"></a>
## `spec.lock` writers

Rule: `architecture.md#apply`. ADR-0041, ADR-0043, ADR-0044, ADR-0045.

`spec.lock` (verified node ↔ symbol hash pairs, committed: `architecture.md#layout`) is neither a spec file nor a `generated` document. Only the owner's terminal `[y/N]` writes it, through these writers alone:

- `spec lock accept` over committed bytes (ADR-0041), or `spec approve` of a `drift` item running it (ADR-0045);
- `spec lock regen`, for a merge (ADR-0041);
- `spec approve --editorial`, in its apply commit (ADR-0043);
- `spec verify --tests`: `[[tested]]` evidence added on a pass, removed on a failure (ADR-0041, ADR-0044).

Each but a merge's and an editorial apply's is its own commit with provenance. Never otherwise `apply_proposal`, nor an MCP tool, a role, a check or a hand edit.

<a id="verify"></a>
## Drift: `spec verify`

Rule: `architecture.md#control`. ADR-0042, ADR-0045.

- `spec verify` shows drift between `spec.lock` and the tree as Fix, Check or Pre-existing findings.
- Only a project's own `[verify] mode` (`enforce-introduced`, `enforce`; default `observe`) makes its CI or hook stop on Fix. Otherwise `spec propose drift` queues the findings as items that stop nothing (ADR-0045), and reads show the same states as `sync` (`#sync-in-reads`).
- A binding it cannot verify is never `ok`: exit 2 in every mode.

<a id="sync-in-reads"></a>
## Sync in reads

Rule: `architecture.md#control`. ADR-0046.

- `spec bundle` (layers 5 and 9), `show --bindings`, `tree`'s counters, `task show`'s `bindings` and `impact`, and their MCP and HTTP twins, show each binding of their nodes as one row: the lock key, its place, signature, layer and `sync`.
- `sync` is `spec verify`'s plain state (no base, no Pre-existing) from the same pure judge, over the code index the read refreshes, `spec.lock` and the read's own parse; never stored, so it may be newer than the last `spec verify`. No exit code, status, claim, approval or gate verdict depends on it.
- A read of nodes never fails on the code layer: a bad `[code]` or a walk gap shows no rows, an invalid or unmerged lock, no git or a git failure shows rows with `sync` unknown; either way one `note: bindings: <cause>`, the spec part and exit unchanged, never `ok`. Without `[code]` such a read is byte-identical to one without a code layer. `impact --symbol` and `--since` take code as input and exit 2 on it, as `spec symbols`.
- Of the reads, these and `refs`, `symbols`, `unmapped`, `index`, `verify` walk code; `show` without `--bindings`, `search` and `graph` never do. Git is read only (`rev-parse`, `ls-tree`, `cat-file`, `hash-object` without `-w`), never `status` or `diff`. `impact --since` compares a git revision with the working tree, untracked files included; `spec.lock` plays no part.

<a id="tests"></a>
## Tests as evidence

Rule: `architecture.md#tasks`. ADR-0044.

SpecEngine runs a project's tests only through its `[verify] tests` argv template, on the owner's terminal (`spec verify --tests`, `#spec-lock`). `verified` is computed from `spec.lock`'s evidence, never from an agent's report.
