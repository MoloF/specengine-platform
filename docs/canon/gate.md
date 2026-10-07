---
class: canon
tier: 2
scope: [crates/specengine-cli, plugin]
owner: owner
reviewed: 2026-10-08
---

# Task gate: the write hook

The decided behaviour of ADR-0037, supplementing ADR-0006 until `spec serve` (`docs/canon/architecture.md#control`). **Not built yet: the `plugin-gate` slice** (`docs/features/plugin-gate.md`). Until it ships no hook runs: `[zones]` and `[gate]` are only name-checked (`crates/specengine-core/README.md`), and approval binds only an agent that claims before it writes.

## Rule

- One question: does a task the owner approved cover this agent write? A write to a controlled path opens only when the file's worktree (the git top of its nearest existing directory, compared as a directory), on its current branch, holds an `in_progress` task of the same git common dir claimed there (`tasks.md` "Place"). A reported run keeps it open; `draft`, `review`, `changes_requested`, `ready`, `done`, `cancelled` do not.
- Nothing else is read: no proposal, `stale`, `affected_nodes`, check or drift (ADR-0012). A plan's scope is not checked: files map to nodes only in Phase 3.
- No verdict is closed: exit 2 naming the cause. A pass is exit 0 with an empty stdout, never `allow`: Claude Code's permission prompts stay.
- Threat model: a cooperative agent that forgets approval, not an adversary (as ADR-0034). Bash, other MCP servers, settings, git, `spec` itself and the owner's editor never pass through it (07 s4).

## Config

```toml
[zones]                # name -> globs, root-relative, the dialect of [paths] exclude
code  = ["src/**", "Cargo.toml"]
tests = ["tests/**"]

[gate]
mode  = "selective"    # off | observe | selective
zones = ["code"]       # the controlled zones; required unless off
```

- **Controlled**: a path inside the project root matching a glob of a listed zone, and the config file itself while the mode is not `off`. Spec files only where a zone names them.
- **Modes**: no `[gate]` table is `off` (`[zones]` alone is unread); `off` passes, reading nothing more; `observe` passes every write and records an uncovered one (`gate.observed`); `selective` refuses an uncovered one.
- **Errors**, exit 2 naming `specengine.toml:<line>`; the reads never see them (`spec show`, MCP keep answering): an unknown key of `[gate]` (`min_files`, `contours`: not built); a mode outside the three (`strict`: not built); `mode` missing; a listed zone undeclared or listed twice; `zones` missing or empty unless `off`; a zone name not of `[a-z0-9_-]`, 1-64 bytes; a zone with no glob, an empty or invalid one. A file that is not TOML: no verdict when one of its lines starts `[gate]`, else off.

## Verdict

Each step runs only when the one before decided nothing:

1. **Input**: the hook's JSON or `--path`. The path joined to the session's directory when relative, `.` and `..` folded, its nearest existing ancestor canonicalised (symlinks, a case-insensitive volume's case), the rest appended.
2. **Config**: the first `specengine.toml` walking up from the path's nearest existing directory. None: pass.
3. **Table**: `[gate]` absent or `off`: pass. Errors: no verdict.
4. **Controlled?** Its root-relative path matching no listed zone and not the config: pass.
5. **Git**, reads only: the worktree top, common dir, branch (detached: none, nothing opens).
6. **Queue**: `<slug>.db` under `HOME`'s data directory (`crates/specengine-cli/README.md`), never created or migrated; absent, or a schema without tasks: no task. A well-formed opening row passes; else a corrupt row of the repository: no verdict; else `selective` refuses, `observe` passes and records.

**No verdict** (exit 2, its cause named, never the refusal's text): a config error; `HOME` unset or relative; git missing, failing or no worktree; a queue of a newer schema; a corrupt row; the queue locked or the run past the deadline (3 s, below the hook's 10 s timeout: a timeout lets the write through); unreadable input; `spec` missing or failing (the wrapper). `observe` turns each into a pass past step 3. The index is never read or refreshed.

## Hook

The plugin's `hooks/hooks.json`: one `PreToolUse` entry, `type: "command"`, matcher `Edit|Write|MultiEdit|NotebookEdit`, `timeout` 10, running the POSIX `sh` wrapper `hooks/gate.sh`: `spec gate --stdin`, its stdout discarded; 0 stays 0, any other code (1, 101, a signal) becomes 2. `spec` not on `PATH`: 2 with a `PATH` hint when the first `specengine.toml` at or above `CLAUDE_PROJECT_DIR` has a `[gate]` line, else 0. Without a config the binary exits before git or the queue: every Edit of every project with the plugin pays one process start.

A refusal's stderr reaches the model: the path, zone and mode; the next step (`claim_task` a `ready` task in this worktree, or the owner's `spec task approve` on a terminal); the repository's `ready` and `in_progress` task IDs with their status and claim place, never a title or goal.

## Event

`gate.observed` `{"path":"src/a.rs","zone":"code","worktree":"<top>","branch":"main"}`: `path` root-relative, `zone` `null` for the config file, `branch` `null` when detached. One per uncovered write under `observe`, in the slug's queue, best-effort: no database, a lock past the deadline, a schema not the build's: none, still exit 0. A dump keeps it as any event (`queue-backup.md`); `specengine-http` streams it; the UI's closed list does not take it.

## Recovery

Locked out: claim an approved task in this worktree; set `mode = "off"` in an editor; or disable the plugin. An older `spec` without `gate` refuses (usage, exit 2): reinstall `spec` and the plugin from one commit.
