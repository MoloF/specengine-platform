---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 8a (WA-1..WA-17; Q1-Q9 decided under the owner's pre-approval, review open); 05 s5.4; 06 s6; 07 s2; 08 s2, AC-8
adrs: [ADR-0044]
---

# verify-tests: bound tests run by the project's own command

## Why

`verified` is today a tester agent's claim (`plugin.md`: "not a machine verdict"); 08 AC-8 wants SpecEngine to run the `@verifies` tests itself; 05 s5.4 hard-codes one runner against ADR-0008, ADR-0027.

Delivered (8a): `[verify] tests`, the project's runner as an argv template; `spec verify --tests [--task T] [--json]` runs the tests `@verifies` binds to the selected nodes, one invocation each, only on the owner's terminal after one `[y/N]`, over a clean committed tree, each run a `verify.tests` event; `spec verify --task T`. 8b `tested-evidence` ("Out of scope") writes the evidence into `spec.lock` and computes `verified`, closing AC-8. ADR-0044 (`architecture.md#apply`, `#tasks`). Needs slices 1, 3, 4a (`code-refs.md`, `spec-lock.md`, `spec-verify.md`).

## Description and interactions

- **The owner** runs it on a terminal after committing. **An agent** never starts it: no MCP tool, route, UI button, hook, CI step or role (ADR-0044); `report_run` changes nothing (07:38). **A project** writes its runner; SpecEngine names none, parses no output; the pipeline never runs a pilot's suite (WA-17).
- **Crates**: core the two `[verify]` keys, expansion, selection, verdicts (pure); store the event (`record_verify_tests`, as `record_gate_observed`), a whole-tree status read; CLI the command, the runner. `libc` =0.2.189 joins the CLI, Unix only (Q4, owner-approved); nothing else.
- **Unchanged**: `spec check`, the gate, task states, claims, `runs`, `INDEX_FORMAT`, the queue schema, MCP, HTTP, UI, `.githooks`, CI; `spec.lock` untouched. **Writes**: the data directory (index, event); the command's own writes are the project's (into the tree: step 10).
- **Overridden live-spec lines**, fixed at shipping: 05:75, 05:95, 05:196, 05:204; 06:140, 06:160; 07:104 and 07 s5's example.

## Data

### The verify table

WA-2. Two keys join 4a's `mode` (`spec-verify.md` "The verify table"):

```toml
[verify]
tests   = ["scripts/run-test", "{package}", "{target_kind}", "{target}", "{test}"]
timeout = 600
```

- `tests`: a TOML array of strings, run as argv, never through a shell; a string -> ``[verify] tests must be an argv list, e.g. ["scripts/run-test", "{test}"]``. The program, first, non-empty, found as `execvp` finds it from the root. Placeholders, closed: `{file}`, `{qpath}`, `{package}`, `{target_kind}`, `{target}`, `{test}`; `{{`, `}}` literal braces, another brace an error; `{test}` or `{qpath}` required. A shell if wanted: `["sh", "-c", "<script using \"$1\">", "sh", "{test}"]`.
- `timeout`: seconds per invocation, integer 1..86 400, default 600.
- Errors `specengine.toml:<line>: <message>`, exit 2, in `spec verify` only; no `tests`: `--tests` exits 2 `[verify] tests is not set in specengine.toml`. This repository adds no `[verify]`.

### Placeholder values

WA-3, from slice 1's binding (`code-refs.md` "The walk", `code-identity.md` "Units and `qpath`"); a test in `mod tests` of `crates/game/src/stamina.rs`, one in `crates/game/tests/stamina.rs`:

| Placeholder | Value | lib test | integration test |
|---|---|---|---|
| `{file}` | root-relative | `crates/game/src/stamina.rs` | `crates/game/tests/stamina.rs` |
| `{qpath}` | the symbol | `game::stamina::tests::regen_stops_at_zero` | `game::test:stamina::drain_is_clamped` |
| `{package}` | `[package] name`; none for the directory fallback | `game` | `game` |
| `{target_kind}`, `{target}` | the unit's Cargo kind, name; the primary unit its own | `lib`, `game` | `test`, `stamina` |
| `{test}` | the qpath after package and unit (libtest's name) | `stamina::tests::regen_stops_at_zero` | `drain_is_clamped` |

A value is inserted into its element as bytes; elements are never split or joined.

### Runnable

WA-4. A `@verifies` binding runs only as a free `function_item` of a rooted, non-`shared:` unit, each used placeholder with a value free of U+0000-U+001F, U+007F. Else `not run`, the first reason: `module`, `ron`, `unrooted`, `shared unit`, `member` (of an `impl`, `trait`), `not a function`, `control character`, `no value {<name>}`. Core never looks for `#[test]`: the runner and the probe decide.

### Selection

WA-5, WA-1. Without `--task`: every node a `@verifies` binding names. `--task T` (`tasks.md` "Exits": unknown -> exit 1; another repository's or `T` in `[ids]` -> exit 2; any state): its frozen targets, criteria references, affected nodes, resolved now as `spec show` does, and the nodes they contain; one naming nothing now -> `note: <ref> names nothing now`.

- A node is **verifiable** only when every binding naming it, any relation, is lockable (`spec-lock.md` "Lockability"); else `not verifiable <state>`, the first non-lockable state.
- A test (`file`, `symbol`) runs once, shared by its nodes, if one selected node it binds is verifiable (SW refinement); else `not run: node not verifiable`.

### Order

WA-6..WA-8, Q8, Q9.

1. `require_terminal("verify --tests")` before any config, task or code read; `--tests` with `--changed`: usage, exit 2.
2. Discovery, config, `[code]` (none: `code-refs.md`'s text), `[verify] tests`; slice 1's walk and binding (a gap: exit 2).
3. A worktree, `HEAD` born, on a branch, no operation in progress (`place`, `operation_in_progress`).
4. The whole worktree clean (`git status --porcelain=v1 -z --untracked-files=all`, no pathspec), else exit 1 naming each path and state, `commit them first`.
5. `committer_ident`; the queue opens; the task is read.
6. Selection; nothing runnable -> exit 1 `empty`, no question, no event.
7. stderr: rows `run | <file> | <kind> <symbol> | <nodes>`, `not run | ... | <reason>`, `node <ID>: not verifiable <state>`; then `run <n> bound tests of <k> nodes as <template> (timeout <t> s each) in <worktree> on <branch> at <commit>? [y/N]`. Only `y`, `yes`; no `--yes`; declined: exit 1, no event. The selection goes to stderr so stdout holds one result (SW refinement).
8. **Probe**: per distinct argv with `{test}` = `specengine_verify_probe_no_such_test` (`{qpath}`'s last segment likewise), one run before any test; it must exit non-zero in time. Exit 0 -> exit 2 `the [verify] tests command passed a test that does not exist; make it fail when no test matches`; a timeout -> exit 2.
9. Each test, one at a time, by `file`, `symbol` bytes; then the verdicts.
10. `HEAD`, branch, step 4's status re-read: a change -> run verdict `changed`, exit 1, no node counts.
11. The event, then stdout.

### Runner

WA-10. One invocation at a time, own process group (`CommandExt::process_group(0)`), cwd the root, stdin null, env minus git's repository variables (`git rev-parse --local-env-vars`). stdout and stderr share one `std::io::pipe`, copied live to SpecEngine's stderr, BLAKE3-hashed whole; a failed or timed-out test keeps its last 4 096 bytes (lossy UTF-8). Nothing reaches stdout; no log file.

- Overrun: `SIGTERM` to the group, 5 s grace, `SIGKILL`, reaped; after the leader the group gets `SIGKILL`; output read to EOF or the deadline. `SIGINT`, `SIGTERM`, `SIGHUP` to SpecEngine: forwarded alike, verdict `interrupted`, exit 2. Limits: a `SIGKILL`ed SpecEngine, a `setsid` escape. Precedent: eval's `kill_group` (`crates/specengine-eval/src/ra/sys.rs`).
- Not Unix, or a command that cannot start (`ENOENT`, `EACCES`): exit 2 naming it.

### Verdicts

WA-9, WA-15. A test: `passed` (exit 0 in time), `failed` (non-zero or a signal), `timed out`, `not run`. A node: `verified` (one runnable test or more, all passed), `failed`, `not verifiable <state>`, `no runnable test`.

| Run verdict | Exit | When |
|---|---|---|
| `verified` | 0 | every selected node verified |
| `unverified` | 1 | a selected node not verified |
| `empty`, `declined`, `dirty`, `changed` | 1 | steps 6, 7, 4, 10 |
| `refused`, `interrupted` | 2 | the probe passed or timed out, a command cannot start; a signal |
| `cannot` | 2 | not a terminal, usage (stdout empty); steps 2, 3, 5 (exit-2 tasks, HOME, data directory, queue, identity); not Unix |

### Event

WA-11, Q3. One `verify.tests` per run that started a command (verdicts `verified`, `unverified`, `changed`, `refused`, `interrupted`), in the slug's queue, written last as `gate.observed` is (`gate.md` "Event": one `Immediate` transaction, the build's schema, streamed, not in the UI's closed list); a failure prints `warning:`, exit unchanged. Payload keys in order: `task` (or null), `worktree`, `branch`, `commit`, `command` (the template), `timeout`, `verdict`, `probes` (`argv`, `exit`, `signal`, `outcome`, `output_hash`), `tests` run (`file`, `symbol`, `nodes`, `argv`, `exit`, `signal`, `outcome`, `output_hash` `b3:`, `tail` or null), `nodes` (`id`, `verdict`, `state`), `lock_commit` (null until 8b), `by` (the committer identity).

### Output

WA-16. **Text**, stdout: per test by key `<passed|failed|timed out|not run> | <file> | <kind> <symbol> | <nodes>[ | <reason>]` (`exit <n>`, `signal <n>`, `after <t> s`, not run's); `node <ID>: <verdict>[ <state>]` by ID bytes; the summary:

```
passed | crates/game/src/stamina.rs | item game::stamina::tests::regen_stops_at_zero | R-12
failed | crates/game/tests/stamina.rs | item game::test:stamina::drain_is_clamped | R-12, R-13 | exit 101
not run | crates/game/src/stamina.rs | item game::stamina::Regen::check | R-14 | member
node R-12: failed
node R-13: failed
node R-14: no runnable test
verify --tests: unverified: 3 nodes (0 verified, 2 failed, 0 not verifiable, 1 no runnable test); 3 tests (1 passed, 1 failed, 0 timed out, 1 not run)
```

**JSON** (exits 0, 1, 2 after discovery), every key, absent null, no timing, output hash, tail or worktree; keys in order: `task`, `branch`, `commit`, `command`, `timeout`, `verdict`, `counts` (`nodes`: `verified`, `failed`, `not_verifiable`, `no_runnable_test`; `tests`: `passed`, `failed`, `timed_out`, `not_run`; `probes`), `tests` (`file`, `kind`, `symbol`, `nodes`, `argv` or null, `outcome`, `exit`, `signal`, `reason`), `nodes` (`id`, `verdict`, `state`), `lock` (null until 8b), `cannot`, `notes`.

### spec verify --task

`spec verify --task T [--changed] [--json]`, read-only as 4a, no terminal: rows, counts, verdict and exit over rows whose node is in the task's set ("Selection"); `cannot:` causes still apply; JSON gains `task`.

## Rules and edge cases

- WHEN a value holds spaces, `;`, `$()`, quotes or `*`, the runner SHALL get those bytes as one argv element.
- WHEN a test overruns or a signal is forwarded, no process of the runner's group SHALL outlive SpecEngine.
- WHEN run from two roots, another cwd or in reverse creation order, stdout and JSON SHALL be byte-identical.
- No core, store or CLI source names a runner or its flags (`cargo`, `nextest`, `test(=`, `--exact`; 07 P2-3, ADR-0027). Nothing blocks (ADR-0012): no state, claim, approval, gate or check reads a run.

## Acceptance criteria

rust-developer (8a), M: mutation -> the test turning red:

- [ ] RD-1 -- the table. M: a string accepted -> `verify_tests_must_be_an_argv_list`; an unknown placeholder -> `verify_tests_unknown_placeholder_is_refused`; no `{test}`/`{qpath}` -> `verify_tests_needs_a_test_placeholder`; timeout 0 -> `verify_timeout_out_of_range_is_refused`.
- [ ] RD-2 -- terminal first. M: config read first -> `verify_tests_not_a_terminal_wins_over_a_bad_config`; an MCP or HTTP path -> `mcp_and_http_never_run_tests` (source scan).
- [ ] RD-3 -- selection. M: module run -> `module_verifies_marker_is_not_run`; an unverifiable node's test run -> `tests_of_an_unverifiable_node_are_not_run`; run twice -> `a_test_of_two_nodes_runs_once`; containment dropped -> `task_selection_includes_contained_sections`.
- [ ] RD-4 -- argv. M: a `sh -c` join -> `placeholder_value_never_becomes_shell_syntax`. Probe skipped -> `runner_passing_an_empty_match_is_refused`.
- [ ] RD-5 -- verdicts. M: exit ignored -> `failing_test_never_verifies`; timeout as pass -> `timed_out_test_is_not_passed`.
- [ ] RD-6 -- runner. M: only the child killed -> `no_grandchild_outlives_a_timeout`; no forwarding -> `interrupt_kills_the_runner_group`; stdin inherited -> `runner_stdin_is_null`; `GIT_DIR` kept -> `runner_env_has_no_git_repository_variables`; output on stdout -> `runner_output_never_reaches_stdout`.
- [ ] RD-7 -- event, tree. M: a transition or `runs` row -> `test_run_moves_no_task`; none on a refused probe -> `refused_run_is_recorded`; no re-check -> `tree_changed_during_the_run_is_not_verified`; untracked ignored -> `untracked_file_refuses_the_run`.
- [ ] RD-8 -- JSON, `--task`. M: timing -> `verify_tests_json_has_no_timing`; a key dropped -> `verify_tests_json_every_key_present`; rows outside the task kept -> `verify_task_scopes_rows`.
- [ ] RD-9 -- only `libc`. M: another crate -> the CLI pin in `build_graph.rs`.
- [ ] RD-10 (optional) -- the plugin's tester line: the machine verdict is the owner's terminal step, no command; PATCH, PINS. M: version kept -> `plugin_files`.

test-engineer (8a):

- [ ] TE-1 -- temp repos from `fixtures/code-a/` (never built); self-terminating fake runners recording argv and pid: pass, fail, hang, a grandchild, a flood, exit 0 on any test, read stdin, write the tree; no `cargo`.
- [ ] TE-2 -- AC-8 so far: `report_run` -> no event; pass -> `verified`, exit 0, `spec.lock` untouched; fail -> exit 1; probe exit 0 -> exit 2, no test; stdin a pipe -> exit 2, a marker file proves no runner started; declined -> exit 1, no event. A consent closure (`apply.rs`).
- [ ] TE-3 -- grandchild pids dead after a 2 s timeout and after `SIGINT` (event `interrupted`); a 1 MiB flood never on stdout.
- [ ] TE-4 -- a case per "Runnable" reason and "Selection" rule; an alias marker; a feature-scoped AC via `--task` criteria. TE-5 -- names with spaces, `;`, `$()`, quotes verbatim; a newline -> `control character`.
- [ ] TE-6 -- modified, staged, untracked -> exit 1, nothing run; a runner writing the tree, `HEAD` moving -> `changed`. TE-7 -- two roots, another cwd, reverse creation: identical stdout, JSON.
- [ ] TE-8 -- genre scan for runner names; `fixtures/spec-c` exits 2 naming `[code]`. TE-9 -- pilots, manual (`#[ignore]`, a scratch clone, answer `n`): 0 bound tests (A2), no build, no pilot name committed.

spec-writer:

- [x] SW-1 -- this spec (8b in "Out of scope"). SW-2 -- ADR-0044, `architecture.md#apply`, `#tasks`. SW-3 -- `spec-verify.md`, `spec-lock.md`, 08 s2. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: a Tier 2 canon; the overridden lines; CLI README "Rules" Writes; `plugin.md`'s tester row; `CLAUDE.md` State.

## Risks

The probe triggers a build (600 s may be short cold); it catches "nothing matched", not a substring filter; a skipped `#[ignore]` test passes unless wrapped; a faked terminal passes (ADR-0034); a non-ignored build directory ends in `changed`; per-test runs match the suite only for order-independent tests (A4).

## Out of scope

- **8b `tested-evidence`** (Q1, Q2, Q7), to draft. The next lock `format` (2, or 3 after 5b): per node `[[tested]]` (`node`, `rev`, `spec_hash`, `bound_hash`, `tests`, `commit`, `verified_at`); `bound_hash` = BLAKE3 over every binding naming the node, in lock key order with its `ast_hash` or `file_blob`, from `HEAD`'s blobs. A run adds or refreshes a verified node's block and removes a failed node's (amends ADR-0041), nothing else, never a `[[binding]]`; an equal block stays byte-identical. Before writing, `HEAD`, branch, lock bytes, status re-read (changed: nothing written, exit 1); `commit_only` of `spec.lock`, hooks run: `spec: verify tests <IDs>`, trailers `Lock:` per node, `Lock-mode: tests`, `Decided-by:`, `Test-command:`. Accept keeps blocks, as does a `drift` item's approve, which runs accept's function (4c, `drift-queue.md`; 8b's tests cover it); a gone node's accept drops its block; regen merges per node (later `verified_at`, then bytes); the editorial refresh sets the block's `spec_hash` too, so since 4b moved it into the apply commit (ADR-0043), an editorial `spec approve` refreshes `[[tested]]`. `verified` per read: the block's `rev`, `spec_hash` and `bound_hash` current, else `stale <spec_changed|code_changed>`, `node_gone`, `unverifiable <state>`: a `tested` section of plain verify, no finding, no exit effect. Criteria: the analysis' RD-15..RD-18.
- Batching; running module or RON bindings; a dry run; MCP, HTTP, UI surfaces; package keys; `verified` in reads, Health, UI (7, 9, Phase 4); the rest of `impl_status`; parsing output; `tests_reaching`; `needs:` (05:211); CI writes; a watcher.

## Open

Q1-Q9 decided under the owner's pre-approval, review open, as applied above (Q1: `[[tested]]` beside `[[binding]]`, verified and `ok` two axes). WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. To record: modules, TE-9's figures, deviations with reasons.
