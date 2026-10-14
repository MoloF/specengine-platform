---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c-1 (WA-1..WA-15, Q1-Q20; split 2026-10-13, Q19; refreshed 2026-10-13, 2026-10-14 with Q-A..Q-D); 05 s5.3; 06 s4; 07 s2; 08 s2
adrs: [ADR-0045]
---

# drift-queue (4c-1): drift reaches the owner's queue; approving it runs the lock accept

## Why

Only whoever runs `spec verify` sees drift; the backlog its Pre-existing findings "go to" (05:188) does not exist. 05 s5.3, 06 s4 give drift three outcomes (accept the code, send it back, change the spec); ADR-0045 allows them (`architecture.md#apply`, `#control`; `architecture-code.md#spec-lock`). 4c-2 (`drift-follow-up.md`) takes `spec reject --follow-up`.

## Description and interactions

- **An agent**, the owner or a project's own hook (documented, not shipped) raises items once `spec verify` shows drift, typing no evidence. Not `report_discrepancy`: its approval writes a decision record.
- **The owner**, on a terminal where an item was raised, approves (the lock takes the node's pair at `HEAD`), approves `--editorial` (`spec_ahead same_revision`: text committed outside apply, an editorial approve's non-vouching entries, a refresh `spec lock regen` dropped; `conflict` at R = `rev_L`; not `spec_changed`, which a plain approve locks), or rejects. A hand `spec lock accept` writes no queue row (`spec-lock.md` "Travel and writes"); the next raise notes the item (Q6).
- **After 4b-1** the commonest item, `spec_ahead marker_behind`, is not lockable: approve exits 1, 4c-2's follow-up settles it.
- **Crates**: as `scope`; HTTP no code (Q-A); no new crate or dependency. **Unchanged**: `spec verify` but a hint, `spec check`, gate, hook, CI, task states, `INDEX_FORMAT`, `spec.lock`'s format, other kinds' apply, a hand accept's bytes. **Writes**: the queue, the index; approve also `spec.lock`, one commit.
- **Overridden live-spec lines** (SW-4): 05:180, 05:181, 05:183 ("a rebind proposal"), 06:118-123, 07:26, 07:91.

## Data

### spec propose drift

