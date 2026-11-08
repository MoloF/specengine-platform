---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-11-09
---

# `spec flow`: readings of the walk, the edges and the rows

How `spec flow` (`spec-flow.md`) reads the cases its rules leave open.

## Readings

- **A self-edge** stays an edge while its document is shown and joins `inner` when folded.
- **`start`** means the walk's source, derived starts included, so `no-start 1` and a node marked `start` can stand together; neither it nor `end` stays a shown node's own: both fold.
- **Only an unresolved `follows` keeps its document** a step; an unresolved `uses` or `supplies` does not. A step whose only `follows` is unresolved, with no marked start, is a derived start with distance 0.
- **Several holders of an ID:** a link uses its first admitted holder; under a ROOT, a first holder outside makes it a crossing edge.
- **A left-out source:** a live document receiving `follows` from a left-out document is neither step nor member.
- **Crossing edges.** Under a ROOT, a `follows` edge crossing the scope is an entry or a way out, so a sub-flow is not called broken. It is no edge of that read.
- **`unjoined-parallel`** walks only splits of two or more branches, breadth-first, one step per branch in turn, stopping at the first step all branches reached; a one-branch split is listed with no walk; a split with no join still walks each branch to its end. The walk covers every admitted document, beyond the scope too, so a sub-flow is not called broken.
- **`gaps`.** A row can name a document outside the read, which no node counts; two holders of one ID count as two.
- **`definition-*` and `supply-unfed`** list what is in scope and read links over every live document: domain-free, by links alone.
