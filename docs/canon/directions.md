---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Directions: a module chosen by node kind

A direction is a `[[module]]` that lists node kinds instead of naming a root: the mechanics, the models, the scenes, so a person sees what an object consists of (`workspaces.md` "The lens"). ADR-0076, built by `departments` (RM-0096, v5 step 4); its table of documents, a read and a route of its own, by `direction-catalog` (RM-0106, `spec-catalog.md`). It adds no table, tool or store column. Code: core `workspace_toml.rs`; CLI `workspace.rs`; UI `src/workspace/{choice.ts,ModuleBar.tsx}`, `src/tree/{TreeView,SpecTree,RowContent,NodePane,Facts,DirectionCatalog}.tsx`, `src/tree/catalog.ts`, `src/doc/{inside.ts,DocInside.tsx}`, `src/flow/FlowView.tsx`.

## Config

```toml
[[module]]
name = "scenes"
title = "Scenes"
kinds = ["scene"]
```

A `[[module]]` holds exactly one of `root` and `kinds` (`MODULE_KEYS`: `name`, `title`, `root`, `kinds`); "both" is decided by the keys' presence, whatever their values. `kinds` is read through the shared list reader (at most 32 entries) and kept in written order. Each entry is checked in this order: 1 to 64 bytes (`MAX_KIND_BYTES`), no control character, no space at an end, listed once. A kind is the project's word: it is matched exactly against a node's `kind` and never checked against `[ids]`. A node with no kind matches none.

Each problem makes the `[[module]]` array malformed, as any bad module table does (`workspaces.md` "Config"): no module is served, and one note is given, the first problem by line, `<config>:<line>: <problem>; no module is served`:

```text
`[[module]]` `<n>` holds both `root` and `kinds`: a module holds one        at `kinds`' line
`[[module]]` is required to hold `root` or `kinds`                          at the table's line
`kinds` lists no kind; a direction lists 1 to 32                            at the value
a list holds at most 32 entries                                             at the 33rd entry
`kinds`: `<k>` is <N> bytes long; a kind has 1 to 64                        at the entry
`kinds`: a kind holds a control character                                   at the entry
`kinds`: `<k>` has a space at an end                                        at the entry
`kinds`: `<k>` is listed twice                                              at the second
```

A wrong type gets the TOML reader's message. When `name` is not a string, `` `<n>` `` is left out of the both-keys message; that message comes first only when such a `name` follows `kinds`. A workspace's `module` may name a direction. While no module is served, a workspace that names one gets "no `[[module]]` is named".

## Read

`spec workspace` text: `module <name> | <title> | kinds <a, b>`, beside a rooted module's `root <REF>`. In JSON, `modules[]` is `{name, title, root, kinds}`: `root` is null for a direction and `kinds` is null for a rooted module. `GET /api/projects/:p/workspaces` serves it byte for byte, with no turn; the daemon key set is 113 `WorkspaceModule`. On `fixtures/studio-lamp`:

```json
{"name": "shore", "title": "The shore", "root": "CHP-SHORE", "kinds": null}
{"name": "scenes", "title": "Scenes", "root": null, "kinds": ["scene"]}
```

The config read reads no node. The kinds are applied by the reads that take a kind.

## Tree

