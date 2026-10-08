---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli]
ref: round analysis 2026-10-08 (HEAD 8e53d8c), working answers taken by the orchestrator for the owner's review; 06 s5, 07 s2, 08 s2 Phase 2
adrs: [ADR-0038]
---

# Question round

## Why

Agents keep working on their working answers (ADR-0012), so open questions and discrepancies pile up across tasks, and the owner settles them one `spec review` and `spec approve` at a time, in the engine's words, on a terminal. The owner wants them on one sheet in the project's own language, ordered by what matters, to print, mail or read on a screen, answered in one sitting, while each answer is still confirmed by its own key (ADR-0035). 06 s5 planned the round; this slice builds both CLI parts on shipped machinery: intake fields, stages, decision records. No new kind, no new storage.

## Description and interactions

- **`spec round new [--limit N] [--json]`**: read-only, byte-deterministic sheet of the current repository's open questions and discrepancies: `docs/canon/round.md` "Selection", "Order", "Sheet", "JSON".
- **`spec round answer FILE`** (a filled sheet or a reply) and **`spec round answer --staged`**: terminal only; parse, check against the queue now, report, stage, then each item's own question: "Answers", "Run" (ADR-0038).
- Reused, unchanged: `spec inbox`'s repository scope (`proposal-queue.md`), intake fields (`agent-intake.md` "Stored"), the package's `open_proposals` rule read the other way (`task-package.md` "Package"), the stage library and the staged approve/reject flows (`decision-staging.md` "Daemon", "Terminal"), record steps (`decision-record.md` "Steps").
- Agents: nothing new. No MCP tool, no plugin skill (`round` stays reserved), no gate change; `get_proposal` shows a round's stage, `ask-owner` counts it as no answer.
- The UI: `ui-round`, a later slice (Out of scope), serves `round new --json` and stages a pasted reply through steps 2-4 only.

## Data

Formats are canon: the sheet's Markdown and its JSON item (19 keys), the grammar's table, the report lines, the run's JSON (`docs/canon/round.md`). No queue schema change (5 stays); events: one `proposal.staged` per staged answer, then approve's and reject's own.

