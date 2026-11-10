---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-11
---

# Workspaces: a trade's lens on a project

## State

The lens below holds; ADR-0076's parts are not built. v5 step 4 `departments` (RM-0096, `docs/features/departments.md`) builds the words, a flow's `title`, `kinds` directions and the hand-off by department. Until it ships the sections after "The lens" hold: the chrome says Workspace, card and Tasks, a module needs `root`, and a `title` in `[[card.flow]]` or `kinds` in `[[module]]` is an unknown key.

<a id="the-lens"></a>
## The lens

A workspace is how one trade sees a project, a **Department** in the chrome: the sections its nav lists, in its order, the section it lands on, the card flows its Board shows, and a default module. Its statuses are those flows' columns, each column's category from its place in its flow (`ui-design.md` "Status categories"); no table names a status. A module is a root document and its subtree, or a **direction**: the nodes of the kinds it lists, with no root. A rooted module roots the Tree and Flow reads; a direction is applied by the reads that take a kind (the Tree's list, a document's Inside), and Flow, which draws from a root, takes rooted modules only. Both tables are the project's config, read apart from every other read, as `[flow]`, `[card]` and `[team]` are: a bad table serves the implicit workspace, every section but Tasks, with a note, and fails nothing.

The chrome's words are the owner's: Department for a workspace, Task for a card, Subtask for a child card, Agent task for a `T-` task. Config keys, the CLI, MCP and the daemon's text keep `workspace`, `module`, `card` and `task`, with no alias key, since a new top-level key would make an older binary refuse the file. A `[[card.flow]]` may carry a `title`, its display name. A hand-off names a department; the card carries a discipline that the new-card rule places in one of that department's flows (`card-flows.md` "Config").

A workspace is never an entitlement, a control point, a filter that hides work or a hidden route. Every section keeps its hash route and its palette entry, every flow stays one Show all flows away, and a workspace changes no right, gate, finding or exit (ADR-0012). `[team]`'s roles stay the acts' entitlements (ADR-0060). Locally the choice is each viewer's own; a hub login's default is RM-0089's. The core names no trade: the gamedev preset is a file outside the crates (ADR-0008). ADR-0073, ADR-0076.

## Config

