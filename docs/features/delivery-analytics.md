---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0107 part a, v5 core slice 6 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 3, DA-1 to DA-7); the orchestrator's working answers, the owner's review open
adrs: []
---

# delivery-analytics: what was delivered, what is overdue and why, per release and per department (RM-0107 a)

## Why

The owner's vision of 2026-11-11 asks for a roadmap of delivered features: where we are, how much is left, what took longer and why (`08-roadmap.md` Phase 7). The Roadmap screen shows lanes of plan items but no dates. The queue holds every fact (a card's `created_at`, `due`, `estimate`, `acceptance.at`, `cancellation`, and its `card.moved` and `card.reestimated` events with their notes and reasons, `docs/canon/cards.md` "Store", "Documents and events"), yet nothing totals them, and a card read stops at its last 64 events.

Now `spec delivery [--as-of D] [--json]`, the daemon's `GET …/delivery` and a lazy Delivery tab on the Roadmap screen show, per release and per department, how many tasks were delivered out of how many, how many are open, overdue or delivered late, and the lead time; then every late task with its reasons: returns with their notes, re-estimates old to new with their reasons, cancelled subtasks with theirs, and the questions raised on it. Computed per read from the queue, the config and the plan, never stored, and never written into `docs/generated/roadmap.md`, which is built from git alone (ADR-0052). It moves, refuses and gates nothing (ADR-0012). No ADR: it rests on ADR-0052, ADR-0056, ADR-0074, ADR-0076 and ADR-0012. Part b, the timeline, is `delivery-timeline`, core slice 13.

## Description and interactions

Each decision is canon now, in `docs/canon/spec-delivery.md`: D1 a card's release, D2 departments, D3 taken and returned, D4 overdue and late, D6 estimates as written: "The rule"; D5 `as_of`: "The rule", "Reads", "Known limits"; D7 one uncapped read: "Reads"; D8 no MCP tool or plugin bump: "Daemon, hub". The tab: `docs/canon/spec-roadmap-screen.md` "Delivery".

The owner's questions, open, their working answers built: **Q1** a card's release from the first plan item it or an ancestor targets, no card field; **Q2** no estimate unit; **Q3** no configured release dates; **Q4** no MCP tool; **Q5** a flow two departments list counts in both, the total once; **Q6** taken the first move out of the first column, reorders aside; late accepted after `due`, or open past it at `as_of`. The writer's W1 to W5 (every card counts, a subtask too; `tasks` without the cancelled; the viewer's day in the UI; release rows only for groups holding a card, every department row; keys under `roadmap.delivery`) are in that canon too.

## Data

In the canon: `spec-delivery.md` "The judge", "Reads", "Text and JSON", "Notes and exits", "Daemon, hub"; the scenario's pinned document and text: `crates/specengine-cli/tests/delivery_scene/data.rs`. **The scenario**, the tests' corpus and part b's, `as_of` 2026-11-20. Config: flows `copy` (columns `todo`, `doing`, `review`, `returned`, `done`; accept `review` to `done`; return `returned`), `plates` (`todo`, `drawing`, `done`), `sound`; departments `words` "Words" (`copy`) and `print` "Print" (`copy`, `plates`). Plan items `RM-0001` (`release: r1`) and `RM-0002` (`release: r2`), `[roadmap] releases = ["r1", "r2"]`. Cards, every time `T09:00:00Z`:
- C-0001 "Write the tide table foreword", `copy`, targets `RM-0001`, assignee `Ann`, reviewer `Bob`, made 11-01, due 11-10, estimate 3; two reorders, then moved to `doing` 11-03; returned from `review` into `returned` 11-06 "Too long" and 11-08 "Wrong tone"; re-estimated 3 to 5 on 11-07, "The tables doubled"; accepted 11-15. PR-0040, a question with `card_id` C-0001, applied.
- C-0002 "Draw the tide plates", `plates`, its parent C-0001, no target, cancelled 11-09, "Plates reused from last year".
- C-0003 "Engrave the harbor map", `plates`, targets `RM-0002`, made 11-04, due 11-12, moved to `drawing` 11-05, open.
- C-0004 "Fix the index", `copy`, no target, made 11-02, accepted 11-05.
- C-0005 "Record the foghorn", `sound`, no target, open.

