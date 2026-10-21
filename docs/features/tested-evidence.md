---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, plugin, fixtures]
ref: Phase 3 slice 8b-1 (RM-0008); Q1-Q8 (2026-10-12), Q-A..Q-I and iteration 1's two developer questions (2026-10-21) decided under the owner's pre-approval, review open; 05 s5.4; 08 AC-8; 2 iterations
shipped: 2026-10-21
adrs: [ADR-0044]
---

# tested-evidence (8b-1): passing tests leave self-staling evidence in `spec.lock`

## Why

After 8a a pass was only a `verify.tests` event: nothing in git said "R-12's bound tests passed at commit X for this text and this code" or noticed a later edit; agents had only their own `report_run` claims. 08 AC-8 was "run half met".

Delivered: one `[[tested]]` block per node in `spec.lock` (format 3), pinned by `rev`, `spec_hash` and a `bound_hash` over the node's bindings, written by 8a's run in its own commit under its one `[y/N]`; one pure function judging it in plain `spec verify` and `check_binding`, never stored, never a verdict; every lock reader and writer taking blocks; plugin 0.17.0. AC-8 is met; the reads are 8b-2 (`tested-reads.md`, RM-0032).

## Description and interactions

- **Canon**: `docs/canon/spec-lock-tested.md` and the files it points to.
- **The owner** answers 8a's one question, now naming the evidence commit; **an agent** reads `tested`, never writes evidence (ADR-0044); `report_run` changes nothing. Untouched: store, model, HTTP, UI, the gate, `.githooks/`, CI; no new dependency, no BLAKE3 in core. test-engineer: the tests, the golden, `verify_tests_pilot.rs:11`'s comment.
- **Decided**: Q1-Q8 (2026-10-12), Q-A..Q-I (2026-10-21; Q-A the split, Q-E blocks counted as entries, Q-F a new canon file, Q-G the tester line); no ADR (ADR-0041 to ADR-0045 and ADR-0050 decide it).

## Data

### Format 3 and the block

Format 3 only with a block, else format 2 byte for byte: `spec-lock-tested.md` "The block"; the **Reader**'s texts: `spec-lock.md` "Entry".

### Bound hash

One function for write, read and rebase; on a read one pass over the judged nodes, each file once; the bound bytes and hash only when no earlier state applies, levels for any node reaching lockability (`cannot_verify` wins over `stale`): `spec-lock-tested.md` "Bound hash".

### The state

`spec-lock-tested.md` "The state".

### Plain verify

`spec-lock-tested.md` "Plain verify"; **Mirror**: `mcp-code.md` "Mirrors". The tester role's claim paragraph ends: "The machine verdict is the owner's own test run through SpecEngine, never your report: its evidence shows as `tested` in `check_binding`, and an edit to a node or its bound code turns it stale."

### The run's write

`spec-verify-tests.md` "Order" (step 6, `text_changed`, the question), `spec-lock-tested.md` "The run's write".

### The commit

`spec-lock-tested.md` "The commit".

### Event

`verify.tests`' `lock_commit`: the verified evidence commit's OID, else `null`.

### Other lock writers

`spec-lock-tested.md` "Other writers"; regen's `--json` `tested` `[{choice, node}]`: `spec-lock.md`.

### Rebase

`spec-lock-rebase.md` "Blocks".

### Fixture

