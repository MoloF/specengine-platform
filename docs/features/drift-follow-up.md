---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-2 (4c split 2026-10-13, Q19; WA-8, Q7; decided under the owner's pre-approval); 05 s5.3; 06 s4; 07 s2; 08 s2; refreshed 2026-10-15 against shipped 4c-1 (d4f19e6) and 4b-2 (55f2e08); Rust 2 iterations, review accepted, tests 2 rounds
shipped: 2026-10-15
adrs: [ADR-0045]
---

# drift-follow-up (4c-2): rejecting a drift item makes a draft task

## Why

ADR-0045 gives a drift item three outcomes. 4c-1 (`drift-queue.md`, shipped 2026-10-15; canon `docs/canon/proposal-drift.md`) ships approve (the lock takes the code) and a plain reject. The other two, "bring the code back to the spec" and "change the spec to match the code", need a task, made ready only by the owner (ADR-0012). After 4b-1 the commonest item, `spec_ahead marker_behind` on each binding of a node whose meaning change was applied, cannot be accepted (`spec-lock.md` "Lockability"): the approve exits 1 naming `spec reject PR --reason …` (`proposal-drift.md` "Approve"), and without a follow-up its task is written by hand. Never cut (plan Q19).

**In scope:** `spec reject PR --reason T --follow-up code|spec` on a `drift` item; store `reject_with_task`; core's title and goal; `proposal.rejected` gains `task` when one is made; a drift item's reject refused only by its completing lock commit (Q11); 4c-1's carry-overs ("Implementation").

## Description and interactions

- **The owner**, on a terminal in the item's repository, as a plain reject, rejects with `--follow-up code` (change the code to match the spec) or `--follow-up spec` (propose a spec edit describing the code): one `[y/N]` showing the task's title and goal, one transaction, the item `rejected`, a `draft` task; `spec task approve` makes it `ready` (ADR-0012).
- **An agent** sees the task once `ready` (`claim_task`); its goal names node, revision, file and rows. `code`: it changes the code, sets the markers, the owner then locks them. `spec`: it raises `propose_change`. Both paths pass 4b-2's rule (`rev-rule.md`): `code` ends in the owner's lock-only accept commit, `spec` in an approve raising the revision; no goal advises a hand spec or lock edit or `--no-verify`.
- **Crates**: core pure (the texts, `task.rs`), store (`reject_with_task`), CLI (`reject`'s flag, the Q11 lookup, the carry-overs), ui (the mock's text). No MCP tool (the owner's terminal) or agent-facing text (`INSTRUCTIONS`, `ask-owner`, plugin 0.8.0 unchanged); no HTTP change; the review document keeps 48 keys, `daemon-keys.json` its 46 sets; no schema step, no new crate or dependency.
- **Unchanged**: plain `spec reject` (stdout, `--json`, its `proposal.rejected` payload), other kinds' reject, task states, `spec task new`. **Changed** for every drift item: its reject's lookup (Q11). **Writes**: the queue (the row, the task, two events) and the index.
- **Overridden live-spec lines**, fixed at shipping: 05 the drift table, 06 s4, 07 s2, 08 s2.

## Data

Canon: `proposal-completion.md` "Reject" (the drift lookup, "Follow-up", the texts).

### reject --follow-up

WA-8, plan Q7. `spec reject PR --reason T [--follow-up code|spec]`; library `reject_follow_up(env, globals, &RejectRequest, FollowUp, consent)` beside `reject`, as `approve_editorial` beside `approve_with` (Q8). Right after the item is read (`run_reject`, after `find`), before its state: another kind, staged or not -> exit 2 `--follow-up: only a drift item has a follow-up`; another value: clap's exit 2 (`code`, `spec`, case-sensitive). Then the recorded root (CLI `follow_up.rs` `recorded_for`; `preflight.rs` `recorded_project`/`recorded_root`): an orphan -> exit 1 `--follow-up: the repository <git_common_dir> of PR-0012 no longer exists; reject without it`; a gone recorded worktree -> exit 1 `--follow-up: the worktree <w> of PR-0012 no longer exists; reject without it`; its config unreadable or its slug another -> exit 2 `--follow-up: <error>`; its `[ids]` taking `T` (core `task_prefix_clash`) -> exit 2, as `spec task new`. Then plain reject's checks, the lookup ("A drift item's reject"), the identity:

1. The node resolved now at the recorded root, its index brought up to date, as `spec task new --nodes` resolves one (CLI `propose.rs` `resolved_at`, `resolved_node`'s twin also returning the node's position; core `check::canonical_id`), its revision and path read there. Gone (no node has that canonical ID, an alias included: `spec-lock-accept.md` "A gone node"; every walked file read and parsed, W18) -> exit 1 `--follow-up: R-12 names no node now; reject without it`; not one node (several holders, declared twice, or any walked file, the holder or not, unread, unparsed or not UTF-8: W18, `follow_up.rs` `nodes_known`) -> exit 1 `--follow-up: <resolver's reason>; reject without it`, for an unparsed holder in practice ``--follow-up: `R-12` resolves to no ID and no alias; reject without it`` (no file named: W18a; a node outside the walk still reads gone: W18b); the index not refreshed (not opened, the tree not walked) -> exit 2; the revision unreadable -> exit 1 `--follow-up: the revision of R-12 in <path> cannot be read now; reject without it`.
2. The one consent prompt on stderr (Q10), escaped whole: a Q11 note if any (W17), `draft task: <title>`, `goal:`, each goal line indented two spaces, then plain reject's question with ` and make a draft task (<kind>)` before `?`: `reject [approved ]PR-0012 (R-12 in <root_rel/>spec.lock on main in <worktree>) and make a draft task (code)? [y/N]`.
3. One `Immediate` transaction, store `SqliteQueue::reject_with_task(id, &Seen, &Decision, &NewTask, now) -> (Proposal, Task)`: `reject_from`'s compare-and-set and a `draft` task as `create_task` stores one (the item's `git_common_dir`, targets the canonical ID of step 1, equal to `target_id`, author `human`); another kind or a task the store refuses (`targets` empty): `Invalid`, neither written, no event. `proposal.rejected` gains `task` (`with_task`, after `with_staged`): a plain reject's payload unchanged (Q1); then `task.created`.
4. stdout `rejected PR-0012` as a plain reject; a note on stderr and in `notes` (Q2): `created T-0003 (draft) for PR-0012: spec task approve T-0003 makes it ready`.

Help: `--follow-up <KIND>` ``A drift item only: also make a draft task, `code` (change the bound code to match the spec) or `spec` (propose a spec edit describing the code); `spec task approve` makes it ready.``; `reject`'s about-line ``… never one whose `Proposal:` commit is on its branch (a drift item: one completing it); …``.

### Title and goal

Core (`task.rs`, P2-3-scanned: `task-package.md` "Genre"; Q7) renders them from the node's ID, revision and path read at step 1 (Q3), the item's ID, the reason and the stored rows' lines, stack-neutral. The lines are CLI `proposals.rs` `drift_row_line`'s (as `spec review` prints them), passed in as text: core formats no row, carries no verify hint. Nothing stored reaches the lock.

- code: title `Bring the code bound to R-12 in line with its spec (PR-0012)`; goal ``R-12 rev 3 (docs/spec/combat.md) is the spec: change the bound code below to match it and set its markers to @3; the owner then locks them (`spec lock accept R-12`, or the approve of a new drift item).``
- spec: title `Propose a spec edit of R-12 matching its bound code (PR-0012)`; goal `The bound code below is right: propose an edit of R-12 rev 3 (docs/spec/combat.md) describing it; once applied, set its markers to the new revision, and the owner accepts the lock.`
- then `Owner's reason: <reason>` (line breaks as spaces; other controls kept, escaped on output; at most 1 021 B + `…`, `FOLLOW_UP_REASON_MAX` 1 024; the whole reason stays the `decision_note`), the stored rows' lines indented two spaces while the goal with its closing line fits `GOAL_MAX` (4 096 B), then ``<k> more row(s): spec review PR-0012``, `<k>` the rows not shown plus the item's `more`; LF-joined. A title over `TITLE_MAX` (256 B) is cut with `…`, on a character boundary.

### A drift item's reject (Q11)

Plain or followed up, CLI `apply.rs` `drift_carried` (before and after the prompt) refuses a `drift` item only for a `Proposal:` commit completing it by completion's predicate (`preflight.rs` `drift_commit`/`lock_commit_problem`: one parent, only `<root_rel>/spec.lock` changed and present, `Lock: <target_id>`, `Lock-mode: accept|editorial`), `carried`'s text, or when git cannot tell: the lookup fails (`cannot_tell`) or a commit cannot be read (`TrailerCommit.unread`): ``cannot tell whether `PR-0012` has its commit in history: the commit <sha> on `<branch>` cannot be read: <error>``. A non-completing one (no `Lock-mode`, `regen`, other paths) never blocks it: one note naming the newest (W17), `` the commit <sha> on `<branch>` carries `Proposal: PR-0012`<more> but does not complete it (<why>); rejected all the same ``, `<more>` ` (<k> older one(s) too)` when k > 0, above the question through `noted` (as the `unknown` note), on stderr and in `notes`; after `y` only a different one is added. Other kinds: `committed`, unchanged.

### Exits

`proposal-completion.md` "Reject"'s, plus `--follow-up`: 1 an orphan or a gone recorded worktree, a gone node, not one node (a walked file unread, unparsed or not UTF-8 included: W18), an unreadable revision, a completing or unreadable lock commit, the state changed since read (nothing written); 2 another kind, another value (clap's), the recorded config unreadable or another slug, `T` in `[ids]`, the recorded index not refreshed, no identity; exit-2 texts start `spec: ` as everywhere.

