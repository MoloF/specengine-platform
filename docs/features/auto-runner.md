---
class: spec
status: shipped
shipped: 2026-11-15
scope: [plugin, crates/specengine-cli]
ref: RM-0123; the owner's word of 2026-11-15 ("Claude Code drives the run itself ... I leave development overnight"); the coordinator's headless facts, checked against Claude Code's docs (build 2.1.325) on 2026-11-15; release auto-mode, plugin 0.47.0
adrs: [ADR-0081]
---

# auto-runner: the plugin's `autopilot.sh` drives a pack overnight (RM-0123)

## Why

A pack needs someone to start Claude Code on each task, call `spec auto approve` between the plan and the implementation, and `spec auto land` after. ADR-0081 keeps that someone outside `spec`: SpecEngine never starts an agent, so the runner is a plain script shipped in the plugin, and every choice it acts on comes from `spec auto next`, a deterministic read. It never skips Claude Code's permissions.

## Description and interactions

Shipped: `plugin/specengine/scripts/autopilot.sh`, POSIX `sh`; its legs, invocation, flags, loop, classification, holds, lanes and log: `docs/canon/auto-runner.md`. The skill is unchanged; the plugin is nineteen files, version 0.47.0 (`docs/canon/plugin-files.md` "Changing it").

## Data

The stream shapes the runner reads, as the tests' shim emits them (fields beyond these ignored), until RN0 records the live ones:

```json
{"type":"system","subtype":"init","session_id":"s1","mcp_servers":[{"name":"plugin:specengine:specengine","status":"connected"}],"plugins":[{"name":"specengine"}]}
{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1763280000}}
{"type":"result","subtype":"success","is_error":false,"num_turns":12,"total_cost_usd":1.2,"terminal_reason":"completed","result":"…","permission_denials":[]}
{"type":"result","subtype":"error_max_turns","is_error":true,"terminal_reason":"max_turns"}
```

**The shim** (`tests/auto_common/runner.rs`): a `claude` first on `PATH` logging its argv, the two variables and its working directory, running its scenario's `spec` commands, printing its stream lines and exiting with its code; a hold flag keeps it alive for concurrency.

## Rules and edge cases

`auto-runner.md` "Rules": overlapping tasks never run at once (`--busy`); a dead leg's state is the queue's; a checked-out integration branch holds lands while the run goes on; the runner never commits, merges, pushes or edits the project; `spec` always gets `--root`; test results are claims; SpecEngine meters nothing.

## Acceptance criteria

`crates/specengine-cli/tests/autopilot.rs`, the script under `sh` and, when present, dash and bash, the shim on `PATH`, `jq` required, over scratch `fixtures/auto-mini` with `rules/on.toml` and a pack started through the library.

- [ ] RN0 — **[owner, gate]** One live run on a scratch copy of a pilot with Claude Code at its current build: a two-task pack, `--lanes 1`; recorded in this spec's Implementation: the build, the init line's server name, a success's, a denial's and a `--max-turns 2` result's `subtype` and exit code, whether a usage-limit event was seen, and whether `SPECENGINE_AUTO_LINE` reaches the session's `spec task new` through Claude Code's Bash tool (if not, a line's leg is held `not-planned`, visibly, and lines wait for a fix).
- [x] RN1 — **No agent inside SpecEngine** (`rn1_no_agent_inside_specengine`). Mutation: `Command::new("claude")` in `auto_next.rs`.
- [x] RN2 — **One worktree per task, never two overlapping legs** (`rn2_one_worktree_per_task_never_two_overlapping_legs`). Mutation: `--busy` not passed.
- [x] RN3 — **Lanes by department** (`rn3_lanes_by_department`). Mutation: `--skip-lane` not passed for a busy lane.
- [x] RN4 — **Stop, until, usage limit, failures** (`rn4_stop_during_a_held_leg`, `rn4_until`, `rn4_a_usage_limit_pauses_and_runs_the_leg_again`, `rn4_two_failures_in_a_row_end_the_run`, `rn4_a_held_lock_is_retried_a_bounded_number_of_times`). Mutations: the pause counted as a failure; the stop answer ignored.
- [x] RN5 — **Permissions are never skipped** (`rn5_permissions_are_never_skipped`). Mutation: `--dangerously-skip-permissions` in the argv.
- [x] RN6 — **A night, end to end** (`rn6_a_night_end_to_end_sh`, `rn6_a_night_end_to_end_other_shells`): three tasks and a line landed, four `--no-ff` merges, `autopilot: done`.
- [x] RN7 — **Classification by the stream** (`rn7_no_mcp_server`, `rn7_max_turns_and_budget`, `rn7_agent_errors`, `rn7_judged_by_the_queue`, `rn7_a_success_exiting_1_goes_on`, `rn7_a_note_opening_with_a_dash_reaches_the_hold_intact`, `rn7_a_missing_working_directory_holds_agent_error`). Mutation: classifying by the exit code.
- [x] RN8 — **The plugin** (`rn8_the_script_parses_and_dry_runs`; mcp `plugin_files.rs` the nineteen files and the `PINS` entry 0.47.0). Mutation: the script left out of the set.

## Out of scope

A runner in Rust or inside the daemon; starting agents from MCP; cost metering; retrying a held item; driving a hub; Board cards; editing the skill.

## Implementation

| Module | What it does |
|---|---|
| plugin `scripts/autopilot.sh` (new) | the runner: flags, the loop over `spec auto next`, legs as process groups, classification, holds, lanes, the log, `--dry-run` |
| plugin `.claude-plugin/plugin.json` | 0.47.0 |
| tests | `tests/autopilot.rs`, `tests/auto_common/runner.rs` (the shim); mcp `plugin_files.rs` |

**The runner's own choices**, accepted: free text passed as `--flag=value`, IDs after `--`; lock refusals counted per task, 10 in a row ending the run (exit 1), past `--until` with nothing running a refusal ending it (exit 0); the land shielded from `INT`; each leg its own process group (`setsid`, else perl `setpgrp`, else the leg's process tree signalled), `TERM` then `KILL` 30 s later, a finished leg's leftovers swept; a missing working directory and a failed `git worktree add` hold `agent-error`; the one lane without `--by-department` named `all`; the log on stderr and `<log>/autopilot.log`; `--dry-run`'s `next:`, `cwd:`, `env:`, `argv:` lines; a land or approve failing three times with no hold ends the run (exit 1); an existing branch reused; past `--max-tasks` further implements passed as `--busy`, then exit 0 `max-tasks N reached`; a usage-limit pause until `resetsAt` + 60 s; a leg's working directory `<worktrees>/<T>/<root_rel>`; hold notes `<status>; no run` and `<pack>/<n> is unbound after the plan leg`.

**Known limits**: the process-tree fallback cannot reach a process already orphaned to init, and a reused pid could in theory be hit in the 30 s sweep; the stream shapes stay the docs' until RN0.
