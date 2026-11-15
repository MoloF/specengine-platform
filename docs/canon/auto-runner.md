---
class: canon
tier: 2
scope: [plugin]
owner: owner
reviewed: 2026-11-15
---

# Auto runner: the plugin's `autopilot.sh` drives a pack overnight

ADR-0081 keeps the agent outside `spec`: `plugin/specengine/scripts/autopilot.sh`, POSIX `sh` (tested under `sh`, dash and bash), built by `auto-runner` (RM-0123). It needs `spec`, `claude`, `git` and `jq` on `PATH` (else exit 2 `autopilot: <tool> is required on PATH`, before anything), loops `spec auto next` and does what it says; every choice is `spec`'s (`spec-auto.md`, `spec-auto-land.md`), the rule's and reasons `auto-mode.md`'s. The skill is unchanged: `/specengine:feature` ends after the plan (`plugin.md` "Routing and legs"), so the runner calls it twice per task, `spec auto approve` between.

## Legs

| `next` says | Where | What |
|---|---|---|
| `plan` | the project root, main worktree | `claude -p "/specengine:feature <T>"`; the task must end in `review` |
| `plan-line` | the same | `SPECENGINE_AUTO_LINE=<pack>/<n>` and the line's text as the prompt: the session's `spec task new` binds it; its task must end in `review` |
| `approve` | `<worktrees>/<T>`, made first | `git -C <root> worktree add -b <branch> <worktrees>/<T> <base>` when absent (an existing branch reused), then `spec auto approve` with `--root <worktrees>/<T>/<root_rel>` |
| `implement` | `<worktrees>/<T>/<root_rel>` | `claude -p "/specengine:feature <T>"`: claim, roles, report, complete; the task must end `done` |
| `land` | the main worktree | `spec auto land`; on `landed`, `git worktree remove <worktrees>/<T>` (a refusal logged, the folder kept) |

Each `spec` call takes `--root=<dir>` and every free-text value as `--flag=value`, IDs after `--`. A leg, stdin `/dev/null`, its own process group (`setsid`, else perl's `setpgrp`; neither: the runner signals the leg's process tree):

```sh
CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=<bg-wait-ms> [SPECENGINE_AUTO_LINE=<pack>/<n>] \
claude -p "<prompt>" --output-format stream-json --verbose --permission-prompts none \
  --max-turns <N> [--max-budget-usd <X>] [--model <M>] [--permission-mode acceptEdits]
```

stdout to `<log>/<item>-<leg>-<k>.jsonl` (`<item>` the task or `<pack>-<n>`, `<k>` from 1), stderr beside it `.err`. Never `--dangerously-skip-permissions`, `--allow-dangerously-skip-permissions` or `bypassPermissions`; `--permission-mode acceptEdits` only under `--accept-edits`; no free claude arguments. Trust and allow rules are the main checkout's (`.claude/settings.json`), so any other tool use is a denial the result lists.

## Flags

`--root DIR` (the current directory: the project root in the main worktree); `--lanes N` (2; 1-16, legs at once); `--by-department` (a lane per `next`'s `lane`, one leg each, within `--lanes`); `--until TIME` (`YYYY-MM-DDTHH:MM:SSZ`: no leg starts at or after it); `--max-tasks N` (implement legs this run); `--max-turns N` (200); `--max-budget-usd X`, `--model M` (passed when given); `--accept-edits`; `--poll S` (30: between `next` calls when nothing changed); `--pause S` (900: a usage limit with no reset time); `--leg-timeout S` (14 400: `TERM`, `KILL` 30 s later, held `agent-error` `timed out after <S> s`); `--bg-wait-ms MS` (3 600 000, so `-p` keeps the skill's subagents past 10 minutes); `--worktrees DIR` (`<parent of the main worktree>/<its name>.auto`, outside the repository); `--log DIR` (`<worktrees>/.log`); `--dry-run` (one `next`, then `next:`, `cwd:`, `env:` and `argv:` lines; runs nothing; exit 0). A bad flag or value: exit 2 `autopilot: <why>`, nothing run.

## Loop

