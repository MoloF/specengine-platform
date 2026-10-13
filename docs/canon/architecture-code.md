---
class: canon
tier: 2
scope: [architecture, crates/specengine-code, crates/specengine-cli]
owner: owner
reviewed: 2026-10-13
---

# SpecEngine architecture rules: the code layer

The detail of the code layer's rules: markers, symbol identity, `spec.lock`, drift, sync in reads, Health's code rows and test evidence. `docs/canon/architecture.md` states each rule in a sentence or two with its ADRs and points at its section here; both change only through a new decision, in the same change (promotion rule). Design: 05 §3.5, §5. Layer A as built: `docs/canon/code-identity.md`; markers bound to code and `spec refs`: `docs/canon/code-bindings.md`; revisions and `norm_hash`: `docs/canon/node-revision.md`; `spec.lock`: `docs/canon/spec-lock.md`; `spec verify`: `docs/canon/spec-verify.md`.

<a id="markers"></a>
## Markers

Rule: `architecture.md#markers`. ADR-0010, ADR-0016, ADR-0018.

- A marker is a comment in `.rs` or `.ron`: `// @implements ID@rev [tiers]`, also `@verifies`, `@configures`, `@assumes`. Grammar, levels, RON field paths: `code-identity.md` "Marker grammar", "RON binding"; the Rust item it binds, its ID and revision states: `code-bindings.md` "Rust binding", "ID state", "Revision state".
- The revision in a marker is raised after the code is brought in line with the node's new text; the AST hashes of the verified state live in `spec.lock` (`#spec-lock`).
- Existing ID citations in code comments automatically become weak `mentions` links, never bindings (`code-bindings.md` "Mentions", listed by `spec refs`). Markers are placed only in new work and when code is touched; there is no mass conversion.

<a id="code-identity"></a>
## Symbol identity and digest

Rule: `architecture.md#code-identity`. ADR-0020, ADR-0021, ADR-0048, ADR-0049.