## Rules and edge cases

- WHEN the follow-up is made, the reject and the task SHALL commit in one transaction: a failure leaves neither, and no event.
- WHEN the item has its completing lock commit on its branch, or git cannot tell, the follow-up SHALL be refused as a plain reject is.
- WHEN a drift item's `Proposal:` commits complete none of it, a reject, plain or followed up, SHALL NOT be refused by them (Q11), and the owner SHALL read one note naming the newest before answering.
- WHEN a reject carries no follow-up, its `proposal.rejected` payload SHALL be byte-identical to 4c-1's (no `task` key).
- WHEN one item in one state is followed up from two roots or another cwd, title and goal SHALL be byte-identical.
- The texts hold no stack, runner or domain word (07 P2-3 scan, the genre scans); the goal never exceeds `GOAL_MAX`, the title `TITLE_MAX`.
- 4b-2's escapes still raise `spec_ahead same_revision`: a follow-up on one stays allowed; "the owner then locks them" holds (editorial accept or a raise).

## Acceptance criteria

rust-developer, mutation -> the test turning red:

- [x] RD-1 -- the task `ready` -> `follow_up_task_is_draft`; two transactions (injected, a store test: `reject_with_task` with `targets` empty refused `Invalid`, then the item still `open`, no event, no task row) -> `follow_up_and_reject_are_one_transaction`; another kind -> `follow_up_on_a_non_drift_kind_is_a_usage_error`; a gone node -> `follow_up_needs_a_node`; an orphan or a gone recorded worktree -> `follow_up_needs_the_recorded_worktree`; the follow-up's write before reject's lookups -> `follow_up_refuses_a_drift_item_with_its_lock_commit` (a completing one; 4c-1's RD-9 lookup kept).
- [x] RD-2 -- a stack word in core `task.rs`'s follow-up texts -> the P2-3 scan; a domain literal -> the genre scans; over `GOAL_MAX` -> `follow_up_goal_is_capped`; over `TITLE_MAX` -> `follow_up_title_is_capped`; a row formatted in core -> the code-reviewer.
- [x] RD-3 -- `task` missing from a follow-up's `proposal.rejected`, or `task.created` before it -> `follow_up_events_name_the_task`; `"task":null` on a plain reject -> `plain_reject_event_has_no_task`; no task line in `--json` `notes` -> `follow_up_note_names_the_task`.
- [x] RD-4 -- carry-overs: one of the six printed paths raw again -> `printed_git_paths_are_literal_pathspecs`, a CLI source scan (no `-- {top_path}` outside `pathspec_word`; Q6); `round.rs`'s pointer, the code-reviewer.
- [x] RD-5 -- Q11: the reject's lookup back to the trailer alone -> `reject_is_not_blocked_by_a_non_completing_lock_commit` (a drift item whose only `Proposal:` commit has `Lock-mode: regen`, and one with none: plain reject and follow-up both pass, the note given); 4c-1's `reject_refuses_a_drift_item_with_its_lock_commit` stays green.

ui-developer:

- [x] UI-1 -- the mock's stale-stage refusal (`ui/src/mocks/MockClient.ts` ~233) in its old words, not the daemon's (CLI `stage.rs` ~267-269) -> its mock test.

test-engineer:

- [x] TE-1 -- in a new file, on CLI `tests/drift_common/` (`drifted`, `ecs_drifted`): follow-ups of both kinds, a draft task, golden title and goal, the events; on a `marker_behind` item and a `code_ahead` one, the note on stderr and in `--json` `notes`; `spec task approve T` makes the task `ready`, `spec task show T` carries the goal; `--follow-up` on a staged item of another kind or `T` in `[ids]` -> exit 2; a gone node, an orphan -> exit 1; on every refusal the queue (proposals, tasks, events), `git status` and `HEAD` unchanged -> CLI `drift_follow_up.rs` `te1_marker_behind_item_follows_up_with_code_on_a_terminal`, `te1_ecs_code_ahead_item_follows_up_with_spec_on_a_terminal`, RD-1's refusal tests; W18 -> `follow_up_a_holder_that_cannot_be_read_is_not_gone` (not UTF-8, mode 000; control: section deleted).

spec-writer:

- [x] SW-1 -- this spec, split from `drift-queue.md` 2026-10-13, refreshed 2026-10-15. SW-3 -- `export index`, `check` clean.
- [x] SW-2 -- at shipping: canon `proposal-completion.md` "Reject" (the drift lookup and note, "Follow-up", the texts, the Code line), `proposal-drift.md` ("Approve"'s `--follow-up`; only a completing commit refuses `spec reject`), `proposal-queue.md` (the command, `.rejected`'s `task`, `reject_with_task`), `tasks.md` (`new` by a follow-up), the quoted pathspecs (`proposal-create-apply.md`, `decision-record.md`); the overridden live-spec lines; the core, store, CLI READMEs; `CLAUDE.md` State, 08. No agent-facing text (`INSTRUCTIONS`, `ask-owner` at their caps, as 4b-2's Q20).

## Out of scope

Follow-ups for other kinds; a follow-up without a reject; a `follow_up` stage key; a follow-up in the UI's drift card or over HTTP (Phase 4); the follow-up task named in the review document (a 49th key); approve's "nothing to accept now" naming `--follow-up` (Q4); an MCP tool or agent-facing text; a task approved at once (ADR-0012); several nodes per task; 4c-1's other optional nits (`drift-queue.md` "Implementation"), the reviewer's.

## Open

- Plan Q7 the outcomes (2026-10-10): `reject --follow-up code|spec`, one transaction, a draft task; the other answer, an approve without a commit, breaks "`applied` = a commit".
- Plan Q19 the split (2026-10-13): this slice, after 4c-1, never cut; the other answer leaves the commonest item's tasks to be written by hand.

Decided 2026-10-15 under the owner's pre-approval (answer; the other answer's cost):

- Q1 **(contract)** `proposal.rejected`'s `task`: only when made, as `record`, `staged_at`; always (`null`): every reject's payload changes, event and backup goldens churn.
- Q2 **(contract)** the task ID: a note on stderr and in `notes`, stdout `rejected PR`; a stdout line: a serde-skipped `ProposalOutcome` field, `raw_text` changed, the note still needed for JSON; a 49th review key: the five 48-key pins, `daemon-keys.json`, MCP's schema and UI types churn.
- Q3 the goal's revision: the node's now (step 1); the stored `node_rev` may be stale: stale `@N` markers, `marker_behind` back.
- Q4 approve's refusal naming `--follow-up`: not here, Phase 4's drift card offers it; else about +25 B in a full canon, the `drift_approve.rs` pins, `drift_apply.rs`'s doc.
- Q5 the mock text: here (UI-1); else words the daemon never says, carried a second time.
- Q6 the pathspecs: a CLI source scan (RD-4); the reviewer alone leaves a regression unpinned.
- Q7 the texts' home: core `task.rs`; `drift.rs`: P2-3's list must grow.
- Q8 the seam: `reject_follow_up` beside `reject`; a `RejectRequest` field breaks every struct literal in tests (no `Default`).
- Q9 the canon home: `proposal-completion.md` "Reject"; a new canon file costs an index entry and links, `proposal-drift.md` has no room.
- Q10 title and goal before `[y/N]`: yes, in the one consent string; else the owner consents to unseen text no command edits (`tasks.md`): cancel and `task new`.
- Q11 4c-1's stuck item (a `Proposal:` commit that is no accept's lock commit: approve finds nothing to accept, reject refused): reject's lookup takes completion's predicate; else the item stays stuck until a hand-made completing commit. Cost: `rejected` recorded while a commit naming the item sits in history, noted.

