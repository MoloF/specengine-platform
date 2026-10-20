---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-10-21
---

# The Roadmap screen

The plan drawn read-only, computing nothing (ADR-0052, ADR-0033); `roadmap-screen`, 2026-10-21; code `ui/src/roadmap/`.

## Route and read

`#/<p>/roadmap[/<ID>]`, nav third of seven. The ID only selects: its card `aria-current` and focused, its box outlined; an ID no group lists: a note and a Tree link. One read, `useRoadmap`: `getRoadmap(p, undefined, signal)`: `spec roadmap --json` uncut (`crates/specengine-http/README.md` "Endpoints"), key `["roadmap", p]`. `RoadmapOptions` `{ release: string; pool?: never } | { pool: true; release?: never }`: the 400 pair does not type.

## Layout

Header: `Plan::summary`'s line, Read again; tabs Lanes, Chain (kept per project).

- **Lanes**, the text twin and keyboard path: jump buttons to lane headings; per group in served order its name, Current, MVP cut, counts, a bar (`aria-hidden`, the sum beside it), cards; the cut rule after the `mvp_cut` release; Pool, then its tasks; Unplaced, `release <as written>`; Notes. A card: position, ID to the Tree, title, `status` and `priority` raw, the estimate with its unit or "unestimated", "Waits on" links, "In a cycle", its tasks' states.
- **Chain**, lazy `@xyflow/react`: a column per group, a box at (column, item index), no layering; an edge per `waits_on` pair of two drawn items, across lanes, none to itself; at most 200 boxes in whole lanes, then `<k> of <n> items not drawn`. A click selects by route, moving no focus. The Graph's read-only props, handles closed (`isConnectableStart`/`End`, `connectOnClick` false); attribution hidden, credited "Drawn with React Flow (MIT)".

## States and live

Loading a skeleton, `aria-busy`. A refusal or the exit-1 document: `ErrorPanel`, the message verbatim, Retry, `NO_PLAN_LINE`. A failed re-read keeps the plan, its line, Retry. No item, or all done: a line above the tabs. Re-read on screen, a read in flight aborted, on `task.*`, `proposal.applied`, a gap; never a stage, `created`, `approved`, `rejected`, `apply_failed` or focus; off screen marked stale.

## Rules

- No write: an item moves by an edit of its file, shown on Read again (3b parked: `roadmap-items.md` N12).
- Marks a label and an icon; no hold word (ADR-0012); no release, priority, unit or item-status literal in app code (`ui_policy.rs`).
- Texts: `ui/src/roadmap/text.ts`, the views; pinned by tests.
- Key sets: the ten, `RoadmapDocument` to `TaskRoadmap`, in `provisional.ts`, last of the 66 in `fixtures/daemon-keys.json`.

## Mock

harbor-sim's plan in its own words (`ui/src/mocks/roadmap.ts`), every shape in `normal`, 500 items in `large`; ledger-api has no `[roadmap]`: its 503, the scenario's under `error`. Plan items are not in the mock corpus: a card's Tree link resolves to nothing.

## Known limits

The join's notes (why a task is missing) are stderr only: `spec roadmap` on a terminal. Hand edits raise no event: Read again. Done items are counted, not listed.