1. Reap finished legs, classifying each; sweep a finished leg's leftover processes.
2. In a usage-limit pause, start nothing until it ends.
3. `spec auto next --slots <free> [--busy <item>]... [--skip-lane <lane>]...`: each running leg's item busy, and each item offered to implement past `--max-tasks`; under `--by-department` each lane running a leg or stopped skipped. `next` failing: logged, retried after `--poll`; three in a row: exit 1.
4. `stop`: wait for running legs, exit 0 `autopilot: stopped (<reason>): <why>`. `land`, `approve`: at once, then loop. `implement`, `plan`, `plan-line`: a leg in the background, unless `--until` passed (exit 0 `autopilot: until <TIME>` with nothing running) or `--max-tasks` reached (busy from then on; exit 0 `autopilot: max-tasks N reached` once nothing runs); a failed `git worktree add` holds `agent-error`. `wait`: sleep `--poll`. `done`: with nothing running exit 0 `autopilot: done`.
5. A `land` refused by the lock is retried after `--poll`; 10 in a row on a task: exit 1 `autopilot: spec auto land T found the lock held 10 times in a row: …`; past `--until` with nothing running a refusal ends the run, exit 0. A land or approve failing three times with no hold: exit 1. One exiting 1 with a hold is finished by `spec`'s own event. The land runs shielded from `INT`.

The stop file is `spec`'s: `next` answers `stop` within one `--poll`. `INT`, `TERM`: each running leg's group sent `TERM` (`KILL` 30 s on), waited for, exit 130 or 143; items keep their state (a killed implement leg leaves its task claimed, and the next run's leg resumes it).

## Classifying

From the leg's stream when it ends, never `claude`'s exit code, in order:
1. **Usage limit**: `{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":<unix s>}}`: a pause until `resetsAt` + 60 s (none: `--pause`), no failure, no hold; the same leg runs again after it.
2. **No MCP**: the first `system`/`init` line lacks an `mcp_servers` entry named `specengine` or ending `:specengine` with `status` `connected` (or no init line): `no-mcp`, `the specengine MCP server was not connected`.
3. **Result**: the last `{"type":"result"}`; none: `agent-error`, `no result; claude exited <code>`.
4. `permission_denials` not empty: `denied-tool`, `denied: <tool_name>[, …]` (distinct, in order, 8 at most).
5. `error_max_turns`: `max-turns`; `error_max_budget_usd`: `max-budget`; another `error_*`, or `is_error`: `agent-error`, `<subtype>: <terminal_reason or ->: <result, 200 bytes>`.
6. **The queue**: after plan the task is `review`, else `not-planned` `<T> is <status> after the plan leg`; after plan-line the line is bound to a task in `review`, else `not-planned` `<pack>/<n> is unbound after the plan leg` or `<T> is <status> after the plan leg`; after implement `done`, else `not-done` `<status>; run <n> <outcome>`, with no run `<status>; no run`.

A missing working directory holds `agent-error`. A hold is `spec auto hold --reason=<r> --note=<note> -- <item>`, its note cut to 1 000 bytes, controls made spaces. A failing leg (steps 2-6) counts against its lane, a passing one resets it; two in a row stop the lane. Without `--by-department` the one lane is `all`, so the run ends after the running legs, exit 1 `autopilot: two failures in a row on lane all`; with it, it goes on while a lane is open.

## Log

stderr and `<log>/autopilot.log`: the runner's `autopilot: …` lines and one line per act, `<UTC time> <item> <leg or act> <outcome> <detail>`, outcome `start`, `ok`, `held <reason>`, `paused <until>`, `landed <sha12>`, `skipped`; controls stripped. The leg streams stay for the owner.

## Rules

- It writes only `<worktrees>`, `<log>`, worktree registrations and task branches (through git) and calls `spec`; it never commits, merges, pushes or edits the project's files.
- Overlapping tasks never run at once: `next` alone orders, every running item `--busy`.
- A test result is the agent's claim in its run report; `verified` and `spec.lock` stay the owner's (ADR-0041, ADR-0044). SpecEngine meters no cost: the caps and the owner's account bound it.

## Known limits

- The stream shapes are Claude Code's documentation's (build 2.1.325), until the owner's live run (RN0) records them.
- The process-tree fallback cannot reach a process already orphaned to init, and a reused pid could in theory be hit within the 30 s sweep.

## Tests

CLI `tests/autopilot.rs` (RN1-RN8, a `claude` shim on `PATH`, `tests/auto_common/runner.rs`).
