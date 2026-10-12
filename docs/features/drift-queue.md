---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, ui, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4c (WA-1..WA-15; Q1-Q12 decided under the owner's pre-approval, review open); 05 s5.3; 06 s4; 07 s2; 08 s2
adrs: [ADR-0045]
---

# drift-queue: drift reaches the owner's queue; approving it runs the lock accept

## Why

Only whoever runs the read-only `spec verify` sees drift; its Pre-existing findings "go to the backlog" (05:190), which does not exist. 05 s5.3 and 06 s4 give drift three outcomes (accept the code into the lock, send the code back, change the spec); 4a left the kind, the accept, the tasks and the backlog to 4c (`spec-verify.md` "Out of scope"). ADR-0045 amends ADR-0041, ADR-0042 and `spec-lock.md` WA-12, which forbade it (`architecture.md#apply`, `#control`).

**In scope:** the `drift` family (WA-11); `spec propose drift` and MCP `report_drift`: `HEAD` judged, one deduplicated item per drifting node, evidence computed (WA-1..WA-5); `spec approve [--editorial]` running accept's own function, completion, the nothing-to-accept refusal (WA-6, WA-7, WA-9); `spec reject --follow-up code|spec`, a draft task (WA-8); the review key `drift`, inbox, round note, stage refusal, queue schema 8, backup (WA-10, WA-12); `INSTRUCTIONS`, the `ask-owner` line (WA-13); 4a's hint (WA-14); ADR-0045 (WA-15).

## Description and interactions

- **An agent**, the owner or a project's own hook (documented, not shipped) raises items once `spec verify` shows drift, typing no evidence (WA-1); `spec verify` stays read-only (its RD-8). Not `report_discrepancy`: its approval writes a decision record.
- **The owner**, on a terminal where an item was raised: approves (the lock takes the node's current pair), approves `--editorial` (a `spec_ahead spec_changed|same_revision` text called editorial), or rejects, `--follow-up code|spec` making a draft task. A hand `spec lock accept` writes no queue row (`spec-lock.md` RD-15); the next raise notes the item (Q6).
- **Crates** (WA-15): core pure (triggers, `drift_hash`, severity, follow-up texts); store (the kind, schema 8, dedup, `reject_with_task`); CLI over slices 1, 3, 4a (walk, binding, accept's function, the judge); MCP; HTTP no code (`fixtures/daemon-keys.json`); UI types and mock; plugin PATCH; fixtures. No new crate or dependency.
- **Unchanged**: `spec verify` but a hint, `spec check`, the gate, hook, CI, task states, `INDEX_FORMAT`, `spec.lock`'s format, `spec lock accept`, the other kinds' apply. **Writes**: the queue and the index; approve also `spec.lock`, one commit; reject also a task.
- **Overridden live-spec lines**, fixed at shipping (SW-4): 05:183, 05:190, 06:118-123, 07:26, 07:104-106.

## Data

### spec propose drift

WA-1, WA-2, Q1, Q3. `spec propose drift [REF…] [A] [--task T] [--json]` (`A`: `agent-intake.md` "Tools"; `--config` as the queue's).

- **REF**, 0-16: each as accept's step 2 (`spec-lock.md`: a node, or a gone ID with entries in `HEAD`'s lock); the node only, not its sections (SW refinement). None: a **sweep**.
- **Order**: discovery, config; `specengine.toml` tracked and clean (exit 1 naming its state, `commit it first`); `[code]`; the place as `proposal-queue.md` "Creation" 5 (`base_commit` = `HEAD`); REFs; `--task` (`tasks.md` "Task-bound proposals"); 4a's judge over `HEAD` alone (`spec-verify.md` "Base"); the insert.
- Uncommitted edits are never judged: the rows are what `--changed` calls Pre-existing, so the queue is the backlog.

### Triggers

WA-3, Q2. Per node (canonical, or the gone ID): `code_ahead`, `spec_ahead`, `conflict`, `broken_binding` raise an item (05:182-185); `unlocked` only for a named REF, never in a sweep (adoption: `spec-verify.md` "Risks"); `cannot_verify`, `bad_marker`, `predated` never on their own (05:187-188). The item lists all its node's non-`ok` rows; severity `high` when one is class `fix`, else `normal`.

### The item

WA-4. A `proposals` row, kind `drift`: `target_id` the node (or gone ID); `target_path` `spec.lock` (root-relative: what approve writes); `target_ids` `[target_id]`; the place (ADR-0032), `base_commit` `HEAD`; `severity`; `summary` its rows' distinct `state[ category]` by key, `, `-joined (`conflict, code_ahead code_changed`; SW refinement: the inbox line); `author`, `task_id` as the intake's; `diagnostics` `[]`; `drift`, `drift_hash` below; all else NULL.

- `drift`: compact JSON `{"rows":[<row>…],"more":<n>}`, each row a `spec-verify.md` "Output" `rows` item (`pre_existing` `true`), the first 64 (`DRIFT_ROWS_MAX`) by key, `more` the rest (SW refinement).
- `drift_hash` = `b3_hash("specengine-drift/v1" LF <compact JSON>)`, one array per non-`ok` row (all), by key: `[node, relation, file, kind, symbol, item_kind, state, category, detail, marker_revs, lock_rev, lock_spec_hash, lock_code, lock_file_blob, rev, norm_hash, code, file_blob]`; `marker_revs` the markers' `@N` ascending, `null` (unversioned) first; `lock_*` the entry's or `null`, `lock_code` its code fields as `render_lock` writes them, one string (format 2: `levels` through `body_bytes`; 2026-10-12, `lock-levels.md` Q7, in place of `lock_ast_hash`); the last four the node's revision and `norm_hash`, `code` (the same fields as verify computed them at `HEAD`, rendered alike) and the blob OID at `HEAD`, or `null`. Not hashed: lines, times, the entry's `commit` and `verified_at`, the `HEAD` commit, the mode. A moved marker line is the same drift; a further code change is new drift.

### Kind family

WA-11. `ProposalKind` (`ALL` 7) has boolean `applies()`, `decides()`, pinned "one, never both" (`tests/queue_decision.rs`). `Drift` joins (`ALL` 8) with an exhaustive `family() -> KindFamily {Applies, Decides, Locks}` replacing both, so each of about 25 call sites (store `queue.rs`, `stage.rs`; CLI `apply.rs`, `review.rs`, `proposals.rs`, `round.rs`, `preflight.rs`) is judged again (names: SW refinement). `create`, `create_intake`, `approve_record_from` refuse a drift (`Invalid`); `approve_from`, `applied_with`, `reject_from` take it; `create_drift` alone stores one.

### Queue schema 8

WA-12. Step 8, one `Immediate` transaction: `ADD COLUMN drift TEXT`, `drift_hash TEXT`; `QUEUE_SCHEMA_VERSION` 8, `PROPOSAL_COLUMNS` 46, `proposal_columns(6)`, `(7)` the first 44; 9 -> exit 2; older builds refused (`SchemaTooNew`), and they read a `drift` row as corrupt by its kind. **Corrupt**: a drift row with either column NULL, JSON off its shape, a hash not `b3:` + 64 hex, a stage; another kind with either set. **Backup** (`queue-backup.md`): format 2, `queue_schema` 8, 46 columns; import takes schemas 4-8; a byte-identical round trip.

### Dedup

WA-5, Q4. No `distinct_from`: the evidence is computed. Store `create_drift(&[NewDrift], now)`, inside the inserting `BEGIN IMMEDIATE`, keyed (`git_common_dir`, `branch`, `target_id`): an `open` or `approved` drift item is a **hit** (another hash: `evidence changed since it was raised`); an `applied` or `rejected` one with an equal `drift_hash` a hit (settled, the newest named); else stored, IDs in node byte order, a `proposal.created` each. Busy over 5 s: exit 2. Each `open` item of this repository and branch whose node was judged all `ok`: ``note: PR-0007: R-20 reads ok at HEAD now; spec reject PR-0007 --reason … settles it``.

### Output

WA-1. Text, by node bytes (a named REF without a trigger: `note: R-30: no drift at HEAD`):

```
created PR-0012 | R-12 | high | conflict, code_ahead code_changed
queued PR-0009 | R-13 | open | evidence changed since it was raised
settled PR-0004 | R-14 | rejected | same evidence
propose drift: 1 created, 2 hit(s); 14 rows of 5 nodes judged at <40 or 64 hex> on main
```

JSON, the drift document (MCP `structuredContent`), every key, absent `null`: `{commit, branch, judged: {rows, nodes}, created: [{id, node, severity, summary}], hits: [{id, node, status, same}], notes}`.

### report_drift and texts

WA-13, WA-14, Q12. `report_drift {node_ids?: 1-16, task_id?, author_role, author_model?, run?}`, `additionalProperties: false`, no field for rows or hashes; the intake writers' annotations and description (`agent-intake.md` "Tools"); place: the server's root.

- `INSTRUCTIONS` gains `- report_drift = spec propose drift: queues HEAD's lock drift for the owner; evidence computed, never typed.`, trimmed elsewhere to stay ASCII and, with the probe text, within `TEXT_LIMIT` 2 048 B (`server.rs`, `mcp_tasks.rs`).
- `ask-owner` (`plugin.md` "Skills", a PATCH) gains `Drift between spec.lock and the code (spec verify findings): report_drift, no node_ids for every drifting node, a named node also when never locked; it takes no evidence; the owner accepts the lock or sends a task.`
- 4a's hint: `hint: drift: spec propose drift queues it for the owner` (`spec-verify.md` "Output").

### Review, inbox, round, stage

WA-10, WA-12, Q8. **Review** (48 keys): `drift` after `revisions` (SW refinement), `{"hash","rows","more"}` as stored, `null` on other kinds; text `drift: <n> row(s)` and the rows in 4a's form, indented. `preview`: accept's steps 2-7 read-only on today's `HEAD` -> `applies` (note `accept: 2 new, 1 refreshed`) or `unavailable` (`not applicable now (step <n>): <reason>`); `diff`, `conflict`, `base_text`, `new_text`, `revisions` `null`. **Inbox**: `PR-0012 | drift | open | R-12 | main | <created_at> | conflict, code_ahead code_changed`. **Round**: off the sheet (`decides()` kinds only); note ``<n> open drift item(s) not on this sheet: <IDs>; spec review <ID>, then spec approve or spec reject``. **Stage**: `Invalid`, HTTP 409. **UI**: kinds are strings; no screen until Phase 4.

### Approve

WA-6, WA-9, Q5, Q9. `spec approve PR [--note T] [--editorial]`: `--option`, `--answer`, `--canon` exit 2; applied or rejected exit 1; the trailer lookup first. Then apply's steps (`proposal-apply.md`), each calling accept's own function (`spec-lock.md` "spec lock accept" 1-10, "--editorial", "A gone node"), never a second implementation:

| Apply step | Drift approve |
|---|---|
| 1 | `require_terminal` before any read |
| 2 Place | apply's; `HEAD` past `base_commit`: `note: HEAD moved since PR-0012 was raised`, never a refusal |
| 3 | the recorded root's config and slug; accept 2-4 for `target_id` (inputs tracked and clean) |
| 4-5 | accept 5-6: `HEAD`'s blobs hashed, the merge (a text changed at R: exit 1) |
| 6 | accept 7, nothing to change (its exit 0 or 1): exit 1 ``PR-0012 not applied: nothing to accept now for R-12 (<why>); spec reject PR-0012 --reason … settles it``, still open (Q6); the identity |
| ask | stdout accept's step-8 lines; stderr `lock R-12 rev 3 (accept: 2 new, 1 refreshed) for PR-0012 at <commit> on <branch> in <worktree>? [y/N]`; editorial, gone: accept's wording + `for PR-0012` |
| 7 | `approve_from` |
| 8 | accept 9's re-read, the atomic write |
| 9 | `commit_only` of `<root_rel>/spec.lock`, hooks run; a failure restores the bytes (a new lock removed), reopens, exit 1 |
| 10 | one parent, that path only, `Proposal:` this ID -> `applied_with` |

Message: accept's, `Proposal: PR-0012` the first trailer, then `Lock:`, `Lock-mode: accept|editorial`, `Decided-by:` (SW refinement). The stored `drift`, `drift_hash` are never read. No spec file, no revision written; the lock-only commit leaves `rev-not-raised` silent. **Completion**: a `Proposal: PR` commit in `base_commit..<branch>`, one parent, only `<root_rel>/spec.lock`, with `Lock: <target_id>`. **Reject** refuses while one exists (`spec approve PR completes it`).

### Reject and the follow-up task

WA-8, Q7. `spec reject PR --reason T [--follow-up code|spec]`; the flag on another kind, staged or not: exit 2. With it: the node resolved now (gone: exit 1 `--follow-up: R-12 names no node now; reject without it`); ask `reject PR-0012 (R-12 in spec.lock on main in <worktree>) and make a draft task (code)? [y/N]`; one `Immediate` transaction (`reject_with_task`): `reject_from` and a `draft` task as `spec task new` makes one (targets `[target_id]`, author `human`); `proposal.rejected` gains `task` (`null` without), then `task.created`; stdout `rejected PR-0012; created T-0003 (draft): spec task approve T-0003 makes it ready`.

Core renders title and goal from IDs, paths, symbols and states, stack-neutral (07 P2-3):

- code: `Bring the code bound to R-12 in line with its spec (PR-0012)`; goal `R-12 rev 3 (docs/spec/combat.md) is the spec: change the bound code below to match it and raise its markers to @3; spec verify then shows its rows ok, or the owner accepts them.`
- spec: `Propose a spec edit of R-12 matching its bound code (PR-0012)`; goal `The bound code below is right: propose an edit of R-12 rev 3 (docs/spec/combat.md) describing it; once applied, the owner accepts the lock.`
- then `Owner's reason: <reason>` (cut at 1 024 B, `…`), the rows in 4a's text form by key while the goal stays within 4 096 B, then ``<k> more row(s): spec review PR-0012``.

### Exits

WA-7. `proposal-queue.md` "Exit codes", apply's and accept's (`spec-lock.md` "Exits"), plus: **raise** 0 judged (created, hits or nothing); 1 a REF naming nothing or several, a refused `--task`, a dirty `specengine.toml`; 2 usage, the author grammar, a look-alike, `PR` in `[ids]`, no worktree, an unborn or detached `HEAD`, 4a's run causes at `HEAD` (config, `[code]`, a walk gap, its lock invalid or of other recipes, a partial base), HOME, the data directory, the queue. **Approve** 1 nothing to accept; 2 a decision flag. **Reject** `--follow-up` 1 a gone node, a lock commit in history; 2 on another kind.

## Rules and edge cases

- WHEN raised from two roots, another cwd, in reverse creation order or on a CRLF checkout, stdout, JSON and stored rows SHALL be byte-identical; WHEN two raises race, one item per key SHALL be stored.
- WHEN an item is approved, `spec.lock` SHALL hold the bytes `spec lock accept <target_id>` writes on that commit, in a commit of `spec.lock` alone.
- No stack, runner or kind literal outside the kind table in core, store, CLI (`proposal_genre.rs`, the genre scans), none in `specengine-mcp/src`. Nothing blocks (ADR-0012, ADR-0042 as amended): no status, claim, gate, check or other approval reads a drift item.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- "Kind family". M: drift as `applies()` -> `drift_item_never_runs_the_update_apply`; the applies-or-decides pin kept -> `every_kind_has_exactly_one_family`.
- [ ] RD-2 -- triggers, pure core. M: `unlocked` in a sweep -> `sweep_never_queues_a_never_locked_node`; `cannot_verify` alone -> `cannot_verify_alone_raises_nothing`; `std::fs` in core -> `model_and_core_sources_do_no_file_io`.
- [ ] RD-3 -- M: the disk judged -> `uncommitted_edit_is_not_queued`.
- [ ] RD-4 -- M: MCP takes `rows` -> `report_drift_takes_no_evidence`; approve reads the stored column (tampered) -> `approve_never_writes_stored_values`.
- [ ] RD-5 -- dedup. M: none -> `second_raise_stores_nothing`; settled ignored -> `rejected_drift_is_not_raised_again`; lines hashed -> `moved_marker_line_is_the_same_drift`; branch ignored -> `drift_on_two_branches_is_two_items`; a race -> `concurrent_raises_store_one_item`.
- [ ] RD-6 -- one accept. M: a second path -> `drift_approve_writes_the_bytes_lock_accept_writes`; the node's file committed -> `drift_approve_commits_only_the_lock`; `--no-verify` -> `drift_approve_runs_hooks`.
- [ ] RD-7 -- M: another worktree -> `drift_approves_only_where_raised`; a read before the terminal check -> `drift_approve_needs_a_terminal_first`; a dirty marker file -> `dirty_bound_file_refuses_drift_approve`.
- [ ] RD-8 -- M: code hashes refreshed -> `drift_editorial_keeps_code_hashes`; `--editorial` exit 2 -> `drift_editorial_runs_accept_editorial`.
- [ ] RD-9 -- M: a killed run re-committed -> `killed_drift_approve_completes_by_trailer`; the lock commit ignored -> `reject_refuses_a_drift_item_with_its_lock_commit`.
- [ ] RD-10 -- M: nothing to accept, yet `applied` -> `drift_with_nothing_to_accept_is_not_applied`.
- [ ] RD-11 -- follow-up, built last, cut if the three iterations run out (Q11). M: `ready` -> `follow_up_task_is_draft`; two transactions -> `follow_up_and_reject_are_one_transaction`; another kind -> `follow_up_on_a_non_drift_kind_is_a_usage_error`; a gone node -> `follow_up_needs_a_node`.
- [ ] RD-12 -- M: a stage stored -> `drift_item_cannot_be_staged` (CLI; HTTP 409).
- [ ] RD-13 -- M: a key dropped -> `review_document_every_key_present` (48); backup bytes lost -> `drift_rows_round_trip_byte_identical`.
- [ ] RD-14 -- M: an MCP write under the root -> the AC-3 `git status` test; `INSTRUCTIONS` over 2 048 B -> `server.rs`'s assert; the pinned tests updated.
- [ ] RD-15 -- M: walk order leaks -> `drift_raise_is_byte_identical_across_roots_and_order`.
- [ ] RD-16 -- M: a domain literal -> the genre scans; a stack word in the follow-up brief -> the P2-3 scan.
- [ ] RD-17 -- M: a new crate -> the CLI pin in `build_graph.rs`. RD-18 -- M: the plugin version kept -> `plugin_files`. RD-19 -- M: the hint missing -> 4a's verify golden test.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/`, slice 3's golden lock: a scenario per trigger state and category; other states raise nothing; a named REF raises its `unlocked` rows.
- [ ] TE-2 -- raise -> review (`applies`) -> approve -> `spec verify` `ok`; the lock bytes equal a twin repository's after `spec lock accept`.
- [ ] TE-3 -- every exit 1 and 2 of raise and approve: `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, `spec.lock`'s bytes, the queue unchanged.
- [ ] TE-4 -- dedup {open, approved, applied, rejected} x {same, other evidence} x {same, other branch}; two concurrent raises, self-terminating.
- [ ] TE-5 -- killed runs (before, after the commit), a failing hook: bytes restored, the item reopened.
- [ ] TE-6 -- follow-ups of both kinds: a draft task, golden title and goal, the events; `--follow-up` on a staged item of another kind -> exit 2; a drift row with a stage reads corrupt; a gone node refused.
- [ ] TE-7 -- MCP parity with `--json`; AC-3; an unknown field refused. TE-8 -- the lock commit passes a hook running `spec check --staged` under `enforce`; `--editorial` after a `--no-verify` text commit -> `ok`.
- [ ] TE-9 -- two roots, another cwd, reverse creation order, CRLF: identical. TE-10 -- `fixtures/spec-c` exits 2 naming `[code]`; the source scans. TE-11 -- HTTP stage 409; `daemon-keys.json` 48 review keys.
- [ ] TE-12 -- pilots, manual: `#[ignore]`, a release build, foreground, scratch clones, after slice 3's TE-11; items per state, raise time against AC-10; no pilot name committed.

ui-developer:

- [ ] UI-1 -- the provisional types and mock gain `kind: "drift"` and the key `drift`. M: the key missing -> `ui/src/api/daemonKeys.test.ts`.

spec-writer:

- [x] SW-1 -- this spec. SW-2 -- ADR-0045; `architecture.md#apply`, `#control` (12 279 of 12 288 B: `#apply`'s generated-document sentence and `#storage`'s dump name reworded, no rule dropped). SW-3 -- `spec-lock.md`, `spec-verify.md`, `rev-discipline.md`, `verify-tests.md`, 08 s2. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: a Tier 2 canon `docs/canon/proposal-drift.md`; `proposal-queue.md`, `agent-intake.md`, `tasks.md`, `decision-staging.md`, `round.md`, `queue-backup.md`, `mcp-read.md`, `plugin.md`; the overridden lines; the store, CLI, MCP READMEs; `CLAUDE.md` State.

## Risks

Canon contradictions (ADR-0041, ADR-0042, `#control`, `spec-lock.md` WA-12, `rev-discipline.md`'s exit 2) are resolved here, before code. The approve door gains a lock write (its commit `spec.lock` alone: AC-3, 08:71); a second accept would drift from the first (ADR-0041's Cost); about 25 call sites could misroute a drift item (RD-1). Noise: a partial fix re-raises; a hand accept leaves items open; a removed worktree's items are only rejected; a huge node is cut at 64 rows. The raise and `preview` parse the whole tree at `HEAD` (AC-10). A project hook running `spec verify --staged` under `enforce` can fail the lock commit over unrelated Fix rows. Schema 8 refuses older binaries.

## Out of scope

A daemon, watcher or CI raising items (06:118); drift screens, UI staging, a `follow_up` stage key (Phase 4); the Health drift row (slice 9); several REFs in one item; a hand accept settling items; `@assumes` tasks (06:103); follow-ups for other kinds; 5b `spec lock rebase`; 8b `[[tested]]` (`tested-evidence.md`: approve shares accept's function, so a block is kept, a gone node's dropped, an editorial approve refreshes its `spec_hash`; whichever of 4c and 8b ships second tests approve).

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 who raises | `spec propose drift`, `report_drift`; `spec verify` read-only | `verify --queue` breaks 4a's RD-8; CLI only stores agents as `human` |
| Q2 which rows | "Triggers"; `unlocked` only when named | always: an adoption flood; never: hand accepts only |
| Q3 which state | `HEAD`; a dirty config exit 1 | the disk queues unfinished edits |
| Q4 dedup | "Dedup"; a rerun exits 0 naming hits | by state misses new code; none repeats settled questions (05:223) |
| Q5 approve | "Approve": accept's function, a lock-only commit; nothing stored written | stored hashes become "verified"; two accepts drift |
| Q6 nothing to accept | exit 1 naming `spec reject`; a hand accept writes no row | auto-settling without a `[y/N]` (ADR-0038) |
| Q7 outcomes | `reject --follow-up code\|spec`, one transaction, a draft task | approve without a commit breaks "`applied` = a commit" |
| Q8 stage, round, UI | stages refused; a round note; UI types only | a `follow_up` stage key, another schema step |
| Q9 4b | no raise; `--editorial` runs accept's | spec-side items only rejectable |
| Q10, Q11, Q12 | ADR-0045; no split, RD-11 last; MCP now | code against ADR-0041/0042; tasks by hand; agents on Bash |

WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs**: slice 1 `code-refs` (walk, binding; its code uncommitted); 3 `spec-lock` (accept as one function taking a consent closure and extra trailers; the gone-node drop); 4a `spec-verify` (the judge, "Base"); 4b `rev-discipline` (schema 7, `approve --editorial`, 47 review keys). All drafts, no code.
- **Order**: 1 -> 3 -> 4a -> 4b -> 4c; this spec and ADR-0045 now, code after 4b. **Independent**: 8a; 8b (the later of 4c and 8b tests a drift approve). **Not independent of 5b** (2026-10-12, `lock-levels.md` Q7): `drift_hash` takes the entry's code fields as rendered, and a rebase commit settles no item (completion needs `Proposal:` and `Lock:`). **Later**: Phase 4 screens; slice 9.

## Implementation

Not started. To record: modules, TE-12's figures, deviations with reasons.
