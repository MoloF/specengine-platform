---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp]
ref: RM-0060's follow-up, from the orchestrator's check of `spec collisions` on a scratch copy of a real game corpus, 2026-11-12; no plugin version
adrs: []
---

# collision-names: a pair names each side by its canonical reference, never by an ambiguous bare ID

## Why

`collision-pass` named a side by its ID as written. Feature-scoped IDs repeat across documents, so a bundle of a pair's names pulled in every holder (27 nodes for one bare `AC-16` on a real corpus), a mark keyed by a bare name could hide another feature's pair, and two holders of one name never paired. Every name the pass prints or takes is now the node's canonical reference, the form intake stores.

## Data

Shapes unchanged; `a.id`, `b.id`, `shared_targets` and a mark's names carry canonical references. One note per W1 ID, IDs in byte order, after the norm-less note, before unreadable-ledger notes: ``<n> node(s) outside feature documents not paired: their ID `<ID>` is held by more than one node (`spec check` reports it)``.

## Acceptance criteria

Over scratch copies of `fixtures/spec-a`: `stamina-tuning.md` gains a `{#AC-01}` section, a new `endurance-tuning.md` holds one, both `Regen floor holds`. Tests: CLI and MCP `tests/collisions.rs` (`cn1_` to `cn7_`), core units.

- [x] CN1 — One pair, `endurance-tuning/AC-01` and `stamina-tuning/AC-01`; `spec bundle` of the two holds those two nodes.
- [x] CN2 — A `distinct` mark by those names hides that pair alone; a third feature's `AC-01` pairs stay listed.
- [x] CN3 — A bare `AC-01` mark exits 1, ``pairs[0].a: `AC-01` is feature-scoped: cite it as `endurance-tuning/AC-01` or `stamina-tuning/AC-01` ``, ledger and `seq` unchanged; a repeated bare `R-97` too.
- [x] CN4 — Two documents of a new kind, `links: {uses: [stamina-tuning/AC-01]}`, list one pair, `shared_targets` `["stamina-tuning/AC-01"]`.
- [x] CN5 — A row keyed by the bare `AC-01` leaves the pair listed, `marked` null.
- [x] CN6 — `R-01` twice outside feature documents: not paired, one note; notes in order; the mixed case.
- [x] CN7 — MCP `content` and `structuredContent` equal the CLI's for CN1 and CN3; `tools/list` hash, `PINS`, `SCHEMA_PINS` unchanged.

Names byte-equal at another root and `HOME`. Review ACCEPT at iteration 1; iteration 2 changed the note's wording.

## Implementation

Canon: `docs/canon/collision-pass.md`.

| Module | What it does |
|---|---|
| core `collisions.rs` | `CollisionNaming {scheme, features}`, private `Names`, `CollisionList.notes`; `collision_pairs(rows, naming, judged, query)`, `collision_pairs_capped(.., caps)`, `judged_notes(rows, naming, pairs)`; `CollisionNode::name()` removed |
| CLI `collisions.rs` | `naming(&ProjectRoot)` from `[ids]` and `[paths] features` |
| store, plugin | unchanged: no 0.44.1 |

**Naming** through `feature_stem` and `feature_scoped`, as `canonical_id` and the resolver; a project-scoped ID repeated in a feature document is `feat/R-97` (`canonical_id`: bare `R-97`), resolving to one node. **W1**: a non-feature node whose ID another live node holds has no name; mixed, the feature's is `F/R-97` and pairs, the other is left out. Holders: universe nodes of all kinds, whatever `--kind`. A W1 end in `shared_targets` stays bare. Repeats in one feature document share one `slug/ID`, never paired.

**Note** (the orchestrator's ruling, both W1 cases): the "Data" wording; `n` counts kept kinds with a norm.

**Refusals 3 and 4**, the developer's wordings: ``held by more than one node: cite it as `feat/R-97` or `other/R-97` ``; ``held by more than one node outside feature documents, never paired (`spec check` reports it)`` (every holder W1).

**CN4's fixture** needs `links: {uses: …}`: in spec-a a bare `uses:` is no link.

**Real corpus**: `slug/AC-NN` names, a pair's bundle 2 nodes; criteria pairs 927 from 862; 0 `id-taken`.
