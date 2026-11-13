---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, ui]
ref: RM-0112; the owner's goal of 2026-11-13 (see what is used where; he likes Obsidian's dependency map); his Q3 answer, the whole corpus at once; the graph filters he chose that day; the analyst's analysis of 2026-11-13, slice 2 (criteria 10-12); an earlier prototype's graph filters
adrs: [ADR-0080]
---

# corpus-map: the whole corpus as one map, a node's links both ways, and the graph's filters

## Why

The owner wants to see what is used where, as Obsidian's global graph shows it: every document at once, the most used ones large, orphans floating apart. Today the Graph needs a REF and walks one way from it. It caps at 200 boxes and cannot show the whole corpus, a node's users and uses together, or a direction's documents together.

This slice adds a whole-corpus map, laid out by force and drawn on a canvas (ADR-0080). It sizes nodes by `usage-orphans`' counts and marks its orphans. It also adds the filters the owner chose: only broken links, only orphans, unresolved targets as ghost nodes, kind chips, and a fixed colour per kind paired with a label. `spec graph` gains `--both` and a direction start (`--kind`), and the local graph gains a Both ways mode.

## Description and interactions

### Decisions

- **D1. Whole corpus, one read** (the owner's Q3). `spec map` serves every live node with its resolved edges, use counts, orphan flag and parents, plus the unresolved targets. The daemon serves it as `GET …/map`. There is no MCP tool: an agent has `get_node` and `get_impact`. The CLI's text is cut at `OUTPUT_CAP_CHARS`; its JSON is never cut, as `spec health`'s, because the JSON's edges name nodes by index.
- **D2. Documents by default, sections on request.** The read serves both, each section with `of`, its document's index. The UI draws documents and folds each section's edges onto its document; the Sections switch draws them apart. Folding maps an end to a served index and merges duplicates. It counts no use (`docs/canon/architecture.md#ui`).
- **D3. One count** (`usage-orphans` D1). `used_by`, `mentioned_by` and `orphan` come from core `usage_of`. A node's size follows `used_by`.
- **D4. The layout engine** (ADR-0080). `d3-force` 3.0.0 runs `MAP_TICKS` ticks from its seeded start on a 2D canvas, in a lazy chunk. `sigma` with `graphology` was judged and rejected (ADR-0080 "Why"). There is no WebGL and no worker.
- **D5. A colour per direction, grey otherwise.** Kinds are the project's words (ADR-0031), so no colour is keyed on a kind's text. A kind takes the slot of the first direction (served order) listing it, slots 1-8 in direction order. Any other kind, and a ninth direction onward, is grey. A colour always comes with words (the legend, the hover card, the List).
- **D6. The canvas rules hold** (`ui/README.md` "Screen rules"): read-only; the wheel scrolls the page; a press of up to 5 px is a click; zoom by key and button; a List twin; lazy. A click opens the document view, and hovering lights a node's neighbours.
- **D7. Filters never lay out again.** Kind chips, Orphans only, Broken only, ghosts and mentions hide or dim nodes and keep positions. Only a change of the node set lays out again: Sections, Include archive, or a new answer.
- **D8. At most 5 000 nodes drawn** (`MAP_DRAWN_MAX`). Past that, the most used are kept, ties in served order, chosen before layout. A line names the rest, and the List lists all.
- **D9. `spec graph` grows two flags**, both on the route.
  - `--both` follows each type both ways and is refused with `--impact`.
  - `--kind K…` starts from every live node of those kinds instead of a REF.

  On the screen, the local graph gets Both ways, and the Map's kind chips and Direction pick serve as the direction start.
- **D10. The Map is the Graph section's home.** `#/<p>/graph` with no REF opens `graph/GraphHome.tsx`. It has a Map view and, once `scope-rings` lands, a Rings view, each a lazy chunk; with a REF, the section keeps the local graph.

### What the owner sees

Graph, with no node chosen, now opens the Map. Every document of the project is a dot:
- the more documents use it, the larger it is;
- its colour follows its direction, named in the legend;
- orphans are drawn with a dashed ring.

Hovering a dot lights it, its neighbours and their links, and shows a card with the title, kind, "Used by N · Mentioned by M" and the place. A click opens the document. A search field dims all but the matches; arrow keys step through them and Enter centres one. Chips filter by kind; switches show sections, parents, mentions, unresolved targets (ghost dots), only orphans or only broken links. Picture and List switch views; the List names every node with its counts. A node's own graph gains "Both ways": what it uses and what uses it, on one canvas.

## Data

### `spec map [--archive] [--json]` (core `map.rs`, CLI `map.rs`, both new)

The input is `spec tree`'s: one `update`, the whole input, `SpecGraph::new` once (`spec-cli-graph.md` "Input").
- **Nodes:** each node of each admitted file (live, plus Tier 3 under `--archive`, never generated): documents (ID-less ones by path) and ID sections, in (path, ord) order.
- **Edges:** resolved links between two admitted nodes, `src` not equal to `dst`, one per (src, dst, type), with `places` counting the written places. `src` is the link's source (`spec-cli-graph.md` "Parents and links"); `dst` is the node it lands on, a section if it lands on a section. `weak` is set for `mentions`. They are sorted by (src, dst, type).
- **Parents:** each admitted document whose `parent:` resolves (first holder by (path, ord)) to an admitted node, sorted by child.
- **Ghosts:** each written end of a `dangling` link from an admitted file, grouped by its written text: `sources` are the distinct source indexes, sorted; `reason` is the first place's; `places` counts them. Ghosts are sorted by `written`. `skipped` and `unchecked` ends are no ghosts.
- **Usage:** `used_by` and `mentioned_by` for every node, `orphan` for documents and `null` for sections, from core `usage_of`. They always count live sources, whatever `--archive` says.

JSON, on `fixtures/spec-a` (19 nodes; abridged):

```json
{"archive": false, "notes": [],
 "counts": {"documents": 12, "sections": 7, "edges": 35, "parents": 3, "ghosts": 0, "orphans": 1},
 "nodes": [
  {"name": "docs/features/stamina-tuning.md", "id": null, "kind": null, "title": "Stamina tuning", "path": "docs/features/stamina-tuning.md", "line": 1, "of": null, "status": "draft", "archived": false, "used_by": 0, "mentioned_by": 1, "orphan": false},
  {"name": "A-102", "id": "A-102", "kind": "requirement", "title": "The lantern burns one unit of oil per minute", "path": "docs/records/A/A-102.md", "line": 1, "of": null, "status": "open", "archived": false, "used_by": 0, "mentioned_by": 0, "orphan": true},
  {"name": "RULE-STAM-REGEN", "id": "RULE-STAM-REGEN", "kind": "rule", "title": "Regeneration", "path": "docs/spec/movement/stamina.md", "line": 21, "of": 16, "status": null, "archived": false, "used_by": 2, "mentioned_by": 3, "orphan": null}],
 "edges": [{"src": 13, "dst": 16, "type": "depends_on", "weak": false, "places": 1}],
 "parents": [{"child": 11, "parent": 9}, {"child": 13, "parent": 11}, {"child": 16, "parent": 11}],
 "ghosts": []}
```

`edges` is 35: CM1's oracle on the fixture, worked out by hand from `show --links` on 2026-11-13, with the test pinning it. 25 of the 35 are `mentions`, and DEC-0023's `supersedes` of the Tier 3 DEC-0007 is out. The key sets are these:
- `MapDocument` (7): `archive, notes, counts, nodes, edges, parents, ghosts`;
- `MapCounts` (6): `documents, sections, edges, parents, ghosts, orphans`;
- `MapNode` (12), as above;
- `MapEdge` (5): `src, dst, type, weak, places`;
- `MapParent` (2): `child, parent`;
- `MapGhost` (4): `written, reason, sources, places`.

`notes` hold the walk's warnings, as `spec tree` words them (a cycle, several holders), sorted, each once.

Text, each value through `one_line`:

```text
map | nodes 19 (12 documents, 7 sections), edges 35, parents 3, ghosts 0, orphans 1
docs/features/stamina-tuning.md | - | used by 0, mentioned by 1 | Stamina tuning | docs/features/stamina-tuning.md:1
A-102 | requirement | used by 0, mentioned by 0 | orphan | The lantern burns one unit of oil per minute | docs/records/A/A-102.md:1
RULE-STAM-REGEN | rule | used by 2, mentioned by 3 | of MEC-STAMINA | Regeneration | docs/spec/movement/stamina.md:21
MEC-SPRINT --depends_on--> MEC-STAMINA
DOM-MOVEMENT in DOM-GAME
ghost MEC-GONE | from MEC-SPRINT, MEC-STAMINA | places 2
```

The `ghost` line is from CM2's copy; the fixture has no ghost. A node's tail is ` | orphan` and ` | archived`; an edge adds ` | places <p>` when there is more than one place. The cut falls at a line, keeping the header and at least one line, with the tail `[truncated: <k> of <n> lines not shown; the daemon's map is whole]`. Exit 0 when answered, zero nodes included; 2 on usage, no project, a config, `HOME` or store error. Never 1.

**Daemon.** `GET /api/projects/:p/map[?archive=true]` serves `spec map --json` less its final LF, in the project's turn, as `tree` is served (`run_read`). A query name other than `archive`, or a bad boolean, is a 400 naming it; exit 2 is a 503. The 404 list names `map` after `catalog`.

### `spec graph` (CLI `graph.rs`)

```text
spec graph (REF | --kind K…) [--both | --impact] [--type T]… [--depth N] [--archive]
```

- **`--both`** follows every strong type both ways (with `--type`, those types), breadth-first, each node once. `types` lists each followed pair, a type's `out` before its `in`. `--both --impact` exits 2: `` spec graph: `--both` and `--impact` exclude each other ``.
- **`--kind K`** (free, repeatable, matched exactly as `spec tree --kind`) starts at distance 0 from every live node of those kinds (`--archive`: Tier 3 too), in (path, ord) order. Its errors:
  - none found: `nodes 0, edges 0` with the note `` graph: no node of kind `<k>` ``, exit 0;
  - a REF with `--kind`: exit 2, `` spec graph: give a REF or `--kind`, not both ``;
  - neither: exit 2, `` spec graph: give a REF or `--kind` ``.
- JSON gains `both` (bool) and `kinds` (the list, or `null`). Under `--kind`, `ref` is `null`. The route's query names stay its echo keys: `both=true`, `kinds=K&kinds=L`. A CLI exit 2 is the route's 400 with that line. The `GraphView` key set grows by 2.

### The UI

- **Seam.**
  - `provisional.ts` gains `MapDocument`, `MapCounts`, `MapNode`, `MapEdge`, `MapParent`, `MapGhost`, and `GraphView.both`, `.kinds`.
  - `client.ts` gains `getMap(project, {archive}, signal)`, and `GraphOptions` gains `both?` and `kinds?`. `http.ts` and `queries.ts` follow (`useMap`, key `["map", p, archive]`, in `READS_OF_THE_SPEC`, so an apply or a gap reads it again).
  - The mock (`mocks/map.ts`, new): harbor-sim has 30 nodes over two directions' kinds, an unlisted kind and a `null` kind, 4 sections, 2 orphans and 3 ghosts. `large` has 5 000 nodes and 15 000 edges from a fixed generator.
- **Home** (`graph/GraphView.tsx`'s no-REF branch, which mounts `graph/GraphHome.tsx`):
  - an `h1` "Graph";
  - with two views, a "View" group of `aria-pressed` buttons (Map, Rings), the choice kept per project for the page's life in the Shell's graph memory;
  - with one view, no switch.

  Whichever of `corpus-map` and `scope-rings` lands first creates `GraphHome.tsx` with its own view; the second adds its view and the switch.
- **The Map** (`ui/src/map/`, the lazy `MapView.tsx`). Its modules:
  - `layout.ts`: pure, d3-force;
  - `draw.ts`: takes a `CanvasRenderingContext2D`-shaped interface;
  - `hit.ts`: a uniform grid;
  - `colours.ts`: `slotOf(kind, directions)`;
  - `filters.ts`: pure;
  - `MapCanvas.tsx`, `MapControls.tsx`, `MapLegend.tsx`, `MapList.tsx`.

  The constants are `MAP_TICKS` 200, `MAP_DRAWN_MAX` 5 000, `MAP_LIST_PAGE` 200, `CLICK_DISTANCE` from `graph/geometry.ts`, and radius `3 + 2·√used_by` px at most 18 (ghost 3).
  - *Layout:*
    - `forceLink` (distance 40), `forceManyBody` (`distanceMax` 400), `forceCenter`, and weak `forceX` and `forceY` (0.05), so orphans float near the edge;
    - `alphaDecay` set so alpha reaches `alphaMin` at tick 200;
    - the d3 default random source;
    - ticks run in slices of at most 16 ms between frames, with a progress line "Laying out N nodes…" (`aria-busy`).
  - *Drawing:*
    - edges are batched by style: strong solid `--map-edge`, mentions thin dashed `--map-edge-weak`, parents dotted;
    - nodes are filled with their slot's role; orphans get a dashed ring and ghosts a hollow dashed circle in `--map-ghost`;
    - labels go on the 40 largest nodes and on lit ones, from zoom 0.8 up, in `--map-label` over a `--map-surface` halo;
    - a lit set keeps full alpha and the rest are drawn at `MAP_DIM_ALPHA` 0.15;
    - a redraw is coalesced per frame.
  - *Pointer:*
    - hovering lights the node, its drawn neighbours both ways and their edges, and shows a card (title, else name; kind words; "Used by N · Mentioned by M"; "Orphan"; `path:line`);
    - a press moving at most 5 px opens the document view (`doc=<name>` on the route);
    - farther, it pans;
    - the wheel event is never `preventDefault`ed.
  - *Keyboard:*
    - the canvas is one tab stop, `aria-hidden` within a labelled region whose description is the count line;
    - `+`/`=` and `-` zoom (0.2 to 4), `0` fits, arrows pan 64 px, Esc clears;
    - `?` lists them;
    - the search field (`map.search`) dims all but the matches (`normalize("NFC").toLowerCase()` over name and title); ArrowDown and ArrowUp step through the matches, Enter centres and lights the current one, and Enter again opens it;
    - a polite live region says "N matches", then the lit node's title, kind and counts;
    - the lit node's pane offers Open document, Show in graph (`#/<p>/graph/<name>`) and Show in Spec tree.
  - *Controls:*
    - kind chips, one per kind present: slotted kinds first, by slot, then the rest in code-point order, `null` last ("No kind"). Each chip shows a swatch, words and a count, `aria-pressed`;
    - a Direction select (All, then the served directions), which presses only its kinds' chips;
    - switches: Sections, Parents, Mentions (on), Unresolved targets, Orphans only, Broken only (a ghost's sources and the ghosts), Include archive (reads again);
    - Zoom in, Zoom out, Fit;
    - the count line: "N documents, E links, O orphans" (+ "Showing 5 000 of N…").
  - *Legend:* each slotted direction's title and kinds, then "Other kinds" in grey, and the marks: orphan, ghost, strong, mention, parent.
  - *List* (Picture | List, as Graph's tabs): a table of the nodes the filters keep, in served order: Name (`DocLink`: title, else name, the name in mono), Kind (the direction's title and the raw kind), Used by, Mentioned by, Links (drawn "out · in"), Marks (Orphan; "N broken"). 200 a page, then "Show 200 more".
  - *States:* a skeleton while reading; a failure as an `ErrorPanel` with the daemon's words and Retry; no node as an `EmptyState`.
- **Local graph, Both ways** (`graph/{settings.ts,GraphControls.tsx,GraphView.tsx,lines.ts}`): `GraphSettings.mode` is `"out" | "both" | "impact"` (was `impact: boolean`), each with its own `bases`. The mode group reads Links, Both ways, Impact. `graphOptions` sends `both: true`. Chips name each (type, direction) pair as the answer's `types` give them.
- **Tokens** (`tokens.css`): `--map-surface`, `--map-kind-1` to `--map-kind-8`, `--map-kind-other`, `--map-edge`, `--map-edge-weak`, `--map-ghost` and `--map-label`, each `light-dark()`. The kinds, the ghost and the edge are at least 3:1 against the surface in both themes, and the label at least 4.5:1.
- **Strings:** about 45 leaves under `map.*` and `graph.home.*` in `en.ts` and `ru.ts`, the Russian a draft. **CSS:** `.map-*` and `.graph-home-*` in `app.css`.
- **Dependency** (ADR-0080):
  - `dependencies` get `d3-force` 3.0.0; `devDependencies` get `@types/d3-force` at its newest exact version at least 7 days old. Both are frozen in `pnpm-lock.yaml`. `d3-quadtree` joins the lockfile only, beside `d3-dispatch` and `d3-timer`.
  - Only `ui/src/map/` imports `d3-force`.
  - **Budget:**
    - first-load JS grows at most 2.5 kB and stays within 845 kB;
    - the `MapView` chunk is at most 60 kB raw and 20 kB gzip, d3-force's share recorded;
    - past the first-load figure, the `map.*` leaves move into the chunk's own catalog part (`ui-languages.md` "Size").

## Rules and edge cases

- WHEN two nodes share an ID, each SHALL be drawn, and `notes` SHALL say so. An edge to that ID SHALL land on the first holder by (path, ord), as resolution does.
- WHEN sections are folded, an edge whose ends fold to one document SHALL not be drawn. Duplicates SHALL merge, their `places` summed, and SHALL count once in the List's Links.
- WHEN Orphans only is on, sections SHALL be hidden: they carry no flag.
- WHEN Broken only is on and there is no ghost, the picture SHALL show the `EmptyState` "No unresolved link".
- WHEN the answer changes (an event), the picture SHALL lay out again and keep zoom and pan. A lit node that is gone SHALL be cleared, and the live region SHALL say so.
- The same answer, Sections switch and archive SHALL give the same positions in one engine.
- The UI SHALL NOT count a use, compute an orphan or resolve a link. It SHALL key no colour on a kind's text, and it SHALL import `d3-force` only in `ui/src/map/`.

## Acceptance criteria

A browser look is marked **[look]** (the orchestrator's, in Chrome), and a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Code comments cite `docs/features/corpus-map.md` "Acceptance criteria" with an ID, never "Data" (`doc_pointers`). "The copy" is a scratch copy of `fixtures/spec-a`.

- [ ] CM1 — **The map read.** On the copy, `spec map --json` has the counts from Data (`documents` 12, `sections` 7, `parents` 3, `ghosts` 0, `orphans` 1), nodes in (path, ord) order, and RULE-STAM-REGEN `of` MEC-STAMINA's index. Each node's usage equals `usage-orphans`' table. The oracle is the union, over every live node X, of `spec graph X --depth 1 --type T` for each type in `types` and for `mentions`, resolved edges only, merged per (src, dst, type); the edges equal it. `--archive` gives `documents` 13 and DEC-0007 `archived` with its `supersedes` edge, while the counts stay those of the live files. Mutations, each red: an edge per place; self-edges kept; Tier 3 nodes without `--archive`.
- [ ] CM2 — **Ghosts.** The copy's `sprint.md` gains `links: {depends_on: [MEC-GONE]}`, and `stamina.md` a wiki mention of `MEC-GONE`. That gives one ghost: `written` MEC-GONE, `sources` [MEC-SPRINT's, MEC-STAMINA's], `places` 2. The `project:` link `shared:DEC-0023@1` gives none. Mutations, each red: skipped links as ghosts; a ghost per place.
- [ ] CM3 — **Text, route, one answer.** The text holds Data's lines; at a forced small cap the tail shows, and the JSON stays whole. `GET …/map` and `?archive=true` are byte-equal to the CLI. `?x=1` and `archive=yes` are 400s naming the name. The copy at two roots and two `HOME`s gives byte-identical text and JSON. `fixtures/daemon-keys.json` holds the six sets. Mutations, each red: the JSON cut with the text; `notes` in input order.
- [ ] CM4 — **Both ways.** `spec graph MEC-STAMINA --both --depth 1 --json` reaches the following, each at distance 1: MEC-SPRINT, DEC-0023 (`canon` in), A-101 and R-12 (`derived_from` out), TERM-exhausted (`uses_term` out). Its `types` list `out` then `in` for each type. `--both --impact` exits 2 with the line, and `?ref=MEC-STAMINA&both=true` is byte-equal. Mutation, red: the in direction not followed (DEC-0023 missing).
- [ ] CM5 — **A direction start.** `spec graph --kind mechanic --depth 0` gives MEC-SPRINT and MEC-STAMINA at distance 0, with `ref` `null` and `kinds` `["mechanic"]`. `--kind nothing` gives `nodes 0, edges 0` and the note, exit 0. A REF with `--kind` exits 2, and `?kinds=mechanic` is byte-equal. Mutation, red: kinds matched by prefix (`--kind mech` finds both).
- [ ] CM6 — **Layout, one picture** (`map/layout.test.ts`). The mock's answer laid out twice gives identical positions, with no `NaN` and exactly `MAP_TICKS` ticks (spied). Run in slices of 1 tick, it equals the run in one slice. With Sections off, the section nodes are not laid out. Mutations, each red: `Math.random` as the random source; ticks counted by time.
- [ ] CM7 — **Drawing and pointer** (`MapCanvas.test.tsx`, `draw.test.ts`, with a recording context and `getContext` stubbed):
  - radii follow `used_by`;
  - a direction's kind fills with its slot's role, an unlisted kind and a ninth direction with `--map-kind-other`;
  - orphans get the dashed ring;
  - hovering a node draws it and its neighbours at full alpha and the rest at 0.15;
  - a press moving 4 px sets `doc=<name>` on the route, and one moving 6 px pans and sets nothing;
  - a wheel event leaves `defaultPrevented` false;
  - `+`, `-`, `0` and the arrows act only with the canvas focused, and never in the search field.

  Mutations, each red: the wheel zooming; slots by a kind's text order; `CLICK_DISTANCE` 0.
- [ ] CM8 — **Filters and the List** (`filters.test.ts`, `MapView.test.tsx`):
  - each kind chip hides its nodes and their edges;
  - Orphans only keeps the 2 orphans; Broken only keeps the 3 ghosts and their sources; Unresolved targets draws the ghosts;
  - Sections draws the 4 sections, their edges unfolded;
  - Parents draws the parents' edges, and Mentions off hides the weak edges;
  - the Direction select presses only its chips;
  - no filter but Sections and Include archive moves a position;
  - the List shows exactly the kept nodes, with counts and marks;
  - search with ArrowDown and Enter lights the second match and announces it, and Enter again opens it.

  Mutations, each red: a filter that lays out again; the List ignoring the chips.
- [ ] CM9 — **The cap.** A built answer of 5 001 nodes draws 5 000, leaving out the least used, last in served order on a tie. The count line names the hidden one, and the List holds 5 001 rows over pages. Mutation, red: the first 5 000 in served order.
- [ ] CM10 — **Both ways on the screen** (`GraphView.test.tsx`, `settings.test.ts`). Both ways sends `both=true`; its chips name each (type, direction) pair; the other two modes keep their bases; a reload of the REF keeps the mode. Mutation, red: Both ways sending `impact`.
- [ ] CM11 — **The dependency and the bundle.**
  - `package.json` and `pnpm-lock.yaml` hold `d3-force` 3.0.0 and the types at exact pins, installed frozen.
  - `ui_policy.rs` takes the allowlist of 21, with ADR-0080 beside ADR-0036 and ADR-0072. Its policy test finds `d3-force` imported only in `ui/src/map/`, and something there imports it.
  - `lazy.test.ts` finds `MapView` lazy.
  - After `pnpm build`, the chunk is not in first load. First load stays within 845 kB, at most 2.5 kB over the figure before this slice. The chunk is within 60 kB raw, and the figures are recorded.

  Mutations, each red: a static import of `MapView`; `d3-force` imported in `graph/`; a `^` on a pin; a 22nd package.
- [ ] CM12 — **[measure] 5 000 nodes in 2 s.** The orchestrator's: Chrome, a production build, `?scenario=large`, the Performance panel from the answer's arrival to the first whole draw. It must take at most 2 s on the owner's laptop. A hover redraw must take at most 33 ms. Both are recorded. Past either, `MAP_TICKS` drops by node count or the draw is batched further, a "Known limits" line is added, and the bar is never raised. Read cost: eval `map_scale.rs` (one default check, the measures `#[ignore]`), on `catalog_scale.rs`'s generated tree, warm, release, median of 5. It must take at most 1 s at 10k nodes; 1k and 100k are recorded.
- [ ] CM13 — **Tokens and words.** `tokens.test.ts` finds the roles and their contrasts in both themes. `policy.test.ts`, `words.test.ts` and `scale.test.ts` pass. `map.*` has every key in `ru.ts`. Mutation, red: a kind role at 2.5:1.
- [ ] CM14 — **[look]** In Chrome, both themes and both languages, at 1920x1080 and 1100x800, on the mock and on this repository's own corpus through `spec serve`:
  - the Map at rest, a hover and a search;
  - each switch, the legend and the List;
  - a 6 px drag that opens nothing;
  - the wheel scrolling the page;
  - Both ways on a node.

  Keyboard only: Tab reaches the search, the chips, the switches, the canvas and the List. VoiceOver reads the count line and a lit node.
- [ ] CM15 — **Gates.** These pass:
  - `cargo nextest run` for `specengine-core`, `-cli` and `-http`, then the full run once;
  - eval `doc_pointers`, `ui_policy`, `anonymity` and `workspace_names`;
  - UI lint, `tsc`, `pnpm test` and `pnpm build`;
  - `export index`, then `check`.

## Out of scope

- An MCP tool for the map.
- WebGL, a Web Worker, stored or saved positions, and dragging a node.
- Clusters, communities or a time slider.
- Arrowheads: the direction is in the card and the List.
- The local graph's kind chips and colours: it keeps its look.
- Coverage findings.
- Scope rings (`scope-rings`, RM-0113).
- A map in `docs/generated/`.
- Editing from the map: every write stays a proposal or a card.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | core `src/map.rs` (new), `src/lib.rs` (`pub mod map;`); CLI `src/map.rs` (new), `src/graph.rs` (`--both`, `--kind`), `src/lib.rs` (`Outcome::Map`, the module, its exports), `src/main.rs` (`Command::Map`, graph's flags); http `src/app.rs` (the route, graph's two names, the 404 list) |
| test-engineer, Rust | core `tests/map.rs`; CLI `tests/map.rs`, `tests/graph.rs` (CM4, CM5); http `tests/map.rs`, `tests/graph.rs`, `daemon_keys.rs` with `fixtures/daemon-keys.json` and `daemonKeys.test.ts`, `hub.rs` `GET_ROUTES`; eval `tests/map_scale.rs`, `ui_policy.rs` (the allowlist of 21, the import confinement) |
| ui-developer | `ui/package.json`, `ui/pnpm-lock.yaml`; `src/map/*` (new); `graph/{GraphView.tsx,GraphHome.tsx,settings.ts,GraphControls.tsx,lines.ts}`; `api/{provisional,client,http,queries}.ts`; `mocks/{map.ts,MockClient.ts}`; `test/builders.ts`; `styles/{tokens,app}.css`; `i18n/{en,ru}.ts` |
| test-engineer, UI | `map/*.test.ts(x)`, `GraphView.test.tsx`, `settings.test.ts`, `app/lazy.test.ts`, `policy.test.ts`, `styles/tokens.test.ts`, `mocks/map.smoke.test.tsx`; CM6-CM13's mutations and figures |

`ui_policy.rs` reads `ui/README.md` and caps it at 8 192 B (8 188 now): the README's two new rows at ship replace text there (see "Canon at ship").

### Overlaps

- **`delivery-timeline`** (in flight) shares CLI `src/{lib,main}.rs`, http `src/app.rs`, the keys fixture with its two tests, and in the UI `api/{provisional,client,http,queries}.ts`, `MockClient.ts`, `test/builders.ts`, `tokens.css`, `app.css`, `en.ts`, `ru.ts`, `app/lazy.test.ts` and `policy.test.ts`. All of these wait for its commit.
- **`release-hardening`** (in flight): R2 edits CLI `main.rs`, and lane U edits `app.css`. This slice's `main.rs` edit follows R2.
- **`usage-orphans`**: this slice calls `usage_of`, so its core waits for that ship. Both edit core `src/lib.rs`, `provisional.ts`, `builders.ts`, the catalogs, `app.css` and the keys fixture.
- **`scope-rings`** shares core `src/lib.rs`, CLI `src/{lib,main}.rs`, http `src/app.rs`, `graph/{GraphView,GraphHome}.tsx`, the seam files, the catalogs, `app.css`, `lazy.test.ts` and the keys fixture.

### Work order

1. **Now:** the UI's own files (`src/map/*` with a built answer, the layout and draw tests) and core `map.rs` against a stub of `usage_of`. Neither touches an in-flight file.
2. **After `usage-orphans` ships and `delivery-timeline` commits:** the CLI and http registration (`lib.rs`, `main.rs` after R2, `app.rs`), then `graph.rs`. The shared Rust files go one lane at a time with `scope-rings`.
3. **The UI seam and home:** the shared UI files, one lane at a time; then `GraphHome.tsx` under its rule (D10); then Both ways.
4. **Tests** overlap; then the keys fixture, written once after both new routes exist (or once per lane, one at a time); then reviews, CM12's measures, the look (CM14) and ship. At most three iterations a lane.

### Canon at ship

| File | Edit |
|---|---|
| `docs/canon/spec-map.md` | "State" goes; the read, its JSON and text, the route, the graph flags, the UI (Home, the Map, filters, the List, tokens), "Known limits" (the cap, colours past 8 directions, per-engine positions, the 2 s figure), "Tests" |
| `spec-cli-graph.md` "spec graph" | `--both` and `--kind`, pointing at `spec-map.md`. Net ≤ 0 (16 B free) |
| `ui/README.md` "Dependencies" and "Screen rules" "Canvas" | the two rows, "the 21"; the canvas line naming the Map's 2D canvas. Net ≤ 0 (`ui_policy.rs` 8 192 B cap) |
| `ui-design.md` "Themes" "Canvases" | the map roles, pointing at `spec-map.md`. Net ≤ 0 (4 B free) |
| CLI and http `README.md` | the command and the route, each by replacing text. Net ≤ 0 |
| `ui-languages.md` "Size" | the figure, and `MapView` among the lazy chunks. Net |
| `CLAUDE.md` "State", 08 Phase 7, RM-0112 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers

- **The owner's:** the whole corpus (Q3), a new dependency with an ADR, and the filters.
- **The orchestrator's:** `d3-force` on a 2D canvas, a lazy chunk, exact pins, a fixed tick count, a List twin, keyboard reach, no wheel zoom, 5 000 nodes in 2 s.
- **The spec-writer's:**
  - `sigma` with `graphology` rejected (ADR-0080);
  - D1, no MCP tool, with the JSON uncut;
  - D2, documents by default;
  - D5, colours by direction slot;
  - D8, the cap by use;
  - D9, the direction start as chips on screen and `--kind` on the CLI;
  - D10, the Map as the Graph section's home;
  - a click opens the document, while search and Enter light a node, so a click never needs a second press;
  - mentions drawn by default, dashed and light, as Obsidian draws every link.