Q1, Q3, Q15, Q16. `spec propose drift [REF…] [A] [--task T] [--json]` (`A`: `agent-intake.md` "Tools"; `--config` as the queue's).

- **Order**: discovery, config; `specengine.toml` tracked and clean (exit 1 naming its state, `commit it first`); `[code]`; the place as `proposal-queue.md` "Creation" 5, `base_commit` the `HEAD` it reads; `--task` (`tasks.md` "Task-bound proposals"); the commit's side; REFs; its rows judged; the insert.
- **The commit's side** (Q16): a `pub(crate)` one-commit judge in CLI `verify_rows.rs`, built here (before 4b-2, 08:41), shared with its `--staged`: `list_commit`, `read_files`, `Files::Commit`, `tree_of_blobs` (empty reuse tree) on the OID the place bound (its documents, config, `spec.lock`), never `"HEAD"`; no working-tree read or worktree lock; `judge_walk`, `read_head` as built.
- **REF**, 0-16 (Q15): resolved as `spec show` does, over the commit's documents: a node (not its sections), or a gone ID with entries in its lock (`spec-lock-accept.md` "A gone node"). A node only the working tree has: exit 1 `` `R-30` names no node at HEAD: commit it first ``. None: a **sweep**.
- Uncommitted edits are never judged; rows the disk already fixed are queued: the queue is the backlog.

### Triggers

Q2, Q20. Per node (canonical, or the gone ID): `code_ahead`, `spec_ahead`, `conflict`, `broken_binding` raise an item (05:180-183); `unlocked` only for a named REF, never in a sweep; `cannot_verify`, `bad_marker`, `predated`, `unlocked key_problem` never on their own (05:185-186), a named node's `key_problem` rows giving ``note: R-30: <n> binding(s) cannot be locked (key_problem); spec verify names each``. The item lists all its node's non-`ok` rows; severity `high` when one is class `fix`, else `normal`.

### The item

A `proposals` row, kind `drift`: `target_id` the node (or gone ID); `target_path` `spec.lock` (root-relative); `target_ids` `[target_id]`; the place (ADR-0032), `base_commit` the commit judged; `severity`; `summary` its rows' distinct `state[ category]` by key, `, `-joined; `author`, `task_id` as the intake's; `diagnostics` `[]`; `drift`, `drift_hash`; all else NULL.

- `drift`: compact JSON `{"rows":[<row>…],"more":<n>}`, each row a `spec-verify-output.md` "JSON" `rows` item (`pre_existing` `true`; CLI `verify.rs` `shown()`), the first 64 (`DRIFT_ROWS_MAX`) by key, `more` the rest.
- `drift_hash` = `b3_hash("specengine-drift/v1" LF <compact JSON>)`, one array per non-`ok` row (all), by key (Q13): `[node, relation, file, kind, symbol, item_kind, state, category, detail, marker_revs, lock_rev, lock_spec_hash, lock_code, lock_file_blob, rev, norm_hash, code, moved_from]`:
  - `marker_revs`: `null` (unversioned) first, the `@N` ascending, then `"bad"` (core `MarkerRev::Bad`);
  - `lock_*`: the entry's, or `null`; `lock_code` in format 1 its `ast_hash` (`null` for `module`, `ron`), in format 2 (5b) its code fields as core `render_block` writes them, one string (`lock-levels.md` Q7);
  - `rev`, `norm_hash`: the node's R and H at the commit (4a's `Row` gains H);
  - `code`, the cascade's current fact (the row gains it): an item compared `Changed(Ast)` its digest, a `module` or `ron` row compared `Changed(Blob)` its blob OID, else `null`. No item `file_blob`: a moved marker line or another edit in the item's file is the same drift;
  - `moved_from`: the row's `anchor.from` as `[file, kind, symbol, item_kind]`, else `null`; `null` until 5c ships (`anchor-states.md` Q6: reserved, so no `v2`).
  - Not hashed: lines, times, the entry's `commit` and `verified_at`, the commit judged, the mode.

### Kind family

Store `ProposalKind`'s `applies()`, `decides()` (`ALL` 7; "one, never both", `tests/queue_decision.rs`) give way to an exhaustive `family() -> KindFamily {Applies, Decides, Locks}` (`Drift` joins, `ALL` 8): 29 call sites (store 11, CLI 18; 9a's if first, `health-code.md` "Interplay") and their tests judged again. "Writes no existing node" (CLI `editorial_refusal`, store `Stage::problem`) is `family() == Decides || new_file()`, else `--editorial` on a drift item exits 2. Store `create`, `create_intake`, `approve_record_from` refuse a drift (`Invalid`); `approve_from`, `applied_with`, `reopen_from`, `log_failure`, `reject_from` take it; only `create_drift` stores one.

### Queue schema 8

After 4b-1's 7 (no column: `stage::upgrade_stages`). Step 8, in the due steps' `Immediate` transaction: `ADD COLUMN drift TEXT`, `drift_hash TEXT`; `QUEUE_SCHEMA_VERSION` 8, `PROPOSAL_COLUMNS` 46; `proposal_columns`' one arm from 6 splits: 6-7 the first 44, 8 all 46; 9 -> exit 2; older builds refused (`SchemaTooNew`). Restore still runs `upgrade_stages`; import follows the constant. **Corrupt**: a drift row with either column NULL, JSON off its shape (a row's later keys, 5b's `levels`, 5c's `anchor`, are not), a hash not `b3:` + 64 hex, a stage; another kind with either set. **Backup** (`queue-backup.md`): format 2, `queue_schema` 8, 46 columns; import takes schemas 4-8; a byte-identical round trip.

### Dedup

