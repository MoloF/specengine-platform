---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-mcp]
ref: decision-apply analysis 2026-10-05, refreshed 2026-10-06; 08 §2 Phase 2, slice 5
---

# Decision apply: records from the queue

## Why

Today `spec reject PR --reason <answer>` settles a question or discrepancy: the answer stays in SQLite, reads `rejected`, has no price or `canon:`, never reaches a branch or clone. Here one terminal action makes the owner's choice an accepted decision record in the project's own shape: one new file, one `spec: apply PR-…` commit where the item was raised (ADR-0004, ADR-0005, ADR-0032); queue and dedup then return it (08 AC-5). No new ADR.

Working answers (the owner's, 2026-10-05, session rule): Q1 these two kinds only, the other four `proposal-kinds`; Q2 the record committed alone, a linked update by its own approve; Q3 `--option N` required, a question's working answer or `--answer T`, never the recommendation by default; Q4 the shape only from `specengine.toml`, none → exit 2, no built-in; Q5 findings shown, never refusing (ADR-0012), structural defects refuse; Q6 no canon diff needed, a note. Refreshed 2026-10-06, to accept: D1 Rules "Canon", D2 Data "Plugin".

## Description and interactions

`spec approve PR [--note T] [--option N | --answer T] [--canon REF]` on a `question` or `discrepancy` (new `ProposalKind::decides()`); `update`, `reject` unchanged (earlier rejections stay); no follow-up task. Writes the record, one commit in the recorded worktree, the queue, the data directory. Core: config, template, render, ID format, the refused-character test (now CLI `escape_controls`); store: schema 3, ops, intent-to-add; CLI: flags, steps, outputs, backup; MCP: `mirror.rs`, three descriptions naming `spec approve`; the plugin.

## Data

**Config**, a new top-level key of `ProjectConfig` (core `project_toml.rs`), checked on load by every command (`specengine.toml:<line>: …`, exit 2):

```toml
[decision_records]
prefix   = "DEC"
dir      = "docs/records/DEC"
template = "templates/decision.md"
```

Three required strings, no other key. `prefix` an `[ids]` entry of shape `number`, scope `project` (else error; an `aliases_from` name → error naming the canonical), its `width`, `aliases_from` used. `dir`, `template` in `[paths]` grammar. Index fingerprint kept.

**Template**: UTF-8, ≤ 64 KiB, opening with a front-matter (`---` … `---`). Slots `{{name}}`, one left-to-right pass, values never re-scanned; list items joined by LF; a `{{` opening no slot → template error.

- Engine, anywhere: `id` (`DEC-0024`), `date` (`YYYY-MM-DD`, the run's `now`, UTC), `status` (`accepted`), `canon` (`--canon`, else the first ID target, canonical, reference form), `targets` (the ID targets, canonical, `[Q-031, RULE-STAM-REGEN]`, `[]` if none: a path is no reference), `proposal` (`PR-0004`).
- **Free text**, body only: `title` (the chosen label or the answer's first line, whitespace runs → one space, trimmed, ≤ 128 bytes, cut + `…`; also front-matter, `"…"`, `\` `"` backslashed), `choice` (label or answer), `effect`, `cost` (the option's price; `--answer`: `price_of_other`; working answer: empty), `summary`, `options` (`- <label> | <effect> | <price>`), `evidence` (`- <file>[:<lines>][ <qpath>] | <observed> | <documented>`; a question: empty, as `effect`, `options`), `note`, `decided_by` (the step-6 identity).

Free text but `title` in the front-matter → template error. A character `escape_controls` escapes but LF, TAB → exit 1 naming its field (`options[1].label: holds U+202E: a decision record never carries it; nothing changed`); rendered > `TEXT_MAX_BYTES` → exit 1. **Structure** (the recorded root's scheme): one front-matter, `class: decision`, `id:` the ID, `status: accepted`, defined IDs exactly the ID, `canon:` read back as the slot; a defect in a free-text value's or `--canon`'s output → exit 1 naming it, else template error.

**ID**: `<prefix>-<n>`, padded to `width` (one core function; past it `id-width` shows); `n` = 1 + the greater of the highest number of the prefix or its `aliases_from` (defined or in `aliases:`) in the recorded root's index refreshed at step 4 and the highest the queue issued (`record_id`). Previewed at step 4, issued at step 7 in the queue's transaction, kept by its proposal across a reopen, never another's; gaps allowed.

**Queue** (`QUEUE_SCHEMA_VERSION` 2 → 3, one `Immediate` transaction): `ADD COLUMN … TEXT` `record_id`, `record_path` (root-relative), `record_title`, `record_text` (the rendered bytes), `choice` (JSON `{"option":1}`, `{"working_answer":true}`, `{"answer":"…"}`); `PROPOSAL_COLUMNS` 35 → 40. Set together at step 7, replaced by a later one, kept by `reopen_from`. Corrupt, named: any on an `update`; set partly; a `choice` of another shape or kind or out of range; `record_path` not clean; an `approved`/`applied` deciding row without them. Events `.approved`, `.applied` add `record`.

Store: read-only `next_record(&RecordSeries {prefix, width, corpus_max})`; `approve_record_from(id, seen, &RecordApproval {series, preview, path, title, text, choice}, decision, now)`: `approve_from`'s compare-and-set, the ID recomputed under the lock, ≠ `preview` → new `QueueError::Issued {next}`. `create`, `approve_from`, `applied_with` of a deciding kind without a record → `Invalid`. `QueueMatch` + `record_id`, `record_path`, `record_title`.

**Review document**: after `linked`, the five (`choice` an object), absent `null`; text: `record_text` a block, `choice` compact JSON; `--brief`: `record_text` `null`. **Inbox**: + `record_id`; an applied one's last column ends ` [<record_id>]`. **Intake match**: + `record` (a corpus hit's `id:`, a queue hit's `record_id`); a queue hit with one: `path`, `answer` its `record_path`, `record_title`. **Backup**: `queue_schema` 3, 40 columns, `STATE_FORMAT` 1; `proposal_columns`, import: 1 (24), 2 (35), 3, missing columns NULL; a schema-1 or -2 DB exports as 3, unmigrated.

**Plugin** (root `README.md` "Claude Code plugin", "Version": in this change). `ask-owner` teaches: a queue hit with a `record` is the owner's decision: `get_node` on it, off this branch `get_proposal` (`choice`); one with only an `answer`: rejected, the owner's answer; an applied item's `get_proposal`: `record_id`, `record_title`, `choice`, `decision_note`; `choice` answers too. `plugin.json` 0.1.3 (PATCH), `PINS` + its pin, `plugin_skills.rs` AC-11's `ask-owner` names + `record_id`.

## Rules and edge cases

**First** (after `main`'s terminal check): `applied` → exit 1 naming its commit and `record_id`; `rejected` → exit 1. Exit 2: `--option` off a discrepancy, `--answer` off a question, `--canon` on an update; an `open` discrepancy without `--option` (``spec: `PR-0004` is a discrepancy: name the owner's choice with `--option N` (0-2); nothing changed``); no ID target, no `--canon` (``spec: `PR-0004` names no ID: name the section its record governs with `--canon REF`; nothing changed``); `--canon` outside core `parse_canon`; `--answer` blank. Exit 1: `--option` ≥ the count (``--option 7: `PR-0004` has options 0-2; nothing changed``); `--answer` > 2 048 bytes, `--canon` > 512, a refused character. An `approved` deciding row (stopped after step 7) takes only `--note`, writes its `record_text`. Then the completion lookup, then the steps, logged, reopened as an update's (`docs/canon/proposal-apply.md` "Apply steps"):

- **2** Place: an update's.
- **3 Config**: the recorded root's `specengine.toml`, slug the queue's; no table → exit 2 ``spec: <root>/specengine.toml has no `[decision_records]` (`prefix`, `dir`, `template`) for `PR-0004`'s record: add it, or `spec reject PR-0004 --reason <answer>`; nothing changed``. Template: no symlink component, regular, tracked, clean, UTF-8, ≤ 64 KiB, slots known and placed; else exit 2 `<template>:<line>: <problem>`.
- **4 ID, path**: index refreshed, the preview; `<dir>/<ID>.md` in the walk (else exit 2 naming `dir`); no existing component a symlink or non-directory, nothing at the path (a dangling symlink too) or in git's index, else exit 1 `` `<path>` exists: a decision record never replaces a file; nothing changed``.
- **5 Render**, **6 Structure**; the tree checked as is and with the record, judged against `HEAD`: `introduced: <n>`, findings above the prompt; the identity as an update's.
- **1 Consent**: stderr `record <ID> at <path>:`, the record escaped, indented two spaces, then `apply PR-0004 as <ID> (option 1 | the working answer | the given answer) on <branch> in <worktree>? [y/N]`.
- **7** `approve_record_from`; `Issued` → exit 1 `` `<ID>` was issued meanwhile (next `<ID'>`): run `spec approve PR-0004` again; nothing written``.
- **8 Write**: place, step 4 re-checked (exit 1); missing `dir` directories made one by one, never via a symlink; an update's temporary name, created new, synced, hard-linked to the path (`link` never replaces: an entry appearing meanwhile → exit 1, untouched), removed; the directory synced.
- **9 Commit**: `git add --intent-to-add -- :(literal)<path>`, store `commit_only`; body `<ID>: <title>`, four trailers. Failed without this ID's commit: `git rm --cached --quiet --ignore-unmatch -- :(literal)<path>`, the file and this run's directories (innermost first, if empty) removed, reopened, exit 1 ``the commit of `<path>` failed, the record removed: <git's error>``.
- **10 Verify**: one parent, the old `HEAD`, name-status exactly `A <path>`, its blob `record_text`, `Proposal:` the ID → `applied_with`, `update_paths`; stdout `applied PR-0004 as <sha> on <branch>: <ID> <path>`.

**Canon** (D1): from an ID only: a reference-form `canon:` passes wherever the ID is defined (`spec-check.md` "References"). A path target's id-less document would need a `#anchor` in a walked `class: canon` document (`canon-form`, `-file`, `-anchor`), rare in an overlay: derived, a guess or a known error; empty, `canon-missing`, refused by a hook at step 9. §5 wants the exact section governed; where no ID names it, the owner does. `--canon` (`ID`, `ID#SECTION`, `path#anchor`) also overrides; the check judges it (Q5).

**Notes**: a linked update not applied → `` `PR-0005` (linked) is <status>: `spec approve PR-0005` changes <canon> ``; none → ``<ID> names <canon> in `canon:`, its text unchanged: `spec propose update` changes it``. **Completion**: the proposal's own commit (trailer, one parent, exactly `A <record_path>`, its blob `record_text`), never re-rendered; else `changes a, b`, `does not carry the record`.

**Known limits**: a record hand-written under the issued ID elsewhere → `id-taken` at merge; no literal `{{` in a template; free text with a refused character only rejected; a queue restored from before step 7, the record committed: neither approve nor reject takes it; a path target meets its record in corpus dedup only if `canon:` lands on it.

## Acceptance criteria

Setup: temp repos of `fixtures/spec-a` (`[decision_records]` `DEC`, `docs/records/DEC`), `fixtures/spec-b` (`ADR`, `docs/records/ADR`), each `templates/decision.md` committed (outside the roots; every slot; `title` in front-matter and H1; `canon: {{canon}}`, `links:` `answers: {{targets}}`, `ref: {{proposal}}`); scratch `HOME`; clock `2026-10-05T12:00:00Z`; a git identity; items raised with `distinct_from` naming their hits; library approve, consent yes. Refused: nothing written, the row as before. M: the mutation turning it red.

- [ ] AC-01 — spec-a discrepancy on `RULE-STAM-REGEN`, 3 options, a tracked file modified, `--option 1`: one commit, one parent, name-status exactly `A docs/records/DEC/DEC-0024.md`, body `DEC-0024: <label 1>`, four trailers; `status: accepted`, `date: 2026-10-05`, `canon: RULE-STAM-REGEN`, option 1's label, effect, price; the other file still modified; check errors as before; `applied` (M: staging the whole tree).
- [ ] AC-02 — question on `Q-031`, no flag: the working answer, `cost` empty; `--answer T`: `T`, cost `price_of_other` (M: cost always `price_of_other`).
- [ ] AC-03 — spec-b, Cyrillic labels, `--option 0` → `docs/records/ADR/ADR-0003.md`, labels verbatim; new sources hold no literal equal to an `[ids]` prefix or kind of spec-a, spec-b, the root config (class names aside), `Cost`, `docs/`, `records/` (M: `"DEC"`).
- [ ] AC-04 — no `[decision_records]`: exit 2 naming it; worktree, git index, `HEAD` unchanged; `open`, one `apply_failed` (step 3), `next_record` unchanged; `reject --reason` works (M: a built-in template).
- [ ] AC-05 — `DEC-0007`, `DEC-0023` → `DEC-0024`; a second worktree on a branch without it → `DEC-0025`; a declined prompt: `next_record` unchanged (M: issuing from the worktree only).
- [ ] AC-06 — label `x\nstatus: rejected` → `status: accepted`, one `title`; effect `## X {#RULE-STAM-REGEN}` → exit 1 naming `options[1].effect`, no prompt; label `{{id}}` literal; U+202E in `summary` → exit 1 (M: raw substitution; a re-scan).
- [ ] AC-07 — stdin a pipe → exit 2 before reading; consent `n` → exit 1, path absent, `git ls-files --stage`, `git status --porcelain` as before, `record_id` NULL (M: the ID before the prompt).
- [ ] AC-08 — `dir` a missing `docs/records/DEC/new`, a pre-commit hook exiting 1 → exit 1; file and `new/` gone, `git ls-files --stage -- <path>` empty; `open`, `record_id` kept, one `apply_failed` (step 9); no hook → the same ID, applied (M: the intent-to-add entry left).
- [ ] AC-09 — the consent callback writes X at the path, then yes → exit 1, X intact, no commit or temporary; the store's create-new on an existing path fails, bytes intact (M: rename over the path).
- [ ] AC-10 — `approved` at step 7, its commit made by hand from `record_text`: approve → `applied`, no new commit or prompt; another `date:` → `does not carry the record` (M: re-rendering).
- [ ] AC-11 — after AC-01, the same summary on `RULE-STAM-REGEN` from the recorded root: `created: false`, hits corpus `DEC-0024` and queue `PR-0001` (`applied`, `path` the record, `answer` the title, `record` `DEC-0024`); `counts()` unchanged; `get_proposal` = `review --brief --json` with the record keys (M: the answer from `decision_note`).
- [ ] AC-12 — every MCP tool on a decided item: `git status --porcelain` empty, no tool added; `export state` `queue_schema` 3, 40 columns; schema 1, 2 dumps restore, re-export byte-identical as 3; a v2 DB: `user_version` 3; gate clean, worst W ≤ 109 484 (M: a column missing).
- [ ] AC-13 — same item, choice, note, clock, identity in two fresh repos → byte-identical records and messages (M: the wall clock).
- [ ] AC-14 — exit 2: `--option` on a question or update, `--answer` on a discrepancy, both, a discrepancy without `--option`, a question on `docs/features/stamina-tuning.md` without `--canon`, `--canon "a b"`; `--option 7` of 3 → exit 1 naming `0-2`; no event (M: an out-of-range option accepted).
- [ ] AC-15 — that question, `--canon docs/spec/movement/stamina.md#regeneration` → that `canon:`, `targets` `[]`, no `canon-*` introduced; the bare path (fresh copy) → `canon-form` shown, written; `Q-031`, `--canon MEC-STAMINA#RULE-STAM-REGEN` → it (M: a path by default).
- [ ] AC-16 — `ask-owner` as Data "Plugin"; `plugin_skills.rs`, `plugin_files.rs` green at `0.1.3`; the three descriptions name `spec approve`, `INSTRUCTIONS` unchanged (M: the skill unedited; no bump).

## Out of scope

Kinds `create`, `decision`, `interpretation`, `amendment` (`proposal-kinds`); a linked update in the same commit; rounds; `changes_requested`, defer, `superseded`; follow-up tasks, `@assumes` (`task-package`); superseding a record; `spec new`; the homoglyph fix; `review_proposal`; UI; this repository's ADRs; `.claude/`; pilots.

## Implementation

Pending. At shipping: a new `docs/canon/decision-record.md` (the queue canons and crate READMEs are full), pointed at from them; the root `README.md` `ask-owner` row, "Version".
