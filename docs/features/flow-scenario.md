---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: RM-0094; v5 step 2 (08 s2 Phase 7, "v5 scope"); Flow v2 step 3c and the owner's path request, both of 2026-11-10; Flow v2 Q1-Q4 and v5 Q14 at their recommendations; FS-1 to FS-24 merge the analysts' FS-1 to FS-14 and FL-1 to FL-15
adrs: []
---

# flow-scenario: Flow draws the scenario, and its left pane is a path (RM-0094)

## Why

The owner looked at Flow on his own project on 2026-11-10, and his verdict was that the design is bad. He found three problems:
- **The canvas is noisy.** Records and notes with no ID stood as a wall of tiny boxes. In one part, 150 boxes were drawn and none was on the walk. Dashed bundles of `uses` lines carried `x13` badges. The fit shrank box text to about 8 px. The real scenario, a walk of steps, ran off the canvas to the right.
- **The panel is technical.** It led with kind, distance, `path:line`, inner counts and English gap codes, and his comment was "this is information for the AI".
- **The left pane shows a scope, not a path.** He wants "a path that goes deeper and deeper". Today the pane gives only an Up link, which it guesses from reads it already holds, and on a fresh deep link it says "Up to the start".

His answers settle the direction:
- the canvas shows the scenario only: flows, steps, groups, and documents with links;
- records go into a "Records for the AI" list, and documents with no ID into the Spec tree;
- the panel is minimal, and technical fields are folded.

This slice is the Flow half of that. It also makes the path a fact the daemon serves, so RM-0095's document modal can build its breadcrumb on it. The document modal and the minimal panel themselves are RM-0095's.

The rule for what counts as part of the flow has to be domain-free (ADR-0008). His records share a directory and kinds that the core must never name. What separates them is that no flow link reaches them. So "in the flow" is the set the read with no ROOT already admits, served as a flag on every node, and the UI only reads it (ADR-0033).

## Description and interactions

### Decisions

- **D1. In the flow.** A document is in the flow when `spec flow` with no ROOT holds it: it holds or receives a resolved flow link (crossing ones too); or it holds a flow link whose other end is left out, or a `follows` that does not resolve; or a `[flow]` table selects it; or it is a live `parent:` holder of any of those (`spec-flow.md` "The read").
  - Every node of every flow read carries `in_flow`. The flag is folded like `start`: a node is in the flow when it, or anything it holds, is.
  - A read with no ROOT therefore serves only `in_flow: true`.
  - The rule is links, `[flow]` and `parent:` alone, with no kind, path, prefix or ID-shape test.
- **D2. The cut is opt-in.** CLI `--in-flow`, MCP `in_flow: true` and the daemon's `in_flow=true` drop each shown node whose `in_flow` is false. The cut runs after the fold and after `--stage`. A node standing for a ROOT holder always stays, because the scope keeps its anchor and its `counts`.
  - `not_in_flow` counts the documents the dropped nodes stand for: 1 + `holds` each.
  - The cut leaves everything else as `--stage` does. Without it, CLI and MCP reads stay whole.
  - With `--stage`, the stage cuts first. A ROOT node that the stage dropped stays dropped.
- **D3. `types`.** Every flow read serves `types`, the model's `FLOW_LINK_TYPES` names in their order. The UI places a type's colour and dash by this list, so a type keeps one look across pictures, and no app code quotes a link type.
- **D4. The served path is `ancestors`.** The plan calls it `path`, but every show node already has a `path`, which is its file. So the key is `ancestors`: the nodes `SpecGraph::ancestors` returns, top first, each as `{id, kind, title, path}`.
  - The chain follows `listed_under`. A section climbs to its enclosing node. A document goes to its `parent:`'s first holder by (path, ord).
  - A `parent:` that dangles, names another project, or is a cycle's break point ends the chain, and no node is listed twice.
  - `path` is in each entry because a section's enclosing document may have no ID.
  - The key is served on `spec show --json`, `get_node` and `GET .../nodes/REF`. Show's text is unchanged.
- **D5. Built from the subset.** A `show` read's plan adds each shown node's chain of `PARENT` rows, file by file, as `bundle` already does (`spec-read-subset.md` "The subset"). It decodes no other file and builds no whole graph. The walk gives the same list from the whole graph and remains the oracle.
- **D6. What Flow reads.** Every drawing read is cut. The count behind "Records for the AI (N)" is the served `not_in_flow`. The list itself is read only on demand, as the same scope read uncut and without trace.
- **D7. Top to bottom (v5 Q14).**
  - Each row is one `distance`, and the band of boxes not on the walk sits below the last row.
  - A stage divider stands above the first row holding a box of that stage. Stages that begin in the same row share one divider. A picture with fewer than two stages draws none.
- **D8. Readable.**
  - Flow gets its own fit floor, `FLOW_FIT_MIN_ZOOM` 0.92, with box titles at 14 px on the RM-0093 scale, so they render at 12.9 px or more. It also gets its own pinch floor, `FLOW_PINCH_MIN_ZOOM` 0.5.
  - Graph keeps `FIT_MIN_ZOOM` 0.6 and its 0.1.
- **D9. Off-walk edges and labels.** An edge is off the walk when either end's `distance` is null in the drawing read.
  - Such an edge is drawn only while one of its ends, or the edge itself, is selected.
  - Canvas labels drop `x<count>`. The List and the edge panel keep the count.
- **D10. Gaps in the bar.**
  - The ten rows leave the panel's scope view for a Gaps control in the canvas bar.
  - Each row shows its plain meaning first. The raw code and the served `cause` sit folded under "Technical details", the cause marked `lang="en"`.
- **D11. The path pane.** The places pane shows the path, read from the scope view's `getNode` that is already made: no extra read, no held-read guessing.
  - Up, `flow.upStart`, `useHeldHolder` and `useHeldTitle` are removed.
  - Before the lens root is known, a top entry links to `scope=<id>`, which draws the same picture as the lens root.
- **D12. Titles, not IDs.**
  - Boxes, List rows, places, gap items and panel headings show a node's `title`. A node with no title falls back to its ID, then its path.
  - IDs, raw kinds, `path:line`, distance and inner counts sit under one "Technical details" disclosure in the panel.
  - Otherwise the panel stays as today until RM-0095.
- **D13. No ADR.** No rule of `architecture.md` moves.
  - ADR-0008 holds because the rule is links and config.
  - ADR-0033 holds because the flag, the count, the path and `types` are served and the UI computes none of them.
  - ADR-0055's three types are unchanged.
  - ADR-0012 holds because gap rows stop nothing and the cut hides no finding.
- **D14. MCP `INSTRUCTIONS` are unchanged.** They are at 2 048 B with BLAKE3 pins, and they already omit `stage`. The tool schema describes `in_flow`.

### The contract (Rust lane)

- `spec flow [ROOT] [--depth N] [--stage S] [--in-flow] [--trace] [--json]`, MCP `get_flow {root?, depth?, stage?, trace?, in_flow?}`, and daemon `GET .../flow?root=&depth=&stage=&trace=&in_flow=` all take the cut.
- **What the cut leaves alone.** The scope, the walk, the ten gap rows, `left_out`, `notes`, `root`, `reason`, `depth`, `FlowRead.shown` and the `--trace` joins (so a card on a dropped record still counts on its holder) are the read's without the cut.
- **What the cut changes.**
  - Each kept node and edge is byte-equal to its object without the cut. `group` names the holder whether it is kept or not.
  - The header counts, the cap, `truncated` and `omitted_*` count what is kept.
