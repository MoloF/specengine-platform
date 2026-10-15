---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-core, crates/specengine-mcp]
owner: owner
reviewed: 2026-10-15
---

# Decision records: a deciding item approved

Phase 2 slice 5 (`decision-apply`), `kind-decision`: `spec approve` on a `question`, `discrepancy` or `decision` (`ProposalKind::family()` `Decides`; a decision: `proposal-decision.md`) makes the owner's choice an accepted decision record in the project's own shape: one new file and one `spec: apply PR-...` commit in the recorded worktree (ADR-0004, ADR-0005, ADR-0032), the queue and the data directory; nothing else. `reject --reason` still settles one with an answer and no record ("Reject"). The engine names no prefix, directory or heading (ADR-0008): the project's table and template do. Config, template, flags: `decision-record-template.md`; updates, consent: `proposal-apply.md`; completion: `proposal-completion.md`; intake: `agent-intake.md`. Code: core `record` (`DecisionRecords`, `Template`, `Slot`, `Choice`, `record_{id,form,title,defect}`, `is_escaped`, caps); store `queue.rs`, `worktree.rs`; CLI `decide.rs`, `apply.rs` (dispatch, reject), `preflight.rs` (completion).

## Config

`[decision_records]` (`prefix`, `dir`, `template`), checked on every load: `decision-record-template.md` "Config".

## Template

The project's template, its slots filled in one pass at step 5: `decision-record-template.md` "Template".

## Flags

`spec approve PR [--note T] [--option N | --answer T] [--canon REF]`, judged by the item's fields: `decision-record-template.md` "Flags".

## Steps

Then the trailer lookup ("Completion"); then in the recorded worktree, each refusal logged and reopened as an update's (`proposal-apply.md` "Apply steps"), named by step: ``spec: `PR-0001` not applied (step 3): ...``.

