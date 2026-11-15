---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-11-15
---

# `spec auto`: show, stop, resume, start, approve, next

The terminal's commands of auto mode (ADR-0081): the rule, switches, reasons, events and trailers are `auto-mode.md`'s; `land` and `hold`: `spec-auto-land.md`; a hub's apply: `hub-auto.md`. Code: CLI `auto.rs` (show, stop, resume), `auto_read.rs` (`load_auto(env, project, git, surface)`, `proposal_facts`, `task_facts`), `auto_pack.rs` (packs, start), `auto_approve.rs`, `auto_next.rs` (next, lanes), `task.rs` (a line's binding); store `queue/auto.rs`. Each takes `--root` and `--json`, reads the rule through `load_auto` (the local surface but `show --surface hub`); under a switch `start`, `approve`, `land` and `hold` exit 1 `auto is off: <why>`, writing nothing, and `next` answers `stop`. An exit-1 refusal prints `spec: <reason>` alone; a hold its own line.

## Show

`spec auto show [--surface local|hub] [--json]`: the effective rule, the switches, every live pack's items and this repository's `open` proposals by ID number, each judged for the surface. It writes nothing but any read's index refresh; exit 0, 2 when it cannot run.

```
auto: on (both), rule b3:<64 hex>
  zones: code, tests; excluded zones: narrative; excluded kinds: scenario; branch: auto; max_tasks 10; max_applies 20
team: <n> login(s)
stop file: <path> absent
pack <seq>: started <at> by <by>, until <until|no end>, on <branch>, rule <hash>
  <item> | <status> | <act> | <verdict> | <reason>: <detail>
proposals:
  <PR> | <kind> | <task_id or -> | <verdict> | <reason>: <detail>
note: <note>
```

`team: none` with no `[team]`, `team: <why>` when it does not read; `... exists` for a stop file there; an `apply` ends `| -`. Off: the first line `auto: off (<reason>): <why>`, the rule's lines left out, every item `hold | <reason>: <why>`. `<item>`: a task's ID, or an unbound line `<pack>/<n>` with `<status>` `unbound`. `<act>`: `plan` (`draft`, `changes_requested`; a line `plan-line`), `approve` (`review`), `implement` (`ready`, `in_progress`), `land` (`done`), `-` (landed, held, cancelled). `<verdict>`: `apply`, `hold`, `landed <sha12>`, `held` (its `auto.held` reason and detail), `cancelled`. Past a pack's `until` its plans and implements are `hold | until`. A `done` task is judged on its diff's paths, never merged or checked here.

```json
{"auto":{"state":"on","reason":null,"why":null,"surface":"local","rule":"b3:…","mode":"both","zones":["code","tests"],"exclude_zones":["narrative"],"exclude_kinds":["scenario"],"branch":"auto","max_tasks":10,"max_applies":20,"logins":1,"stop_file":"<path>","stopped":false,"applies_today":null,"gate":"observe"},
 "packs":[{"pack":812,"at":"…","by":"…","until":null,"branch":"auto","rule":"b3:…","items":[{"item":"T-0041","task":"T-0041","line":null,"text":null,"status":"review","act":"approve","verdict":"apply","reason":null,"detail":null,"commit":null,"lane":"general"}]}],
 "proposals":[{"id":"PR-0007","kind":"update","task_id":"T-0041","verdict":"apply","reason":null,"detail":null}],
 "notes":[]}
```

Every key present. Off: `state` `off`, `reason` and `why` set, the rule's keys `null`. `logins` 0 with no `[team]`; `applies_today` the hub surface's count, else `null`; `gate` `[gate] mode`, `off` when absent. A mode other than `selective` adds `note: [gate] mode is <m>: an agent's uncovered write passes; spec feed --needs-me lists them`.

## Stop and resume

`spec auto stop [--json]` needs no project: `HOME`'s data directory (`crates/specengine-cli/README.md` "Database"), made when absent. It writes `<data dir>/auto-stop`, `stopped <now> by <git identity, else unknown>` and LF; stdout `stopped: <path>`; already there: exit 0, `stopped: <path> (since <its first line>)`, unchanged. JSON `{"path":…,"stopped":true,"notes":[]}`.

