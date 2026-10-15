---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-10-15
---

# Proposal kind `drift`: lock drift in the owner's queue

`spec propose drift` (MCP `report_drift`) judges one commit as `spec verify` does and queues one `drift` item per node whose lock pair no longer holds, its evidence computed, never typed; approving one runs that node's `spec lock accept` (ADR-0045, `architecture.md#apply`, `#control`, `architecture-code.md#spec-lock`). Nothing blocks: no status, claim, gate, check or other approval reads a drift item. The raise writes the queue and the data directory; the approve `spec.lock`, one commit, the queue. Code: core `drift`; store `create_drift`; CLI `drift.rs`, `drift_apply.rs`, `lock.rs`, `preflight.rs`, `verify_rows.rs` (`spec-verify.md` "Base"); MCP `intake.rs`, `mirror.rs`.

## Raise

`spec propose drift [REF…] [--task T] [A] [--json]` (`A`: `agent-intake.md` "Tools", which lists `report_drift`: no field for rows or hashes). In order, the first refusal answering:

1. The project and its queue, the author's grammar, more than 16 REFs (`<n> REFs: name at most 16 nodes, or none for every drifting node`): exit 2.
2. The place (`proposal-queue.md` "Creation" 5): a worktree, `HEAD` born, on a branch, else exit 2; `base_commit` the `HEAD` read, the one commit judged, never read again.
3. `specengine.toml` tracked and clean, else exit 1 ``` `specengine.toml` is <state>: drift is judged under the config committed at HEAD; commit it first; nothing queued ```; `[code]` of the commit's config (none: exit 2). The place precedes this check (it needs the worktree): a dirty config with a detached `HEAD` exits 2.
4. `--task` (`tasks.md` "Task-bound proposals"): exit 1 `--task: <reason>`.
5. The commit's side alone (`read_commit` on the bound OID): no working-tree file is read, an uncommitted edit never judged; its causes exit 2.
6. Each REF (one given twice counts once) resolved as `spec show` resolves it over the commit's documents, a written `@rev` dropped with a note: a node with an ID, or a gone ID with entries in the commit's lock (`spec-lock-accept.md` "A gone node"). Several nodes, none, no ID, or a node only the working tree has (`` `R-30` names no node at HEAD: commit it first ``): exit 1; a look-alike: exit 2. No REF: a **sweep** of every node a row resolves or a lock entry names.
7. The rows (`judge_commit`) per node, canonical or the gone ID. Core `raises(state, detail, named)`: `code_ahead`, `spec_ahead`, `conflict`, `broken_binding` raise an item; `unlocked` only for a named node, never in a sweep, never `unlocked key_problem` (named: note ``R-30: <n> binding(s) cannot be locked (key_problem); spec verify names each``); `cannot_verify`, `bad_marker`, `predated` never on their own. A named node raising nothing: note `R-30: no drift at HEAD`.
8. The items stored ("Dedup").

Exit 0 whatever was stored. Text, by node bytes:

```
created PR-0012 | R-12 | high | conflict, code_ahead code_changed
queued PR-0009 | R-13 | open | evidence changed since it was raised
settled PR-0004 | R-14 | rejected | same evidence
propose drift: 1 created, 2 hit(s); 14 rows of 5 nodes judged at <oid> on main
```

JSON (MCP `structuredContent`, `DriftDocument`), every key, absent `null`: `{commit, branch, judged: {rows, nodes}, created: [{id, node, severity, summary}], hits: [{id, node, status, same}], notes}`. `notes` are the stderr `note:` lines, same set and order, on a refusal too (its reason last); a refusal prints no stdout. Byte-identical from any root, cwd, creation order or line ending.

## The item

Kind `drift`: `target_id` the node's canonical ID (or the gone ID); `target_path` `spec.lock` (core `LOCK_FILE`); `target_ids` `[target_id]`; the place (ADR-0032), `base_commit` the commit judged; `severity` `high` when a row is class `fix`, else `normal`; `summary` its rows' distinct `state[ category]`, key order, `, `-joined; `author`, `task_id` as the intake's; `diagnostics` `[]`; every other column NULL but:

