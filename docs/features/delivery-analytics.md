---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0107 part a, v5 core slice 6 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 3, DA-1 to DA-7); the orchestrator's working answers, the owner's review open
adrs: []
---

# delivery-analytics: what was delivered, what is overdue and why, per release and per department (RM-0107 a)

## Why

The owner's vision of 2026-11-11 asks for a roadmap of delivered features: where we are, how much is left, what took longer and why (`08-roadmap.md` Phase 7). The Roadmap screen shows lanes of plan items but no dates. The queue holds every fact (a card's `created_at`, `due`, `estimate`, `acceptance.at`, `cancellation`, and its `card.moved` and `card.reestimated` events with their notes and reasons, `docs/canon/cards.md` "Store", "Documents and events"), yet nothing totals them, and a card read stops at its last 64 events.

After this slice `spec delivery [--as-of D] [--json]`, the daemon's `GET …/delivery` and a lazy Delivery tab on the Roadmap screen show, per release and per department, how many tasks were delivered out of how many, how many are open, overdue or delivered late, and the lead time; then every late task with its reasons: returns with their notes, re-estimates old to new with their reasons, cancelled subtasks with theirs, and the questions raised on it. Computed per read from the queue, the config and the plan, never stored, and never written into `docs/generated/roadmap.md`, which is built from git alone (ADR-0052). It moves, refuses and gates nothing (ADR-0012). No ADR: it rests on ADR-0052, ADR-0056, ADR-0074, ADR-0076 and ADR-0012. Part b, the timeline (created, taken and accepted per task, with due dates and release markers), is a later slice, core slice 13.

## Description and interactions

### Decisions

