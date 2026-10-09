---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-core]
owner: owner
reviewed: 2026-10-09
---

# Decision records: config, template, flags

What `spec approve` of a deciding item (`question`, `discrepancy`, `decision`) reads before its steps: the project's `[decision_records]`, its template, the owner's flags. The steps, ID, completion and queue columns: `decision-record.md`. Code: core `project_toml`, `record`; CLI `decide.rs` (`first_checks`).

## Config

```toml
[decision_records]
prefix   = "DEC"                    # an [ids] prefix
dir      = "docs/records/DEC"       # root-relative, inside the walk
template = "templates/decision.md"  # tracked; may lie outside the walk
```

Optional: only approving a deciding item reads it. Exactly these three strings, checked whenever `specengine.toml` is loaded (any command: exit 2 `specengine.toml:<line>: decision_records.<key>: ...`): `prefix` an `[ids]` entry of shape `number` and scope `project` (an `aliases_from` name: error naming the canonical prefix); `dir`, `template` in `[paths]` grammar. Adding it re-parses nothing (the index fingerprint reads `[ids]`). No built-in shape: no table at step 3 -> exit 2 naming the table, its keys and the way out `spec reject PR --reason <answer>`.

## Template

Read at step 3 under the recorded root: no symlink component, a regular file, tracked and clean in the worktree, UTF-8, at most 64 KiB, no character the queue's output escapes (core `is_escaped`; CR too), opening with a front-matter, every `{{` opening a known slot, `{{canon}}` in the front-matter; else exit 2 `<template>:<line>: <problem>` (file-level: line 1).

Slots are replaced in **one left-to-right pass**; a value is never scanned again (a label `{{id}}` stays that text); lists joined by LF. A template holds no literal `{{`.

| Slot | Value |
|---|---|
| `id`, `proposal` | the record's ID (`DEC-0024`); `PR-0004` |
| `date`, `status` | `YYYY-MM-DD` of the run's clock, UTC; `accepted` |
| `canon` | `--canon`, else the first ID target, canonical, reference form |
| `targets` | the ID targets, `[Q-031, RULE-STAM-REGEN]`, `[]` if none (a path is no reference) |
| `title` * | the chosen label, or the answer's first non-blank line: whitespace runs one space, trimmed, at most 128 bytes (cut at a character, ending U+2026) |
| `choice` * | the label, or the answer |
| `effect`, `cost` * | the option's effect and price; `--answer`: empty, `price_of_other`; the working answer: both empty |
| `summary`, `options`, `evidence` * | the item's; `- <label> \| <effect> \| <price>`; `- <file>[:<lines>][ <qpath>] \| <observed> \| <documented>`; a question, a decision: empty |
| `note`, `decided_by` * | `--note`; the committer identity |

\* Free text: body only, else a template error; `title` excepted, which the template writes unquoted (`title: {{title}}`) and the engine writes as `"..."`, `\` and `"` backslashed.

## Flags

`spec approve PR [--note T] [--option N | --answer T] [--canon REF]`; library `approve_with` with `ApproveFlags {option, answer, canon}`. A decision flag on an applying kind -> exit 2 naming the deciding kinds by `decides()` (`create::deciding_kinds`): `` `--option` decides a `question`, `discrepancy` or `decision`; `PR-0002` is an update: `spec approve PR-0002` applies it as proposed; nothing changed ``. Then, after the terminal check, no event (a stage too: `decision-staging.md`), `first_checks` judges by the item's fields, never its kind: one without options (a question, a decision) takes the working answer or `--answer`, one with options (a discrepancy) `--option`:

- exit 2: `--option` on one without options (`` `--option` names a discrepancy's option, and `PR-0001` is a decision: its working answer, or `--answer T`, answers it; nothing changed ``, the kind with its article); `--answer` on a discrepancy; an open discrepancy without `--option` (naming the range, `0-2`); no ID target and no `--canon`; `--canon` not `ID`, `ID#SECTION` or `path#anchor` (core `parse_canon`); a blank `--answer`;
- exit 1: `--option` past the options (naming `0-2`); `--answer` over 2 048 bytes, `--canon` over 512; an escaped character in any free-text source, named (``options[1].label: holds U+202E: a decision record never carries it; nothing changed``).

The choice: a discrepancy's option; a question's or decision's working answer (no flag; never the recommendation by default) or `--answer T`. **Canon only from an ID**: a path target gives no reference-form `canon:`, so an item on a path alone needs `--canon`, which overrides anywhere; the check judges it (findings shown, never refusing). An `approved` item (stopped after step 7) takes only `--note`, any decision flag exit 2, and writes its stored `record_text`, never rendered again.
