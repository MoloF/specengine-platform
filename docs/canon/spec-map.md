---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# The corpus map: every document and its links in one picture

ADR-0080 says how the whole-corpus map is laid out and drawn. Related pages:
- the uses it sizes nodes by: `usage-orphans` (RM-0111), canon at its ship;
- one node's walk: `spec-cli-graph.md` "spec graph";
- the other canvases: `ui/README.md` "Screen rules", `spec-flow-layout.md` "Layout".

## State

The rule below holds, but none of it is built yet. `corpus-map` (RM-0112, `docs/features/corpus-map.md`) builds four things:
- `spec map` and the daemon's `GET /api/projects/:p/map`;
- `spec graph REF --both` and `spec graph --kind K`;
- the Graph section's Map, at `#/<p>/graph` with no REF;
- the local graph's Both ways mode.

Until it ships, the Graph section needs a REF and shows one node's walk.

<a id="the-rule"></a>
## The rule

The map draws the whole live corpus as one picture: each document, and on request each ID section, with its resolved links. The daemon serves the nodes, the edges, each node's use counts and orphan flag, and the unresolved targets. The UI lays them out and draws them. It never counts a use, and it reads, stages or writes nothing.

- **Layout.** `d3-force` at an exact pin, imported only in `ui/src/map/`. It runs a fixed number of ticks (`MAP_TICKS`) from its seeded start: d3's phyllotaxis placement and its fixed-seed random source, never `Math.random`. Nodes go in in served order, so one answer gives one picture. Ticks run in slices between frames, so a progress line paints, and the result equals one uninterrupted run. Filters hide or dim nodes and never lay out again; only a change of the node set does, such as showing sections.
- **Drawing.** One 2D `<canvas>`, sized to its box at the device's pixel ratio. Colours come from `tokens.css` roles, read through a hidden swatch per role and read again when the theme changes. A node's size follows its use count. Its colour is the slot of the first direction (in served order) listing its kind, slots 1-8, and grey for any other kind. A colour is always paired with words: the legend, the hover card and the List.
- **The canvas rules** (`ui/README.md` "Screen rules"):
  - the map is read-only;
  - the wheel scrolls the page and is never taken;
  - a press moving at most `CLICK_DISTANCE` (5 px) is a click, which opens the document view; past it the press pans;
  - zoom is by key and by button, with a fit button;
  - the picture is `aria-hidden`; a search field and a List twin reach every node by keyboard;
  - the chunk is lazy, never in first load.
- **The cap.** At most 5 000 nodes are drawn: those used most, ties in served order. A line says how many are hidden; the List lists all.