- **D1. A card's release** is the group of the first plan item, in plan order, among its `targets` (`docs/canon/spec-roadmap.md` "Position": a task executes the item it targets), else its parent's, walking up, else `none`. Plan order is the group order: releases in `[roadmap] releases`' order, then the pool, then the unplaced items; done items count too. No card field (Q1).
- **D2. A department** is every served workspace whose `flows` lists the card's flow, `null` listing every flow (`docs/canon/workspaces.md` "Config"). A flow two departments list counts in both; the total counts each card once. A card in no department counts in the total alone. Rows go by release and by department, never by person: no served key names an assignee, reviewer or actor.
- **D3. Taken** is the card's first `card.moved` or `card.accepted` whose `from` differs from `to`: the first move out of the column it was created in, a reorder never one. **Returned** is a `card.moved` into its flow's `return` column (`docs/canon/card-flows.md` "Return"), from another column, with its note.
- **D4. Overdue** is an open card whose `due` is before `as_of`; **delivered late** an accepted card whose `acceptance.at` date is after its `due`. Days are UTC calendar dates. The word is "overdue", never "blocked" (ADR-0056). `as_of` judges open cards only: it never rewinds the queue.
- **D5. `as_of` is an input**: `--as-of YYYY-MM-DD`, the daemon's `as_of`, default the store's `today_utc()`, always printed and served. The UI always sends the viewer's own calendar day, as the Board judges its overdue badge (`ui/src/board/due.ts` `useToday`), so the two agree.
- **D6. Estimates have no unit** and are never summed, averaged or compared (`cards.md` "Store", Fields): a late task shows its estimate and each re-estimate as written. Lateness comes from `due` alone (Q2).
- **D7. One uncapped events read per call**, store `events_of_types` (RM-0105's, `docs/features/upstream-signal.md` D5), never `card_history`, which caps at 64. If `upstream-signal` has not landed when this slice starts, this slice adds it with that signature.
- **D8. No MCP tool and no plugin bump** (Q4): a tool would need a required `as_of`, since no MCP result holds a date (`docs/canon/mcp-read.md` "Tools", Parity).

"Data" fixes every name, so the Rust and UI lanes never wait on each other.

## Data

### Core, store, CLI library

Core, `crates/specengine-core/src/delivery.rs`, pure, no clock, I/O or queue:

```rust
pub const REASONS_MAX: usize = 16;                   // per list of a late task, the oldest kept
pub fn unix_days(date: &str) -> Option<i64>;         // "YYYY-MM-DD" or its timestamp's date part; inverse of date_from_unix_days
pub struct DeliveryInput<'a> {
    pub as_of: &'a str,                              // is_calendar_date, judged by the caller
    pub cards: &'a [Card],                           // every readable row, any state
    pub moves: &'a [Moved<'a>],                      // card.moved and card.accepted, by seq
    pub reestimates: &'a [Reestimated<'a>],          // card.reestimated, by seq
    pub items: &'a [CardItemRef<'a>],                // question, discrepancy, decision rows with a card_id
    pub groups: &'a BTreeMap<String, GroupRef>,      // roadmap::item_groups
    pub departments: &'a [Workspace],                // the served workspaces, served order
    pub returns: &'a BTreeMap<String, String>,       // flow -> its return column
}
pub struct Moved<'a> { pub seq: i64, pub card: &'a str, pub from: &'a str, pub to: &'a str, pub note: Option<&'a str>, pub at: &'a str }
pub struct Reestimated<'a> { pub seq: i64, pub card: &'a str, pub old: Option<&'a str>, pub new: Option<&'a str>, pub reason: &'a str, pub at: &'a str }
pub struct CardItemRef<'a> { pub id: &'a str, pub card: &'a str, pub kind: &'a str, pub status: &'a str }
pub fn delivery(input: &DeliveryInput<'_>) -> Delivery;    // the JSON's structs, plus notes
```

Core `roadmap.rs` gains `item_groups(&CheckInput, &RoadmapConfig) -> BTreeMap<String, GroupRef>`: every item, open or done, by `plan`'s steps 1 and 3; `GroupRef {kind: GroupKind, name: Option<String>, order: usize}`, `order` the group's index in `Plan::groups`. `plan` is unchanged.

The judge:
1. **Release**: the card's targets that `groups` holds, the least `order` winning; none, its parent's release, up the chain, a repeat or an unreadable parent ending it at `none`.
2. **Departments**: `departments` whose `flows` is `None` or holds `card.flow`, in served order.
3. **Taken**: the first `Moved` of the card with `from != to`; **returns** its `Moved` with `from != to` and `to` its flow's return column; **re-estimates** its `Reestimated`; **cancelled subtasks** the cards whose `parent` is it and `cancellation` is set, by ID; **items** its `CardItemRef`, by ID. Each list keeps its oldest `REASONS_MAX`, the rest counted in `more`.
4. **Counts** of a row: `tasks` its cards not cancelled; `delivered` those accepted; `delivered_late` those accepted on a date after `due`; `open` neither accepted nor cancelled; `overdue` open with `due < as_of`; `cancelled`. `lead_days` over its delivered cards: `unix_days(acceptance.at) - unix_days(created_at)`; `median` the lower of the two middles, `max`; `null` with none delivered.
5. **Rows**: release rows for each group holding a card (cancelled ones too), in group order, then `none`; department rows in served order, every one, zeros too; `total` over every card once.
6. **Late**: every card delivered late or overdue; `days_late` `unix_days(acceptance.at) - unix_days(due)` or `unix_days(as_of) - unix_days(due)`; by `days_late` descending, then ID number.

Store: `events_of_types(&[EVENT_CARD_MOVED, EVENT_CARD_ACCEPTED, EVENT_CARD_REESTIMATED])` (D7); `list_existing_cards` (read-only, no schema step, as `spec roadmap` reads tasks); one `list_readable(&ProposalFilter {git_common_dir: None, statuses: vec![]})`, kept to question, discrepancy and decision rows with a `card_id`. A payload missing a field is passed over.

CLI library, new `crates/specengine-cli/src/delivery.rs`: `DeliveryRequest {as_of: Option<String>}`, `delivery(env, globals, request) -> Result<DeliveryOutcome, CliError>` and `delivery_with_view(…, View)`. One call: the config (`[roadmap]`, `[card]`, `[[workspace]]`, each read apart, a bad one a note), the plan's input as `spec roadmap` builds it (`roadmap.rs`, the walked files, no index), then the queue once (`SqliteQueue::open_existing`). No git, no index, no write.

### JSON (`--json`, the daemon's route)

**The scenario**, the tests' corpus too, `as_of` 2026-11-20. Config: flows `copy` (columns `todo`, `doing`, `review`, `returned`, `done`; accept `review` to `done`; return `returned`), `plates` (`todo`, `drawing`, `done`), `sound`; departments `words` "Words" (`copy`) and `print` "Print" (`copy`, `plates`). Plan items `RM-0001` (`release: r1`) and `RM-0002` (`release: r2`), `[roadmap] releases = ["r1", "r2"]`. Cards, every time `T09:00:00Z`:
- C-0001 "Write the tide table foreword", `copy`, targets `RM-0001`, assignee `Ann`, reviewer `Bob`, made 11-01, due 11-10, estimate 3; two reorders, then moved to `doing` 11-03; returned from `review` into `returned` 11-06 "Too long" and 11-08 "Wrong tone"; re-estimated 3 to 5 on 11-07, "The tables doubled"; accepted 11-15. PR-0040, a question with `card_id` C-0001, applied.
- C-0002 "Draw the tide plates", `plates`, its parent C-0001, no target, cancelled 11-09, "Plates reused from last year".
- C-0003 "Engrave the harbor map", `plates`, targets `RM-0002`, made 11-04, due 11-12, moved to `drawing` 11-05, open.
- C-0004 "Fix the index", `copy`, no target, made 11-02, accepted 11-05.
- C-0005 "Record the foghorn", `sound`, no target, open.

```json
{"as_of": "2026-11-20", "notes": [],
 "releases": [
  {"group": "release", "release": "r1", "tasks": 1, "delivered": 1, "delivered_late": 1, "open": 0, "overdue": 0, "cancelled": 1, "lead_days": {"median": 14, "max": 14}},
  {"group": "release", "release": "r2", "tasks": 1, "delivered": 0, "delivered_late": 0, "open": 1, "overdue": 1, "cancelled": 0, "lead_days": null},
  {"group": "none", "release": null, "tasks": 2, "delivered": 1, "delivered_late": 0, "open": 1, "overdue": 0, "cancelled": 0, "lead_days": {"median": 3, "max": 3}}],
 "departments": [
  {"name": "words", "title": "Words", "tasks": 2, "delivered": 2, "delivered_late": 1, "open": 0, "overdue": 0, "cancelled": 0, "lead_days": {"median": 3, "max": 14}},
  {"name": "print", "title": "Print", "tasks": 3, "delivered": 2, "delivered_late": 1, "open": 1, "overdue": 1, "cancelled": 1, "lead_days": {"median": 3, "max": 14}}],
 "total": {"tasks": 4, "delivered": 2, "delivered_late": 1, "open": 2, "overdue": 1, "cancelled": 1, "lead_days": {"median": 3, "max": 14}},
 "late": [
  {"id": "C-0003", "title": "Engrave the harbor map", "flow": "plates", "column": "drawing", "parent": null,
   "group": "release", "release": "r2", "departments": ["print"],
   "created_at": "2026-11-04T09:00:00Z", "taken_at": "2026-11-05T09:00:00Z", "accepted_at": null,
   "due": "2026-11-12", "days_late": 8, "lead_days": null, "estimate": null,
   "returns": [], "reestimates": [], "cancelled_subtasks": [], "items": [],
   "more": {"returns": 0, "reestimates": 0, "cancelled_subtasks": 0, "items": 0}},
  {"id": "C-0001", "title": "Write the tide table foreword", "flow": "copy", "column": "done", "parent": null,
   "group": "release", "release": "r1", "departments": ["words", "print"],
   "created_at": "2026-11-01T09:00:00Z", "taken_at": "2026-11-03T09:00:00Z", "accepted_at": "2026-11-15T09:00:00Z",
   "due": "2026-11-10", "days_late": 5, "lead_days": 14, "estimate": "5",
   "returns": [{"at": "2026-11-06T09:00:00Z", "from": "review", "note": "Too long"},
               {"at": "2026-11-08T09:00:00Z", "from": "review", "note": "Wrong tone"}],
   "reestimates": [{"at": "2026-11-07T09:00:00Z", "old": "3", "new": "5", "reason": "The tables doubled"}],
   "cancelled_subtasks": [{"id": "C-0002", "title": "Draw the tide plates", "at": "2026-11-09T09:00:00Z", "reason": "Plates reused from last year"}],
   "items": [{"id": "PR-0040", "kind": "question", "status": "applied"}],
   "more": {"returns": 0, "reestimates": 0, "cancelled_subtasks": 0, "items": 0}}],
 "truncated": false, "omitted": null}
```

Every key present, absent `null`. `group` `release`, `pool`, `unplaced` or `none`, `release` the name for `release` alone. A department row is `{name, title}` and the counts, `total` the counts alone. A late row's `estimate` is the card's as written; a re-estimate's `new` is `null` on a clear. `omitted` counts the late rows cut, `null` uncut. The daemon (`View::Browser`) never cuts.

### Text

The scenario's `spec delivery --as-of 2026-11-20`:

```text
delivery | as of 2026-11-20 | tasks 4, delivered 2, late 1, open 2, overdue 1, cancelled 1 | lead median 3d, max 14d
release r1 | tasks 1, delivered 1, late 1, open 0, overdue 0, cancelled 1 | lead median 14d, max 14d
release r2 | tasks 1, delivered 0, late 0, open 1, overdue 1, cancelled 0 | lead -
no release | tasks 2, delivered 1, late 0, open 1, overdue 0, cancelled 0 | lead median 3d, max 3d
department words | Words | tasks 2, delivered 2, late 1, open 0, overdue 0, cancelled 0 | lead median 3d, max 14d
department print | Print | tasks 3, delivered 2, late 1, open 1, overdue 1, cancelled 1 | lead median 3d, max 14d
late C-0003 | plates/drawing | r2 | due 2026-11-12 | open | overdue 8d | Engrave the harbor map
late C-0001 | copy/done | r1 | due 2026-11-10 | accepted 2026-11-15T09:00:00Z | late 5d | Write the tide table foreword
  returned 2026-11-06T09:00:00Z | from review | Too long
  returned 2026-11-08T09:00:00Z | from review | Wrong tone
  re-estimated 2026-11-07T09:00:00Z | 3 -> 5 | The tables doubled
  subtask C-0002 cancelled 2026-11-09T09:00:00Z | Plates reused from last year
  item PR-0040 | question | applied
```

- A group line: `release <name>`, `pool`, `unplaced` or `no release`; `late` in the counts is `delivered_late`. A late row's release is the name, `pool`, `unplaced` or `-`; a re-estimate to a clear prints `3 -> -`; past the cap, `  more returns <a>, re-estimates <b>, cancelled subtasks <c>, items <d>`, only the non-zero ones.
- Values pass `one_line`. The cut at `OUTPUT_CAP_CHARS`: the header, group and department lines whole, then late rows whole while they fit; then `[truncated: <k> of <n> late tasks not shown]`.
- Notes: `note: <n>` on stderr, JSON `notes`, sorted, each once:
  - `delivery: no [roadmap] table: every task is in no release`; `delivery: [roadmap] does not read: <config>:<line>: <message>; every task is in no release`;
  - `delivery: [card] does not read: no return is counted`; a workspaces note as `spec workspace` prints it;
  - `delivery: no queue yet: no card read`; `<C>: its row does not read (<column>: <reason>); not counted`; `<C>: <created_at or acceptance.at> is no timestamp; no lead time`.
- Exit: 0 answered, a note included; 2 on a bad `--as-of` (`` `--as-of`: `<v>` is no calendar date, YYYY-MM-DD ``), no project, or a queue that cannot be read (a newer schema). No `[roadmap]` is no error.

### Daemon, hub

- **Route**: `GET /api/projects/:p/delivery`, query `as_of` (optional); `spec delivery --json`, browser view, in the project's turn as `roadmap`. A bad `as_of` is a 400 naming it, nothing run; any other parameter the 400 listing `as_of`. `tests/door.rs` and `no_domain.rs` hold.
- **Keys**, `fixtures/daemon-keys.json`, 11 new sets in "JSON"'s order: `Delivery` (8), `DeliveryRelease` (9), `DeliveryDepartment` (9), `DeliveryTotal` (7), `LeadDays` (2), `DeliveryLate` (20), `DeliveryReturn` (3), `DeliveryReestimate` (4), `DeliveryCancelled` (4), `DeliveryItem` (3), `DeliveryMore` (4).
- **The hub** serves the same route over its own queue and checkout. No MCP tool, `HUB_TOOLS` and the plugin version unchanged.

### The UI line

- **Types** (`provisional.ts`, citing DA-11): `DeliveryDocument`, `DeliveryCounts`, `DeliveryRelease`, `DeliveryDepartment`, `LeadDays`, `DeliveryLate`, `DeliveryReturn`, `DeliveryReestimate`, `DeliveryCancelled`, `DeliveryItem`, `DeliveryMore`, the sets above; `group` `KnownDeliveryGroup | Unlisted`. `client.ts` `getDelivery(project, {asOf}, signal)`, `http.ts` `?as_of=`; the mock (`src/mocks/delivery.ts`): harbor-sim's cards in every shape (each group, an unknown group, a department of zeros, an overdue and a delivered-late task, every reason, `more` above 0, a re-estimate to a clear), and an empty project.
- **The read** (`queries.ts`): `useDelivery(project, asOf)`, key `["delivery", p, asOf]`; `deliveryOf(p)` re-read on every `card.*` event, every `proposal.*` but a stage, and a gap, a read in flight aborted.
- **The tab**: `RoadmapTab` gains `"delivery"`, third after Lanes and Chain, remembered per project as they are. `RoadmapView.tsx` holds `const RoadmapDelivery = lazy(() => import("./RoadmapDelivery")…)` in a `Suspense` with a skeleton, as `RoadmapChain` is; `lazy.test.ts` pins it.
- **The view** (`src/roadmap/RoadmapDelivery.tsx`; labels and the one compare of a `group` word in `src/roadmap/delivery.ts`):
  - an "As of" date field (`.field`, `<input type="date">`), its value the served `as_of`, starting at `useToday()`; a change re-reads; a subtle `Button` "Today" returns to the viewer's day;
  - two `<table>`s with captions, By release and By department, a `<th scope="row">` per row: Release (the name, or Pool, Unplaced, No release; an unknown group raw, `lang="en"`) or Department (its title in a `bdi`), Delivered (`{{delivered}} of {{tasks}}`), Open, Overdue, Delivered late, Cancelled, Lead time (`median {{median}}, max {{max}} days`, else "None delivered"); By department ends in a `<tfoot>` row "All tasks", the served `total`;
  - Late tasks, an `ol` in served order: each the card's ID linking `sectionHash(p, "board", id)`, its title, "Overdue by N days" or "Delivered N days late", a `PropertyList` (Due, Created, Taken, Accepted, Estimate as written, Departments by title), then its reasons: each return, re-estimate, cancelled subtask and item (the item linking `sectionHash(p, "inbox", id)`, its kind's word, its status raw `lang="en"`), and a "more" line per list above 0; none late, "No task is late.";
  - loading a skeleton, a failure `ErrorPanel` with the daemon's words and Retry, no card an `EmptyState` (level 3), `notes` a note list `lang="en"`.
