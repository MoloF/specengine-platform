---
class: canon
tier: 2
scope: [ui]
owner: owner
reviewed: 2026-11-12
---

# The Roadmap screen

The plan drawn read-only, computing nothing (ADR-0052, ADR-0033); `roadmap-screen`, 2026-10-21; code `ui/src/roadmap/`.

## Route and read

`#/<p>/roadmap[/<ID>]`. The ID only selects: its card `aria-current` and focused, its box outlined; an ID no group lists: a note and a Tree link. One read, `useRoadmap`: `getRoadmap(p, undefined, signal)`: `spec roadmap --json` uncut (`crates/specengine-http/README.md` "Endpoints"), key `["roadmap", p]`. `RoadmapOptions` `{ release: string; pool?: never } | { pool: true; release?: never }`: the 400 pair does not type.

## Layout

Header: `Plan::summary`'s line, Read again; tabs Lanes, Chain and Delivery ("Delivery"), kept per project.

- **Lanes**, the text twin and keyboard path: jump buttons to lane headings; per group in served order its name, Current, MVP cut, counts, a bar (`aria-hidden`, the sum beside it), cards; the cut rule after the `mvp_cut` release; Pool, then its tasks; Unplaced, `release <as written>`; Notes. A card: position, ID a document link (`doc-view.md`), title, `status` raw in a neutral Lozenge (an sr-only "Status: " name, no `lang`: plan content), `priority` raw, the estimate with its unit or "unestimated", "Waits on" links, "In a cycle", its tasks' states.
- **Chain**, lazy `@xyflow/react`: a column per group, a box at (column, item index), no layering; an edge per `waits_on` pair of two drawn items, across lanes, none to itself; at most 200 boxes in whole lanes, then `<k> of <n> items not drawn`. A box's tasks are one line: its first `CHAIN_TASKS_SHOWN` (2, `chain.ts`) cells, then "<n> more"; the twin test (`RoadmapChain.test.tsx`) matches them to each card's first two, and cells plus count to its total. A click, a press moving up to `CLICK_DISTANCE` 5 px (`spec-flow-layout.md` "Layout", Clicks), selects by route, moving no focus; a longer one pans. The Graph's read-only props, handles closed (`isConnectableStart`/`End`, `connectOnClick` false); attribution hidden, credited "Drawn with React Flow (MIT)".

## States and live

Loading a skeleton, `aria-busy`. A refusal or the exit-1 document: `ErrorPanel`, the message verbatim, Retry, `NO_PLAN_LINE`. A failed re-read keeps the plan, its line, Retry. No item: an `EmptyState` (`h2`, `roadmap.emptyNext`) above the tabs; all done: a line. Re-read on screen, a read in flight aborted, on `task.*`, `proposal.applied`, a gap; never a stage, `created`, `approved`, `rejected`, `apply_failed` or focus; off screen marked stale.

## Delivery

`spec delivery` drawn, its rule and document `spec-delivery.md`; RM-0107 a, 2026-11-12. The third tab (`RoadmapTab` `"delivery"`), a lazy chunk (`RoadmapDelivery` in a `Suspense` with a skeleton, under a `RegionBoundary`), shown once the plan reads. `useDelivery(p, asOf)`, key `["delivery", p, asOf]`: `getDelivery(p, {asOf}, signal)`, `?as_of=`, a judged read, only a 200 resolving. Re-read, every day cached (`deliveryOf(p)`), on every `card.*` event, every `proposal.*` but a stage, and a gap, a read in flight aborted.

- **As of**: a date field starting at the viewer's day (`useToday()`, as the Board's overdue badge), always sent; a change re-reads, showing the day asked and a skeleton, no placeholder data; "Today" returns to it.
- **Two tables**, captioned, each in a focusable scrolling region, a row header per row: By release (the name, Pool, Unplaced, No release; an unknown group raw, `lang="en"`) and By department (titles in `bdi`; a footer row All tasks, the served `total`); columns Delivered (N of M), Open, Overdue, Delivered late, Cancelled, Lead time (`median N, max M days`, no plural forms; else None delivered).
- **Late tasks**, an `ol` in served order: the ID linking the Board, the title, a lozenge "Overdue by N days" or "Delivered N days late", a `PropertyList` labelled by the ID (Due, Created, Taken, Accepted, Estimate as written, Departments by title), then each return, re-estimate (a clear its own line), cancelled subtask (its ID linking the Board) and item (linking the Inbox, its kind a word, its status raw, `lang="en"`), a "more" line per cut list; none, "No task is late."
- **States**: a skeleton; an `ErrorPanel` with the daemon's words and Retry; an `EmptyState` (level 3) when `total.tasks` and `total.cancelled` are 0; `notes` a list, `lang="en"`.
- The UI computes no count. Strings: 47 `roadmap.delivery.*` leaves and `roadmap.boundary.delivery`, none saying "blocked"; CSS `.delivery-*`, scale tokens only.
- Tests: `RoadmapDelivery.test.tsx`, `src/mocks/delivery.test.ts`, `delivery.smoke.test.tsx`, `src/api/delivery.live.test.tsx`, `daemonKeys.test.ts`, `lazy.test.ts`, `policy.test.ts`.

## Rules

- No write: an item moves by an edit of its file, shown on Read again (3b, RM-0019, unparked by ADR-0062 on 2026-11-03, not built).
- Marks a label and an icon; no hold word (ADR-0012); no release, priority, unit or item-status literal in app code (`ui_policy.rs`); a delivery group word or item kind compared only in `src/roadmap/delivery.ts` (`policy.test.ts`).
- Texts: `ui/src/roadmap/text.ts`, the views; pinned by tests.
- Key sets in `provisional.ts`: the ten, `RoadmapDocument` to `TaskRoadmap`, sets 57 to 66 of `fixtures/daemon-keys.json`; Delivery's eleven, 127 to 137.

## Mock

harbor-sim's plan in its own words (`ui/src/mocks/roadmap.ts`), every shape in `normal`, 500 items in `large`; ledger-api has no `[roadmap]`: its 503, the scenario's under `error`. Plan items are not in the mock corpus: a card's document link finds nothing.

Delivery's, `ui/src/mocks/delivery.ts`: stored task facts judged at the day asked, as `roadmap.ts` sums; the scenario of `docs/features/delivery-analytics.md` "Data" at 2026-11-20 byte-equal to its pinned JSON; harbor-sim every shape (each group, an unknown `milestone`, no release holding only a cancelled task, Chandlery a department of zeros, a task with every reason and 17 returns, a cleared estimate, tasks falling overdue a day or two on); `empty` no task; ledger-api the no-`[roadmap]` note; a bad day a 400. Its third department and `milestone` are its own (`workspaces.ts` serves two), its dates not the Board mock's.

## Known limits

The join's notes (why a task is missing) are stderr only: `spec roadmap` on a terminal. Hand edits raise no event: Read again. Done items are counted, not listed. On the Chain the MVP cut's label can overlap the next lane's title (harbor-sim's `hull`, 1920x1080, dark, Russian, seen 2026-11-12; likely older than `design-sweep-b`, not fixed).
