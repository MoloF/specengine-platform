---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-10-08
---

# Question rounds

ADR-0038 under ADR-0035 (`docs/canon/architecture.md#apply`): a batch front end on shipped machinery, no new kind, no new storage. `spec round new` prints the open questions and discrepancies on a sheet; `spec round answer` stages each returned answer, then asks its item's own `[y/N]`. Core `round`: bounds, order key, grammar, reading time; CLI `round.rs`: commands over `stage_with`, `record_ready`, `lists_proposal`, store ops. Daemon, UI: `round-ui.md`; `round_stage` (text) stays for a later paste.

## Selection

`spec round new [--limit N] [--json]`, read-only. Items: `open` proposals whose kind `decides()` (question, discrepancy) of the current repository (`spec inbox`'s scope, `proposal-queue.md` "Place, IDs, repositories"); a staged one is listed with its stage. The rest are `notes` (text: closing `note:` lines), IDs named up to 10, then ` and <k> more`:

- ``<n> more item(s) not on this sheet: spec round new --limit <m>`` (`m` the total, at most 200), first;
- ``<n> held item(s) not on this sheet (approved: an apply in progress or stopped): <IDs>; spec approve <ID> completes each``;
- ``<n> open change proposal(s) not on this sheet: <IDs>; spec inbox lists them`` (applying kinds);
- ``<n> item(s) of another repository of the project `<slug>` not on this sheet: <IDs>; run `spec round new` there`` (open or held);
- ``<n> item(s) of a repository that no longer exists not on this sheet: <IDs>; spec reject <ID> --reason ... takes one out``;
- a corrupt row as `spec inbox` words it + `; not listed` (a task's: `; not counted among the waiting tasks`).

Settled ones (`rejected`, `applied`) are neither listed nor noted. `--limit` 1..=200, default 20, else exit 2 `--limit N: the limit is 1 to 200`. Nothing is stored; the index refreshed as by every read.

## Order

1. `severity` `high`, `normal`, `low`, then none;
2. waiting tasks, most first: the repository's tasks neither `done` nor `cancelled` whose package `open_proposals` would list the item (`lists_proposal`, `task-package.md` "Package": bound to the task, or `target_ids` meet the snapshot, before approval what it would freeze);
3. `created_at`, oldest first; 4. the ID's number.

One database and tree state give byte-identical text and JSON: no clock, no relative age, no read time.

## Sheet

Markdown on stdout (print, mail). Fixed English frame words; every text from the queue, the index or a task verbatim through `escape_controls` (`agent-intake.md` "Escaping and size"), U+2028, U+2029 written `\u{2028}`, `\u{2029}`. **Queue, index or task text never starts a line**: a multi-line text is printed one line per LF, each indented two spaces; a one-line slot (title, label, effect, price) follows a frame word, LF and CR written `\n`, `\r`. So a text holding `PR-0009: 1` cannot plant an answer line.

```
# Open questions: <slug>

project: <slug>

1 item, about 1 min of reading. Answer each on a line of its own, at the start of the line: `PR-0003: ok`. ok takes the working answer, a number takes that option, `reject: <reason>` rejects, ? or nothing leaves it open, other text is your own answer to a question. Keep the project line. Then, in a terminal: spec round answer <file>; each answer is asked again there, one at a time.

## PR-0003 - question, high

  Does regeneration wait for rest?

Working answer:
  yes, 1.5 s

Concerns: Stamina (MEC-STAMINA); Regeneration (RULE-STAM-REGEN)
Waiting: Tune stamina (T-0004)
Staged 2026-10-07T17:49:50Z: spec approve PR-0003 --answer "no"

PR-0003:
```

- `## <ID> - <kind>[, <severity>]`. A question: its text, `Working answer:`, `Another answer costs:`. A discrepancy: its summary, `Working answer:` when given, `Options:` as `spec review` prints them (`  - [i] label | effect | price`, ` (recommended)`).
- `Concerns:` each target `<title> (<ID or path>)`, `; `-joined, resolved now; gone: `<ID or path> (not found now)`. `Waiting:` order 2's tasks by ID, `<title> (<task ID>)`, 10, then `; <k> more`; absent with none. `Staged <at>: <the staged line>` (`decision-staging.md` "Terminal").
- The blank slot `<ID>:` at column 0 ends each item; the unedited sheet answers nothing.
- Reading time `ceil(c / 1000)` minutes, `c` the characters of the item blocks; `1 item`, `<n> items`. None: `No open questions or discrepancies.`, the notes, exit 0.
- No worktree, branch, hash, `gap_type`, author, evidence in the text; no word starting `block`.

## JSON

`--json`: `{"project", "items", "more", "minutes", "notes"}`; an item's 19 keys in this order, raw strings as stored, absent `null`, lists `[]`: `id, kind, severity, gap_type, summary, working_answer, price_of_other, options [{label, effect, price}], recommendation, evidence [{file, qpath, lines, observed, documented}], targets [{id, path, title}], tasks [{id, status, title}], worktree, branch, author {type, role, model, run}, staged, staged_at, created_at, updated_at`; a target's `id` `null` for a path, `title` when gone; `tasks` all. `more`: the items past `--limit`; `minutes` 0 with none.

## Answers

The file: regular (else `<file>: not a regular file: give the filled sheet or a reply as a file`), at most 8 388 608 bytes (`ROUND_FILE_MAX_BYTES`, `--input`'s; else `<file>: <n> bytes; at most 8388608: return only the answer lines`), judged before the open, opened `O_NONBLOCK`, judged again on the handle before any byte; UTF-8 (else `<file>:<line>: not UTF-8`); a leading U+FEFF and each line's trailing CR dropped. Ignored: blank lines, lines starting with whitespace or `>`, every other line but two:

- **project line**: column 0 `project:`, the slug trimmed. Exactly one, naming this project; else exit 2, nothing staged, no event: ``the answers name no project: keep the line `project: <slug>` ``, ``the answers are for `<x>`, this is `<slug>` ``, ``the answers name a project on lines <lines>: keep one line `project: <slug>` ``.
- **answer line**: column 0 `PR-<digits>:`, then the answer trimmed. A column-0 line reading `PR-<digit>` once leading invisible characters are skipped, case and look-alikes made Latin, but not exactly `PR-<digits>:`, is refused, never ignored: ``` `<written>` uses look-alike letters or digits; proposal IDs are Latin only: write `<Latin ID>:` at the start of the line ``` (none: `` `<written>` is no answer line: write ... ``; invisible characters written `\u{..}`). `PR-1:` -> `no proposal`.

| Answer | Question | Discrepancy |
|---|---|---|
| blank, `?` | left open | left open |
| `ok` (ASCII, any case) | approve, the working answer | refused, naming `0-2` |
| `N` or `[N]`, ASCII digits | refused: no options | approve `--option N`; out of range: ``option 5: `PR` has options 0-2`` |
| `reject: <reason>` (any case) | reject, the reason | reject, the reason |
| `reject`, a blank reason | ``needs a reason: write `reject: <reason>` `` | same |
| `reject` not directly followed by `:` | ``takes its reason after a colon right behind it: write `reject: <reason>` `` | same |
| other text (`rejected ...` too) | approve `--answer`, the text | ``takes one of its options by number, 0-2, or `reject: <reason>` `` |

- One line per answer; `ok` never takes a recommendation. A blank slot is no answer; two or more other lines for one ID (`?` too; a refused malformed line for the ID its Latin form names) refuse every one: `answered on lines 4 and 9: give one answer per item`, at most 10 lines named (`2, 3, ..., 11 and <k> more`; the project lines' refusal alike).
- Caps and characters of a stage (`decision-staging.md` "The stage"): `the answer: <n> bytes; at most 2048`, `the reason: <n> bytes; at most 4096`, `holds U+202E: an answer never carries it`; never cut.
- Then against the queue now, each refused alone: `no proposal`; another repository's, an orphan's (only `reject:` passes) as `spec approve` words them; `` `PR` is applied: commit <sha> ``, `is rejected`, `is held (...)`; ``is a change proposal (update): `spec approve PR` decides it``; for an approve, only path targets (``needs --canon: `spec approve PR --canon REF` ``) and apply steps 2 and 3 read-only (`record_ready`: the place, `[decision_records]`, its template, its prefix among `[ids]`): ``not applied (step 3): <reason>; `reject: <reason>` still settles it`` (`queue_checked`'s bare reason + this tail); a corrupt row, its own error.

## Run

`spec round answer FILE` or `spec round answer --staged` (both or neither: exit 2):

1. stdin not a terminal -> exit 2 before anything is read or logged, the file unopened, with `proposal-apply.md` "Consent"'s message naming `spec round answer`. Agents never stage through it.
2. The file, the grammar, the project line, the queue checks ("Answers"). The clock is read once per run. Exit-2 texts start `<file>:` or `<file>:<line>:` (`round_stage`, a text: `line <n>:`) and end `; nothing changed`.
3. Stage each, round order, as the decision route stages (`stage_with`, compare-and-set on this run's read; one `proposal.staged` each); one lost is refused.
4. The report, stdout, file order: `<ID> line <n>: staged: approve, the working answer` (`option 1: <label>`, `your answer`; `reject`) + `; replaces the choice staged <at>`; `left open`; `refused: <reason>; nothing staged`; then `answers: <s> staged, <o> left open, <r> refused`.
5. The walk, stderr, round order, what step 3 staged: each item's own `spec approve|reject PR` with no typed flag (`decision-staging.md` "Terminal"), `note: this answer replaces the choice staged <at>` above a replaced one, `... staged) on <branch> in <worktree>? [y/N]` (reject: `..., staged)? [y/N]`). `y`: the command's own lines, one record (`decision-record.md`) and one `spec: apply PR-...` commit where raised, or a reject. Else `kept staged: spec approve <ID> or spec round answer --staged confirms it` (`spec reject`; a gone repository's: `kept staged: spec reject <ID> confirms it`), `open` and staged. A stage replaced since step 3 is refused unasked (`` `PR` changed since this round staged it: its staged choice was replaced or removed ``), one replaced while asked by step 7; refusals print `<ID>: refused: <reason>` and the walk goes on. End of input keeps the rest, each named.
6. `round: <a> applied, <j> rejected, <k> kept staged, <f> refused`. Exit 0 with nothing refused or kept; 1 otherwise; 2 cannot run.

`--json`: one document after the walk (questions on stderr): `{"project","answers":[{"id","line","answer","outcome","stage","reason"}],"walk":[{"id","outcome","commit","record","reason"}],"notes"}`; `outcome` `staged|open|refused`, then `applied|rejected|kept|refused`; `stage` `null` unless this run staged it; a kept item's `reason` its kept line, after `end of input: ` when input ended.

**`--staged`**: no file, no report, no stage written; steps 1, 5, 6 over the repository's `open` deciding items holding a stage, round order. Noted, never asked: ``<n> staged change proposal(s) not asked: <IDs>; spec approve <ID> confirms each``; ``<n> staged item(s) of another repository of the project `<slug>` not asked: <IDs>; run `spec round answer --staged` there``; ``<n> staged item(s) of a repository that no longer exists not asked: <IDs>; spec reject <ID> confirms each``; a corrupt row + `; not asked`.

## Known limits

- Consent fatigue: a question per answer, each record whole (`--limit`; `n` keeps the stage).
- A planted column-0 line in a pasted reply is staged before it is read: shown whole, step 7 compared, characters and caps refused; confirmed, it is the owner's. The rendered sheet is no sandbox: only file text is parsed.
- Type-ahead: a line typed ahead answers the next question (only exact `y`, `yes` consent); flushing needs a new pinned dependency, an ADR.
- A U+2010, U+2011 hyphen or an invisible character inside an ID: no answer line, ignored.
- The sheet is not cut at `OUTPUT_CAP_CHARS` (200 items ~2 MB); a returned one over 8 MiB: return the answer lines. A debug build takes ~7 s at the cap.
- `O_NONBLOCK`: a constant per target, no `libc` (04 s6; the owner's call); elsewhere 0, the metadata read the only guard.
- Intake takes CR in a question's text, a record refuses it (`holds U+000D`): its approve stages, never applies; `reject:` settles it (a follow-up task).
