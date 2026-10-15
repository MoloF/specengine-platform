---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store]
owner: owner
reviewed: 2026-10-15
---

# Proposal kind `drift`: the item, its evidence, dedup

What a `drift` item stores, how its evidence is hashed and how a raise deduplicates it, for the core and store reader. The raise, review, stage, approve and limits: `docs/canon/proposal-drift.md`. Code: core `drift` (`DriftEvidence`, `StoredJson`, `drift_hash_input`); store `create_drift`.

## The item

Kind `drift`: `target_id` the node's canonical ID (or the gone ID); `target_path` `spec.lock` (core `LOCK_FILE`); `target_ids` `[target_id]`; the place (ADR-0032), `base_commit` the commit judged; `severity` `high` when a row is class `fix`, else `normal`; `summary` its rows' distinct `state[ category]`, key order, `, `-joined; `author`, `task_id` as the intake's; `diagnostics` `[]`; every other column NULL but:

- `drift`: compact JSON `{"rows":[…],"more":<n>}`: all its node's non-`ok` rows by key, the first 64 (`DRIFT_ROWS_MAX`) each a `spec-verify-output.md` "JSON" row as `spec verify --json` writes it (`pre_existing` `true`), `more` the rest.
- `drift_hash` = `b3_hash("specengine-drift/v1" LF <compact JSON>)` (`drift_hash_input`), one array per non-`ok` row (all), by key: `[node, relation, file, kind, symbol, item_kind, state, category, detail, marker_revs, lock_rev, lock_spec_hash, lock_code, lock_file_blob, rev, norm_hash, code, moved_from]`. `marker_revs`: `null` (unversioned) first, the `@N` ascending, then `"bad"`; `lock_*` the entry's or `null`, `lock_code` its `ast_hash` (`null` for `module`, `ron`; lock format 2: its code fields as rendered); `rev`, `norm_hash` the node's R and H at the commit; `code` the cascade's current fact (an item changed by hash: its digest; a `module` or `ron` changed by blob: the blob OID; else `null`); `moved_from` `null`, reserved for 5c. Not hashed: lines, times, the entry's `commit` and `verified_at`, the commit judged, the mode, the item's file blob (a moved marker or an edit beside the item is the same drift).

**Store.** `ProposalKind::Drift`; `family() -> KindFamily {Applies, Decides, Locks}`, exhaustive (`ALL` 8), its family `Locks`; "writes no existing node" is `family() == Decides || new_file()`. `create`, `create_intake`, `approve_record_from` refuse it (`Invalid`); `approve_from`, `applied_with`, `reopen_from`, `log_failure`, `reject_from` take it; only `create_drift(&[NewDrift], now)` stores one, refusing (`Invalid`, nothing written) a `target_path` other than `spec.lock`, a blank summary, evidence that does not parse, a malformed hash. Queue schema 8 (`proposal-queue.md` "Store"): `drift`, `drift_hash` TEXT. **Corrupt**, named: on a drift row either NULL; `drift` off its shape, a row missing any key `spec verify --json` writes (a `null` value is none, nor a later build's key: 5b's `levels`, 5c's `anchor`, kept as stored); a hash not `b3:` + 64 lowercase hex; another intake column or `linked` set, `target_ids` not `[target_id]`, `severity` or `summary` NULL; a stage; either column on another kind.

## Dedup

`create_drift`, one `Immediate` transaction, each `--task` checked as `create_with_task` does; per item, keyed (`git_common_dir`, `branch`, `target_id`) over the project's drift items read under the write lock: an `open` or `approved` one is a hit (`queued`, `same evidence` when its `drift_hash` is equal, else `evidence changed since it was raised`; JSON `same`); else the newest `rejected` one with an equal hash, settled; else stored `open` under the next ID, IDs in node byte order, `proposal.created` each. An `applied` one never settles: equal evidence means its lock commit was undone, note ``PR-0013: R-14 drifts as when PR-0004 was applied by <applied_commit>; that lock is not in effect at the commit judged``. Each `open` item of this repository and branch whose judged node reads all `ok` (a hand `spec lock accept` writes no queue row): ``PR-0007: R-20 reads ok at HEAD now; spec reject PR-0007 --reason … settles it``. Two concurrent raises store one item per key; busy over 5 s: exit 2.
