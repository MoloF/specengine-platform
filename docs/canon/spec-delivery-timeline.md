---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# spec delivery --timeline: every task on one axis

Built by `delivery-timeline` (RM-0107 b), 2026-11-13, no ADR: one more output of `spec-delivery.md`'s rule and read, the judge's step 7. Code: core `delivery.rs` (`timeline`), CLI `delivery.rs` (`timeline_lines`, `fitting`), http `app.rs` (`delivery`); the view: `spec-roadmap-screen.md` "Delivery".

## The rule

Asked, `spec delivery --timeline` or the daemon's `timeline=true`, `Delivery.timeline` is `{from, to, markers, tasks}`, built from part a's one read. Not asked, `null` and nothing of it built (no `DeliveryTimeline` seam event); the text is part a's byte for byte, the JSON part a's with `"timeline": null`.

- **Tasks**: a row per card `total.tasks` counts (no cancelled card, every subtask), by group order, then `created_at`, then ID number: `{id, title, flow, group, release, departments, created_at, taken_at, accepted_at, due, days_late}`, stored values as written. `group` is a `DeliveryGroup` (`release`, `pool`, `unplaced`, `none`), `release` the name for `release` alone; `taken_at` is `spec-delivery.md` "The rule"'s take; `days_late` is a late row's, `null` when neither late nor overdue. A `created_at` that is no timestamp gives no row: an open card is noted `` <C>: `<created_at>` is no timestamp; not on the timeline `` (only when asked), a delivered one keeps part a's lead-time note alone.
- **Markers**: one per group holding a row, in that order, `{group, release, first_taken_at, last_accepted_at}`: the least `taken_at` and the greatest `accepted_at` of its rows, of the values that read as a day, each `null` with none. Derived, never a configured date.
- **Axis**: `from` is the least `created_at` day; `to` is the greatest of `as_of` and every stored day of a row (`created_at`, `taken_at`, `accepted_at`, `due`), so it is never before `from` and an open card made or taken after `as_of` stays on it. Both `YYYY-MM-DD`, both `null` with no row.
- **No clock**: the daemon answers `timeline=true` without `as_of` with a 400, running nothing (`spec-delivery.md` "Daemon, hub"); the CLI keeps `today_utc()` without `--as-of`.
- **The UI places, never judges**: a day is a stored value's first ten characters, a UTC date as `unix_days` reads it, at `(day - from) / (to - from)`; lateness, release, department and take are served.

## Text and JSON

Key sets (`fixtures/daemon-keys.json`, 146 to 148): `DeliveryTimeline` 4, `DeliveryMarker` 4, `DeliveryTask` 11; `Delivery` 9, `timeline` after `late`. The text, after part a's late rows and their tail:

```text
timeline | <from|-> to <to|-> | tasks <n>
marker <release R|pool|unplaced|no release> | first taken <at|-> | last accepted <at|->
task <C> | <R|pool|unplaced|-> | created <at> | taken <at|-> | accepted <at|-> | due <D|-> | <late Nd|overdue Nd|-> | <title>
[truncated: <k> of <n> timeline tasks not shown]
```

With no row, `timeline | - to - | tasks 0`. Values are one-lined and lines escaped. The cut at `OUTPUT_CAP_CHARS` goes on past the late rows: the head, marker and task lines print whole while every line but the tails fits, then the tail, `k` the tasks not printed. Both tails can print. The JSON's timeline is never cut; `truncated` and `omitted` count late rows alone. The scenario's pinned timeline at 2026-11-20, C-0004 taken 11-03 by its move to `review`: CLI `tests/delivery_scene/data.rs`.

## Known limits

- **A department's markers**: under the Timeline view's Department filter, a group keeps its served marker, which is computed over all its tasks.
- **Untested cut edges**: the cap falling among the marker lines (no task line prints, `k` every task), and late rows cut while timeline lines still print.
- **Cost**, seeded as part a's (20 000 cards, 200 000 events, 2 000 items, a 100-item plan, release, warm): `--timeline --json` median 251 ms (the library read 249), 5 234 615 B, against 219 ms and 109 963 B without; bar 1.5 s. Part a's four reads stay 1 each with or without it.

## Tests

Core, CLI and http `tests/delivery.rs` (their timeline sections); CLI `src/delivery_tests.rs` (the seam), `tests/delivery_scene/data.rs`; http `daemon_keys.rs`; eval `tests/delivery_scale.rs`; UI `src/roadmap/timeline.test.ts`, `RoadmapTimeline.test.tsx`, `src/mocks/timeline.smoke.test.tsx`. Criteria: `docs/features/delivery-timeline.md`.
