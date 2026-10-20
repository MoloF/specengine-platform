---
class: spec
status: draft
scope: [ui]
ref: Phase 6 slice 3a of `roadmap-items.md` (its N9, N12); the requirement-analyst's breakdown of 2026-10-20, decided under the owner's pre-approval, review open; 07 s3; 08 s2
adrs: [ADR-0052]
---

# roadmap-screen (Phase 6, 3a): the Roadmap screen, read-only

A compact draft, refreshed against slice 2 as built before it starts.

## Why

The owner asked to see the path, the chain, the MVP line and the pool at a glance (2026-09-30: "decorative, additional"; 2026-10-20: needed now). After slice 2 the daemon serves the plan; the UI draws it, computing nothing (ADR-0033, `docs/canon/architecture.md#ui`).

## Description and interactions

- **Route**: a seventh nav section `roadmap`, `#/<p>/roadmap[/<ID>]` (N9; `ui/src/app/sections.ts`, `routes.ts`' "six" -> "seven", `Shell.tsx`), after Tasks; Health keeps its place.
- **Lanes**: one per release in config order, the current release and the MVP cut marked in text, not by colour alone; then the pool (with slice 2's tasks executing no item) and unplaced. Each item a card: ID, title, status, priority, estimate, `waits on`, `cycle`, its task (slice 2), linking to its Tree node.
- **Chain**: a lazy, read-only `@xyflow/react` canvas of `depends_on` between items, laid out by the Graph screen's hand-written layered layout; a List twin carries the same content as text.
- **Ranges**: a bar per release from the sums (min..max in `unit`), no dates, the unestimated count beside it.
- **Data**: the daemon's `GET /api/projects/:p/roadmap` (slice 2) through the one client interface; the mock scenario for "Mock data". It re-reads on `task.*`, `proposal.approved` and `proposal.apply_failed`, one trailing read (as Health since 9a-3).
- **No write**: no drag, no stage, no command to copy for a move (3b, below).

## Acceptance criteria

- [ ] UI-1 -- generated types for the roadmap document; `daemonKeys.test.ts`' count follows slice 2's keys.
- [ ] UI-2 -- routes: seven sections in order (`routes.test.ts`).
- [ ] UI-3 -- no state by colour alone (`crates/specengine-eval/tests/ui_policy.rs`).
- [ ] UI-4 -- the canvas is read-only (no drag handler, no edge creation) and the List twin carries the same items, edges and marks as text.
- [ ] UI-5 -- live re-reads: `roadmap.live.test.tsx` (each event once, one trailing read).
- [ ] UI-6 -- `mocks/roadmap.smoke.test.tsx`: normal, empty (no `[roadmap]`: the daemon's reason shown), error, large (500 items: lanes virtualized or capped with the count), slow.

spec-writer: 07 s3 a Roadmap row; `ui/README.md` (8 181 of 10 240); a "Screen" section in `spec-roadmap.md`, or `docs/canon/spec-roadmap-screen.md` if it does not fit.

## Files

ui-developer: `ui/src/app/{sections,routes}.ts`, `Shell.tsx`, a `ui/src/roadmap/` screen, `ui/src/api` (the read), `ui/src/mocks/roadmap.*`. Can start on mocks after 1a; lands after slice 2.

## Out of scope

Writes of any kind; dates and a timeline; filters beyond `--release`, `--pool`'s; editing items.

## Later

**3b `roadmap-moves`** (parked, no spec file): a move between lanes or a priority change staged from the screen and applied as a `spec: apply PR-…` commit with provenance on the owner's terminal (ADR-0004, ADR-0035). It waits on N12: `spec approve` commits only its own path and hooks run (`docs/canon/proposal-apply.md` "Apply steps"), so under an `enforce` hook an item's apply is refused for the 1b view's (and a create's, the index's) drift; first an ADR letting an apply commit carry SpecEngine-rendered outputs. Until then the owner moves an item by editing its front-matter.

## Implementation

Not started.
