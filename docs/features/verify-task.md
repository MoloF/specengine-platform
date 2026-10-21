---
class: spec
status: shipped
scope: [crates/specengine-cli]
ref: Phase 3 plan 2026-10-10, slice 8a-2, split from verify-tests 2026-10-16 (its P1, P2, P6); refreshed 2026-10-20 against 5c as built, Q1-Q5 decided under the owner's pre-approval, review open; 06 s6; 07 s2; 08 s2; developer 1 iteration, review 1 accepted with nits
shipped: 2026-10-20
adrs: []
---

# verify-task (8a-2): `spec verify --task T`

## Why

A task's agent before `report_run`, or the owner at review, wants verify's findings for the task's nodes. Plain `spec verify` judges every lock key against the whole tree (`docs/canon/spec-verify.md`): the answer holds every other node's drift, and under `enforce*` a task's check blocks on Fix rows the task never touched.

Delivered: `spec verify --task T [--changed] [--json]`, plain verify's rows kept to 8a-1's task node set, read-only, no terminal; how it works now: `docs/canon/spec-verify-scope.md`. No ADR: ADR-0042 (the judge, the exits; `architecture-code.md#verify`), `architecture.md#control` (verify stops nothing unless the project's own mode says so), ADR-0012/0039 (no gate reads a finding), ADR-0008 (task records and node IDs only), 8a-1's canon (`spec-verify-tests.md` "The node set"). ADR-0044 names only `--tests [--task T]`; it does not cover this form.

## Data

### spec verify --task

The form, the order, the set, the filter, Q1-Q5's notes, the exits and the known limits: `docs/canon/spec-verify-scope.md` "`--task`"; the text and JSON: `spec-verify-output.md` "Text", "JSON".

- **Text**: plain's row lines, `cannot:` lines, hints and notes; the summary `verify --task <T>: ` + plain's text after `verify: `.
- **JSON**: plain's document, `task` (the task's ID) after `base`, `null` without `--task`: 10 top keys (`TOP_KEYS`); `notes` plain's plus the set's; 6b-2's `node` and `symbol` follow `task`.

```
verify --task T-0007: observed [observe]: 0 fix, 1 check, 0 cannot, 1 ok; 1 by blob, 1 by ast; 0 skipped
```

### Why a base here, not under `--node`

P2. A scope that filters the judged rows (`--task`) can take a base: the whole tree is judged anyway. One that narrows the files judged (`--node`, `verify-node.md` "spec verify --node", **Files**) cannot: a base reads all of `HEAD`'s code, against MCP's 1 s. Plain-only `--task` would block a task's check on pre-existing Fix rows under `enforce-introduced`.

## Acceptance criteria

rust-developer (Mn = the mutation that must turn the named test red; all in the new CLI `tests/verify_task.rs`):