Q4, Q14. No `distinct_from`: the evidence is computed. Store `create_drift(&[NewDrift], now)`, in the inserting `BEGIN IMMEDIATE`, keyed (`git_common_dir`, `branch`, `target_id`): an `open` or `approved` item is a **hit** (another hash: `evidence changed since it was raised`); a `rejected` one with an equal `drift_hash` a hit, settled (the newest named); else stored, IDs in node byte order, a `proposal.created` each. An `applied` item never settles (Q14): equal evidence means its lock commit was undone; the new item gets ``note: PR-0013: R-14 drifts as when PR-0004 was applied by <sha>; that lock is not in effect at the commit judged``. Busy over 5 s: exit 2. Each `open` item of this repository and branch whose node was judged all `ok`: ``note: PR-0007: R-20 reads ok at HEAD now; spec reject PR-0007 --reason … settles it``.

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

- `INSTRUCTIONS` gains `- report_drift = spec propose drift: queues HEAD's lock drift for the owner; evidence computed, never typed.`, ASCII (`code-tools.md` re-measures).
- `ask-owner` (`plugin.md` "Skills", a PATCH on top of 4b-1's) gains `Drift between spec.lock and the code (spec verify findings): report_drift, no node_ids for every drifting node, a named node also when never locked; it takes no evidence; the owner decides on a terminal.`
- **Measured before building**, each with its line: `ask-owner`'s body (4 096 B, MCP `tests/plugin_files.rs`), `INSTRUCTIONS` plus the probe text (2 048, `server.rs`).
- Verify's hint (`spec-verify-output.md` "Hints"): once, when a row is `code_ahead`, `spec_ahead`, `conflict` or `broken_binding`, after the other hints, `hint: drift: commit first; spec propose drift queues HEAD's drift for the owner`.

### Review, inbox, round, stage

Q8, Q-A. **Review** (48 keys): `drift` after `revisions`, `{"hash","rows","more"}` as stored, `null` on other kinds (MCP `DriftShape`; `daemon-keys.json` `DriftEvidence`, 46 sets; rows untyped until Phase 4); text `drift: <n> row(s)`, the rows in 4a's form, indented; `diff`, `conflict`, `base_text`, `new_text`, `revisions` `null`. **Preview** `null` everywhere, no walk; an open or approved item notes ``spec approve PR-0012 shows the rows before asking``. **Inbox**: `PR-0012 | drift | open | R-12 | main | <created_at> | conflict, code_ahead code_changed`. **Round**: off the sheet (`Decides` only); note ``<n> open drift item(s) not on this sheet: <IDs>; spec review <ID>, then spec approve or spec reject``. **Stage**: refused by CLI `stage.rs` `checked` before `preview_run` (`StageCause::Refused`, HTTP 409) ``PR-0012 is a drift item, decided on a terminal only; nothing staged``, and store `Stage::problem` (`Invalid`).

### Accept's seams

Q-A: no split; CLI `lock.rs` `lock_accept` serves `spec lock accept` and the approve. `LockAcceptRequest` gains:

- `proposal: Option<String>`: each question gains ` for PR-0012` before ` at <commit>`; `Proposal: PR-0012` follows `Decided-by:`; the text-changed refusal's parenthetical reads `(spec approve PR-0012 --editorial)`, the editorial no-entry-at-R one `spec approve PR-0012`;
- `branch: Option<String>`: step 3 refuses another, exit 1 ``HEAD left `<branch>` after PR-0012's place was checked; nothing written``.

Step 10's fifth check: each caller trailer present with its value (store `trailer_values`), else "a check fails": a hook dropping `Proposal:` leaves no `applied` item. `LockOutcome` gains `written` (`#[serde(skip)]`: `--json` keeps eight keys): `none`, `committed` (verified), `restored` (`Undo`'s `Restored::Back`), `unverified` (a commit made, failing a check or unreadable), `left` (not restored); the approve maps it, never parsing texts. Q-D: `Undo`'s five raw pathspecs (~1860, ~1864, ~1879, ~1892, ~1899) by `pathspec_word`, moved beside `one_line` in CLI `lib.rs`; its warnings name the worktree.

### Approve

Q5, Q9, Q-B. `spec approve PR [--note T] [--editorial]`: `--option`, `--answer`, `--canon` exit 2, `flag_on_apply` saying ``spec approve PR-0012 runs the lock accept for R-12``. `run_approve`'s drift branch, after `flag_on_apply`: the trailer lookup, then apply's steps and runs (`proposal-apply.md` "Apply steps") through `lock_accept` (`spec-lock-accept.md`). The `--editorial` help adds a drift item's `spec lock accept --editorial`.

| Apply step | Drift approve |
|---|---|
| 1 | `require_terminal` before any read |
| 2 Place | apply's; `HEAD` past `base_commit`: `note: HEAD moved since PR-0012 was raised` |
| 3-6 | `lock_accept`, `Globals { root: <recorded root> }` (not the cwd's), `proposal`, the recorded `branch`: accept 2-7 for `target_id`; text changed at R: exit 1; nothing to change: exit 1 ``PR-0012 not applied: nothing to accept now for R-12 (<why>); spec reject PR-0012 --reason … settles it``, still open (Q6) |
| ask | accept 8 (no identity: exit 2, logged as step 7); `lock R-12 rev 3 (accept: 2 new, 1 refreshed, 0 kept, 0 dropped) for PR-0012 at <commit> on <branch> in <worktree>? [y/N]` |
| 7 | `approve_from` in the consent closure after `y`; lost: `PR-0012 changed meanwhile; nothing written` |
| 8-10 | accept 9-10: `committed` -> `applied_with`, `proposal.applied`; `restored` -> `reopen_from`, logged as step 9 (step 10 never reopens); `unverified`, `left`: `approved`, exit 1. Q-B: made, branch and `HEAD` unmoved (a hook resetting it): accept's row, restored, reopened, not 4b-1's Q16 |

The message accept's, `Proposal: PR-0012` last; the stored `drift`, `drift_hash` never read; no spec file or revision written, so 4b-2's rule parses neither lock (`rev-rule.md` "Rules and edge cases"). `--editorial` refreshes every entry at R (accept's, not 4b-1's vouching filter). **Completion**, in `base_commit..<branch>`, either `Lock-mode`: a drift arm in CLI `preflight.rs` `trailer_commits` before `trailer_commit` (`target_path` `spec.lock` would pass there, `held_at` parsing it as Markdown); `completes_when`'s fourth arm ``a commit on `<branch>` with the trailers `Proposal: PR-0012` and `Lock: R-12`, one parent, changing only `<root_rel>/spec.lock` completes it (`spec approve PR-0012`)``. **Reject** (plain): refused while one exists.

### Exits

`proposal-queue.md` "Exit codes", accept's ("Exits"), plus: **raise** 0 judged; 1 a REF naming nothing, several nodes or a working-tree-only node, a refused `--task`, a dirty `specengine.toml`; 2 usage, the author grammar, a look-alike, `PR` in `[ids]`, no worktree, an unborn or detached `HEAD`, 4a's run causes but (5) at the commit (5b: a format-1 lock), the queue. **Approve** also 1 nothing to accept, `HEAD` off the branch, `written` not `committed`.

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
- [ ] RD-6 -- a second accept -> `drift_approve_writes_the_bytes_lock_accept_writes`; the node's file committed -> `drift_approve_commits_only_the_lock`; `--no-verify` -> `drift_approve_runs_hooks`; `Proposal:` first -> `lock_commit_trailers_parse`; no fifth check -> `dropped_proposal_trailer_fails_step_10`; `written` serialized -> `lock_accept_json_keeps_its_keys`.
- [ ] RD-7 -- another worktree -> `drift_approves_only_where_raised`; a read before the terminal check -> `drift_approve_needs_a_terminal_first`; a dirty marker file -> `dirty_bound_file_refuses_drift_approve`; the cwd's root -> `drift_approve_locks_the_recorded_root`; `branch` unchecked -> `drift_approve_locks_the_recorded_branch`.
- [ ] RD-8 -- code hashes refreshed -> `drift_editorial_keeps_code_hashes`; `editorial_refusal` as `!= Applies` -> `drift_editorial_runs_accept_editorial`; on `spec_changed` -> `editorial_without_entry_at_r_refuses`; vouching filter -> `drift_editorial_refreshes_every_entry_at_r`.
- [ ] RD-9 -- a killed run re-committed, or judged by `trailer_commit` -> `killed_drift_approve_completes_by_trailer`; the lock commit ignored -> `reject_refuses_a_drift_item_with_its_lock_commit`; reopened after a commit -> `drift_approve_reopens_only_restored_bytes`; Q16 taken -> `unmoved_lock_commit_reopens_the_drift_item`.
- [ ] RD-10 -- nothing to accept, yet `applied` -> `drift_with_nothing_to_accept_is_not_applied` (`marker_behind` only); a preview computed -> `drift_review_has_no_preview`. RD-11 -- a stage stored, or refused by the store only -> `drift_item_cannot_be_staged`.
- [ ] RD-12 -- a key dropped -> the five review-key pins, 48 (`proposal_create.rs`, `kind_interpretation.rs` twice, `kind_amendment.rs`, `rev_raise.rs`), and the 46 sets (`kind_amendment.rs`); backup bytes lost -> `drift_rows_round_trip_byte_identical`. RD-13 -- an MCP write under the root -> AC-3's `git status` test; `INSTRUCTIONS` too long -> `server.rs`'s assert.
- [ ] RD-14 -- walk order leaks -> `drift_raise_is_byte_identical_across_roots_and_order`. RD-15 -- a domain literal -> the genre scans. RD-16 -- a new crate -> the CLI pin in eval `build_graph.rs`. RD-17 -- the plugin version kept -> `ac10_the_content_and_version_are_the_last_pin`. RD-18 -- the hint missing -> a hint test in `verify_states.rs`.
- [ ] RD-19 -- seams, the code-reviewer: `verify_judges_a_given_walk`, `judge_is_source_neutral` green; the approve calls `lock_accept`, no second accept; `tests/lock_commit.rs` unchanged but the fifth check's test.
- [ ] RD-20 -- carry-overs, the code-reviewer: CLI `apply.rs` docs (~31-33, ~848-850) read "stay as the commit left them"; "slice 4c" at `lock.rs` ~134 (and `tests/lock_commit.rs` ~209, the test-engineer's) cites canon `proposal-drift.md`; trailer keys from core (`lock.rs` keeps only `Decided-by`); a raw pathspec -> `lock_restore_commands_quote_the_path`.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/`, golden lock `fixtures/spec-lock/code-a.lock`: a scenario per trigger state and category; other states raise nothing; a named REF's `unlocked` rows raise, `key_problem` rows note.
- [ ] TE-2 -- raise -> review (`preview` `null`, the note) -> approve -> `spec verify` `ok`; the lock bytes a twin repository's after `spec lock accept`. TE-3 -- each exit 1 and 2 of raise and approve: `git status` (untracked too), `HEAD`, `spec.lock`, the queue unchanged.
- [ ] TE-4 -- dedup {open, approved, applied, rejected} x {same, other evidence} x {same, other branch}; two concurrent raises, self-terminating. TE-5 -- killed runs (before, after the commit), a failing hook, a hook resetting the commit (Q-B): restored, reopened; a hook dropping `Proposal:`: `approved`.
- [ ] TE-6 -- schema 6, 7 -> 8; a drift row with a stage, a NULL column or a bad hash, another kind with either column: corrupt. TE-7 -- MCP parity with `--json`, `drift` in `get_proposal`'s schema; AC-3; an unknown field refused; `get_proposal`'s `preview` `null`, its note.
- [ ] TE-8 -- the lock commit's hooks: `spec check --staged` passes; `spec verify --changed` (`--staged` after 4b-2) passes under `enforce-introduced`, refuses under `enforce` with another node's Fix row (restored, reopened); `--editorial` after a `--no-verify` text commit -> `ok`.
- [ ] TE-9 -- two roots, another cwd, reverse creation order, CRLF: identical. TE-10 -- `fixtures/spec-c` exits 2 naming `[code]`; the source scans. TE-11 -- HTTP stage 409; `daemon-keys.json` (46 sets, 48 keys).
- [ ] TE-12 -- pilots, manual (`verify_pilot.rs`), scratch clones locked by the test's own accept; items per state, raise time against AC-10; no pilot name committed.

ui-developer:

- [ ] UI-1 -- provisional types and mock gain `kind: "drift"`, the key `drift` (`DriftEvidence`); the key missing -> `ui/src/api/daemonKeys.test.ts`.

spec-writer:

- [x] SW-1 -- this spec, split 2026-10-13, refreshed 2026-10-14. SW-2 -- ADR-0045; `architecture.md#apply`, `#control`. SW-3 -- the drafts naming 4c, 08 s2, 05:188. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: Tier 2 canon `docs/canon/proposal-drift.md`; `spec-lock-accept.md` (`proposal`, `branch`, fifth check, `written`, :46 quoted); `proposal-review.md` (keys, preview, sets), `proposal-completion.md` (the drift arm), `proposal-queue.md` "Store" (schema table), `proposal-revisions.md` (:69 removed), `proposal-apply.md`, `agent-intake.md`, `tasks.md`, `decision-staging.md`, `round.md`, `queue-backup.md`, `mcp-read.md`, `plugin{,-files}.md`; `family()` in `proposal-{kinds,decision,interpretation,amendment}.md`, `decision-record{,-template}.md`; `spec-verify-output.md` "Hints"; `architecture-code.md#spec-lock`, `#verify`; the overridden lines; store, CLI, MCP READMEs; `CLAUDE.md` State.

## Risks

Size L with Q-A (4b-1 took three iterations). **Fallback**: iteration 2 not ACCEPT with minors only -> MCP `report_drift` (tool, `INSTRUCTIONS`, `ask-owner`, plugin version; RD-4's first half, RD-13, RD-17, TE-7) moves to 4c-2. Noise: a partial fix re-raises; a removed worktree's items are only rejected (ADR-0045's Cost); 5b's rebase re-raises rejected items once. A project's `spec verify` hook under `enforce` fails the lock commit over unrelated Fix rows (TE-8).

## Out of scope

A daemon, watcher or CI raising items (06:118); drift screens, a drift preview, UI staging (Phase 4); the Health drift row (9b); several REFs in one item; a hand accept settling items; `@assumes` tasks (06:103); 4c-2's `--follow-up`; 5b `spec lock rebase`; 8b `[[tested]]` (`tested-evidence.md`).

## Open

Owner pre-approved, review open (Q1-Q12 2026-10-10, Q13-Q20 2026-10-13); each answer where its Q is cited. The other answer costs:

- Q1 `verify --queue`: verify writes. Q2 always: a flood; never: hand accepts only. Q3 the disk: unfinished edits queued. Q4 by state: new code missed; none: settled questions repeated. Q5 stored hashes trusted; two accepts. Q6 settling without `[y/N]` (ADR-0038). Q8 a stage key. Q9 spec-side items only rejectable. Q10 against ADR-0041/0042. Q12 agents on Bash. Q13 any file edit re-raises. Q14 a reverted lock hides drift. Q15 a dirty rename judged. Q16 a bad disk file refuses a raise. Q20 an item only ever rejected. Q7, Q19: `drift-follow-up.md`.
- 2026-10-14, decided under the owner's pre-approval, review open: Q-A (overrides Q17, Q18; until Phase 4's drift screen) lock.rs's 11-row commit table refactored, a `ReviewRequest` field in HTTP, a walk per review. Q-B a drift-only branch in accept's restore. Q-C (no editorial-plan preview) two plans per review. Q-D (`create.rs`, `decide.rs` in 4c-2) the glob hazard stays, or two more modules. The fallback ("Risks").

## Dependencies and ordering

- **Needs**: slices 1, 3, 4a; 4b-1 `rev-discipline`, shipped 2026-10-14 (`proposal-revisions.md`).
- **Order**: 1 -> 3 -> 4a -> 4b-1 -> 4c-1 -> 4c-2. **Independent**: 8a; 4b-2; 8b (one accept keeps its blocks; the later of 4c-1 and 8b tests a drift approve). **Not independent of 5b** (`lock-levels.md` Q7): `lock_code` as rendered; a rebase commit settles nothing. **Later**: 5c fills `moved_from`; 9a takes `family()`; 9b re-reads on a drift approve's `proposal.applied`.

## Implementation

Not started. To record: modules, TE-12's figures, the two measured sizes, deviations with reasons. Renamed 2026-10-14: `drift_preview_needs_no_identity` -> `drift_review_has_no_preview`.
