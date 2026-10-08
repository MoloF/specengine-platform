---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-10-08
---

# Question rounds

**Not built yet: the `round` slice** (`docs/features/round.md`). ADR-0038 under ADR-0035 (`docs/canon/architecture.md#apply`): a batch front end on shipped machinery, no new kind, no new storage. `spec round new` prints a sheet of the open questions and discrepancies; `spec round answer` stages each returned answer, then asks that item's own `[y/N]`. Staging: `decision-staging.md`; records: `decision-record.md`; intake fields: `agent-intake.md`. Core holds the pure parts (selection key, order, grammar, reading time), the CLI the commands; the store's existing ops only.

## Selection

`spec round new [--limit N] [--json]`, read-only. Items: `open` proposals whose kind `decides()` (question, discrepancy) of the current repository (`spec inbox`'s scope, `proposal-queue.md` "Place, IDs, repositories"); a staged one is listed with its stage. Not listed, counted in `notes` (text: `note:` lines at the end): held ones (`approved`, an apply in progress or stopped) with their IDs and ``spec approve <ID>``; open `update` and `create` proposals (``spec inbox``); another repository's of the project (``run `spec round new` there``); an orphan's, with IDs and ``spec reject <ID> --reason ...``; a corrupt row as `spec inbox` notes it. Settled ones (`rejected`, `applied`) are neither listed nor noted. `--limit` 1..=200, default 20, else exit 2; past it, note ``<n> more item(s) not on this sheet: spec round new --limit <m>`` (`m` the total, at most 200). Nothing is stored; the index is refreshed as by every read.

## Order

1. `severity` `high`, `normal`, `low`, then none;
2. waiting tasks, most first: the tasks of the repository, neither `done` nor `cancelled`, whose package `open_proposals` would list the item (`task-package.md` "Package": `target_ids` meet the snapshot, before approval what it would freeze, or bound to the task);
3. `created_at`, oldest first; 4. the ID's number.

One database and tree state give byte-identical text and JSON: no clock, no relative age, no read time.

## Sheet

Markdown on stdout, for print, mail or the UI's renderer. Fixed English frame words; every text from the queue, the index or a task verbatim through `escape_controls` (`agent-intake.md` "Escaping and size"), U+2028 and U+2029 written `\u{2028}`, `\u{2029}`. **Queue, index or task text never starts a line**: a multi-line text is printed one line per LF, each indented two spaces; a one-line slot (title, label, effect, price) follows a frame word, LF and CR written `\n`, `\r`. So a text holding `PR-0009: 1` cannot plant an answer line.

```
# Open questions: <slug>

project: <slug>

2 items, about 2 min of reading. Answer each on a line of its own, at the start of the line: `PR-0003: ok`. ok takes the working answer, a number takes that option, `reject: <reason>` rejects, ? or nothing leaves it open, other text is your own answer to a question. Keep the project line. Then, in a terminal: spec round answer <file>; each answer is asked again there, one at a time.

## PR-0003 - question, high

  Does regeneration wait for rest?

Working answer:
  yes, 1.5 s

Another answer costs:
  R-28 rebalanced

Concerns: Stamina (MEC-STAMINA); Regeneration (RULE-STAM-REGEN)
Waiting: Tune stamina (T-0004)
Staged 2026-10-07T17:49:50Z: spec approve PR-0003 --answer "no"

PR-0003:

## PR-0005 - discrepancy, normal

  The cooldown is enforced twice.

Options:
  - [0] keep both | double wait | none
  - [1] keep the system's | one wait | edit RULE-CD-01 (recommended)

Concerns: Cooldown (RULE-CD-01)

PR-0005:
```

- Heading `## <ID> - <kind>[, <severity>]`. A question: its text, `Working answer:`, `Another answer costs:`. A discrepancy: its summary, `Working answer:` when given, `Options:` as printed by `spec review` (`[i] label | effect | price`, ` (recommended)`).
- `Concerns:` each target `<title> (<ID or path>)`, `; `-joined, resolved now; gone: `<ID or path> (not found now)`. `Waiting:` the tasks of order 2 by ID, `<title> (<task ID>)`, 10, then `; <k> more`; absent with none. `Staged <at>: <the staged line>` as `decision-staging.md` "Terminal" prints it.
- The blank answer slot `<ID>:` at column 0 ends each item; the unedited sheet answers nothing.
- Reading time: `ceil(c / 1000)` minutes, `c` the characters of the item blocks (heading through slot, LF included). No items: `No open questions or discrepancies.` and the notes; exit 0.
- Text only: no worktree, branch, hash, `gap_type`, author, evidence. None starts `block`.

## JSON

`--json`: `{"project", "items", "more", "minutes", "notes"}`; an item, keys in this order, absent `null`, lists `[]`:

```json
{"id":"PR-0005","kind":"discrepancy","severity":"normal","gap_type":"contradicts","summary":"...",
"working_answer":null,"price_of_other":null,"options":[{"label":"...","effect":"...","price":"..."}],"recommendation":1,
"evidence":[{"file":"...","qpath":null,"lines":"10-12","observed":"...","documented":"..."}],
"targets":[{"id":"RULE-CD-01","path":"docs/spec/combat/cooldown.md","title":"Cooldown"}],
"tasks":[{"id":"T-0004","status":"ready","title":"..."}],"worktree":"...","branch":"...",
"author":{"type":"agent","role":"...","model":null,"run":null},"staged":null,"staged_at":null,
"created_at":"...","updated_at":"..."}
```

Raw strings, as stored; `targets[].id` `null` for a path target, `title` `null` when gone; `tasks` every one (no cap); `more` the items past `--limit`; `minutes` 0 with none.

## Answers

The file: a regular file (else exit 2), at most 8 MiB, 8 388 608 bytes (`ROUND_FILE_MAX_BYTES`, `--input`'s), its size read before any byte (over -> exit 2 `<file>: <n> bytes; at most 8388608: return only the answer lines`), UTF-8 (else exit 2 naming the line), a leading U+FEFF dropped; lines split at LF, a trailing CR dropped. Ignored: blank lines, lines starting with whitespace or `>` (indented and quoted), every other line but these two:

- **project line**: column 0 `project:`, the slug after it trimmed. Exactly one, naming the current project's slug; missing, twice or another slug -> exit 2 (``the answers name no project: keep the line `project: <slug>` ``; ``the answers are for `<x>`, this is `<slug>` ``), nothing staged, no event. Proposal numbers repeat across projects.
- **answer line**: column 0 `PR-<digits>:`, then the answer trimmed. A column-0 line that starts `PR-<digits>` up to case or a look-alike (U+0420 for `P`) but is not exactly `PR-<digits>:` is refused, named with its Latin form, never ignored. An ID the queue never wrote (`PR-1`) -> `no proposal`.

| Answer | Question | Discrepancy |
|---|---|---|
| blank, `?` | left open | left open |
| `ok` (ASCII, any case) | approve, the working answer | refused, naming the options `0-2` |
| `N` or `[N]`, digits | refused: no options | approve `--option N`; out of range refused naming `0-2` |
| `reject: <reason>` (any case) | reject, the reason | reject, the reason |
| `reject` alone, a blank reason | refused, naming the form | refused |
| other text | approve `--answer`, the text | refused, naming `0-2` |

- An answer is one line; ok never takes a recommendation. A blank slot is no answer: two or more non-blank lines for one ID refuse them all, named with their lines, nothing staged for it.
- Caps and characters as a stage's (`decision-staging.md` "The stage"): an answer over 2 048 bytes, a reason over 4 096, a character the queue escapes (U+202E, U+0007), each refused and named, never cut.
- Then checked against the queue now, each refused and named, the rest going on: another repository, an orphan (only `reject:`), no longer `open` (applied: its commit; rejected; held), not a deciding kind (``an update: `spec approve PR-0012` decides it``), a question on a path alone (``needs --canon: `spec approve PR --canon REF` ``), and, for an approve, apply steps 2 and 3 read-only (a gone worktree, no `[decision_records]`, a bad template: `reject: <reason>` still settles).

## Run

`spec round answer FILE` or `spec round answer --staged` (both or neither: exit 2), in order:

1. stdin not a terminal -> exit 2 before anything is read or logged, the file unopened: ``spec: `spec round answer` asks the owner for consent on a terminal, and stdin is not one (a pipe, a script or an agent's shell): run it in a terminal; nothing changed``. Agents never stage through it.
2. The file, the grammar, the project line, the queue checks ("Answers").
3. The report, stdout, file order: `<ID> line <n>: staged: approve, the working answer` (`option 1: <label>`, `your answer`; `reject`) + `; replaces the choice staged <at>`; `left open`; `refused: <reason>; nothing staged`; then `answers: <s> staged, <o> left open, <r> refused`.
4. Stage each, round order, as the decision route stages (`stage`, compare-and-set on this run's read; one `proposal.staged` each); a lost one refused, named.
5. The walk, round order, only what step 4 staged: each item's own `spec approve|reject PR` with no typed flag (`decision-staging.md` "Terminal"): the stage line (a replaced stage named again), the option line or the record preview, introduced findings, `... staged)? [y/N]`. `y`: one record and one `spec: apply PR-...` commit where raised, or a reject. Else `kept staged: spec approve <ID> or spec round answer --staged confirms it`, the item `open` and staged. A refusal (steps 2-10; step 7: the stage replaced or removed since the question) is named and the walk goes on. End of input ends it, the rest named as kept.
6. `round: <a> applied, <j> rejected, <k> kept staged, <f> refused`. Exit 0, nothing refused or kept; 1 otherwise; 2 cannot run.

`--json`: one document after the walk, stdout (questions stay on stderr): `{"project","answers":[{"id","line","answer","outcome","stage","reason"}],"walk":[{"id","outcome","commit","record","reason"}],"notes"}`; `outcome` `staged|open|refused`, then `applied|rejected|kept|refused`; `stage` the stored stage or `null`.

**`--staged`**: no file, no stage written; steps 1, 5, 6 over every `open` deciding item of the repository holding a stage, round order. A staged `update` or `create` is not asked: note ``<n> staged change proposal(s) not asked: spec approve <ID>``.

## Known limits

Consent fatigue: one question per answer, each record shown whole (`--limit`; the report first; `n` keeps the stage). A pasted reply's planted line, at column 0, is staged before it is read: shown whole, step 7 compared, refused characters and caps; a confirmed answer becomes the owner's record. Rendered, agent text may still look like Markdown headings: the sheet is no sandbox; only its file text is parsed. A returned 200-item sheet over 8 MiB: return the answer lines.