`fixtures/verify-tests/` (8a-1's), 8a's fake runners; golden `fixtures/spec-lock/verify-tests.tested.lock`: `format = 3`, no binding, blocks `CONE-01`, `KILN-HOLD`, `KILN-RAMP`, `glaze-check/GLZ-01`, none for `KILN-COOL`.

## Rules and edge cases

- WHEN a writer touches the lock, a block SHALL change only as the canon says: only 8a's completed run adds one.
- WHEN no block exists, every lock byte and verify's stdout SHALL equal 5b's.
- WHEN the blocks cannot be judged (a git `hash-object` failure, the walk, the grammar), verify's exit, verdict, counts and rows and `check_binding`'s `isError` SHALL NOT change: `tested` is `null`, never `[]`, with one note `tested: <cause>`.
- WHEN a block reads an earlier state, its bound bytes and hash SHALL NOT be computed; WHEN the lock holds no block, `--task` SHALL NOT build its set for `tested`; rebase SHALL hash through `tested::bound_hash`.
- No status, claim, approval, gate, check or exit reads `tested` (ADR-0044, ADR-0012); no source names a runner (ADR-0008).

## Acceptance criteria

M1-M8: the counted mutations, 8 of 8 killed; plus M9, a judging failure pushed as a run cause again. CLI `tests/` unless named.

- [x] RD-1 -- core `lock_model.rs`. M1: a byte changed without a block -> `evidence_free_lock_bytes_unchanged`. `tested_needs_format_3`, `tested_block_bytes_match_the_golden`.
- [x] RD-2 -- `lock_format.rs` `every_lock_reader_reads_format_3`, a row per reader. M2: format 3 refused under `Some`.
- [x] RD-3 -- `tested_hash.rs`. M3: `file_blob` hashed -> `comment_elsewhere_keeps_verified`. `sha256_repository_same_bound_hash`; a `stale spec_changed` block hashes nothing; a file two nodes share is parsed once. Cost (`tested_read_cost_record`, `#[ignore]`): plain verify with 4 blocks, median 36.7 ms against 26.6 ms without (x1.38).
- [x] RD-4 -- core `tested_state.rs`; `verify_output.rs`, `verify_node.rs`, `verify_task.rs`. M4: a block changing exit, verdict, counts, class or hints -> `tested_never_changes_verify_exit`. `base_modes_tested_is_null`, `evidence_free_verify_text_unchanged`; M9 -> `tested_hash_failure_never_changes_verify_exit` (plain, `--node`, MCP `check_binding`): `null`, one note, exit and `isError` unchanged.
- [x] RD-5 -- `verify_tests_lock.rs`, `passing_task_commits_its_evidence`. M5: a failed node keeps its block -> `failing_test_removes_evidence`. `text_changed_at_r_is_not_verifiable`, `unreadable_lock_is_cannot_before_the_question`, `refused_evidence_commit_exits_1`.
- [x] RD-6 -- `lock_commit.rs`, `verify_tests_genre.rs`, `drift_approve.rs`. M6: a second question -> `one_question_covers_the_write`. M7: `tests` in `TRAILERS` -> `drift_completion_ignores_a_test_lock_commit`. `only_terminal_run_writes_tested`, `event_names_the_lock_commit`.
- [x] RD-7 -- `lock_accept.rs`, `lock_deleted.rs`, `rev_*.rs`, `lock_regen.rs`, `lock_rebase.rs`, `drift_raise.rs`. M8: a block re-derived without the proof -> `rebase_drops_unproven_tested`. `tested_block_vouches_like_an_entry`, `regen_merges_format_2_and_3_stages`, `lock_accept_json_keeps_its_keys` (nine); regen `--json`'s `tested` equals its text rows.
- [x] RD-8 (uncounted) -- MCP `mcp_code.rs` types `tested`; `plugin_files.rs` 0.17.0, `PINS` `b4d6b1ce…ab48`, `SCHEMA_PINS` `f7ad1114…37a7` (both checked through `as_of_0_16_0`); `plugin_roles.rs` the tester line.
- [x] TE-1 -- `tested_e2e.rs`, one history through every state, back to `verified` after comments and rustfmt, the block removed by a failing run; `report_run` alone writes none.
- [x] TE-2 -- two roots, another cwd, reverse creation order, `core.autocrlf=true`, a SHA-256 repository: identical lock, stdout and JSON except `commit` (a SHA-256 OID), with `now` fixed. M: a path or time of day in a block.
- [x] TE-3 -- a hook refusing the commit (restored, exit 1, the event written), `HEAD` moving, an ignored first lock: status, `HEAD`, refs, index unchanged.
- [x] TE-4 -- `tested_writers.rs`: every writer's rule over one repository, a grammar-bump rebase giving `rebased`, `entry_dropped`, `unproven`.
- [x] TE-5 -- over a format-3 lock each 7a/7b read prints what it prints without the blocks; `check_binding`'s `tested` equals `spec verify --node --json`'s.
- [x] TE-6 -- `check_genre.rs`, `verify_tests_genre.rs`, `mcp_door.rs`, `daemon_keys.rs` unchanged.
- [ ] TE-7 -- the owner's: `tested_pilot.rs` (`#[ignore]`), a scratch clone of A: the write `<pending>`, plain verify `<pending>`; B: no bound test.
- [x] SW-1..SW-5 -- new `spec-lock-tested.md` 9 785 B; `architecture-code.md` 12 287 (+1), `node-revision.md` 12 252 (-34), `spec-verify.md` untouched, READMEs core 10 232, CLI 10 234; the other canon within its room; 05-08 (AC-8 met), `CLAUDE.md`, RM-0008; `export index && check` clean.

## Out of scope

The reads (8b-2), Health (9b), the UI; evidence levels, `impl_status`, `needs:`; drift items for stale evidence; batching, a dry run, `--yes`; MCP, HTTP, CI or hook triggers; running module or RON bindings; per-binding blobs; a provenance audit.

## Implementation

| Module | What it does |
|---|---|
| core `lock.rs` | `Tested`, `TestedTest`, `TESTED_KEYS`; `LOCK_FORMAT` 3, `BINDING_FORMAT` 2, `Lock::{holds, format_number}`, `LockFormat::entries_number`; `bound_bytes` over `Bound` (`BOUND_TAG`); `tested_state` (`TestedNode`, `TestedState`); `merge_tested` (`TestedRun`, `TestedMerge`); the writers' rules: `AcceptRefusal::TestedChanged`, `Editorial.tested`, `drop_node`, `rebase_tested` (`TestedDrop`), `Regen.tested` |
| core `check/rev_rule.rs` | a block vouches; `lock_entries` counts it |
| CLI `tested.rs` (new) | `bound_hash`, `fresh_bound_hash`, `first_refusal`, `tested_entries` (`TestedEntry`) |
| CLI `lock.rs` | `LockSubject::Tests` and its check 3, `LockMode::Tests`; `node_bindings` over `Source::{Head, Disk}` (`committed_bindings` wraps it); accept, `--editorial`, gone block rows; `LockOutcome.tested` |
| CLI `verify_tests.rs` | step 6's lock read, `text_changed`, the question's tail, `write_evidence`, `VerifyTestsLock`, exit 1, `lock_commit` |
| CLI `verify.rs`, `verify_node.rs` | `VerifyOutcome.tested`, JSON key 11, the text lines |
| CLI `lock_rebase.rs`, `lock_regen.rs`, `revisions.rs`, `bump.rs`, `bindings.rs` | blocks rebased or dropped (`RebaseCounts.tested_*`), merged, refreshed, counted; `cut_lock` drops them for the 7a/7b reads |
| MCP `mirror.rs`; `plugin/` | `VerifyDocument.tested`; 0.17.0, the tester sentence |

Seams: `tested_state` takes lockability and the bound hash as closures, so the CLI computes them lazily; `rebase_tested` takes the CLI's hash function; `write_and_commit`'s consent closure returns the run's `y`.

Readings taken (accepted): several holders, the first by path and source order; a `tests` problem at the value's line; the format/block mismatch at reader check (5); all bindings gone reads `stale code_changed`. Texts chosen: the rebase tail inside the parenthesis; accept's `kept | tested` counted in `kept`; the evidence commit's own question never shown, the fallback `the commit of spec.lock was not verified` unreachable; all-`unchanged` rows print alone.

Divergences from the draft (owner, iteration 1's review): D1 regen `--json` gains `tested`; D2 a hashing failure is `tested: null` and a note, never a run cause. Iteration 2 also: one pass over the judged nodes, bound bytes and hash lazy ("each file once" kept); `--task`'s set only with a block; rebase through `tested::bound_hash`; `LockFormat::number` renamed `entries_number`; `Source::Disk`'s skip a `debug_assert`. Q-I's fallback was not needed.

Known limits: a grammar that fails to load fails every judged node's evidence (`null`, the note), even a node whose bindings would reuse entries; markers are judged once per holder.

Tests: nextest workspace 3 820 run, 59 skipped, 3 818 passed before the commit; the other 2, eval `import_cli` and `layout_cli`, refuse the uncommitted `fixtures/verify-tests/expected.json` and `fixtures/spec-lock/verify-tests.tested.lock`, passing once committed. fmt, clippy, parity, the eval policy tests (86 of 86) clean.
