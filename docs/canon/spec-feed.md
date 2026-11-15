---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-15
---

# `spec feed`: what happened, newest change first, and what needs the owner

A read of the queue's `events` and the index: the terminal's `spec feed`, the daemon's `GET .../feed`, the UI's Activity, and the Tasks list's sort. Built by `auto-feed` (RM-0124, no ADR); events, reasons and trailers: `auto-mode.md`; the `auto` head is `spec auto show`'s judging for the reader's surface (`spec-auto.md`). It needs no `[auto]` table, works in manual mode too, decides nothing and writes nothing: no event, no stored choice on the server, the index refreshed only when a `[[workspace]]` names a module (a lane needs the tree). Code: CLI `feed.rs` (`feed(env, globals, &FeedRequest) -> FeedOutcome`), store `queue/feed.rs` (`FeedSnapshot`, one `open_existing` snapshot: no schema step, no database reads as no event), http `src/feed.rs`, UI `src/feed/`, `src/api/feedQueries.ts`, `feedMerge.ts`, `src/tasks/sort.ts`.

## Command

`spec feed [--since SEQ|TIME] [--needs-me] [--task T] [--department D] [--json]`: the project's events (the slug's, as the live tail streams them) in `seq` order. `--since SEQ` (a non-negative decimal): `seq` above it; `--since TIME` (`YYYY-MM-DDTHH:MM:SSZ`): `at` at or after it; neither: the newest 100 (`FEED_DEFAULT`), `more` false. At most 512 (`FEED_MAX`), the oldest first, `more` true when one more was there (the next `--since` is `last_seq`); the filters apply before the cap. `--task T`: the entries whose `task` is T (a proposal's: its row's `task_id`) and the `auto.started` listing T. `--department D`: entries whose `lane` is D (`spec-auto.md` "Next", Lanes); an unknown one, none and a note. A bad value: exit 2; exit 0, else 2 when it cannot run.

```text
1201 | 2026-11-16T01:02:03Z | task.run_reported | T-0042 | Fix the door timer | completed: the timer resets on close
1204 | 2026-11-16T01:05:40Z | proposal.applied | PR-0007 | R-02 | as 3f2a1c9d0b4e by rule
1206 | 2026-11-16T01:06:02Z | auto.landed | T-0042 | Fix the door timer | on auto as 9b1e77c2d0aa
needs me: held
  <seq> | <at> | <item> | <act> | <reason>: <detail>
needs me: open
  <id> | <kind> | <target_id> | <task_id> | <created_at>
needs me: failed runs
  <task> | run <n> | <outcome> | <summary's first line> | <ended_at>
needs me: gate observed
  <seq> | <at> | <path> (<zone>) | <branch>
needs me: unlanded
  <task> | <status> | pack <pack>
```