## Rules and edge cases

In the canon: `spec-delivery.md` "The rule", "Known limits". **Rollout**: no schema step, backup format, config key or tool; a daemon or hub before this slice answers the route 404; a hub takes it at its next upgrade, the owner's call.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red. Rows and events are written through the store with fixed `now`s, as its tests do; corpora are built at run time with invented names; "the scenario" is "Data"'s.

- [x] DA-1 — **Uncapped.** C-0001 reordered 70 times in `todo`, then moved to `doing`: `taken_at` is that move's. Mutation, red: the events read through `card_history(…, 64)`.
- [x] DA-2 — **A reorder is no take.** C-0001's two reorders before 11-03: `taken_at` 2026-11-03T09:00:00Z; a card accepted straight from its first column: the accept's `at`; a card only reordered: `null`. Mutation, red: the first `card.moved` of any kind.
- [x] DA-3 — **Release.** C-0002 (no target) counts under r1 through its parent; a card targeting `RM-0002` and `RM-0001` is r1; one targeting a done item of r2 is r2; a pool item `pool`; a release not listed `unplaced`; no item up the chain `none`. Mutations, each red: the parent fallback dropped; the last target's group; done items ignored.
- [x] DA-4 — **Departments.** On the scenario `words` counts C-0001 and C-0004, `print` C-0001 to C-0004, `total` `tasks` 4 counts C-0001 once, and C-0005 (`sound`) only in `total`; with no `[[workspace]]`, one row `all` equal to `total`. Mutation, red: `total` as the sum of the departments.
- [x] DA-5 — **`as_of`, no clock.** C-0003 (due 11-12) is overdue at 2026-11-20 (`days_late` 8), not at 2026-11-01 nor at 2026-11-12; C-0001 is delivered late by 5 whatever `as_of`. The same queue with `--as-of 2026-11-20` is byte-identical twice and at another root and `HOME`. Without `--as-of`, `as_of` equals `today_utc()` read before or after the run. Mutations, each red: the clock read when `--as-of` is given; `<=` for `<`.
- [x] DA-6 — **No sum, no person.** A walk of the scenario's JSON finds no key holding `estimate` but a late row's `estimate`, and none named `as`, `by`, `assignee`, `reviewer`, `created_by` or `decided_by`; no line of the text holds `Ann` or `Bob`. Mutations, each red: an `estimate_sum` key; `assignee` on a late row.
- [x] DA-7 — **Lazy.** After `pnpm build`, the Delivery chunk is not among first load's files; first-load JS grows at most 3 kB, ≤ 845 kB. Mutation, red: a static import of `RoadmapDelivery` in `RoadmapView.tsx` (`lazy.test.ts`).
- [x] DA-8 — **Reasons.** On the scenario C-0001 lists the two returns with their notes and `from`, the re-estimate 3 to 5 with its reason, C-0002 cancelled with its reason, PR-0040; a reorder within `returned` is no return. Twenty returns list the oldest 16, `more.returns` 4, the text ending `  more returns 4`. Mutations, each red: any move with a note taken as a return; the lists uncapped.
- [x] DA-9 — **Text and JSON as pinned.** `spec delivery --as-of 2026-11-20` prints "Text"'s lines byte for byte, `--json` "JSON"'s document; lead days median over 3 and 14 is 3. No `[roadmap]`: every row `none`, the note, exit 0. A card row made unreadable in SQL: its note, not counted, exit 0. A bad `--as-of`: exit 2, nothing read. Mutation, red: the upper middle as the median.
- [x] DA-10 — **[measure] One read per call.** A test seam counts `list_existing_cards`, `events_of_types` and `list_readable` calls and plan builds: 1 each over the scenario and over 200 cards. Seeded through the store (`crates/specengine-eval/tests/delivery_scale.rs`, `#[ignore]` above 1 000): 20 000 cards with 10 events each, 2 000 items, a 100-item plan; `spec delivery --json`, warm, release, ≤ 1 s, the queue half timed alone too; times in "Implementation". Past the bar, "Known limits" names an `events(project, type)` index, a queue schema step. Mutation, red: `card_items` per card.
- [x] DA-11 — **The daemon serves it, the UI types it.** `GET …/delivery?as_of=2026-11-20` is byte-equal to `spec delivery --as-of 2026-11-20 --json`; `as_of=2026-13-01` and an unknown parameter are 400s naming them. The fixture holds the 11 sets; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `as_of` not passed through; a set left out of the fixture.
- [x] DA-12 — **The tab**, on the mock. Delivery is the third tab, remembered per project; it sends `as_of` the viewer's day; a new date re-reads with it. By release shows "1 of 1" for r1, "Pool", "No release" and an unknown group raw; By department ends in "All tasks"; the late list shows each reason, a "more" line, the item linking the Inbox, the card the Board; an empty project the `EmptyState`. No `roadmap.delivery` leaf in `en.ts` or `ru.ts` holds `block` (`\u` escapes for Russian); `policy.test.ts`, `scale.test.ts` and `words.test.ts` pass, the slice's CSS naming no legacy alias (`git diff` grep). Mutations, each red: `group` compared outside `delivery.ts`; a key missing in `ru.ts`; `as_of` not sent.
- [x] DA-13 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: the mock's tab, the two tables, a late task with every reason, the date field moved past a due date, the empty project.
- [x] DA-14 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Part b, the timeline (core slice 13); RM-0087's states; RM-0086's department overview; an MCP tool; a card `release` field, an estimate unit, configured release dates (Q1-Q3); summing or comparing estimates; roll-ups by person; time travel (`as_of` rewinding the queue); `T-` tasks; a stored, cached or committed figure; the Delivery tab without a `[roadmap]` table.

