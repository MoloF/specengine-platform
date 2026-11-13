---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# The corpus map: every document and its links in one picture

Built by `corpus-map` (RM-0112, 2026-11-13); ADR-0080 chose the layout engine. Related: the counts that size a node, `spec-usage.md`; one node's walk, `spec-cli-graph.md` "spec graph"; the Graph home's other view, `spec-rings.md`; the other canvases, `ui/README.md` "Screen rules". No MCP tool: an agent has `get_node` and `get_impact`.

<a id="the-rule"></a>
## The rule

The map draws the whole live corpus as one picture: each document, and on request each ID section, with its resolved links. The daemon serves the nodes, the edges, each node's use counts and orphan flag, and the unresolved targets. The UI lays them out and draws them. It never counts a use, computes an orphan or resolves a link, and it reads, stages or writes nothing.

- **Layout.** `d3-force` at an exact pin, imported only in `ui/src/map/`. It runs a fixed number of ticks (`MAP_TICKS`) from its seeded start: d3's phyllotaxis placement and its fixed-seed random source, never `Math.random`. Nodes go in in served order, so one answer gives one picture. Ticks run in slices between frames, so a progress line paints, and the result equals one uninterrupted run. Filters hide or dim nodes and never lay out again; only a change of the node set does, such as showing sections.
- **Drawing.** One 2D `<canvas>`, sized to its box at the device's pixel ratio. Colours come from `tokens.css` roles, read through a hidden swatch per role and read again when the theme changes. A node's size follows its use count. Its colour is its kind's: the drawn set's kinds ranked by how many nodes hold each, ties in code-point order, the first 8 take slots 1-8, any other kind and no kind grey. Count decides a slot, never what a kind says. A colour is always paired with words: the legend, the hover card and the List.
- **The canvas rules** (`ui/README.md` "Screen rules"):
  - the map is read-only;
  - the wheel scrolls the page and is never taken;
  - a press moving at most `CLICK_DISTANCE` (5 px) is a click, which opens the document view; past it the press pans;
  - zoom is by key and by button, with a fit button;
  - the picture is `aria-hidden`; a search field and a List twin reach every node by keyboard;
  - the chunk is lazy, never in first load.
- **The cap.** At most 5 000 nodes are drawn: those used most, ties in served order. A line says how many are hidden; the List lists all.

## spec map

`spec map [--archive] [--json]`, core `map.rs` (`corpus_map`, `node_names`, `MapNaming {scheme, features}`), CLI `map.rs` (`map`, `MapRequest {archive}`, `MapOutcome`): `spec tree`'s input (`spec-cli-graph.md` "Input"), computed per read, never stored.

- **Nodes**: each node of each admitted file (live; Tier 3 under `--archive`; never generated), documents and ID sections, by (path, ord); a section's `of` is its document's index.
- **Names**: canonical, as `collision-names`: `slug/ID` in a feature document when the ID is feature-scoped or held more than once in any file; else the ID; else the path (`fixtures/spec-a`: `stamina-tuning/AC-07`).
- **Edges**: resolved links between two admitted nodes, `src` not `dst`, one per (src, dst, type), `places` merged, `weak` for `mentions`, by (src, dst, type); `src` the link's source (`spec-cli-graph.md` "Parents and links"), `dst` the node or section it lands on. An end with several holders lands on its first admitted holder by (path, ord).
- **Parents**: each admitted document whose `parent:` resolves (its first admitted holder), by child.
- **Ghosts**: the written ends of admitted files' `dangling` links, one per text (`[[X|label]]` as `X`): `sources` distinct, sorted; `reason` the first place's; `places` counted; by `written`. `skipped`, `unchecked` ends are none.
- **Uses**: `used_by`, `mentioned_by`, a document's `orphan` (a section's `null`): core `usage_of` (`spec-usage.md`), live sources whatever `--archive` says.

JSON `{archive, notes, counts, nodes, edges, parents, ghosts}`, never cut (ends, `of`, `sources` are node indexes): `counts {documents, sections, edges, parents, ghosts, orphans}`; a node `{name, id, kind, title, path, line, of, status, archived, used_by, mentioned_by, orphan}`; an edge `{src, dst, type, weak, places}`; a parent `{child, parent}`; a ghost `{written, reason, sources, places}`. `fixtures/spec-a`: 12 documents, 7 sections, 35 edges (25 `mentions`), 3 parents, 0 ghosts, 1 orphan; `--archive`: 13 documents, DEC-0007 `archived` with its `supersedes` edge, the uses unchanged.

