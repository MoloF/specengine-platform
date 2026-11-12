---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# spec delivery: what was delivered, what is overdue and why

Built by `delivery-analytics` (RM-0107 part a), no ADR: it rests on ADR-0052 (the plan computed per read), ADR-0056 (cards; "overdue", never "blocked"), ADR-0074 (a return is a move), ADR-0076 (departments) and ADR-0012 (nothing gated). Related: `spec-roadmap.md`, `cards.md`, `workspaces.md` "Config". Code: core `delivery.rs`, `roadmap.rs` (`item_groups`); CLI `delivery.rs`, `subset.rs` (seams); http `app.rs`, `args.rs` (`Args::date`); UI: `spec-roadmap-screen.md` "Delivery".

## The rule

Per release and per department, the counts and the lead time; then every late task with its reasons. Computed per read from the queue, the config and the plan's files; never stored, cached or written into `docs/generated/roadmap.md` (git alone, ADR-0052); it moves, refuses and gates nothing. Every card counts, a subtask too.

- **Release.** The group of the first plan item, in plan order, among the card's `targets`; none, its parent's, up the chain; a repeat or a parent not read ends at no release (`none`). Plan order is group order: `[roadmap] releases` in order, the pool, then unplaced; a done item places as an open one does (core `item_groups`: `spec-roadmap.md` "Plan" steps 1 and 3 over every item). No card field.
- **Departments.** Every served workspace whose `flows` is absent or lists the card's flow, in served order: a flow two list counts in both, the total once; with none configured, the implicit `all`. A card in no department counts in the total alone; a flow no longer configured, in those listing every flow. No row or key names a person.
- **Taken**: the card's first `card.moved` or `card.accepted` with `from` ≠ `to`: an accept from its first column is its take, a reorder never one. **Returned**: a move from another column into its flow's `return` column (`card-flows.md` "Return"), with its note; a flow with none, or gone from the config, has no returns.
- **Standing.** Cancelled: counted only as `cancelled`. Accepted: delivered, late when `acceptance.at`'s date is after `due`. Else open, overdue when `due` is before `as_of` (on the due day, not). No `due`, or one that is no date: neither. Days are UTC calendar dates (`unix_days`).
- **`as_of`** is an input, `--as-of D` or the daemon's `as_of`, always printed and served; it judges open cards only and never rewinds the queue.
- **Counts** of a row: `tasks` (not cancelled), `delivered`, `delivered_late`, `open`, `overdue`, `cancelled`; `lead_days` `{median, max}` over the delivered whose two dates read (the accept's day less the creation's), `null` with none.
- **Estimates** have no unit and are never summed, averaged or compared: a late task shows its estimate and each re-estimate as written; lateness is `due`'s alone.
- **Rows.** A release row per group holding a card, cancelled ones too, in group order, then `none` when it holds one (no card: `releases` `[]`); a department row per served workspace, zeros too; `total` over every card once.
- **Late**: every card delivered late or overdue, `days_late` past `due` (to the accept's day, or `as_of`), by `days_late` descending, then ID number. Its reasons: returns and re-estimates by `seq`; cancelled subtasks (its children with a `cancellation`) and its question, discrepancy and decision items, by ID. Each list keeps its oldest `REASONS_MAX` 16, `more` counting the rest.

## The judge

Core `delivery.rs`, pure, no clock, I/O or queue: `delivery(&DeliveryInput {as_of, cards, moves, reestimates, items, groups, departments, returns}) -> Delivery`; `Moved {seq, card, from, to, note, at}` (moves and accepts), `Reestimated {seq, card, old, new, reason, at}` (`new` `None` on a clear), `CardItemRef {id, card, kind, status}`; `unix_days`, a date's or a time stamp's day, the inverse of `date_from_unix_days`. Events go by `seq`, never `at`; reasons are grouped for the late cards alone. Core `roadmap.rs` `item_groups(&CheckInput, &RoadmapConfig) -> BTreeMap<ID, GroupRef {kind, name, order}>`, `order` the group's index in `Plan::groups`; `plan` unchanged.

## Reads

CLI `delivery.rs`: `delivery(env, globals, &DeliveryRequest {as_of}) -> DeliveryOutcome {delivery, shown, truncated}` (`omitted()`), `delivery_with_view(.., View)`, `delivery_as_of` (the date check, the daemon's too). One call, in order: `--as-of` judged, before discovery; the config's `[roadmap]`, `[card]` (the return columns) and `[[workspace]]`, each apart, one that does not read a note; with a `[roadmap]` that reads, the plan's groups from the walked files as `spec roadmap` builds its input (`check_input`, no index); then the queue once, `SqliteQueue::open_existing` (read-only, no schema step): `list_existing_cards`, then, cards found, one uncapped `events_of_types([card.moved, card.accepted, card.reestimated])` (never the 64-capped `card_history`) and one `list_readable` of every status and repository, kept to questions, discrepancies and decisions naming a card. No git, index, lock or write. Without `--as-of`, the store's `today_utc()`; given, no clock is read: one queue, config and file state give byte-identical text and JSON at any root or `HOME`.

A move missing `id`, `from` or `to`, or a re-estimate missing `old`, `new` or `reason`, is passed over; a note that is no text reads as none; a `due` that is no date counts as none (the store judges it at write).

**Seams** (`subset.rs`, DA-10): `DeliveryCards`, `DeliveryEvents`, `DeliveryItems`, `DeliveryPlan`, 1 each a call; no database, none; a queue with no card, cards 1, events and items 0; no `[roadmap]`, no plan build.

## Text and JSON

JSON, every key present, absent `null`: `{as_of, notes, releases, departments, total, late, truncated, omitted}`. A release row `{group, release, tasks, delivered, delivered_late, open, overdue, cancelled, lead_days}`, `group` `release`, `pool`, `unplaced` or `none`, `release` the name for `release` alone; a department row `{name, title}` and the counts; `total` the counts. A late row: `{id, title, flow, column, parent, group, release, departments, created_at, taken_at, accepted_at, due, days_late, lead_days, estimate, returns [{at, from, note}], reestimates [{at, old, new, reason}], cancelled_subtasks [{id, title, at, reason}], items [{id, kind, status}], more {returns, reestimates, cancelled_subtasks, items}}`, `departments` names. `omitted` the late rows cut, `null` uncut. The scenario (`docs/features/delivery-analytics.md` "Data"), its pinned document and text: CLI `tests/delivery_scene/data.rs`. Some of its lines:

```text
delivery | as of 2026-11-20 | tasks 4, delivered 2, late 1, open 2, overdue 1, cancelled 1 | lead median 3d, max 14d
late C-0001 | copy/done | r1 | due 2026-11-10 | accepted 2026-11-15T09:00:00Z | late 5d | Write the tide table foreword
  returned 2026-11-06T09:00:00Z | from review | Too long
```

A group line `release <name>`, `pool`, `unplaced` or `no release`, `late` the delivered late; a late row's release the name, `pool`, `unplaced` or `-`; a clear `3 -> -`; past a cap, `  more returns <a>, re-estimates <b>, cancelled subtasks <c>, items <d>`, the non-zero ones. Values one-lined, lines escaped, JSON raw. The cut at `OUTPUT_CAP_CHARS`: the header, group and department lines whole, then late rows whole while they fit, then `[truncated: <k> of <n> late tasks not shown]`; the JSON holds the rows printed.

## Notes and exits

`note: <n>` on stderr, escaped; JSON `notes` raw; one line each, sorted, each once; none changes the exit.

- `delivery: no [roadmap] table: every task is in no release`; `delivery: [roadmap] does not read: <config>:<line>: <message>; every task is in no release` (`<config>: <message>` with no line)
- `delivery: plan <path> does not read (<reason>); its item places no task`, a file under `[roadmap] dir`: its read error, `not parsed`, or its front matter's `line <n>: <message>`
- `delivery: plan <path> does not read (<reason>); its items place no task`, for that directory, a place in it or a parent of it (`.` the root): `the directory cannot be listed`, `the [paths] roots entry names no directory and no .md file` (written roots), `a name in it is not UTF-8`; other walk problems are `spec check`'s
- `delivery: [card] does not read: no return is counted`; a workspaces note as `spec workspace` prints it (escaped in JSON too)
- `delivery: no queue yet: no card read`, no database file
- `<C>: its row does not read (<column>: <reason>); not counted`, a card row; `<PR>: its row does not read (<column>: <reason>); not listed`, an item row, read only with cards
- `<C>: <value> is no timestamp; no lead time`, a delivered card's `created_at` or `acceptance.at` as stored

Exit 0 answered; 2: a bad `--as-of` (`` spec: `--as-of`: `<v>` is no calendar date, YYYY-MM-DD ``, nothing read), no project, a config discovery refuses, a queue that cannot be read (a newer schema). No `[roadmap]` is no error.

## Daemon, hub

`GET /api/projects/:p/delivery`, query `as_of`, optional: `spec delivery [--as-of D] --json`, `View::Browser` (nothing cut, `truncated` false, `omitted` `null`), `run_read`, in the project's turn as `roadmap`; `as_of` passed through, the library's default without it. 400s, nothing run: `` `as_of`: `<v>` is no calendar date, YYYY-MM-DD ``; `` `<name>` is no query name of delivery: it takes as_of ``. The 404's route list names `delivery` between `events` and `workspaces`. Key sets (`fixtures/daemon-keys.json`, 137): `Delivery` 8, `DeliveryRelease` 9, `DeliveryDepartment` 9, `DeliveryTotal` 7, `LeadDays` 2, `DeliveryLate` 20, `DeliveryReturn` 3, `DeliveryReestimate` 4, `DeliveryCancelled` 4, `DeliveryItem` 3, `DeliveryMore` 4.

A hub serves the same route to any login (`tests/hub.rs` `GET_ROUTES`, 29), rolled up by department, never by person, over its own queue and checkout from its next upgrade; an older daemon answers 404. No MCP tool (no result holds a date, so it would need a required `as_of`), plugin version or `HUB_TOOLS` change; no schema step, backup format or config key.

## Screen

The Roadmap's third tab, Delivery, lazy, shown once the plan reads; it sends the viewer's own day as `as_of` and computes no count: `spec-roadmap-screen.md` "Delivery".

## Known limits

- **No tab without a plan**: with no `[roadmap]` the Roadmap screen shows its error, so Delivery is never reached, though `spec delivery` answers (every task in no release).
- **A hub's default day**: without `as_of` the route judges at the server's UTC day (`today_utc()` in the daemon), while the UI always sends the viewer's own day.
- **`as_of` judges open cards only**: a past day never rewinds the queue, so a card accepted after it still counts delivered.
- **The lower middle**: of an even count of lead times the median is the lower of the two middles.
- **No shared snapshot**: cards, events and items are three read transactions; a write between them can show a card without its latest event or item, until the next read.
- **An accept into the return column** would count as a return; `[card]` forbids such an accept, so none is stored.
- **Plan notes only around `[roadmap] dir`**: a file outside it, or a place neither it, in it nor its parent, that does not read is the check's finding alone; an item there places no task without a note.
- **The hub's queue alone**: a hub counts its own cards, never a terminal's.
- **Cost**, per read, uncapped, every card and event of the three types in memory: 20 000 cards, 200 000 events, 2 000 items, a 100-item plan, release, warm: `spec delivery --json` median 292 ms (the queue half 212), peak resident 246.6 MB (`/usr/bin/time -l`). Past 1 s, an `events(project, type)` index, a queue schema step.

## Tests

Core `tests/delivery.rs` (the judge, `unix_days` round-tripped 1970 to 2100); CLI `tests/delivery.rs`, `src/delivery_tests.rs` (one read per call), `tests/delivery_scene/` (the scenario and its pinned JSON and text, shared with http); http `tests/delivery.rs`, `daemon_keys.rs`; eval `tests/delivery_scale.rs` (the measure ignored); UI: `spec-roadmap-screen.md` "Delivery". Criteria: `docs/features/delivery-analytics.md`.