- **Strings** (`en.ts`, `ru.ts`, the Russian a draft with plural forms), under `roadmap.delivery`: `tab`, `loading`, `failed`, `asOf`, `asOfHint`, `today`, `byRelease`, `byDepartment`, `column.{release,department,delivered,open,overdue,deliveredLate,cancelled,lead}`, `deliveredOf`, `lead`, `noLead`, `group.{pool,unplaced,none}`, `total`, `late`, `noLate`, `overdueBy_one|_other`, `lateBy_one|_other`, `property.{due,created,taken,accepted,estimate,departments}`, `returned`, `reestimated`, `estimateCleared`, `subtaskCancelled`, `item.{question,discrepancy,decision}`, `more_one|_other`, `empty`, `emptyText`, `notes`. No leaf says "blocked", in either language.
- **CSS**: scale-step tokens only (`--space-025`..`--space-600`, `--font-size-<step>`, `--line-height-<step>`, `--radius-*`), no legacy alias, no `SCALE_EXCEPTIONS` entry; tables in `app.css` under `.delivery-*`.

## Rules and edge cases

- WHEN `--as-of` is given, the read SHALL read no clock; one queue, config and file state with one `as_of` SHALL give one text and JSON, byte-identical at other roots and `HOME`s. `at` values are stored data. Order is by `seq`, never by `at`.
- WHEN a card is accepted from the column it was created in, the accept SHALL be its take. WHEN a card has moved only by reorders, `taken_at` SHALL be `null`.
- WHEN `as_of` equals `due`, the card SHALL NOT be overdue. WHEN a card has no `due`, it SHALL be neither overdue nor late. A cancelled card SHALL be neither, and counts only as `cancelled`.
- WHEN a target names a done item, that item's group SHALL count, as an open one's does. WHEN a card names items of two groups, the first in plan order SHALL win.
- WHEN no workspace is configured, the implicit `all` SHALL be the one department, listing every card.
- Re-estimates and estimates SHALL NOT be summed, averaged or compared; no key SHALL total one. No key SHALL name a person.
- A card's flow no longer configured: no return column, so no returns; its departments those whose `flows` is `null`.
- Writes: none. The read opens the queue read-only, takes no lock and refreshes no index.

