---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 8a-1 (WA-1..WA-17; Q1-Q9 2026-10-10; refreshed and split 2026-10-16, S1-S14, P1-P13); 05 s5.4; 06 s6; 07 s2; 08 s2, AC-8; Rust 3 iterations, review accepted, tests 3 rounds; TE-9 the owner's
shipped: 2026-10-16
adrs: [ADR-0044]
---

# verify-tests (8a-1): bound tests run by the project's own command

## Why

`verified` was a tester agent's claim (`plugin.md`: "not a machine verdict"); 08 AC-8 wants SpecEngine to run the `@verifies` tests itself; 05 s5.4 hard-coded one runner against ADR-0008, ADR-0027.

Delivered (8a-1): `[verify] tests`, the project's runner as an argv template, and `timeout`; `spec verify --tests [--task T] [--json]` runs the tests `@verifies` binds to the selected nodes, one invocation per distinct argv, only on the owner's terminal after one `[y/N]`, over a clean committed tree, each started run a `verify.tests` event; the node-set functions 8a-2 and 6b-2 share (P2). **8a-2** `verify-task.md` (plain `spec verify --task T`) follows 5b-1; **8b** `tested-evidence.md` writes the evidence into `spec.lock` and computes `verified`, closing AC-8. No new ADR: ADR-0044, ADR-0042, ADR-0012/0039, ADR-0008/0027.

## Description and interactions

Canon: `docs/canon/spec-verify-tests.md` (all of it), `spec-verify-output.md` "`--tests`", `spec-verify.md` "The `[verify]` table"; `architecture-code.md#sync-in-reads` (P11), "Tests as evidence"; `proposal-queue.md` "States and events"; `stack.md` (`libc`); the core, store, CLI, HTTP READMEs; 05 s3.4, s3.5, s5.4, s5.5; 06 s6, s8; 07 s2, s5; 08 s2, AC-8.

## Data

`spec-verify-tests.md`: "The `[verify]` keys" (WA-2), "The node set" (P2, P6), "Placeholders" (WA-3, P5), "Runnable and verifiable" (WA-4, P4, P7), "Order" (WA-6..WA-8, Q8, Q9, V2, V3), "Runner" (WA-10, P8), "Verdicts and event" (WA-9, WA-11, WA-15, P13, V1), "Known limits"; texts, text and JSON: `spec-verify-output.md` "`--tests`" (WA-16, S14).

### Fixture

P10. `fixtures/verify-tests/`, shared with 8b, never built: a `[code]` project with an `expected.json`; a unit test in `src/`'s `mod tests`, an integration test under `tests/`, a module marker, a `cfg` twin pair, one node unverifiable by a refusal 5b-1 keeps, one test bound to two nodes.

## Rules and edge cases

- WHEN a value holds spaces, `;`, `$()`, quotes or `*`, the runner SHALL get those bytes as one argv element.
- WHEN a test overruns or a signal is forwarded, no process left in the runner's group SHALL outlive `spec` (escapes: "Known limits").
- WHEN two tests give one argv, it SHALL run once and both SHALL take its outcome.
- WHEN run from two roots, another cwd or in reverse creation order, stdout and JSON SHALL be byte-identical.
- Genre (P12): 8a's files and core `verify.rs` name no runner or its flags (`nextest`, `test(=`, `--exact`, `cargo test`); a qpath is data, never split. Nothing blocks (ADR-0012, ADR-0039): no state, claim, approval, gate, check or task transition reads a run.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [x] RD-1 -- the table, the forms (P1). M: a string accepted -> `verify_tests_must_be_an_argv_list`; an unknown placeholder -> `verify_tests_unknown_placeholder_is_refused`; no `{test}`/`{qpath}` -> `verify_tests_needs_a_test_placeholder`; timeout 0 or 86 401 -> `verify_timeout_out_of_range_is_refused`; `--tests` with `--changed` or `--staged` accepted -> `verify_tests_with_staged_is_usage`; `--task` alone run -> `verify_task_without_tests_is_usage` (8a-2 replaces it). 4a's `is not built` cases (core `verify_unknown_key_is_refused`, CLI `verify_modes.rs`) flipped.
- [x] RD-2 -- terminal first, one door. M: config read first -> `verify_tests_not_a_terminal_wins_over_a_bad_config`; an MCP or HTTP path to the runner -> `mcp_and_http_never_run_tests`.
- [x] RD-3 -- selection, verifiability. M: module run -> `module_verifies_marker_is_not_run`; an unverifiable node's test run -> `tests_of_an_unverifiable_node_are_not_run`; run twice -> `a_test_of_two_nodes_runs_once`; ` #cfg(…)` left in `{test}` -> `cfg_twin_runs_its_undiscriminated_name`; an ignored file's test run -> `ignored_file_test_is_not_run`; a second predicate passing a null-hash item -> `verifiable_is_accepts_predicate`.
- [x] RD-4 -- argv, probe. M: a `sh -c` join -> `placeholder_value_never_becomes_shell_syntax`; `{test}` cut from the rendered qpath -> `placeholders_come_from_the_structured_name`; the probe skipped -> `runner_passing_an_empty_match_is_refused`.
- [x] RD-5 -- verdicts. M: exit ignored -> `failing_test_never_verifies`; a time-out as a pass -> `timed_out_test_is_not_passed`.
- [x] RD-6 -- runner. M: only the child killed -> `no_grandchild_outlives_a_timeout`; no forwarding -> `interrupt_kills_the_runner_group`; stdin inherited -> `runner_stdin_is_null`; `GIT_DIR` kept -> `runner_env_has_no_git_repository_variables`; output on stdout -> `runner_output_never_reaches_stdout`; only the tail hashed -> `output_hash_covers_the_whole_stream`.
- [x] RD-7 -- event, tree. M: a transition or `runs` row -> `test_run_moves_no_task`; none on a refused probe -> `refused_run_is_recorded`; no re-check -> `tree_changed_during_the_run_is_not_verified`; untracked files ignored -> `untracked_file_refuses_the_run`.
- [x] RD-8 -- JSON. M: timing -> `verify_tests_json_has_no_timing`; a key dropped -> `verify_tests_json_every_key_present`.
- [x] RD-9 -- only `libc` (P8). M: a `[target.'cfg(unix)']` table -> `store_and_cli_normal_dependencies_are_2a1_s`; any other new crate -> it and `cli_direct_dependencies_are_unchanged_by_w`.
- [ ] RD-10 (optional) -- the plugin's tester line, plugin 0.10.1: not done ("Implementation").
- [x] RD-11 -- the node set (P2, P6). M: the snapshot read -> `task_set_reads_the_columns_now`; containment dropped -> `task_selection_includes_contained_sections`; a homoglyph marker or a lock-entry row dropped -> `node_set_rows_follow_refs_listing`; an unknown or another repository's task at exit 1 -> `verify_tests_task_unknown_is_exit_2`.

