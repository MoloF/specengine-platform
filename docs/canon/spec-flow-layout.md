---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-10
---

# The Flow screen: layout and forms

How a flow read is placed and how its edges are drawn; `flow-canvas`, `flow-scenario`, code `ui/src/flow/layout.ts`, `labels.ts`. The screen around it: `spec-flow-screen.md`.

## Layout

`layout.ts` `layoutFlow(view)`, hand-written and pure: a read in; absolute boxes, its lane, stage dividers and edges out, inputs sorted first; one read, one lane. Only the read's nodes are drawn: the drawing reads are cut to the flow (`spec-flow-cuts.md`).

- **Layer.** The box's `distance` in the read drawing it: a folded node carries the least of what it holds. A scope's walk restarts at 0.
- **Rows.** Top to bottom, one row per distinct distance. In a row boxes run left to right, ordered by four barycentre sweeps, then eight passes align each row with its neighbours. A forward edge skipping a row gets a passage slot there, so it passes no box. A box is 160 px wide (`BOX_WIDTH`: four boxes and their gutters fit a 1100 px studio's canvas at the fit floor) and 92 tall, its title alone at 14 px, up to two lines. It is keyed by path and line; a second holder of an ID gets its own id, and links land on the first.
- **Stage dividers.** A full-width divider, a third node type, `aria-hidden` and never focusable, stands above the first row holding a box of a stage; stages beginning in one row share one, their raw words joined ` · `. A picture of fewer than two stages, a stage view among them, draws none; a band box opens none. A box's accessible name adds its stage.
- **Routes.** Rows decide, never a type or `kind`: forward S-curves in the gaps between rows; against the walk an orthogonal arc on its own track right of the walk; same row beside; to itself beside the box; to or from the band. No arc or band edge crosses a box. Ports spread along a side; labels stack.
- **The band.** `distance` null: definitions, unreached steps, a part linked to the flow that no step of this picture reaches, children entered from outside, whom the read's own walk does not reach, so the caption is true to the read. Below the last row, captioned `flow.offWalk` with its count and `flow.offWalkAbout` (at least 320 px, `BAND_CAPTION_WIDTH`, the caption clamped to it), wrapped at the walk's width or four boxes, whichever is wider; edges to one band box share a trunk by (target, type, kind); linked neighbours take a direct line.
- **Off the walk.** An edge with an end whose `distance` is null is drawn only while one of its ends, or the edge itself, is selected (the List, a gap item and the hash select too); Esc's clear hides it again.
- **Stacking and view.** Node types: a box, the band's caption, a divider; stacking is manual, edges under boxes. A picture fits once, when its read lands; then only the owner, `c` and a gap activation move it. The fit takes the width, at most 100 %, at least `FLOW_FIT_MIN_ZOOM` 0.92, so a title renders at 12.9 px or more, centred on a side it fits, top left on one it overflows: a tall picture is panned, never shrunk. A pinch goes down to `FLOW_PINCH_MIN_ZOOM` 0.5; Graph keeps its own 0.6 and 0.1. The canvas fills the studio's centre pane; the wheel pans it, a pinch or the zoom key zooms it, and a selection moves nothing; the minimap, bottom-left, shows only while the picture is not whole in view.

## Forms

`labels.ts` alone tells gap states, codes, rollups and `edge.kind` apart; an unknown rollup is a neutral badge. `FlowEdge` needs no type word: `from` and `to` are in reading direction, and rows and arcs ask no type.

- **Type.** Colour `--flow-edge-0` to `-3` and plain dash go by place in the served `types` (`typesOf(view)`), so a type keeps one look in every picture; a type missing from the list takes the first.
- **Kind.** `FLOW_DASHES`, seven distinct: four by a type's place, of which a read fills three, three by `kind`; `repeat` alone is dotted, in every project, and heavier. The closed kinds `alternative`, `parallel`, `repeat` are SpecEngine's words, shown through `flow.edgeKind.*` on the canvas, the List, the legend, the panel and accessible names; an unknown value shows raw, with its type's dash.
- **Label.** On the canvas, the kind and `when` (clipped at 24 characters), no count; the List keeps `when` whole and `×<count>`, the edge panel the count. `FlowLegend` prints each sample, the type raw, and the arc, "runs against the walk", its about text the top-to-bottom walk and the selection rule.