No head line; `needs me:` sections only under `--needs-me` (an empty one `needs me: <list>: none`), then `note:` lines on stderr; `-` for an absent value, controls escaped. **Subject**: a `proposal.*` event's proposal; else the task, else the proposal, else the card, else a line `<pack>/<n>`, else `-`. The title and status follow it: the task's title now (else the proposal's `target_id`), the card's title, the task's or proposal's state now. **Words**: `task.run_reported` `<outcome>: <the summary's first line, 160 characters>`; `task.approved` `by rule` or `-`; `proposal.applied` `as <sha12>`, ` by rule` when so; `proposal.rejected` the reason's first line; `proposal.apply_failed` `step <n>: <reason>`; `auto.started` `<n> task(s), <m> line(s) on <branch>`; `auto.bound` `line <pack>/<n>`; `auto.held` `<reason>: <detail>`; `auto.landed` `on <branch> as <sha12>`; `gate.observed` `<path> (<zone>)`, a config `(config)`; others `-`.

## JSON

```json
{"auto":{"state":"on","reason":null,"why":null,"rule":"b3:…"},
 "after":1200,"last_seq":1207,"more":false,
 "entries":[{"seq":1204,"at":"2026-11-16T01:05:40Z","type":"proposal.applied","task":"T-0042","proposal":"PR-0007","card":null,"line":null,"title":"R-02","status":"applied","by":"rule","outcome":null,"summary":null,"commit":"<sha>","reason":null,"detail":null,"path":null,"lane":"general"}],
 "needs_me":null,
 "notes":[]}
```

Every key present, `null` where none. `task`: a `task.*` payload's `id`, an `auto.*` payload's `task`, a proposal event's row's `task_id`; `proposal`: a `proposal.*` payload's `id`, `auto.held`'s; `card`: a `card.*` payload's `id`; `line`: `auto.bound`'s or a line's `auto.held`; `by` `"rule"` when the payload says so; `outcome`, `summary` (at most 512 bytes): `task.run_reported`'s run; `commit`: `proposal.applied`'s, `auto.landed`'s merge; `reason`, `detail`: `auto.held`'s, `detail` also a rejection's reason and an apply failure's `step <n>: <reason>`; `path`: `gate.observed`'s; `lane`: the task's. Another repository's task keeps its ID, its `title`, `status` and `lane` `null`. A payload that is not JSON: its `type` and `seq`, every other key `null` (`at` too), and the note `event <seq>: its payload is not JSON`. `after`: the `--since` SEQ given, else `null`; `last_seq`: the last entry's `seq`, else that SEQ, else `null`.

`needs_me` under `--needs-me`:

```json
{"held":[{"seq":1207,"at":"…","pack":812,"task":"T-0043","line":null,"proposal":null,"act":"land","reason":"check","detail":"…"}],
 "open":[{"id":"PR-0009","kind":"question","target_id":"R-02","task_id":"T-0042","created_at":"…"}],
 "failed_runs":[{"task":"T-0044","run":1,"outcome":"failed","summary":"…","ended_at":"…"}],
 "gate_observed":[{"seq":1190,"at":"…","path":"src/x.rs","zone":"code","branch":"auto-T-0044"}],
 "unlanded":[{"task":"T-0045","status":"done","pack":812}]}
```

- `held`, whatever `--since`: an item (a task, else an unbound line, else a hub's proposal held alone, `pack` and `task` `null`) whose last `auto.*` event is `auto.held`, until nothing waits on it: a task landed after (its last `auto.*` is `auto.landed`) or `cancelled`; a proposal held alone once its row is no longer `open`, whoever decided it. A gone row, or another repository's task, stays.
- `unlanded`: a pack's task `done` whose last `auto.*` event is neither `auto.held` nor `auto.landed`. Both count this repository's packs alone.
- `open`: this repository's `open` proposals by ID number.
- `failed_runs` (`failed`, `partial`, `abandoned`) and `gate_observed`: the `--since` window, else the newest 100 events; never narrowed by `--task` or `--department`.

## Route

`GET /api/projects/:p/feed`, local and hub, any login on a hub, in the project's turn as every read (`GET_ROUTES`). Queries `since`, `needs_me` (`1`), `task`, `department`, each at most once, as the flags; another key or a bad value: 400 with the error body. The body is `spec feed --json`'s on the same state, the `auto` head judged for the hub's surface on a hub. Key sets `Feed`, `FeedEntry`, `FeedNeedsMe` and its five row sets (`fixtures/daemon-keys.json`).

## UI

- **Activity** (`#/<p>/feed`, `#/<p>/feed/needs-me`, a lazy chunk): the nav's second entry after Overview in every department, never one of the nine sections; the palette's Sections group (eleven entries). A head line from `auto`: `Auto: on, rule b3:<12 hex>` or `Auto is off: <why>`. Tabs **All** (entries newest first) and **Needs me** (the five lists, newest first, each empty one saying so); a Department select when the project has workspaces. An entry: the local time, the type's label, a link (`words.ts` `subjectOf`: its task's Tasks detail, else its proposal's view, else its card's modal; a proposal event bound to a task links the task), the title, the words, `By rule` when `by` is `rule`. Hold reasons (all 29), acts and proposal kinds read as plain labels, the code in `title`; an unknown code raw. A gap of two hours or more between entries reads "Nothing happened for H h M min": a usage-limit pause has no event.
- **New since your last visit**: per viewer, `specengine.feed.seen.<p>`, on a hub `.<login>` appended (`ui-client.md` "Stored choices"), read when the view opens; entries above it, and those arriving while open, are `New`; with none kept every entry is. Written with the highest `seq` shown when the view closes (`pagehide` too) and on **Mark all seen**, which never lowers it and is announced. The chip's count shows `100+` at the read's cap.
- **Live**: every event of the project's tail, and a gap, re-read the feed through `readAgainKept` (`ui-client.md` "Live re-reads"), never aborting a read in flight: `since` the highest `seq` held, the answer appended; a gap re-reads it whole.
- **Tasks, Sort**: `By state` (the groups, the default) or `Last changed` (one list by `updated_at`, newest first, ties by ID number, highest first), `specengine.tasks.sort.<p>`.
- **Applied by rule**: a decided proposal's read-only view (`proposal-review.md` "Inbox") whose document holds `delegated` shows `Applied by rule`, the commit's 12 hex and the rule hash's 12, the whole hash its accessible name; no Confirm, no stage.

## Known limits

- `FeedEntry` lacks `auto.started`'s counts and branch, `auto.landed`'s branch and `gate.observed`'s zone: the text has them, the JSON and the page do not.
- First load stands 0.22 kB under its bar (`ui-languages.md` "Size"): the next screen goes lazy first.

## Tests

CLI `tests/feed.rs` (FD1, FD2); http `tests/feed_route.rs` (FD3), `daemon_keys.rs`, `hub.rs` (`GET_ROUTES`); UI `src/feed/*.test.ts{,x}`, `src/api/feed.{http,live}.test.ts{,x}`, `src/tasks/TasksView.test.tsx` (FD4), `src/inbox/ClosedProposal.test.tsx` (FD7), `src/mocks/feed.smoke.test.tsx`.
