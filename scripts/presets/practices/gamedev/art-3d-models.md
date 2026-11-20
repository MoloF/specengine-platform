---
id: PRA-ART-MODELS
class: canon
tier: 2
scope: [practices]
owner: art-lead
reviewed: 2026-11-20
disciplines: [art]
---

<!--
An example practice of the gamedev preset (docs/features/department-practices.md).
Copy this file into your project under docs/spec/practices/art/, or propose
it there with `spec propose create`.
Declare its prefix in [ids]: PRA = { kind = "practice", shape = "name" }.
The rules below are examples: replace them with your department's own.
-->

# 3D models: agreed rules

The art department's agreed rules for every 3D model that goes into the game. A task card of the `art` discipline carries them as its practices; cite a rule by its ID when a model keeps or departs from it.

## Degree of realism {#PRA-ART-REALISM}

Stylised, not photoreal: shapes are simplified and read at a glance from the game camera's usual distance. Proportions may be exaggerated where it helps silhouettes; surface detail stays in the textures, not in the geometry.

## Normal maps {#PRA-ART-NORMALS}

Every model with surface detail ships a tangent-space normal map baked from a high-poly source. The bake uses the same tangent basis as the engine; seams sit on hard edges or hidden faces, and a model without a baked map says why in its card.

## Level of detail {#PRA-ART-LOD}

A model larger than a hand prop ships at least three levels of detail, each about half the triangles of the one before. The switch distances are set in the engine and checked from the game camera; the silhouette must not pop at a switch.

## Polygon and texture budgets {#PRA-ART-BUDGETS}

A hero character stays within 30 000 triangles and two 2048 x 2048 texture sets; a hand prop within 2 000 triangles and one 512 x 512 set; a building within 15 000 triangles and one 2048 x 2048 set. A model over its budget names the reason and the lead's agreement in its card.

## Optimisation {#PRA-ART-OPTIMISATION}

Materials are shared wherever two models look alike, textures are packed into atlases where the engine allows, hidden faces are removed, and transforms are applied before export. A model is profiled in the scene it lives in before it is called done.
