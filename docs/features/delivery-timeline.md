---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0107 part b, v5 core slice 13 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; docs/features/delivery-analytics.md (part a); the orchestrator's working answers, the owner's review open
adrs: []
---

# delivery-timeline: when each task was created, taken and accepted, against its due date and its release (RM-0107 b)

## Why

Part a, `delivery-analytics`, counted delivery per release and department and listed late tasks with their reasons, but it showed no time. The owner asked where we are and what took longer (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"). Now `spec delivery --timeline` and `GET …/delivery?as_of=D&timeline=true` add a `timeline` to part a's document, from its one read: every task not cancelled with its creation, take, accept, due date and days late; a marker per group, its first take and last accept; a served axis. The Delivery tab gains a lazy Timeline view: a row per task on that axis, drawn in CSS with no chart library, with a table twin; every row is reachable by keyboard and read whole by a screen reader. No ADR. Canon: `docs/canon/spec-delivery.md` "Timeline", `docs/canon/spec-delivery-timeline.md`, `docs/canon/spec-roadmap-screen.md` "Delivery".

## Acceptance criteria

**[look]** marks a browser look and **[measure]** a timed or counted run; every other criterion is a test that its named mutation turned red. "The scenario" is part a's, with C-0005 made 2026-11-06.

- [x] DT1 — **Rows.** On the scenario, `--as-of 2026-11-20 --timeline --json` gives the pinned `timeline` (CLI `tests/delivery_scene/data.rs`), and the text gives its lines after part a's. The rows are C-0001, C-0003, C-0004 and C-0005, with no row for C-0002. C-0004 is taken 2026-11-03 by its move to `review` (`copy` accepts only from `review`) and accepted 11-05. An open card whose `created_at` is no timestamp has no row and gets its note. Mutations, each red: cancelled cards as rows; rows by `created_at` alone; `days_late` on an on-time card; the note dropped.
- [x] DT2 — **Markers.** r1 11-03 and 11-15; r2 11-05 and `null`; `none` 11-03 and 11-05. A group whose one task is open and untaken: `null` twice. A group of cancelled cards alone: no marker. Mutations, each red: the first accept as `last_accepted_at`; markers over cancelled cards.
- [x] DT3 — **[measure] One read, still.** The DA-10 seam counts 1 each with and without `--timeline`. `delivery_scale.rs` (20 000 cards, 200 000 events, 2 000 items, a 100-item plan, release, warm): `--timeline --json` median 251 ms, 5 234 615 B; without the flag 219 ms, 109 963 B; the library read 249 ms; bar 1.5 s. Mutation, red: a second `events_of_types` call.
- [x] DT4 — **Off by default.** Without `--timeline`, the text is DA-9's pin byte for byte, and the JSON is that pin plus `"timeline": null`. The seam (`subset.rs` `Event::DeliveryTimeline`, `ReadCounts.delivery_timelines`) records no build. Mutation, red: the timeline built whatever the flag.
- [x] DT5 — **The axis, no clock.** `from` 2026-11-01, `to` 2026-11-20; at `--as-of 2026-11-01`, `to` is 2026-11-15, the greatest of `as_of` and every stored day of a row. Output is byte-identical twice, and at another root and `HOME`. `timeline=true` without `as_of` gives the 400. Mutations, each red: `to` as `as_of` alone; the daemon defaulting `as_of`.
- [x] DT6 — **Served and typed.** `?as_of=2026-11-20&timeline=true` is byte-equal to the CLI's `--json`; `timeline=yes` is a 400 naming it. The fixture holds `Delivery` (9 keys) and `DeliveryTimeline` 4, `DeliveryMarker` 4, `DeliveryTask` 11 (sets 146 to 148 of 148). `provisional.ts` types them and `AHEAD_OF_FIXTURE` is empty. Mutations, each red: `timeline` not passed through; a set left out.
- [x] DT7 — **UTC days** (`timeline.test.ts`, `TZ` `Pacific/Kiritimati`). `dayOf("2026-11-03T23:30:00Z")` is the 3rd, and day numbers equal `unix_days`. `from` sits at 0, `to` at 1, and C-0001's take at 2/19. A take before creation is an empty stretch. Mutations, each red: the day through `new Date(…).getDate()`; a negative width.
- [x] DT8 — **The view, on the mock** (`RoadmapTimeline.test.tsx`, `timeline.smoke.test.tsx`). It checks:
  - rows in served order under their headings, with markers in words;
  - Board links, and each link's description: its title, dates and lateness;
  - tracks `aria-hidden`, with only start and width inline;
  - the table, Department Print keeping C-0001, C-0003 and C-0004, the empty timeline, and the daemon's 400 with Retry.

  Mutations, each red: lateness from `due` in the UI; `group` compared outside `delivery.ts`; a `timeline.*` key missing in `ru.ts`; a description without lateness; a track not `aria-hidden`.
- [x] DT9 — **[measure] 2 000 tasks.** `large` mounts exactly 200 rows, at most 12 elements each; "Show 200 more" gives 400 and moves focus. Timed in Chrome on a production build, `?scenario=large`, with scripted rAF timing rather than the Performance panel:
  - warm, Tables to Timeline: 49-63 ms to 200 rows painted;
  - "Show 200 more": 41 ms;
  - the first press: 368 ms cold, including the lazy chunk's fetch.

  Warm runs are within 100 ms, so `TIMELINE_PAGE` stays 200. Mutation, red: every row mounted at once.
- [x] DT10 — **Lazy and size.** The `RoadmapTimeline` chunk (11.17 kB) is not in first load. First load is 834.16 kB, up 0.76 kB of the 1.5 kB allowed, under 845. Mutation, red: a static import (`lazy.test.ts`).
- [x] DT11 — **[look]** The orchestrator's, on a production build over the mock:
  - `normal`, 1440x900, light, English: dashed markers, C-0005 "Delivered 5 days late", C-0001 "Overdue by 2 days", the untaken C-0013 hatched, the As of line, ticks "Nov 2026 | 9 | 16 | 23" with no overlap;
  - 1100x800, dark, Russian: the ticks apart, the lateness words translated;
  - Tab order: the switch, Department, Show as table, the region "Tasks over time", then each row's link.

  VoiceOver was not run. C-0005's accessible description was read from the DOM instead: "Mooring fees table", then "Created 28 Oct 2026. Taken 29 Oct 2026. Accepted 9 Nov 2026. Due 4 Nov 2026. Delivered 5 days late."
- [x] DT12 — **Gates.** Both reviews returned ACCEPT after iteration 2. The orchestrator ran the Rust tests, UI lint, `tsc`, `pnpm test` (5 852) and `pnpm build`, all green. At ship, `spec check` and eval `doc_pointers`, `ui_policy` and `workspace_names` passed. `anonymity` failed once, on a Russian quote in `docs/roadmap/RM-0112.md`, which is outside this slice.

## Out of scope

RM-0087's states; configured release dates, a card `release` field, an estimate as a bar; zoom, pan, drag or a canvas; a chart library; an MCP tool; a timeline in `docs/generated/roadmap.md` (ADR-0052); `as_of` rewinding the queue; cancelled cards as rows; rows by person.

## Implementation

| Module | What it does |
|---|---|
| core `delivery.rs` | step 7: `DeliveryInput.timeline`, `Timeline`, `TimelineMarker`, `TimelineTask`; the open-card note |
| CLI `delivery.rs`, `main.rs`, `lib.rs`, `subset.rs` | `DeliveryRequest.timeline`, `timeline_shown`, `timeline_omitted()`; the lines and the cut (`timeline_lines`, `fitting`); `--timeline`; the DT4 seam |
| http `app.rs` | `timeline`, its two 400s, the unknown-name list |
| tests | core, CLI, http `delivery.rs`; `delivery_scene/`, `delivery_tests.rs`, `kind_amendment.rs`; `daemon_keys.rs` with `fixtures/daemon-keys.json`; eval `delivery_scale.rs` |
| UI `api/{provisional,client,http,queries}.ts` | three types and `timeline`; `getDelivery({asOf, timeline})`, `&timeline=true`; `useDelivery(p, asOf, timeline)` |
| UI `roadmap/{RoadmapDelivery,RoadmapTimeline}.tsx`, `roadmap/{timeline,delivery}.ts` | the View switch; the lazy view and its table; day arithmetic and ticks; `groupKey`, `lateMark` |
| UI `mocks/`, `i18n/`, `styles/` | the mock judge, `large`, `empty`; 24 leaves; `.delivery-timeline-*`, seven colour roles, two sizes |

**Rust departures**, accepted and now canon:
1. `to` covers every stored day of a row.
2. The group type is `DeliveryGroup`, which has `none`.
3. Markers are taken over readable days only.
4. The text cut keeps both tails.
5. The DT4 seam lives in the CLI.
6. Iteration 2: an open card with an unreadable `created_at` is noted, only with `--timeline`.

**UI departures:**
1. The view choice lives in the tab's state, and leaving the tab resets it.
2. Part a's leaves are reused for loading, the failure title and five headers.
3. The as-of line is drawn per group, plus a mark on the axis.
4. Group headings are `h2`; the empty timeline is an `EmptyState` at `h3`, and an empty department a plain line.
5. The accept mark is a `::after`.
6. The table's Due shows the stored day.
7. The label reads "Show min(200, k) more (k left)".
8. Part a's notes appear on Tables only.
9. Iteration 2: ticks thinned to at most 11 labels, Mondays near a month's first dropped, year labels added, and the description starts with the title.
10. The ui-developer edited `tokens.css`, tests, builders and stubs, and wrote the UI tests, as in part a.
11. The Russian is a draft.

**Corrected at ship:**
- C-0004's take is 11-03, not 11-05. CLI `data.rs` keeps the old value as `SPEC_TAKE` and rewrites it, while the UI stub and mock still pin 11-05.
- `to` now has a single wording.
- `GroupKind` became `DeliveryGroup`.
- Part a's note stands for delivered cards only.

**Open:** the owner's review of the working answers (derived markers, no chart library, a lazy chunk); the Russian draft; a VoiceOver run.