```text
map | nodes <n> (<d> documents, <s> sections), edges <e>, parents <p>, ghosts <g>, orphans <o>
<name> | <kind or -> | used by <u>, mentioned by <m>[ | of <document>][ | orphan] | <title or -> | <path>:<line>[ | archived]
<src> --<type>--> <dst>[ | places <p>]
<child> in <parent>
ghost <written> | from <source>, … | places <p>
[truncated: <k> of <n> lines not shown; the daemon's map is whole]
```

Values through `one_line`; the cut at `OUTPUT_CAP_CHARS` falls at a line, the first whole; `places` prints above 1 on an edge, always on a ghost.

**Notes**, sorted, once: the walk's warnings as `spec tree` words them (a `parent:` cycle, a parent's several holders), the input's, and a name two or more mapped nodes hold, `` `<name>` has <n> holders, each mapped, its links on the first: <path>:<line>, … `` (an ID repeated outside feature documents, or twice in one). Unlike other reads, `notes` carries warnings. stderr, escaped, as `spec catalog`: `note:` lines, then `warning:` lines. Exit 0 answered, no node included; 2 on usage, no project, a config, `HOME` or store error; never 1.

**Route.** `GET /api/projects/:p/map[?archive=true]`: `spec map --json` less its final LF, in the project's turn, trusted as `tree` (`run_read`). Another name, a non-boolean `archive`: a 400 naming it; exit 2: a 503. The 404 list: `… catalog, map, rings, workspaces`.

## Graph flags

`spec graph (REF | --kind K…) [--both | --impact] [--type T]… [--depth N] [--archive]` (CLI `graph.rs`):
- `--both` follows each strong type (or each `--type`) both ways, breadth-first, each node once; `types` lists a type's `out` before its `in`; CLI `walk_both` over `SpecGraph::links`.
- `--kind K` (repeatable, matched whole as `spec tree --kind`) starts at distance 0 from every live node of those kinds (`--archive`: Tier 3 too), by (path, ord); `--depth` counts from the starts. A kind matching nothing: the note `` graph: no node of kind `<k>` `` once; no start: `nodes 0, edges 0`, exit 0.
- Refusals (`GraphRequest::refusal()`), exit 2, on the route a 400 with the line, nothing run: `` spec graph: give a REF or `--kind`, not both ``, `` spec graph: give a REF or `--kind` ``, `` spec graph: `--both` and `--impact` exclude each other ``.
- `GraphView`, 13 keys: `{ref, reason, impact, both, kinds, types, depth, archive, notes, left_out, truncated, nodes, edges}`; `ref` `null` under `--kind`, `kinds` `null` with a REF. The route's names: `ref`, `impact`, `both`, `types`, `kinds` (repeated), `depth`, `archive`; another is a 400 listing them.

## The UI

**Home.** `#/<p>/graph` with no REF: `graph/GraphHome.tsx`, an `h1` and a "View" group (`aria-pressed`), Map (default) and Rings (`spec-rings.md`), each a lazy chunk, kept per project for the page's life (`GraphSettings.home`).

**Seam.** `provisional.ts` `Map{Document,Counts,Node,Edge,Parent,Ghost}`, `GraphView.both`, `.kinds`, `ref` nullable; `getMap(project, {archive}, signal)`; `GraphOptions.both`, `.kinds`; `useMap`, key `["map", p, archive]`, in `READS_OF_THE_SPEC`. Mock `mocks/map.ts`: harbor-sim 30 nodes (2 orphans, 3 ghosts); `large` 5 000 nodes, 15 000 edges.