- The marker provides a link's identity; a symbol's `qpath` is only its name. The digest is a normalized tree-sitter AST.
- Layers (05 §5.1): A, tree-sitter and markers, the MVP, built (`code-identity.md`, bindings `code-bindings.md`); B, a build-derived file (Bevy's `schedule_data` dump), Phase 3; C, rust-analyzer as a library in its own executable, the Phase 3 precision layer (`crates/specengine-ra`, outside `default-members`). What A cannot enumerate, a generic system's instances, needs B or C.
- Layer B (ADR-0048): the project writes the file and names it in `[[code.dump]]` with a format name (Bevy 0.19's own RON: `bevy-0.19-schedule`); SpecEngine never patches, builds or runs the project for it, and never walks it as code. Its BLAKE3 keys the index. Its entries annotate layer A symbols as role rows tagged `b`, matched by crate, module path and name, else a unique name, else `ambiguous` or a counted miss; never a symbol, binding, `spec.lock` key, verify row or check finding. Missing, unreadable or stale: a state line and two counts (dumped entries with no symbol, detector registrations not dumped), never an exit. On what is registered in which group, B rows come first; A rows stay, flagged where they disagree. Readers live in `specengine-code`; core and store hold a path and an opaque format name. Not built: `docs/features/bevy-dump.md`.
- Layer C (ADR-0049): `ra_ap_*` is linked only into `specengine-ra`, an optional binary installed from the same commit as `spec`; `spec`, MCP and the daemon never link it. Only `spec resolve` spawns it, found beside `spec`, else on `PATH`: its own process group under a budget, one versioned JSON request on stdin (kept open as the liveness signal), JSON lines on stdout, an exact protocol match. No cargo build, so no project build script or proc macro runs; `cargo metadata` does, locked and offline, its target and temporary directories in the data directory. `spec` stores the answer, a moniker or a state per item, only after a complete reply over an unchanged tree: one snapshot per worktree, keyed by a fingerprint of code files, manifests, lock, toolchain and cargo config files; any change leaves every row stale, old values shown. It joins layer A rows by file and start byte. The marker binds, the qpath names for `spec.lock` and verify, the moniker is display and exact lookup; a disagreement is counted and shown, never fixed or a finding. Missing or failing, only `spec resolve` notices. Not built: `docs/features/layer-c.md`.

<a id="spec-lock"></a>
## `spec.lock` writers

Rule: `architecture.md#apply`. ADR-0041, ADR-0043, ADR-0044, ADR-0045, ADR-0050.

`spec.lock` (verified node ↔ symbol hash pairs, committed: `architecture.md#layout`) is neither a spec file nor a `generated` document. Only the owner's terminal `[y/N]` writes it, through these writers alone:

- `spec lock accept` over committed bytes (ADR-0041), or `spec approve` of a `drift` item running it (ADR-0045);
- `spec lock regen`, for a merge (ADR-0041);
- `spec lock rebase`, into this build's `format` and `code_recipe`: each entry's code hashes re-derived from its own `file_blob`, never `HEAD`'s tree or the working tree; key, `rev`, `spec_hash`, `file_blob`, `commit`, `verified_*` kept; an entry it cannot re-derive dropped and named; never another `norm_recipe` or an older format (ADR-0050);
- `spec approve --editorial`, in its apply commit (ADR-0043);
- `spec verify --tests`: `[[tested]]` evidence added on a pass, removed on a failure (ADR-0041, ADR-0044).

Each but a merge's and an editorial apply's is its own commit with provenance. Never otherwise `apply_proposal`, nor an MCP tool, a role, a check or a hand edit.

As built (slice 3): the file, its entries, lockability and `regen` in `docs/canon/spec-lock.md`; `accept`, its inputs from a clean `HEAD`, its verified commit, `--editorial` and a gone node's drop in `docs/canon/spec-lock-accept.md`. The approve, rebase (`docs/features/lock-levels.md`) and `verify --tests` writers are not built yet.

<a id="verify"></a>
## Drift: `spec verify`

Rule: `architecture.md#control`. ADR-0042, ADR-0045, ADR-0050.

- `spec verify` shows drift between `spec.lock` and the tree as Fix, Check or Pre-existing findings.
- Only a project's own `[verify] mode` (`enforce-introduced`, `enforce`; default `observe`) makes its CI or hook stop on Fix. Otherwise `spec propose drift` queues the findings as items that stop nothing (ADR-0045), reads show the same states as `sync` (`#sync-in-reads`) and Health lists them (`#health`).
- A binding it cannot verify is never `ok`: exit 2 in every mode; a lock of another format or recipe too, until `spec lock rebase` carries it over (`#spec-lock`).

As built (slice 4a): rows, states, the cascade, the base and the run causes in `docs/canon/spec-verify.md`; text, hints and JSON in `docs/canon/spec-verify-output.md`. `spec propose drift` (`docs/features/drift-queue.md`) is not built yet.

<a id="sync-in-reads"></a>
## Sync in reads

Rule: `architecture.md#control`. ADR-0046.

- `spec bundle` (layers 5 and 9), `show --bindings`, `tree`'s counters, `task show`'s `bindings` and `impact`, and their MCP and HTTP twins, show each binding of their nodes as one row: the lock key, its place, signature, layer and `sync`.
- `sync` is `spec verify`'s plain state (no base, no Pre-existing) from the same pure judge, over the code index the read refreshes, `spec.lock` and the read's own parse; never stored, so it may be newer than the last `spec verify`. No exit code, status, claim, approval or gate verdict depends on it.
- A read of nodes never fails on the code layer: a bad `[code]` or a walk gap shows no rows, an invalid or unmerged lock, no git or a git failure shows rows with `sync` unknown; either way one `note: bindings: <cause>`, the spec part and exit unchanged, never `ok`. Without `[code]` such a read is byte-identical to one without a code layer. `impact --symbol` and `--since` take code as input and exit 2 on it, as `spec symbols`.
- Of the reads, these and `refs`, `symbols`, `unmapped`, `index`, `verify`, `health` (`#health`) walk code; `show` without `--bindings`, `search` and `graph` never do. Git is read only (`rev-parse`, `ls-tree`, `cat-file`, `hash-object` without `-w`), never `status` or `diff`. `impact --since` compares a git revision with the working tree, untracked files included; `spec.lock` plays no part.

<a id="health"></a>
## Health's code rows

Rule: `architecture.md#control`, `architecture.md#universal`. ADR-0047.

`spec health` (and its route) lists rows 4-6 of "What is left" (06 s7) beside row 3, each its own key, informational only:

- **Row 4**, code assuming a decision: each marker naming a queue item (`code-bindings.md` "ID state" `queue`), looked up by exact ID in the slug's queue: pending, as assumed (the working answer, or the recommended option), otherwise, applied, rejected or unknown; each `@assumes` naming a node whose document is Tier 3, or which an open or approved item targets. A project's status words are never interpreted.
- **Row 5**, unbound nodes: the live nodes, documents or `{#ID}` sections, that the project's `[[health.bound]]` rules select (`[[check.rules]]`' selectors: kinds, classes, paths, front-matter values; a section by its ID's prefix kind) and that no marker of the rule's relations binds (an ID resolved, an alias or each ambiguous holder; an item, module, RON path or `cannot_verify`). Mentions, look-alikes, `unanchored` never bind; a revision never matters. No rule: not measured, never "all unbound"; `spec check` never reads the table.
- **Row 6**, drift: plain `spec verify`'s judgement, run in-process, its rows but `ok`; never its mode or verdict.
- One fresh code walk per call, never 6a's code index; the queue is opened only if it exists; git only through verify (`hash-object` without `-w`). Only the data directory's index is written. A row it cannot measure shows its causes, never 0, and leaves the other rows and the exit as they are; no exit 1.

<a id="tests"></a>
## Tests as evidence

Rule: `architecture.md#tasks`. ADR-0044.

SpecEngine runs a project's tests only through its `[verify] tests` argv template, on the owner's terminal (`spec verify --tests`, `#spec-lock`). `verified` is computed from `spec.lock`'s evidence, never from an agent's report.