`[[workspace]]`, `[[module]]` in `specengine.toml`, read by core `WorkspaceConfig::from_toml` (`workspace_toml.rs`, `card_toml`'s family), pure, never failing: `{source: config|implicit, workspaces, modules, notes}`, notes `{line, message}` by line. `ProjectConfig` only name-checks both keys (`RawFile`).

| `[[workspace]]` | Rule; absent |
|---|---|
| `name` | required; `name_problem`'s `[a-z0-9_-]`, 1-64 bytes; once |
| `title` | any script, 1-64 UTF-8 bytes, no control character; the name |
| `sections` | at most 32 of `inbox`, `tasks`, `board`, `roadmap`, `flow`, `tree`, `graph`, `health`, `questions` (`SECTIONS`); the eight but `tasks` |
| `landing` | `overview` or a kept section; `overview` |
| `flows` | at most 32 `[[card.flow]]` names; `null`, every flow |
| `module` | a served module; `null` |

`[[module]]`: `name` as above; `title` as a workspace's; `root` required, 1-256 bytes, no control character or end space, a REF in `spec show`'s forms, unresolved. At most 32 workspaces, 64 modules; an empty array is no key. No `[[workspace]]`: the implicit `all`, "All sections", the eight, `overview`, every flow, no module, no note.

**Judging.** `[[module]]`, then `[[workspace]]`: a malformed table (not an array of tables, an unknown key, a wrong type, `name` or `root` missing, a bad name, title or root, a name twice, a limit passed) gives its default, no module or the implicit workspace, and one note, the first problem by line. Then each workspace: an unknown or repeated section or flow is left out, a landing not kept is `overview`, an unserved module `null`, each with a note; no flow left, or `flows = []` (`[card]` unread), is `null`. Flows are judged by `card_from_toml`; if `[card]` does not read and a workspace lists flows, the lists are served as written, repeats too, with one note. Not TOML: the implicit workspace, one note (`ProjectConfig` refuses it first). A note's line: the value's, a missing key's header's, else 1. `<config>` as given:

```text
<config>:<line>: <problem>; the implicit workspace is served
<config>:<line>: <problem>; no module is served
<config>:<line>: workspace `<w>`: `<s>` is no section (inbox, tasks, board, roadmap, flow, tree, graph, health, questions): left out
<config>:<line>: workspace `<w>`: section `<s>` is listed twice: the second left out
<config>:<line>: workspace `<w>`: no `[[card.flow]]` is named `<f>`: left out
<config>:<line>: workspace `<w>`: flow `<f>` is listed twice: the second left out
<config>:<line>: <card's problem>; the workspaces' flows are served unjudged
<config>:<line>: workspace `<w>`: landing `<l>` is not one of its sections: it lands on the Overview
<config>:<line>: workspace `<w>`: no `[[module]]` is named `<m>`: no default module
```

`<problem>`: the TOML reader's (a wrong-typed `name` or `root` too), `name_problem`'s, `` `[[workspace]]` is required to hold `name` `` (`[[module]]`, `root`), `` at most N `[[workspace]]` tables ``, `a list holds at most 32 entries`, `` `title` is N bytes long; a title has 1 to 64 ``, `` `title` holds a control character ``, both for `root` (1 to 256), `` `root` `x` has a space at an end ``, `` `[[workspace]]` `x` is named twice ``.

## Read

`spec workspace [--json]` (CLI `workspace.rs`): the config once (`discover_with_text`), no index, no write, no cut. Text: `workspaces <source> | workspaces <n>, modules <m>`; `workspace <name> | <title> | landing <l> | sections <a, b> | flows <a, b> | module <m>`, `(all)` for `null` flows, `(none)` for no module or sections; `module <name> | <title> | root <REF>` (card-press `expected.json`). JSON `{source, notes, workspaces: [{name, title, landing, sections, flows, module}], modules: [{name, title, root}]}`, every key, that order. Notes and lines are one line, controls escaped (a newline a space, ESC `\u{1b}`): `note:` lines equal JSON `notes`. Exit 0 whatever the tables; 2 on usage, no project, a config `ProjectConfig` refuses.

`GET /api/projects/:p/workspaces` (http `app.rs`): that JSON byte for byte; a query 400, not GET 405 `Allow: GET`, a refused config 503. **No turn**: config only, as `/api/projects` (`serve.md` "Concurrency"). A hub mounts it for every login, never reading `[team]`. Daemon key sets 111-113: `WorkspaceDocument`, `Workspace`, `WorkspaceModule`. No MCP tool: an agent works on documents and cards, not a viewer's nav.

## Screen

UI `src/workspace/`; `useWorkspaces` is read again on window focus (a config edit raises no event).

- **Nav** (`routes.ts` `navOf`): Overview, then the workspace's sections that pass `isSectionId`. Pending: a skeleton, `aria-busy`; failed: the nine, or the kept answer's. A section off the nav renders on its route, no redirect, no item `aria-current`; the palette lists all nine.
- **Keys** (`choice.ts`): `specengine.workspace.<p>`, a name, unserved: the first workspace. `specengine.module.<p>`: `""` All modules; a served name; none or unserved: the workspace's `module`. Choosing a workspace removes it. Storage refusing: held until a reload.
- **Switcher**: a select "Workspace" in the header, titles in served order, `workspaceHint` its description and `title`, with two or more. Choosing stores, removes the module key, pushes the landing (`#/<p>` for `overview`).
- **Landing**: `#/` is replaced, no history entry, by the first project's landing once read, `#/<p>` on failure; a typed `#/<p>` is the Overview.
- **Notes**: `Config notes (n)` beside the switcher or alone: the notes verbatim, in order, or a failed read's message and Retry; else none.
- **Module bar** (`ModuleBar.tsx`, the Tree's; Flow's, of links, in its places pane): when `modules` is not empty, never from the tree; group "Modules", All modules then titles, `aria-pressed`, wrapping at 320 px. Tree's root starts at the module's, reset on a change; Flow's lens root, the module also in its hash (`spec-flow-place.md`). Board, Tree, Flow draw once the read answers. A root naming nothing: the read's `reason`, the bar kept.
- **Board**: a tab per workspace flow the cards read serves, in its order; `null` or none served: all. Hidden ones: `Showing <k> of <n> card flows.` and Show all flows (then "Show this workspace's flows") for the visit, reset on a switch; a routed card in a hidden flow turns Show all flows on, once the cards are read.
- **Strings**: every chrome string is a key of the catalog's `workspace` group, named as `WORKSPACE_TEXT`'s members were (`ui-languages.md`); section labels: `app.sections`.

## Preset

`scripts/presets/gamedev.toml`: five `[[card.flow]]`, `narrative`, `development`, `art`, `level-design`, `production`, each its own discipline, the role research's columns, `accept` its last two, no `moves`, and the return column `sent-back`, titled "Returned", second in `columns`, `return = "sent-back"` (`card-flows.md` "Config"); its header says to add `return` only once every terminal, plugin and hub serving the project runs a build that reads it. No `[[module]]` (roots are a project's IDs). A project copies it, adds modules and each `module`, retitles. No name echoes `[team]`'s `lead`.

| Workspace | Sections | Landing | Flows |
|---|---|---|---|
| `writing` | flow, tree, board, questions, inbox | flow | narrative |
| `development` | board, inbox, tree, flow, graph, health, roadmap, questions | board | development |
| `art` "Art and levels" | board, flow, tree, questions, inbox | board | art, level-design |
| `production` | inbox, roadmap, board, flow, tree, graph, health, questions | overview | all |

## Guards

http `tests/no_domain.rs` takes each `[[workspace]]` and `[[module]]` name of the repository's `specengine.toml` files and `scripts/presets/*.toml`, and each preset flow's `return` column, as words; eval `tests/workspace_names.rs` finds none, nor a preset flow or `return` column, as a literal in `crates/*/src` or `ui/src` app code; `ui/src/policy.test.ts` none of the mock's. `ui_policy.rs` pins `SECTIONS` to `sections.ts`; `anonymity.rs` reads `scripts/presets`.

## Known limits

- **Older binaries.** `RawFile` is `deny_unknown_fields` (`crates/specengine-core/src/project_toml.rs`), so a `spec`, daemon or MCP server built before this step refuses a whole config holding either table. A hub at `v0.29.1` serving such a project fails every read with 503. The rule: **do not commit these tables to a hub-served project before its hub runs a build of this step**; every terminal and plugin upgrades first too. The gate's hook is unaffected (`docs/canon/gate.md`).
- One root per module: a kind selector is an unknown key. A choice is per browser. No `spec check` finding: only the notes.

## Tests

core `workspace_toml.rs`; CLI `workspace_cli.rs`; mcp `mcp_workspaces.rs`; http `workspaces.rs`, `turn.rs`, `hub.rs` (28 routes), `daemon_keys.rs`; the guards; `card-press`, `studio-lamp` (the preset verbatim); UI `src/workspace/`, `mocks/workspace*.smoke`. Not asserted: no cut, `sections (none)`, escapes.