- **The bar** (`ModuleBar.tsx`). It holds two groups, each a `role="group"`; the bar's own element is not one. Group "Modules" holds All modules and then the rooted modules' titles. Group "Directions" (`workspace.directionsLabel`) holds each direction's title and is drawn only when a direction is served. One choice spans both groups, shown by `aria-pressed`, and it is stored in `specengine.module.<p>` as any module is. With directions only, Modules holds All modules alone.
- **The list.** `TreeView` takes a `moduleKinds` prop beside `moduleRoot`. While the view's root is the module's start and a direction is chosen, it reads `getTree({kinds, archive})` with no root: the read `spec tree --kind` serves. That read's `depth` and `parent` are the whole tree's (`spec-cli-graph.md`), so the rows are drawn flat, in served order. Each row's depth is taken as 0 before `buildRows`, so every row is `aria-level="1"` and none has `aria-expanded`. Flatness is read from the answer (no root, kinds given), so a placeholder answer is drawn as it was read. A flat tree is `tree tree-flat`: no chevron's column, no `.tree-toggle`. With one kind in the answer's `kinds` (`oneKind`, read as flatness is), `RowContent` draws no kind chip; two or more keep it. A rooted read keeps `tree`, the chevrons and the indent, 16 px a level.
- **The pane** (all of the Tree, not only a direction). The tree pane holds an sr-only `h2` (`app.sections.tree.label`) before the node's `h1`, so the view's outline opens with it, skips no level and keeps one `h1` (`ui-tree-node.md` AC-14, relaxed: the `h1` no longer leads); its empty states are `h3`. The node (`NodePane.tsx`) is an `IssueView` pane (`ui-design.md` "The work item"): Breadcrumbs Project (`useProjectName`), Spec tree, the REF current; one holder's raw `status:` a neutral `Lozenge`, `lang="en"`; Open document and Show in graph, subtle compact; Details, `tree/Facts.tsx` `factsOf(holder, t)`'s rows in a `PropertyList` with `foldEmpty` (a section's Status and Rev under More fields), several holders' each under its `h2`, labelled "Details at <path:line>"; main the notes, `CardsBlock`, the tabs. No node open: the view's `h1` and an `EmptyState`, level 2, or at a direction's start the catalog below in its place; none found: one in the pane's main, the empty side gone.
- **The catalog** (`DirectionCatalog.tsx`, a lazy chunk in a `Suspense` with a skeleton; the read: `spec-catalog.md`). At a direction's start with no node open, the node column holds, under the pane's `h1` (kept outside the chunk for the Shell's focus), the direction's table. `useCatalog(p, kinds, archive)`, key `["catalog", p, kinds, archive]`, keeps the last table while Include archive re-reads; `catalog` is in `READS_OF_THE_SPEC`, and `catalogsOf(p)` is re-read on every `card.*` and every `proposal.*` but a stage. Captioned "<title>: N nodes" (the Shell's `moduleTitle`, a `bdi`); columns Title, Version, State, Open items, Tasks, Last change. A row: the title (else the ID) a `DocLink` to the document modal, at least 18ch wide, breaking only between words, the ID below in mono; "Version N" or "No version"; a `Badge` of `rollupLook`, an unknown state raw `lang="en"`; the open items; tasks `0` when both counts are 0, else "N · M open"; small text, a `<time>` with "by" and `by_name` below it, or "Not committed" on `null`; no email, OID or proposal drawn. The lozenge, the ID, the version, the counts and the time never wrap. A shown section row adds one "(file)" hint under the table. **Sort**: each `<th>` a subtle compact `Button`, `aria-sort` on the sorted column alone; Title (`Intl.Collator`, the viewer's language, numeric) and State (`ROLLUP_WORDS`' order) ascending first, the rest descending; a second press reverses; missing values last both ways; ties in served order; a new direction resets it; the one state compare in `tree/catalog.ts`. 50 rows (`CATALOG_SHOWN`), then "Show N more", focus to the first newly shown title; a sort keeps the count. On a narrow pane the table scrolls sideways in its own focusable region, labelled by the caption, never the page. Loading a skeleton; a failure an `ErrorPanel` in the daemon's words with Retry; no row an `EmptyState`, level 2; `notes` a list, `lang="en"`. Strings `tree.catalog.*`, `tasks` and `more` plural; CSS `.catalog-*`.
- **Moving.** Show as root on a row leaves the list for a rooted read. Whole tree returns to the start, which is the direction's list. All modules reads the whole tree. Kinds that match no node show the read's empty answer.

## Inside

`insideOf(tree, directions)`, where `insideDirections(modules)` gives the directions in served order. They come from the Shell's held workspaces read (`useHeldWorkspaces`, which never fetches). Within each holder (`doc-view.md` "Inside"), a child whose kind a direction lists joins that direction's set, under the first such direction in served order. Direction sets come first, in served order, each a `role="group"` labelled by its title in a `bdi` with no `lang`. Kinds that no direction lists stay raw groups (`lang="en"`, code-point order). `InsideGroup.sets` holds `InsideSet`, either `{by: "direction", name, title, entries}` or `{by: "kind", kind, entries}`. While the workspaces read is pending or has failed, the groups are raw kinds at once, never waiting. No request is added to the one `getTree({root, depth: 2})`. Direction titles are the words for kinds; link types stay raw (ADR-0031).

## Flow

Flow draws from a root, so it takes rooted modules only (`spec-flow-place.md` "The module"). The places pane lists rooted modules. A stored direction reads as All modules, and its key is left as it is. A hash naming a direction counts as unserved and is replaced: the written hash names the module in force, `module=` (All modules) when a rooted module is served, and no key otherwise. A Flow drawn through a direction is RM-0102, in the pool.

## Rules

- When two directions list a kind, Inside puts the node under the first in served order, and the Tree's list of each direction lists it.
- Nothing branches on a direction's name, title or kinds (ADR-0008).
- **Scale.** A direction's list is today's whole-tree walk with a line filter, about 956 ms at 100k for `get_tree` (`scale-eval.md` "Baseline"), the same cost as All modules. Inside's grouping reads nothing. The catalog is one more read, its cost `spec-catalog.md` "Known limits".

## Guards

- http `no_domain.rs` takes every `kinds` entry of the repository's configs and presets, lowercased; on this repository its reader returns exactly `{"scene"}`.
- eval `workspace_names.rs` checks that no entry appears as a literal in `crates/*/src` or `ui/src` app code. Its scratch control, `kinds = ["quarry"]` against `'quarry'` in a probe `.tsx`, is red.
- `policy.test.ts` bans the mock's direction name and kinds, quoted, in app code.

## Known limits

- **Older binaries.** A build before `departments` reads `kinds` as an unknown key. It then drops every module with one note, and each workspace naming a module gets its own note. ADR-0073's upgrade rule holds (`workspaces.md` "Known limits").
- Flow draws no direction (RM-0102). A direction is chosen by kind alone, never by status, path or tag.
- `oneKind` follows the drawn answer, not `moduleKinds`: while another direction's read is pending, the last answer's chips stand. The mock serves no two-kind direction, so its chips are seen in `TreeView.test.tsx` alone.
- **The catalog's words**: "Not committed" also stands for not reached and git not read; the notes say which (an open wording question). The mock's Mechanics (`mocks/catalog.ts`) holds 130 rows, harbor-sim's real mechanics and invented `MEC-CAT-*` ones the left list lacks, their titles opening the modal's "names nothing"; `?scenario=empty` an empty direction; no mock project has the kind `scene`, so the pinned scenario renders only in stub tests.

## Tests

- core `departments.rs` (DP-2, DP-3), `workspace_toml.rs`;
- CLI `departments.rs` (`spec tree --kind scene`: `SCN-ARRIVAL`, `SCN-STORM`), `workspace_cli.rs`;
- http `workspaces.rs`, `daemon_keys.rs`, `no_domain.rs`; eval `workspace_names.rs`;
- `fixtures/studio-lamp`, whose third module is `scenes`;
- UI `tree/{TreeView,NodePane,Facts}.test.tsx`, `mocks/workspaces.smoke.test.tsx` (harbor-sim's `mechanics`), `doc/inside.test.ts`, `doc/DocBody.test.tsx` (`docStub`'s "Records", "Asked"), `flow/FlowView.test.tsx`, `policy.test.ts`; the catalog's: `spec-catalog.md` "Tests".