`spec auto resume [--json]`: stdin no terminal, exit 2 before anything is read (`proposal-apply.md` "Consent"; no `--yes`). Absent: exit 1 `` not stopped: <path> is absent; nothing changed ``. Else stderr `resume auto mode on this machine, removing <path> (<its first line>)? [y/N]`; `y` or `yes` removes it, stdout `resumed: <path> removed`, JSON `{"path":…,"stopped":false,"notes":[]}`; else exit 1 `` not resumed: the answer was not `y`; nothing changed ``. The file is read by existence only, its text shown, never parsed.

## Packs

A pack is its `auto.started` event, named by its `seq`: its tasks in the order given, then its lines `<pack>/<n>`, `n` from 1 in the order of the kept lines. A line is bound by the first `auto.bound` naming it. An item is **finished** when its task has an `auto.landed`, its last `auto.*` event is `auto.held`, or its task is `cancelled`; an unbound line, when its last `auto.*` event is `auto.held`. A pack is **live** while an item is unfinished, whatever its `until` (`Pack::live`): past it (`Pack::ended`) it still lands, while approve and implement hold `until`, so a task that never finishes there joins no new pack until it is cancelled or held. A task belongs to the pack listing it or holding its line, and to at most one live pack, judged again inside the transaction recording a start (store `record_auto_checked`).

## Start

`spec auto start T... [--lines FILE] [--until TIME]`, in order:
1. stdin no terminal: exit 2 before anything is read, ``spec: `spec auto start` asks the owner for consent on a terminal, and stdin is not one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed``. No `--yes`.
2. No task and no `--lines`: exit 2. A `T` that is no task ID: exit 1; a look-alike: exit 2 (`tasks.md` "Exits"); a repeat is kept once with a note. A state but `draft`, `review`, `changes_requested`, `ready`: exit 1 ``T-0042 is <status>: a pack takes draft, review, changes_requested or ready; nothing started``; in another live pack: exit 1 ``T-0042 is in live pack <n>; nothing started``. `--lines FILE` (`-`: exit 2): UTF-8, lines trimmed, empty and `#`-led ones skipped, each at most 4 096 bytes with no control character, else exit 1 `--lines: line <n>: <why>`. Items 1 to `max_tasks` (an empty pack too), else exit 1 ``the pack holds <n> items; the rule's max_tasks is <m>; nothing started``. `--until`: `YYYY-MM-DDTHH:MM:SSZ` after now, else exit 2 `--until: <why>`.
3. A switch: exit 1. No git identity in the main worktree: exit 2, as `spec task approve`.
4. stderr, controls escaped, then the question:

```text
auto pack: 2 task(s), 1 line(s); rule b3:<64 hex>; on auto (3f2a1c9d0b4e, or: new at 3f2a1c9d0b4e); until 2026-11-16T07:00:00Z
  T-0041 | review | Add the save slot list | apply
  T-0042 | draft | Fix the door timer | hold (excluded-zone): `docs/narrative/intro.md` is in the excluded zone `narrative`
  line 1 | Make the inventory sortable by weight
note: [gate] mode is observe: an agent's uncovered write passes; spec feed --needs-me lists them
start this pack: its tasks approved and landed on `auto` by the rule, with no further key? [y/N]
```

A task's mark is the approve's path checks on its targets and affected nodes now, a hint, never a refusal; `no end` with no `until`. The rule's notes are listed; the branch checked out in a worktree adds `` note: `auto` is checked out in <w>: a land holds until it is not ``.
5. `y` or `yes`: the branch made at the main worktree's `HEAD` when absent (`git branch`; failing: exit 2, nothing recorded), then `auto.started` (`frozen` the canonical form as a JSON object, `by` the identity as `Decided-by` reads). stdout `started pack 812: 2 task(s), 1 line(s) on auto`; JSON `{"pack":812,"tasks":["T-0041","T-0042"],"lines":["Make the inventory sortable by weight"],"branch":"auto","branch_at":"<sha>","until":"2026-11-16T07:00:00Z","rule":"b3:…","notes":[]}`. A start the transaction refuses deletes the branch it just made, by a compare-and-set `update-ref -d`. Else exit 1 `` not started: the answer was not `y`; nothing changed ``, no branch, no event.

## A line's task