## Dependencies and ordering

- **Needs**: 4c-1 `drift-queue`, shipped 2026-10-15 (the kind, the item, its stored rows, plain reject with the lock commit lookup).
- **After 4b-2** (`rev-rule.md`, shipped 55f2e08), re-checked: I1 `canonical_id` in core (`check::canonical_id`), `resolved_node` in CLI `propose.rs`; I3 verify's `SAME_REVISION_STEP` (`verify.rs:832`), a verify hint, not in `drift_row_line`'s rows: no goal churns. **Independent**: 5b, 5c, 8b, 9b.

## Implementation

| Module | What it does |
|---|---|
| core `task.rs` | `FollowUp {Code, Spec}` (`ALL`, `as_str`, case-sensitive `parse`), `FollowUpItem`, `follow_up_title`, `follow_up_goal`, `FOLLOW_UP_REASON_MAX`; private `cut_with_ellipsis` |
| store `queue.rs`, `queue/tasks.rs` | `reject_with_task`, one `self.write()` transaction; `reject_if` -> `rejectable`, `write_rejected` (`with_task`); `create_task` -> `new_task_problem`, `next_task_id`, `insert_draft`, `existing_task`, shared |
| CLI `follow_up.rs` (new) | `ONLY_DRIFT`, `recorded_for`, `planned` (step 1, texts; W18 `nodes_known`), `shown`, `question_tail`, `created_note` |
| CLI `apply.rs` | `reject_follow_up`; `run_reject(.., Option<FollowUp>, ..)`; Q11 `drift_carried` (one note), `older_too` (shared with `carried`), `cannot_tell` |
| CLI `preflight.rs`, `propose.rs`, `proposals.rs` | `TrailerCommit.unread`, `lock_commit_problem` -> `Result<Option<String>, String>`; `resolved_at`; `drift_row_line` `pub(crate)` |
| CLI `main.rs`, `lib.rs` | `--follow-up <KIND>` (`PossibleValuesParser`, then `and_then(FollowUp::parse)`), the about-line; re-exports |
| carry-overs | CLI `create.rs`, `decide.rs`: six `pathspec_word`s; `round.rs`'s canon pointer; store `git.rs`, CLI `bump.rs`: 4b-2's comment nits |
| ui `mocks/MockClient.ts` | UI-1: the daemon's words (`stage.rs` 266-270), two exact pins |

