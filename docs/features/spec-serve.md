---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-http]
ref: spec-serve analysis 2026-10-08 (A1-A13; Q1-Q3 decided under the owner's pre-approval, review open); 08 s2 Phase 2, the daemon
shipped: 2026-10-08
adrs: [ADR-0039]
---

# spec serve

## Why

The owner ran the UI as two processes, and nothing made "the daemon" one thing: two could run under one `HOME`, no client could find one. `specengine-http` already was the daemon's HTTP API and SSE; this slice gave it an entry point and an identity: `spec serve` launches it, one daemon per data directory, `daemon.json` says where it listens. The gate stays daemon-free for good (ADR-0039, superseding the two gate decisions whose rule held "until `spec serve`"). The `spec mcp` bridge showed no gain (warm direct MCP reads 2-72 ms, events reach the SSE within 1 s, direct writers safe by design): it waits for a trigger. Cut: `spec-serve` (this), `ui-embed` next in Phase 4 (`rust-embed` 8.12.0 approved, Q3), `mcp-bridge` on a trigger.

## Description and interactions

Canon: `docs/canon/serve.md` ("Launcher", "Start", "Concurrency"), `crates/specengine-http/README.md` "Binary", `crates/specengine-cli/README.md`, `docs/canon/architecture.md#storage`, `#control`. `spec serve [DIR...] [--port N]`: roots, guard, lookup beside `spec` then on `PATH`, a probe that creates nothing, `exec` of `specengine-http`; the daemon itself takes the lock (Rust opens files close-on-exec) and publishes after its bind. Untouched: `spec gate`, the plugin (0.3.0), `specengine-mcp`, the fence, the routes, `ui/` code.

## Data

`docs/canon/serve.md` "Files" (`daemon.lock`, `daemon.json`: exactly `pid`, `port`, `url`, `projects` of `{slug, root}`, `started`; no token) and "Lines" (every line as built).

## Rules and edge cases

`docs/canon/serve.md` "Rules", "Known limits": the "already running" answer, the restart command (roots escaped, then shell-quoted, ` --port` when not 7777, an ephemeral one too), `daemon_text`, stale files, two starts, one daemon per data directory.

## Acceptance criteria

Scratch git byte copies of `fixtures/spec-a`, `-b`, a scratch `HOME`, `--port 0`. Tests: http `tests/serve.rs` (17), CLI `gate.rs`, `gate_wrapper.rs`; mutations 13 + 6, all red.

- [x] AC-S1 -- ADR-0039 (1366 B, `scope: [process]`, `canon: docs/canon/architecture.md#control`) supersedes both earlier gate decisions, each `status: superseded-by ADR-0039`; the canon cites it; `rg -n 'until .spec serve.' docs/canon CLAUDE.md` finds nothing.
- [x] AC-S2 -- 05 s1 (6), 07 s1.1, s2, s3, 08 s1, s2 reworded, the bridge's triggers named.
- [x] AC-D1 -- `d1_spec_serve_from_a_subdirectory_serves_the_found_root`. M: the cwd for the discovered root.
- [x] AC-D2 -- eval `build_graph` 35/35, `CLI_FORBIDDEN` and both graph tests unchanged. M: `tokio` in `specengine-cli`.
- [x] AC-D3 -- `d3_no_daemon_beside_spec_or_on_path_is_exit_2_creating_nothing`. M: the directory created first (the exit-0 half not covered).
- [x] AC-D4 -- `d4_a_second_start_under_the_same_home_exits_2_naming_the_first`, `d4_two_simultaneous_starts_leave_exactly_one_listening`. M: skip the lock.
- [x] AC-D5 -- `d5_a_daemon_serving_the_roots_is_answered_not_started_again`. M: always exec.
- [x] AC-D6 -- `d6_daemon_json_holds_exactly_the_pid_the_bound_port_and_the_roots`. M: a `token` key; the configured port.
- [x] AC-D7 -- `d7_after_kill_9_the_next_start_replaces_the_stale_file`. M: refuse whenever the file exists.
- [x] AC-D8 -- `d8_the_served_roots_stay_byte_identical_through_a_daemons_life`, `d8_a_data_directory_inside_a_served_root_is_refused_creating_nothing`. M: the file into the root.
- [x] AC-D9 -- `gate.rs` `serve_d9_the_gate_decides_alike_with_a_daemon_up_or_none`, `gate_wrapper.rs` `serve_d9_the_wrapper_decides_alike_with_a_daemon_up_or_none`. M: refuse without `daemon.json` (red in `gate.rs`; the wrapper's stays green).
- [x] AC-D10 -- `d10_usage_refusals_exit_2_creating_nothing`. M: accept `--json`.
- [x] AC-D11 -- `d11_the_fence_and_the_live_tail_hold_for_a_daemon_spec_serve_started`; the http suite 94/94, its "nothing written" checks now "nothing beyond the data directory's `daemon.lock` and `daemon.json`" (`common::beyond_daemon`, 15 sites); `plugin_files` 7/7, `.mcp.json` and 0.3.0 byte-identical. M: drop the Host check.
- [x] Rules -- `rules_a_held_lock_decides_and_readers_take_the_five_keys`, `rules_the_restart_command_quotes_and_a_renamed_slug_is_not_served`, `a_dirs_error_names_the_dir_as_given_then_the_clis_line`, `serve_is_never_told_to_pass_config_and_the_other_commands_still_are`, `texts_from_daemon_json_are_printed_escaped_on_one_line`, `a_lock_holder_that_ends_during_the_wait_lets_spec_serve_start`.
- [x] AC-C1, AC-C2 (review) -- accepted in iterations 1 and 2: `serve` is roots, guard, lookup, probe, exec; no route added; no domain literal.
- [ ] O1 (owner) -- in this repository `spec serve`, then `pnpm --dir ui dev`: the tree shows; a second `spec serve` prints "already running" and the URL; Ctrl-C ends the first; `spec serve` again starts clean.

Not covered: the Unix-only line, an exec failure, clap's `--port` refusal, raw controls in an unpublished reason (unreachable).

## Out of scope

`spec mcp`, `/mcp`; detach, idle exit, auto-start, a log file; registering into a live daemon; per-root `--config`; a watcher; the embedded UI; the `specengine` alias; any gate or plugin change. **`mcp-bridge`** (Q2, ADR-0019 unchanged) on a trigger: a watcher index, Phase 5's MCP staging forms, measured `Busy` exits, a cross-process reject-versus-apply lock; then a byte relay to the daemon's `SpecEngineServer`, never tools onto REST; plugin 0.4.0.

## Open

1. Q1-Q3 decided under the owner's pre-approval; the owner's explicit review of ADR-0039 is open (the superseded gate decision's folds into it); ADR-0038's stays open.
2. O1, the owner's manual check.
3. Code comments cite this spec's sections; they now hold pointers: a later sweep may cite `docs/canon/serve.md`.

## Implementation

Two iterations; the review accepted both.

| Module | What it does |
|---|---|
| CLI `src/serve.rs` (new) | the launcher's decision: roots, guard, lookup, probe, `Serve::{Running, Start}`; `DaemonFile`, `DaemonProject`, `read_daemon_file`, `unpublished_daemon`, `daemon_text`, `DAEMON_*` |
| CLI `src/main.rs` | `Command::Serve`, `run_serve`: the Unix check, `--json` refused, `exec` |
| CLI `src/project.rs` | `locate_hinted`, `discover_without_config_hint`: `serve`'s missing-config line without the `--config` hint |
| CLI `src/location.rs`, `src/lib.rs` | `checked_data_dir`, `prepared_data_dir` public; re-exports |
| http `src/daemon.rs` (new) | `lock` (`try_lock`, 500 ms), `publish` (`daemon.json.tmp`, rename) |
| http `src/start.rs`, `src/main.rs` | `served()` guards every root, then the data directory, the lock, the runtime, the bind, the file, the lines |

Deviations from the draft, all accepted: 1 `checked_data_dir` public too; 2 a DIR's error is `spec serve: DIR <given>: <CLI line>`, the inner `spec:` kept (as `specengine-http: --root <given>: spec: ...`), the no-DIR line without the `--config` hint; 3 the no-binary line names the full `current_exe` path and a literal `<clone>`; 4 extra exit-2 lines: `cannot open|lock <lock>` (both sides), `cannot run <path>`, `cannot write <daemon.json>` after the bind; 5 the Unix check before `--json`; 6 every text read from `daemon.json` (url, slug, root, the unpublished reason) printed through `daemon_text`, the restart roots escaped, then quoted, so a root holding a control character is named escaped (a limit); 7 a refusal after the lock (the port taken) leaves the data directory and an empty `daemon.lock`; 8 the probe retries the shared lock before each read of its 2 s wait; 9 a `--port 0` daemon's restart command names its ephemeral port. Limits (canon "Known limits"): the stale-file window between a winner's lock and its publish answers `already running` with a dead pid; `execvp` runs a non-binary through `/bin/sh` (ENOEXEC).