- `drift`: compact JSON `{"rows":[…],"more":<n>}`: all its node's non-`ok` rows by key, the first 64 (`DRIFT_ROWS_MAX`) each a `spec-verify-output.md` "JSON" row as `spec verify --json` writes it (`pre_existing` `true`), `more` the rest.
- `drift_hash` = `b3_hash("specengine-drift/v1" LF <compact JSON>)` (`drift_hash_input`), one array per non-`ok` row (all), by key: `[node, relation, file, kind, symbol, item_kind, state, category, detail, marker_revs, lock_rev, lock_spec_hash, lock_code, lock_file_blob, rev, norm_hash, code, moved_from]`. `marker_revs`: `null` (unversioned) first, the `@N` ascending, then `"bad"`; `lock_*` the entry's or `null`, `lock_code` its `ast_hash` (`null` for `module`, `ron`; lock format 2: its code fields as rendered); `rev`, `norm_hash` the node's R and H at the commit; `code` the cascade's current fact (an item changed by hash: its digest; a `module` or `ron` changed by blob: the blob OID; else `null`); `moved_from` `null`, reserved for 5c. Not hashed: lines, times, the entry's `commit` and `verified_at`, the commit judged, the mode, the item's file blob (a moved marker or an edit beside the item is the same drift).

**Store.** `ProposalKind::Drift`; `family() -> KindFamily {Applies, Decides, Locks}`, exhaustive (`ALL` 8), its family `Locks`; "writes no existing node" is `family() == Decides || new_file()`. `create`, `create_intake`, `approve_record_from` refuse it (`Invalid`); `approve_from`, `applied_with`, `reopen_from`, `log_failure`, `reject_from` take it; only `create_drift(&[NewDrift], now)` stores one, refusing (`Invalid`, nothing written) a `target_path` other than `spec.lock`, a blank summary, evidence that does not parse, a malformed hash. Queue schema 8 (`proposal-queue.md` "Store"): `drift`, `drift_hash` TEXT. **Corrupt**, named: on a drift row either NULL; `drift` off its shape, a row missing any key `spec verify --json` writes (a `null` value is none, nor a later build's key: 5b's `levels`, 5c's `anchor`, kept as stored); a hash not `b3:` + 64 lowercase hex; another intake column or `linked` set, `target_ids` not `[target_id]`, `severity` or `summary` NULL; a stage; either column on another kind.

## Dedup

`create_drift`, one `Immediate` transaction, each `--task` checked as `create_with_task` does; per item, keyed (`git_common_dir`, `branch`, `target_id`) over the project's drift items read under the write lock: an `open` or `approved` one is a hit (`queued`, `same evidence` when its `drift_hash` is equal, else `evidence changed since it was raised`; JSON `same`); else the newest `rejected` one with an equal hash, settled; else stored `open` under the next ID, IDs in node byte order, `proposal.created` each. An `applied` one never settles: equal evidence means its lock commit was undone, note ``PR-0013: R-14 drifts as when PR-0004 was applied by <applied_commit>; that lock is not in effect at the commit judged``. Each `open` item of this repository and branch whose judged node reads all `ok` (a hand `spec lock accept` writes no queue row): ``PR-0007: R-20 reads ok at HEAD now; spec reject PR-0007 --reason … settles it``. Two concurrent raises store one item per key; busy over 5 s: exit 2.

## Review, stage, round

- **Inbox**: `PR-0012 | drift | open | R-12 | main | <created_at> | conflict, code_ahead code_changed` (the summary alone); JSON `severity`, `summary` set.
- **Review** (`proposal-review.md`, its text form too): `drift` `{hash, rows, more}`, `rows` the column's as stored (core `StoredJson`: each row's keys in stored order, later keys kept, in `--json` and HTTP; MCP `get_proposal` keeps content and later keys, its keys sorted: it parses into `serde_json::Value`); texts, `diff`, `conflict`, `revisions` `null`; **preview** `null`, no walk, an open or approved item noting ``spec approve PR-0012 shows the rows before asking``.
- **Stage**: never. CLI `stage.rs` `checked` refuses before any preview (`StageCause::Refused`, HTTP 409) ``PR-0012 is a drift item, decided on a terminal only; nothing staged``; store `Stage::problem` too (`Invalid`).
- **Round**: off the sheet (`Decides` only); note ``<n> open drift item(s) not on this sheet: <IDs>; spec review <ID>, then spec approve or spec reject``; `round answer` on one: the stage's refusal.

