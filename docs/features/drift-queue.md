---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-1 (split 2026-10-13, Q19; WA-1..WA-15, Q1-Q20); refreshed 2026-10-13 against shipped spec-lock, spec-verify; 05 s5.3; 06 s4; 07 s2; 08 s2
adrs: [ADR-0045]
---

# drift-queue (4c-1): drift reaches the owner's queue; approving it runs the lock accept

## Why

Only whoever runs the read-only `spec verify` sees drift; its Pre-existing findings "go to the backlog" (05:188), which does not exist; 05 s5.3 and 06 s4 give drift three outcomes (accept the code, send it back, change the spec). ADR-0045 amends ADR-0041 and ADR-0042 to allow it (`architecture.md#apply`, `#control`; `architecture-code.md#spec-lock`).

**4c-2** (`drift-follow-up.md`) takes `spec reject --follow-up`; the rest of ADR-0045 is here.

## Description and interactions

- **An agent**, the owner or a project's own hook (documented, not shipped) raises items once `spec verify` (still read-only) shows drift, typing no evidence. Not `report_discrepancy`: its approval writes a decision record.
- **The owner**, on a terminal where an item was raised, approves (the lock takes the node's pair at `HEAD`), approves `--editorial` (an editorial `spec_ahead same_revision`, or `conflict` at R = `rev_L`; `spec_changed` has no entry at R: a plain approve locks it), or rejects. A hand `spec lock accept` writes no queue row (`spec-lock.md` "Travel and writes", `lock_accept_writes_no_queue_row`); the next raise notes the item (Q6).
- **After 4b-1** the commonest item is `spec_ahead marker_behind` on each binding of a node whose meaning change was applied: not lockable (`spec-lock.md` "Lockability"), so approve exits 1; 4c-2's follow-up settles it.
- **Crates**: as `scope`; HTTP no code; no new crate or dependency. **Unchanged**: `spec verify` but a hint, `spec check`, gate, hook, CI, task states, `INDEX_FORMAT`, `spec.lock`'s format, other kinds' apply, a hand `spec lock accept`'s bytes. **Writes**: the queue, the index; approve also `spec.lock`, one commit.
- **Overridden live-spec lines**, fixed at shipping (SW-4): 05:180, 05:181, 05:183 ("a rebind proposal"), 06:118-123, 07:26, 07:91; 07:95 is 4c-2's.

## Data

### spec propose drift

Q1, Q3, Q15, Q16. `spec propose drift [REF…] [A] [--task T] [--json]` (`A`: `agent-intake.md` "Tools"; `--config` as the queue's).

- **Order**: discovery, config; `specengine.toml` tracked and clean (exit 1 naming its state, `commit it first`); `[code]`; the place as `proposal-queue.md` "Creation" 5, `base_commit` the `HEAD` it reads; `--task` (`tasks.md` "Task-bound proposals"); the commit's side; REFs; its rows judged; the insert.
- **The commit's side** (Q16): a new `pub(crate)` judge of one commit in CLI `verify_rows.rs`, shared with 4b-2's `--staged` (`rev-rule.md` "spec verify --staged"; the first to ship builds it): `list_commit(<base_commit>)`, `read_files`, `Files::Commit`, `tree_of_blobs` with an empty reuse tree, the commit's documents, config, `spec.lock`. The OID the place bound, never `"HEAD"` again (`read_head` re-reads it). No `read_tree`, `hash-object`, working-tree code or worktree lock; `judge_walk`, `read_head` as built.
- **REF**, 0-16 (Q15): resolved as `spec show` does, over the commit's documents: a node (not its sections), or a gone ID with entries in its lock (`spec-lock-accept.md` "A gone node"). A node only the working tree has: exit 1 `` `R-30` names no node at HEAD: commit it first ``. None: a **sweep**.
- Uncommitted edits are never judged; rows the disk already fixed are queued: the queue is the backlog.

### Triggers

Q2, Q20. Per node (canonical, or the gone ID): `code_ahead`, `spec_ahead`, `conflict`, `broken_binding` raise an item (05:180-183); `unlocked` only for a named REF, never in a sweep (adoption: `spec-verify.md` "Known limits"); `cannot_verify`, `bad_marker`, `predated`, `unlocked key_problem` never on their own (05:185-186), a named node's `key_problem` rows giving ``note: R-30: <n> binding(s) cannot be locked (key_problem); spec verify names each``. The item lists all its node's non-`ok` rows; severity `high` when one is class `fix`, else `normal`.

### The item

A `proposals` row, kind `drift`: `target_id` the node (or gone ID); `target_path` `spec.lock` (root-relative); `target_ids` `[target_id]`; the place (ADR-0032), `base_commit` the commit judged; `severity`; `summary` its rows' distinct `state[ category]` by key, `, `-joined; `author`, `task_id` as the intake's; `diagnostics` `[]`; `drift`, `drift_hash`; all else NULL.

- `drift`: compact JSON `{"rows":[<row>…],"more":<n>}`, each row a `spec-verify-output.md` "JSON" `rows` item (`pre_existing` `true`; CLI `verify.rs` `shown()` made `pub(crate)`), the first 64 (`DRIFT_ROWS_MAX`) by key, `more` the rest.
- `drift_hash` = `b3_hash("specengine-drift/v1" LF <compact JSON>)`, one array per non-`ok` row (all), by key (Q13): `[node, relation, file, kind, symbol, item_kind, state, category, detail, marker_revs, lock_rev, lock_spec_hash, lock_code, lock_file_blob, rev, norm_hash, code, moved_from]`:
  - `marker_revs`: `null` (unversioned) first, the `@N` ascending, then `"bad"` (core `MarkerRev::Bad`);
  - `lock_*`: the entry's, or `null`; `lock_code` in format 1 its `ast_hash` (`null` for `module`, `ron`), in format 2 (5b) its code fields as core `render_block` writes them, one string (`lock-levels.md` Q7);
  - `rev`, `norm_hash`: the node's R and H at the commit (4a's `Row` gains H);
  - `code`, the cascade's current fact (the row gains it): an item compared `Changed(Ast)` its digest, a `module` or `ron` row compared `Changed(Blob)` its blob OID, else `null`. No item `file_blob`: a moved marker line or another edit in the item's file is the same drift;
  - `moved_from`: the row's `anchor.from` as `[file, kind, symbol, item_kind]`, else `null`; `null` until 5c ships (`anchor-states.md` Q6: reserved, so no `v2`).
  - Not hashed: lines, times, the entry's `commit` and `verified_at`, the commit judged, the mode.

### Kind family

`ProposalKind` (`ALL` 7) has `applies()`, `decides()`, each a `matches!` (store `queue.rs`): a new variant is neither; pinned "one, never both" (store `tests/queue_decision.rs`). `Drift` joins (`ALL` 8) with an exhaustive `family() -> KindFamily {Applies, Decides, Locks}` replacing both, so each of the 28 call sites in `src` (store 11, CLI 17: `interpret.rs`, `create.rs`, `stage.rs` too; 9a's if first, `health-code.md` "Interplay") and the tests using them are judged again. Store `create`, `create_intake`, `approve_record_from` refuse a drift (`Invalid`); `approve_from`, `applied_with`, `reopen_from`, `log_failure`, `reject_from` take it; `create_drift` alone stores one.

### Queue schema 8

After 4b-1's 7, which adds no column. Step 8, one `Immediate` transaction: `ADD COLUMN drift TEXT`, `drift_hash TEXT`; `QUEUE_SCHEMA_VERSION` 8, `PROPOSAL_COLUMNS` 46, `proposal_columns(6)` and `(7)` the first 44 (the store's `STEP_n` pattern); 9 -> exit 2; older builds refused (`SchemaTooNew`). **Corrupt**: a drift row with either column NULL, JSON off its shape (a row's later keys, 5b's `levels`, 5c's `anchor`, are not), a hash not `b3:` + 64 hex, a stage; another kind with either set. **Backup** (`queue-backup.md`): format 2, `queue_schema` 8, 46 columns; import takes schemas 4-8; a byte-identical round trip.

### Dedup

Q4, Q14. No `distinct_from`: the evidence is computed. Store `create_drift(&[NewDrift], now)`, in the inserting `BEGIN IMMEDIATE`, keyed (`git_common_dir`, `branch`, `target_id`): an `open` or `approved` drift item is a **hit** (another hash: `evidence changed since it was raised`); a `rejected` one with an equal `drift_hash` a hit, settled (the newest such named); else stored, IDs in node byte order, a `proposal.created` each. An `applied` item never settles (Q14): equal evidence means its lock commit was undone, so the new item gets ``note: PR-0013: R-14 drifts as when PR-0004 was applied by <sha>; that lock is not in effect at the commit judged``. Busy over 5 s: exit 2. Each `open` item of this repository and branch whose node was judged all `ok`: ``note: PR-0007: R-20 reads ok at HEAD now; spec reject PR-0007 --reason … settles it``.

### Output

Text, by node bytes (a named REF without a trigger: `note: R-30: no drift at HEAD`):

```
created PR-0012 | R-12 | high | conflict, code_ahead code_changed
queued PR-0009 | R-13 | open | evidence changed since it was raised
settled PR-0004 | R-14 | rejected | same evidence
propose drift: 1 created, 2 hit(s); 14 rows of 5 nodes judged at <40 or 64 hex> on main
```

JSON, the drift document (MCP `structuredContent`), every key, absent `null`: `{commit, branch, judged: {rows, nodes}, created: [{id, node, severity, summary}], hits: [{id, node, status, same}], notes}`.

### report_drift and texts

Q12. `report_drift {node_ids?: 1-16, task_id?, author_role, author_model?, run?}`, `additionalProperties: false`, no field for rows or hashes; the intake writers' annotations and description (`agent-intake.md` "Tools"); place: the server's root.

- `INSTRUCTIONS` gains `- report_drift = spec propose drift: queues HEAD's lock drift for the owner; evidence computed, never typed.`, ASCII, within `TEXT_LIMIT` with the probe text (MCP `server.rs`; `code-tools.md` re-measures).
- `ask-owner` (`plugin.md` "Skills", a PATCH on top of 4b-1's) gains `Drift between spec.lock and the code (spec verify findings): report_drift, no node_ids for every drifting node, a named node also when never locked; it takes no evidence; the owner decides on a terminal.`
- Verify's hint (`spec-verify-output.md` "Hints"): once, when a row is `code_ahead`, `spec_ahead`, `conflict` or `broken_binding`, after the other hints, `hint: drift: commit first; spec propose drift queues HEAD's drift for the owner`.

### Review, inbox, round, stage

Q8, Q18. **Review** (48 keys; 4b-1 makes 47): `drift` after `revisions`, `{"hash","rows","more"}` as stored, `null` on other kinds; text `drift: <n> row(s)` and the rows in 4a's form, indented; `diff`, `conflict`, `base_text`, `new_text`, `revisions` `null`. **Preview** (open, approved): `spec review` runs `lock_plan` on the place's `HEAD` (no identity; writes the index only): `applies` (note `accept: 2 new, 1 refreshed, 0 kept, 0 dropped`) or `unavailable` (`not applicable now (step <n>): <reason>`); `--brief` (MCP `get_proposal`) and HTTP: `null`, ``note: preview: spec review PR-0012 on a terminal computes it`` (CLI `ReviewRequest` gains a field only `spec review` sets). **Inbox**: `PR-0012 | drift | open | R-12 | main | <created_at> | conflict, code_ahead code_changed`. **Round**: off the sheet (`Decides` only); note ``<n> open drift item(s) not on this sheet: <IDs>; spec review <ID>, then spec approve or spec reject``. **Stage**: never (`Invalid`, HTTP 409). **UI**: kinds are strings; no screen until Phase 4.

### Accept's seams

Q17. CLI `lock.rs`, one implementation for `spec lock accept`, the approve and the preview:

- `lock_plan(env, globals, request) -> LockPlan`: steps 2-7, writing only the index. `lock_commit(env, plan, consent, trailers) -> LockOutcome`: steps 8-10. `lock_accept` runs the two.
- `LockAcceptRequest` gains `proposal: Option<String>`: each mode's question gains ` for PR-0012` before ` at <commit>`; the trailer `Proposal: PR-0012` follows `Decided-by:` (the shipped order, `lock_commit_trailers_parse`; `%(trailers:key=…)` lookups ignore order); the text-changed refusal names `spec approve PR-0012 --editorial`.
- Step 10's fifth check: every caller trailer present with its value (store `trailer_values`), else "a check fails" (`spec-lock-accept.md` "The lock commit"), so a hook dropping `Proposal:` leaves no `applied` item.
- `LockOutcome` gains `written`: `none`, `committed` (verified), `restored` (bytes restored), `unverified` (a commit made, failing a check or unreadable), `left` (bytes left as written). The approve maps it, never parsing refusal texts.

### Approve

Q5, Q9. `spec approve PR [--note T] [--editorial]`: `--option`, `--answer`, `--canon` exit 2 (4b-1's flag check lets `--editorial` through on `drift`); applied or rejected exit 1; the trailer lookup first. Then apply's steps and runs (`proposal-apply.md` "Apply steps"), through accept's plan and commit (`spec-lock-accept.md` "Steps", "`--editorial`", "A gone node"), never a second implementation:

| Apply step | Drift approve |
|---|---|
| 1 | `require_terminal` before any read |
| 2 Place | apply's; `HEAD` past `base_commit`: `note: HEAD moved since PR-0012 was raised`, never a refusal |
| 3 | `lock_plan`, `Globals { root: <recorded root> }` (never the cwd's), `proposal` the ID: accept 2-4 for `target_id` |
| 4-5 | accept 5-6 (text changed at R: exit 1) |
| 6 | accept 7, nothing to change: exit 1 ``PR-0012 not applied: nothing to accept now for R-12 (<why>); spec reject PR-0012 --reason … settles it``, still open (Q6) |
| ask | `lock_commit`'s step 8 (no identity: exit 2, logged as step 7); `lock R-12 rev 3 (accept: 2 new, 1 refreshed, 0 kept, 0 dropped) for PR-0012 at <commit> on <branch> in <worktree>? [y/N]` |
| 7 | `approve_from` in the consent closure after `y`; its compare-and-set lost: `PR-0012 changed meanwhile; nothing written` |
| 8-10 | accept 9-10: re-read, write, `commit_only` with hooks; `restored`: `reopen_from`; `unverified`, `left`: stays `approved` (exit 1 both); five checks -> `applied_with`, `proposal.applied` |

Message: `spec: lock R-12`, an empty line, `Lock: R-12`, `Lock-mode: accept|editorial`, `Decided-by: <ident>`, `Proposal: PR-0012`. The stored `drift`, `drift_hash` are never read; no spec file or revision written, so 4b-2's rule parses neither lock (`rev-rule.md` "Rules and edge cases"). `--editorial` refreshes every entry at R, accept's (not 4b-1's vouching filter): the owner confirms the entries listed. **Completion** (per-kind seams in CLI `preflight.rs`: `trailer_commit`'s judging, `completes_when`): ``a commit on `<branch>` with the trailers `Proposal: PR-0012` and `Lock: R-12`, one parent, changing only `<root_rel>/spec.lock` completes it (`spec approve PR-0012`)``, in `base_commit..<branch>`. **Reject** (plain): refused while one exists.

### Exits

`proposal-queue.md` "Exit codes", apply's and accept's (`spec-lock-accept.md` "Exits"), plus: **raise** 0 judged (created, hits or nothing); 1 a REF naming nothing, several nodes or a working-tree-only node, a refused `--task`, a dirty `specengine.toml`; 2 usage, the author grammar, a look-alike, `PR` in `[ids]`, no worktree, an unborn or detached `HEAD`, 4a's run causes but (5), at the commit (`spec-verify.md` "Verdict and exit"; under 5b a format-1 lock, with the rebase hint), the queue. **Approve** 1 nothing to accept, a lost compare-and-set, `written` not `committed`; 2 a decision flag, no identity.

## Rules and edge cases

- WHEN raised from two roots, another cwd, in reverse creation order or on a CRLF checkout, stdout, JSON and stored rows SHALL be byte-identical; WHEN two raises race, one item per key SHALL be stored.
- WHEN an item is approved, `spec.lock` SHALL hold the bytes `spec lock accept <target_id>` writes on that commit, in a commit of it alone.
- No stack, runner or kind literal outside the kind table (the genre scans), none in `specengine-mcp/src`. Nothing blocks (ADR-0012, ADR-0042 as amended): no status, claim, gate, check or other approval reads a drift item.

## Acceptance criteria

rust-developer, mutation -> the test turning red:

- [ ] RD-1 -- drift as `applies()` -> `drift_item_never_runs_the_update_apply`; the old pin kept -> `every_kind_has_exactly_one_family`; 9a's call site (if 9a is first) -> its health test.
- [ ] RD-2 -- `unlocked` in a sweep -> `sweep_never_queues_a_never_locked_node`; `cannot_verify` alone -> `cannot_verify_alone_raises_nothing`; a named `key_problem` row raising -> `named_key_problem_is_a_note`; `std::fs` in core -> `model_and_core_sources_do_no_file_io`.
- [ ] RD-3 -- the disk judged -> `uncommitted_edit_is_not_queued`, `raise_ignores_an_unreadable_disk_file`; `HEAD` re-read -> `raise_judges_the_bound_base_commit`; REF in the working tree -> `uncommitted_ref_is_refused`.
- [ ] RD-4 -- MCP takes `rows` -> `report_drift_takes_no_evidence`; approve reads the stored column -> `approve_never_writes_stored_values`.
- [ ] RD-5 -- no dedup -> `second_raise_stores_nothing`; settled ignored -> `rejected_drift_is_not_raised_again`; lines hashed -> `moved_marker_line_is_the_same_drift`; the item's file blob hashed -> `edit_beside_the_item_is_the_same_drift`; `applied` settling -> `reverted_lock_commit_raises_again`; branch ignored -> `drift_on_two_branches_is_two_items`; a race -> `concurrent_raises_store_one_item`.
- [ ] RD-6 -- a second accept -> `drift_approve_writes_the_bytes_lock_accept_writes`; the node's file committed -> `drift_approve_commits_only_the_lock`; `--no-verify` -> `drift_approve_runs_hooks`; `Proposal:` first -> `lock_commit_trailers_parse`; no fifth check -> `dropped_proposal_trailer_fails_step_10`.
- [ ] RD-7 -- another worktree -> `drift_approves_only_where_raised`; a read before the terminal check -> `drift_approve_needs_a_terminal_first`; a dirty marker file -> `dirty_bound_file_refuses_drift_approve`; the cwd's root -> `drift_approve_locks_the_recorded_root`.
- [ ] RD-8 -- code hashes refreshed -> `drift_editorial_keeps_code_hashes`; exit 2 -> `drift_editorial_runs_accept_editorial`; on `spec_changed` -> `editorial_without_entry_at_r_refuses`; vouching filter -> `drift_editorial_refreshes_every_entry_at_r`.
- [ ] RD-9 -- a killed run re-committed -> `killed_drift_approve_completes_by_trailer`; the lock commit ignored -> `reject_refuses_a_drift_item_with_its_lock_commit`; reopened after a commit -> `drift_approve_reopens_only_restored_bytes`.
- [ ] RD-10 -- nothing to accept, yet `applied` -> `drift_with_nothing_to_accept_is_not_applied` (`marker_behind` only); a preview needing an identity -> `drift_preview_needs_no_identity`. RD-11 -- a stage stored -> `drift_item_cannot_be_staged`.
- [ ] RD-12 -- a key dropped -> the 48-key pins (`proposal_create.rs` `REVIEW_KEYS`, `kind_interpretation.rs` AC-R7, `kind_amendment.rs` AC-R10); backup bytes lost -> `drift_rows_round_trip_byte_identical`. RD-13 -- an MCP write under the root -> AC-3's `git status` test; `INSTRUCTIONS` too long -> `server.rs`'s assert.
- [ ] RD-14 -- walk order leaks -> `drift_raise_is_byte_identical_across_roots_and_order`. RD-15 -- a domain literal -> the genre scans. RD-16 -- a new crate -> the CLI pin in eval `build_graph.rs`. RD-17 -- the plugin version kept -> `ac10_the_content_and_version_are_the_last_pin`. RD-18 -- the hint missing -> a hint test in `verify_states.rs`.
- [ ] RD-19 -- seams, the code-reviewer: `verify_judges_a_given_walk`, `judge_is_source_neutral` green; accept, approve, review share `lock_plan`, `lock_commit`; `tests/lock_commit.rs` unchanged but the fifth check's test.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/`, golden lock `fixtures/spec-lock/code-a.lock`: a scenario per trigger state and category; other states raise nothing; a named REF's `unlocked` rows raise, `key_problem` rows note.
- [ ] TE-2 -- raise -> review (`applies`) -> approve -> `spec verify` `ok`; the lock bytes a twin repository's after `spec lock accept`. TE-3 -- each exit 1 and 2 of raise and approve: `git status` (untracked too), `HEAD`, `spec.lock`, the queue unchanged.
- [ ] TE-4 -- dedup {open, approved, applied, rejected} x {same, other evidence} x {same, other branch}; two concurrent raises, self-terminating. TE-5 -- killed runs (before, after the commit), a failing hook: restored, reopened; a hook dropping `Proposal:`: `approved`.
- [ ] TE-6 -- schema 6, 7 -> 8; a drift row with a stage, a NULL column or a bad hash, another kind with either column: corrupt. TE-7 -- MCP parity with `--json`; AC-3; an unknown field refused; `get_proposal`'s `preview` `null`, its note.
- [ ] TE-8 -- the lock commit's hooks: `spec check --staged` passes; `spec verify --changed` (`--staged` after 4b-2) passes under `enforce-introduced`, refuses under `enforce` with another node's Fix row (restored, reopened); `--editorial` after a `--no-verify` text commit -> `ok`.
- [ ] TE-9 -- two roots, another cwd, reverse creation order, CRLF: identical. TE-10 -- `fixtures/spec-c` exits 2 naming `[code]`; the source scans. TE-11 -- HTTP stage 409; `daemon-keys.json` 48 review keys.
- [ ] TE-12 -- pilots, manual, as `verify_pilot.rs` does, each scratch clone locked by the test's own accept (slice 3's TE-11 never ran); items per state, raise time against AC-10; no pilot name committed.

ui-developer:

- [ ] UI-1 -- provisional types and mock gain `kind: "drift"`, the key `drift`; the key missing -> `ui/src/api/daemonKeys.test.ts`.

spec-writer:

- [x] SW-1 -- this spec, refreshed and split 2026-10-13. SW-2 -- ADR-0045; `architecture.md#apply`, `#control`. SW-3 -- the drafts naming 4c, 08 s2, 05:188. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: Tier 2 canon `docs/canon/proposal-drift.md`; `spec-lock-accept.md` (plan, commit, `proposal`, fifth check, `written`); `proposal-{queue,apply,completion,review}.md`, `agent-intake.md`, `tasks.md`, `decision-staging.md`, `round.md`, `queue-backup.md` (schema 8, import 4-8), `mcp-read.md`, `plugin.md`, `plugin-files.md`; `family()` for `applies()`/`decides()` there and in `proposal-{kinds,decision,interpretation,amendment}.md`, `decision-record{,-template}.md`; `spec-verify-output.md` "Hints"; `architecture-code.md#spec-lock`, `#verify`; the overridden lines; store, CLI, MCP READMEs; `CLAUDE.md` State.

## Risks

Noise: a partial fix re-raises; a removed worktree's items are only rejected (ADR-0045's Cost); 5b's rebase re-raises rejected items once. A project's `spec verify` hook under `enforce` fails the lock commit over unrelated Fix rows (TE-8).

## Out of scope

A daemon, watcher or CI raising items (06:118); drift screens, UI staging (Phase 4); the Health drift row (9b); several REFs in one item; a hand accept settling items; `@assumes` tasks (06:103); 4c-2's `--follow-up`, follow-ups for other kinds; 5b `spec lock rebase`; 8b `[[tested]]` (`tested-evidence.md`).

## Open

Owner pre-approved, review open (Q1-Q12 2026-10-10, Q13-Q20 2026-10-13); each answer where its Q is cited. The other answer costs:

- Q1 `verify --queue`: verify writes; CLI-only raises store agents as `human`. Q2 always: an adoption flood; never: hand accepts only. Q3 the disk: unfinished edits queued. Q4 by state: new code missed; none: settled questions repeated (05:221). Q5 stored hashes "verified"; two accepts. Q6 settling without a `[y/N]` (ADR-0038). Q7 (4c-2) `applied` without a commit. Q8 a stage key, a schema step. Q9 spec-side items only rejectable. Q10 code against ADR-0041/0042. Q12 agents on Bash.
- Q13 the moved-marker test fails, any file edit re-raises. Q14 a reverted lock hides drift for good. Q15 a dirty rename judged as another node. Q16 a bad disk file or worktree lock refuses a raise about `HEAD`. Q17 the preview needs an identity; refusal texts parsed. Q18 a code walk per `get_proposal`, per UI GET. Q19 (Q11's "no split" superseded) the commonest item's tasks by hand. Q20 an item only ever rejected.

## Dependencies and ordering

- **Needs**: slices 1, 3, 4a, shipped; 4b-1 `rev-discipline`, shipped 2026-10-14 (schema 7, the `--editorial` flag check, 47 review keys, `commit_only_paths`: `proposal-revisions.md`).
- **Order**: 1 -> 3 -> 4a -> 4b-1 -> 4c-1 -> 4c-2. **Independent**: 8a; 4b-2 (the commit's judge shared; its rule silent on the lock-only commit); 8b (one accept keeps its blocks; the later of 4c-1 and 8b tests a drift approve). **Not independent of 5b** (`lock-levels.md` Q7): `lock_code` as rendered; a rebase commit settles nothing. **Later**: 5c fills `moved_from`; 9a takes `family()`; 9b re-reads on a drift approve's `proposal.applied`; Phase 4 screens.

## Implementation

Not started. To record: modules, TE-12's figures, deviations with reasons. Carry-over: "slice 4c" in CLI `lock.rs` (module, `LockTrailer` docs) and `tests/lock_commit.rs` -> canon `proposal-drift.md` at SW-4. From 4b-1, code only: CLI `lock.rs` undo messages (~1864, ~1892) quote pathspecs by `revisions::pathspec_word` at the `lock_plan`/`lock_commit` split; `apply.rs` module doc (~32) and `verify_leaving` doc (~848): "left written and staged" -> "stay as the commit left them". Criteria renumbered at the split (2026-10-13), old -> new: RD-1..RD-10 kept; RD-11 -> 4c-2 RD-1; RD-12..RD-19 -> 11..18 (RD-16's follow-up half -> 4c-2 RD-2); RD-19 new; TE-1..TE-12 kept (TE-6's follow-ups -> 4c-2 TE-1); UI-1, SW-1..SW-5 kept (follow-up canon -> 4c-2 SW-2). 4c-2: `drift-follow-up.md`.
