---
class: canon
tier: 2
scope: [crates/specengine-cli, plugin]
owner: owner
reviewed: 2026-10-08
---

# Task gate: the write hook

ADR-0039 (`docs/canon/architecture.md#control`), as built by `plugin-gate`: the plugin's hook runs `spec gate` before every agent file write, with or without a daemon. Code: CLI `gate.rs` (`gate`, `read_hook_input`, `gate_session`); core `gate.rs`, pure (`GateConfig`, `controlled`, `opens`, `fold`, `mentions_gate`); store `queue/gate.rs` (`open_for_gate`, `GateQueue::tasks`, `record_gate_observed`), `WorktreeGit::new_until`.

## Rule

- One question: does a task the owner approved cover this agent write? A write to a controlled path opens only when the file's worktree (the git top of its nearest existing directory, compared as a directory), on its current branch, holds an `in_progress` task of the same git common dir claimed there (`tasks.md` "Place"). A reported run keeps it open; `draft`, `review`, `changes_requested`, `ready`, `done`, `cancelled` do not. A submodule's file is refused even when claimed in the superproject: its git top differs.
- Nothing else is read: no proposal, `stale`, `affected_nodes`, check, drift, index (ADR-0012) or daemon (ADR-0039), and no title or goal is printed. A plan's scope is not checked: files map to nodes only in Phase 3.
- No verdict is closed: exit 2 naming the cause. A pass is exit 0 with an empty stdout, never `allow`: Claude Code's permission prompts stay.
- Threat model: a cooperative agent that forgets approval, not an adversary (as ADR-0034). Bash, other MCP servers, settings, git, `spec` itself and the owner's editor never pass through it (07 s4).

## Config

```toml
[zones]                # name -> globs, root-relative
code  = ["src/**", "Cargo.toml"]
tests = ["tests/**"]

[gate]
mode  = "selective"    # off | observe | selective
zones = ["code"]       # the controlled zones; required unless off
```

Read apart (core `GateConfig::from_toml`); `ProjectConfig` only name-checks both tables, so `spec show` and MCP answer whatever `[gate]` holds.

- **Modes**: no `[gate]` table is `off`; `off` reads only `[gate]`'s keys and `mode` (`[zones]` unread), so it always recovers; `observe` passes every write and records an uncovered one (`gate.observed`); `selective` refuses an uncovered one.
- **Globs**: the dialect of `[paths]` exclude, relative and clean: `*`, `**`, `?`, every other character literal, case-sensitive; `[`, `]`, `{`, `}`, `\` and control characters refused.
- **Errors**, no verdict naming `<root>/specengine.toml:<line>`, hint `Fix <path>:<line>.`: an unknown `[gate]` key (`min_files`, `contours`: not built); a mode missing or outside the three (`strict`: not built); `zones` not a list of strings; unless `off`: `zones` missing or empty, a zone listed twice or undeclared, a declared zone not an array, named outside `[a-z0-9_-]` 1-64 bytes, with no glob or a bad one, `[project] slug` missing or invalid (it names the queue). An unreadable config: no verdict. A file that is not TOML: no verdict when a line names the table (core `mentions_gate`: blanks, an optional `[`, `gate` bare or quoted, then `]`, `.` or `=`: `[ gate ]`, `["gate"]`, `[gate.x]`, `gate.mode =`; not `[gates]`, `gateway =`, `# [gate]`), else off.

## Controlled paths

Under a root whose mode is not `off`: a root-relative path matching a glob of a listed zone (spec files only where a zone names them); and any path whose last component is `specengine.toml` in any ASCII case, at any depth, the root's own included (`zone` `null`, "would be a project config"): a new config below the root would re-root zone files under one without `[gate]`. An existing nested config still governs its subtree.

