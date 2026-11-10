---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: RM-0095 with RM-0091 and RM-0101; v5 step 3 (08 s2 Phase 7, "v5 scope"); Flow v2 step 3d and the owner's answers of 2026-11-10; Flow v2 Q3 and Q4 at their recommendations; DM-1 to DM-22
adrs: []
---

# doc-modal: one document view everywhere, a modal and a page, the path its breadcrumb (RM-0095)

## Why

The owner looked at his own project on 2026-11-10. In Flow's right panel a document came last, after kind, distance, `path:line`, inner counts and gap codes, in a column too narrow to read. His words: he wants one universal modal, like Jira's, holding all of a document's information; a minimal right side; nothing technical for people ("this is information for the AI"). Asked where, he answered "everywhere": Flow, the Spec tree, the Jump-to palette, links in cards and questions. A Cmd-click opens the document's own page, as it does for cards.

Today:
- **Every screen links into the Spec tree's node pane**, the technical view: kind, place, hashes, five tabs. 36 call sites in 27 files build `sectionHash(…, "tree", …)` by hand.
- **Links show names, not titles.** `show --links` serves a link's other end by ID or path only, so a screen must read each end to title it.
- **What an object consists of** (the documents under it and under its sections) shows only as Spec tree rows.
- **English slips** (RM-0091): "1 links not shown", "1 tokens".
- **Gaps reads the whole tree** (RM-0101) to title a member folded into a group.

This slice gives one document view, served titles at both ends of a link and at gap members, and a Flow panel that only points at the document. It changes no rule of `docs/canon/architecture.md`, so it has no ADR.

## Description and interactions

### Decisions