## Approve

`spec approve PR [--note T] [--editorial]` (`run_approve`'s drift branch, after `flag_on_apply` and the trailer lookup): `--option`, `--answer`, `--canon` exit 2 ``… `PR-0012` is a drift: spec approve PR-0012 runs the lock accept for R-12; nothing changed``. Accept's steps (`spec-lock-accept.md`) run through `lock_accept` around apply's (`proposal-apply.md` "Apply steps"); nothing stored in the item is read:

| Apply step | Drift approve |
|---|---|
| 1 | not a terminal: exit 2, before any read |
| 2 | apply's place (exit 2, logged); `HEAD` not `base_commit`: `note: HEAD moved since PR-0012 was raised` |
| 3-6 | accept 2-7 for `target_id` at the recorded root (not the cwd's), `proposal`, the recorded `branch`; a refusal logged at step 3, its exit; text changed at R names `spec approve PR-0012 --editorial`; nothing to change: exit 1 ``PR-0012 not applied: nothing to accept now for R-12 (<why>); spec reject PR-0012 --reason … settles it``, logged, still open (`marker_behind` alone: 4c-2's follow-up) |
| ask | accept 8, the rows then the question in the one consent prompt on stderr: `lock R-12 rev 3 (accept: 2 new, 1 refreshed, 0 kept, 0 dropped) for PR-0012 at <commit> on <branch> in <worktree>? [y/N]`; no identity: exit 2, logged at step 7 |
| 7 | after `y`, in the consent closure: `approve_from` on the state read; lost: exit 1 `PR-0012 changed meanwhile; nothing written` |
| 8-10 | accept 9-10, mapped from `LockOutcome.written`, never its text: `committed` -> `applied_with`, `proposal.applied`; `restored` -> reopened, logged at step 9 (a commit a hook reset, branch and `HEAD` unmoved, included: accept restores it); `unverified`, `left` -> `approved`, exit 1, step 10; nothing written after `y` -> reopened at step 8 |

The commit is accept's, `Proposal: PR-0012` after `Decided-by:` (its fifth check, its hooks, `rev-not-raised` silent: `spec-lock-accept.md` "The lock commit"); no spec file, revision or index entry is written. `--editorial` is accept's: every entry at R.

**Completion**, before the place: CLI `preflight.rs` `drift_commit`, ahead of `trailer_commit` (which would parse the lock as Markdown). A `Proposal: PR-0012` commit in `base_commit..<branch>` completes the item when it has one parent, changes only `<root_rel>/spec.lock`, holds it, carries `Lock: R-12` and `Lock-mode: accept` or `editorial`; else why (`has N parents`, `changes a, b`, `has no <path>`, a missing trailer). Such a commit never blocks the approve (other kinds refuse at step 5), which runs its own accept: after one deleting `spec.lock`, a new lock commit applies the item; after one with no `Lock-mode` or `Lock-mode: regen`, nothing to accept now, still `open` (`drift_approve.rs` `only_an_accepts_lock_commit_completes_a_drift_item`). Hint: ``a commit on `<branch>` with the trailers `Proposal: PR-0012` and `Lock: R-12`, one parent, changing only `<path>` completes it (`spec approve PR-0012`)``. A plain `spec reject` is refused while one exists (`proposal-completion.md` "Reject").

## UI

The inbox card keys off the review's `drift`, never the kind: "Drift evidence" first (rows, `more`, commit, hash, each row as JSON), then "Decided on a terminal" for the decision bar: the worktree, three commands to copy, `spec approve PR`, `spec approve PR --editorial` (always, its condition in words: rows untyped until Phase 4), `spec reject PR --reason "..."`; no Options or Section diff; a/r/c/d only announce. Mock: `ledger-api` PR-0010, in every scenario but `empty`; a stage refused after the `updated_at` check, a round answer before it, in the CLI's words.

## Known limits

- `HEAD moved since PR was raised` also fires when `HEAD` moved backwards.
- A partial fix re-raises; a removed worktree's items are only rejected; 5b's rebase re-raises rejected items once.
- `StoredJson` relies on serde_json's `arbitrary_precision` staying off.
