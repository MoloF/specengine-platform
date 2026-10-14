---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-2 (4c split 2026-10-13, Q19; WA-8, Q7; decided under the owner's pre-approval, review open); 05 s5.3; 06 s4; 07 s2; 08 s2
adrs: [ADR-0045]
---

# drift-follow-up (4c-2): rejecting a drift item makes a draft task

## Why

ADR-0045 gives a drift item three outcomes. 4c-1 (`drift-queue.md`) ships approve (the lock takes the code) and a plain reject. The other two, "bring the code back to the spec" and "change the spec to match the code", need a task, made ready only by the owner (ADR-0012). After 4b-1 the commonest item, `spec_ahead marker_behind` on each binding of a node whose meaning change was applied, cannot be accepted (`spec-lock.md` "Lockability"): without a follow-up its task is written by hand. Never cut (Q19).

**In scope:** `spec reject PR --reason T --follow-up code|spec` on a `drift` item; store `reject_with_task`; core's title and goal; `proposal.rejected` gains `task`.

## Description and interactions

- **The owner**, on a terminal where the item was raised, rejects with `--follow-up code` (change the code to match the spec) or `--follow-up spec` (propose a spec edit describing the code): one `[y/N]`, one transaction, the item `rejected`, a `draft` task; `spec task approve` makes it `ready` (ADR-0012).
- **An agent** sees the task once `ready` (`claim_task`); its goal names node, revision, file and rows. `code`: it changes the code, sets the markers, the owner then accepts the lock. `spec`: it raises `propose_change`.
- **Crates**: core pure (the texts), store (`reject_with_task`), CLI (`reject`'s flag). No MCP tool (the owner's terminal), no HTTP or UI change, no schema step, no new crate or dependency.
- **Unchanged**: plain `spec reject`, other kinds' reject, task states, `spec task new`. **Writes**: the queue (the row, the task, two events) and the index.
- **Overridden live-spec lines**, fixed at shipping (SW-2): 05:180 ("a task is created/updated": a draft task, by this follow-up), 06:122-123 (the task outcomes), 07:95 (`spec reject … [--follow-up code|spec]`).

## Data

### reject --follow-up

WA-8, Q7. `spec reject PR --reason T [--follow-up code|spec]`. The flag on another kind, staged or not: exit 2 `--follow-up: only a drift item has a follow-up`; another value: exit 2. With it, after plain reject's checks (`proposal-completion.md` "Reject"; its lock commit looked up before and after the prompt: `drift-queue.md` "Approve", completion):

1. The node resolved now, in the recorded root's refreshed index; gone: exit 1 `--follow-up: R-12 names no node now; reject without it`.
2. Ask `reject PR-0012 (R-12 in spec.lock on main in <worktree>) and make a draft task (code)? [y/N]`.
3. One `Immediate` transaction, store `reject_with_task`: `reject_from` on the state read and a `draft` task as `spec task new` makes one (`tasks.md`: targets `[target_id]`, author `human`); `proposal.rejected` gains `task` (`null` on every reject without one), then `task.created`.
4. stdout `rejected PR-0012; created T-0003 (draft): spec task approve T-0003 makes it ready`.

### Title and goal

Core renders them from IDs, paths, symbols and states, stack-neutral (07 P2-3), from the item's stored rows (text only: nothing stored reaches the lock):

- code: title `Bring the code bound to R-12 in line with its spec (PR-0012)`; goal `R-12 rev 3 (docs/spec/combat.md) is the spec: change the bound code below to match it and set its markers to @3; spec verify then shows its rows ok, or the owner accepts them.`
- spec: title `Propose a spec edit of R-12 matching its bound code (PR-0012)`; goal `The bound code below is right: propose an edit of R-12 rev 3 (docs/spec/combat.md) describing it; once applied, set its markers to the new revision, and the owner accepts the lock.` (SW refinement: after 4b-1 the applied edit raises the revision, so the markers lag until set.)
- then `Owner's reason: <reason>` (cut at 1 024 B, `…`), the rows in `spec-verify-output.md` "Text" form by key while the goal stays within 4 096 B, then ``<k> more row(s): spec review PR-0012``.

### Exits

`proposal-completion.md` "Reject"'s, plus `--follow-up`: 1 a gone node, a lock commit in history, the state changed since read (nothing written); 2 another kind, another value, no identity.

## Rules and edge cases

- WHEN the follow-up is made, the reject and the task SHALL commit in one transaction: a failure leaves neither, and no event.
- WHEN the item has its lock commit on its branch, the follow-up SHALL be refused as a plain reject is.
- The texts hold no stack, runner or domain word (07 P2-3 scan, the genre scans); the goal never exceeds 4 096 B.

## Acceptance criteria

rust-developer, mutation -> the test turning red:

- [ ] RD-1 -- the task `ready` -> `follow_up_task_is_draft`; two transactions -> `follow_up_and_reject_are_one_transaction`; another kind -> `follow_up_on_a_non_drift_kind_is_a_usage_error`; a gone node -> `follow_up_needs_a_node`; the lock commit ignored -> `follow_up_refuses_a_drift_item_with_its_lock_commit`.
- [ ] RD-2 -- a stack word in the brief -> the P2-3 scan; a domain literal -> the genre scans; a goal over 4 096 B -> `follow_up_goal_is_capped`.
- [ ] RD-3 -- `task` missing from `proposal.rejected`, or `task.created` before it -> `follow_up_events_name_the_task`.

test-engineer:

- [ ] TE-1 -- follow-ups of both kinds on 4c-1's fixtures: a draft task, golden title and goal, the events; `--follow-up` on a staged item of another kind -> exit 2; a gone node refused; every exit: `git status`, `HEAD`, the queue unchanged.

spec-writer:

- [x] SW-1 -- this spec, split from `drift-queue.md` 2026-10-13. SW-3 -- `export index`, `check` clean.
- [ ] SW-2 -- at shipping: canon `proposal-drift.md` (a follow-up section beside 4c-1's), `proposal-completion.md` "Reject" (the flag, the event's `task`), `tasks.md`; the overridden lines; the core, store, CLI READMEs; `CLAUDE.md` State.

## Out of scope

Follow-ups for other kinds; a follow-up without a reject; a `follow_up` stage key, UI or HTTP (Phase 4); an MCP tool; a task approved at once (ADR-0012); several nodes per task.

## Open

- Q7 the outcomes (2026-10-10): `reject --follow-up code|spec`, one transaction, a draft task; the other answer, an approve without a commit, breaks "`applied` = a commit".
- Q19 the split (2026-10-13): this slice, after 4c-1, never cut; the other answer leaves the commonest item's tasks to be written by hand.

## Dependencies and ordering

- **Needs**: 4c-1 `drift-queue` (the kind, the item, its stored rows, plain reject with the lock commit lookup).
- **Order**: 4c-1 -> 4c-2. **Independent**: 4b-2, 5b, 5c, 8b, 9b.

## Implementation

Not started. Criteria from `drift-queue.md` (2026-10-13), old -> new: RD-11 -> RD-1 (plus the lock commit test), RD-16's follow-up half -> RD-2 (plus the cap), RD-3 new; TE-6's follow-ups -> TE-1; SW-4's follow-up canon -> SW-2.