With `SPECENGINE_AUTO_LINE` set and not empty, `spec task new` binds the task it creates, in the same transaction (store `create_task_bound`), to line `<pack>/<n>` when the value is two decimals, the pack an `auto.started` of this repository, live, holding line `n` unbound, and the switches pass: `auto.bound`, `task.created` still `{id}`, the line `bound to pack 812 line 1` after `created T-0050` (JSON: the text in `notes`). Otherwise the task is created unbound, plus the note `` SPECENGINE_AUTO_LINE `<v>`: <why>; T-0050 is bound to no line `` (`no <pack>/<line>`, `no live pack <n> here`, `pack <n> has no line <k>`, `line <n>/<k> is bound to T-0049`, `auto is off: <why>`). Unset or empty: today's command, byte for byte.

## Approve

`spec auto approve T`, no terminal (the runner's), run in the worktree the task will be claimed in, as `spec task approve` freezes from the caller's. In no live pack: exit 1 `` T-0042 held (not-in-pack): `T-0042` is in no live pack; nothing approved ``, no event. Judged `until`, `status` (needs `review`), then the paths and kinds of its targets and affected nodes. Apply: `approve_task_by_rule` (the approve and `task.approved` by rule in one transaction), the snapshot as `spec task approve` freezes it, `by` the worktree's identity; stdout `approved T-0042: ready (by rule, pack 812)`; JSON `{"id":"T-0042","status":"ready","by":"rule","pack":812,"rule":"b3:…","reason":null,"detail":null,"notes":[]}`. A hold: exit 1 `` T-0042 held (<reason>): <detail>; nothing approved ``, one `auto.held` (`act` `approve`), but `status`, a caller's error, logs none. A lost compare-and-set: `tasks.md`'s text, exit 1.

## Next

`spec auto next [--busy ITEM]... [--skip-lane D]... [--slots N]`: read-only, deterministic, the runner's next act; always one JSON document (`--json` changes nothing), exit 0, 2 only when it cannot run.

```json
{"action":"implement","reason":null,"why":null,"pack":812,"item":"T-0042","task":"T-0042","line":null,"text":null,"title":"Fix the door timer","lane":"general","branch":"auto-T-0042","base":"auto"}
```

- **Stop**: a switch, `{"action":"stop","reason":"<off|team|stopped>","why":"…"}`, every other key `null`. A live pack whose own switches fail (its branch changed since its start, or a frozen rule that does not read: core `judge_pack_switches`) is skipped; every live pack skipped: `stop` with the first one's reason.
- **Running**: the packs' `in_progress` tasks and `done` ones neither landed nor held, plus each `--busy` item. Two overlap when their targets and affected nodes, canonical, and their holder paths meet; a line overlaps nothing.
- **Candidates**, the first wins, by action, then pack `seq`, then item order: `land` (a pack task `done`, neither landed nor held); `approve` (`review`, overlapping nothing running); `implement` (`ready`, overlapping nothing running, not busy); `plan` (`draft`, `changes_requested`, not busy); `plan-line` (a line unbound and unheld, not busy); all but `land` need `--slots` (default 1) above 0. Past its `until` a pack offers only `land`. `--skip-lane D` (repeatable) passes over a lane's candidates.
- **None**: `wait` when anything runs (a `--busy` item, a line too) or a candidate was passed over only for an overlap or the slots; else `done`.
- `branch` (`implement`, `approve`): `<rule branch>-<T>`, `base` the rule's branch, else both `null`; `text` a line's; `title` the task's, else `null`.

**Lanes** (`auto_next::Lanes`, `spec feed`'s too): a task's `lane` is the first `[[workspace]]`, in config order, whose `module` holds every target (a rooted module: its root is the target or an ancestor; a direction: the target's kind is one it lists); else, and for a line, `general`. Advisory, never a right (ADR-0073), nothing refused.

## Tests

CLI `tests/auto_show.rs`, `auto_manual.rs`, `auto_start.rs` (pty), `auto_approve.rs`, `auto_line.rs`, `auto_next.rs`; core `tests/auto_rule.rs`, `auto_predicate.rs`; store `tests/auto_events.rs`; over `fixtures/auto-mini` (its `rules/*.toml` committed by each test).