- [x] RD-1 -- forms (P1). M1: `--task --staged` accepted -> `verify_task_with_staged_is_usage`: exit 2, stdout empty, stderr naming `--task` and `--staged`, `--json` too; plain `--task` and `--task --changed`, stdin a pipe: no prompt, exit 0. Replaces 8a-1's `verify_task_without_tests_is_usage` (`verify_tests_forms.rs`, deleted; its `--staged` cells move here).
- [x] RD-2 -- rows (P2). M2: a row outside the set kept, or a set row dropped -> `verify_task_rows_equal_plain_rows_filtered` (TE-1's differential: each kept row object equals plain's, in order; TE-4's read-only asserts inside).
- [x] RD-3 -- both halves, Q2. M3: the entry half dropped, or the marker half matched by exact ID -> `verify_task_keeps_lock_only_and_look_alike_rows`: `symbol_gone` and `marker_gone` lock-only rows, a homoglyph marker, an alias marker, Q2's note.
- [x] RD-4 -- counts, text, Q3. M4: the filter after the verdict, or `skipped` kept -> `verify_task_counts_its_rows_only`: under `enforce`, a Fix row outside the set: plain exit 1, `--task` exit 0; `delta.affected_nodes`; the `verify --task <T>: ` prefix; a run cause still exit 2 with plain's `cannot:` lines; Q3's note.
- [x] RD-5 -- base (P2). M5: `--changed` ignored under `--task` -> `verify_task_changed_marks_pre_existing` (TE-2).
- [x] RD-6 -- the task (Q1). M6: an unknown or another repository's T at exit 1, or run as plain -> `verify_task_unknown_is_exit_2`: unknown, a look-alike, not an ID, another repository's, `T` in `[ids]`, Q1's `--config`; stdout empty, `--json` too.
- Pin, no mutation counted: `TOP_KEYS` 10 (`task` after `base`) through `verify_json_every_key_present` (CLI `verify_output.rs`).

test-engineer (library calls; the binary where clap, stdin or stdout is asserted; at most 4 tasks per scenario; no new fixture, no `expected.json` change: the differential is the oracle):

- [x] TE-1 -- the differential over (a) `verify_common::golden` with `verify_output.rs`'s `drift()` edits; (b) `fixtures/anchor-states`, its baseline and 18 scenarios (merged, ambiguous, lost rows); (c) `fixtures/verify-tests`: the document `KILN-FIRING`, the alias `PYRO-01`, `glaze-check/GLZ-01` via `plan`, a `draft` task, a ref renamed away; (d) `verify_common::ecs`.
- [x] TE-2 -- `enforce-introduced`, a pre-existing Fix row on a task node: `--task` exit 1, with the note; `--task --changed` exit 0, `pre_existing` true, `base` `HEAD`.
- [x] TE-3 -- determinism: two roots (one `HOME` each), a subdirectory cwd, the files written in reverse order: byte-identical stdout and JSON.
- [x] TE-4 -- read-only: `git status --porcelain=v1 -z --untracked-files=all` and the `spec.lock` bytes unchanged; the task's `revision`, `updated_at`, its `runs` and `events` unchanged.

spec-writer:

- [x] SW-1 -- this spec (split 2026-10-16; refreshed 2026-10-20 against 5c as built).
- [x] SW-2 -- at shipping (2026-10-20), bytes before -> after (budget; caps: Tier 2 12 288, Tier 1 10 240, Tier 0 16 384): new `docs/canon/spec-verify-scope.md` 2 493 (2 500); `spec-verify.md` 12 099 -> 12 156 (12 180: the opening's form, a pointer); `spec-verify-output.md` 10 474 -> 10 610 (10 700: "Text" the prefix, "JSON" `task`); `spec-verify-tests.md` 9 853 -> 9 795 ("The node set", "Order" step 1); CLI README 9 979 -> 10 038 (10 040); `architecture-code.md` 12 072 -> 12 128 (12 132: a Cannot row outside a scope not reported); `CLAUDE.md` 6 039 -> 6 054; 06 s6; 07 s2; 08 s2 (the slice, the owner's order of 2026-10-20, Phase 6's heading), :44; `verify-node.md` (the `--node --task` pair, `TOP_KEYS` 10 -> 13, its canon's option, ordering); `tested-evidence.md`'s pointer; this spec compacted.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Decided

| Point | Answer (else: the other answer's cost) |
|---|---|
| P1 forms | plain and `--changed`; `--staged`, `--node`: usage (else: a second resolver over the index) |
| P2 base | `--changed` allowed here, not under `--node` ("Why a base here") |
| P6 keys | `task` after `base`; 6b-2's after `task` (else: 6b-2 re-pinned) |
| Q1 before T resolves | exit 2, stdout empty, stderr; `--config` only the root's own (else: a cause place `spec-verify.md` has no bytes for, the JSON carrying an unresolved `task`, one flag failing in two shapes) |
| Q2 rows naming no node | not kept, one note (else: every task gets the tree's orphans; silent: an agent's typo reads `clean`) |
| Q3 set nodes without a row | one note, as 8a-1's (else: `clean` on code with no markers reads as success) |
| Q4 canon home | new Tier 2 `spec-verify-scope.md`, `--node` joins; `spec-verify.md` a pointer |
| Q5 the task's place | the caller's tree, a known limit (else: a note naming the worktree breaks "no absolute path"; a refusal stops review elsewhere) |

P1, P2, P6 decided 2026-10-16 (`verify-tests.md` "Open"); Q1-Q5 2026-10-20, the analyst's recommendations; both under the owner's pre-approval, review open.

## Out of scope

`--tests` (8a-1); `--task --staged` (a hook knows no task; a second resolver over the index's corpus); an MCP tool or HTTP route; 8b's `tested` key (`tested-evidence.md` "Plain verify": `--task` scopes it as rows, at 8b's shipping).

## Later

Closed by `hygiene-pass.md` (RM-0029, shipped 2026-10-21). Done: one `canonical_nodes`, in `node_set.rs` on `canonical_of`, serves the judge, `Named::of` and `tested.rs`, so Q2's "names no node" matches `node_gone` by construction. Left, needing new tests and a fixture: Q3 for sections without an ID; Q1's newer schema.

## Implementation

| Module | What it does |
|---|---|
| CLI `src/main.rs` | `Verify`'s `--task`: `conflicts_with = "staged"` for `requires = "tests"`, help naming both uses, the about's `--task` sentence; the manual check and `task_without_tests` gone; `VerifyRequest.task` passed |
| CLI `src/verify.rs` | `VerifyRequest.task`, `VerifyOutcome.task`, `Gathered.task`; T resolved after discovery and `slug()` (`open_context`, `named_task`, `escaped_error`), before any run cause; `keep_task_rows` in the judged arm, only with no run cause, before `concluded`: `retain` over `NodeSet::row_nodes`, both halves, Q2 and Q3 notes, `skipped` 0; `render_text`'s prefix; `VerifyJson.task`; `STAGED_TASK` |
| CLI `src/node_set.rs` | `named_task` moved here verbatim (`pub(crate)`); `Named` (`of`, `any`): whether a row names any node of the corpus now (Q2) |
| CLI `src/verify_tests.rs` | its private `named_task` dropped, `node_set`'s called |
| CLI `src/lib.rs` | docs: the `spec verify [--changed \| --staged]` sentence, the `verify` item; the `verify_tests` item left, still accurate |
| tests | new CLI `verify_task.rs`; `verify_common/mod.rs` (`TOP_KEYS` 10; `new_task`, `plan_task`, `approve_task`, `claim_task`); `verify_tests_forms.rs` (`verify_task_without_tests_is_usage` deleted) |

Deviation, kept (the owner's decision, 2026-10-20): the library refuses `VerifyRequest { tree: Staged, task: Some(_) }` before discovery with clap's text, `the argument '--task <T>' cannot be used with '--staged'` (`CliError`, exit 2), so a library caller never runs `--staged` silently ignoring the task; unreachable through the binary, where clap refuses first.

After the tests, a micro developer pass (the review's nits, no behaviour) rewrapped `named_task`'s doc citation in `node_set.rs`, read by `doc_pointers.rs` again, and dropped `Named`'s unused `Clone`, `Default` derives.

Tests: nextest core, code, CLI, MCP, HTTP, store 2 925 passed, 12 skipped; eval `build_graph`, `doc_pointers`, `anonymity` 58 passed; clippy `--workspace --all-targets -D warnings` clean. New CLI `verify_task.rs`, 10 tests: the oracle is plain verify kept to a hand-written set; TE-4's read-only checks inside every `--task` run; TE-1 over the golden with the `drift()` edits, `fixtures/anchor-states` (18 scenarios together), `fixtures/verify-tests` (kiln), `verify-ecs`. Mutations 6 of 6 killed by their named tests, variants a, b, c each red (M3b by the homoglyph, not the alias). A run cause skips the filter, so the set's notes are absent then (step 4 of the canon).