test-engineer:

- [x] TE-1 -- temp repos from `fixtures/verify-tests/` (never built); self-terminating fake runners recording argv and pid: pass, fail, hang, a grandchild, a flood, exit 0 on any test, read stdin, write the tree; no `cargo`.
- [x] TE-2 -- AC-8 so far: `report_run` -> no event; pass -> `verified`, exit 0, `spec.lock` untouched; fail -> exit 1; probe exit 0 -> exit 2, no test; stdin a pipe -> exit 2, no runner started; declined -> exit 1, no event. A consent closure.
- [x] TE-3 -- grandchild pids dead after a 2 s timeout and after `SIGINT` (event `interrupted`); a 1 MiB flood never on stdout, its hash the whole stream's.
- [x] TE-4 -- a case per "Runnable" reason and selection rule; an alias marker; a feature-scoped AC via `--task` criteria; a `cfg` twin pair, one run. TE-5 -- names with spaces, `;`, `$()`, quotes verbatim; a newline -> `control character`.
- [x] TE-6 -- modified, staged, untracked -> `dirty`, exit 1, nothing run; a runner writing the tree, `HEAD` moving -> `changed`. TE-7 -- two roots, another cwd, reverse creation: identical stdout, JSON.
- [x] TE-8 -- genre scan (P12); `fixtures/spec-c` exits 2 naming `[code]`.
- [ ] TE-9 -- pilots, the owner's manual run (`verify_tests_pilot.rs`, `#[ignore]`, a scratch clone, answer `n`): 0 bound tests (A2), no build, no pilot name committed.

spec-writer:

