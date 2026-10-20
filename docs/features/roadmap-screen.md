---
class: spec
status: draft
scope: [ui]
ref: Phase 6 slice 3a of `roadmap-items.md` (its N9, N12); the requirement-analyst's breakdown of 2026-10-20, decided under the owner's pre-approval, review open; 07 s3; 08 s2
adrs: [ADR-0052]
---

# roadmap-screen (Phase 6, 3a): the Roadmap screen, read-only

A compact draft, refreshed before it starts. 2b (`roadmap-surfaces.md`, RM-0030, shipped 2026-10-20) serves its data: the route `crates/specengine-http/README.md` "Endpoints", the document `docs/canon/spec-roadmap.md` "Command", "Surfaces".

## Why

The owner asked to see the path, the chain, the MVP line and the pool at a glance (2026-09-30: "decorative, additional"; 2026-10-20: needed now). After 2b the daemon serves the plan; the UI draws it, computing nothing (ADR-0033, `docs/canon/architecture.md#ui`).

## Description and interactions

- **Route**: a seventh nav section `roadmap`, `#/<p>/roadmap[/<ID>]` (N9; `ui/src/app/sections.ts`, `routes.ts`' "six" -> "seven", `Shell.tsx`), after Tasks; Health keeps its place.
- **Lanes**: one per release in config order, the current release and the MVP cut marked in text, not by colour alone; then the pool (with 2a's tasks executing no item) and unplaced. Each item a card: ID, title, status, priority, estimate, `waits on`, `cycle`, its open tasks (2a), linking to its Tree node.
- **Chain**: a lazy, read-only `@xyflow/react` canvas of `depends_on` between items, laid out by the Graph screen's hand-written layered layout; a List twin carries the same content as text.
- **Ranges**: a bar per release from the sums (min..max in `unit`), no dates, the unestimated count beside it.
- **Data**: the daemon's `GET /api/projects/:p/roadmap[?release=R | ?pool=true]` (2b: uncut, `omitted` 0; 404 the exit-1 document; 503 without `[roadmap]` or with a bad one) through the one client interface; the join's notes are stderr only, so the body has none: the refresh decides whether the screen needs a key; the mock scenario for "Mock data". It re-reads on `task.*`, `proposal.approved` and `proposal.apply_failed`, one trailing read (as Health since 9a-3).
- **No write**: no drag, no stage, no command to copy for a move (3b, below).

## Acceptance criteria

- [ ] UI-1 -- the roadmap document's types and daemon key sets, left here by 2b (its Q-H): `ui/src/api/provisional.ts` (`RoadmapGroup` the group object beside 2b's `RoadmapGroupKind`), `fixtures/daemon-keys.json`, `daemonKeys.test.ts`' count.
- [ ] UI-2 -- routes: seven sections in order (`routes.test.ts`).
- [ ] UI-3 -- no state by colour alone (`crates/specengine-eval/tests/ui_policy.rs`).
- [ ] UI-4 -- the canvas is read-only (no drag handler, no edge creation) and the List twin carries the same items, edges and marks as text.
- [ ] UI-5 -- live re-reads: `roadmap.live.test.tsx` (each event once, one trailing read).
- [ ] UI-6 -- `mocks/roadmap.smoke.test.tsx`: normal, empty (no `[roadmap]`: the daemon's reason shown), error, large (500 items: lanes virtualized or capped with the count), slow.

spec-writer: 07 s3 a Roadmap row; `ui/README.md` (8 181 of 10 240); a "Screen" section in `spec-roadmap.md`, or `docs/canon/spec-roadmap-screen.md` if it does not fit.

## Files

ui-developer: `ui/src/app/{sections,routes}.ts`, `Shell.tsx`, a `ui/src/roadmap/` screen, `ui/src/api` (the read), `ui/src/mocks/roadmap.*`. Can start on mocks after 1a; lands after 2b.

## Out of scope

Writes of any kind; dates and a timeline; filters beyond `--release`, `--pool`'s; editing items.

## Later

**3b `roadmap-moves`** (parked, no spec file): a move between lanes or a priority change staged from the screen and applied as a `spec: apply PR-…` commit with provenance on the owner's terminal (ADR-0004, ADR-0035). It waits on N12: `spec approve` commits only its own path and hooks run (`docs/canon/proposal-apply.md` "Apply steps"), so under an `enforce` hook an item's apply is refused for the 1b view's (and a create's, the index's) drift; first an ADR letting an apply commit carry SpecEngine-rendered outputs. Until then the owner moves an item by editing its front-matter.

## Implementation

Not started.
