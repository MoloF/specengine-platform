---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli]
ref: queue-export analysis 2026-10-05; 08 §2 Phase 2, slice 2
---

# Queue backup and restore

## Why

The queue (`proposals`, `events` in `<slug>.db`) is the only state git cannot rebuild, and nothing backs it up: a lost DB loses open work and restarts IDs at `PR-0001`, colliding with old `Proposal:` commits. ADR-0003: a JSONL dump to a backup directory, restored by `spec import-state`; binding too: ADR-0004, ADR-0017, ADR-0032. No new ADR: `export state` spells ADR-0003's `export --state` as a mode, like `export index`.

Working answers (the owner's, 2026-10-05, session rule): Q1 `queue.md`, bare `spec export` → `task-package`; Q2 the commands below; Q3 the destination below, no config key; Q4 a non-empty queue refused; Q5 rows as stored; Q6 consent for import only; Q7 no automatic backups; then header counts.

## Description and interactions

- `spec export state [--out PATH]`: the root slug's whole queue (every repository); no consent, no index refresh. Default `<data dir>/backups/<slug>-<YYYYMMDDTHHMMSSZ>.jsonl` (UTC, injected clock; `backups/` made 0700). stdout `wrote <path>: <p> proposal(s), <e> event(s)` (`--json` `{path, proposals, events}`), never the dump.
- `spec import-state FILE`: into the slug's empty queue, after one question on a terminal. stdout `restored <p> proposal(s), <e> event(s) into <db>` (`--json` `{db, proposals, events}`).
- Library `export_state` (`now`), `import_state` (`Consent`); the store (`SqliteQueue`) reads and inserts rows.

## Data

Format 1: UTF-8 compact JSON, one object per LF-ended line.

1. Header `{"format":1,"queue_schema":1,"project":"<slug>","proposals":<p>,"events":<e>}`, keys in this order: `STATE_FORMAT`, `QUEUE_SCHEMA_VERSION`, the root's slug, the row counts (same snapshot).
2. Every `proposals` row by ID number (`ORDER BY length(id), id`), then every `events` row by `seq`, as `{"<table>":{…}}`: every column in table order (`proposal-queue.md` "Store"), TEXT a string, NULL `null`, `seq` a number; `author`, `diagnostics`, `payload` stay strings.

No export time or host inside: equal queues, equal bytes. Oracle: `SqliteQueue::dump()`.

```
{"format":1,"queue_schema":1,"project":"spec-a","proposals":1,"events":1}
{"proposals":{"id":"PR-0001","project":"spec-a",…,"updated_at":"…"}}
{"events":{"seq":1,"project":"spec-a","type":"proposal.created","payload":"{\"id\":\"PR-0001\"}","at":"…"}}
```

## Rules and edge cases

**Export.** Both tables in one read transaction, raw, never `list_readable` (unreadable rows too); a row whose `project` is not the slug → exit 2 naming it. WHEN the destination's nearest existing ancestor, canonical, lies inside the root's worktree top (no git: the root) → exit 2. `--out` is cwd-relative, its parent must exist. Written to `<path>.partial` (create-new, 0600), synced, renamed only while `<path>` does not exist, else exit 2, its bytes untouched; a failure removes the partial, a crash leaves only it. Exit 2 writes nothing.

**Import**, in order, nothing written before step 5:

1. stdin not a terminal → exit 2 before FILE is opened (approve's, naming `spec import-state`).
2. The whole file; a defect → exit 2 `<FILE>:<line>: <defect>`, never the line's text: not UTF-8, not a JSON object, an empty line or file, no final LF; a header missing, other keys or types, `format` or `queue_schema` not the build's (higher: `… upgrade SpecEngine`), `project` not the root's slug (both named); a row not `{"<known table>":{…}}`, a missing or extra column, a TEXT value not string or `null`, `seq` not an integer ≥ 1, `id` not as `proposal_id` writes it, `project` not the header's, a repeated `id` or `seq`; at the end, rows read not the header's counts → exit 2 `<FILE>: header counts <p>, <e>; found <p'>, <e'>`.
3. Either table not empty (any project) → exit 2 ``the queue of `<slug>` in <db> holds <p> proposal(s), <e> event(s): import-state restores only into an empty queue (a fresh data directory, or <db> moved aside); nothing changed``.
4. stderr `restore <p> proposal(s) and <e> event(s) of <slug> from <FILE> into <db>? [y/N]`, escaped as the queue's prompts; only `y`/`yes`, else exit 1 ``not restored: the answer was not `y`; nothing changed``.
5. One `Immediate` transaction: step 3 again, every row inserted as given, commit. No event of its own; next ID and `seq`: highest + 1.

**One door**: neither command writes a spec file, commits, runs apply, completion or git in a worktree; an `approved` row stays so until `spec approve` completes it. Export writes only the destination, import only the two queue tables, on a read-only project too.

**As stored**: no path rewritten; a recorded common dir gone makes an orphan (`proposal-queue.md` "Place, IDs, repositories"), taken only by reject; same machine: move it back, `git worktree repair`.

**Known limits** (ADR-0012), R1: a dump older than the queue loses what came after; their IDs are issued again; one with an old `Proposal:` commit on a branch blocks reject. A DB lost before its first export is lost; deleting the data directory deletes `backups/` (copy dumps out).

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a` (`spec-b` if named), scratch `HOME`, injected clock, consent yes; fresh: another `HOME`. M: the mutation turning it red.

- [ ] AC-01 — every state, a second repository, an orphan, an unreadable row (bad `base_commit`), their events: export, import fresh, `dump()` equal (M: export via `list_readable`; filter by common dir; drop `decision_note`).
- [ ] AC-02 — equal rows inserted in another order, exported later: byte-identical files (M: order by rowid; export time inside).
- [ ] AC-03 — restored highest `PR-0007`, `seq` N: the next `propose` prints `PR-0008`, its event `seq` N+1; import adds no event (M: renumber from `PR-0001`; an import event).
- [ ] AC-04 — non-empty queue: exit 2 naming both counts, no prompt, `dump()` unchanged (M: insert-or-ignore).
- [ ] AC-05 — one defect on a valid dump's last line (not JSON, unknown table, unknown or missing column, a number in a TEXT column, repeated `id`, repeated `seq`, `project` ≠ header), each exit 2 naming the line; cut after a whole row: exit 2 naming the counts; queue empty (M: commit per row; drop any one check; count check removed).
- [ ] AC-06 — `format` 2, `queue_schema` 2: exit 2 with `upgrade SpecEngine`; a spec-b dump into spec-a: exit 2 naming both; queue empty (M: either check removed).
- [ ] AC-07 — an `approved` proposal with its commit on its branch, exported, imported fresh: both worktrees' `git status` empty, `HEAD` and tips unchanged, still `approved` (M: import runs the completion lookup).
- [ ] AC-08 — `--out` in a missing subdirectory of the worktree, or via a symlink into it: exit 2, nothing written; an existing file: exit 2, bytes untouched; the file 0600, `backups/` 0700, no `.partial` left; stdout: path, counts (M: worktree check removed; truncate; partial kept).
- [ ] AC-09 — a proposal's common dir gone: `inbox` notes it, `review`, `approve` exit 2 (orphan), `reject` takes it; stored `git_common_dir`, `worktree` byte-equal to the dump (M: import rewrites paths).
- [ ] AC-10 — piped stdin: exit 2 with the terminal message, a missing FILE too; an answer other than `y`/`yes`: exit 1, queue unchanged (M: terminal check removed or after the read).
- [ ] AC-11 — the new tests pass under process `HOME=/nonexistent`; no dump under `fixtures/`; the anonymity test green (M: a test using the process `HOME`; a committed dump).
- [ ] AC-12 — gate clean; worst W ≤ 108 744 B; new Tier 2 canon ≤ 12 288 B, index ≤ 10 240 B, CLI README ≤ 10 194 B; `CLAUDE.md`, store README not grown; `grep -r "until slice 2" docs/canon crates/*/README.md` empty; `architecture.md#storage`, 07 §2 spell both commands (M: one byte added to `CLAUDE.md`).

## Out of scope

`queue.md`, bare `spec export`; merging; re-targeting; automatic backups, retention (daemon); other tables (their slices extend the format); MCP tools; Beads/Task Master (Phase 5); slug rename; an ID floor from trailers.

## Implementation

Pending.