**Rollout.** No schema step, backup format, config key or tool. A daemon or hub before this slice answers the route 404. The hub takes it at its next upgrade, the owner's call.

**Test churn.** The daemon's 404 route listing and its pins; `daemon_keys.rs` and `daemonKeys.test.ts` (11 sets); the CLI usage golden if it lists commands; the Roadmap screen's tab tests (three tabs).

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Rows and events are written through the store with fixed `now`s, as its tests do; corpora are built at run time with invented names; "the scenario" is "Data"'s.

- [ ] DA-1 — **Uncapped.** C-0001 reordered 70 times in `todo`, then moved to `doing`: `taken_at` is that move's. Mutation, red: the events read through `card_history(…, 64)`.
- [ ] DA-2 — **A reorder is no take.** C-0001's two reorders before 11-03: `taken_at` 2026-11-03T09:00:00Z; a card accepted straight from its first column: the accept's `at`; a card only reordered: `null`. Mutation, red: the first `card.moved` of any kind.
- [ ] DA-3 — **Release.** C-0002 (no target) counts under r1 through its parent; a card targeting `RM-0002` and `RM-0001` is r1; one targeting a done item of r2 is r2; a pool item `pool`; a release not listed `unplaced`; no item up the chain `none`. Mutations, each red: the parent fallback dropped; the last target's group; done items ignored.
- [ ] DA-4 — **Departments.** On the scenario `words` counts C-0001 and C-0004, `print` C-0001 to C-0004, `total` `tasks` 4 counts C-0001 once, and C-0005 (`sound`) only in `total`; with no `[[workspace]]`, one row `all` equal to `total`. Mutation, red: `total` as the sum of the departments.
- [ ] DA-5 — **`as_of`, no clock.** C-0003 (due 11-12) is overdue at 2026-11-20 (`days_late` 8), not at 2026-11-01 nor at 2026-11-12; C-0001 is delivered late by 5 whatever `as_of`. The same queue with `--as-of 2026-11-20` is byte-identical twice and at another root and `HOME`. Without `--as-of`, `as_of` equals `today_utc()` read before or after the run. Mutations, each red: the clock read when `--as-of` is given; `<=` for `<`.
- [ ] DA-6 — **No sum, no person.** A walk of the scenario's JSON finds no key holding `estimate` but a late row's `estimate`, and none named `as`, `by`, `assignee`, `reviewer`, `created_by` or `decided_by`; no line of the text holds `Ann` or `Bob`. Mutations, each red: an `estimate_sum` key; `assignee` on a late row.
- [ ] DA-7 — **Lazy.** After `pnpm build`, the Delivery chunk is not among first load's files; first-load JS grows at most 3 kB, ≤ 845 kB. Mutation, red: a static import of `RoadmapDelivery` in `RoadmapView.tsx` (`lazy.test.ts`).
- [ ] DA-8 — **Reasons.** On the scenario C-0001 lists the two returns with their notes and `from`, the re-estimate 3 to 5 with its reason, C-0002 cancelled with its reason, PR-0040; a reorder within `returned` is no return. Twenty returns list the oldest 16, `more.returns` 4, the text ending `  more returns 4`. Mutations, each red: any move with a note taken as a return; the lists uncapped.
- [ ] DA-9 — **Text and JSON as pinned.** `spec delivery --as-of 2026-11-20` prints "Text"'s lines byte for byte, `--json` "JSON"'s document; lead days median over 3 and 14 is 3. No `[roadmap]`: every row `none`, the note, exit 0. A card row made unreadable in SQL: its note, not counted, exit 0. A bad `--as-of`: exit 2, nothing read. Mutation, red: the upper middle as the median.
- [ ] DA-10 — **[measure] One read per call.** A test seam counts `list_existing_cards`, `events_of_types` and `list_readable` calls and plan builds: 1 each over the scenario and over 200 cards. Seeded through the store (`crates/specengine-eval/tests/delivery_scale.rs`, `#[ignore]` above 1 000): 20 000 cards with 10 events each, 2 000 items, a 100-item plan; `spec delivery --json`, warm, release, ≤ 1 s, the queue half timed alone too; times in "Implementation". Past the bar, "Known limits" names an `events(project, type)` index, a queue schema step. Mutation, red: `card_items` per card.
- [ ] DA-11 — **The daemon serves it, the UI types it.** `GET …/delivery?as_of=2026-11-20` is byte-equal to `spec delivery --as-of 2026-11-20 --json`; `as_of=2026-13-01` and an unknown parameter are 400s naming them. The fixture holds the 11 sets; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `as_of` not passed through; a set left out of the fixture.
- [ ] DA-12 — **The tab**, on the mock. Delivery is the third tab, remembered per project; it sends `as_of` the viewer's day; a new date re-reads with it. By release shows "1 of 1" for r1, "Pool", "No release" and an unknown group raw; By department ends in "All tasks"; the late list shows each reason, a "more" line, the item linking the Inbox, the card the Board; an empty project the `EmptyState`. No `roadmap.delivery` leaf in `en.ts` or `ru.ts` holds `block` (`\u` escapes for Russian); `policy.test.ts`, `scale.test.ts` and `words.test.ts` pass, the slice's CSS naming no legacy alias (`git diff` grep). Mutations, each red: `group` compared outside `delivery.ts`; a key missing in `ru.ts`; `as_of` not sent.
- [ ] DA-13 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: the mock's tab, the two tables, a late task with every reason, the date field moved past a due date, the empty project.
- [ ] DA-14 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Part b, the timeline (core slice 13); RM-0087's states; RM-0086's department overview; an MCP tool; a card `release` field, an estimate unit, configured release dates (Q1-Q3); summing or comparing estimates; roll-ups by person; time travel (`as_of` rewinding the queue); `T-` tasks; a stored, cached or committed figure; the Delivery tab without a `[roadmap]` table.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{delivery,roadmap,lib}.rs`; `crates/specengine-store/src/queue/cards.rs` (only if `events_of_types` has not landed); `crates/specengine-cli/src/{main,lib}.rs` and a new `delivery.rs`; `crates/specengine-http/src/{app,args}.rs` |
| test-engineer, Rust | `delivery.rs` in core `tests/`, CLI and http (new); DA-10's seam; `crates/specengine-eval/tests/delivery_scale.rs`; `daemon_keys.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`) with, in the same change, `daemonKeys.test.ts`'s 11 sets, names and count, the delivery entries out of `AHEAD_OF_FIXTURE`; the churn |
| ui-developer | "The UI line": `provisional.ts`, `client.ts`, `http.ts`, the mock, `queries.ts`, `src/roadmap/{RoadmapView,RoadmapDelivery}.tsx`, `src/roadmap/delivery.ts`, `src/app/lazy.test.ts`, `src/styles/app.css`, `en.ts`, `ru.ts`. The 11 sets in `AHEAD_OF_FIXTURE` until the fixture lands |
| test-engineer, UI | the UI gates, DA-7's figure, DA-12's mutations |

Comments cite `docs/features/delivery-analytics.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after `pack` ships (one Rust slice at a time), `upstream-signal`'s `events_of_types` in the tree.
2. The ui-developer lands the UI line behind `AHEAD_OF_FIXTURE`; the rust-developer goes core, CLI, http.
3. The Rust test-engineer overlaps: core (DA-1 to DA-6 at the judge), CLI (DA-8, DA-9), DA-10, http (DA-11). Then the Rust and UI reviews.
4. The orchestrator's look (DA-13). Ship: the spec-writer ("Canon at ship").

