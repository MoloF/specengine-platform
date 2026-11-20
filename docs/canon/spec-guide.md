---
class: canon
tier: 2
scope: [docs, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-20
---

# spec guide: SpecEngine's own guide, served beside a project

ADR-0085, built by `specengine-guide` (RM-0129); the task spec, `docs/features/specengine-guide.md`, holds the shapes until it ships, when this page takes them.

<a id="the-rule"></a>
## The rule

**What the guide is.** SpecEngine's user guide is canon of this repository: one page per topic, `docs/guide/<topic>.md`, in English (ADR-0024).
- **Each page** says what the feature solves, when to use it, how (CLI, MCP, screen), where it lives in a project, and what not to do.
- **Each rule** is a `{#ID}` section under this repository's `GD` prefix.
- **`summarises:`** names the canon pages the page summarises. When one of them was reviewed after the page, `spec check` warns `summary-stale`: a warning, never an error.

**How it is served.** Every build of `spec`, `specengine-mcp` and `specengine-http` embeds the pages at compile time, and serves them read-only, with no project and no turn:
- `spec guide [TOPIC[#ID]]`;
- the MCP tool `get_guide`, on stdio and on a hub, and the resource template `guide://{topic}`;
- `GET /api/guide[/:topic]`;
- the "SpecEngine guide" shelf in Documents (`spec-library.md`).

**What it never does.**
- It never enters a project: no project file, index row, check, map, library row or ID space holds it.
- `spec init` copies nothing.
- No bundle, card or task read includes it unasked, so W is unchanged (`documentation-system.md` §3).
- A project cites a rule as `guide:<topic>#<ID>`, a form outside its `[ids]`, so the citation raises no finding.