- **Membership.** It is computed once per read, with Live-only admission. When every asked ROOT is live, it reuses the read's links and `parent:` map, so a rooted read gains one linear pass and no second `flow_read`. A Tier 3 ROOT's own node is never a member, but it is in the flow by the fold when it holds a member.
- `spec show REF [--links] [--bindings] --json` gives each node `ancestors`, and `get_node` and `nodes/REF` give the same. The subset and the walk agree on every fixture.
- **Plugin.** The version goes to 0.33.0 (`plugin-files.md` "Version": a key that a tool takes and returns). `read-spec`'s flow line names `in_flow`, and its node line names `ancestors`.

### The screen (UI lane)

Built on RM-0093's tokens and primitives (`docs/features/design-foundation.md`).

| Primitive | Where in Flow |
|---|---|
| `Button` | the bar's Legend and Gaps toggles (subtle, compact), Records for the AI (subtle), the dialog's Show all and Close, the panel acts as today (at most one primary per area) |
| `Lozenge` | the Gaps toggle's state; a box's and the panel's stage stays text |
| `EmptyState` | every empty canvas (heading level 2) and the empty Records dialog |
| the disclosure helper (`Disclosure`) | "Technical details" in the panel and in each gap row |
| `Breadcrumbs` | not used. The path pane is a vertical list marking `location` in a nav; `Breadcrumbs` marks `page` on one line. RM-0095's modal uses it, fed by `ancestors`. |

- **Reads.** `flowForm` gains `in_flow: true` in all three forms. `recordsForm(root)` is `{root, depth: 1}`, with no trace and no cut, and is read only when the Records dialog opens. The lanes, scope-view and box reads are unchanged ("Data", "UI types and forms").
- **The canvas.**
  - **Layout.** Only the read's (cut) nodes are drawn, top to bottom, one row per `distance`. Boxes in a row run left to right, ordered by the barycentre sweeps. A forward edge that skips a row gets a passage slot. An edge against the walk runs on its own track right of the walk. Self-edges sit beside their box.
  - **The band.** It is captioned `flow.offWalk` plus its count, with `flow.offWalkAbout`. It wraps at the walk's width or four boxes, whichever is wider.
  - **Stage dividers.** Each divider is full-width, labelled with the raw stage word or words joined by ` · `, and drawn as a third node type that is `aria-hidden` and not focusable. A box's accessible name adds its stage.
  - **Fit.** The fit takes the width, at most 100 % and at least 0.92. A tall picture is panned.
  - **Keys.** Up and Down move across rows, the band last. Left and Right move within a row.
- **Edges** follow D9. `typesOf(view)` returns `view.types`. A type missing from the list takes the first colour. The legend's about text says top to bottom and states the selection rule.
- **The bar** holds Legend, then Gaps, then `flow.notDrawn` past the cap.
  - **Gaps toggle.** A subtle compact Button reading "Gaps", then a Lozenge:
    - warning, `flow.gap.found` with the number of rows found, when any row is measured above 0;
    - neutral, `flow.gaps.notAll`, when any row is not measured;
    - success, `flow.gap.none`, otherwise.
  - **Opening.** It opens as the legend does: over the canvas from its bottom edge, at most half its height, scrolling inside. Only one of Legend and Gaps is open at a time. Esc closes it, with focus to its toggle, and a focus or click in the canvas also closes it.
  - **Content, in order:**
    1. rows found, in served order, each with its `flow.gapMeaning.*` sentence, its state in words, and its items;
    2. rows not measured;
    3. the rows at 0, as one line `flow.gaps.zero`.
  - **Items.** Each item is a button naming its members by title. Activating it closes the control, then works as today (`spec-flow-screen.md` "Gap rows"). "Hand to your AI" is unchanged.
  - **Technical details.** Each row's disclosure holds its code and its `cause` (`lang="en"`).
- **Records for the AI.**
  - **The button.** A subtle Button sits at the foot of the places pane, reading `flow.records.open` with N. N is the drawing read's `not_in_flow` for a scope read. In a stage view, N comes from the scope read already held, with no read of its own; without that read the button shows `flow.records.openPlain`. There is no button at the unrooted Top or at N = 0.
  - **The dialog.** Activating opens a 600 px dialog, `role="dialog"`, `aria-modal`, labelled by its `h2` `flow.records.title` with N, then `flow.records.about`.
  - **Its read** is `recordsForm(scope)`. It lists the read's nodes with `in_flow: false` other than the scope's own node.
    - Nodes with an ID are grouped by raw `kind` (sorted, `flow.records.noKind` last) in served order. Each is a link by title to the Spec tree; a folded node adds `flow.holds`.
    - Nodes without an ID are not listed. One line `flow.records.noId` links to the Spec tree at the scope.
    - The first 200 entries show, then `flow.records.showAll`.
  - **States:** a skeleton while reading; on failure, the daemon's words and Retry; if the uncut read holds none, `EmptyState` `flow.records.none`.
  - Close or Esc returns focus to the button. A live mark re-reads only while the dialog is open.
- **The path pane** (`FlowPlaces.tsx`), inside the existing `nav` "Flow places", top to bottom:
  1. **Modules**, as today.
  2. **Path**: an `ol` labelled `flow.path`, with entries from the scope view's `getNode(R)` first node's `ancestors`, top first, and then the scope.
     - Each entry is an `<a href={flowHash(...)}>` showing title ?? ID, indented one step per level up to six levels. Long titles wrap.
     - The last entry is `aria-current="location"`.
     - Entries above the lens root are left out. With a module in force, the lens root is the module's root. Without one, it is Top's one node, known only once Top has been read.
     - An entry equal to a known lens root writes `scope` null. Any other entry writes `scope=<id>`.
     - At the lens root, the path is one entry: its title, or `flow.topTitle` for an unrooted Top.
     - While the node read is pending, the ancestors show as a skeleton line. If it fails, only the current entry shows, with `flow.pathFailed` and Retry.
  3. **Groups under the current entry**, indented one level more: for All stages, the drawing read's nodes with `holds > 0` other than the scope; for a stage, the distinct `group`s its nodes name, as today.
  4. **Stages**, as today.
  5. **Records for the AI.**

  Group rows keep their chevron on the title's line: `flex-wrap: nowrap`, the words `flex: 1 1 0` with `min-width: 0`, and the icon `flex: none`.
- **The panel** stays as today, except:
  - the scope view loses its gap rows (now in the bar), and moves its name, kind and path under "Technical details";
  - a box's view keeps its title, stage, group (by title, with Open), Open, Cards, text, links and acts, and moves its name, kind, `path:line`, distance, start or end, and inner counts under "Technical details" (`ui.technicalDetails`, collapsed).
- **Empty states**, each an `EmptyState` in the canvas region:
  - The existing four keep their keys: project, scope, stage, and no stage.
  - **A cut that leaves only the scope's own node**, at a scope where nothing is in the flow, shows `flow.emptyAside` with `flow.emptyAsideMore` (N), and Records for the AI as its one act.
  - The failed and `reason` states are unchanged.