- **D1. One view, two frames.** The document body renders in a modal over any screen and on a page, `#/<p>/tree/<REF>/page`. The Spec tree's node view (`#/<p>/tree/<REF>`) stays the technical view, and its own links keep moving the tree (Flow v2 Q3).
- **D2. One hash key, `doc`.** The modal is the pair `doc=<REF>` after a `?` on any project route, so a reload, a shared link, Back and Forward reopen it (Flow v2 Q4). `parseHash` splits the first raw `?` on every route, not only Flow's.
- **D3. The card modal's manners.** History, focus, Esc, Close, the scrim and Back behave as the card modal's do (`docs/canon/board-screen.md` "The card modal and page"). The document modal is always the top dialog, over the card modal and over Flow's Records dialog too.
- **D4. Link ends are titled.** Each link object of `show --links`, `get_node` and the daemon's nodes read gains `title` and `end_kind`: the other end's (`name`'s) node title and kind, `null` when unresolved or untitled. `kind` is taken: it is a labelled edge's (`alternative`, `parallel`, `repeat`). JSON only; the text is byte-equal. `spec health`'s amendment links share the object and gain both.
- **D5. RM-0101 rides along.** Each gap row serves `titles`, parallel to `items`. The Gaps control titles members from it and drops its tree read. Both keys are keys a tool returns: one plugin MINOR, 0.34.0, for D4 and D5.
- **D6. One link component, `DocLink`.** Its `href` is the page; a plain click opens the modal (a push from a screen, a replace inside the modal); a modifier, middle or context click is the browser's. 22 call sites become it, 12 stay Spec tree links on purpose, 2 leave with the old panel ("Data", "Call sites"); `policy.test.ts` pins the twelve, with the tree branches `Markdown` and `DriftPart` keep.
- **D7. The sections**, in order: head (breadcrumb from `ancestors`, title, work state, raw `status:`), acts, text at reading width, links in and out by title, cards, Inside, Technical details folded.
- **D8. Inside reads the tree.** `getTree({root: REF, depth: 2})`, made on the first expansion of a collapsed "Inside", never on open. A flow read is documents only and cannot say which section a child names; the tree read can (`parent` is the name listed under). Children group by the holder they are listed under (the document, then each of its sections), then by raw `kind`. RM-0096 adds directions as a third level. Inside covers the "Records for the AI" of RM-0095's item text: records are children.
- **D9. Flow's panel is minimal**: title, stage, work state, marks, the first paragraph, at most three acts, and Flow's own facts folded. The whole text, links and cards move into the modal. The panel stops reading `getTrace({ref: N})`: its work state and marks come from the drawing read.
- **D10. RM-0091 folds in**: thirteen English `_one` forms that read wrong at 1 ("Data", "RM-0091"), each pinned by a test.
- **D11. Size.** The document body (`DocBody`) is a lazy chunk: first-load JS stays under the 845 kB bar (815.13 kB today).
- **D12. Link types stay raw.** Types are free strings that app code never quotes (ADR-0031, `ui/README.md` "Screen rules"), so the modal shows `follows`, `uses` as data, `lang="en"`, in a neutral lozenge. Words for them need a served label: the owner's question Q-A.

### The contract (Rust lane)

- **Link ends.** CLI `links.rs` `node_links` fills `ShownLink.title` and `ShownLink.end_kind` from the node `name` names: `Endpoint::Nodes`' first node, outgoing the target, incoming the source, the same one `name` reads; `None` for `Dangling`, `Skipped`, `Unchecked`, and for a node with no title or kind. `LinkJson` prints both, always present, right after `name`. The text block is unchanged.
- **Gap titles.** Core `flow.rs` `FlowGap.titles: Vec<Vec<Option<String>>>`, built beside `items`, the member document's title. The CLI's JSON prints `titles` after `items`, cut with `items` at `GAP_ITEMS_SHOWN` (50). The text is unchanged.
- **Twins.** MCP `get_node` and `get_flow`, and the daemon's `GET /api/projects/:p/nodes/<REF>?with=links` and `/flow` serve the CLI's document, so they change with it. The MCP mirror (`mirror.rs`) gains `ShowLink.title`, `ShowLink.end_kind`, `FlowGap.titles`, so the schemas' hash moves.
- **Plugin** 0.34.0 (`plugin.json`, `PINS`, `SCHEMA_PINS`, `scripts/deploy/.env.example`). No skill or role text changes: `read-spec` is at its cap, and the schema describes the keys.

### The view (UI lane)

- **Routes.** `doc` is read on every project route; the page `#/<p>/tree/<REF>/page` routes like the card's page. Hash writes keep an open modal: `replaceHash` carries the current `doc` pair unless the modal itself closes; a push drops it.
- **The modal** (`src/doc/DocModal.tsx`) mounts in the Shell after the view when the route has `doc`: a `Dialog`, `aria-modal`, labelled by its `h2` (the title once read, the REF until then), focus on it, 968 px or the viewport less 2rem, the screen under 36rem, its body scrolling. Tools on the breadcrumb line: Open as a page, Close. The view and any dialog beneath are inert.
- **The page** (`src/doc/DocPage.tsx`): the same body, its `h1` the title, no tools; the nav marks Spec tree; a reload keeps it.
- **The body** (`src/doc/DocBody.tsx`, lazy) is an `IssueView`:
  - **Head.** Breadcrumbs Project (`#/<p>`), each ancestor top first by title (else ID, else path), the document current. An ancestor crumb's `href` is its page; in the modal its plain click replaces `doc` (`onFollow`); on the page it follows the link. Under the title, `meta` (a new optional `IssueView` slot): the work state, a `Lozenge` from the node's trace row (`rollupLook`), none while reading, "not known" with no row, as the Cards block says; the raw `status:`, a neutral `Lozenge` under `lang="en"`, none for a section. Acts: New card (primary, `NewCardDialog` preset to the document), Open in Flow, Open in Spec tree.
  - **Main.** A REF that several holders answer: the first holder, and the line `doc.holders`. Then **Text**: the body without its front-matter block and without a first heading equal to the title, through `src/markdown/` at `--font-size-document` and `--reading-width`; headings one level below the frame's title; anchors as `DocLink`s; then `doc.throughAi`. **Links** (`DocLinks.tsx`): Out, then In, each the served strong links whose `state` is `resolved`, in served order, a row the raw type then the end's `title` (else `name`) as a `DocLink`; `mentions` both ways under a collapsed `Disclosure` with their count; none at all, `doc.links.none`. **Inside** (`DocInside.tsx`): "Data", "Inside".
  - **Side**, at most 330 px. **Cards**: `CardsBlock` (heading one below the frame's title, its own New card off, since the head holds it). **Technical details**, a collapsed `Disclosure` (`ui.technicalDetails`): the ID with a copy button (ID-less: the path); `Facts` (moved out of `NodePane` into `src/tree/Facts.tsx`, shared): kind, `path:line-end`, status, rev, tokens, span hash, flags; the norm hash; the front-matter block verbatim; the links not resolved, each `<type> <written>` and its state and reason under `lang="en"`; `left_out`; the read's notes under `lang="en"`; `doc.technical.inTree` and Open in Graph.
- **Open in Flow** is `flowHash(p, {module: null, scope: <the last ancestor's name, null at a root>, stage: null, select: {node: <name>}})`; Flow's own rules place it ("Rules and edge cases"). **Open in Spec tree** is `sectionHash(p, "tree", <name>)`. **Open as a page** is `docPageHash(p, <REF>)`. Each is a push, so the modal closes.
- **`DocLink`** replaces the 22 sites listed in "Data", "Call sites". In the Spec tree, links stay tree links: `Markdown` and `DriftPart` keep a tree branch, taken there. `NodePane` gains an Open document act, which opens the modal over the tree. The palette's `ref` and `hit` options open the modal over the current screen.
- **Flow's panel** (`FlowPanel.tsx`):
  - **A box**: `backTo(scope)` on top, unchanged. Then its title (`h2`); a line with its stage, its work state (a `Lozenge` from the drawing read's `rollup`) and the marks the canvas gives it (suspect, departed, Gap), each a label and an icon. Then its text's first paragraph (the first top-level `paragraph` that `src/markdown/` parses from the body, anchors as `DocLink`s, none when there is none). Acts: Open document (primary), New card, and for a group its existing `flow.openGroup` act. A box whose `group` is not the scope shows that group by title as a place link (a `flowHash`). Last, Technical details, collapsed: name, kind, place, distance, start and end, `inner` per type.
  - **The scope** (nothing selected): its title (`flow.topTitle` at Top), its counts line and bar, the first paragraph, Open document (primary), New card, Technical details (name, kind, path). An unrooted Top has no document and no acts.
  - **Gone from the panel**: the rest of the text, `LinksPanel`, `CardsBlock`, Open in Spec tree, Open in Graph, `flow.throughAi`, and the box read `getTrace({ref: N})`. An edge's panel is unchanged.
- **Gaps** take each member's title from the read's `titles`, else its name: no `getTree` while the control is open (`useTreeOnDemand` leaves Flow).
- **Records for the AI**: an entry is a `DocLink`, so it opens the modal over the dialog. The ID-less line still links the Spec tree.
- **RM-0091**: the forms in "Data", "RM-0091" are fixed in `en.ts`. Russian keeps its forms. Every test that expected the old English expects the new.
- **Both languages**: every new string is a `doc.` key, or a `flow.`, `tree.` or `ui.` key in the directory that shows it. `src/doc/` is scanned by `policy.test.ts` from the start. The daemon's words (a 404 reason, notes, a link's `reason`) stay verbatim under `lang="en"`.
- **Tokens**: the doc CSS takes tokens only. `SCALE_EXCEPTIONS` loses its `until: "RM-0095"` entry (`.md-url, .md-image-path` `0.92em`, now a type token), and the `until` type loses `"RM-0095"`.

## Data

### `spec show --links` JSON: a link

`{type, origin, at, name, title, end_kind, written, path, line, state, reason}`, then `kind` and `when` when labelled (unchanged). `title` and `end_kind` are the `name` node's `title` and `kind`; both are `null` when `state` is not `resolved`, or when that node has none. On `fixtures/flow-aside`, `spec show POT-BISQUE --links --json`, the first outgoing link and the first incoming one:

```json
{"type": "follows", "origin": "frontmatter", "at": null, "name": "POT-DRY", "title": "Drying on the racks", "end_kind": "potstep", "written": "POT-DRY", "path": "docs/workshop/run/bisque.md", "line": 10, "state": "resolved", "reason": null}
{"type": "depends_on", "origin": "frontmatter", "at": null, "name": "PUZZLE-0003", "title": "A puzzle of a cone that bends late, the first", "end_kind": "puzzle", "written": "POT-BISQUE", "path": "docs/puzzles/PUZZLE-0003.md", "line": 9, "state": "resolved", "reason": null}
```

The other three: `uses WBAY-KILN` ("Kiln bay", `workbay`), `uses CONE-BISQUE` ("Bisque cone", `conerule`), `in follows POT-GLAZE` ("Dipping in glaze", `potstep`). The text block stays `  out follows POT-DRY | docs/workshop/run/bisque.md:10`.

### `spec flow` JSON: a gap row

`{code, state, count, items, titles, cause}`: `titles[i][j]` is the title of `items[i][j]`, else `null`; `[]` where `items` is. On `flow-aside`, `spec flow ATL-POTTERY --depth 1 --in-flow --json`, the members are folded into `KRUN-FIRING` and `WBAY-GALLERY`, and their own titles are served:

```json
{"code": "two-starts", "state": "measured", "count": 1, "items": [["POT-WEDGE", "docs/workshop/gallery/open-day.md"]], "titles": [["Wedging", "Open day"]], "cause": "more than one start: narrow [[flow.start]] to one"}
```

### Key sets and the plugin

- `fixtures/daemon-keys.json`: still 115 sets. `ShownLink` becomes `["type", "origin", "at", "name", "title", "end_kind", "written", "path", "line", "state", "reason"]`; `FlowGap` becomes `["code", "state", "count", "items", "titles", "cause"]`.
- MCP mirror: `ShowLink.title: Option<String>`, `ShowLink.end_kind: Option<String>`, `FlowGap.titles: Vec<Vec<Option<String>>>`, each required and nullable.
- `plugin.json` 0.34.0; `PINS` and `SCHEMA_PINS` gain 0.34.0; `.env.example` `SPECENGINE_VERSION=0.34.0`. `docs/canon/plugin-files.md` "Version" gains: 0.34.0 `get_node`'s link `title`, `end_kind`, and `get_flow`'s gap `titles`.

### The hash

```text
#/<p>[/<section>[/<id>[/page]]][?<pair>[&<pair>]...]
doc=<ref>    the document modal, on any project route; <ref> through encodeURIComponent
/page        after `board/<C>` (a card) and `tree/<REF>` (a document) only
```

- `parseHash` splits the first raw `?` on every route. The first `doc` pair whose value decodes (`decodeURIComponent`) to a non-empty string is `doc`, on the project and section routes. Every `doc` pair is removed before Flow reads its pairs: `query` is the rest joined by `&`, absent when the `?` held only `doc` pairs, so `#/<p>/flow?doc=X` is a bare Flow under the modal. Other sections ignore other pairs. A raw `?` therefore ends a Tree id too; `sectionHash` never writes one.
- `withDoc(hash, ref)` removes every `doc` pair and appends `doc=<enc>` after `?`, or `&` when pairs remain. `withoutDoc(hash)` removes every `doc` pair, and the `?` when none remain. `docOf(hash)` is `parseHash`'s `doc` or `null`.
- `docPageHash(project, ref)` is `` `${sectionHash(project, "tree", ref)}/page` ``. `parseHash` gives `view: "page"` for exactly `board/<C>/page` and `tree/<REF>/page`; any other fourth segment names nothing.
- `Route`: `{type: "project", project, doc?}`; `{type: "section", …, view?: "page", doc?}`.
- Examples, on the mock: `#/harbor-sim/board/C-0003?doc=MEC-MOORING`; `#/harbor-sim/flow/MEC-MOORING?module=&scope=DOM-BERTHS&doc=MEC-MOORING`; `#/harbor-sim/tree/DOM-HARBOR/page`; `#/harbor-sim?doc=DOM-HARBOR%23RULE-HARBOR-CLOCK`.

### `src/app/location.ts` and `src/doc/`

```ts
export function replaceHash(hash: string, options?: { keepDoc?: boolean }): void; // keepDoc defaults to true: an open `doc` pair is carried over
export function openDoc(ref: string): void;  // push withDoc(current, ref) when no modal is open; replace when one is
export function closeDoc(): void;            // history.back() when this Shell pushed the modal's entry, else replaceHash(withoutDoc(current), {keepDoc: false})
export function DocLink(props: { project: string; docRef: string; children: ReactNode; className?: string; tabIndex?: number; draggable?: boolean; ref?: Ref<HTMLAnchorElement> }): ReactNode;
export interface InsideEntry { name: string; title: string | null; kind: string | null; path: string }
export interface InsideGroup { holder: string; title: string | null; direct: boolean; kinds: { kind: string | null; entries: InsideEntry[] }[] }
export function insideOf(tree: TreeView): { groups: InsideGroup[]; noId: number };
export const INSIDE_SHOWN = 50;
```

`DocLink` renders `<a href={docPageHash(project, docRef)}>`. On a click with button 0, no Meta, Ctrl, Shift or Alt, and not `defaultPrevented`, it calls `preventDefault` and `openDoc(docRef)`, and records itself as the opener. Any other click, `auxclick` and `contextmenu` stay the browser's.

### Inside

`insideOf(tree)`, over `getTree({root: REF, depth: 2})`:
- **Own nodes**: the rows whose `path` is a depth-0 row's path, that is the document and its nested sections.
- **Children**: the rows whose `path` is not an own path and whose `parent` is an own node's name. Deeper rows are dropped: a child's own children show in its own Inside.
- **Groups**: one per own node that holds a child. The document comes first (`direct: true`, `doc.inside.direct`), then its sections in tree order, each `doc.inside.under` with the section's title, else its ID.
- **Kinds**: inside each group, by raw `kind`, sorted by code point, `null` last (`doc.inside.noKind`). Entries stay in tree order.
- **ID-less children** are counted in `noId`, not listed: one line, `doc.inside.noId`, linking the Spec tree at REF.

Each group is a `section` with a heading one level below Inside's. Each kind is a `role="group"` labelled by its raw kind under `lang="en"`. An entry is a `DocLink` by title, else ID. A kind shows its first `INSIDE_SHOWN`, then `doc.inside.showAll`, which moves focus to the 51st.

States:
- reading: a skeleton, `doc.inside.reading`;
- failed: `doc.inside.failed`, the daemon's words under `lang="en"`, Retry;
- no child: `doc.inside.none`.

The expanded choice lives in the Shell per project for the page's life (`DocState {insideOpen}`), so a viewer who keeps Inside open pays its read on each document opened.

On `parent-ref`, `spec tree NEST-01 --depth 2` gives `NEST-01`, its section `NEST-02` and `AUK-01` under `NEST-02`: one group, "Under Upper ledge", kind `sighting`, one entry. On `flow-aside`, `WBAY-KILN` gives 146 rows. Its Inside is one direct group of 144: `conerule` 2, `edict` 32, `hunch` 60 (then Show all 60), `puzzle` 50. `CONE-GLOST-SHELF`, a section of a child, is not listed.

### Reads

| Part | Call | When |
|---|---|---|
| Document | `getNode(REF, {with: ["links"]})`, its first node; `ancestors` the breadcrumb | on open; the subset read (`spec-read-subset.md`), about 56 ms at 100k |
| Work state, Cards | `getTrace({ref: REF})`, as the Spec tree's `CardsBlock` reads it | on open |
| Inside | `getTree({root: REF, depth: 2})` | on the first expansion, per open |
| Flow box N | `getNode(N, {with: ["links"]})` (the modal's key, so Open document draws at once) | N selected; `getTrace({ref: N})` no longer |

Each read uses the existing `node`, `trace` and `tree` keys of `READS_OF_THE_SPEC`, so an apply, a gap or a card event re-reads an open modal as it does the Spec tree. There is no new query key.

### Call sites

Today `sectionHash(…, "tree", …)` appears 36 times, outside tests, in 27 files.

**`DocLink`, or `openDoc` for the palette and the keyboard (22 sites):**
- `tasks/OverviewPanel.tsx` (3), `tasks/SpecChangesPanel.tsx`;
- `inbox/ProposalCard.tsx`, `inbox/InterpretationSection.tsx` (keeping `ref={keep}`);
- `board/BoardCard.tsx` (keeping `tabIndex={-1}`, `draggable={false}`), `board/CardPanel.tsx` (a card's targets, stacking);
- `roadmap/RoadmapLanes.tsx`;
- `markdown/Markdown.tsx` (outside the Spec tree);
- `health/AssumesPart.tsx`, `health/AmendmentsPart.tsx` (2), `health/UnboundPart.tsx`, `health/parts.tsx` with `health/FindingsRegion.tsx`'s Enter (`openDoc`), `health/DriftPart.tsx` (on Health);
- `graph/GraphList.tsx`;
- `round/RoundItemCard.tsx`;
- `palette/options.ts` (2, `openDoc`);
- `flow/FlowRecords.tsx` (an entry).

**Spec tree links, kept (12, the `policy.test.ts` allowlist):**
- the tree's own navigation: `tree/TextPanel.tsx`, `tree/LinksPanel.tsx`, `tree/TreeView.tsx` (2), `tree/BundlePanel.tsx` (2);
- explicit acts: `roadmap/RoadmapView.tsx` (`roadmap.openInTree`), `graph/GraphDetails.tsx` (Open in Spec tree), `flow/FlowView.tsx`'s lost-name link;
- the `o` key: `graph/GraphView.tsx`, `flow/FlowView.tsx`;
- `flow/FlowRecords.tsx`'s ID-less line (owner's answer: ID-less documents live in the Spec tree).

`Markdown` and `DriftPart` keep a tree branch for their use inside the tree: the allowlist is these twelve plus those two, 14 calls in 11 files.

**Removed (2):** `flow/FlowPanel.tsx`'s two Open in Spec tree links.

**New `DocLink`s:** the modal's links, Inside, the prose anchors, and the Records entries.

### Stub `src/test/docStub.ts` (new, invented)

A river ferry, served by a stub client:
- **`DOM-FERRY`**, "The ferry", `domain`, `docs/ferry/ferry.md`, `status: accepted`, `ancestors` `[DOM-RIVER "The river"]`. Its text: front-matter, `# The ferry`, the paragraph "A flat boat crosses the river on a cable.", the paragraph "It runs from dawn to dusk.", then `## Tides {#RULE-FERRY-TIDES}` with "The ferry waits for slack water."
- **Its links**: out `uses MEC-BOARDING` ("Boarding", `mechanic`); out `mentions DOM-RIVER`; out `uses MEC-GANGWAY`, dangling; in `follows` from `MEC-LANDING` ("Landing").
- **Its trace row**: in work, one card, `C-0007`.
- **The tree read** `{root: "DOM-FERRY", depth: 2}`:
  - `DOM-FERRY`, and its section `RULE-FERRY-TIDES`;
  - under the section: `ASM-0001` "The tide table is right" and `ASM-0002` "Fog stops the ferry" (`assumption`), and `QST-0001` "Who rings the bell?" (`question`);
  - `MEC-BOARDING` (parent `DOM-FERRY`), and under it `ASM-0003` (a grandchild);
  - `docs/ferry/notes.md`, ID-less, parent `DOM-FERRY`.

### Catalog keys (English; Russian in `ru.ts` alone)

| Key | English |
|---|---|
| `doc.reading`, `doc.failed` | Reading the document… / The document could not be read. |
| `doc.openPage`, `doc.close`, `doc.closeWhat` | Open as a page / Close{{what}} / " the document" |
| `doc.newCard`, `doc.openFlow`, `doc.openTree`, `doc.openGraph` | New card / Open in Flow / Open in Spec tree / Open in Graph |
| `doc.workState`, `doc.status` | Work state / Status (visually hidden labels before each lozenge) |
| `doc.holders_one`, `_other` | {{ref}} is declared in {{count}} place; this shows the first. / … {{count}} places; … |
| `doc.text`, `doc.throughAi` | Text / Changes go through your AI, as proposals. |
| `doc.links.heading`, `.out`, `.in`, `.none` | Links / Links out / Links in / No links. |
| `doc.links.mentions_one`, `_other` | Mentions ({{count}}) |
| `doc.inside.heading`, `.direct`, `.under`, `.noKind` | Inside / Directly under this document / Under {{title}} / No kind |
| `doc.inside.noId_one`, `_other` | {{count}} document without an ID is in the Spec tree / {{count}} documents without an ID are in the Spec tree |
| `doc.inside.none`, `.reading`, `.failed` | Nothing is listed under this document. / Reading what is inside… / What is inside could not be read. |
| `doc.inside.showAll_one`, `_other` | Show all {{count}} |
| `doc.technical.id`, `.norm`, `.frontMatter`, `.notes` | ID / Norm hash / Front-matter / Notes |
| `doc.technical.unresolved_one`, `_other` | {{count}} link does not resolve / {{count}} links do not resolve |
| `doc.technical.inTree` | Code bindings, the context bundle, proposals and the lock diff are in the Spec tree. |
| `tree.node.openDoc`, `flow.openDoc` | Open document |

Keys that the panel change leaves unused are removed (`flow.textHeading`, `flow.linksHeading`, `flow.throughAi`, and others the parity test names).

### RM-0091

Each English `_one`, fixed; `_other` is unchanged.

| Key | `_one` becomes |
|---|---|
| `board.view.omitted` | {{count}} card not shown. |
| `tree.links.omitted` | {{count}} link not shown |
| `tree.binding.tokens` | {{count}} token |
| `graph.holders` | {{count}} holder |
| `tree.node.holders` | {{count}} holder: {{ref}} is declared in one place. It is shown below. |
| `graph.details.moreText` | The column at distance {{column}} holds {{count}} more box than the canvas draws; the List holds it. |
| `tree.view.cut` | Showing the first {{count}} node: the list was cut |
| `tree.commands.verifyOnly` | Only verify is offered: {{ref}} names {{count}} node, and {{drift}} and {{accept}} each name one. |
| `graph.limit` | The canvas shows distance 0-{{last}}: {{shown}} of {{count}} node; see the List. |
| `flow.notDrawn` | {{hidden}} of {{count}} box not drawn: choose a stage, enter a group, or read the List |
| `roadmap.notDrawn` | {{hidden}} of {{count}} item not drawn |
| `workspace.flowsShown` | Showing {{shown}} of {{count}} card flow. |
| `health.findings.totalShown` | {{shown}} of {{count}} finding shown, by code, errors first |

The other 45 groups whose `_one` equals `_other` read right at 1 ("{{count}} found", "Show all {{count}}").

## Rules and edge cases

- WHEN a route has a `doc` pair and names a project that is read, the system SHALL draw the view and the modal over it. On `#/` and on not-found routes there is no modal. The `#/` replacement to the first project's home drops `doc`.
- WHEN a screen replaces the hash while the modal is open (Flow's bare-hash replacement, a seek, a correction, a module drop), the `doc` pair SHALL survive. Only `closeDoc` and pushes drop it.
- WHEN a `DocLink` is followed inside the modal (its links, Inside, prose, an ancestor crumb) or the palette picks a document while the modal is open, the system SHALL replace `doc`: one history entry for the modal however far one walks.
- WHEN Esc, Close or the scrim closes the modal, the system SHALL go back if this Shell pushed its entry, else replace with `withoutDoc`. Focus SHALL return to the opener while it is connected and not inert, else to the view's `h1`, or, under a card modal, to that modal's title. Back closes it; with a dialog over it (New card), Back closes both. Esc closes only the top dialog, never mid-IME.
- WHEN the card modal mounts after the document modal (a reload of `#/<p>/board/<C>?doc=<R>`), the document modal SHALL stay on top and the card modal inert beneath it.
- WHEN a write dialog over the modal (New card) has a submit out, nothing SHALL close (`ui/README.md` "Screen rules", Dialogs).
- WHEN `getNode` answers 404, the modal and the page SHALL show the daemon's reason verbatim under `lang="en"`. Close and Open in Spec tree stay; the other acts are absent.
- WHEN the REF is a section, the body SHALL be the section's span, the status lozenge absent, Inside rooted at the section.
- WHEN the body's first heading, without its `{#…}` attribute, equals the title, it SHALL NOT be rendered. A different first heading renders.
- WHEN a link's `title` is null, the row SHALL use its `name`. An unresolved link SHALL appear only under Technical details. The link type SHALL be shown raw, never translated or branched on (ADR-0031).
- WHEN Open in Flow names a document the cut leaves out, Flow SHALL behave as for any undrawn selected name (`spec-flow-place.md` "The seek"); this slice does not change Flow's seek (Known limit).
- The core gains no domain word (ADR-0008). The UI branches on no kind, status, link type or rollup word but through `labels.ts` (`policy.test.ts`). No spec write: changes go through the AI (ADR-0004), as `doc.throughAi` says.
- **Scale.** The modal opens on the subset read (O(k), about 56 ms at 100k: `scale-eval.md` "Baseline", `get_node_links`) and the trace read the Spec tree already pays. Inside is the one O(N) read: a rooted tree walk, about 1 s at 100k (`get_tree` 956 ms). It is acceptable because it runs only on a person's expansion, never on open or in Flow's panel, behind its own skeleton, and nothing else in the modal waits for it. On a hub it takes one project turn. If a real corpus measures it over 1 s, the follow-up is a served list of children from the index's `PARENT` rows, as a new item. Link-end and gap titles come from parses the reads already hold: no new read.

### Test churn

Only these change; every other test passes unchanged. The test-engineer adds what is missing under "Implementation".

| Test | Becomes | Why |
|---|---|---|
| CLI `links.rs`, `show.rs`, `health.rs`, `common/keys.rs`; goldens of `flow_linear.rs`, `trace_views.rs`, `store_reads.rs`, `read_trust.rs`, `tested_reads.rs` | link keys gain `title`, `end_kind`; goldens compared after removing exactly them and gap `titles`; the text is never stripped | D4, D5 |
| core and CLI `flow_read.rs`, CLI `flow_scenario.rs`, MCP `mcp_flow.rs` | gap keys gain `titles` | D5 |
| MCP `mcp_read.rs`, `plugin_files.rs`, `plugin_skills.rs`, `plugin_roles.rs`; HTTP `daemon_keys.rs`, `spec_ids_hub.rs`; `fixtures/daemon-keys.json` | the two key sets; 0.34.0 | D4, D5 |
| UI `api/daemonKeys.test.ts` | the two sets | D4, D5 |
| `app/routes.test.ts` "splits the first raw `?` for Flow alone" | `?` ends every path; `doc` | D2 |
| tests that click a document link and expect `#/<p>/tree/<X>`: `palette/Palette.test.tsx`, `board/*`, `tasks/*`, `inbox/*`, `health/*`, `graph/*`, `round/*`, `roadmap/*`, `mocks/*.smoke.test.tsx` | the hash `…?doc=<X>`, or the link's `href` the page | D6 |
| `flow/FlowView.test.tsx` panel blocks (text, links, Cards, Open in Spec tree and Graph), bounded reads counting `getTrace({ref})` on a selection, FS-17's titles by the tree read; `mocks/flow.smoke.test.tsx` | the minimal panel; no trace read on selection; titles from the read | D5, D9 |
| `tree/NodePane.test.tsx`, `trace/CardsBlock.test.tsx` | `Facts` moved; Open document; Cards' New card off | D6, D7 |
| tests expecting the RM-0091 English; `styles/scale.test.ts`' exception count (14 to 13) | the fixed forms; 13 | D10, tokens |

## Acceptance criteria

A browser look is marked **[look]**; a timed run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red.

- [ ] DM-1 — **Link ends titled.** `crates/specengine-cli/tests/doc_modal.rs` (new), on scratch copies of `spec-a`, `spec-b` and `flow-aside`.
  - For every node `spec tree --json` lists, `spec show <name> --links --json`: each link with `state` `resolved` has `title` and `end_kind` equal to the first node of `spec show <link.name> --json`; every other link has both `null`.
  - The keys stand in "Data" order, after `name`, in every link, plain ones too.
  - The text output is byte-equal to the build before (the goldens).
  - The `POT-BISQUE` links equal "Data"'s; spec-b's dangling mention at `docs/spec/cli.md:25` has both `null`.

  Mutations, each red: `title` from the shown node (`here`) instead of the other end; `end_kind` from the edge label; `skip_serializing_if` on `title`; an incoming link titled by its target.
- [ ] DM-2 — **Twins, keys, plugin.**
  - MCP `get_node` with `with: ["links"]` on `POT-BISQUE`: `structuredContent` equals the CLI's JSON and validates against `outputSchema`, whose link object requires `title` and `end_kind`, nullable.
  - HTTP `GET /api/projects/flow-aside/nodes/POT-BISQUE?with=links` is byte-equal to the CLI's JSON.
  - `spec health --json`'s amendment links carry both keys.
  - `daemon_keys.rs` and `fixtures/daemon-keys.json`: 115 sets, `ShownLink` and `FlowGap` as "Data".
  - `plugin_files.rs`: `PINS` and `SCHEMA_PINS` end at 0.34.0. `spec_ids_hub.rs` and `tag-check.sh` read 0.34.0.

  Mutations, each red: the mirror without `end_kind`; `plugin.json` left at 0.33.0.
- [ ] DM-3 — **Gap titles (RM-0101).** `doc_modal.rs`:
  - on `flow-aside`, `spec flow ATL-POTTERY --depth 1 --in-flow --json`, the `two-starts` row as "Data";
  - on `flow-gaps`, `spec flow --json`, every row's `titles` has `items`' shape, and each title equals `spec show <member> --json`'s; the `two-starts` row is `[["The first closed desk", "Ask at the desk", "The second closed desk"]]`;
  - the text is byte-equal;
  - MCP `get_flow`'s schema requires `titles`; HTTP `/flow` is byte-equal to the CLI.

  Core `tests/flow_read.rs`: a member with neither `title:` nor a heading serves `null`; 51 unreachable steps print 50 items and 50 title lists. Mutations, each red: titles from the folded box (`"Firing run"`); `titles` not cut with `items`.
- [ ] DM-4 — **Subset parity, cost.**
  - The store differential (subset against walk) stays byte-equal on every configured fixture, with the new keys, and the subset's counters (`subset::seams`) show no whole decode for `show --links`.
  - **[measure]** eval `scale` at 10 000, release, four alternating runs against 0.33.0's build, median p50: `get_node_links` and `get_flow_root` within +10 %.

  Mutation, red: titles read from a whole decode in the subset path.
- [ ] DM-5 — **The hash.** `ui/src/app/routes.test.ts`:
  - `parseHash` gives `doc` on `#/p?doc=A`, `#/p/board/C-1?doc=A`, `#/p/tree/R/page?doc=A`, and `#/p/flow?scope=S&doc=A&stage=x` (`query` `scope=S&stage=x`);
  - `#/p/flow?doc=A` has no `query`; `doc=` empty or undecodable is dropped; the first decodable `doc` wins;
  - `#/p/tree/A?B` is id `A`; `#/p/tree/R/page` is `view: "page"`, `#/p/tree/R/pages` names nothing;
  - `withDoc` and `withoutDoc` round-trip Flow's `edge=` with its raw commas;
  - `docPageHash(p, "ID#SEC")` encodes the `#`.

  Mutations, each red: `doc` read only on Flow; `withDoc` keeping a second `doc`.
- [ ] DM-6 — **Hash writes keep the modal.** `ui/src/app/location.test.ts` (new) and `FlowView.test.tsx`:
  - with `?doc=A` open, Flow's bare-hash replacement, a seek's replace and a module drop each keep `doc=A`;
  - `pushHash` drops it; `closeDoc` after `openDoc` goes back, and after a URL arrival replaces.

  Mutation, red: `replaceHash` not carrying `doc`.
- [ ] DM-7 — **`DocLink`.** `ui/src/doc/DocLink.test.tsx` (new):
  - `href` is `docPageHash`;
  - a plain click pushes `withDoc` (one history entry) and prevents the default; inside the modal it replaces;
  - with Meta, Ctrl, Shift or Alt, a middle click (`auxclick`), or `contextmenu`, nothing is prevented and the hash is unchanged;
  - Enter on the focused link acts as a plain click.

  Mutation, red: preventing the default under `metaKey`.
- [ ] DM-8 — **One link component.** `ui/src/policy.test.ts`: over non-test `src/`, the `sectionHash(…, "tree", …)` calls are exactly the allowlist of "Data", "Call sites", by file and count. `ui/src/mocks/doc.smoke.test.tsx` (new), on harbor-sim, from each of the Board tile, the card modal's target, Tasks' overview, the Inbox's proposal, Health's unbound row, Graph's list, the Questions round, the Roadmap lanes and Flow's Records: a plain click opens the modal on the linked document, and that screen stays mounted beneath. Mutations, each red: `BoardCard.tsx` back on `sectionHash(…, "tree", …)`; one allowlisted call dropped from the list.
- [ ] DM-9 — **The modal.** `ui/src/doc/DocModal.test.tsx` (new), on `docStub`:
  - one `role="dialog"`, `aria-modal`, labelled by its `h2` "The ferry", focused; the view inert, live regions outside;
  - opened from a link, one entry is pushed; Esc and Close go back, focus to that link; opened by URL, Esc replaces with `withoutDoc` and focus goes to the view's `h1`;
  - a scrim click closes; Back closes; Back with New card open over it closes both;
  - a link inside replaces `doc`, adding no entry;
  - a 404 shows the reason verbatim under `lang="en"`.

  Mutations, each red: no `closeOnScrim`; Esc always going back; focus left on `body` after a close.
- [ ] DM-10 — **Stacking.** `ui/src/board/CardModal.test.tsx` (new describe):
  - a target link in the card modal pushes `#/<p>/board/<C>?doc=<T>`; the card modal is inert under the document modal;
  - Esc closes only the document, focus back to the target link; Back likewise;
  - rendering `#/<p>/board/<C>?doc=<T>` cold puts the document on top, though the card read lands later.

  `FlowView.test.tsx`: a Records entry opens the modal over the dialog; Esc closes the modal, then the dialog. Mutation, red: the document modal mounted before the view.
- [ ] DM-11 — **The page.** `ui/src/doc/DocPage.test.tsx` (new) and `routes.test.ts`:
  - `#/p/tree/DOM-FERRY/page` shows the body with `h1` "The ferry" and no tools, and the nav marks Spec tree;
  - a remount keeps it;
  - an ancestor crumb's plain click moves to that page (a push);
  - a 404 shows the reason under `lang="en"`;
  - `#/p/board/C-1/page` is unchanged.

  Mutation, red: `view: "page"` accepted for `graph`.
- [ ] DM-12 — **The sections.** `ui/src/doc/DocBody.test.tsx` (new), on `docStub`:
  - **Breadcrumbs**: Project, "The river", "The ferry" (`aria-current="page"`, no `href`).
  - **Meta**: the in-work lozenge (label and icon); `accepted` neutral under `lang="en"`.
  - **Acts**, in order: New card (the one primary), Open in Flow (`flowHash` with `scope=DOM-RIVER` and `/DOM-FERRY`), Open in Spec tree; the tools Open as a page and Close.
  - **Text**: no `id: DOM-FERRY` line and no second "The ferry" heading; both paragraphs and the section render, at the reading-width class; `doc.throughAi` follows.
  - **Links**: Out holds "Boarding" (`uses`, raw, `lang="en"`), each a `DocLink`; Mentions (1) is collapsed; the dangling `MEC-GANGWAY` is absent from Links and listed under Technical details. In holds "Landing" (`follows`).
  - **Cards** lists `C-0007` with no New card of its own.
  - **Technical details** is collapsed; open, it holds the ID with a copy button, kind, `docs/ferry/ferry.md:1-<end>`, rev, tokens, both hashes, the front-matter, the unresolved count, and Open in Graph.

  Mutations, each red: the front-matter rendered in the body; links by `name` instead of `title`; Technical details open by default.
- [ ] DM-13 — **Inside.** `ui/src/doc/inside.test.ts` (new) and `DocBody.test.tsx`:
  - no `getTree` until Inside expands, then exactly one, `{root: "DOM-FERRY", depth: 2}`;
  - groups "Directly under this document" (`mechanic`: Boarding) and "Under Tides" (`assumption`: two entries; `question`: one), in that order; `doc.inside.noId` with 1; `ASM-0003` absent;
  - `WBAY-KILN` from the `aside` stub shows `hunch` with 50 entries and Show all 60, which focuses the 51st;
  - a second expansion in the same modal reads nothing; the choice holds on the next document for the page's life;
  - skeleton, failure with Retry, and the empty line.

  Mutations, each red: grouping by `parent` without the own-path test (`ASM-0003` listed); kinds unsorted; the read made on open.
- [ ] DM-14 — **Flow's panel.** `FlowView.test.tsx`, on `asideStub`:
  - selecting `POT-BISQUE` shows the heading "Bisque firing", the stage `bisque`, its work state, and the paragraph "Bone-dry pots are fired once, slowly, to a low cone.";
  - the acts are exactly Open document (primary) and New card; `KRUN-FIRING` adds "Open Firing run";
  - there is no `LinksPanel`, `CardsBlock`, Open in Spec tree or Graph in the panel;
  - Technical details is collapsed with name, kind, place, distance;
  - the selection reads no `getTrace({ref})`; Open document pushes `withDoc`, and the modal draws from the held node read with no new call.

  `ui/src/markdown/Markdown.test.tsx`: the first paragraph skips front-matter, headings, a leading list, a table and a fence; a text with none renders nothing. Mutations, each red: the whole text in the panel; `getTrace({ref: N})` on a selection.
- [ ] DM-15 — **Gaps from the read.** `FlowView.test.tsx` "the Gaps control (FS-17)", scope `ATL-POTTERY`, Gaps open: the item reads "Wedging + Open day", and no `getTree` is made. A member whose title is `null` shows its name. Mutation, red: `useTreeOnDemand` restored.
- [ ] DM-16 — **Spec tree and palette.**
  - `TreeView.test.tsx`: a link in the node pane's text, Links, Bundle or section list still pushes `#/<p>/tree/<X>` and opens no modal; Open document pushes `?doc=<REF>` over the tree.
  - `Palette.test.tsx`: a ref and a hit open the modal over the current screen (`#/alpha/inbox?doc=<X>`); with the modal open they replace `doc`; the search option is unchanged.

  Mutation, red: the tree's `LinksPanel` switched to `DocLink`.
- [ ] DM-17 — **RM-0091.** `ui/src/i18n/plural.test.ts` (an English block): each key of "Data", "RM-0091" at count 1 equals the fixed form, and at 2 is unchanged; Russian is untouched. Mutation, red: `tree.links.omitted_one` restored.
- [ ] DM-18 — **Both languages.** `i18n/catalog.test.ts` passes with the new keys: parity, plurals, placeholders. `policy.test.ts` scans `src/doc/`. `app/screens.language.test.tsx` switches the modal and the page to Russian: chrome from `ru.ts`, titles and text verbatim, the daemon's words under `lang="en"`. Mutation, red: a literal "Links" in `DocLinks.tsx`.
- [ ] DM-19 — **Tokens.** `styles/scale.test.ts`:
  - `SCALE_EXCEPTIONS` holds 13 entries, none `until: "RM-0095"`, and the type has no `"RM-0095"`;
  - `.md-url, .md-image-path` take a type token; the doc CSS uses tokens alone;
  - `IssueView.test.tsx` covers `meta`.

  Mutation, red: `font-size: 15px` in `.doc-body`.
- [ ] DM-20 — **[measure] Size.** After `pnpm build`, first-load JS (`ui-languages.md` "Size") is at most 845 kB. No chunk holding `DocBody` is the entry or a `modulepreload`. The figure, gzip and `ru-*.js` are printed under "Implementation". Over +10 kB on 815.13, the reason is written there.
- [ ] DM-21 — **[look]** Chrome DevTools, light and dark, English and Russian, at 1440x900, 1100x800 and 320 wide, on the mock and on a scratch copy of the owner's project:
  - the modal over Flow, the Board and a card modal (stacked);
  - the page;
  - Flow's panel for a box, a group and the scope;
  - Inside of the largest part, landing within 1 s (the network timing recorded);
  - long Russian titles wrapping in the breadcrumb and Inside;
  - at 320, one column filling the screen;
  - focus rings visible, nothing cut unseen.
- [ ] DM-22 — **Gates.**
  - UI: lint 0, `tsc` clean, the full test run green twice.
  - Rust: the workspace run green.
  - `cargo nextest run -p specengine-eval --test doc_pointers --test anonymity --test ui_policy`.
  - The docs check clean.

## Out of scope

- Words for link types, and for kinds (D12, RM-0096's directions).
- A served list of children (Inside reads the tree).
- Editing a document in the modal (ADR-0004).
- "Hand to your AI" from the mockup.
- The edge panel.
- Flow's seek for documents out of the flow.
- The Spec tree node view's own layout (RM-0099).
- The word "Task" for a card (RM-0096).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-cli/src/links.rs` (`ShownLink.title`, `.end_kind`, `LinkJson`); `crates/specengine-core/src/flow.rs` (`FlowGap.titles`); `crates/specengine-cli/src/flow.rs` (the gap JSON); `crates/specengine-mcp/src/mirror.rs`; `plugin/specengine/.claude-plugin/plugin.json`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `crates/specengine-cli/tests/doc_modal.rs` (new); core and CLI `flow_read.rs`; the "Test churn" Rust rows; `fixtures/daemon-keys.json`; DM-4's run |
| ui-developer | `src/api/provisional.ts`; `src/app/{routes,location,Shell}.ts(x)`; `src/doc/` (new: `DocLink`, `DocModal`, `DocPage`, `DocBody` lazy, `DocLinks`, `DocInside`, `inside.ts`, `DocTechnical`); `src/ui/IssueView.tsx`; `src/tree/{Facts,NodePane}.tsx`; `src/markdown/Markdown.tsx` (link branch, title drop, first paragraph); `src/flow/{FlowPanel,FlowGaps,FlowView,FlowRecords}.tsx`, `src/api/queries.ts`; the 22 call sites; `src/i18n/{en,ru}.ts`; `src/styles/{app.css,scale-exceptions.ts}`; `src/mocks/` (the new keys, rooted `getTree` with `depth`); `src/test/docStub.ts` (new); and the UI tests of DM-5 to DM-19 |
| test-engineer, UI | gates, mutations, DM-20, `ui_policy.rs` and `doc_pointers` if a pin moves |

Comments cite `docs/features/doc-modal.md` "Acceptance criteria" with a DM number, or an existing canon heading. They never cite "Data" or "Description and interactions": compaction drops both at ship (doc_pointers).

### Work order

1. The rust-developer and the ui-developer start together; the contract is fixed above. The UI lane lists the three new keys in `AHEAD_OF_FIXTURE` until the Rust lane's `daemon-keys.json` lands, and the list is empty at ship.
2. The Rust test-engineer follows the Rust lane, overlapping the UI lane; then the Rust review.
3. The UI lane completes; the UI test-engineer runs gates and mutations; then the UI review.
4. The orchestrator's looks (DM-21).
5. Ship: the spec-writer ("Canon at ship").

At most three iterations a lane. If the Rust lane runs out on D5 alone, RM-0101 goes back to the pool, and D4 ships.

### Risks

- **Size.** 29.9 kB of headroom. The body is lazy; a static import of `DocBody` or of `src/markdown/` into first load eats it.
- **Hash writes.** A screen's own replace dropping `doc` closes the modal by surprise (DM-6). A hand-typed Tree id with a raw `?` now ends there.
- **Stacking order on a reload** (DM-10), and focus across two dialogs.
- **Churn.** Many screen tests assert tree hashes on a click (DM-8 lists the screens).
- **Scale.** Inside is O(N) and a hub's project turn. A viewer who keeps it open pays it per document.
- **The subset read.** If a link end's title needs a file the subset does not decode, the subset widens to the ends' holders, and `spec-read-subset.md` "The subset" says so at ship.
- **Policy.** Raw link types are shown, and none is quoted (D12).
- **Accessibility.** The disclosures, the breadcrumb's `aria-current`, the opener's focus.
- **Both languages.** Long Cyrillic titles must wrap.
- **Caps** at ship: see below.

### Canon at ship

The new Tier 2 canon is **`docs/canon/doc-view.md`**, scope `[ui]`, at most 12 KB. It holds:
- the `doc` key and `/page`, history and focus;
- `DocLink` and its allowlist;
- the sections, Inside, Technical details and the reads;
- Known limits: Inside O(N); sections nested two deep; raw link types; Open in Flow for out-of-flow documents;
- Tests.

`docs/index-canon.md` gains one line: 9 463 B plus about 140, under 10 240, so no shard split. The next unclaimed canon file may need one.

Edits, each within its cap (free bytes today in brackets):

| File | Edit |
|---|---|
| `docs/canon/ui-design.md` [26] | "Primitives": `IssueView`'s `meta?`. "The work item": a document's, `doc-view.md`. "Guards and exceptions": 13, the modal's entry gone. The net is negative. |
| `ui/README.md` [2 068: 8 172 of 10 240] | "Shell": the card page's form becomes "pages and `doc`: `doc-view.md`". "Dialogs": "a card's modal" becomes "a work item's modal". |
| `docs/canon/board-screen.md` [137] | "Targets link to the tree" becomes "targets open the document over it (`doc-view.md`)". |
| `docs/canon/spec-flow-screen.md` [367] | "Panel" minimal; "Reads": Box N without `getTrace`, the Gap titles row gone; "Known limits": Gap titles gone; the Cards-block paragraph points to the modal. The net is negative. |
| `docs/canon/spec-flow-bar.md` | "Gaps" Titles from `titles`; "Records for the AI": entries are document links |
| `docs/canon/spec-flow-place.md` | "Splitting": every route; `doc` |
| `docs/canon/spec-cli-graph.md` [33] | the link key list gains `title, end_kind` (+17 B) |
| `docs/canon/spec-cli-read.md` | the two keys' meaning |
| `docs/canon/spec-flow.md` [478] | "Output": the gap's `titles` |
| `docs/canon/plugin-files.md` | 0.34.0 |
| `docs/canon/hub-deploy.md` [1] | "0.33.0, `flow-scenario`'s" becomes "0.34.0, `doc-modal`'s" (−4 B) |
| `docs/canon/health.md` [6] | "the path linked to the tree" becomes "the path a document link" (−3 B) |
| `docs/canon/drift-screens.md` [5] | "its node a tree link with the tree view's `onFollow`, on Health a document link" (not longer) |
| `docs/canon/ui-languages.md` | "Known limits": the RM-0091 line dropped |
| `CLAUDE.md`; 08 s2 Phase 7 step 3; RM-0095, RM-0091, RM-0101 | shipped |
| this spec | compacted to at most 3 KB |

`mcp-read.md` [7] and the CLI README [10] are not touched.

### Working answers and the owner's questions

**Working answers taken:**
- **W1 (Flow v2 Q3).** The page lives under the Spec tree's route. The tree's node view and its own links stay technical.
- **W2 (Q4).** The modal is the `doc` pair, on every route.
- **W3.** The palette opens the modal (the owner's "everywhere").
- **W4.** Flow's panel keeps its own facts, folded.
- **W5.** "New card" keeps its word until RM-0096.
- **W6.** RM-0101 rides in this slice (D5).

**The owner's questions:**
- **Q-A. Link type words.** Should `follows`, `uses` and `supplies` read as words ("Comes after", "Relies on") in both languages? Types are free strings app code never quotes (ADR-0031). Words would need a served label per type, a model or config key: a new item and an ADR. Working answer: raw, `lang="en"`, until he asks.
- **Q-B. Inside on open.** Should Inside read on open instead of on expansion? That is one O(N) read, about 1 s at 100k, a hub's project turn, per document opened. Working answer: collapsed, read on expansion, the choice kept per project for the page's life.
- **Q-C. The palette.** Should the palette's document hits open the modal, or the Spec tree as today? Working answer: the modal; Open in Spec tree is one act inside it.
