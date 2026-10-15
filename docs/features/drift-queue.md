---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-1 (WA-1..WA-15, Q1-Q20; split 2026-10-13, Q19; refreshed 2026-10-13, 2026-10-14 with Q-A..Q-D; decided under the owner's pre-approval, review open, "Open"); 05 s5.3; 06 s4; 07 s2; 08 s2; Rust 2 iterations, review accepted
shipped: 2026-10-15
adrs: [ADR-0045]
---

# drift-queue (4c-1): drift reaches the owner's queue; approving it runs the lock accept

## Why

Only whoever ran `spec verify` saw drift; the backlog its Pre-existing findings "go to" (05:188) did not exist. 05 s5.3, 06 s4 give drift three outcomes (accept the code, send it back, change the spec); ADR-0045 allows them (`architecture.md#apply`, `#control`; `architecture-code.md#spec-lock`).

Delivered: `spec propose drift` (MCP `report_drift`, plugin 0.8.0) judging the commit `HEAD` names and queueing one `drift` item per drifting node, its evidence computed and deduplicated; the kind family `Locks`; queue schema 8; the review's `drift` key (48), no preview, no stage; `spec approve` of an item running its node's `spec lock accept`, one lock commit with `Proposal:`; completion by that commit; verify's drift hint; the UI's drift card. 4c-2 (`drift-follow-up.md`) takes `spec reject --follow-up`.

## Description and interactions

Canon: `docs/canon/proposal-drift.md` (new: the raise, the item, dedup, review, stage, round, approve, completion, UI, known limits); the rest: SW-4.

- **An agent**, the owner or a project's own hook raises items once `spec verify` shows drift, typing no evidence; not `report_discrepancy`, whose approval writes a decision record.
- **The owner**, on a terminal where an item was raised, approves (the lock takes the node's pair at `HEAD`), approves `--editorial` (`spec_ahead same_revision`, `conflict` at R = `rev_L`), or rejects. A hand `spec lock accept` writes no queue row; the next raise notes the item (Q6). `spec_ahead marker_behind` alone is not lockable: approve exits 1, 4c-2's follow-up settles it.
- **Crates**: as `scope`; HTTP no code (Q-A); no new crate or dependency. **Unchanged**: `spec verify` but its hint, `spec check`, gate, hook, CI, task states, `INDEX_FORMAT`, `spec.lock`'s format, other kinds' apply, a hand accept's bytes. Live-spec lines fixed: 05:180-183, 05:188, 06:118-123, 07:26, 07:92, 08:41, 08:44.

## Data

As built in the canon; the headings stay for the pointers in code, tests and neighbouring specs.

### spec propose drift

`docs/canon/proposal-drift.md` "Raise" (Q1, Q3, Q15, Q16); its one-commit judge `spec-verify.md` "Base".

### Triggers

`docs/canon/proposal-drift.md` "Raise", step 7 (Q2, Q20).

### The item

`docs/canon/proposal-drift.md` "The item" (Q13).

### Kind family

`docs/canon/proposal-drift.md` "The item", "Store".

### Queue schema 8

`docs/canon/proposal-drift.md` "The item", "Store"; `proposal-queue.md` "Store"; `queue-backup.md` "Format".

### Dedup

`docs/canon/proposal-drift.md` "Dedup" (Q4, Q14).

### Output

`docs/canon/proposal-drift.md` "Raise".

### report_drift and texts

`agent-intake.md` "Tools", `mcp-read.md` "Tools", `plugin.md` "Skills", `plugin-files.md` "Changing it", `spec-verify-output.md` "Hints" (Q12).

### Review, inbox, round, stage

`docs/canon/proposal-drift.md` "Review, stage, round"; `proposal-review.md`; `decision-staging.md` "The stage" (Q8, Q-A).

### Accept's seams

`docs/canon/spec-lock-accept.md` "Steps", "The lock commit" (Q-A, Q-D).

### Approve

`docs/canon/proposal-drift.md` "Approve" (Q5, Q9, Q-B).

## Rules and edge cases

In `docs/canon/proposal-drift.md`: a raise byte-identical from any root, cwd, creation order or line ending, one item per key under racing raises; an approved item's `spec.lock` the bytes `spec lock accept <target_id>` writes on that commit, a commit of it alone; no stack, runner or kind literal outside the kind table, none in `specengine-mcp/src`; nothing reads a drift item to block (ADR-0012, ADR-0042 as amended).

## Acceptance criteria

rust-developer, mutation -> the test turning red:

- [x] RD-1 -- drift as `applies()` -> `drift_item_never_runs_the_update_apply`; the old pin kept -> `every_kind_has_exactly_one_family`; 9a's call site (if 9a is first) -> its health test.
- [x] RD-2 -- `unlocked` in a sweep -> `sweep_never_queues_a_never_locked_node`; `cannot_verify` alone -> `cannot_verify_alone_raises_nothing`; a named `key_problem` row raising -> `named_key_problem_is_a_note`; `std::fs` in core -> `model_and_core_sources_do_no_file_io`.
- [x] RD-3 -- the disk judged -> `uncommitted_edit_is_not_queued`, `raise_ignores_an_unreadable_disk_file`; `HEAD` re-read -> `raise_judges_the_bound_base_commit`; REF in the working tree -> `uncommitted_ref_is_refused`.
- [x] RD-4 -- MCP takes `rows` -> `report_drift_takes_no_evidence`; approve reads the stored column -> `approve_never_writes_stored_values`.
- [x] RD-5 -- no dedup -> `second_raise_stores_nothing`; settled ignored -> `rejected_drift_is_not_raised_again`; lines hashed -> `moved_marker_line_is_the_same_drift`; the item's file blob hashed -> `edit_beside_the_item_is_the_same_drift`; `applied` settling -> `reverted_lock_commit_raises_again`; branch ignored -> `drift_on_two_branches_is_two_items`; a race -> `concurrent_raises_store_one_item`.
- [x] RD-6 -- a second accept -> `drift_approve_writes_the_bytes_lock_accept_writes`; the node's file committed -> `drift_approve_commits_only_the_lock`; `--no-verify` -> `drift_approve_runs_hooks`; `Proposal:` first -> `lock_commit_trailers_parse`; no fifth check -> `dropped_proposal_trailer_fails_step_10`; `written` serialized -> `lock_accept_json_keeps_its_keys`.
- [x] RD-7 -- another worktree -> `drift_approves_only_where_raised`; a read before the terminal check -> `drift_approve_needs_a_terminal_first`; a dirty marker file -> `dirty_bound_file_refuses_drift_approve`; the cwd's root -> `drift_approve_locks_the_recorded_root`; `branch` unchecked -> `drift_approve_locks_the_recorded_branch`.
- [x] RD-8 -- code hashes refreshed -> `drift_editorial_keeps_code_hashes`; `editorial_refusal` as `!= Applies` -> `drift_editorial_runs_accept_editorial`; on `spec_changed` -> `editorial_without_entry_at_r_refuses`; vouching filter -> `drift_editorial_refreshes_every_entry_at_r`.
- [x] RD-9 -- a killed run re-committed, or judged by `trailer_commit` -> `killed_drift_approve_completes_by_trailer`; the lock commit ignored -> `reject_refuses_a_drift_item_with_its_lock_commit`; reopened after a commit -> `drift_approve_reopens_only_restored_bytes`; Q16 taken -> `unmoved_lock_commit_reopens_the_drift_item`.
- [x] RD-10 -- nothing to accept, yet `applied` -> `drift_with_nothing_to_accept_is_not_applied` (`marker_behind` only); a preview computed -> `drift_review_has_no_preview`. RD-11 -- a stage stored, or refused by the store only -> `drift_item_cannot_be_staged`.
- [x] RD-12 -- a key dropped -> the five review-key pins, 48 (`proposal_create.rs`, `kind_interpretation.rs` twice, `kind_amendment.rs`, `rev_raise.rs`), and the 46 sets (`kind_amendment.rs`); backup bytes lost -> `drift_rows_round_trip_byte_identical`. RD-13 -- an MCP write under the root -> AC-3's `git status` test; `INSTRUCTIONS` too long -> `server.rs`'s assert.
- [x] RD-14 -- walk order leaks -> `drift_raise_is_byte_identical_across_roots_and_order`. RD-15 -- a domain literal -> the genre scans. RD-16 -- a new crate -> the CLI pin in eval `build_graph.rs`. RD-17 -- the plugin version kept -> `ac10_the_content_and_version_are_the_last_pin`. RD-18 -- the hint missing -> `drift_hint_follows_the_other_hints`.
- [x] RD-19 -- seams, the code-reviewer: `verify_judges_a_given_walk`, `judge_is_source_neutral` green; the approve calls `lock_accept`, no second accept; `tests/lock_commit.rs` unchanged but the fifth check's test.
- [x] RD-20 -- carry-overs, the code-reviewer: CLI `apply.rs` docs read "stay as the commit left them"; "slice 4c" at `lock.rs` and `tests/lock_commit.rs` cites canon `proposal-drift`; trailer keys from core (`lock.rs` keeps only `Decided-by`); a raw pathspec -> `lock_restore_commands_quote_the_path`.

test-engineer:

- [x] TE-1 -- temp repos from `fixtures/code-a/`, golden lock `fixtures/spec-lock/code-a.lock`: a scenario per trigger state and category; other states raise nothing; a named REF's `unlocked` rows raise, `key_problem` rows note.
- [x] TE-2 -- raise -> review (`preview` `null`, the note) -> approve -> `spec verify` `ok`; the lock bytes a twin repository's after `spec lock accept`. TE-3 -- each exit 1 and 2 of raise and approve: `git status` (untracked too), `HEAD`, `spec.lock`, the queue unchanged.
- [x] TE-4 -- dedup {open, approved, applied, rejected} x {same, other evidence} x {same, other branch}; two concurrent raises, self-terminating. TE-5 -- killed runs (before, after the commit), a failing hook, a hook resetting the commit (Q-B): restored, reopened; a hook dropping `Proposal:`: `approved`.
- [x] TE-6 -- schema 6, 7 -> 8; a drift row with a stage, a NULL column, a bad hash or a missing row key, another kind with either column: corrupt. TE-7 -- MCP parity with `--json`, `drift` in `get_proposal`'s schema; AC-3; an unknown field refused; `get_proposal`'s `preview` `null`, its note.
- [x] TE-8 -- the lock commit's hooks: `spec check --staged` passes; `spec verify --changed` passes under `enforce-introduced`, refuses under `enforce` with another node's Fix row (restored, reopened); `--editorial` after a `--no-verify` text commit -> `ok`.
- [x] TE-9 -- two roots, another cwd, reverse creation order, CRLF: identical. TE-10 -- `fixtures/spec-c` exits 2 naming `[code]`; the source scans. TE-11 -- HTTP stage 409; `daemon-keys.json` (46 sets, 48 keys).
- [ ] TE-12 -- pilots, manual (`drift_pilot.rs`, `#[ignore]`, written, not run: the owner's), scratch clones locked by the test's own accept; items per state, raise time against AC-10; no pilot name committed.

ui-developer:

- [x] UI-1 -- provisional types and mock gain `kind: "drift"`, the key `drift` (`DriftEvidence`); the key missing -> `ui/src/api/daemonKeys.test.ts`; the drift card (`TerminalDecision`), no stage ever offered -> `drift.test.tsx`, `drift.smoke.test.tsx`.

spec-writer:

- [x] SW-1 -- this spec, split 2026-10-13, refreshed 2026-10-14. SW-2 -- ADR-0045; `architecture.md#apply`, `#control`. SW-3 -- the drafts naming 4c, 08 s2, 05:188. SW-5 -- `export index`, `check` clean.
- [x] SW-4 -- at shipping, 2026-10-15: `proposal-drift.md` (new); `spec-lock-accept.md`; `proposal-{review,completion,queue,revisions,apply,kinds,decision,interpretation,amendment}.md`, `decision-record{,-template}.md`, `decision-staging.md`, `round{,-ui}.md`, `tasks.md`, `queue-backup.md`, `agent-intake.md`, `mcp-read.md`, `plugin{,-files}.md`, `spec-verify{,-output}.md`, `architecture-code.md#spec-lock`, `#verify`; core, store, CLI, MCP, HTTP READMEs, `ui/README.md`, root `README.md`, `CLAUDE.md`; the live-spec lines; `drift-follow-up.md`, `rev-rule.md`, `anchor-states.md`, `health-drift.md`, `code-tools.md`.

## Out of scope

A daemon, watcher or CI raising items; drift screens, a drift preview, UI staging (Phase 4); the Health drift row (9b); several REFs in one item; a hand accept settling items; `@assumes` tasks; 4c-2's `--follow-up`; 5b `spec lock rebase`; 8b `[[tested]]`.

## Open

Owner pre-approved, review open (Q1-Q12 2026-10-10, Q13-Q20 2026-10-13, Q-A..Q-D 2026-10-14, Q-PV 2026-10-14); the other answer's cost:

- Q1 `verify --queue`: verify writes. Q2 always: a flood; never: hand accepts only. Q3 the disk: unfinished edits queued. Q4 by state: new code missed; none: settled questions repeated. Q5 stored hashes trusted; two accepts. Q6 settling without `[y/N]` (ADR-0038). Q8 a stage key. Q9 spec-side items only rejectable. Q10 against ADR-0041/0042. Q12 agents on Bash. Q13 any file edit re-raises. Q14 a reverted lock hides drift. Q15 a dirty rename judged. Q16 a bad disk file refuses a raise. Q20 an item only ever rejected. Q7, Q19: `drift-follow-up.md`.
- Q-A (overrides Q17, Q18, until Phase 4's drift screen): no preview, no `lock_plan` split; the other answer refactors `lock.rs`'s commit table, adds a `ReviewRequest` field in HTTP and a walk per review. Q-B a drift-only branch in accept's restore. Q-C two plans per review. Q-D (`create.rs`, `decide.rs` to 4c-2) the glob hazard stays, or two more modules. Q-PV plugin 0.8.0 MINOR (canon over the spec's "a PATCH").
- Accepted as built at review 1: the `HEAD moved` note whenever `HEAD` is not `base_commit`; the `ask-owner` drift bullet without a bold label (4 096 B cap).

## Implementation

| Module | What it does |
|---|---|
| core `drift.rs` (new), `lib.rs` | `DriftEvidence` (`parse_stored`), `DriftRow` (every key required), `StoredJson`, `drift_hash_input`, `raises`, `drift_summary`; `DRIFT_KIND` |
| store `queue.rs`, `queue/{stage,state}.rs` | `ProposalKind::Drift`, `family() -> KindFamily` (29 call sites); `create_drift`, `decode_drift`; schema 8, 46 columns; `Stage::problem` |
| CLI `drift.rs` (new) | `propose_drift`: the raise, `DriftDocument`, notes, text |
| CLI `drift_apply.rs` (new), `apply.rs`, `main.rs`, `preflight.rs` | `approve_drift` around `lock_accept`; `run_approve`'s drift branch, `flag_on_apply`; `drift_commit` before `trailer_commit`, `completes_when` |
| CLI `lock.rs`, `lib.rs` | `LockAcceptRequest.{proposal, branch}`, `Check::Untrailed`, `LockOutcome.{written, nothing}`, core's trailer keys; `pathspec_word` beside `one_line`, `Undo` quoted |
| CLI `verify_rows.rs`, `code_walk.rs`, `verify.rs` | `commit_config`, `read_commit`, `judge_commit`, `CodeTree::empty()`; `FindingLine`, `DRIFT_HINT` |
| CLI `proposals.rs`, `review.rs`, `inbox.rs`, `round.rs`, `stage.rs`, `state_file.rs`, `create.rs`, `interpret.rs`, `revisions.rs` | `drift` (48 keys) and its text; no preview; inbox, round, stage; dump schema 8; `family()` |
| MCP `intake.rs`, `mirror.rs`, `server.rs`; plugin; UI | `report_drift`, `DriftShape`, `DriftDocument`, `INSTRUCTIONS`; `ask-owner`, 0.8.0; `DriftEvidence`, `TerminalDecision`, mock PR-0010 |

Measured, each trimmed in words, pinned phrases kept: `ask-owner`'s body 4 059 -> 4 090 B of 4 096; `INSTRUCTIONS` 1 885 -> 1 886 B, 2 043 with `probes`, of 2 048 (`mcp-read.md` "Texts").

Deviations, accepted: 1 plugin 0.8.0 MINOR, not "a PATCH" (`plugin-files.md` "Version": a newly named tool; Q-PV); 2 the place before the config-clean check, which needs the worktree (a dirty config with a detached `HEAD`: exit 2, not 1); 3 `LockOutcome.nothing`, serde-skipped: the approve parses no text; 4 accept's refusals before the question logged at step 3, a missing identity at 7, nothing to accept one `apply_failed`, still open; 5 the rows in the one consent prompt on stderr (`Consent` takes one string); 6 rows as stored by core `StoredJson`, not `serde_json::Value`, which sorts keys here (no `preserve_order` but on tree-sitter's build-dependency edge): a serde_json feature is a dependency change, workspace-wide it reorders every `Value` output; none changed, MCP still sorts; 7 UI: `--editorial` always listed, its condition in words (rows untyped). Working answers: count = stored rows + `more`; the reverted note names `applied_commit`; a repeated REF counts once; `node_ids: []` invalid-params; corrupt also another intake column, `linked`, `target_ids`.

Iteration 2: test round 1's DEFECT (`drift_row_missing_a_key_is_corrupt`), review 1's minor (the fifth check apart: `Untrailed`, no restore on either path) and nits (notes parity, `drift_commit` as accept's step 10, `DRIFT_KIND`, the target path, one row formatter, `raises` for the hint); UI: `AHEAD_OF_FIXTURE` empty and guarded, the mock's refusals in the CLI's words and order, no Options or Section diff on a drift card.

Tests: core `drift_stored.rs`; store `queue_drift.rs`; CLI `drift_{raise,dedup,approve,pilot}.rs`, `lock_commit.rs`, `verify_states.rs`; MCP `mcp_drift.rs`; HTTP `drift.rs`, `daemon_keys.rs` (`fixtures/daemon-keys.json`: 46 sets); UI `drift.test.tsx`, `drift.smoke.test.tsx`, `daemonKeys.test.ts`. Mutations: 15 in 16 runs red (the item's file blob and `written` serialized are no one-line mutants); UI 12 red. TE-12: not run.

Known limits: `proposal-drift.md`. Carried to 4c-2 (`drift-follow-up.md` "Implementation"): `create.rs`, `decide.rs`' six raw pathspecs; the mock's stale-stage text. Optional nits: `preflight.rs`'s doc ("and holding it"), the `Lock-mode` reason's wording, a `StoredJson` note on `arbitrary_precision`, CLI `round.rs` citing a "Round" heading; code pointers to this spec's "Data" may move to `docs/canon/proposal-drift.md`.