| Part | Contract |
|---|---|
| core `round` (new, pure, no IO) | `ROUND_LIMIT_DEFAULT` 20, `ROUND_LIMIT_MAX` 200, `ROUND_FILE_MAX_BYTES` 8 388 608 (`INTAKE_INPUT_MAX_BYTES`'s value); the order key (severity rank, waiting tasks, `created_at`, ID number); `parse_answers(&str)` -> project line(s) and answer lines `{line, id, token}`, token `Open` (blank, `?`), `Ok`, `Option(n)`, `Reject(reason)`, `Text(t)`, a malformed ID-like line with its Latin form; duplicates and caps; `reading_minutes(chars)` |
| CLI `round` (new) | `round_new(env, globals, limit)` -> the sheet document; `round_stage(env, globals, text, now)` -> the run document with `walk` `[]` (steps 2-4: the daemon's later POST); `round_answer` (steps 1-6) driving `approve`/`reject` with no typed flag |
| CLI `main.rs` | the terminal check before the file is opened, as `approve`'s |
| store | existing ops only (listing, tasks, `stage_from`, approve and reject ops) |

Rust names are the developer's; texts, keys, order and exit codes are binding.

A reply (only line 1 and the answer lines count):

```
project: game
PR-0003: ok
PR-0005: [1]
PR-0006: Regeneration waits 2 s, not 1.5
PR-0007: reject: duplicate of PR-0003
> PR-0008: ok
  PR-0009: 1
```

Report, then four questions on stderr, then the summary (all `y`):

```
PR-0003 line 2: staged: approve, the working answer
PR-0005 line 3: staged: approve, option 1: keep the system's
PR-0006 line 4: staged: approve, your answer
PR-0007 line 5: staged: reject
answers: 4 staged, 0 left open, 0 refused
round: 3 applied, 1 rejected, 0 kept staged, 0 refused
```

## Rules and edge cases

- WHEN stdin is not a terminal THEN `spec round answer` SHALL exit 2 before the file or the queue is opened (ADR-0038: agents never stage through it). Checked before staging, never after.
- WHEN one answer is refused (stale, unknown, a look-alike, an update's ID, a cap, an escaped character, a duplicate) THEN only it SHALL be named and skipped; the rest go on. Only the file, the project line and the terminal stop the run (exit 2, nothing staged, no event).
- WHEN the owner answers anything but `y` THEN the item SHALL stay `open` with its stage; WHEN its stage changes between step 4 and `y` THEN step 7 SHALL refuse it, named, and the next is asked.
- `round new` writes nothing under the root, nothing in the queue, no event; `round answer` writes stages, then only what approve and reject write (ADR-0004, ADR-0005, ADR-0032; a record and a commit where raised).
- Queue, index and task text never starts a sheet line; one-line slots escape LF and CR. Frame words fixed English (ADR-0014, ADR-0024); the project's text verbatim, non-Latin included.
- Domain-free (ADR-0008): no node-kind or ID-prefix literal in the round's sources; no word starting `block` in its texts or keys.
- A stage blocks nothing; task approval stays the only control point (ADR-0012, `architecture.md#control`).
- Before staging an approve, apply steps 2 and 3 run read-only: no `[decision_records]` or a gone worktree refuses every approve, named; `reject:` still settles.
- 06 s5 is superseded where it differs: s5.3's batch confirmation (ADR-0038); s5.2's created R/A records, cancelled assumptions and rephrasing (Out of scope).

## Acceptance criteria

Fixtures and queues are built per test in a scratch `HOME` and git repository; terminal runs under a pseudo-terminal.

- [ ] AC-01 - Open: question Q1, discrepancy D1; also an `approved` (held) question, a rejected and an applied one, an orphan's, another repository's (same slug), an open `update`, an open `create`. `spec round new --json` lists exactly Q1, D1; `notes` name the held one, the orphan, the other repository's and the two change proposals; the rejected and applied IDs appear nowhere (M: list `approved` items).
- [ ] AC-02 - H severity `high`; N1 `normal`, newer, its target in a `ready` task's snapshot; N2 `normal`, older, its target only in a `done` task's: text and JSON order H, N1, N2 (M: count `done` tasks).
- [ ] AC-03 - Two runs, text and `--json`, with the injected clock 1 h apart: byte-equal; `git status --porcelain`, `HEAD` and the queue's event count unchanged (M: print the time or an age).
- [ ] AC-04 - On `fixtures/spec-b`: a question's Cyrillic summary and a discrepancy's Cyrillic labels appear byte-equal, with the target titles; the text holds no `b3:`, worktree path, branch name or `gap_type` value; a genre test finds no node-kind or ID-prefix literal and no `block` in the round's sources (M: print the worktree).
- [ ] AC-05 - 25 items: 20 listed, note `5 more item(s) not on this sheet: spec round new --limit 25`, JSON `more` 5; `--limit 0`, `--limit 201` exit 2 (M: drop the note).
- [ ] AC-06 - Q1's summary holds a line `PR-0009: 1` (PR-0009 an open discrepancy): the sheet, fed back unedited to `spec round answer`, stages nothing: no `proposal.staged`, no question, every ID `left open`, exit 0 (M: agent text not indented).
- [ ] AC-07 - One file: `ok`, `1` (a discrepancy), own text, `reject: dup`, `?`, blank, an indented and a quoted `PR-0006: ok`, a pair `PR-0007: ok` / `PR-0007: no`: the report names each; exactly four staged (`spec review` after declining all: working answer, option 1, the text, the reject); PR-0006 untouched; PR-0007 refused naming both lines. A 2 049-byte answer and one holding U+202E: refused, named, nothing staged (M: the last duplicate wins; truncation).
- [ ] AC-08 - A discrepancy, options 0-2: `ok`, free text and `5` each refused naming `0-2`, nothing staged; `[2]` and `2` stage option 2 (M: `ok` takes the recommendation).
- [ ] AC-09 - Answers to an item applied after the sheet, an open update's ID, `PR-9999`, a U+0420 look-alike of `PR-0003`, and two valid ones: the four named (the commit; `spec approve`; `no proposal`; `PR-0003`), not staged; the two staged and asked (M: one stale item refuses the file).
- [ ] AC-10 - Stdin a pipe, `FILE` a FIFO with no writer: exit 2 at once with the consent message, also with `--staged`. On a terminal: a sparse file of 8 388 609 bytes: exit 2 naming the cap; `project:` missing, or another slug: exit 2, no stage, no event (M: the terminal checked after staging).
- [ ] AC-11 - Three answers, keys `y`, `n`, `y`: three questions, each `staged)? [y/N]`; two records in two `spec: apply` commits, each name-status `A <path>`; the second item `open` with its stage; summary `2 applied, 0 rejected, 1 kept staged`, exit 1 (M: one question for the round).
- [ ] AC-12 - Two answers; before the first `y` the test restages the first item: step 7 refuses it, named; the second is asked and applied (M: the walk stops).
- [ ] AC-13 - `--staged`, two staged questions (staged in reverse round order) and a staged `update`: two questions in round order; the update named in a note, not asked (M: the update is asked).
- [ ] AC-14 - ADR-0038 <= 1 536 B with "Cost", `canon:` `docs/canon/architecture.md#apply`, whose text names one `[y/N]` per decision and `spec round answer`; `decision-staging.md` "Rule" names it; `export index` then `check` clean (M: an ADR over the cap).

## Out of scope

`ui-round`, the next Phase 4 slice: `GET .../round` (= `round new --json`), `POST .../round/answers` (steps 2-4 only, behind the decision route's fence; `door.rs` admits it as staging only), the Questions screen (sheet, Print, Copy, paste, outcomes, the `--staged` command), print tokens in `tokens.css`; its AC-15, AC-16. Also: one consent for a round; stored rounds; `update` and `create` items; clarification, defer, `interpretation`; R or A records, `@assumes`, follow-up tasks; MCP tools; the `round` skill; natural-language parsing; multi-line answers; `--note`, `--canon` in the grammar.

## Open

1. **ADR-0038 stays accepted; the owner's explicit review is open.** It rests on working answers: one key per answer in one run, confirmed inside `round answer`. Another answer is a new ADR superseding it.
2. Working answers for the owner: nothing stored (no round table, the file not kept); options keyed as printed (`[1]` or `1`); English frame words; `ok` and `reject:` any case; a blank slot is no answer (no duplicate); 8 MiB file cap; exit 1 when anything is refused or kept.
3. `ui-round`: N `proposal.staged` events from one paste against the outside-change alert's one per write; the 16 384-byte body cap against a quoted reply.

## Implementation

Not built. At shipping, besides the canon's "Not built yet" marker: 07 s2 (`spec round new [--limit N] [--json]`, `spec round answer FILE | --staged`); 06 s5.1-5.3 shortened to a pointer to `round.md`, s5.3's batch confirmation replaced by ADR-0038's one key per answer; 08 s2 (`round` shipped; `ui-round` next); `proposal-apply.md` "Consent" lists `spec round answer`; CLI and core READMEs; `CLAUDE.md` State. For the developer: `crates/specengine-cli/src/stage.rs`'s module doc says no `spec` command stages; with ADR-0038, `spec round answer` stages before asking.