- **Removed:** Up and its links, `flow.up`, `flow.upStart`, `flow.scopeLabel`, `useHeldHolder`, `useHeldTitle`, Flow's import of Graph's `FIT_MIN_ZOOM`, and Flow's `SCALE_EXCEPTIONS` (`until: "RM-0094"`). Flow's CSS moves from the legacy aliases (`--font-size-s`, `--space-1..6`) to the scale's names, as `docs/features/design-foundation.md` "Guards and staging" assigns to this slice.

## Data

### `spec flow` JSON and text

- **Read:** `{root, reason, depth, notes, left_out, truncated, omitted_nodes, omitted_edges, not_in_flow, types, gaps, nodes, edges}`, 13 keys.
  - `not_in_flow` is an integer under the cut and `null` without it.
  - `types` is `["follows", "uses", "supplies"]` on every read.
- **Node:** 16 keys, `{id, kind, title, path, line, distance, group, stage, start, end, in_flow, holds, inner, gaps, rollup, counts}`. `in_flow` is a boolean.
- **Text:** the header gains `[; not in the flow: <d>]` under the cut, after `left out`. No other line changes, and a read without the flag prints byte for byte as before.

```text
flow WBAY-KILN | nodes 4, edges 0, depth 1; not in the flow: 141
```

```json
{"root":"WBAY-KILN","reason":null,"depth":1,"notes":[],"left_out":{"generated":0,"tier3":0},
 "truncated":false,"omitted_nodes":0,"omitted_edges":0,"not_in_flow":141,
 "types":["follows","uses","supplies"],"gaps":["... ten rows ..."],
 "nodes":[{"id":"HUNCH-0007","kind":"hunch","title":"Glaze crawls on damp bisque","path":"docs/hunches/HUNCH-0007.md","line":1,
           "distance":null,"group":"WBAY-KILN","stage":null,"start":false,"end":false,"in_flow":true,
           "holds":0,"inner":[],"gaps":0,"rollup":null,"counts":null},"... WBAY-KILN, CONE-BISQUE, CONE-GLOST ..."],
 "edges":[]}
```

Core: `FlowNode.in_flow: bool`, `FlowAsk.in_flow_only: bool`, and `FlowRead.not_in_flow: Option<usize>`. Every `FlowAsk` literal gains the field. CLI: `FlowRequest.in_flow: bool`. The `--in-flow` help reads: "leave out each node holding nothing of the flow (a ROOT stays); the header counts the documents left out".

### `spec show` JSON

A node has 20 keys: `ancestors` comes before `links`, as `[{id, kind, title, path}]`, top first, `[]` for a root. `id`, `kind` and `title` may be `null`; `path` is the file.

```json
{"id":"CONE-GLOST-SHELF","kind":"conerule","title":"Shelf spacing","path":"docs/workshop/cones/glost.md", "...": "...",
 "omitted":null,
 "ancestors":[{"id":"ATL-POTTERY","kind":"atelier","title":"The pottery","path":"docs/workshop/atelier.md"},
              {"id":"WBAY-KILN","kind":"workbay","title":"Kiln bay","path":"docs/workshop/bays/kiln.md"},
              {"id":"CONE-GLOST","kind":"conerule","title":"Glost cone","path":"docs/workshop/cones/glost.md"}],
 "links":null,"bindings":null}
```

### MCP and daemon

- **MCP.** `FlowArgs.in_flow: Option<bool>`, described as "Leave out each node holding nothing of the flow; a ROOT stays; not_in_flow counts the documents left out. The gap rows stay whole." The mirrors are closed: `FlowDocument` gains `not_in_flow` and `types`, `FlowNode` gains `in_flow`, `ShowNode` gains `ancestors`, and `ShowAncestor`, which is new, holds `{id, kind, title, path}`. A hub's MCP serves the same tools.
- **Daemon.** `flow` takes `in_flow` through `args.boolean`; another value gives 400 `` `in_flow=<v>`: not `true` or `false` ``. Freshness stays `trace`'s choice. `nodes/{*ref}` carries `ancestors` with no new argument.

### Key sets

`fixtures/daemon-keys.json`:
- `FlowView` gets 13 keys and `FlowNode` 16, as above.
- `ShownNode` gets 20.
- Set 115, `ShownAncestor`, is `["id", "kind", "title", "path"]`.

Rust `daemon_keys.rs` and the UI's `daemonKeys.test.ts` read the file. The UI half lands with the Rust lane as UI step 0 ("Work order").

### Plugin

`plugin.json` goes to 0.33.0: one minor above 0.32.0, or above the version current at merge. `plugin_files.rs` gains a `PINS` entry and a `SCHEMA_PINS` entry. `read-spec`'s line 20 adds "`in_flow: true` leaves out what holds nothing of the flow"; its line 17 adds "`ancestors`, the node's path from the top". `INSTRUCTIONS` are unchanged (D14).

### UI types and forms

```ts
// src/api/provisional.ts
interface FlowView { /* ... */ not_in_flow: number | null; types: string[]; }
interface FlowNode { /* ... */ in_flow: boolean; }
interface ShownAncestor { id: string | null; kind: string | null; title: string | null; path: string; }
interface ShownNode { /* ... */ ancestors: ShownAncestor[]; }
// src/api/client.ts
interface FlowOptions { root?: string; depth?: number; stage?: string; trace?: boolean; in_flow?: boolean; }
// src/api/queries.ts
flowForm(null, null)  -> { depth: 0, in_flow: true, trace: true }
flowForm(R, null)     -> { root: R, depth: 1, in_flow: true, trace: true }
flowForm(R, S)        -> { root: R, stage: S, in_flow: true, trace: true }
recordsForm(R)        -> { root: R, depth: 1 }
// src/flow/geometry.ts
export const FLOW_FIT_MIN_ZOOM = 0.92;   // 0.92 x 14 px >= 12 px
export const FLOW_PINCH_MIN_ZOOM = 0.5;
```

**The mock** (`src/mocks/flow.ts`) gains three things:
- `in_flow`, the cut, `not_in_flow` and `types` on every read;
- `ancestors` on every node read;
- the harbor-sim scenarios `aside` and `aside40`, which generate the fixture's table below with its IDs, titles, stages and links. `aside40` holds 5 680 records.

Only the mock computes membership.

### Catalog keys

New, in `en.ts` and `ru.ts`. The Russian is the ui-developer's, and lives in `ru.ts` only.

| Key | English |
|---|---|
| `flow.path` | Path |
| `flow.pathFailed` | The path could not be read. |
| `flow.records.open_one`, `_other` | Records for the AI ({{count}}) |
| `flow.records.openPlain`, `flow.records.title` | Records for the AI |
| `flow.records.about` | Documents in {{scope}} that hold no part of the flow. Your AI reads them; the canvas leaves them out. |
| `flow.records.noKind` | No kind |
| `flow.records.noId_one`, `_other` | {{count}} document without an ID is in the Spec tree; {{count}} documents without an ID are in the Spec tree |
| `flow.records.showAll_one`, `_other` | Show all {{count}} |
| `flow.records.reading`, `flow.records.failed` | Reading the records; The records could not be read |
| `flow.records.none` | Every document here is part of the flow. |
| `flow.gaps.toggle` | Gaps |
| `flow.gaps.notAll` | not all measured |
| `flow.gaps.zero_one`, `_other` | {{count}} row finds nothing; {{count}} rows find nothing |
| `flow.emptyAside` | Nothing here is part of the flow. |
| `flow.emptyAsideMore_one`, `_other` | {{count}} document here is for your AI.; {{count}} documents here are for your AI. |
| `flow.offWalkAbout` | Linked to the flow; no step in this picture leads to them. |
| `ui.technicalDetails` | Technical details (RM-0095 reuses it) |