**The Map** (`ui/src/map/`, lazy `MapView.tsx`):
- `layout.ts`: `MAP_TICKS` 200; `forceLink` (40), `forceManyBody` (`distanceMax` 400), `forceCenter`, `forceX`, `forceY` (0.05); alpha reaches 0.001 at tick 200; slices of at most 16 ms, "Laying out N nodes…", `aria-busy`.
- `filters.ts`: documents, each section's links folded onto its document (merged, a self-edge dropped); Sections draws them apart. `MAP_DRAWN_MAX` 5 000, chosen before layout. Kind chips; Direction, pressing only its kinds' chips; Parents, Mentions, Unresolved targets; Orphans only (no section); Broken only (the ghosts and their sources; none: "No unresolved link"). `DEFAULT_FILTERS` all off, Mentions too (mentions bury a large corpus's links), not persisted. Sections, Include archive (reads again), a new answer lay out again, keeping zoom and pan; a lit node gone is cleared and announced.
- `colours.ts` `kindColours`: the rule's slots; the legend each coloured kind with its count, "Other kinds", the marks.
- `draw.ts`: radius `3 + 2·√used_by`, at most 18 (ghost 3); strong solid `--map-edge`, mentions dashed `--map-edge-weak`, parents dotted; orphans a dashed ring, ghosts hollow `--map-ghost`; labels on the 40 largest and the lit from zoom 0.8, `--map-label` on a `--map-surface` halo; the unlit at 0.15; the base cached offscreen, freed on unmount. `hit.ts`: a grid.
- `MapCanvas`, `MapStage`: the canvas `aria-hidden` in a focusable region described by the count line; a hover is the stage's state alone. Overlay cards, no actions: a node's (title, kind, "Used by N · Mentioned by M", "Orphan", `path:line`); a ghost's ("Unresolved target", `written`, the reason `lang="en"`, "N places", 8 sources then "and N more"). A click opens `doc=<name>`, a drag pans; `touch-action: pan-y pinch-zoom`. Region keys: `+`/`=`, `-` (0.2 to 4), `0` fit, arrows 64 px, Esc, `?`. The fit frames every point: a filter never moves the view.
- Search: dims all but name or title matches; ArrowDown, ArrowUp step, Enter centres and lights, again opens; a polite "N matches", then the lit node. The lit pane (Open document, Show in graph, Show in Spec tree) stays while lit; closed with focus inside, focus goes to the region.
- List (Picture | List), kept nodes in served order: Name (`DocLink`), Kind, Used by, Mentioned by, Links ("out · in"), Marks; 200 a page, "Show 200 more" focusing the first new row; then "Broken links", the kept ghosts. Count line "N documents, E links, O orphans" (+ "Showing the 5 000 most used of N nodes; …"). Notes: 3 (`MAP_NOTES_SHOWN`), `lang="en"`, then "Show all N notes". Skeleton, `ErrorPanel` with Retry, `EmptyState`.

**Local graph.** `GraphSettings.mode` `"out" | "both" | "impact"`, each with its `bases`: Links, Both ways (`both: true`, chips per type and direction), Impact.

**Tokens**, `light-dark()`: `--map-surface`, `--map-kind-1` to `-8`, `--map-kind-other`, `--map-edge`, `--map-edge-weak`, `--map-ghost` (3:1 on the surface), `--map-label` (4.5:1). `map.*`, `graph.home.*`; `.map-*`, `.graph-home-*`. `MapView` chunk 45.18 kB raw, 15.07 kB gzip.

## Known limits

- At most 5 000 nodes drawn; a ninth kind on is grey.
- Positions are one engine's: another browser may lay one answer out apart.
- Cost: the read (release, warm, median of 5) 17 ms at 1k nodes, 105 ms at 10k, 1 253 ms at 100k. A scratch copy of a real game corpus (2 422 nodes, 10 546 edges): the request 0.55 s (1.36 MB), layout and draw of 1 405 documents about 0.6 s, no long task; hover 12-17 ms. 5 000 nodes in 2 s is extrapolated from it: the `large` mock's 2.6 s includes generating it in the browser. Should a measure pass 2 s, the fallback, not built, is `MAP_TICKS` by node count; the bar never rises.
- The map mock counts orphans from its own links, the Health mock from `usageAt`: 2 against 5.
- `walk_both` repeats core `walk`'s loop. The Russian strings are a draft.

## Tests

Core `tests/map.rs`; CLI `tests/map.rs`, `tests/graph.rs`; http `tests/map.rs`, `tests/graph.rs`, `daemon_keys.rs` (sets 155-160, `GraphView` 13), `hub.rs`; eval `map_scale.rs` (`#[ignore]` measures), `ui_policy.rs` (21 packages, `d3-force` in `src/map/` alone); UI `src/map/*.test.ts(x)`, `GraphView.test.tsx`, `settings.test.ts`, `mocks/map.smoke.test.tsx`, `tokens.test.ts`. Criteria: `docs/features/corpus-map.md`.
