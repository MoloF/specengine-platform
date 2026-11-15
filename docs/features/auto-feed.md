---
class: spec
status: shipped
shipped: 2026-11-15
scope: [crates/specengine-cli, crates/specengine-http, ui]
ref: RM-0124; the owner's word of 2026-11-15 ("I follow how it runs, get the tasks filtered by the last change and see how everything flows, with some details"); release auto-mode, plugin 0.47.0
adrs: []
---

# auto-feed: what happened, newest change first, and what needs the owner (RM-0124)

## Why

Under auto mode the owner reads the night in the morning: what landed, what was held and why, which runs failed, what waits for him. The queue's `events` already hold all of it; this slice reads them into one feed, on the terminal, the local page and the hub alike, and sorts the Tasks list by last change. It decides nothing.

## Description and interactions

Shipped: `spec feed`, `GET /api/projects/:p/feed`, the UI's Activity, the Tasks list's Sort and a decided proposal's Applied by rule. Canon: `docs/canon/spec-feed.md` (command, text, JSON, Needs me, route, UI, known limits); events and reasons: `docs/canon/auto-mode.md`; stored keys and live re-reads: `docs/canon/ui-client.md`.

## Data

`spec feed --json`, its `needs_me` lists and the route's queries: `spec-feed.md` "JSON", "Route". Key sets `Feed`, `FeedEntry`, `FeedNeedsMe` and the five row sets in `fixtures/daemon-keys.json`; `QUEUE_EVENT_TYPES` gains `auto.started`, `auto.bound`, `auto.held`, `auto.landed`, `gate.observed`.

### The UI

`spec-feed.md` "UI": Activity at `#/<p>/feed[/needs-me]`, the nav's second entry in every department and in the palette's Sections; its head, tabs, entries, plain labels and quiet gaps; New since your last visit (`specengine.feed.seen.<p>[.<login>]`); live, never aborting; the Tasks Sort (`specengine.tasks.sort.<p>`); Applied by rule. `FeedView` and the task page `CardPage` are lazy chunks.

## Rules and edge cases

With no `[auto]` table the feed lists the manual events, its head `Auto is off: no [auto] table in specengine.toml@HEAD`; a read only; a payload that is not JSON listed with its `type` and `seq` and a note; another repository's task keeps its ID, its `title`, `status` and `lane` `null`.

## Acceptance criteria

CLI `tests/feed.rs`; http `tests/feed_route.rs`, `daemon_keys.rs`, `hub.rs` (`GET_ROUTES`); UI `src/feed/*.test.ts{,x}`, `src/tasks/TasksView.test.tsx`, `src/api/feed.live.test.tsx`, `feed.http.test.ts`, `src/inbox/ClosedProposal.test.tsx`, `src/mocks/feed.smoke.test.tsx`, `daemonKeys.test.ts`.

- [x] FD1 — **The feed lists the night in order** (`fd1_the_feed_lists_the_night_in_seq_order`): pinned text and JSON in `seq` order; `--since <seq>`, `--since <time>`, `--task`, `--department`. Mutations: ordering by `at`; `by` copied from a manual apply.
- [x] FD2 — **Needs me** (`fd2_needs_me`, `fd2_a_confirm_after_a_hold_leaves_held`): each list; a held task landed after, and a hub's held proposal decided after, leave `held`. Mutation: `held` from any past `auto.held`.
- [x] FD3 — **One body, local and hub** (`fd3_the_local_route_is_spec_feed_json`, `fd3_on_a_hub_any_login_reads_it_and_the_head_is_the_hubs`; `GET_ROUTES`, the key sets). Mutation: the hub's head judged as a terminal's.
- [x] FD4 — **Tasks by last change** (`TasksView.test.tsx` "the list's Sort (FD4 of auto-feed)"). Mutation: the default changed.
- [x] FD5 — **New since the last visit** (`FeedView.test.tsx` "New since your last visit", "the first visit's count"; `feed.test.ts` "the stored point"). Mutation: writing `seen` on open.
- [x] FD6 — **Live, never aborting** (`feed.live.test.tsx` "the feed's live re-read"). Mutation: `cancelRefetch: true`.
- [x] FD7 — **Applied by rule** (`ClosedProposal.test.tsx` "a rule's apply on a closed proposal"). Mutation: the line for any `applied` row.
- [ ] FD8 — **[owner, gate G4]** One overnight run on the owner's game project with three or more tasks, read in the morning from Activity's Needs me and All, newest first.

## Out of scope

Notifications outside the page; a stored per-person feed on the server; acting from the feed; cost or token figures.

## Implementation

| Module | What it does |
|---|---|
| CLI `feed.rs` (new), `main.rs`, `lib.rs` | `feed`, `FeedRequest`, `FeedOutcome`, `FeedEntry`, `FeedNeedsMe`, the text, `feed_since`, `feed_task` |
| store `queue/feed.rs` (new) | `FeedSnapshot`: events, tasks, runs, proposals and cards in one `open_existing` read |
| http `src/feed.rs` (new), `app.rs` | the route, its queries, the hub's surface |
| UI `src/feed/` (new: `FeedView`, `words.ts`, `seen.ts`), `api/feedQueries.ts`, `feedMerge.ts`, `client.ts`, `http.ts`, `provisional.ts`, `queries.ts` | Activity, its read, merge and live re-read; the stored point |
| UI `app/routes.ts`, `Shell.tsx`, `palette/options.ts`, `overview/HomeView.tsx` | the routes, the nav's Activity, lazy `FeedView` and `CardPage`, the palette's uncut Sections, the Overview's link |
| UI `tasks/sort.ts`, `TaskList.tsx`, `TasksView.tsx`; `inbox/ClosedProposal.tsx`; `i18n/{en,ru}.ts`, `styles/app.css`, `mocks/` | the Sort; Applied by rule; strings, styles, the `feed` scenario |

**Decisions** (the orchestrator's): an item leaves `held` once nothing waits on it (a task landed or cancelled; a proposal held alone once not `open`; a gone row or another repository's task stays); `--task T` keeps the `auto.started` naming T.

**Departures**: Activity is in the left nav, second after Overview, in every department, and in the palette's Sections (eleven entries), the owner's word over this spec's off-nav; reasons (29), acts and kinds as plain labels, the code in `title`; Needs me lists newest first; a "Nothing happened for H h M min" line for gaps of two hours or more (a usage-limit pause has no event); Mark all seen never lowers the point and is announced; the New chip shows `100+` at the cap; a gap re-reads the feed whole; a proposal event's subject, title and status are the proposal's; no `--since`: `more` false; the filters before the 512 cap; `failed_runs`, `gate_observed` over the window or the newest 100 events; `held`, `unlanded` this repository's packs alone; a non-JSON payload's `at` `null`; no `auto` head line in the text. The page links a proposal event bound to a task to the task.

**Known limits**: `FeedEntry` lacks `auto.started`'s counts and branch, `auto.landed`'s branch and `gate.observed`'s zone. First load is 844.78 kB, 0.22 kB under its 845 kB bar (`ui-languages.md` "Size"): the next screen is the next lazy split.
