---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-08
---

# Workspaces: a trade's lens on a project

## State

Not built. v4 step 1a `workspaces` (RM-0081, `docs/features/workspaces.md`) builds it. Until it ships, every `spec`, daemon, MCP server and hub refuses a whole `specengine.toml` that holds `[[workspace]]` or `[[module]]`, at that table's line, so no project holds them yet.

<a id="the-lens"></a>
## The lens

A workspace is how one trade sees a project: the sections its nav lists, in its order, the section it lands on, the card flows its Board shows, and a default module. A module is one root document and its subtree; choosing it roots the Tree and Flow reads there. Both are the project's config, read apart from every other read, as `[flow]`, `[card]` and `[team]` are: a bad table serves the implicit workspace, every section but Tasks, with a note, and fails nothing.

A workspace is never an entitlement, a control point or a hidden route. Every section keeps its hash route and its palette entry, and a workspace changes no right, gate, finding or exit (ADR-0012). `[team]`'s roles stay the acts' entitlements (ADR-0060). Locally the choice is each viewer's own; a hub login's default is RM-0089's. The core names no trade: the gamedev preset is a file outside the crates (ADR-0008). ADR-0073.