## Implementation

The UI lane built ahead in a worktree, merged onto `main`. DA-13 and DA-14 ticked on the owner's word.

| Module | Does |
|---|---|
| core `delivery.rs` (new), `roadmap.rs`, `lib.rs` | the judge, `unix_days`; `GroupRef`, `item_groups` (`plan`'s steps 1 and 3 shared, `plan` unchanged) |
| cli `delivery.rs` (new), `lib.rs`, `main.rs`, `subset.rs` | the read, notes, text, cut, JSON; `Outcome::Delivery`; the subcommand; DA-10's four seams |
| http `app.rs`, `args.rs`, `answer.rs` | the route and the 404 listing; `Args::date`; doc |
| ui `api/{provisional,client,http,queries}.ts`, `roadmap/{RoadmapView,RoadmapDelivery}.tsx`, `roadmap/delivery.ts`, `i18n/`, `styles/app.css`, `mocks/{delivery,MockClient}.ts` | 11 types, `getDelivery`, `useDelivery` and its re-read; the tab, the view; the words; 48 keys; `.delivery-*`; the mock |

No store change. **Rust departures**, settled after iteration 2, canon now (`spec-delivery.md`): (1) the 404 lists `delivery` between `events` and `workspaces`, so the older pins hold; (2) the daemon's 400 names its `as_of`, not the CLI's `--as-of`; (3) without `as_of` the library's `today_utc()` runs in the daemon, the handler reading no clock (a Known limit); (4) a queue with no card reads no events or items (an index-only database has no `events` table): DA-10's 1 each holds with cards; (5) the `none` row only when it holds a card, `releases` `[]` with none; (6) the silent cases: iterations 2 and 3's notes below, a non-text move note none; a `due` that is no date, a move missing `id`, `from` or `to`, a re-estimate missing `old`, `new` or `reason` stay silent.

**UI departures** (`spec-roadmap-screen.md` "Delivery", "Mock"): (1) `roadmap.boundary.delivery`, the `RegionBoundary` name, outside `roadmap.delivery`; (2) the date field shows the day asked, a new day a skeleton, no placeholder data; (3) a cancelled subtask's ID links the Board; (4) `DeliveryLate.due` a `string`, `DeliveryItem.kind` a plain `string`; (5) `getDelivery` a judged read, only a 200 resolving; (6) the mock's Chandlery and `milestone` its own, its dates too; (7) a late task's `PropertyList` labelled by its ID; (8) empty is `total.tasks` and `total.cancelled` 0; (9) no plural forms in the lead text; (10) the types hand-written, no regenerate step.

**Review.** Rust ACCEPT, asking a note per item row that does not decode and per plan file or walk place not read, a non-text move note read as none, reasons grouped for late cards only; the test lane's reds (an `RM-` literal in `delivery_tests.rs` against `genre`, the module undeclared, no DA-10 scale test) went to the test-engineer. No UI review is recorded.

**Iteration 2** built them, the exact notes: `<PR>: its row does not read (<column>: <reason>); not listed`, read only with cards; `delivery: plan <path> does not read (<reason>); its item places no task`, `<reason>` the stored read error, `not parsed` or `line <n>: <message>`; `…; its items place no task`, `.` the root, `<reason>` `the directory cannot be listed`, `the [paths] roots entry names no directory and no .md file` or `a name in it is not UTF-8`. Core 17, CLI 23, unit 2, http 5 tests green; clippy clean. **Iteration 3**, from DA-13: a file note only under `[roadmap] dir`, a place note only for that directory, a place in it or a parent of it; other walk problems left to `spec check`.

**Test differences**, the test-engineer's: (1) DA-1's setup keeps the take inside the last 64 events, so its mutation stays green: the test adds 70 reorders after the take; (2) `reestimates` holds the word `estimate`, DA-6's walk excepting it; (3) the daemon's 400 names `as_of`; (4) no card, `releases` `[]`; (5) the item note names the column; (6) `delivery` before `workspaces` in the 404 list; (7) DA-10's 1 each only with cards, no queue file 0 queue reads and 1 plan build; (8) iteration 3's rule. **Pins moved**: http `tests/hub.rs` `GET_ROUTES` gains `/api/projects/{p}/delivery`, 29 (a hub serves the read, by department, never by person); `kind_amendment`'s set count 126 to 137; `layer_c_scan`'s `LATER` gains `delivery.rs`.

**Numbers.** First load 833.46 kB, +2.31 from 831.15 at `d8d47416`, gzip 243.48; the Delivery chunk 9.64 kB (2.62 gzip), outside it; `ru-*.js` 157.23 kB (36.24 gzip); measured in the worktree. UI 192 files, 5 468 tests after the merge; the UI's 11 mutations red. DA-10, release, warm, 20 000 cards, 200 000 card events, 2 000 items, a 100-item plan: `spec delivery --json` median 292 ms (293, 290, 292, 286, 292), the library's read 274, the queue half 212; peak resident 246.6 MB (`/usr/bin/time -l`), not the review's ~100 MB estimate. DA-14: core, CLI and http 4 055 of 4 057, the two (`gate_wrapper` stalled, `show_bindings`) load flakes passing twice alone; clippy clean; store `genre`, the eval guards and `delivery_scale` 63 of 63; UI `daemonKeys` 421 of 421.

**DA-13**, 2026-11-12: a scratch copy of a real game corpus (9 tasks, 1 cancelled, two subtasks given due dates), the debug daemon, a main-tree build, light, Russian, 1440x900. The first `spec delivery` printed 8 `plan <root> does not read` notes for roots missing from the copy, hence iteration 3; then none. The tab: As of and Today; By release, No release 0 of 9, open 9, overdue 1, cancelled 1, None delivered; By department, two rows and All tasks; Late tasks, C-0009 overdue by 7 days with its facts; at 2026-11-25, C-0009 overdue 20, C-0010 5. Not looked at: dark, English, 1100x800, an accepted task with a lead time. For the owner's Russian review: the tab's draft name may read better as the Russian word for deadlines.