- [x] SW-1 -- this spec (2026-10-10; refreshed and split 2026-10-16: `verify-task.md`).
- [x] SW-2 -- at shipping: Tier 2 `docs/canon/spec-verify-tests.md` and its index line; `spec-verify-output.md` "`--tests`" (texts, text, JSON; keeps the new canon near 8.7 KB for 8b); `spec-verify.md` pointers (net +35 B); `architecture-code.md` (P11's narrowing, an "As built" line); `proposal-queue.md`, `stack.md`; the core, store, CLI, HTTP READMEs; 05 s3.4, s3.5, s5.4, s5.5; 06 s6, s8; 07 s2, s5; 08 s2, AC-8; `CLAUDE.md` State; the pointers in `verify-task.md`, `verify-node.md`, `layer-c.md`, `tested-evidence.md`.
- [x] SW-3 -- no new ADR (ADR-0044 holds); `export index`, `check` clean.

## Risks

The owner's notes (2026-10-16) are canon: `spec-verify-tests.md` "Known limits". V2 narrows provenance, not trust: `specengine.toml` and a root-relative program must be files of `HEAD`, but a `skip-worktree` or `assume-unchanged` file, an absolute program, one found on `PATH` or what a program calls is not traced, and an agent's commit can still change what runs. The owner's `y` on the template shown in the question remains the real guard. The pipeline's own tests: a scratch `HOME`, every fake runner its own alarm, never `cargo`; pilots only by TE-9's manual run.

## Out of scope

8a-2 `verify-task.md` (plain `spec verify --task T`); 8b `tested-evidence.md` (the evidence, format 3, `verified`); batching; module or RON bindings run; a dry run; MCP, HTTP, UI surfaces; `tested` in reads and Health; `impl_status`; parsing output; `tests_reaching`; CI writes; a watcher; a runner off Unix.

## Open

Decided under the owner's pre-approval (2026-10-10, 2026-10-16): Q1-Q9; the split; P1 `--tests` alone with `--changed`/`--staged` usage; P2 one node set; P3 a Tier 2 canon; P4 the disk walk, `not committed`; P5 structured names, equal argv once; P6 the task's columns now, exit 2; P7 accept's predicate; P8 plain `libc`; P9 `--no-renames`, a streaming hash; P10 a new fixture; P11 `:68` narrowed; P12 scoped scans; P13 exit 1 "not verified"; the environment: the owner's minus git's. After review 1: V1 (deviation 4) `nodes: []` whenever step 9 did not complete; V2 the template's provenance checked. After review 2: V3 a `..` in the program path climbs only before the first written name.

## Implementation

| Module | What it does |
|---|---|
| core `verify.rs` | `tests`, `timeout` leave `NOT_BUILT_KEYS`; `TestTemplate` (`parse`, `expand`), `PlaceholderValues`, `PROBE_TEST`, `TIMEOUT_{DEFAULT,MAX}`; pure `node_verdict`, `tests_verdict` |
| store `worktree.rs`, `lib.rs`, `queue.rs` | `status_all`; `B3Stream`; `record_verify_tests` (one `Immediate` transaction) |
| CLI `verify_tests.rs` (new) | steps 1-11, selection, `Tracked::program_at_head` (V2, V3), the question, verdicts, text, JSON, the event |
| CLI `test_runner.rs` (new, unix) | group, pipe, `waitid` `WNOWAIT`, `killpg`, forwarding: the CLI's only `unsafe` |
| CLI `node_set.rs` (new) | `task_set`, `NodeSet::row_nodes` (P2) |
| CLI `lock.rs`, `task.rs`, `code_walk.rs` | `committed_bindings`, `node_at_head` `pub(crate)`, bodies unchanged (P7); `not_a_task`; `tree_packages` |
| CLI `main.rs`, `lib.rs`, `Cargo.toml` | `--tests`, `--task`, Unix, terminal, `ask`; exports; `libc` |

Review 1's deviations, accepted, now canon: (1) probes stop at the first refusal, the event lists those started, `counts.probes` the planned count; (2) a test stopped by a forwarded signal: `not run`, `interrupted`, exit, signal, hash, tail null; (3) `timed_out`: exit, signal null; (4) V1; (5) step 10 only after a completed step 9, a git failure there `changed`; (6) a keyless binding: kind its state, symbol its category, `not a function`; (7) the node's own file not in `HEAD`: `not committed`; (8) `rev_unknown` or `hash_null` from the corpus, never the refusal's text; (9) open texts: `spec-verify-output.md` "`--tests`"; (10) `open_context` failing: a `cannot` document, task errors exit 2, stdout empty; (11) off Unix the library refuses before the terminal check.

Iteration 2: test round 1's defect fixed (`--changed` or `--staged` with `--task` and no `--tests` ran plain verify, clap waiving `requires` beside a conflict: now usage, exit 2); V1 `nodes: []` unless step 9 completed; V2 the root's `specengine.toml` and a root-relative program files of `HEAD`, else `cannot`; review 1's minor (`primary_target` compares components); nits: a keyless row per marker comment, a leader that cannot be waited on never signalled, first-appearance maps. Iteration 3: V3 (review 2's minor, owner-approved): a `..` after a written name is `cannot`, so an ignored symlinked directory never redirects the program. Review 2's nits are canon lines (absolute program, waived forms' usage text, `SIGCHLD` ignored).

RD-10 not done (`plugin/` not writable in the run): plugin 0.10.0's tester line stays "not a machine verdict" until 8b. Pins churned by design: core `verify_unknown_key_is_refused`, CLI `verify_reads_its_table_closed`, eval `build_graph` ×3 and `proposal_genre::ac20_no_dependency_is_added` (`libc`), `code_refs_walk` `PATH_LITERAL_EXEMPT` (the literal `verify.tests`). Frozen bytes held; `specengine-code` untouched.

Tests: round 1, 8/8 mutations killed, one defect found and fixed; round 2, 5/5 killed; round 3: V3 targets. TE-9, the owner's pilot run, pending.