Changed: `flow.legend.about` becomes "Top to bottom is the walk from the start; each box sits by its first step. A line beside the boxes runs against the walk. A line to a box off the walk shows when one of its ends is selected." Removed: `flow.up`, `flow.upStart`, `flow.scopeLabel`. Reused: `flow.gap.*`, `flow.gapMeaning.*`, `flow.holds_*`, `flow.offWalk`, `flow.topTitle`.

### Fixture `flow-aside`

An invented pottery workshop, with no game word and no pilot's word. The genre scans read its scheme as they read every fixture's.

```toml
# Fixture corpus flow-aside: an invented pottery workshop. What is in the flow is
# told by links, [flow] and parent: alone (docs/features/flow-scenario.md "Data").
[project]
slug = "flow-aside"

[ids]
ATL    = { kind = "atelier",   shape = "name" }
KRUN   = { kind = "kilnrun",   shape = "name" }
POT    = { kind = "potstep",   shape = "name" }
WBAY   = { kind = "workbay",   shape = "name" }
CONE   = { kind = "conerule",  shape = "name" }
DESK   = { kind = "deskpaper", shape = "name" }
HUNCH  = { kind = "hunch",  width = 4 }
PUZZLE = { kind = "puzzle", width = 4 }
EDICT  = { kind = "edict",  width = 4 }

[[flow.step]]
kinds = ["potstep"]

[[flow.start]]
paths = ["docs/workshop/run/wedge.md", "docs/workshop/gallery/open-day.md"]

[[flow.end]]
paths = ["docs/workshop/run/pack.md", "docs/workshop/gallery/open-day.md"]
```

| Documents | Path | `parent:` | Links |
|---|---|---|---|
| `ATL-POTTERY` "The pottery" | `docs/workshop/atelier.md` | none | none |
| `KRUN-FIRING` "Firing run" | `docs/workshop/run/firing.md` | `ATL-POTTERY` | none |
| 10 steps `POT-WEDGE`, `-THROW`, `-TRIM`, `-DRY`, `-BISQUE`, `-GLAZE`, `-FIRE`, `-SORT`, `-MEND`, `-PACK` | `docs/workshop/run/<word>.md` | `KRUN-FIRING` | each `follows` the one before. `MEND` follows `SORT` `{kind: alternative, when: cracked}`. `PACK` follows `SORT` and `MEND`. `GLAZE` also follows `FIRE` `{kind: repeat}`. 11 edges in all |
| stages | | | `wheelwork`: WEDGE, THROW, TRIM; `bisque`: DRY, BISQUE; `glazing`: GLAZE, FIRE; `crating`: SORT, MEND, PACK |
| `uses`, 15 | | | WEDGE: `WBAY-CLAY`; THROW and TRIM: `WBAY-WHEEL`; TRIM: `WBAY-DRYING`; BISQUE: `WBAY-KILN`, `CONE-BISQUE`; GLAZE: `WBAY-GLAZE`; FIRE: `WBAY-KILN`, `CONE-GLOST`, `HUNCH-0007`; SORT and MEND: `WBAY-SORTING`; PACK: `WBAY-PACKING`, `WBAY-STORE`, `WBAY-YARD` |
| 14 parts `WBAY-CLAY`, `-WHEEL`, `-DRYING`, `-KILN`, `-GLAZE`, `-SORTING`, `-PACKING`, `-STORE`, `-YARD`, `-GALLERY`, `-OFFICE`, `-WATER`, `-FUEL`, `-CANTEEN` | `docs/workshop/bays/<word>.md` | `ATL-POTTERY` | none of their own |
| `CONE-BISQUE`; `CONE-GLOST` with section `CONE-GLOST-SHELF` | `docs/workshop/cones/{bisque,glost}.md` | `WBAY-KILN` | none of their own |
| `DESK-LEDGER`, `DESK-ROTA` | `docs/workshop/office/{ledger,rota}.md` | `WBAY-OFFICE` | none |
| the ID-less start "Open day" | `docs/workshop/gallery/open-day.md` | `WBAY-GALLERY` | `[flow]` start and end |
| 8 ID-less notes | `docs/notes/note-1.md` to `note-8.md` | `ATL-POTTERY` | none |
| 142 records: `EDICT-0001` to `-0032`, `HUNCH-0001` to `-0060`, `PUZZLE-0001` to `-0050` | `docs/{edicts,hunches,puzzles}/<ID>.md` | `WBAY-KILN` | `PUZZLE-0003` has `depends_on: [POT-BISQUE]`; no other links |

Titles the criteria read:
- `POT-WEDGE` "Wedging";
- `WBAY-KILN` "Kiln bay" and `WBAY-GALLERY` "Gallery bay";
- `CONE-GLOST` "Glost cone" and its section "Shelf spacing";
- `HUNCH-0007` "Glaze crawls on damp bisque".

Every other document has a title of its own, and no title repeats an ID.

The fixture holds 181 documents; 26 are in the flow, and 155 are not (141 records, 8 notes, 4 parts and 2 desk papers). The records sort before `docs/workshop/`, so an uncut 40x read fills its first 200 nodes with records. `expected.json` pins check's view, and `spec check` gives 0 errors. The 40x variant is built by the tests in a scratch directory and is never committed: each record is copied 40 times under the same prefixes, and the rest stays as above.

## Rules and edge cases

- WHEN a flow read has no ROOT, every node SHALL be `in_flow: true` and the cut SHALL drop nothing (`not_in_flow` 0).
- WHEN the cut leaves only a ROOT's node, the read SHALL keep that node with `in_flow: false` and exit 0. A stage nothing carries still gives 0 nodes.
- WHEN `--stage` and `--in-flow` are both given, the stage SHALL cut first, and `not_in_flow` SHALL count what the in-flow cut drops from what the stage kept.
- WHEN a record is dropped, its cards and gap names SHALL still count on the kept node holding it: `counts` and `gaps` are computed before any cut (`FlowRead.shown`).
- WHEN a group holds dropped documents, its `holds` SHALL still count them. A box saying "144 documents" that shows 3 inside is true, and is listed under "Known limits" at ship.
- WHEN an ID-less document is in the flow (the ID-less start), it SHALL be drawn. Only the domain-free rule decides (W2).
- WHEN `parent:` dangles or names another project, the chain SHALL end at the node holding it. WHEN two documents hold the parent ID, the first by (path, ord) SHALL be used. WHEN a `parent:` cycle is met, the chain SHALL be the whole graph's, breaking at the cycle's first member by (path, ord), so no node is listed twice.
- WHEN the subset read walks (a mismatch twice, `NotIndexed`, `covers`), `ancestors` SHALL be the walk's, byte-equal.
- WHEN the scope view's node read fails, the path SHALL show only the current entry, its words and Retry, and the canvas SHALL stand.
- WHEN a module is in force, entries above its root SHALL be left out. WHEN the scope lies outside it, the whole chain SHALL show.
- WHEN an off-walk edge's end is selected by the List, a gap item or the hash, the edge SHALL be drawn. Esc's clear hides it again.
- WHEN a picture holds fewer than two stages, no divider SHALL be drawn. A band box never opens a divider.
- **Unchanged:**
  - Gap rows and marks stop nothing (ADR-0012).
  - No link type, kind, stage or rollup word is quoted in `src/flow/` (`policy.test.ts`).
  - Every place link is an `<a>` whose `href` is a `flowHash`.
  - The daemon's text carries `lang="en"` and is never translated.

