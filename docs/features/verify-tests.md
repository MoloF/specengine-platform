---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 8a-1 (WA-1..WA-17; Q1-Q9 2026-10-10; refreshed and split 2026-10-16 against shipped 4b-2, 6a-1..6b-1, S1-S14, P1-P13, decided 2026-10-16 under the owner's pre-approval, review open); 05 s5.4; 06 s6; 07 s2; 08 s2, AC-8
adrs: [ADR-0044]
---

# verify-tests (8a-1): bound tests run by the project's own command

## Why

`verified` is today a tester agent's claim (`plugin.md`: "not a machine verdict"); 08 AC-8 wants SpecEngine to run the `@verifies` tests itself; 05 s5.4 hard-codes one runner against ADR-0008, ADR-0027.

Delivered (8a-1): `[verify] tests`, the project's runner as an argv template, and `timeout`; `spec verify --tests [--task T] [--json]` runs the tests `@verifies` binds to the selected nodes, one invocation per distinct argv, only on the owner's terminal after one `[y/N]`, over a clean committed tree, each run a `verify.tests` event; the node-set functions 8a-2 and 6b-2 share (P2). Split 2026-10-16 (owner): **8a-2** `verify-task.md` (plain `spec verify --task T`) after 5b-1, so its rows carry `levels` from the start. 8b `tested-evidence.md` writes the evidence into `spec.lock` and computes `verified`, closing AC-8. No new ADR: ADR-0044 (`architecture.md#apply`, `#tasks`; `architecture-code.md#spec-lock`, `#tests`), ADR-0042, ADR-0012/0039, ADR-0008/0027.

## Description and interactions

- **The owner** runs it on a terminal after committing. **An agent** never starts it: no MCP tool, route, UI button, hook, CI step or role (ADR-0044); `report_run` changes nothing (07:38). **A project** writes its runner; SpecEngine names none and parses no output (WA-17).
- **Crates.** Core `verify.rs`: the keys leave `NOT_BUILT_KEYS` (:42) for `VerifyConfig`; expansion, verdicts pure (`judge_row`, `verdict` unchanged). Store: P9's three additions. CLI: the flags, the run, the runner (one `#[cfg(unix)]` module, the CLI's first `unsafe`), the node set; `lock.rs` `committed_bindings`, `node_at_head` `pub(crate)` (P7); `libc` (P8). Untouched: `verify_rows.rs`; `specengine-code` (`levels.rs`, `hash.rs`, `ron/` frozen).
- **Unchanged**: plain, `--changed`, `--staged` verify; `spec check`, the gate, task states, claims, `runs`, `INDEX_FORMAT`, the queue schema, MCP, HTTP code, UI, `.githooks`, CI; `spec.lock` (8b writes it). **Writes**: the data directory (the index refresh, the queue when absent, the event); the command's own writes are the project's (step 10).
- **Overridden at shipping** (S8, S9): SW-2's lines.

## Data

### The verify table

WA-2. Two keys join 4a's `mode` (`docs/canon/spec-verify.md` "The `[verify]` table"):

```toml
[verify]
tests   = ["scripts/run-test", "{package}", "{target_kind}", "{target}", "{test}"]
timeout = 600
```

- `tests`: a TOML array of strings, run as argv, never through a shell; a string -> ``[verify] tests must be an argv list, e.g. ["scripts/run-test", "{test}"]``. Element 0, the program, non-empty after expansion: with a `/` and relative, joined to the root (std leaves a relative program under `current_dir` platform-specific); else looked up on `PATH` as `execvp` does. Placeholders, closed: `{file}`, `{qpath}`, `{package}`, `{target_kind}`, `{target}`, `{test}`; `{{`, `}}` literal braces, any other brace an error; `{test}` or `{qpath}` required. A shell if wanted: `["sh", "-c", "<script using \"$1\">", "sh", "{test}"]`.
- `timeout`: seconds per invocation, probe included, integer 1..86 400, default 600.
- Errors `specengine.toml:<line>: <message>`, exit 2; plain verify reads them as run cause (1), as `mode`'s. No `tests`: `--tests` exits 2 `[verify] tests is not set in specengine.toml`. This repository adds no `[verify]`.

### The node set

P2, P6. Two CLI functions, built here, shared with 8a-2 and 6b-2 (`verify-node.md` "spec verify --node"):