Iteration 2 (review 1's minor, nits; pre-approved): W17 per "A drift item's reject (Q11)", `carried` unchanged; W18 gone only when every walked file was read and parsed (step 1); W19 another value: clap's exit 2 alone.

Known limits (owner pre-approved working answers; canon "Known limits"): W18a the refusal names no unread file (`CheckFile::unreadable` reads as unparsed); W18b a node in a directory that cannot be listed, or under a non-UTF-8 name, reads "names no node now", the refresh warns; W18c a holder whose front-matter does not read (`FrontmatterYaml`, `-Unclosed`, `-NotMapping`) still counts as known: a gone document-level target there reads "names no node now". Carried to the next CLI slice: `nodes_known` also requires `front_matter_failure(parsed).is_none()`; `reject_follow_up`'s doc comment lists every exit-1 case.

Divergences, accepted by review 1: 1 an unreadable revision at step 1: exit 1; 2 not one node: the resolver's reason, exit 1; 3 recorded config unreadable or another slug: exit 2; 4 an unreadable `Proposal:` commit is "cannot tell", a drift item's only (approve unchanged); 5 the Q11 note: W17; 6 reason line breaks as spaces, other controls stored, escaped on output; 7 the target the canonical ID of step 1 (= `target_id`); 8 store `reject_with_task` refuses another kind; 9 exit-2 texts carry `spec: `.

Tests: round 1 green, 9/9 mutations killed; round 2 green, W18's test added, 4/4 killed, targeted 66/66, gates clean (3 failures outside 4c-2 pass on rerun). New CLI `drift_follow_up.rs`, `drift_carry_overs.rs`, store `queue_follow_up.rs`; pins moved: CLI `drift_approve.rs` (Q11), `decision_apply.rs`, `proposal_kinds.rs` (`':(literal)…'`).
