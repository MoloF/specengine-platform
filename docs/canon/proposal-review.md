---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-21
---

# Proposal review: inbox, review document, preview

What the owner and agents read of the queue: `spec inbox`, `spec review PR`, and the review document, the JSON of `propose`, `review`, `approve`, `reject` (MCP `get_proposal`, the daemon's and the UI's `Proposal`). Flags, outputs of the other commands, exits: `proposal-queue.md` "Commands"; `--brief`, the intake keys: `agent-intake-documents.md` "Review document". Review writes in no worktree. Code: CLI `inbox.rs`, `review.rs`, `proposals.rs` (the document); MCP `mirror.rs` (its schema).

## Inbox

`spec inbox [--all]` -> the current repository's `open` and `approved` (`--all`: every state) by ID number, `<id> | <kind> | <status> | <target_id> | <branch> | <created_at> | <rationale's first line>` (over 80 characters: 79 + `…`; an intake's `<severity>: <summary>`, a drift item's summary alone); text state `open (staged)` for a staged one (`decision-staging.md`). JSON `{proposals: [{id, kind, status, target_id, target_ids, branch, created_at, rationale, severity, summary, record_id, staged_at}], notes}` (12 keys), `target_ids` every target (an update: `[target_id]`). Notes (`note:`, JSON `notes`), exit 0: ``<n> proposal(s) of another repository of the project `<slug>` not listed: `spec inbox` lists the current repository's``; ``<n> proposal(s) of a repository that no longer exists not listed: <IDs>; `spec reject <ID> --reason …` takes …`` (10 IDs, then ` … and <k> more`); a corrupt row: ``proposal PR-0003: the stored `<column>` cannot be read: <why>; not listed``. Known limit: `inbox --all`'s gone note lists gone applied and rejected IDs too.

## Review text

`spec review PR` -> every document key as `key: value`, absent `-`; `base_text`, `new_text`, `rationale`, `diff`, `conflict`, `decision_note` as blocks indented two spaces; `diagnostics`, `notes` as `key: <n>` and an indented line each; `staged:` compact JSON, `staged_at:`; `drift: <n> row(s)` (stored plus `more`), each stored row indented two spaces in `spec verify`'s row form (`spec-verify-output.md` "Text"; a keyless binding `-`), then `  … and <k> more`; `drift: -` on other kinds.

## Review document

Every key, absent = `null`, none starting `block`; 48, in this order:

`{id, project, kind, status, target_id, target_path, worktree, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author, diagnostics, diff, preview, conflict, revisions, drift, decided_by, decided_at, decision_note, applied_commit, created_at, updated_at, <eleven intake keys>, <a record's five>, task_id, staged, staged_at, interpretation, notes}`

Keys defined elsewhere: `revisions` `proposal-revisions.md` "Review"; `drift` `{hash, rows, more}` (`null` on other kinds; rows as stored, later keys kept: `--json` and the daemon keep each row's key order, MCP sorts keys, parsing into `serde_json::Value`) `proposal-drift.md` "Review, stage, round"; the eleven, `target_ids` to `linked`, `agent-intake-documents.md` "Review document"; `record_id` to `choice` `decision-record.md` "Queue and documents"; `task_id` `tasks.md` "Task-bound proposals"; `staged` (the stage object or `null`), `staged_at` `decision-staging.md` "The stage"; `interpretation` `proposal-interpretation.md` "Row and review". `diff`: base -> new hunks of `git diff --no-index --no-color --no-ext-diff --diff-algorithm=myers -U3` under `--- base <path>`, `+++ proposed <path>` (from the worktree top).

**Pinned**: `fixtures/daemon-keys.json` holds the UI's 76 key sets, among them `Proposal`, `InboxEntry`, the stage's two shapes (`StagedApprove` 7 keys, `StagedReject` 2) and `DriftEvidence` (`hash`, `rows`, `more`; rows `StoredDriftRow` in the UI, unpinned). MCP `get_proposal` = `review --brief --json`, its output schema's `staged` two closed objects, `drift` `DriftShape`. UI `daemonKeys.test.ts` compares the provisional types with the file, set by set in the slices' order; its `AHEAD_OF_FIXTURE` (keys served before the file has them) is empty, and a listed key the file names fails. Backups keep stages (`queue-backup.md` "Format").

## Preview

`preview` (open, approved; a drift item's `null`, no walk, note ``spec approve PR shows the rows before asking``): apply steps 2-6 (`proposal-apply.md` "Apply steps") read-only -> `applies`, `rebases`, `conflicts` (+ `conflict`, the merge's text) or `unavailable` (note `not applicable now (step <n>): <reason>`; a raise refused: `proposal-revisions.md` "Review"); its own commit on the branch -> `unavailable`, steps not run, note ``its commit <sha> is on `<branch>`: `spec approve PR` completes it; no new apply is needed``; a lookup git cannot make -> note ``cannot tell whether its commit is on `<branch>`: <why>``.
