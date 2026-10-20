---
class: spec
status: shipped
scope: [ui, crates/specengine-http, crates/specengine-cli, crates/specengine-eval, fixtures]
ref: Phase 6 slice 3a of `roadmap-items.md` (its N9, N12), plan item RM-0004; the requirement-analyst's breakdown of 2026-10-20 refreshed against 1a, 1b, 2a and 2b as built (87b8ae2), Q-1..Q-9 decided under the owner's pre-approval, review open; 07 s3; 08 s2; ui-developer iteration 1 accepted by the review (two minors, nits); iteration 2 (decided under the same pre-approval) accepted by the re-review
shipped: 2026-10-21
adrs: [ADR-0052]
---

# roadmap-screen (Phase 6, 3a): the Roadmap screen, read-only

UI-only over 2b's uncut route (`roadmap-surfaces.md`); no rust-developer, no ADR.

## Why

The owner asked to see the path, the chain, the MVP line and the pool at a glance (2026-09-30: "decorative, additional"; 2026-10-20: needed now). After 2b the daemon serves the whole plan; the UI draws it and computes nothing (ADR-0033, `docs/canon/architecture.md#ui`). It applies ADR-0052 (`#storage`); ADR-0011, ADR-0014, ADR-0033, ADR-0036 (`#ui`); ADR-0012, ADR-0039 (`#control`: an item "waits on", never a hold word); ADR-0035 (no write); ADR-0008, ADR-0031 (`#universal`: release, priority, unit and item status are project vocabulary).

Delivered: the seventh nav section `#/<p>/roadmap[/<ID>]`, third after Inbox and Tasks; one uncut read of the plan; Lanes (release lanes with Current, the MVP cut, range bars; Pool with its tasks; Unplaced; Notes), the text twin of a lazy read-only Chain canvas; live re-reads; the roadmap document's ten daemon key sets (56 -> 66); harbor-sim's mock plans. How it works now: `docs/canon/spec-roadmap-screen.md`; the route `crates/specengine-http/README.md` "Endpoints"; the document `docs/canon/spec-roadmap.md` "Command".

## Decisions

2026-10-20, the owner's pre-approval, review open: one read of the whole plan; Lanes the canvas's text twin; done items not shown (not served); no refetch on window focus; the mock's vocabulary not this repository's.

| | Answer |
|---|---|
| Q-1 | no JSON key for the join's notes (stderr only): a known limit |
| Q-2 | re-read on `task.*`, `proposal.applied` and a gap; reads abortable |
| Q-3 | the nav place: after Tasks |
| Q-4 | canvas placed by the served order (lane, item index), no layering |
| Q-5 | canvas: click only; Lanes is the keyboard path |
| Q-6 | the mock vocabulary scanned out of app code (TE-2) |
| Q-7 | a "Read again" button |
| Q-8 | two harbor-sim tasks execute mock items, one `in_progress` |
| Q-9 | the canon at shipping: `docs/canon/spec-roadmap-screen.md` |
| review | `RoadmapOptions` takes the `never` form; iteration 2: the Chain's text credit, `RoadmapLanes` memoised |

## Acceptance criteria

M marks the mutation that must turn the named test red (M1-M6).

ui-developer:

