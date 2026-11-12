---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0107 part b, v5 core slice 13 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 3, RM-0107, the timeline); docs/features/delivery-analytics.md (part a); the orchestrator's working answers, the owner's review open
adrs: []
---

# delivery-timeline: when each task was created, taken and accepted, against its due date and its release (RM-0107 b)

## Why

Part a, `delivery-analytics`, counts delivery per release and department and lists late tasks with reasons, but shows no time. The owner asks where we are and what took longer (`08-roadmap.md` Phase 7, "Owner's vision, 2026-11-11"): every task on one axis, created, taken, accepted, against its due date, with when each release started and last delivered.

After this slice `spec delivery --timeline` and `GET …/delivery?timeline=true` add a `timeline` to part a's document from its one read, and the Delivery tab a lazy Timeline view: a row per task on a shared axis in CSS, no chart library, a table of the same dates its text twin, each row reachable by keyboard and read whole by a screen reader. No ADR: part a's (ADR-0052, ADR-0056, ADR-0074, ADR-0076, ADR-0012) and `docs/canon/architecture.md#ui`.

## Description and interactions

### Decisions

- **D1. Part a's read, one more output.** `delivery()` already finds each card's take, accept and release; no new store read or event type (DT3).
- **D2. Only when asked**: `--timeline`, `timeline=true`; else `timeline: null`, so the tables' read stays light (at 20 000 cards the timeline alone is megabytes).
- **D3. The daemon reads no clock for it**: `timeline=true` needs `as_of`, else 400. The CLI keeps part a's `today_utc()` default on a terminal.
- **D4. Rows are part a's `tasks`**: every card not cancelled, subtasks too (part a's W1). A cancelled card stays a count and a reason.
- **D5. Release markers are derived**: a group's first take and last accept among its tasks; no configured date (part a's Q3).
- **D6. Served axis, drawn by the UI**: `from`, `to` served; the UI places a day at `(day - from) / (to - from)`, a day being a stored value's first ten characters, the UTC date core's `unix_days` reads, never local time. `days_late` is served; the UI judges no lateness.
- **D7. No MCP tool, no plugin bump** (part a's D8).

## Data

### Core (`crates/specengine-core/src/delivery.rs`)

```rust
pub struct DeliveryInput<'a> { /* part a's fields */ pub timeline: bool }
pub struct Timeline { pub from: Option<String>, pub to: Option<String>, pub markers: Vec<TimelineMarker>, pub tasks: Vec<TimelineTask> }
pub struct TimelineMarker { pub group: GroupKind, pub release: Option<String>, pub first_taken_at: Option<String>, pub last_accepted_at: Option<String> }
pub struct TimelineTask { pub id: String, pub title: String, pub flow: String, pub group: GroupKind, pub release: Option<String>,
  pub departments: Vec<String>, pub created_at: String, pub taken_at: Option<String>, pub accepted_at: Option<String>,
  pub due: Option<String>, pub days_late: Option<i64> }
```

`Delivery` gains `timeline: Option<Timeline>`, `None` unless asked. Judge step 7:
- **Tasks**: every card `total.tasks` counts, but one whose `created_at` is no timestamp (part a's note stands; left out); by group order (part a's step 5), then `created_at`, then ID number; `days_late` part a's step 6, `null` when neither late nor overdue.
- **Markers**: one per group holding a task, that order; `first_taken_at` the least `taken_at`, `last_accepted_at` the greatest `accepted_at`, each `null` with none.
- **Axis**: `from` the least `created_at` date; `to` the greatest of `as_of`, every `accepted_at` date and every `due`; both `null` with no task.

CLI: `DeliveryRequest.timeline: bool`, `spec delivery [--as-of D] [--timeline] [--json]`; no git, index or write, as part a.

### JSON

`{as_of, notes, releases, departments, total, late, timeline, truncated, omitted}`. Part a's scenario (`docs/features/delivery-analytics.md` "Data"), C-0005 made 2026-11-06T09:00:00Z (part a leaves it open), `--as-of 2026-11-20 --timeline`:

```json
"timeline": {"from": "2026-11-01", "to": "2026-11-20",
 "markers": [
  {"group": "release", "release": "r1", "first_taken_at": "2026-11-03T09:00:00Z", "last_accepted_at": "2026-11-15T09:00:00Z"},
  {"group": "release", "release": "r2", "first_taken_at": "2026-11-05T09:00:00Z", "last_accepted_at": null},
  {"group": "none", "release": null, "first_taken_at": "2026-11-05T09:00:00Z", "last_accepted_at": "2026-11-05T09:00:00Z"}],
 "tasks": [
  {"id": "C-0001", "title": "Write the tide table foreword", "flow": "copy", "group": "release", "release": "r1", "departments": ["words", "print"],
   "created_at": "2026-11-01T09:00:00Z", "taken_at": "2026-11-03T09:00:00Z", "accepted_at": "2026-11-15T09:00:00Z", "due": "2026-11-10", "days_late": 5},
  {"id": "C-0003", "title": "Engrave the harbor map", "flow": "plates", "group": "release", "release": "r2", "departments": ["print"],
   "created_at": "2026-11-04T09:00:00Z", "taken_at": "2026-11-05T09:00:00Z", "accepted_at": null, "due": "2026-11-12", "days_late": 8},
  {"id": "C-0004", "title": "Fix the index", "flow": "copy", "group": "none", "release": null, "departments": ["words", "print"],
   "created_at": "2026-11-02T09:00:00Z", "taken_at": "2026-11-05T09:00:00Z", "accepted_at": "2026-11-05T09:00:00Z", "due": null, "days_late": null},
  {"id": "C-0005", "title": "Record the foghorn", "flow": "sound", "group": "none", "release": null, "departments": [],
   "created_at": "2026-11-06T09:00:00Z", "taken_at": null, "accepted_at": null, "due": null, "days_late": null}]}
```

C-0002, cancelled, has no row; C-0004, accepted from its first column, takes at its accept (part a's DA-2). Without `--timeline` the document is part a's plus `"timeline": null`. `truncated` and `omitted` still count late rows; the timeline is never cut in JSON.

### Text

With `--timeline`, after the late rows:

```text
timeline | 2026-11-01 to 2026-11-20 | tasks 4
marker release r1 | first taken 2026-11-03T09:00:00Z | last accepted 2026-11-15T09:00:00Z
marker release r2 | first taken 2026-11-05T09:00:00Z | last accepted -
marker no release | first taken 2026-11-05T09:00:00Z | last accepted 2026-11-05T09:00:00Z
task C-0001 | r1 | created 2026-11-01T09:00:00Z | taken 2026-11-03T09:00:00Z | accepted 2026-11-15T09:00:00Z | due 2026-11-10 | late 5d | Write the tide table foreword
task C-0003 | r2 | created 2026-11-04T09:00:00Z | taken 2026-11-05T09:00:00Z | accepted - | due 2026-11-12 | overdue 8d | Engrave the harbor map
task C-0004 | - | created 2026-11-02T09:00:00Z | taken 2026-11-05T09:00:00Z | accepted 2026-11-05T09:00:00Z | due - | - | Fix the index
task C-0005 | - | created 2026-11-06T09:00:00Z | taken - | accepted - | due - | - | Record the foghorn
```

Group words are part a's (a marker `release <name>`, `pool`, `unplaced`, `no release`; a task the name, `pool`, `unplaced`, `-`); no task: `timeline | - to - | tasks 0`; values pass `one_line`. The cut at `OUTPUT_CAP_CHARS` extends part a's: after the late rows, the head and marker lines whole while they fit, task lines while they fit, then `[truncated: <k> of <n> timeline tasks not shown]`. Without `--timeline`, part a's text byte for byte.

### Daemon

`GET /api/projects/:p/delivery` takes `timeline` (`true`/`false`, `args.boolean`) beside `as_of`. `timeline=true` without `as_of`: 400 `` `timeline` needs `as_of`: the daemon reads no clock for a timeline ``; a bad value the boolean's 400 naming `timeline`. `?as_of=2026-11-20&timeline=true` is byte-equal to `spec delivery --as-of 2026-11-20 --timeline --json`. Key sets (`fixtures/daemon-keys.json`): `Delivery` 8 -> 9; new `DeliveryTimeline` (4), `DeliveryMarker` (4), `DeliveryTask` (11).

### The UI line

- **Types** (`provisional.ts`, citing DT6): `DeliveryTimeline`, `DeliveryMarker`, `DeliveryTask`, `DeliveryDocument.timeline`; `client.ts` `getDelivery(project, {asOf, timeline}, signal)`, `http.ts` `&timeline=true`. `useDelivery(project, asOf, timeline)`, key `["delivery", p, asOf, timeline]`, re-read as part a's. The mock (`src/mocks/delivery.ts`): every shape, an unknown group, an empty timeline; `large`, 2 000 tasks over 5 releases and 4 departments.
- **The switch**: `RoadmapDelivery` gains a group "View", two `Button`s `aria-pressed`, Tables (first) and Timeline, kept for the visit. Timeline mounts `const RoadmapTimeline = lazy(() => import("./RoadmapTimeline").then(…))` in a `Suspense` with a skeleton and reads with `timeline` true.
- **The view** (`src/roadmap/RoadmapTimeline.tsx`; day arithmetic in `src/roadmap/timeline.ts`, `dayOf(stored)`, `placeOf(day, from, to)`):
  - controls: a "Department" select (All, then served departments by title) keeping rows whose `departments` holds the name; a subtle `Button` "Show as table" / "Show as timeline";
  - an `aria-hidden` axis: UTC ticks on each month's 1st, plus Mondays under a 60-day span, labelled in the viewer's language; an "As of" line; `from`, `to`, `as_of` also as text above it;
  - a group per marker, served order: a heading with the release (Pool, Unplaced, No release; unknown raw, `lang="en"`) and its markers in words ("First taken 3 Nov 2026, last accepted 15 Nov 2026" or "nothing accepted yet"), two `aria-hidden` lines across its rows;
  - rows, an `ol` per group: a label (the ID linking `sectionHash(p, "board", id)`, the title in `<bdi>`, part a's "Overdue by N days" or "Delivered N days late" with an icon) and an `aria-hidden` track: waiting from `created_at` to `taken_at` (else `as_of`), work from `taken_at` to `accepted_at` (else `as_of`), an accept mark, a due tick. The link's `aria-describedby` names a hidden line of its dates in words ("Created 1 Nov 2026. Taken 3 Nov 2026. Accepted 15 Nov 2026. Due 10 Nov 2026. Delivered 5 days late."), missing ones left out;
  - the first 200 rows across groups (`TIMELINE_PAGE`), then "Show 200 more (k left)";
  - the table: caption "Timeline", columns Task, Release, Created, Taken, Accepted, Due, Late, same rows and paging, dates in `ui/time.ts`'s UTC forms;
  - below 1100 px the track keeps a token minimum width in a region that scrolls alone (`role="region"`, labelled, `tabindex="0"`); bars placed by percentage `inset-inline-start` and `width`, as `RoadmapLanes.tsx`'s, no other inline size; a backward segment drawn empty; states as part a's, `tasks` `[]` "No task to place in time".
- **Strings**: about 25 leaves under `roadmap.delivery.timeline` in `en.ts` and `ru.ts` (the Russian a draft, plural forms for `more`), none saying "blocked". **CSS**: `app.css` `.delivery-timeline-*`, scale tokens and `tokens.css` roles, no `SCALE_EXCEPTIONS` entry.

## Rules and edge cases

- WHEN `timeline` is not asked, the system SHALL serve `timeline: null` and build nothing for it; WHEN the route gets `timeline=true` without `as_of`, it SHALL answer 400 and run nothing.
- WHEN `as_of` precedes a task's `created_at`, its open segments SHALL be drawn empty; `to` still covers every stored date. WHEN a card is accepted from its first column, its work segment SHALL be empty.
- The UI SHALL NOT compute lateness, a release, a department or a take, and SHALL read a day from a stored value's first ten characters only.
- One queue, config and file state with one `as_of` SHALL give one text and JSON, byte-identical at other roots and `HOME`s. No key names a person; no estimate is drawn, summed or compared (part a's D6).

**Rollout.** No schema step, backup format, config key or tool. A daemon before this slice answers `timeline=true` with part a's 400 for an unknown parameter; the UI ships inside its daemon; the hub takes it at its next upgrade, the owner's call.

**Test churn.** Part a's JSON golden (`"timeline": null`); `daemon_keys.rs`, `fixtures/daemon-keys.json`, `daemonKeys.test.ts` (four sets); the route's 400 parameter list; the Delivery tab's tests (the switch).

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Rows and events go through the store with fixed `now`s; "the scenario" is part a's, C-0005 made 2026-11-06.

- [ ] DT1 — **Rows.** On the scenario, `--as-of 2026-11-20 --timeline --json` gives Data's `timeline` exactly, and the text "Text"'s lines after part a's: no C-0002 row, C-0004's `taken_at` its `accepted_at`. Mutations, each red: cancelled cards as rows; rows by `created_at` alone; `days_late` on an on-time card.
- [ ] DT2 — **Markers.** r1 11-03 and 11-15, r2 11-05 and `null`, `none` 11-05 twice; a group whose one task is open and untaken `null` twice; a group of cancelled cards alone no marker. Mutations, each red: the first accept as `last_accepted_at`; markers over cancelled cards.
- [ ] DT3 — **[measure] One read, still.** Part a's DA-10 seam counts the same store calls and plan builds with and without `--timeline`. Seeded as DA-10 (`delivery_scale.rs`, 20 000 cards), `--timeline --json`, warm, release, ≤ 1.5 s, its bytes and time in "Implementation". Mutation, red: a second `events_of_types` call for the timeline.
- [ ] DT4 — **Off by default.** Without `--timeline` the text is byte-equal to part a's DA-9 pin and the JSON that pin plus `"timeline": null`; a seam shows no `TimelineTask` built. Mutation, red: the timeline built whatever the flag.
- [ ] DT5 — **The axis, no clock.** `from` 2026-11-01, `to` 2026-11-20; at `--as-of 2026-11-01`, `to` 2026-11-15. Byte-identical twice, at another root and `HOME`. `GET …/delivery?timeline=true` without `as_of` is the 400. Mutations, each red: `to` as `as_of` alone; the daemon defaulting `as_of` for a timeline.
- [ ] DT6 — **The daemon serves it, the UI types it.** `?as_of=2026-11-20&timeline=true` is byte-equal to the CLI's `--json`; `timeline=yes` a 400 naming `timeline`. The fixture holds `Delivery` (9) and the three sets; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `timeline` not passed through; a set left out.
- [ ] DT7 — **UTC days** (`timeline.test.ts`, `process.env.TZ` set to `Pacific/Kiritimati`, UTC+14, before any `Date`). `dayOf("2026-11-03T23:30:00Z")` is 2026-11-03's day; `placeOf` puts `from` at 0, `to` at 1, C-0001's take at 2/19; a take before creation an empty segment. Mutations, each red: the day through `new Date(…).getDate()`; a negative width.
- [ ] DT8 — **The view, on the mock** (`RoadmapTimeline.test.tsx`, the smoke test). Rows in served order under their headings with markers in words; each ID links the Board; each link's description names its dates and lateness; tracks `aria-hidden`; "Show as table" a `table` of the same rows and dates; Department Print keeps C-0001, C-0003, C-0004; an empty timeline its line. `policy.test.ts`, `scale.test.ts`, `words.test.ts` pass. Mutations, each red: lateness computed from `due` in the UI (a mock row whose served `days_late` disagrees with its dates shows the served one); `group` compared outside `delivery.ts`; a `timeline.*` key missing in `ru.ts`.
- [ ] DT9 — **[measure] Render at 2 000 tasks.** Counted: the `large` document mounts exactly 200 rows, at most 12 elements a row; "Show 200 more" 400. Timed, the orchestrator's: Chrome, a production build, `?scenario=large`, the Performance panel's scripting plus rendering from the Timeline press to the first 200 rows painted, and for one "Show 200 more", each ≤ 100 ms on the owner's laptop, recorded in "Implementation"; past it, `TIMELINE_PAGE` 100 and a "Known limits" line. Mutation, red: every row mounted at once.
- [ ] DT10 — **Lazy and size.** After `pnpm build` the `RoadmapTimeline` chunk is not in first load; first-load JS grows at most 1.5 kB, within 845 kB, the figure recorded. Mutation, red: a static import of `RoadmapTimeline` in `RoadmapDelivery.tsx` (`lazy.test.ts`).
- [ ] DT11 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: the mock's timeline (markers, an overdue, a late and an untaken row, the As of line), the table, the filter, "Show 200 more" in `large`; keyboard only, Tab reaches the switch, the filter, the scroll region and each row's link in order; VoiceOver reads one row's dates.
- [ ] DT12 — **Gates.** `cargo nextest run` green for `specengine-core`, `-cli`, `-http`, then the full run once; the eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass; UI lint, `tsc`, `pnpm test`, `pnpm build` pass.

## Out of scope

RM-0087's states; configured release dates, a card `release` field, an estimate as a bar; zoom, pan, drag or a canvas; a chart library; an MCP tool; a timeline in `docs/generated/roadmap.md` (ADR-0052); `as_of` rewinding the queue; cancelled cards as rows; rows by person.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/delivery.rs`; `crates/specengine-cli/src/{delivery,main}.rs`; `crates/specengine-http/src/app.rs` (the parameter, the 400) |
| test-engineer, Rust | core and CLI `delivery.rs` tests (DT1, DT2, DT4, DT5), DT3's seam and `crates/specengine-eval/tests/delivery_scale.rs`, http (DT5, DT6); `daemon_keys.rs` and `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`, once) with `daemonKeys.test.ts` in the same change |
| ui-developer | `provisional.ts`, `client.ts`, `http.ts`, `queries.ts`, `src/mocks/delivery.ts`, `src/roadmap/{RoadmapDelivery,RoadmapTimeline}.tsx`, `src/roadmap/timeline.ts`, `src/app/lazy.test.ts`, `src/styles/app.css`, `en.ts`, `ru.ts` |
| test-engineer, UI | `timeline.test.ts`, `RoadmapTimeline.test.tsx`, the smoke test; DT7 to DT10's mutations and figures |

Comments cite `docs/features/delivery-timeline.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. After part a ships and after core slice 12 `hub-department`, one Rust slice at a time; the UI after part a's UI and `design-sweep-b`.
2. Core, CLI, http; the UI behind `AHEAD_OF_FIXTURE`; tests overlap; reviews; the look (DT11); ship ("Canon at ship"), closing RM-0107. At most three iterations a lane.

### Risks

- **Render cost**: DT9 is the bar, paging the lever; a row stays a few elements, no per-row listener but its link.
- **Day arithmetic in two languages**: `timeline.ts` mirrors `unix_days` by string; DT7 runs in a far zone so a local-time slip turns red.
- **First load**: about 25 English leaves against DT10's 1.5 kB; past it, they move into the chunk's own catalog part (`ui-languages.md` "Size").

### Canon at ship

| File | Edit |
|---|---|
| `docs/canon/spec-delivery.md` (part a's, new at its ship) | "Timeline": flag, rule, markers, axis, JSON and text, the route's 400; "Known limits": paging, the measured figures |
| `docs/canon/spec-roadmap-screen.md` | "Layout": Delivery's Tables and Timeline views, the timeline lazy |
| `docs/canon/ui-languages.md` | "Size": the figure. Net |
| READMEs: cli, http | a pointer each (`--timeline`, `timeline=`). Net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 13; RM-0107 | shipped, both parts |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered: release markers are the first take and the last accept (part a's Q3); no chart library; a lazy chunk.

**The spec-writer's:** D2 (a flag), D3 (the route needs `as_of`; the CLI keeps part a's UTC default) and D6 (day offsets per row would add four keys a task for a division the UI can make); paging at 200, as the Chain's 200-box cap (`docs/canon/spec-roadmap-screen.md` "Layout"); C-0005's creation, which part a's scenario left open.