- **A task's set** (`--task T`): the task's `targets`, `criteria` refs and `affected_nodes` columns (`tasks.md` "Store"), in any state, never `snapshot` (a `draft` task has none), each resolved now as `spec show` resolves a REF, plus every section they contain. A ref naming nothing now: `note: verify: <ref> names nothing now`, skipped. `T` unknown, another repository's, in `[ids]` or a look-alike: exit 2 in every mode, stdout empty, `tasks.md` "Commands"' texts (S13: verify's exit 1 means a Fix, ADR-0042).
- **A row in a set**: 6b-2's Rows rule over a set. It holds a marker `spec refs` lists under a set node (`code-bindings.md` "ID state": `resolved`, `alias`, each `ambiguous`, `homoglyph` under its fix's node), or its lock entry's `node` is a set node's ID; a section or parent only when the builder added it (`--task` adds contained sections, `--node` never). `--tests` applies the marker half to `@verifies` bindings; 8a-2 and 6b-2 apply both halves to verify's rows.

### Placeholder values

WA-3, P5. From the structured name (`specengine-code` `placed_name_file`: `role`, `paths`), never by splitting the qpath (`symbol-names.md` "Discriminator"). A test in `mod tests` of `crates/game/src/stamina.rs`, one in `crates/game/tests/stamina.rs`:

| Placeholder | Value | lib test | integration test |
|---|---|---|---|
| `{file}` | root-relative | `crates/game/src/stamina.rs` | `crates/game/tests/stamina.rs` |
| `{qpath}` | `Name::undiscriminated()` | `crates/game::stamina::tests::regen_stops_at_zero` | `crates/game::test:stamina::drain_is_clamped` |
| `{package}` | the manifest's `[package] name`; the walk's directory-name fallback gives none | `game` | `game` |
| `{target_kind}`, `{target}` | the unit's (`Unit::kind_str`, `name`); the primary target's own from the package's layout table | `lib`, `game` | `test`, `stamina` |
| `{test}` | `QPath` `module` segments and `name` joined by `::` (libtest's name) | `stamina::tests::regen_stops_at_zero` | `drain_is_clamped` |

A value goes into its element as bytes; elements are never split or joined. The layout ignores `[lib] name` and `[[test]] path` (`code-targets.md`): a wrong `{target}` reads `failed`, never `passed`. Tests whose argv are equal run once and share the outcome: `cfg` twins, or one `{test}` in two targets of a `{test}`-only template.

### Runnable

WA-4, P4. A `@verifies` binding runs only as a free `function_item` of a rooted, non-`shared:` unit, in a file `HEAD` holds, each used placeholder with a value free of U+0000-U+001F, U+007F. Else `not run`, the first reason: `not committed` (`is_tracked` false: an ignored file the walk reads), `module`, `ron`, `unrooted`, `shared unit`, `member` (of an `impl`, `trait`), `not a function`, `control character`, `no value {<name>}`; then `node not verifiable` when no node of the set it binds is verifiable. Core never looks for `#[test]`: the runner and the probe decide.

### Verifiable

P7. A node is verifiable when accept's own sorting refuses none of its bindings, any relation: CLI `lock.rs` `committed_bindings`, after step 4, over the walk's marker files naming the node, `HEAD`'s bytes, `node_at_head`'s `rev` and an empty old lock (no `spec.lock` read: 8b's), gives no `unlockable` and no `keyless` entry. Else `not verifiable <state>`: the first refusal, keyed ones by key, then keyless ones by place, in the predicate's words, never typed here, so 5b-1's new refusals (`bad_levels`, parse errors) change no call or text. A marker file naming the node that `HEAD` lacks: `not committed` (accept's "tracked and clean"), the predicate not called. `node_at_head` refusing: `rev_unknown` or `hash_null` (`spec-verify.md` "States"). 8a calls no hasher: 5b-1's pin is untouched.

### Selection

WA-5, WA-1. The set: with `--task T`, "A task's set"; else every node a `@verifies` binding names. The tests: every `@verifies` binding in the set ("A row in a set"), listed once per (`file`, `symbol`) with its set nodes. A node is reported when a `@verifies` binding names it; with `--task`, `note: verify --tests: <n> nodes of <T> have no @verifies binding`.

### Order

WA-6..WA-8, Q8, Q9. `tested-evidence.md` cites steps 6, 7, 10, 11.

1. Usage, exit 2, stdout empty (P1): `--tests` with `--changed` or `--staged`; `--task` without `--tests` (8a-2 lifts it). Not Unix: exit 2 `spec verify --tests runs on Unix only`. `require_terminal("verify --tests")` before any config, task or code read.
2. Discovery, config: `[verify]`, `[code]` (none: `code-bindings.md` "The `[code]` table"'s text); exit 2.
3. Git (store `WorktreeGit`): a worktree, `HEAD` born, on a branch, no operation in progress (`place`, `operation_in_progress`); else exit 2.
4. The whole tree clean (P9's status, no pathspec): any entry -> `dirty`, exit 1, nothing more read. Disk then equals `HEAD` but for ignored files (P4).
5. `committer_ident`; the slug's queue opened as `spec task` opens it (created when absent; a newer schema: exit 2); with `--task` the task (exits: "The node set").
6. The corpus, the code walk (`code_walk::read_tree`; a gap: exit 2, `code-bindings.md` "The walk"), the set, the tests, "Runnable", "Verifiable". Nothing to run -> `empty`, exit 1, no question, no event.
7. stderr: per test `run | <file> | <kind> <symbol> | <nodes>` or `not run | … | <reason>`, per node `node <ID>: not verifiable <state>`; then `run <n> bound tests of <k> nodes as <template> (timeout <t> s each) in <worktree> on <branch> at <commit>? [y/N]` (`<n>` distinct argv, `<k>` verifiable nodes). Only `y`, `yes`; no `--yes`; else `declined`, exit 1, no event. Ctrl-C here: `spec` ends, nothing started, no event.
8. **Probe**: per distinct argv with `{test}` = `specengine_verify_probe_no_such_test` and `{qpath}` the test's with that last segment, other values the test's: one run before any test, as "Runner". It must exit non-zero or by a signal within `timeout`. Exit 0 -> `refused`, exit 2, `the [verify] tests command passed a test that does not exist; make it fail when no test matches`; a time-out -> `refused`, exit 2 naming it.
9. Each distinct argv once, in the order of its first test by (`file`, `symbol`) bytes; then the verdicts.
10. `HEAD`, the branch and step 4's status read again: any difference -> `changed`, exit 1, no node verdict.
11. The event, then stdout.

### Runner

WA-10, P8. One invocation at a time (std `Command`), own process group (`process_group(0)`), cwd the root, stdin null, environment store `WorktreeGit::vars()` (the owner's environment minus `git rev-parse --local-env-vars` and `LOCAL_ENV_VARS`, `GIT_TERMINAL_PROMPT=0`; the owner's decision: "Risks"). stdout and stderr share one `std::io::pipe`, copied live to `spec`'s stderr and BLAKE3-hashed whole (P9's streaming hash); a failed or timed-out run keeps its last 4 096 bytes (lossy UTF-8). Nothing reaches stdout; no log file.

- **Overrun**: `SIGTERM` to the group, 5 s grace, `SIGKILL`. **Every exit**: the leader's exit seen without reaping it (`waitid` `WNOWAIT`), the group gets `SIGKILL`, then the reap, so a signalled group ID is never a reused one; the pipe read to EOF or 5 s more.
- **Signals**: from step 8 to the end of step 9, `SIGINT`, `SIGTERM`, `SIGHUP` to `spec` (handlers by `sigaction`, a flag) are forwarded to the group as `SIGTERM`, the same escalation; verdict `interrupted`, exit 2, the event written.
- A program that cannot start (`ENOENT`, `EACCES`): `refused`, exit 2 naming it and the error.
- **Code**: std and `libc` only (process crates banned: `build_graph.rs`); `killpg`, `sigaction`, `waitid` in one module, a `// SAFETY:` per block, copied after eval's `ra/sys.rs` `kill_group` (`CLI_FORBIDDEN`). `libc.workspace = true` in plain `[dependencies]` (a target table is banned), the runner `#[cfg(unix)]`.

### Verdicts

WA-9, WA-15, P13. A test: `passed` (exit 0 in time), `failed` (non-zero or a signal), `timed out`, `not run`. A node: `verified` (one runnable test or more, all passed), `failed`, `not verifiable <state>`, `no runnable test`.

| Run verdict | Exit | When |
|---|---|---|
| `verified` | 0 | every reported node verified |
| `unverified` | 1 | a reported node not verified |
| `dirty`, `empty`, `declined`, `changed` | 1 | steps 4, 6, 7, 10 |
| `refused`, `interrupted` | 2 | a probe passed or timed out, a program cannot start; a forwarded signal |
| `cannot` | 2 | steps 1-3, 5, 6's exit 2 (stdout empty before discovery) |

Exit 1 here means "not verified", ADR-0044's command outside ADR-0042's judge (P13), never a Fix finding.

### Event

WA-11, Q3, P9. One `verify.tests` per run that reached step 8 (`verified`, `unverified`, `changed`, `refused`, `interrupted`), written last on step 5's connection by store `record_verify_tests` (a twin of `record_gate_observed`, `queue/gate.rs` :152: one `Immediate` transaction, the build's schema); a failure: `warning: verify.tests not recorded: <error>`, exit unchanged. Streamed by `specengine-http`, outside the UI's closed list (`QUEUE_EVENT_TYPES`), kept by a dump. Payload keys in order: `task` (or null), `worktree`, `branch`, `commit`, `command` (the template), `timeout`, `verdict`, `probes` (`argv`, `exit`, `signal`, `outcome`, `output_hash`), `tests` (`file`, `symbol`, `nodes`, `argv`, `exit`, `signal`, `outcome`, `output_hash` `b3:` or null, `tail` or null), `nodes` (`id`, `verdict`, `state`), `lock_commit` (null until 8b: `tested-evidence.md` "The run's write"), `by` (the committer identity).

**Store** (P9): `status --porcelain=v1 -z --no-renames --untracked-files=all`, no pathspec (`status_entries` gives `[]` without paths, `worktree.rs` :1069, and stays); a streaming BLAKE3 giving `b3_hash`'s form; `record_verify_tests`. No new dependency.

### Output

WA-16. **Text**, stdout: per test by (`file`, `symbol`) `<passed|failed|timed out|not run> | <file> | <kind> <symbol> | <nodes>[ | <reason>]` (`exit <n>`, `signal <n>`, `after <t> s`, not run's); `node <ID>: <verdict>[ <state>]` by ID bytes; the summary `verify --tests[ --task <T>]: <verdict>: ` + counts, or the reason for `dirty`, `empty`, `declined`, `changed`, `refused`, `interrupted`; a test never started is `not run`, reason the run verdict. `dirty`'s and `changed`'s reasons name each path with its two status letters and end `commit them first`.

```
passed | crates/game/src/stamina.rs | item crates/game::stamina::tests::regen_stops_at_zero | R-12
failed | crates/game/tests/stamina.rs | item crates/game::test:stamina::drain_is_clamped | R-12, R-13 | exit 101
not run | crates/game/src/stamina.rs | item crates/game::stamina::Regen::check | R-14 | member
node R-12: failed
node R-13: failed
node R-14: no runnable test
verify --tests: unverified: 3 nodes (0 verified, 2 failed, 0 not verifiable, 1 no runnable test); 3 tests (1 passed, 1 failed, 0 timed out, 1 not run)
```

**JSON** (exits 0, 1, 2 after discovery; S14): one document, every key present, absent `null`, no timing, output hash, tail or worktree; keys in order: `task`, `branch`, `commit`, `command`, `timeout`, `verdict`, `reason` (null for `verified`, `unverified`, `cannot`; SW refinement), `counts` (`nodes`: `verified`, `failed`, `not_verifiable`, `no_runnable_test`; `tests`: `passed`, `failed`, `timed_out`, `not_run`; `probes`), `tests` (`file`, `kind`, `symbol`, `nodes`, `argv` or null, `outcome` `passed|failed|timed_out|not_run`, `exit`, `signal`, `reason`), `nodes` (`id`, `verdict` `verified|failed|not_verifiable|no_runnable_test`, `state`; `[]` under `changed`), `lock` (null until 8b), `cannot`, `notes`.

### Fixture

P10. A new `fixtures/verify-tests/`, shared with 8b: a `[code]` project with an `expected.json`; a unit test in `src/`'s `mod tests`, an integration test under `tests/`, a module marker, a `cfg` twin pair, one node unverifiable by a refusal 5b-1 keeps (`ambiguous`, `homoglyph`, `spec_ahead`, `bad_rev`, `unanchored`), one test bound to two nodes. Never built. `code-a` has no cleanly verifiable node, `verify-ecs` no `@verifies` marker (S10).

## Rules and edge cases

- WHEN a value holds spaces, `;`, `$()`, quotes or `*`, the runner SHALL get those bytes as one argv element.
- WHEN a test overruns or a signal is forwarded, no process left in the runner's group SHALL outlive `spec` (escapes: "Risks").
- WHEN two tests give one argv, it SHALL run once and both SHALL take its outcome.
- WHEN run from two roots, another cwd or in reverse creation order, stdout and JSON SHALL be byte-identical.
- Genre (P12, S11): 8a's files and core `verify.rs` name no runner or its flags (`nextest`, `test(=`, `--exact`, `cargo test`; 07 P2-3, ADR-0027); a qpath is data, never split.
- Nothing blocks (ADR-0012, ADR-0039): no state, claim, approval, gate, check or task transition reads a run.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- the table, the forms (P1). M: a string accepted -> `verify_tests_must_be_an_argv_list`; an unknown placeholder -> `verify_tests_unknown_placeholder_is_refused`; no `{test}`/`{qpath}` -> `verify_tests_needs_a_test_placeholder`; timeout 0 or 86 401 -> `verify_timeout_out_of_range_is_refused`; `--tests` with `--changed` or `--staged` accepted -> `verify_tests_with_staged_is_usage`; `--task` alone run -> `verify_task_without_tests_is_usage` (8a-2 replaces it). 4a's `is not built` cases (core `verify_judge.rs` `verify_unknown_key_is_refused`, CLI `verify_modes.rs`) flip to these.
- [ ] RD-2 -- terminal first, one door. M: config read first -> `verify_tests_not_a_terminal_wins_over_a_bad_config`; an MCP or HTTP path to the runner -> `mcp_and_http_never_run_tests` (P12's scan for the runner's entry function).
- [ ] RD-3 -- selection, verifiability. M: module run -> `module_verifies_marker_is_not_run`; an unverifiable node's test run -> `tests_of_an_unverifiable_node_are_not_run`; run twice -> `a_test_of_two_nodes_runs_once`; ` #cfg(…)` left in `{test}` -> `cfg_twin_runs_its_undiscriminated_name`; an ignored file's test run -> `ignored_file_test_is_not_run`; a second predicate passing a null-hash item -> `verifiable_is_accepts_predicate`.
- [ ] RD-4 -- argv, probe. M: a `sh -c` join -> `placeholder_value_never_becomes_shell_syntax`; `{test}` cut from the rendered qpath -> `placeholders_come_from_the_structured_name` (a module file `src/a #1.rs`); the probe skipped -> `runner_passing_an_empty_match_is_refused`.
- [ ] RD-5 -- verdicts. M: exit ignored -> `failing_test_never_verifies`; a time-out as a pass -> `timed_out_test_is_not_passed`.
- [ ] RD-6 -- runner. M: only the child killed -> `no_grandchild_outlives_a_timeout`; no forwarding -> `interrupt_kills_the_runner_group`; stdin inherited -> `runner_stdin_is_null`; `GIT_DIR` kept -> `runner_env_has_no_git_repository_variables`; output on stdout -> `runner_output_never_reaches_stdout`; only the tail hashed -> `output_hash_covers_the_whole_stream`.
- [ ] RD-7 -- event, tree. M: a transition or `runs` row -> `test_run_moves_no_task`; none on a refused probe -> `refused_run_is_recorded`; no re-check -> `tree_changed_during_the_run_is_not_verified`; untracked files ignored -> `untracked_file_refuses_the_run`.
- [ ] RD-8 -- JSON. M: timing -> `verify_tests_json_has_no_timing`; a key dropped -> `verify_tests_json_every_key_present`.
- [ ] RD-9 -- only `libc` (P8): both CLI lists gain it. M: a `[target.'cfg(unix)']` table -> `store_and_cli_normal_dependencies_are_2a1_s`; any other new crate -> it and `cli_direct_dependencies_are_unchanged_by_w`.
- [ ] RD-10 (optional) -- the plugin's tester line: the machine verdict is the owner's terminal step, no command; a PATCH over the version it starts from (0.10.1 over 10a-2's 0.10.0), `PINS`. M: version kept -> `plugin_files`.
- [ ] RD-11 -- the node set (P2, P6). M: the snapshot read (a `draft` task selects nothing) -> `task_set_reads_the_columns_now`; containment dropped -> `task_selection_includes_contained_sections`; a homoglyph marker or a lock-entry row dropped -> `node_set_rows_follow_refs_listing` (both halves); an unknown or another repository's task at exit 1 -> `verify_tests_task_unknown_is_exit_2`.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/verify-tests/` (P10, never built); self-terminating fake runners recording argv and pid: pass, fail, hang, a grandchild, a flood, exit 0 on any test, read stdin, write the tree; no `cargo`.
- [ ] TE-2 -- AC-8 so far: `report_run` -> no event; pass -> `verified`, exit 0, `spec.lock` untouched; fail -> exit 1; probe exit 0 -> exit 2, no test; stdin a pipe -> exit 2, a marker file proves no runner started; declined -> exit 1, no event. A consent closure (`apply.rs`).
- [ ] TE-3 -- grandchild pids dead after a 2 s timeout and after `SIGINT` (event `interrupted`); a 1 MiB flood never on stdout, its hash the whole stream's.
- [ ] TE-4 -- a case per "Runnable" reason and "Selection" rule; an alias marker; a feature-scoped AC via `--task` criteria; a `cfg` twin pair, one run. TE-5 -- names with spaces, `;`, `$()`, quotes verbatim; a newline -> `control character`.
- [ ] TE-6 -- modified, staged, untracked -> `dirty`, exit 1, nothing run; a runner writing the tree, `HEAD` moving -> `changed`. TE-7 -- two roots, another cwd, reverse creation: identical stdout, JSON.
- [ ] TE-8 -- genre scan (P12): 8a's files and core `verify.rs` for `nextest`, `test(=`, `--exact`, `cargo test`; `fixtures/spec-c` exits 2 naming `[code]`. TE-9 -- pilots, manual (`#[ignore]`, a scratch clone, answer `n`): 0 bound tests (A2), no build, no pilot name committed.

spec-writer:

- [x] SW-1 -- this spec (2026-10-10; refreshed and split 2026-10-16: `verify-task.md`; the pointers in `tested-evidence.md`, `verify-node.md`, `layer-c.md`).
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 at `f9a80ff` (Tier 2 cap 12 288, Tier 1 10 240): a new Tier 2 `docs/canon/spec-verify-tests.md` (≤ about 8 000: "Data" but "Fixture", P3) and its line in `docs/index-canon-spec.md` (2 592); `spec-verify.md` (11 710: :11 "never … a queue row or an event", :20 "is not built", as pointers, net ≤ 80 B; the rest kept for 5b-2); `architecture-code.md` (10 711: :68, `verify` among the reads never running `status`, narrowed to the reads, P11; an "As built" line under `#tests`, about 160 B); `proposal-queue.md` (11 658: `verify.tests` in :44's closed set); `stack.md` (5 613: :39's `libc` row names the CLI); READMEs reworded net ≤ 0: HTTP (10 201, 39 B left: `verify.tests` at :64), store (10 236, 4 B left: the event, the status read, the streaming hash), core (9 968, 272 B left, which 5b-1 needs: the keys); the CLI README (9 578: "Rules" Writes, the modules, about 250 B); with RD-10 `plugin.md` (9 568: tester row) and `plugin-files.md` (4 654: version); 05:75, :95, :188, :196; 06:140, :160; 07:105, a `[verify]` table in 07 s5; 08 s2, AC-8; `CLAUDE.md` State (5 772 of 16 384).
- [x] SW-3 -- no new ADR (ADR-0044 holds); `export index`, `check` clean.

## Risks

Safety, the owner's notes (2026-10-16):

- **Code execution** with the owner's rights; the command comes from the committed `specengine.toml`, so an agent's commit can change what runs. Only the owner's `y` after the listing starts it, the question showing the template: review `[verify]` edits as code.
- **No overall cap**: N runs × the timeout; Ctrl-C, forwarded, stops it.
- **Escapes**: a `setsid` or double-forked daemon leaves the group; a `SIGKILL`ed `spec` leaves the group running.
- **`unsafe`**: the CLI's first (`killpg`, `sigaction`, `waitid`): one module, a `// SAFETY:` per block.
- **Environment** (owner decision): the owner's variables (tokens, `SSH_AUTH_SOCK`) reach the tests; only git's repository variables are removed. A project that wants less writes `env -i …` into its template.
- **Tree writes**: a test writing into the tree ends in `changed`; so does a build directory git does not ignore.
- **The pipeline's own tests**: a scratch `HOME` and `GIT_CONFIG_GLOBAL`; every fake runner its own alarm (`perl -e 'alarm N; …'`); never `cargo`, a fixture build or a second build directory; the consent closure, never a pty. Pilots never run in the pipeline: TE-9 is manual, and nothing starts before `y` (the probe is step 8, after step 7's question); pilot B stays read-only.
- **Runner semantics**: the probe triggers a build (600 s may be short cold); it catches "nothing matched", not a substring filter; a skipped `#[ignore]` test passes unless wrapped; a faked terminal passes (ADR-0034); per-test runs match the suite only for order-independent tests (A4); a `{test}`-only template runs same-named tests of two targets once.
- **Cost**: accept's predicate parses a node's marker files from `HEAD`, per node.

## Out of scope

- **8a-2** `verify-task.md`: plain `spec verify --task T [--changed]`.
- **8b** `tested-evidence.md` (Q1, Q2, Q7): the evidence, format 3, its commit under this question, `verified`.
- Batching; running module or RON bindings; a dry run; MCP, HTTP, UI surfaces; package keys; `tested` in reads and Health; `impl_status`; parsing output; `tests_reaching`; `needs:` (05:211); CI writes; a watcher; a runner off Unix.

## Open

| Point | Answer (else: the other answer's cost) |
|---|---|
| Split (owner) | 8a-1 now, M+; 8a-2 after 5b-1, S (else: an L slice, its read half re-pinned at 5b-1) |
| P1 forms | `--tests` without `--changed`, `--staged` (else: a silent no-op) |
| P2 node set | one, shared by 8a-2, 6b-2 (else: Rows rules drift) |
| P3 canon | Tier 2 `spec-verify-tests.md` (else: `spec-verify.md` over its cap at 5b-2) |
| P4 bindings | the disk walk; `not committed` (else: an ignored file's test counted for a commit lacking it) |
| P5 names | structured; equal argv once (else: a discriminated `{test}` never matches) |
| P6 task | the columns now; exit 2 (else: a `draft` task selects nothing; exit 1 reads `blocked`) |
| P7 verifiable | accept's predicate (else: two checks diverge on null hashes; 8b inherits it) |
| P8 `libc` | plain `[dependencies]` (else: a target table breaks the graph pin) |
| P9 store | `--no-renames`, streaming hash (else: rename pairs, a flood in memory, `blake3` in the CLI) |
| P10 fixture | new, shared with 8b (else: `code-a` copies edited here and in 8b) |
| P11 `:68` | narrowed at shipping (else: an amending ADR if ADR-0046 covers every mode) |
| P12 scans | scoped (else: a CLI-wide `cargo` scan fails at `serve.rs:291`) |
| P13 exit 1 | not verified (else: a decline reads as "cannot run") |
| Environment (owner) | the owner's minus git's (else: a scrub list, a genre leak) |

Q1-Q9 (2026-10-10), the split and P1-P13 (2026-10-16) decided under the owner's pre-approval, review open. S-n: the 2026-10-16 refresh's stale points; WA-n: the 2026-10-10 analysis' working answers; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs** (shipped): 1 `code-refs` (the walk), 3 `spec-lock` (accept's predicate), 4a `spec-verify`, 4b-2 `rev-rule` (`--staged`), 6a-1 `symbol-discriminator` (`Name`, `placed_name_file`), 6b-1 (plugin 0.9.0).
- **Order**: now, before 5b-1 (one working tree, one build directory); 5b-1 rebases onto the exposed `committed_bindings` and edits its body, this call unchanged; then 8a-2, then 8b.
- **Assumptions**: no project sets `[verify] tests` (refused as not built): nothing migrates; pilots have 0 bound tests (A2); a Unix-only runner is acceptable (the owner's macOS; CI runs only the docs gate).

## Implementation

Not started. The split (2026-10-16): the old "spec verify --task" section and RD-8's `verify_task_scopes_rows` -> `verify-task.md`; S1-S14 fixed in place; RD-1, RD-3, RD-6, RD-9 grown, RD-11 new, TE-1, TE-8 rescoped. To record: modules, TE-9's figures, deviations with reasons.