- [x] UI-1 -- `provisional.ts` the ten types in record order. `ui/src/api/daemonKeys.test.ts`: a `ROADMAP_SCREEN` group after `HEALTH_CODE`, counts `[15, 2, 4, 3, 8, 11, 2, 2, 4, 7]`; `ALL` 66; the "Roadmap" name pin the ten; the order pins the ten last; `AHEAD_OF_FIXTURE` empty (TE-1's fixture names them), its `toEqual([])` pin kept. M1: `cycle` dropped from `RoadmapItem` (type and record) -> "%s: the daemon's keys are the type's" fails.
- [x] UI-2 -- seven sections. `ui/src/app/routes.test.ts` (parse; `sectionHash` for `roadmap`, bare and with RM-0004), the nav pins (`routes.test.ts`, `App.test.tsx`, `ui/src/overview/HomeView.test.tsx`, `ui/src/palette/Palette.test.tsx`, `options.test.ts`), `App.test.tsx`: `#/alpha/roadmap` shows the h1 "Roadmap". M2: the Shell's roadmap branch removed -> `App.test.tsx` "builds the Roadmap" fails.
- [x] UI-3 -- `ui/src/roadmap/RoadmapView.test.tsx`: text plus an icon for Current, MVP cut, In a cycle, Waits on, unestimated, every task state, an unknown one raw; links (ID to the Tree, "Waits on" to the roadmap, a task to Tasks); selection by route (`aria-current`, focus); the unlisted-ID note; the header line, the cut rule, `release not a string`. M3: the "MVP cut" text removed, its class kept -> fails.
- [x] UI-4 -- `ui/src/roadmap/RoadmapChain.test.tsx`: read-only as `ui/src/graph/GraphView.test.tsx` checks the Graph (no `.draggable` or `.selectable` node, no connectable handle, only the shell's key listener, no delete, move, drag or connect wording); canvas and Lanes carry the same items, `waits_on` pairs (cross-lane too), cycle marks and task cells; no self-edge; past 200 boxes whole lanes and the count. M4: cross-lane edges dropped -> the parity test fails.
- [x] UI-5 -- `ui/src/api/roadmap.live.test.tsx`: entering reads once with a signal; on screen `task.*` (an ID and none), `proposal.applied`, a gap each read once more; a burst during a read aborts it, one answered read; `proposal.created`, `.approved`, `.rejected`, `.apply_failed`, `.staged`, `.unstaged` read nothing; off screen nothing, entering reads once. M5: roadmap removed from the `task.*` branch -> fails.
- [x] UI-6 -- `ui/src/mocks/roadmap.smoke.test.tsx`: normal, ledger-api's 503 (message verbatim, Retry, the fixed line), empty, error, large (500 cards, `344 of 500 items not drawn`), slow. `ui/src/mocks/roadmap.test.ts`: in every scenario each open mock task once, on an item or in `pool_tasks`, its package agreeing. `ui/src/api/http.test.ts`: the three queries, the 404 document, 400 and 503 verbatim, status 0, the signal. `ui/src/app/liveScreens.test.tsx` "Roadmap over the daemon"; `ui/src/mocks/tasks.test.ts` the two item tasks. M6: the 503's message replaced by fixed text -> the smoke test fails. `pnpm lint`, `pnpm test`, `pnpm build` clean.
- [x] UI-7 (iteration 2) -- the Chain panel shows "Drawn with React Flow (MIT)" as text (`ui/README.md` "Screen rules", Canvas), asserted in `RoadmapChain.test.tsx` (red without the line); `RoadmapLanes` wrapped in `memo`.

test-engineer:

- [x] TE-1 -- `crates/specengine-http/tests/daemon_keys.rs`: a git copy of `fixtures/roadmap-a` served as `tide-runner`, one open task on an item, one pool task; the route plain (200) and `?release=nope` (the 404 document), the item task's `tasks/:id`; the ten sets after `UnboundMarker`, `ac09` asserting 66 and their counts; the other 56 pins (`daemon_keys.rs`, CLI `kind_amendment.rs`) 66; `fixtures/daemon-keys.json` regenerated (`SPECENGINE_WRITE_DAEMON_KEYS=1`), no other set changed.
- [x] TE-2 -- `crates/specengine-eval/tests/ui_policy.rs`: `MOCK_PLAN`, the mock plan's seven words (releases `keel`, `hull`, `sails`; priorities `gale`, `breeze`, `calm`; unit `tides`; the cut `keel`), each quoted in a non-test file under `ui/src/mocks/`, distinct, disjoint from this repository's `[roadmap]` words (read from `specengine.toml`) and from `MOCK_KINDS`; none of them, nor of this repository's, quoted in `ui/src` outside the mocks and tests; a positive control (a scratch `ui/` quoting a mock release in app code fails); the hold-word scan green; its own mutation red.
- [x] TE-3 -- M1-M6 killed, each by its test (M6 in two forms: the screen's text and the mock's message); `ui_policy`, `anonymity`, `doc_pointers` and the touched nextest files, then one full run.

spec-writer (bytes by `wc -c`, before -> after / cap):

- [x] SW-0 -- the refresh (2026-10-20).
- [x] SW-1 -- `docs/canon/spec-roadmap-screen.md` new, 2 999 / 12 288 (planned at most 3 000); the index regenerated.
- [x] SW-2 -- 07 s3 the Roadmap row (+253); `ui/README.md` 8 181 -> 8 177 / 8 192 (its own budget: `ui-shell.md` AC-20, `ui_policy.rs`; the mock plan's pointer, a redundant clause cut); `docs/canon/spec-roadmap.md` 12 243 -> 12 281 / 12 288 (a pointer).
- [x] SW-3 -- HTTP README 10 232 "56" -> "66", byte-neutral, as `docs/canon/health-code.md` and `health.md`; RM-0004 shipped, the view regenerated; 08 s2 Phase 6; `CLAUDE.md` State 6 311 -> 6 329 / 16 384; this spec compacted.

## Owner's manual check

This repository's own plan: install `spec`, `specengine-mcp`, `specengine-http` from one commit, `pnpm --dir ui build`, `spec serve`, open the printed address, `#/specengine-platform/roadmap`.

1. The header line equals `spec roadmap`'s first line less `roadmap: `; lanes `mvp`, `v1`, `v2`, then Pool, Unplaced, as `spec roadmap` prints them; Current on its release; MVP cut on `mvp` and the rule after it.
2. A card's ID opens its Tree node; a "Waits on" link selects and focuses that card; Chain draws an edge wherever `spec roadmap` prints `waits on` between two items, and credits React Flow as text.
3. The route with the ID RM-0005 selects and focuses its card; with RM-0004 (shipped) it shows the unlisted-ID note and the Tree link.
4. Edit an item's `priority` by hand: nothing moves until Read again, then the order equals `spec roadmap`.
5. A project without `[roadmap]` served too: its Roadmap shows the 503 message, Retry and the fixed line.
6. `pnpm --dir ui dev`, `?scenario=normal#/harbor-sim/roadmap`, then `empty`, `large`, `error`, `slow`; `#/ledger-api/roadmap`. A card's Tree link resolves to nothing there: the mock corpus holds no plan item.
7. Keyboard alone through Lanes; VoiceOver on a card and a lane header; 200 % zoom: cards wrap.

## Out of scope

Any write; 3b `roadmap-moves` (parked, no spec file: a move staged from the screen waits on N12; until then the owner edits an item's front-matter); dates and a timeline; filters beyond the daemon's `release` and `pool`; done items; a JSON key for the join's notes; keyboard on the canvas; a light theme.

## Implementation

| Module (`ui/src/`) | What it does |
|---|---|
| `roadmap/RoadmapView.tsx` | the screen: `useRoadmap`, the header line, Read again, the failure, re-read, empty and unlisted lines, tabs (memory from the Shell), the selected card's focus |
| `roadmap/RoadmapLanes.tsx` | jump buttons, lanes, `RangeBar`, `ItemCard`, the cut rule, `PoolTasks`, notes; memoised (iteration 2) |
| `roadmap/RoadmapChain.tsx`, `chain.ts` | the lazy canvas; `chainOf`: boxes at (column, index), wait edges, the 200 cap by whole lanes; read-only props, `CHAIN_ARIA_LABELS`, the text credit (iteration 2) |
| `roadmap/text.ts` | `headerLine`, `sumText`, `estimateText`, the marks' `Look`s, `NO_PLAN_LINE`, the fixed lines |
| `api/client.ts`, `http.ts`, `queries.ts` | `RoadmapOptions`, `getRoadmap`; `queryKeys.roadmap`, `useRoadmap`; `roadmap` in `READS_OF_THE_SPEC`, `readPlanAgain` after `task.*` |
| `api/provisional.ts` | the ten types |
| `app/sections.ts`, `routes.ts`, `Shell.tsx`, `shortcuts.tsx` | the section, third of seven; the Shell's branch and tab memory; the shortcuts intro |
| `mocks/roadmap.ts`, `MockClient.ts`, `tasks.ts` | harbor-sim's plans (`normal`, `empty`, `large`), `roadmapOf`, `taskPlaceOf`; ledger-api's 503; T-0109, T-0112 on PLAN-0006, PLAN-0012 |
| `ui/Icon.tsx`, `styles/tokens.css`, `app.css` | icons `current`, `cutLine`; `--roadmap-*` tokens (3:1); the layout |
| `test/builders.ts`, `stubClient.ts`, `roadmapStub.ts` | test plans and clients |

No new dependency; React Flow shares one lazy chunk with the Graph. Deviations, recorded under the owner's pre-approval, review open: `RoadmapOptions` is `{ release: string; pool?: never } | { pool: true; release?: never }` (the plain union lets the 400 pair through TypeScript); fixed texts added: "Lanes of the plan" (jumps), "Views of the plan" (tab list), "Pool tasks", "Reading the plan of <p>", "Loading the canvas", "Retry"/"Retrying" on the re-read line, "Selected" on the chosen box, "State: " before a task cell's state, `CHAIN_ARIA_LABELS`, the view's about plus " Read only."; a card's estimate carries the unit ("2-4 days"; the CLI's item line none); the exit-1 document shows in the same ErrorPanel with the fixed line; handles also `isConnectableStart`/`End` false, the canvas `connectOnClick` false; mock items are not in harbor-sim's corpus (a card's Tree link, T-0109's and T-0112's targets resolve to nothing); under `error` ledger-api answers the scenario's 503, `spec: specengine.toml has no [roadmap]` in every other; after a Chain click focus is not moved (Lanes is the keyboard path).

Tests: UI `pnpm lint`, `pnpm build` clean, `pnpm test` 90 files, 2 146 passed; nextest `daemon_keys` 3, `kind_amendment` 24, `ui_policy` 28, the full run 3 600, all passed; clippy `--all-targets -D warnings`, fmt clean. Mutations 6 of 6 killed by the criteria's tests, re-verified by the test-engineer; `fixtures/daemon-keys.json` regenerated, only the ten sets added.
