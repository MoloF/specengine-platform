---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-mcp]
ref: decision-apply analysis 2026-10-05; 08 §2 Phase 2, slice 5 (after queue-path-targets)
---

# Decision apply: records from the queue

## Why

A question or discrepancy is settled today by `spec reject PR --reason <answer>`: the answer lives only in SQLite, reads `rejected`, has no price or `canon:`, never reaches the branch, a bundle or a clone. Here one terminal action turns the owner's choice into an accepted decision record in the project's own shape, one new file in one `spec: apply PR-…` commit where the item was raised (ADR-0004, ADR-0005, ADR-0032); the queue and dedup then return that decision (08 AC-5). No new ADR.

Working answers (the owner's, 2026-10-05, session rule): Q1 these two kinds only, the other four `proposal-kinds`; Q2 the record committed alone, a linked update by its own approve; Q3 `--option N` required, a question's working answer or `--answer T`, never the recommendation by default; Q4 the shape only from `specengine.toml`, none → exit 2, no built-in; Q5 findings shown, never refusing (ADR-0012), structural defects refuse; Q6 no canon diff needed, a note.

## Description and interactions

`spec approve PR [--note T] [--option N | --answer T]` on a `question` or `discrepancy` (`ProposalKind::decides()`); `update`, `reject` unchanged (items rejected earlier stay so); no follow-up task. Writes: the record and one commit in the recorded worktree, the queue, the data directory. Core: config, template, render, ID format; store: schema 3, ops, intent-to-add; CLI: flags, steps, outputs, backup; MCP: `mirror.rs`, descriptions, no new tool.

## Data

**Config**, a top-level key of `ProjectConfig`, checked on load by every command (`specengine.toml:<line>: …`, exit 2):

```toml
[decision_records]
prefix   = "DEC"                    # an [ids] prefix: shape "number", scope "project"
dir      = "docs/records/DEC"       # a record is <dir>/<ID>.md
template = "templates/decision.md"  # tracked, read from the recorded worktree
```

Three required strings, no other key. `prefix` an `[ids]` entry (an `aliases_from` name → error naming the canonical; shape `name`, scope `feature` → error), its `width` and `aliases_from` used. `dir`, `template` in `[paths]` grammar. The index fingerprint unchanged.

**Template**: UTF-8, ≤ 64 KiB, a first line `---` and a closing `---` (its front-matter). Slots `{{name}}`, one left-to-right pass, values never re-scanned (`{{id}}` in agent text stays literal); list items joined by LF; a `{{` opening no slot → template error.

- Engine, anywhere: `id` (`DEC-0024`), `date` (`YYYY-MM-DD` of the run's `now`, UTC), `status` (`accepted`), `canon` (the first target, canonical: reference form), `targets` (`[Q-031, RULE-STAM-REGEN]`), `proposal` (`PR-0004`).
- **Free text**, body only: `title` (the chosen label or the answer's first line; whitespace runs → one space, trimmed, ≤ 128 bytes, cut + `…`; also front-matter, as `"…"` with `\`, `"` backslashed), `choice` (the label or the answer), `effect`, `cost` (the option's price; `--answer`: `price_of_other`; the working answer: empty), `summary`, `options` (`- <label> | <effect> | <price>` each), `evidence` (`- <file>[:<lines>][ <qpath>] | <observed> | <documented>` each; a question: `effect`, `options`, `evidence` empty), `note` (`--note`), `decided_by` (the step-6 identity).

Free text but `title` in the front-matter → template error. A value holding a character `escape_controls` escapes but LF and TAB (C0 with CR, DEL, C1, bidi) → exit 1 naming its source field (`options[1].label: holds U+202E: a decision record never carries it; nothing changed`). Rendered > `TEXT_MAX_BYTES` → exit 1. **Structure** (parsed under the recorded root's scheme): one front-matter, `class: decision`, `id:` the ID, `status: accepted`, defined IDs exactly the ID; a defect inside a free-text value's output → exit 1 naming its field, else template error. Free text lands after the closing `---`, so front-matter-looking lines stay body.

**ID**: `<prefix>-<n>`, zero-padded to `width` (one core function; past it the check's `id-width` shows). `n` = 1 + the greater of the numbers of IDs with the prefix or its `aliases_from`, defined or in `aliases:`, in the recorded root's index refreshed at step 4, and the numbers the project's queue issued (`record_id`). Previewed at step 4, issued at step 7 in the queue's `Immediate` transaction; kept and reused by its proposal across a reopen, never another's; gaps allowed.

**Queue** step 2 → 3 (`QUEUE_SCHEMA_VERSION` 3, one `Immediate` transaction): `ALTER TABLE proposals ADD COLUMN <c> TEXT`, in order `record_id`, `record_path` (root-relative), `record_title`, `record_text` (the rendered bytes), `choice` (JSON `{"option":1}`, `{"working_answer":true}` or `{"answer":"…"}`); `PROPOSAL_COLUMNS` 40. Set together at step 7, replaced by a later one, kept by `reopen_from`. Corrupt, named: any on an `update`; set partly; `choice` of another shape or kind, `option` out of range; `record_path` not clean; an `approved`/`applied` deciding row without them. Events `.approved`, `.applied` add `record`.

Store: `next_record(&RecordSeries {prefix, width, corpus_max})`, read-only; `approve_record_from(id, seen, &RecordApproval {series, preview, path, title, text, choice}, decision, now)`: `approve_from`'s compare-and-set, the ID recomputed under the lock, ≠ `preview` → `QueueError::Issued {next}`. `create`, `approve_from`, `applied_with` of a deciding kind without a record → `Invalid`.

**Review document**: after `linked`, the five columns (`choice` an object), `null` when absent; text: `record_text` a block, `choice` compact JSON; `--brief`: `record_text` `null`. **Inbox** entries gain `record_id`; an applied one's last column ends ` [<record_id>]`. **Intake match** gains `record` (a corpus hit's `id:`, a queue hit's `record_id`); a queue hit with a record: `path`, `answer` its `record_path`, `record_title`; text unchanged. **Backup**: `queue_schema` 3, 40 columns; import takes 1 (24), 2 (35), 3, missing columns NULL; a schema-1 or -2 DB exports as 3.

**Fixtures** (the test-engineer's): `[decision_records]` in `fixtures/spec-a` (`DEC`, `docs/records/DEC`), `fixtures/spec-b` (`ADR`, `docs/records/ADR`), `template = "templates/decision.md"` (outside the default roots), using every slot, `title` in front-matter and the H1, `class: decision`, `scope: [stamina]` (`[sync]`), `links:` `answers: {{targets}}`, `ref: {{proposal}}`.

## Rules and edge cases

**First** (after `main`'s terminal check): `applied` → exit 1 naming its commit and `record_id`; `rejected` → exit 1. `--option` on a question or update, `--answer` on a discrepancy or update, both, an `open` discrepancy without `--option` → exit 2 (``spec: `PR-0004` is a discrepancy: name the owner's choice with `--option N` (0-2); nothing changed``); `--option` ≥ the count → exit 1 ``--option 7: `PR-0004` has options 0-2; nothing changed``; `--answer` blank → exit 2, over 2 048 bytes or a refused character → exit 1. An `approved` deciding row (stopped after step 7) takes no choice (exit 2) and writes its `record_text`. Then the completion lookup; then the steps, numbered, logged and reopened as an update's (`docs/canon/proposal-apply.md` "Apply steps"):

- **2** Place: an update's.
- **3 Config**: the recorded root's `specengine.toml`, slug the queue's; no table → exit 2 ``spec: <root>/specengine.toml has no `[decision_records]` (`prefix`, `dir`, `template`) for `PR-0004`'s record: add it, or `spec reject PR-0004 --reason <answer>`; nothing changed``. The template: no symlink component, regular, tracked and clean, UTF-8, ≤ 64 KiB, slots known and placed; else exit 2 `<template>:<line>: <problem>`.
- **4 ID, path**: the index refreshed, the preview; `<dir>/<ID>.md` in the walk (else exit 2 naming `dir`); no existing component a symlink or non-directory, no entry at the path (dangling symlinks too), none in git's index; else exit 1 `` `<path>` exists: a decision record never replaces a file; nothing changed``.
- **5 Render**, **6 Structure**; then the tree checked as is and with the record, judged as against `HEAD`: `introduced: <n>` and findings above the prompt. The identity as an update's.
- **1 Consent**: stderr `record <ID> at <path>:`, the record escaped, indented two spaces, then `apply PR-0004 as <ID> (option 1 | the working answer | the given answer) on <branch> in <worktree>? [y/N]`.
- **7** `approve_record_from`; `Issued` → exit 1 `` `<ID>` was issued meanwhile (next `<ID'>`): run `spec approve PR-0004` again; nothing written``.
- **8 Write**: place and step 4 re-checked (exit 1); missing directories of `dir` made one by one, never through a symlink; `.<name>.specengine-<pid>-<n>.tmp` created new, synced, hard-linked to the path (`link` never replaces: an entry appearing meanwhile → exit 1, untouched), removed; the directory synced.
- **9 Commit**: `git add --intent-to-add -- :(literal)<path>`, then an update's `commit --only`; body `<ID>: <title>`, the four trailers. Failed without this ID's commit: `git rm --cached --quiet --ignore-unmatch -- :(literal)<path>`, the file and the directories this run made (innermost first, if empty) removed, reopened, exit 1 ``the commit of `<path>` failed, the record removed: <git's error>``.
- **10 Verify**: one parent, the old `HEAD`, name-status exactly `A <path>`, its blob `record_text`, `Proposal:` the ID → `applied_with`, `update_paths`; stdout `applied PR-0004 as <sha> on <branch>: <ID> <path>`.

**Notes**: a linked update not applied → `` `PR-0005` (its linked update) is <status>: `spec approve PR-0005` changes <canon> ``; none → ``<ID> names <canon> in `canon:`, its text unchanged: `spec propose update` changes it``. **Completion**: the proposal's own commit (trailer, one parent, exactly `A <record_path>`, its blob `record_text`), never re-rendered; else `changes a, b`, `does not carry the record`.

**Known limits**: a record hand-written under the issued ID on another branch → `id-taken` at merge; a pty passes consent; no literal `{{` in a template; free text with a refused character only rejected; a queue restored from before step 7, the record committed: neither approve nor reject takes it.

## Acceptance criteria

Setup: temp repos of `fixtures/spec-a`, `fixtures/spec-b`, templates committed; scratch `HOME`; clock `2026-10-05T12:00:00Z`; a git identity; items raised with `distinct_from` naming their hits (`DEC-0023`); library approve, consent yes. Refused: nothing written, the row as before. M: the mutation turning it red.

- [ ] AC-01 — spec-a discrepancy on `RULE-STAM-REGEN`, 3 options, a tracked file modified, `--option 1`: one commit, one parent, name-status exactly `A docs/records/DEC/DEC-0024.md`, body `DEC-0024: <label 1>`, four trailers; `status: accepted`, `date: 2026-10-05`, `canon: RULE-STAM-REGEN`, option 1's label, effect, price; the other file still modified; `spec check` errors as before; `applied` (M: staging the whole tree).
- [ ] AC-02 — question on `Q-031`, no flag: the working answer, `cost` empty; `--answer T`: `T`, cost `price_of_other` (M: cost always `price_of_other`).
- [ ] AC-03 — spec-b, Cyrillic labels, `--option 0` → `docs/records/ADR/ADR-0003.md`, labels verbatim; the new sources hold no literal equal to an `[ids]` prefix or kind of spec-a, spec-b, the root config (class names aside), `Cost`, `docs/`, `records/` (M: `"DEC"`).
- [ ] AC-04 — no `[decision_records]`: exit 2 naming it; worktree, git index, `HEAD` unchanged; `open`, one `apply_failed` (step 3), `next_record` unchanged; `reject --reason` works (M: a built-in template).
- [ ] AC-05 — `DEC-0007`, `DEC-0023` → `DEC-0024`; then a second worktree on a branch without it → `DEC-0025`; a declined prompt: `next_record` unchanged (M: issuing from the worktree only).
- [ ] AC-06 — label `x\nstatus: rejected` → `status: accepted`, one `title`; effect `## X {#RULE-STAM-REGEN}` → exit 1 naming `options[1].effect`, no prompt; label `{{id}}` literal; U+202E in `summary` → exit 1 (M: raw substitution; a re-scan).
- [ ] AC-07 — stdin a pipe → exit 2 before reading; consent `n` → exit 1, path absent, `git ls-files --stage`, `git status --porcelain` as before, `record_id` NULL (M: the ID before the prompt).
- [ ] AC-08 — `dir` a missing `docs/records/DEC/new`, a pre-commit hook exiting 1 → exit 1; file and `new/` gone, `git ls-files --stage -- <path>` empty; `open`, `record_id` kept, one `apply_failed` (step 9); without the hook → the same ID, applied (M: the intent-to-add entry left).
- [ ] AC-09 — the consent callback writes X at the path, then yes → exit 1, X intact, no commit, no temporary; the store's create-new on an existing path fails, its bytes intact (M: rename over the path).
- [ ] AC-10 — `approved` at step 7, its commit made by hand from `record_text`: approve → `applied`, no new commit or prompt; with another `date:` → `does not carry the record` (M: re-rendering).
- [ ] AC-11 — after AC-01, the same summary on `RULE-STAM-REGEN` from the recorded root: `created: false`, hits include corpus `DEC-0024` and queue `PR-0001` (`applied`, `path` the record, `answer` the title, `record` `DEC-0024`); `counts()` unchanged; `get_proposal` = `review --brief --json` with the record keys (M: the answer from `decision_note`).
- [ ] AC-12 — every MCP tool on a decided item: `git status --porcelain` empty, `tools/list` unchanged; `export state` `queue_schema` 3, 40 columns; schema 1, 2 dumps restore, re-export byte-identical as 3; a v2 DB: `user_version` 3; gate clean, worst W ≤ 109 484 (M: a column missing).
- [ ] AC-13 — same item, choice, note, clock, identity in two fresh repos → byte-identical records and messages (M: the wall clock).
- [ ] AC-14 — `--option` on a question or update, `--answer` on a discrepancy, both, a discrepancy without `--option` → exit 2; `--option 7` of 3 → exit 1 naming `0-2`; no event (M: an out-of-range option accepted).

## Out of scope

Kinds `create`, `decision`, `interpretation`, `amendment` (`proposal-kinds`); a linked update in the same commit; rounds; `changes_requested`, defer, `superseded`; follow-up tasks, `@assumes` (`task-package`); superseding a record; `spec new`; the homoglyph fix; `review_proposal`; UI; this repository's ADRs; `.claude/`; pilots.

## Implementation

Pending. At shipping (the queue canons are full): a new `docs/canon/decision-record.md`; pointers from `proposal-apply.md`, `proposal-queue.md` "Store", `queue-backup.md` "Format", `agent-intake.md` "Settling"; READMEs not grown on balance.