**Normalisation**: a leading `~` or `~/` expanded against `HOME` (`~user` kept as written); joined to the session's directory when relative (`--worktree`, the hook's `cwd`); `.` and `..` folded; the nearest existing ancestor canonicalised (symlinks, a case-insensitive volume's case: `SRC/a.rs` is `src/a.rs`), the rest appended (`src/new/dir/b.rs` matches `src/**`).

**Two candidates**, each judged on its own (config, table, zone): the path as written (its directory canonical, its last component kept) and the canonical path, a dangling final link followed by `read_link` for at most 16 hops (`LINK_HOPS`; a longer chain or a loop: no verdict under a gated root, a note under `observe`, ungated a pass). Controlled when either is: `notes/x.rs -> ../src/new.rs` is `src/new.rs`; a zone file `src/out.rs` linking outside stays `src/out.rs`. A closing no verdict wins, then a controlled candidate (git runs from its nearest existing directory), then an `observe` no verdict, then the canonical candidate's pass.

## Verdict

Each step runs only when the one before decided nothing, all within `GATE_DEADLINE` (3 s) of the start:

1. **Input**: `--path`, or the hook's JSON. One naming no file (not JSON, no path, NUL, a relative `cwd` for a relative path, `~` without `HOME`, stdin not closed in time) is decided by the **session's project** (`CLAUDE_PROJECT_DIR`, else the current directory; neither: pass): no config, no `[gate]` or `off` pass; `observe` passes with a note; `selective` or a broken `[gate]`: no verdict naming both causes. Input never refuses a write outside a gated project.
2. **Config**: per candidate, the first `specengine.toml` walking up from its nearest existing directory. None: pass.
3. **Table**: `[gate]` absent or `off`: pass. An error: no verdict.
4. **Controlled?** No: pass.
5. **Git**, reads only, each call killed at the deadline (`WorktreeGit::new_until`): the worktree top, common dir, branch (detached: none, nothing opens).
6. **Queue**: `<slug>.db` under `HOME`'s data directory (`crates/specengine-cli/README.md` "Database"), read-only (`open_for_gate`): never created, migrated or checkpointed; absent, or a schema without tasks: no task. An opening task passes; else a corrupt row of the repository (id, status or claim undecodable, `in_progress` with no claim, no common dir): no verdict, hint `spec task show T`; else `selective` refuses, `observe` passes and records.

**No verdict**, exit 2, `specengine gate: no verdict (<cause>): the write is refused.` + a hint, never the refusal's text: a config error or an unreadable config; `HOME` unset or relative; git missing, failing or no worktree; a queue of a newer schema (hint: reinstall `spec` and the plugin from one commit); a corrupt row; a lock or any step past the deadline; a dangling chain; a usage error (an unknown argument: the reinstall hint); a panic; `spec` missing, failing or stalled (the wrapper). Past step 3 `observe` turns each into a pass with one `note:`. **Backstop**: a thread at the deadline + 0.4 s and the panic hook exit by the stage reached (`GateProgress`): open 0; `observe` 0 with a note; closed 2; nothing read yet: the session's project (`gate_session`).

## CLI

`spec gate --worktree DIR --path FILE [--json]` (a terminal), `spec gate --stdin` (the hook); run apart, before the environment is read. Exits 0 or 2 only: Claude Code proceeds on any other code. `--root`, `--config`, `--json` with `--stdin`: a usage no verdict. `--path`: stdout `pass: <reason>` on 0. `--json`: one compact document on 0 and on 2 (a refusal is a verdict), every key present: `verdict` `pass|refuse|unavailable`; `reason` `no-config|off|not-controlled|claimed|observed|no-claim|input|config|home|git|queue|deadline`; `mode` (`off` when no table); `path` (root-relative once a root is found); `root`, `zone`, `worktree`, `branch`; `task` (the lowest opening); `tasks` `[{id, status, worktree, branch}]` on a refusal; `cause`. `--stdin`: zero stdout bytes whatever the outcome; stderr the message on 2, on 0 only `observe` notes.

A refusal, one line: ``specengine gate (selective): src/a.rs is in the zone `code` and no in_progress task is claimed in <top> on `feat-x`. Tasks here: T-0003 ready; T-0004 in_progress, claimed in <w> on `main`. Claim a ready task in this worktree with claim_task, or ask the owner to approve one (`spec task approve T` on a terminal). Nothing was written.`` The tasks: the repository's `ready` and `in_progress` by number, at most 8, then `and N more`; none: `none ready or in_progress`; detached: `nothing opens on the detached HEAD of <top> (a claim names a branch)`.

## Hook

`hooks/hooks.json`: one `PreToolUse` entry, matcher `Edit|Write|MultiEdit|NotebookEdit`, `type: "command"`, `sh "${CLAUDE_PLUGIN_ROOT}/hooks/gate.sh"`, `timeout` 10. It runs wherever the plugin is enabled. `spec gate --stdin` reads stdin to EOF, no cap, on a thread bound by the deadline, keeping only `cwd` and `tool_input.file_path` (else `.notebook_path`; not a string: absent); a `Write`'s content streams by unheld; after a parse error the rest is still drained (no closed pipe for Claude Code). Without a config it exits before git or the queue.

The wrapper `hooks/gate.sh`, POSIX `sh`, reading no stdin itself, makes every failure 2:

- `spec` in the background, stdin passed on, stdout discarded, stderr to a `mktemp` file replayed at the end (a stalled `spec` holds no Claude Code pipe); the wrapper's messages on fd 4.
- **Limit** 7 s, below the hook's 10: a watchdog `sh -c` sleeps 1 s at a time, checks each second that the wrapper is still its parent, then sends `USR1`; the trap kills `spec` (`KILL`), exits 2 `(spec gave no answer within 7 s)`. A gone wrapper is never signalled; its watchdog ends within a second.
- Traps `HUP`, `INT`, `TERM` (Claude Code stopping the hook): `spec` and the watchdog killed, the temp file removed, 2 `(the hook was stopped)`; `PIPE` ignored.
- 0 stays 0; 2 keeps `spec`'s stderr; any other code (1, 101, a signal) is 2 `(spec ended with status N)`.
- **Version skew**: `spec` not on `PATH` (hint: its directory on the `PATH` Claude Code starts with), or one without `gate` (`unrecognized subcommand`; hint: reinstall both from one commit): the first `specengine.toml` at or above `CLAUDE_PROJECT_DIR` (else `$PWD`) decides: a line matching `mentions_gate` (`grep -E`) or an unreadable file 2, else (or none) 0.

## Event

`gate.observed` `{"path":"src/a.rs","zone":"code","worktree":"<top>","branch":"main"}`, keys in this order: `path` root-relative, `zone` `null` for a config, `branch` `null` when detached. One per uncovered write under `observe`, in the slug's queue, one `Immediate` transaction (`record_gate_observed`), only at the build's schema, no schema step; best-effort: no database, a lock past the deadline, another schema: none, exit 0 with a `note:`. A dump keeps it as any event (`queue-backup.md`); `specengine-http` streams it; the UI's closed list (`QUEUE_EVENT_TYPES`, 16) does not take it.

## Recovery

Locked out: claim an approved task in this worktree; edit `[gate]` in the owner's editor (an agent's edit of a config is gated); set `mode = "off"`; or disable the plugin. An unreadable `specengine.toml` is no verdict even without `[gate]`: fix its permissions. An older `spec`: reinstall `spec` and the plugin from one commit. A `SIGKILL`ed wrapper leaves its temp file (`$TMPDIR/specengine-gate.*`); `spec` ends by its backstop.

## Known limits

A hook timeout (a stalled `sh` past 10 s, or a shorter timeout set in Claude Code) lets the write through. The first gated write after installing `spec` on macOS may wait on the system's first-run scan and be refused at 7 s; a retry passes. A queue read may create or touch `<slug>.db-wal`, `-shm`. Cost per write (debug build): controlled 30-40 ms, uncontrolled 6 ms, the wrapper 7 ms more. The owner's own Claude Code sessions are gated too.