At most three iterations a lane.

### Risks

- **Day arithmetic**: `unix_days` must agree with `date_from_unix_days` over leap years; a round-trip test from 1970 to 2100 guards it.
- **First load**: about 40 English leaves; DA-7's 3 kB is the bar. Past it, the strings move into the lazy chunk's own catalog part (`ui-languages.md`).
- **Overlap**: RM-0086 counts per department later; it reads this slice's rows rather than counting again.

### Canon at ship

Free bytes now in brackets; "net" means trimmed in the same file to fit.

| File | Edit |
|---|---|
| `docs/canon/spec-delivery.md` (new, Tier 2, `spec-*` shard) | the rule: release, departments, taken, returns, overdue, late, lead; `as_of`; JSON and text; daemon, hub; the tab; "Known limits" (no tab without a plan, the hub's queue alone, `as_of` judges open cards only, the measured cost); "Tests" |
| `docs/canon/spec-roadmap-screen.md` [9 161] | "Layout": the Delivery tab, lazy; "Rules": `group` words only in `delivery.ts` |
| `docs/canon/spec-roadmap.md` [31], `docs/canon/cards.md` [73] | "Surfaces": a pointer; `due` feeds overdue. Net |
| `docs/canon/ui-languages.md` [9] | "Size": the first-load figure. Net |
| READMEs: core [9], cli [1], http [4] | a pointer; CLI "Determinism": `delivery` one date, as `check`; http's route row. Net |
| `CLAUDE.md` [2 552]; 08 "Owner's word, 2026-11-12" row 6; RM-0107 | shipped, part a |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q1. What is a card's release? | The first plan item, in plan order, it or an ancestor targets | A card field: queue schema 13, backup format 7, an ADR, older binaries refusing the queue, a hub upgrade, 2-3 hours |
| Q2. Do card estimates get a unit? | No: lateness from `due` and re-estimates, estimates shown as written | A `[card]` key older binaries refuse; 1-2 hours |
| Q3. Configured release dates? | No: part b derives markers (first take, last accept) | A `[roadmap]` key older `spec roadmap` refuses |
| Q4. An MCP tool? | No | A required `as_of` and a plugin bump |
| Q5. A flow in two departments? | Counts in both; the total once | Splitting a card's weight invents a fraction no one reads |
| Q6. What is "taken", what is "late"? | The first move out of its first column, reorders ignored; accepted after `due`, or open past it at `as_of` | "Taken" by assignment counts cards assigned at creation as taken |

**The spec-writer's:**
- W1: every card counts, a subtask too: its department's work is its department's delivery, and DA-3 places it in its parent's release.
- W2: `tasks` leaves cancelled cards out, so "delivered N of M" is `delivered` of `tasks`; `cancelled` stands beside it.
- W3: the UI sends the viewer's day, so Delivery and the Board's overdue badge agree near midnight; the CLI's default is UTC.
- W4: release rows list only groups holding a card; department rows list every department, zeros too, since a department is the reader's lens.
- W5: keys under `roadmap.delivery`, the screen's namespace, as `board.upstream` is the Board's.
