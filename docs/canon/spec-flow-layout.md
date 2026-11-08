---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-09
---

# The Flow screen: layout and forms

How a flow read is placed and how its edges are drawn; `flow-canvas`, code `ui/src/flow/layout.ts`, `labels.ts`. The screen around it: `spec-flow-screen.md`.

## Layout

`layout.ts`, hand-written and pure: reads in; absolute boxes, lanes and edges out, inputs sorted first, a lane per read, nested.

- **Layer.** The box's `distance` in the read drawing it: a folded node carries the least of what it holds. A scoped walk restarts at 0, so each lane stands alone.
- **Grid.** Columns are the lane's distinct distances. A forward edge skipping a column gets a passage slot there, so it passes no box. Four barycentre sweeps order rows; eight isotonic passes pull joins between branches. A box is keyed by path and line; a second holder of an ID gets its own id, and links land on the first.
- **Routes.** Columns decide, never `kind`: forward S-curves in the gaps; against the walk an orthogonal arc on its own track under the lane; same column beside; to itself under the box; to or from the band. No arc or band edge crosses a box. Ports spread along a side; labels stack.
- **Not on the walk.** `distance` null: definitions, unreached steps, a child with no flow link, and children entered from outside, whom the read's own walk does not reach, so the caption is true to the read. The band is a grid with free gutters; edges to one band box share a trunk by (target, type, kind); linked neighbours take a direct line.
- **Stacking and view.** One node type serves all forms; stacking is manual: frame, lane edges, lane boxes. The viewport fits until the owner moves it, then keeps the box in hand in place. The fit takes the width, at most 100 %, no less than readable, centred on a side it fits, top left on one it overflows: a tall picture is panned, never shrunk. The canvas takes the height of the fitted picture and its controls, within the Graph's measure; the controls lie flat; the minimap shows only while the picture is not whole in view.

## Forms

`labels.ts` alone tells gap states, codes, rollups and `edge.kind` apart; an unknown rollup is a neutral badge. `FlowEdge` needs no type word: `from` and `to` are in reading direction, and layers and arcs ask no type.

- **Type.** Colour `--flow-edge-0` to `-3` and plain dash go by place in the sorted types of Top's `edges` and `inner`, which hold every edge, so folding moves no dash.
- **Kind.** `FLOW_DASHES`, seven distinct: four by a type's place, of which a read fills three, three by `kind`; `repeat` alone is dotted, in every project, and heavier. An unknown value shows raw, with its type's dash.
- **Label.** `kind`, `when` (clipped at 24 characters, whole in the List), `x<count>` above 1. `FlowLegend` prints each sample, words raw, and the arc, "runs against the walk".
