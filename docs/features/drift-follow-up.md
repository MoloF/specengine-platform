---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-2 (4c split 2026-10-13, Q19; WA-8, Q7; decided under the owner's pre-approval, review open); 05 s5.3; 06 s4; 07 s2; 08 s2; refreshed 2026-10-15 against shipped 4c-1 (d4f19e6) and 4b-2 as implemented in the working tree (re-checked when it ships)
adrs: [ADR-0045]
---

# drift-follow-up (4c-2): rejecting a drift item makes a draft task

## Why

ADR-0045 gives a drift item three outcomes. 4c-1 (`drift-queue.md`, shipped 2026-10-15; canon `docs/canon/proposal-drift.md`) ships approve (the lock takes the code) and a plain reject. The other two, "bring the code back to the spec" and "change the spec to match the code", need a task, made ready only by the owner (ADR-0012). After 4b-1 the commonest item, `spec_ahead marker_behind` on each binding of a node whose meaning change was applied, cannot be accepted (`spec-lock.md` "Lockability"): the approve exits 1 naming `spec reject PR --reason …` (`drift_apply.rs` doc, `proposal-drift.md` "Approve"), and without a follow-up its task is written by hand. Never cut (plan Q19).

**In scope:** `spec reject PR --reason T --follow-up code|spec` on a `drift` item; store `reject_with_task`; core's title and goal; `proposal.rejected` gains `task` when one is made; a drift item's reject refused only by its completing lock commit (Q11); 4c-1's carry-overs ("Implementation").

## Description and interactions

- **The owner**, on a terminal in the item's repository, as a plain reject, rejects with `--follow-up code` (change the code to match the spec) or `--follow-up spec` (propose a spec edit describing the code): one `[y/N]` showing the task's title and goal, one transaction, the item `rejected`, a `draft` task; `spec task approve` makes it `ready` (ADR-0012).
- **An agent** sees the task once `ready` (`claim_task`); its goal names node, revision, file and rows. `code`: it changes the code, sets the markers, the owner then locks them. `spec`: it raises `propose_change`. Both paths pass 4b-2's rule (`rev-rule.md`): `code` ends in the owner's lock-only accept commit, `spec` in an approve raising the revision; no goal advises a hand spec or lock edit or `--no-verify`.
- **Crates**: core pure (the texts, `task.rs`), store (`reject_with_task`), CLI (`reject`'s flag, the Q11 lookup, the carry-overs), ui (the mock's text). No MCP tool (the owner's terminal) or agent-facing text (`INSTRUCTIONS`, `ask-owner`, plugin 0.8.0 unchanged); no HTTP change; UI: only the mock's stale-stage text (carry-over); the review document keeps 48 keys, `daemon-keys.json` its 46 sets; no schema step, no new crate or dependency.
- **Unchanged**: plain `spec reject` (stdout, `--json`, its `proposal.rejected` payload), other kinds' reject, task states, `spec task new`. **Changed** for every drift item: its reject's lookup (Q11). **Writes**: the queue (the row, the task, two events) and the index.
- **Overridden live-spec lines**, fixed at shipping (SW-2): 05:180-181 ("4c-2": a draft task, by this follow-up), 06:122-123 (the task outcomes, "4c-2"), 07:96 (`spec reject … [--follow-up code|spec]`), 08:41 (`drift-follow-up` … draft -> shipped); re-count after 4b-2's SW-2 (it overrides 05:103).

## Data

### reject --follow-up

WA-8, plan Q7. `spec reject PR --reason T [--follow-up code|spec]`; library `reject_follow_up(env, globals, &RejectRequest, FollowUp, consent)` beside `reject`, as `approve_editorial` sits beside `approve_with` (Q8). The flag on another kind, staged or not: exit 2 `--follow-up: only a drift item has a follow-up`; another value: exit 2. Checked right after the item is read (`run_reject`, after `find`), before its state. A project whose `[ids]` takes `T` (core `task_prefix_clash`): exit 2, as `spec task new`. An orphan, or a recorded worktree that no longer exists: exit 1 `--follow-up: the worktree <w> of PR-0012 no longer exists; reject without it`. Then plain reject's checks (`proposal-completion.md` "Reject"; the lock commit looked up before and after the prompt, "A drift item's reject" below):

1. The node resolved now at the recorded root (CLI `preflight.rs` `recorded_project`/`recorded_root`, as apply step 4 and the drift approve read it), as `spec task new --nodes` resolves one (CLI `propose.rs` `resolved_node`, canonical through core `check::canonical_id`, 4b-2's), its revision and path read there; gone (no node has that canonical ID, an alias included: `spec-lock-accept.md` "A gone node"): exit 1 `--follow-up: R-12 names no node now; reject without it`.
2. In the one consent prompt on stderr (Q10): the title, the goal, then plain reject's question with ` and make a draft task (<kind>)` before `?`: `reject [approved ]PR-0012 (R-12 in <root_rel/>spec.lock on main in <worktree>) and make a draft task (code)? [y/N]`; controls escaped as the question's.
3. One `Immediate` transaction, store `SqliteQueue::reject_with_task(id, &Seen, &Decision, &NewTask, now)` (beside `applied_refreshing`, the other op changing a proposal and a task in one transaction): `reject_from`'s compare-and-set and a `draft` task as `create_task` stores one (`NewTask`: the item's `git_common_dir`, targets `[target_id]`, author `human`); a task the store refuses (`Invalid`) leaves neither. `proposal.rejected` gains `task` only then, as `record` and `staged_at` are added (store `with_record`, `with_staged`): a plain reject's payload unchanged (Q1); then `task.created`.
4. stdout `rejected PR-0012` as a plain reject; a note, on stderr and in the document's `notes` (Q2): `created T-0003 (draft) for PR-0012: spec task approve T-0003 makes it ready`.

### Title and goal

Core (`task.rs`, a P2-3-scanned task source: `task-package.md` "Genre"; Q7) renders them from the node's ID, revision and path read at step 1 (Q3), the item's ID, the reason and the stored rows' lines, stack-neutral (07 P2-3). The lines are the CLI's one row formatter's (`proposals.rs` `drift_row_line`, as `spec review` prints them, `proposal-review.md` "Review text"), passed in as text: core formats no row and carries no verify hint (4b-2's `spec bump` clause in `SAME_REVISION_STEP` churns no goal). Text only: nothing stored reaches the lock.

- code: title `Bring the code bound to R-12 in line with its spec (PR-0012)`; goal ``R-12 rev 3 (docs/spec/combat.md) is the spec: change the bound code below to match it and set its markers to @3; the owner then locks them (`spec lock accept R-12`, or the approve of a new drift item).``
- spec: title `Propose a spec edit of R-12 matching its bound code (PR-0012)`; goal `The bound code below is right: propose an edit of R-12 rev 3 (docs/spec/combat.md) describing it; once applied, set its markers to the new revision, and the owner accepts the lock.` (After 4b-1 the applied edit raises the revision, so the markers lag until set.)
- then `Owner's reason: <reason>` (cut at 1 024 B with `…`; the whole reason stays the item's `decision_note`), the stored rows' lines while the goal stays within core `GOAL_MAX` (4 096 B), then ``<k> more row(s): spec review PR-0012``, `<k>` the stored rows not shown plus the item's `more`. A title over `TITLE_MAX` (256 B) is cut with `…`.

### A drift item's reject (Q11)

Plain or followed up, CLI `apply.rs` `run_reject` (`committed` -> `carried`, before and after the prompt) refuses a `drift` item only for a `Proposal:` commit that completes it, by completion's predicate (`preflight.rs` `drift_commit`/`lock_commit_problem`: one parent, only `<root_rel>/spec.lock` changed and present, `Lock: <target_id>`, `Lock-mode: accept|editorial`), or when git cannot tell. A non-completing one (no `Lock-mode`, `Lock-mode: regen`, other paths) never blocks it: a note on stderr and in `notes`, `` the commit <sha> on `<branch>` carries `Proposal: PR-0012` but does not complete it (<why>); rejected all the same ``. Other kinds: unchanged (any such commit refuses).

### Exits

`proposal-completion.md` "Reject"'s, plus `--follow-up`: 1 a gone node, an orphan or a gone recorded worktree, a completing lock commit in history, the state changed since read (nothing written); 2 another kind, another value, `T` in `[ids]`, no identity.

## Rules and edge cases

- WHEN the follow-up is made, the reject and the task SHALL commit in one transaction: a failure leaves neither, and no event.
- WHEN the item has its completing lock commit on its branch, or git cannot tell, the follow-up SHALL be refused as a plain reject is.
- WHEN a drift item's `Proposal:` commits complete none of it, a reject, plain or followed up, SHALL NOT be refused by them (Q11).
- WHEN a reject carries no follow-up, its `proposal.rejected` payload SHALL be byte-identical to 4c-1's (no `task` key).
- WHEN one item in one state is followed up from two roots or another cwd, title and goal SHALL be byte-identical.
- The texts hold no stack, runner or domain word (07 P2-3 scan, the genre scans); the goal never exceeds `GOAL_MAX`, the title `TITLE_MAX`.
- 4b-2's escapes still raise `spec_ahead same_revision`: a follow-up on one stays allowed; "the owner then locks them" holds (editorial accept or a raise).

## Acceptance criteria

rust-developer, mutation -> the test turning red:

- [ ] RD-1 -- the task `ready` -> `follow_up_task_is_draft`; two transactions (injected, a store test: `reject_with_task` with `targets` empty refused `Invalid`, then the item still `open`, no event, no task row) -> `follow_up_and_reject_are_one_transaction`; another kind -> `follow_up_on_a_non_drift_kind_is_a_usage_error`; a gone node -> `follow_up_needs_a_node`; an orphan or a gone recorded worktree -> `follow_up_needs_the_recorded_worktree`; the follow-up's write before reject's lookups -> `follow_up_refuses_a_drift_item_with_its_lock_commit` (a completing one; the pin that the follow-up keeps 4c-1's RD-9 lookup).
- [ ] RD-2 -- a stack word in core `task.rs`'s follow-up texts -> the P2-3 scan; a domain literal -> the genre scans; over `GOAL_MAX` -> `follow_up_goal_is_capped`; over `TITLE_MAX` -> `follow_up_title_is_capped`; a row formatted in core -> the code-reviewer.
- [ ] RD-3 -- `task` missing from a follow-up's `proposal.rejected`, or `task.created` before it -> `follow_up_events_name_the_task`; `"task":null` on a plain reject -> `plain_reject_event_has_no_task`; no task line in `--json` `notes` -> `follow_up_note_names_the_task`.
- [ ] RD-4 -- carry-overs: one of the six printed paths raw again -> `printed_git_paths_are_literal_pathspecs`, a CLI source scan (no `-- {top_path}` outside `pathspec_word`; Q6); `round.rs`'s pointer, the code-reviewer.
- [ ] RD-5 -- Q11: the reject's lookup back to the trailer alone -> `reject_is_not_blocked_by_a_non_completing_lock_commit` (a drift item whose only `Proposal:` commit has `Lock-mode: regen`, and one with none: plain reject and follow-up both pass, the note given); 4c-1's `reject_refuses_a_drift_item_with_its_lock_commit` stays green.

ui-developer:

- [ ] UI-1 -- the mock's stale-stage refusal (`ui/src/mocks/MockClient.ts` ~233) in its old words, not the daemon's (CLI `stage.rs` ~267-269) -> its mock test.

test-engineer:

- [ ] TE-1 -- in a new file, on CLI `tests/drift_common/` (`drifted`, `ecs_drifted`): follow-ups of both kinds, a draft task, golden title and goal, the events; on a `marker_behind` item and a `code_ahead` one, the note on stderr and in `--json` `notes`; `spec task approve T` makes the task `ready`, `spec task show T` carries the goal; `--follow-up` on a staged item of another kind or `T` in `[ids]` -> exit 2; a gone node, an orphan -> exit 1; on every refusal the queue (proposals, tasks, events), `git status` and `HEAD` unchanged.

spec-writer:

- [x] SW-1 -- this spec, split from `drift-queue.md` 2026-10-13, refreshed 2026-10-15. SW-3 -- `export index`, `check` clean.
- [ ] SW-2 -- at shipping (sizes of 2026-10-15): `proposal-completion.md` "Reject" (6 296 B; Q9), a **Follow-up** paragraph of about 1-1.4 KB (flag and check order, question, transaction, note, texts, caps, exits), Q11's drift lookup and note, its Code line + core `task` texts, store `reject_with_task`; `proposal-drift.md` (8 722 of 12 288 B after the canon split of 2026-10-15: the item and dedup in `proposal-drift-item.md`), no section: "Approve"'s "(`marker_behind` alone: 4c-2's follow-up)" -> "(`marker_behind` alone: `--follow-up`)", its Completion paragraph's "A plain `spec reject` is refused while one exists" -> "Only a completing commit refuses `spec reject`" (Q11); `proposal-queue.md` (11 375 B) `:20` `spec reject PR --reason T [--follow-up code|spec]`, `:44` `.rejected` (`reason`; `task` with a follow-up), about +50 B; `tasks.md` (11 607 B) `:17` or the `new` row `:37`, ≤ 150 B: `new` also by `spec reject PR --follow-up code|spec`; `proposal-create-apply.md:31`, `decision-record.md:50` ("Killed run"): each `<path>` a quoted literal pathspec (`pathspec_word`), as `spec-lock-accept.md` "The lock commit", about +60 B each; the overridden lines; the core, store, CLI READMEs, one line each (9 900, 10 128, 8 441 of 10 240 B: the store's room first); `CLAUDE.md` State `drift-queue` -> `drift-*`. No agent-facing text: `INSTRUCTIONS` and `ask-owner` are at their caps (as 4b-2's Q20); if wanted, a role file or the `implement` skill with plugin 0.8.1.

## Out of scope

Follow-ups for other kinds; a follow-up without a reject; a `follow_up` stage key; a follow-up in the UI's drift card or over HTTP (Phase 4); the follow-up task named in the review document (a 49th key); approve's "nothing to accept now" naming `--follow-up` (Q4); an MCP tool or agent-facing text; a task approved at once (ADR-0012); several nodes per task; 4c-1's other optional nits (`drift-queue.md` "Implementation"), the reviewer's.

## Open

- Plan Q7 the outcomes (2026-10-10): `reject --follow-up code|spec`, one transaction, a draft task; the other answer, an approve without a commit, breaks "`applied` = a commit".
- Plan Q19 the split (2026-10-13): this slice, after 4c-1, never cut; the other answer leaves the commonest item's tasks to be written by hand.

Decided 2026-10-15 under the owner's pre-approval, review open (answer; the other answer's cost):

- Q1 **(contract)** `proposal.rejected`'s `task`: only when made, as `record`, `staged_at`; always (`null`): every reject's payload changes, event and backup goldens churn.
- Q2 **(contract)** the task ID: a note on stderr and in `notes`, stdout `rejected PR`; a stdout line: a serde-skipped `ProposalOutcome` field, `raw_text` changed, the note still needed for JSON; a 49th review key: the five 48-key pins, `daemon-keys.json`, MCP's schema and UI types churn (over 2 iterations).
- Q3 the goal's revision: the node's now (step 1); the stored `node_rev` may be stale: stale `@N` markers, `marker_behind` back.
- Q4 approve's refusal naming `--follow-up`: not here, Phase 4's drift card offers it; else about +25 B in a full canon, the `drift_approve.rs` pins, `drift_apply.rs`'s doc.
- Q5 the mock text: here (UI-1, `ui` in scope); else words the daemon never says, carried a second time.
- Q6 the pathspecs: a CLI source scan (RD-4); the reviewer alone (4c-1's Q-D) leaves a regression unpinned.
- Q7 the texts' home: core `task.rs`; `drift.rs`: P2-3's list must grow.
- Q8 the seam: `reject_follow_up` beside `reject`; a `RejectRequest` field breaks every struct literal in tests (no `Default`).
- Q9 the canon home: `proposal-completion.md` "Reject"; a new canon file costs an index entry and links, `proposal-drift.md` has no room.
- Q10 title and goal before `[y/N]`: yes, in the one consent string (decide shows the record, drift approve the rows); else the owner consents to unseen text no command edits (`tasks.md`): cancel and `task new`.
- Q11 4c-1's stuck item (a `Proposal:` commit that is no accept's lock commit: approve finds nothing to accept, reject and follow-up refused): reject's lookup takes completion's predicate ("A drift item's reject"); else the item stays stuck until a hand-made completing commit. Cost: `rejected` recorded while a commit naming the item sits in history, noted.

## Dependencies and ordering

- **Needs**: 4c-1 `drift-queue`, shipped 2026-10-15 (the kind, the item, its stored rows, plain reject with the lock commit lookup: `proposal-drift.md`).
- **Order**: 4c-1 -> 4c-2. **After 4b-2** (`rev-rule.md`; refreshed ahead of its shipping): re-checked when it ships, I1 `canonical_id` moved to core (`check::canonical_id`) and `resolved_node`'s name and module (CLI `propose.rs`), I3 verify's `SAME_REVISION_STEP` (rows only in a goal); its SW-2 writes `proposal-drift.md` first; shared: `main.rs`, `propose.rs`, core `lib.rs`, `tests/drift_common/`, `tests/drift_approve.rs` (4c-2's tests in a new file). **Independent**: 5b, 5c, 8b, 9b.

## Implementation

Not started. Criteria from `drift-queue.md` (2026-10-13), old -> new: RD-11 -> RD-1 (plus the lock commit test), RD-16's follow-up half -> RD-2 (plus the cap), RD-3 new; TE-6's follow-ups -> TE-1; SW-4's follow-up canon -> SW-2 (numbering of 2026-10-13, before drift-queue's 2026-10-14 refresh).

Carry-overs from 4c-1 (code; RD-4, UI-1): its Q-D, six raw pathspecs printed (two `git commit … -- <path>`, four `git rm --cached`; the worktree already named) in CLI `create.rs` `left_behind` (~1131; text ~1158-1160) and step 9's `Made::Nothing` warning (~1406-1410), `decide.rs` step 9's `Made::Nothing` warning (~644-648) and `left_behind` (~1169; text ~1200-1203): quote each by `pathspec_word` (CLI `lib.rs`, beside `one_line`); the canon quoting them: SW-2. CLI `round.rs` ~220-221 cites task spec `drift-queue`, "Round", a heading it lacks: cite canon `proposal-drift`, "Review, stage, round". The UI mock's stale-stage text (`ui/src/mocks/MockClient.ts` ~233, ``changed since it was read (updated X, read Y): read it again; nothing changed``) to the daemon's words (CLI `stage.rs` ~267-269, ``changed since it was read: updated at X, the choice was made on the one of Y; read it again; nothing staged``), the ui-developer's. These texts are pinned in CLI tests; the pins churn, as expected.
