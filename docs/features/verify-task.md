---
class: spec
status: draft
scope: [crates/specengine-cli]
ref: Phase 3 plan 2026-10-10, slice 8a-2, split from verify-tests 2026-10-16 (its P1, P2, P6; decided 2026-10-16 under the owner's pre-approval, review open); 06 s6; 07 s2; 08 s2
adrs: []
---

# verify-task (8a-2): `spec verify --task T`

## Why

A task's agent before `report_run`, or the owner at review, wants verify's findings for the task's nodes. Plain `spec verify` judges every lock key against the whole tree (`docs/canon/spec-verify.md`): the answer holds every other node's drift, and under `enforce*` a task's check blocks on Fix rows the task never touched.

Delivered: `spec verify --task T [--changed] [--json]`, plain verify's rows kept to the task's node set, read-only, no terminal. Split from `verify-tests.md` (8a-1) on 2026-10-16 (owner): its read-only half is what 5b-1 churns (`levels` in every row, new lockability refusals), so it ships after 5b-1 with `levels` from the start. No ADR: ADR-0042 (judge, exits; `architecture-code.md#verify`), ADR-0044 (names `--task`), ADR-0012/0039 (no gate reads a finding), ADR-0008 applied.

## Description and interactions

- **An agent** runs `spec verify --task T` before `report_run`; a Fix or cannot row fixed or raised with a working answer. **The owner** runs it at a task's review, with `--changed` to set aside what `HEAD` already has.
- **Crates**: CLI `verify.rs` only: `--task` in `main.rs` and `VerifyRequest`; one filter between `judge_walk` and `verdict` in `run` (:551-564), over 8a-1's "The node set" (`spec-verify-tests.md`); the summary prefix and the JSON key in `render_text` (:964) and the document. `verify_rows.rs`, core (`judge_row`, `verdict`), store, MCP, HTTP, UI unchanged.
- **Unchanged**: plain, `--changed`, `--staged` verify's rows and text (their JSON gains `task` `null`); `spec check`, the hook, the gate, CI, `INDEX_FORMAT`, the queue schema. **Writes**: the data directory only (the index refresh, as plain); never `spec.lock`, a queue row, an event, the task (no `revision` raise) or a file under the root.
- **Overridden at shipping** (SW-2): `spec-verify.md`'s opening command line; `spec-verify-output.md` "Text" (the summary prefix), "JSON" (`task`); 8a-1's `spec-verify-tests.md` (its `--task`-alone usage line); 07:105.

## Data

### spec verify --task

P1, P2, P6. `spec verify --task T [--changed] [--json]` (+ `--root`, `--config`); with `--staged` or `--node`: usage, exit 2 (whichever of this and 6b-2 ships second adds its pair); with `--tests`: 8a-1's run. Order: config, `[verify]`, the task, then plain verify's run (walk, base, lock, judge), the filter, the verdict.

- **The task**: 8a-1's "A task's set" (columns now, any state, contained sections added; a ref naming nothing: a `note:`); `T` unknown, another repository's, in `[ids]` or a look-alike: exit 2, stdout empty, `tasks.md` "Commands"' text (S13 of 8a-1: exit 1 is Fix only, ADR-0042).
- **Rows**: plain verify's rows (`spec-verify.md` "Rows", "States"; 5b-1's `levels`; 5c's `anchor` once shipped) kept by 8a-1's "A row in a set", both halves: a marker `spec refs` lists under a set node, or a lock entry naming one. **Pre-existing** with `--changed`: plain `--changed`'s, judged over the whole tree before the filter.
- **Counts**: `counts`, `cascade`, `delta`, the hints, the verdict and the exit over the kept rows; `skipped` 0. A run cause is the whole run's, never filtered: `cannot:` lines, `rows` `[]`, exit 2, as plain. Plain `enforce-introduced` reads as `enforce` with plain's note.
- **Text**: plain's row lines, `cannot:` lines, hints; the summary `verify --task <T>: ` + plain's text after `verify: `.
- **JSON**: plain's document (`spec-verify-output.md` "JSON"), `task` (the ID) after `base`, `null` without `--task`; 6b-2's `node` and `symbol` follow `task` (`verify-node.md` "spec verify --node").

```
check | code_ahead code_changed body | LAW-BOUND | implements | crates/tally/src/counter.rs | item crates/tally::counter::impl Tally #1 impl_item
verify --task T-0007: observed [observe]: 0 fix, 1 check, 0 cannot, 1 ok; 1 by blob, 1 by ast; 0 skipped
```

```json
{"mode":"observe","base":null,"task":"T-0007","verdict":"observed",
 "counts":{"rows":2,"ok":1,"fix":0,"check":1,"cannot":0,"pre_existing":0,"skipped":0},"cascade":{"by_blob":1,"by_ast":1},
 "rows":[…],"delta":{…},"cannot":[],"notes":[]}
```

### Why a base here, not under `--node`

P2. A scope that filters the judged rows (`--task`) can take a base: the whole tree is judged anyway. One that narrows the files judged (`--node`, `verify-node.md` "spec verify --node", **Files**) cannot: a base reads all of `HEAD`'s code, against MCP's 1 s. Plain-only `--task` would block a task's check on pre-existing Fix rows under `enforce-introduced`.

## Rules and edge cases

- WHEN `--task T` and plain `spec verify --json` run on one state, `--task`'s rows SHALL equal plain's filtered by the set, every key and value included (no golden: 5b-1, 5c and 8b change rows).
- WHEN a run cause stops the run, the report SHALL be plain's: causes are never filtered by the set.
- WHEN run from two roots, another cwd or in reverse creation order, stdout and JSON SHALL be byte-identical.
- No state, claim, approval, task transition or gate reads a result (ADR-0012, ADR-0039); the task is read, never changed.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- forms (P1). M: `--task` with `--staged` accepted -> `verify_task_with_staged_is_usage`; a terminal required -> `verify_task_needs_no_terminal` (stdin a pipe, exit 0), replacing 8a-1's `verify_task_without_tests_is_usage`.
- [ ] RD-2 -- rows (P2). M: a row outside the set kept, or a set row dropped -> `verify_task_rows_equal_plain_rows_filtered` (differential, every scenario of 4a's suite); a homoglyph marker or a lock-only row dropped -> 8a-1's `node_set_rows_follow_refs_listing`.
- [ ] RD-3 -- counts, text. M: counts or `delta` over all rows, `skipped` kept -> `verify_task_counts_its_rows_only`; plain's prefix -> `verify_task_summary_names_the_task`; a run cause filtered away -> `verify_task_keeps_run_causes`.
- [ ] RD-4 -- exits (P6). M: an unknown task at exit 1 -> `verify_task_unknown_is_exit_2`; another repository's task read -> `verify_task_of_another_repository_is_exit_2`.
- [ ] RD-5 -- base. M: `--changed` ignored under `--task` -> `verify_task_changed_marks_pre_existing` (`enforce-introduced`: a pre-existing Fix row blocks plain `--task`, not `--task --changed`).
- [ ] RD-6 -- keys (P6). M: `task` absent or not after `base` -> `verify_json_every_key_present` (CLI `verify_output.rs`), re-pinned.
- [ ] RD-7 -- read-only. M: an event, a `runs` row or a `revision` raise -> `verify_task_changes_no_task`.

test-engineer:

- [ ] TE-1 -- over 4a's TE-1 scenarios on 5b-1's format-2 golden: per scenario, a task over each node, one with a section target, a `draft` task: `--task`'s rows equal plain's filtered; determinism (two roots, reverse creation).
- [ ] TE-2 -- `--changed` under `enforce-introduced` with a pre-existing Fix row of a task node: plain `--task` exit 1, `--task --changed` exit 0.

spec-writer:

- [x] SW-1 -- this spec, split from `verify-tests.md` 2026-10-16.
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 at `f9a80ff`: `spec-verify.md` (11 710: the opening's command line; whatever 5b-2 leaves); `spec-verify-output.md` (5 380: "Text" the prefix, "JSON" `task`, about 250 B); `spec-verify-tests.md` (8a-1's: the usage line); the CLI README (9 578: the `--task` line); 07:105; 08 s2; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Risks

- **Churn**: 5b-1 and 5c rewrite rows; the differential test, not a golden, keeps this slice out of their churn.
- **Wide sets**: a task naming a document takes every section it contains; its rows are most of plain's.

## Out of scope

`--tests` (8a-1); `--task --staged` (a hook knows no task; a second resolver over the index's corpus); an MCP tool or HTTP route for it; 8b's `tested` key (`tested-evidence.md` "Plain verify": `--task` scopes it as rows, at 8b's shipping).

## Open

| Point | Answer (else: the other answer's cost) |
|---|---|
| Split (owner) | this after 5b-1, with or before 6b-2 and before 8b (else: an L 8a, its rows re-pinned at 5b-1) |
| P1 forms | plain and `--changed`; `--staged`, `--node`: usage (else: a second resolver over the index) |
| P2 base | `--changed` allowed here, not under `--node` ("Why a base here") |
| P6 keys | `task` after `base`; 6b-2's after `task` (else: 6b-2 re-pinned) |

Decided 2026-10-16 under the owner's pre-approval, review open; P-n as numbered in `verify-tests.md` "Open".

## Dependencies and ordering

- **Needs**: 8a-1 `verify-tests` ("The node set", the `--task` flag's parsing); 5b-1b `lock-levels-verify` (`levels` in rows).
- **Order** (owner, 2026-10-16): 8a-1 -> 5b-1b -> 8a-2 (this), with or before 6b-2 (its JSON keys follow `task`), before 8b (`--task` scopes `tested`). Independent of 5b-2, 5c.

## Implementation

Not started. From the split (2026-10-16): 8a's "spec verify --task" section and RD-8's `verify_task_scopes_rows` (now RD-2's differential). To record: modules, deviations with reasons.