### Test churn

Only these tests change; every other test passes unchanged.

| Test | Becomes | Why |
|---|---|---|
| CLI `flow_linear.rs`, `trace_views.rs`, `store_reads.rs`, against their `*_before.json` goldens | compare JSON after removing exactly `in_flow`, `not_in_flow`, `types` and `ancestors`; text is unchanged. The goldens and their pinned blobs do not change | new keys (FS-5) |
| core and CLI `tests/flow_read.rs`, every `FlowAsk` literal | `in_flow_only: false` | new field |
| tests listing node keys: CLI `show.rs`, MCP `mcp_read.rs`, `mcp_flow.rs`; http `daemon_keys.rs` (114 to 115) | the new keys | as above |
| MCP `plugin_files.rs` | 0.33.0 `PINS`, `SCHEMA_PINS` | schema change |
| tests over every configured fixture (`store_reads.rs`' differential, `projection`) | `flow-aside` joins | new fixture |
| `flow/geometry.test.ts`, the 0.6 and 0.768 fits | 0.92 | Flow's own floor |
| `flow/layout.test.ts`, columns by distance and arcs under the lane | rows by distance, arcs right of the walk | Q14 |
| `FlowView.test.tsx` "the panel (ST-5) ... name, kind, counts ... ten rows" | title and counts; name and kind under Technical details; rows in Gaps | D10, D12 |
| `FlowView.test.tsx` "names Up the holder a held read gives ..." and "names Up by the holder's title ..." | removed; FS-19 | D11 |
| `FlowView.test.tsx` "orders types by name over the read's edges ..." | the served `types` | D3 |
| `FlowView.test.tsx` "says an edge's kind, its when clipped, its count" | no count on the canvas; the List keeps it | D9 |
| `FlowView.test.tsx` "gap items and the hand-off (D11, cut 2c)" | the same, from the Gaps control | D10 |
| `FlowView.test.tsx` "the places pane lists All stages ... under the current, the scope's groups" | under the path's current entry | D11 |
| `FlowView.test.tsx` "the List holds every node and arrow ..." (`<type> <to name>, <to title>`) | `<type> <to title>` | D12 |
| `FlowView.test.tsx` "the lens root and the reads"; `mocks/flow.smoke.test.tsx` "bounded reads" | the forms with `in_flow` | D6 |
| `flow/labels.test.ts`, `flow/styles.test.ts`, `mocks/flow.test.ts` | served types, the edge label, scale names, the chevron | as above |
| `api/daemonKeys.test.ts` | four keys, set 115 | FS-11 |

## Acceptance criteria

A browser look is marked **[look]**; a timed run, **[measure]**. Each is recorded in "Implementation" with its date. Every other criterion is a test that the named mutation turns red. Numbers below follow from "Data", "Fixture `flow-aside`".

- [ ] FS-1 — **The fixture.** `fixtures/flow-aside` is laid out as "Data" says.
  - `spec check --root fixtures/flow-aside` gives 0 errors and 0 warnings, and `expected.json` matches.
  - `anonymity.rs` passes with the fixture walked, and so do the genre scans with its scheme: core `check_genre.rs` `the_flow_sources_name_no_kind_or_prefix_of_a_fixture`, http `no_domain.rs`, and MCP `mcp_genre.rs`.
  - Mutations, each red: a pilot's name planted in a note (anonymity); the kind `hunch` quoted in core `flow.rs` (check_genre M6).
- [ ] FS-2 — **The rule.** `crates/specengine-cli/tests/flow_scenario.rs` (new).
  - On `flow-aside`, `flow-gaps`, `flow-relay`, `flow-hire`, `trace-almanac`, `spec-c` and `roadmap-a`, let F be the paths of `spec flow --json`. Every node of that read is `in_flow: true`.
  - For every document holding a child, at depth 0, 1 and unbounded, a node is `in_flow` exactly when its path, or a path the unbounded read places under it by `group`, is in F.
  - On `flow-aside`, F has 26 paths. `spec flow ATL-POTTERY --depth 1 --json` gives 24 nodes, 12 of them in the flow: `ATL-POTTERY`, `KRUN-FIRING` and the parts other than `-OFFICE`, `-WATER`, `-FUEL` and `-CANTEEN`.
  - Core `tests/flow_read.rs`: a Tier 3 ROOT holding a live step is `in_flow` by the fold.
  - Mutations, each red:
    - membership taken from the rooted scope (every node true);
    - `parent:` holders left out (`WBAY-GALLERY` false);
    - `[flow]` selection left out (`open-day.md` false);
    - the fold dropped (the Tier 3 ROOT false).
- [ ] FS-3 — **Domain-free.** `flow_scenario.rs` on `flow-aside`:
  - `HUNCH-0007`, a record's kind, is in the flow, and so is `open-day.md`, which has no ID.
  - `PUZZLE-0003`, whose only link is `depends_on` on a step, is not, and neither is any other record or any of the 8 notes.
  - Mutations, each red: `depends_on` counted as a flow link (`PUZZLE-0003` true); a node with no ID taken as out of the flow (`open-day.md` false).
- [ ] FS-4 — **The cut.** `flow_scenario.rs`:
  - `--depth 1 --in-flow --json` gives:
    - `ATL-POTTERY`: 12 nodes, `not_in_flow` 14;
    - `WBAY-KILN`: 4 nodes (`HUNCH-0007`, `WBAY-KILN`, `CONE-BISQUE`, `CONE-GLOST`), 141;
    - `WBAY-OFFICE`: 1 node, `WBAY-OFFICE` with `in_flow: false`, 2;
    - `KRUN-FIRING`: 11 nodes, 0.
  - Without the flag, `not_in_flow` is `null`.
  - Each kept node and edge is byte-equal to the uncut read's. So are `gaps`, `left_out`, `notes`, `root`, `reason` and `depth`.
  - With `--trace`, after a card is made on `PUZZLE-0010` in a scratch copy, `WBAY-KILN`'s `counts` under the cut equal the uncut read's, so the card still counts.
  - The text header ends `; not in the flow: 141`.
  - `KRUN-FIRING --stage glazing --in-flow` gives 2 nodes and 0.
  - Mutations, each red:
    - the cut before the fold (holds change);
    - counting nodes, not documents (14 becomes 12);
    - dropping a ROOT's node (`WBAY-OFFICE` gives 0 nodes);
    - `not_in_flow` 0 without the flag;
    - `counts` computed after the cut.
- [ ] FS-5 — **Default reads stay byte-equal.** The three goldens of "Test churn" pass with only those four keys removed. Every text stream, MCP content included, is byte-equal to the goldens with nothing removed. Mutation: a ` | in flow` field on a text node line, which turns red.
- [ ] FS-6 — **At 40x.** `flow_scenario.rs` builds the 40x variant.
  - `spec flow WBAY-KILN --depth 1 --in-flow --json` gives 4 nodes, `not_in_flow` 5 679, and `truncated` false.
  - Without the flag, the capped read is `truncated` true, and none of `CONE-BISQUE`, `CONE-GLOST` or `HUNCH-0007` is printed.
  - Mutation: the flag ignored, which turns red.
- [ ] FS-7 — **Types.** In `flow_scenario.rs`, every read's `types` equals `specengine_model::FLOW_LINK_TYPES`' names in order, `WBAY-KILN`'s edgeless read included. Mutation: `types` taken from the read's edges (`[]` there), which turns red.
- [ ] FS-8 — **The served path.** `crates/specengine-cli/tests/show_ancestors.rs` (new).
  - On `flow-aside`, `spec show <REF> --json` gives `ancestors` by ID:

    | REF | `ancestors` |
    |---|---|
    | `HUNCH-0007` | `[ATL-POTTERY, WBAY-KILN]` |
    | `POT-FIRE` | `[ATL-POTTERY, KRUN-FIRING]` |
    | `CONE-GLOST-SHELF` | `[ATL-POTTERY, WBAY-KILN, CONE-GLOST]` |
    | `docs/workshop/gallery/open-day.md` | `[ATL-POTTERY, WBAY-GALLERY]` |
    | `ATL-POTTERY` | `[]` |

    Each entry is `{id, kind, title, path}` as "Data" shows. `--links` gives the same list. On `code-a`, where `--bindings` takes the whole read, its lists equal plain `show`'s.
  - On `parent-ref`, `AUK-04`'s list is `[]`.
  - On scratch corpora:
    - a `parent:` dangling two levels up ends the chain there;
    - a parent ID held by two files gives the first by (path, ord);
    - in a cycle A to B with C under A, C's list equals the whole graph's `ancestors` reversed, each member once.
  - `spec show`'s text is unchanged.
  - Mutations, each red: nearest first; the last holder; a cycle member twice; `ancestors` printed in the text.
- [ ] FS-9 — **From the subset.** `crates/specengine-cli/src/store_reads_tests.rs` (`#[cfg(test)]`):
  - `show HUNCH-0007` on `flow-aside` decodes C(REF) plus the two chain files, with whole decodes 0, whole graphs 0 and walks 0.
  - `store_reads.rs`' differential gives equal `ancestors` from the subset and `show_walk` for every ID of every configured fixture, `flow-aside` included.
  - Mutations, each red: the chain from a whole decode; the chain left out of the plan (`[]` against the walk's list).
- [ ] FS-10 — **The twins.**
  - MCP `tests/mcp_flow.rs`: `get_flow {root: "WBAY-KILN", depth: 1, in_flow: true}` gives `structuredContent` equal to the CLI's JSON and content equal to its text. `in_flow: "yes"` is refused. The output schema is closed and holds the new keys.
  - `tests/mcp_read.rs`: `get_node {id: "HUNCH-0007"}`'s `ancestors` equals the CLI's.
  - http `tests/flow.rs`: `GET .../flow?root=WBAY-KILN&depth=1&in_flow=true` gives the CLI's JSON, and `in_flow=yes` gives 400 with the words of "Data". `GET .../nodes/HUNCH-0007` carries `ancestors`.
  - Mutations, each red: the route not passing `in_flow` (145 nodes); `ancestors` missing from `ShowNode`.
- [ ] FS-11 — **Key sets and plugin.**
  - `fixtures/daemon-keys.json` holds the sets of "Data", "Key sets"; http `daemon_keys.rs` counts 115. The UI `daemonKeys.test.ts` passes with `AHEAD_OF_FIXTURE` empty.
  - `plugin.json` is 0.33.0. `plugin_files.rs`' `PINS` and `SCHEMA_PINS` gain an entry, `tool_schemas_follow_the_plugin_version` passes, and `read-spec` names `in_flow` and `ancestors`.
  - `mcp_decision.rs` and `mcp_path.rs` keep their `INSTRUCTIONS` pins.
  - Mutations, each red: the schema changed at 0.32.0; a key missing from the fixture.
- [ ] FS-12 — **[measure] Cost.** `specengine-eval scale --nodes 10000 --seed 1` runs on one machine for the slice's parent commit and for the head.
  - `get_flow_top`, `get_flow_root`, `get_flow_stage` and `get_node` p50 are each at most 10 % above the base.
  - Mutation: membership from a second `flow_read` puts `get_flow_root` near double.
- [ ] FS-13 — **Flow's reads.** `FlowView.test.tsx` "the lens root and the reads":
  - The three drawing forms carry `in_flow: true`.
  - On a cold mount at `#/flow-aside/flow?scope=CONE-GLOST`, Flow's own calls on the stub client are exactly `getFlow(scope form)`, `getNode("CONE-GLOST", {with: ["links"]})` and `getTrace({ref, summary: true})`, with no Top read, no `getTree` and no read of an ancestor.
  - On `aside40`, `WBAY-KILN` draws 3 boxes.
  - Mutations, each red: `in_flow` dropped from `flowForm` (200 record boxes); the path's titles from one `getNode` per ancestor.
- [ ] FS-14 — **Readable boxes.**
  - `flow/geometry.test.ts`: `FLOW_FIT_MIN_ZOOM` is 0.92, and `fitOf({width: 4000, height: 300}, 832, 600)` gives zoom 0.92. `graph/geometry.test.ts` is unchanged: `FIT_MIN_ZOOM` stays 0.6.
  - `flow/styles.test.ts`: the box title is `var(--font-size-body)`, and 0.92 times the token's px is at least 12. `FlowCanvas` passes `minZoom={FLOW_PINCH_MIN_ZOOM}` (0.5), and `GraphCanvas` keeps 0.1.
  - No `SCALE_EXCEPTIONS` entry names RM-0094, and `scale.test.ts` passes.
  - Mutations, each red: Flow importing Graph's floor; the title at `--font-size-small`; a raw `font-size` in `.flow-box`.
- [ ] FS-15 — **Top to bottom.** `flow/layout.test.ts` on `aside`'s `KRUN-FIRING` read:
  - `y` grows with `distance`, and one distance shares one `y`. `POT-MEND` and `POT-PACK` (8) stand side by side.
  - Four dividers, `wheelwork`, `bisque`, `glazing` and `crating`, stand above rows 0, 3, 5 and 7.
  - The repeat edge `POT-FIRE` to `POT-GLAZE` runs right of the walk and crosses no box (`stretchCrosses`).
  - A stage view draws no divider. The band lies below the last row and wraps at four boxes.
  - Mutations, each red: `x` by distance; a divider per row; a divider in a stage view.
- [ ] FS-16 — **Edges.** `FlowView.test.tsx` at `aside`'s lens root:
  - With nothing selected, none of the 9 lifted edges is drawn. Selecting `KRUN-FIRING` draws 9, selecting `WBAY-KILN` draws 1, and Esc's clear draws 0 again.
  - In `KRUN-FIRING`, all 11 edges are drawn unselected.
  - No canvas label holds `x<count>`. The List row of `KRUN-FIRING` to `WBAY-KILN` ends `x5`.
  - `labels.test.ts`: `typesOf` is the served `types`, so a picture lacking a type dashes the others as one holding all three.
  - Mutations, each red: off-walk edges drawn unselected; `x<count>` back on the canvas; `typesOf` from the read.
- [ ] FS-17 — **Gaps.** `FlowView.test.tsx` at `aside`'s lens root.
  - **The toggle.** The bar holds Legend, then Gaps with `aria-expanded="false"` and the warning Lozenge "1 found".
  - **Open:**
    - `two-starts` first, in its plain words;
    - the two `definition-*` rows as not measured;
    - "7 rows find nothing";
    - each code and cause in a collapsed "Technical details", the cause under `lang="en"`.
  - **The items** read "Wedging + Open day". Activating it closes the control, selects by a replace and moves focus to the panel heading. "Hand to your AI" makes one `createCard` and no task.
  - Legend and Gaps are never open together. Esc closes the control, with focus to its toggle. The scope view lists no row.
  - Mutations, each red: the cause unfolded; a row left in the panel; an item by ID.
- [ ] FS-18 — **Records for the AI.** `FlowView.test.tsx` and `mocks/flow.smoke.test.tsx` on `aside`.
  - **The button.** At `WBAY-KILN` it reads "Records for the AI (141)" and reads nothing.
  - **Opening** makes one `getFlow({root: "WBAY-KILN", depth: 1})` and shows a dialog labelled by its `h2`.
  - **At `WBAY-KILN`:** groups `edict` 32, `hunch` 59 and `puzzle` 50, each entry a Spec tree link by title.
  - **At the lens root:** `workbay`'s four entries, `WBAY-OFFICE` with "2 documents", and "8 documents without an ID are in the Spec tree".
  - **On `aside40`:** 200 entries, then "Show all 5679".
  - Esc returns focus to the button. There is no button at an unrooted Top or in `KRUN-FIRING` (N = 0). A stage view without the held scope read shows `flow.records.openPlain`.
  - Mutations, each red: the read on mount; listing `in_flow: true` nodes; listing ID-less nodes.
- [ ] FS-19 — **The path pane.** `FlowView.test.tsx` on `aside`.
  - At `scope=CONE-GLOST`, the "Flow places" nav holds an `ol` named "Path" with "The pottery", "Kiln bay" and "Glost cone". Each is an `<a>` whose `href` is a `flowHash`, and the last is `aria-current="location"`.
  - At the lens root, the groups under it are "Gallery bay", "Firing run" and "Kiln bay", in served order. The other in-flow parts hold nothing. With the stub's `[[module]]` rooted at `WBAY-KILN`, "The pottery" is left out.
  - A failed node read shows the current entry, `flow.pathFailed` and Retry.
  - No Up link renders, and `flow.up`, `flow.upStart`, `useHeldHolder` and `useHeldTitle` exist nowhere in `src/`.
  - Mutations, each red: `aria-current` dropped; the path from held reads; Up restored.
- [ ] FS-20 — **Titles; technical details folded.** `FlowView.test.tsx`:
  - The rendered boxes, List rows, path and group entries, gap items and panel headings hold no ID of a node that has a title. A title-less node shows its ID, and an ID-less one its path.
  - The box view's name, kind, `path:line`, distance and inner, and the scope view's name, kind and path, sit under "Technical details" with `aria-expanded="false"`.
  - Mutations, each red: `flow-places-name` restored; Technical details open by default.
- [ ] FS-21 — **Empty states.** `FlowView.test.tsx`:
  - At `scope=WBAY-OFFICE`, the canvas holds an `EmptyState` titled "Nothing here is part of the flow.", reading "2 documents here are for your AI.", with Records for the AI as its one act.
  - The project, scope, stage and no-stage empties render through `EmptyState`.
  - Mutation: the one-node read drawn as a blank canvas, which turns red.
- [ ] FS-22 — **The chevron.** `flow/styles.test.ts` pins the declarations of "The screen", "The path pane".
  - **[look]** At 1100x800 in Russian, with the mock's longest group title, the chevron sits on the title's first line.
  - Mutation: `flex-wrap: wrap` back on group rows, which turns red.
- [ ] FS-23 — **Catalogs, gates, size, churn.**
  - Every key of "Data", "Catalog keys" is in both catalogs, and `tsc` makes the Russian catalog complete.
  - `app/screens.language.test.tsx` renders Flow in Russian with Gaps, the Records dialog and Technical details open, and no new English except served text.
  - `pnpm lint`, `build` and `test` pass. So do `cargo nextest run -p specengine-eval --test ui_policy --test anonymity --test doc_pointers` and the docs gate.
  - First-load JS is at most 845 kB, printed in "Implementation". If needed, the Records dialog becomes a lazy chunk.
  - Only "Test churn"'s tests change.
  - Mutations, each red: a `ru.ts` key removed; `"follows"` quoted in `layout.ts` (`policy.test.ts`).
- [ ] FS-24 — **[look] The owner's words.**
  - **The setup.** At 1440x900, 1920x1080 and 1100x800, in English and Russian, light, plus 1440x900 dark in both: eight looks. Each is taken on the mock's `aside` (the lens root, `KRUN-FIRING`, `WBAY-KILN` with Gaps and then Records open) and on `spec serve` over `fixtures/flow-aside`.
  - **What each look must show:**
    - DF-10's snippet gives text only at 12, 14, 16, 20 and 24 px outside the canvas, and a canvas transform of at least 0.92 at fit;
    - the walk reads top to bottom with four dividers;
    - no row runs off the right edge;
    - no badge-carrying edge bundle is drawn;
    - the path reads by titles.
  - Each look is recorded with a screenshot name against the owner's words "readable" and "the scenario only".
  - Mutation: a fit floor of 0.6, after which the transform reads 0.6.

## Out of scope

- **RM-0095:**
  - the document modal and page;
  - link ends' `title` and `kind`;
  - the minimal panel;
  - the breadcrumb built on `ancestors`;
  - a record shown inside its document's modal.
- **RM-0099:** Graph's look and its fit; the other screens.
- **RM-0096:** the owner's words.
- **Not done:**
  - a text line for `ancestors` in `spec show`;
  - an `INSTRUCTIONS` change;
  - a kind, path or column filter in the core;
  - a layout library;
  - keyboard zoom;
  - a record count inside a group's box;
  - a cut that changes `holds`;
  - persisting the Records dialog's place in the hash.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

The **rust-developer** writes the Rust lane, which starts at once:

| Area | Files | What changes |
|---|---|---|
| Core | `crates/specengine-core/src/flow.rs` | the membership pass, `in_flow`, `in_flow_only`, `not_in_flow` |
| CLI | `crates/specengine-cli/src/flow.rs`, `src/main.rs` | `--in-flow`, the header, `in_flow`, `not_in_flow`, `types` |
| CLI | `src/show.rs`, `src/subset.rs` | `ancestors` in both answers; `show`'s plan takes the `PARENT` chain |
| MCP | `crates/specengine-mcp/src/read.rs`, `src/mirror.rs` | `FlowArgs.in_flow`; the four mirrors |
| HTTP | `crates/specengine-http/src/app.rs` | `in_flow` on `flow` |
| Plugin | `plugin/specengine/.claude-plugin/plugin.json`, `skills/read-spec/SKILL.md` | 0.33.0; two lines |

The **test-engineer** writes:
- `fixtures/flow-aside/`, new;
- `fixtures/daemon-keys.json`;
- CLI `tests/flow_scenario.rs` and `tests/show_ancestors.rs`, both new, plus `flow_read.rs`, `flow_linear.rs`, `trace_views.rs`, `store_reads.rs` and `src/store_reads_tests.rs`;
- core `tests/flow_read.rs`;
- MCP `tests/mcp_flow.rs`, `mcp_read.rs` and `plugin_files.rs`;
- http `tests/flow.rs` and `daemon_keys.rs`.

The test-engineer also runs the gates, the mutations and FS-12.

The **ui-developer** writes in `ui/**` but `ui/README.md`. The UI lane starts once RM-0093 ships, against the mock. Its files:
- `src/api/{provisional,client,queries}.ts` and `daemonKeys.test.ts`;
- `src/mocks/flow.ts` and its node read;
- `src/flow/{FlowView,FlowCanvas,FlowPlaces,FlowPanel,FlowGaps,FlowList,FlowLegend}.tsx`, `FlowRecords.tsx` (new), `layout.ts`, `geometry.ts` and `labels.ts`;
- `src/styles/app.css` and `scale-exceptions.ts`;
- `src/i18n/{en,ru}.ts`;
- the tests of FS-13 to FS-23.

### Work order

**The Rust lane:**
1. The fixture (test-engineer), while the core's membership and cut are written (FS-1 to FS-4).
2. The CLI, MCP and daemon (FS-5 to FS-7, FS-10).
3. `ancestors` and the subset chain (FS-8, FS-9).
4. The key sets and the plugin (FS-11), together with **UI step 0**: the ui-developer's types-only change to `provisional.ts`, `client.ts` and `daemonKeys.test.ts`, so both halves of the key test stay green. It touches no file RM-0093 edits.
5. The goldens, the gates and FS-12.

**The UI lane**, after RM-0093:
1. The mock.
2. The reads (FS-13).
3. Layout, fit and edges (FS-14 to FS-16).
4. The path pane (FS-19).
5. The bar: Gaps, then Records (FS-17, FS-18).
6. The panel, titles, empty states, chevron and CSS (FS-20 to FS-22).
7. Catalogs and gates (FS-23), then the looks over the Rust lane's daemon (FS-24).

### Risks

- **Scale.** Membership must stay one linear pass per read (FS-12). The chain adds k files to a `show` read, and k is the depth.
- **Genre.** No kind, path or column word enters the core (FS-1, FS-3).
- **Policy.** No link type is quoted in the UI: `types` is served.
- **Graph.** Graph shares `FIT_MIN_ZOOM`, so Flow takes its own floor, and Graph's test is not touched.
- **Hidden marks.** Marks on dropped records must still count on their holders (FS-4).
- **Accessibility:** the dialog's focus return, one overlay at a time, disclosures, the dividers hidden from assistive technology, and long Russian titles wrapping in the path.
- **Size.** 797.95 kB was measured before RM-0093. The dialog becomes lazy if the bar is near.
- **The key test.** It spans both lanes (Work order step 4).
- **Test churn** is listed in "Test churn".
- **Canon caps** are covered under "Canon at ship".
- **Two hashes, one picture.** A cold-mount top entry may write the lens root as `scope=`. It draws the same picture and is listed as a known limit.

### Canon at ship

`spec-flow.md` (12 099 of 12 288 B), `spec-flow-screen.md` (11 623), `mcp-read.md` (12 280) and the http README (10 236 of 10 240) cannot grow, so the detail moves into two new Tier 2 files, both claimed by `docs/index-canon-spec.md`, which has about 3.9 kB free.

- **New `docs/canon/spec-flow-cuts.md`**, scope core, cli, mcp and http, about 3 KB. Its sections:
  - "In the flow", D1 and the fold;
  - "The cuts", the `--stage` bullet moved from `spec-flow.md` with `--in-flow` beside it;
  - "Count and types";
  - "Tests".
- **`spec-flow.md`.** The usage line gains `--in-flow`, `in_flow`. The `--stage` bullet becomes a pointer. "Output" gains the keys and the header. The net change must be negative. The five code comments citing "The read" for the stage cut repoint to the new file.
- **New `docs/canon/spec-flow-bar.md`**, scope `ui`, about 3.5 KB. It covers the Gaps control (moved from `spec-flow-screen.md` "Gap rows"), Records for the AI, and the cut's empty state.
- **`spec-flow-screen.md`.** These sections change:
  - "Gap rows" becomes a pointer;
  - "Reads" gains the forms and `recordsForm`;
  - "Studio" gets the bar;
  - "Panel" gets Technical details;
  - "Keyboard" gets the new arrow keys;
  - "Known limits": Up out, the two hashes in;
  - "Mock" gains `aside` and `aside40`.

  The net change must be at most 0.
- **`spec-flow-place.md`.** "Places" becomes the path, and the "Known limits" entry on Up is replaced.
- **`spec-flow-layout.md`** covers rows by distance, dividers, the band's wrap, off-walk edges, labels without count, `types`, and Flow's own floors.
- **Other canon:**
  - **`spec-cli-read.md` "Output":** `ancestors`.
  - **`spec-read-subset.md` "The subset":** `show REF`'s row takes the `PARENT` chain.
  - **`mcp-read.md`:** `get_flow {root?, depth?, stage?, trace?}` becomes `get_flow` (its arguments: `spec-flow.md`), a net reduction.
  - **http README:** the flow row's arguments become `spec-flow.md`'s, a net reduction.
  - **`plugin-files.md` "Version":** 0.33.0.
- **The plan:** `CLAUDE.md`'s state line, the step 2 row in `08-roadmap.md`, `RM-0094` set to shipped, and this spec compacted to 3 KB or less.

### Working answers and the owner's questions

These working answers are the spec-writer's; the owner's review is open.
- **W1.** The key is `ancestors`, not `path` (D4).
- **W2. Owner's question.** Should an ID-less document that is in the flow stay on the canvas?
  - The owner said documents with no ID belong only in the Spec tree.
  - The working answer is yes, keep it. The domain-free rule draws a document with no ID when a flow link or `[flow]` reaches it (the fixture's "Open day"). Hiding it would mean the core telling documents apart by ID shape.
  - The 8 notes with no links leave the canvas, as he asked.
- **W3.** A part holding nothing of the flow is not drawn, as Flow v2 Q2 recommended, and stands in Records for the AI as one entry with its count.
- **W4.** The Records button sits at the foot of the places pane, as in the mockup, and opens a dialog rather than a pane.
- **W5.** Records are grouped by raw kind. A sentence is not split by kind word, and the dialog is labelled as the AI's information.
- **W6.** The fit floor stays at 0.92, as the plan item says, giving 12.9 px titles. The bare 12 px minimum would allow 0.86.
- **W7.** Dividers stand where a stage first appears. Interleaved stages draw no second divider for a stage already begun.

<!-- On shipping (status: shipped + shipped: date) the document is compacted to "Why", the criteria and a summary <= 3 KB; whatever became true moves into the canon. -->