- **2** Place: an update's (exit 2).
- **3** Config, template: exit 2.
- **4** The recorded root's index refreshed, the ID previewed ("ID"); `<dir>/<ID>.md` inside the walk (else exit 2 naming `dir`); no symlink or non-directory on its way, nothing at the path (a dangling symlink too) nor in git's index, else exit 1 `` `<path>` exists: a decision record never replaces a file; nothing changed `` ("Killed run"). The identity, as an update's.
- **5** Render; an escaped character in `decided_by`, a record over `TEXT_MAX_BYTES`: exit 1.
- **6** Structure (the recorded root's scheme): one front-matter, `class: decision`, `id:` the ID, `status: accepted`, no other ID defined, `canon:` read back as the slot. A defect named by its source: with every free-text value empty, the template's (exit 2 `<template>: the record it renders is no decision record: <defect>`) or `--canon`'s when the template reads back a probe canon (exit 1); else the first value that brings it, re-added in input order (exit 1 `<field>: with it, <defect>; nothing changed`). Then the tree checked as is and with the record against `HEAD`: `introduced: <n>` and its lines above the prompt.
- **1** Consent: stderr `record <ID> at <path>:`, the record escaped, indented two spaces, then `apply PR-0004 as <ID> (option 1 | the working answer | the given answer) on <branch> in <worktree>? [y/N]`.
- **7** `approve_record_from`: the ID issued; `Issued` -> exit 1 naming the next ID (run again; nothing written).
- **8** Write: the place and step 4 re-checked (exit 1); store `create_file`: missing `dir` components made one by one, the bytes to a new dot-named sibling, synced, **hard-linked** to the path (`link` never replaces: an entry appearing meanwhile -> exit 1, untouched; no hard links: exit 1), the sibling removed, the directory synced.
- **9** Commit: `git add --intent-to-add -- :(literal)<path>`, then an update's `git commit --only` of that path: whatever else is staged or modified stays. Body `<ID>: <title>`, the four trailers. Failed with no commit of this ID made: `git rm --cached --quiet --ignore-unmatch`, the file and this run's directories removed (innermost first, if empty), reopened, the ID kept, exit 1 ``the commit of `<path>` failed, the record removed: <git's error>``.
- **10** Verify: one parent, the old `HEAD`, name-status exactly `A <path>`, its blob `record_text`, `Proposal:` the ID -> `applied_with`, `update_paths`; stdout `applied PR-0004 as <sha> on <branch>: <ID> <path>`; a note names a linked update still to approve, else that the `canon:` section's text is unchanged, naming `spec propose interpretation <target> --decision <ID>` (`proposal-interpretation.md`).

## ID

`<prefix>-<n>` padded to the prefix's `width` (core `record_id`), `n` one more than the greater of the highest number of the prefix or its `aliases_from` in the recorded root's index (defined or in `aliases:`) and the highest `record_id` of the prefix the queue issued (`next_record`). Previewed at step 4 for the prompt; **issued only after consent**, at step 7, in the `Immediate` transaction where `approve_record_from` recomputes it under the write lock (not the preview -> `Issued`). A proposal keeps its ID across a reopen, never another's; a declined or refused run issues none; gaps are allowed.

## Completion

By the `Proposal:` trailer, as an update's (`proposal-completion.md` "Completion"): one parent, name-status exactly `A <record_path>`, its blob the stored `record_text` byte for byte, never re-rendered; else why (`changes M a, A b`, `does not carry the record`, `has N parents`). With no record issued none completes it; a trailer commit that does not complete it refuses a new apply (step 4). **A held record completed by hand**: `record_text` committed at `record_path` with the trailer; `spec approve PR` completes it (`approved`: no prompt; `open`: the `complete` question). An open item holding a record completes as recorded: `--option` or `--answer` naming another choice, or `--canon` reading back another canon -> exit 2 naming the stored ones, no prompt, nothing changed.

## Killed run

A run killed between steps 8 and 9 leaves the item `approved` with its record, the file and its intent-to-add entry. A new approve refuses at step 4 `exists`; when the file's bytes are the held `record_text` and the entry intent-to-add, the refusal names **two ways out**: commit it by hand (``git commit --only --trailer 'Proposal: PR' -m '<ID>' -- <path>``), then `spec approve PR` completes it; or `git rm --cached -- <path>`, delete the file, then `spec approve PR` writes it again.

## Reject

`reject --reason`: as `agent-intake.md` "Settling", refused by its history only when a commit may exist. Holding a record (issued at an earlier step 7): read as an update's, unreadable history refused. Holding none: only a found commit carrying its trailer refuses; history git cannot read (worktree removed, **branch deleted** after a merge) -> rejected, a note before the prompt and on stderr (`` `PR` holds no record, and its history cannot be read (<why>): rejected without looking for a commit of it ``). `decided_by`: the history's committer when one may exist, else the current repository's.

## Queue and documents

Queue step 2 -> 3 (`QUEUE_SCHEMA_VERSION` 3): `ADD COLUMN ... TEXT` `record_id`, `record_path` (root-relative, clean), `record_title`, `record_text`, `choice` (one-key JSON `{"option":1}`, `{"working_answer":true}`, `{"answer":"..."}`, core `Choice`); `PROPOSAL_COLUMNS` 40 (later: `proposal-queue.md` "Store"). Set together by `approve_record_from` (`RecordApproval {series, preview, path, title, text, choice}`), replaced by a later one, kept by `reopen_from`. A deciding kind: `create`, `approve`, `approve_from` -> `Invalid`; `applied`, `applied_with` without its record -> `Invalid`. Corrupt, named: any on an update; set partly; `record_id` no ID; `record_path` not clean; `choice` of another shape or kind, or out of range; an `approved` or `applied` deciding row without them. Events `.approved`, `.applied` add `record`.

- **Review document** (`get_proposal`): after `linked`, `record_id`, `record_path`, `record_title`, `record_text`, `choice` (an object), then `task_id`, absent `null`; text: `record_text` a block, `choice` compact JSON; `--brief`: `record_text` `null`.
- **Inbox**: + `record_id`; an applied one's last column ends ` [<record_id>]`.
- **Intake match**: + `record`: a corpus hit's `id:`; an applied queue hit's `record_id`, with `path` its `record_path` and `answer` its `record_title` (approved but stopped, or reopened: as asked).
- **Backup**: `queue-backup.md`. **MCP**: `mirror.rs` mirrors the keys; three tool descriptions name `spec approve`. **Plugin** 0.1.3: `ask-owner` reads a hit's `record`.

## Known limits

Accepted at shipping (2026-10-06); none blocks.

- A record hand-written under the issued ID elsewhere: `id-taken` at merge. A foreign file taking a kept ID's `<dir>/<ID>.md` (a merge) refuses every approve `exists` for good: only reject.
- A queue restored from before step 7 while the record's commit is in a readable history: neither approve nor reject takes it.
- Free text with a refused character is only rejected; a path target meets its record in corpus dedup only if `canon:` lands on it.
- The killed-run hint with the project root below its worktree's top is tried by hand only; the no-title exit 2, unreachable past the intake caps, untested.
