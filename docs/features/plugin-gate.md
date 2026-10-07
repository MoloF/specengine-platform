---
class: spec
status: draft
scope: [plugin, crates/specengine-cli]
ref: plugin-gate analysis 2026-10-08 (HEAD b3da10c), working answers Q1-Q8 for the owner's review; 08 s2 Phase 2, Plugin
adrs: [ADR-0037]
---

# Plugin gate

## Why

The owner's one control point is task approval (ADR-0012), yet nothing stops an agent writing code before it: 08 s3 AC-4 is unmet, the skills only mention the task tools, and `#control` closed the hook "when the daemon is unavailable" while no daemon exists: read literally, the gate never opens. ADR-0037 (the owner's review open) supplements ADR-0006 until `spec serve`: `spec gate` in a `command` `PreToolUse` hook, reading the config, git and the queue itself; no verdict is closed. This slice builds it (`docs/canon/gate.md`, normative: read it first) and two skills walking an agent through a task, `analyze` and `implement`.

**Order** (08 s2): next after `decision-staging`; `plugin-roles` (stack-neutral roles, a plugin `/feature`) after it (Q8). `rust-developer`: core, store, CLI, `plugin/`; `test-engineer`: tests, `PINS`. No http, UI, CI, crate or dependency.

**Assumptions**: A1 plan before approval, claim after; A2 the binary stays `spec`, reading SQLite itself until `spec serve`; A3 a cooperative agent; A4 neither this repository's nor a pilot's `specengine.toml` has `[zones]` or `[gate]`; A5 the owner enables the plugin in user settings (not local scope, as the root README says): the hook runs in every project; A6 an implementer edits where it claimed (an MCP claim's `worktree` is relative to the server's directory, the session's).

## Description and interactions

The owner adds `[zones]` and `[gate]`, `observe` first (findings as events), then `selective`, and approves on a terminal as now. An `implement` agent runs `get_task`, `claim_task` (its role as the project names it, this worktree), writes, `report_run`, `complete_task`; at `done` the gate closes again. Without a claim a gated write is refused; the model reads why and the next step, reports it, never works around it. An `analyze` agent reads, asks, `submit_plan`s and claims nothing: its code writes are refused by design. Without a config or `[gate]`: exit 0 before git or the queue. `spec approve`, the owner's editor, git and the shell are never gated.

## Data

**Config**: `[zones] <name> = [<glob>...]`, `[gate] mode = "off|observe|selective"`, `zones = [<name>...]` (canon "Config", normative: rules, errors), parsed apart from the reads by core `gate.rs`; `ProjectConfig::from_toml` keeps name-checking both, so a bad `[gate]` fails no read.

**CLI** `spec gate` (07 s2):

```
spec gate --worktree DIR --path FILE [--json]   # DIR the session's directory; a relative FILE joins it
spec gate --stdin                                # the hook: Claude Code's PreToolUse JSON on stdin
```

- Exits: 0 pass; 2 refuse, no verdict, usage (clap's). Never 1 or another code: Claude Code proceeds on any code but 2. `--root`, `--config`, and `--json` with `--stdin`: usage.
- `--path`: stdout `pass: <reason>` on 0; on 2 stdout empty, the message on stderr. `--stdin`: stdout always empty; stderr the message on 2, on 0 only an `observe` record's failure `note:`.
- `--json` (with `--path`): one compact document on 0 and on 2, an exception to the CLI's "none on 2" (a refusal is a verdict); a crash prints none. Every key present, absent `null`:

```json
{"verdict":"refuse","reason":"no-claim","mode":"selective","path":"src/a.rs","root":"<root>","zone":"code","worktree":"<top>","branch":"feat-x","task":null,"tasks":[{"id":"T-0003","status":"ready","worktree":null,"branch":null}],"cause":null}
```

`verdict` `pass|refuse|unavailable`; `reason` `no-config|off|not-controlled|claimed|observed`, `no-claim`, `input|config|home|git|queue|deadline`; `path` root-relative once a root is found, else folded absolute; `zone` `null` for the config file or an uncontrolled path; `task` the lowest-numbered opening task; `tasks` on a refusal only (else `[]`): the repository's `ready` and `in_progress` tasks by number, at most 8, a claim's `worktree`, `branch`; `cause` the no-verdict message.

**Messages**, stderr, one line. Refuse: ``specengine gate (selective): src/a.rs is in the zone `code` and no in_progress task is claimed in <top> on `feat-x`. Tasks here: T-0003 ready; T-0004 in_progress, claimed in <w> on `main`. Claim a ready task in this worktree with claim_task, or ask the owner to approve one (`spec task approve T` on a terminal). Nothing was written.`` (the config file: `is the gate's config`; none: `Tasks here: none ready or in_progress.`). No verdict: ``specengine gate: no verdict (<cause>): the write is refused. <hint>``, hints `reinstall spec and the plugin from one commit` (a newer schema, an unknown subcommand), `fix specengine.toml:<line>`, `spec task show <T>` (a corrupt row, its column named).

**Library**: CLI `gate.rs` `gate(&Env, &GateRequest {input: GateInput::{Path {dir, path}, Hook(bytes)}, now, deadline}) -> GateOutcome` (the JSON's keys, `exit_code()`), never `CliError`. Core `gate.rs`: `GateConfig::from_toml(text) -> Result<Option<GateConfig>, ProjectError>`, `GateMode`, `controlled(rel) -> Option<Control {zone}>`, `fold(path)`, `opens(&[GateTask], &Place) -> Option<TaskId>`, all pure. Store `queue/gate.rs`: `open_for_gate(db, project, deadline) -> Result<Option<GateQueue>, QueueError>` (no file `None`; never creates or steps a schema; newer `SchemaTooNew`), `GateQueue::tasks(git_common_dir) -> (Vec<GateTask {id, status, claim}>, Vec<Corrupt>)`, `record_gate_observed(db, project, &GateObserved, now, deadline) -> Result<bool, QueueError>`, `EVENT_GATE_OBSERVED`. Git through `WorktreeGit`, reads only. `GATE_DEADLINE` 3 s from the start: each git call killed and SQLite's busy handler cut at what remains.

**Hook stdin** (Claude Code; AC-16 confirms): `{session_id, transcript_path, cwd, permission_mode, hook_event_name, tool_name, tool_input}`; read only `cwd` and `tool_input.file_path` (`NotebookEdit`: `.notebook_path`); not JSON, over 1 MiB, a relative `cwd`, no path -> `input`. It rests on: exit 2 refuses the call and hands stderr to the model; any other non-zero code and a timeout let it proceed; exit 0's stdout is read as hook JSON, where `"permissionDecision": "allow"` skips the permission prompt; `SessionStart`, `UserPromptSubmit` cannot refuse a tool call.

**Plugin** `plugin/specengine/` 0.1.6 -> **0.2.0** (MINOR), one `PINS` entry appended. `hooks/hooks.json`:

```json
{"hooks": {"PreToolUse": [{"matcher": "Edit|Write|MultiEdit|NotebookEdit",
  "hooks": [{"type": "command", "command": "sh \"${CLAUDE_PLUGIN_ROOT}/hooks/gate.sh\"", "timeout": 10}]}]}}
```

`hooks/gate.sh`, POSIX `sh`, reading no stdin itself:

```sh
if ! command -v spec >/dev/null 2>&1; then
  d=${CLAUDE_PROJECT_DIR:-$PWD}
  while :; do
    if [ -f "$d/specengine.toml" ]; then
      grep -q '^[[:space:]]*\[gate\]' "$d/specengine.toml" || exit 0
      echo "specengine gate: no verdict (spec is not on PATH): the write is refused. Put the directory of spec on the PATH Claude Code starts with." >&2
      exit 2
    fi
    case $d in /|.|'') exit 0 ;; esac
    d=$(dirname "$d")
  done
fi
spec gate --stdin >/dev/null && exit 0
exit 2
```

**Skills** `skills/{analyze,implement}/SKILL.md` under the root README's rules (descriptions of the five <= 1 200 B together, a body <= 4 096 B, no enum value or digit, the precedence sentence, all thirteen tools named across the five). Reserved names left: `prepare-task`, `round`.

| Skill | Use when | Teaches |
|---|---|---|
| `analyze` | asked to analyse or plan a task | `get_task` (package, staleness), `get_context_bundle` on its targets; a gap: `ask_question` or `report_discrepancy` with `task_id` and a working answer, keep going; `submit_plan` (`plan_md`, `criteria`, `affected_nodes`; replaces the last; before approval only); the owner approves on a terminal; never claim or edit code |
| `implement` | asked to implement a task | `get_task` (approved, or claimed here); `claim_task` once, `role` as the project names yours, `worktree` the one you edit; work only there, on its branch; a refused write: tell the owner the refusal and its next step, never work around it (no shell write, other worktree or config edit); `task_id` on questions; `report_run` (`outcome`, `summary`, `changed_files`), then `complete_task`; a subagent elsewhere is refused |

**Event** `gate.observed` (canon "Event"): payload keys `path, zone, worktree, branch` in this order; one `Immediate` transaction, `project` the slug; no schema step.

## Rules and edge cases

The canon's ("Rule", "Config", "Verdict", "Hook"), and:

- WHEN the path names a new file in new directories THEN the existing ancestor is canonicalised, the rest appended: `src/new/dir/b.rs` matches `src/**`.
- WHEN the volume is case-insensitive THEN the existing prefix takes its on-disk case (`SRC/a.rs` is `src/a.rs`); globs compare case-sensitively.
- WHEN a symlink leads out of the root THEN the canonical path decides, under its own config if any.
- WHEN `HEAD` is detached THEN nothing opens (`branch` `null`).
- WHEN a claim is compared THEN `same_dir`, `same_repository`, never strings: another clone of the slug opens nothing.
- WHEN any step passes the deadline THEN `deadline`: exit 2 (`observe` 0) within the deadline + 0.5 s.
- WHEN `--stdin` THEN stdout carries zero bytes whatever the outcome; no output names `permissionDecision`.
- WHEN the path is the config file and the mode is not `off` THEN it is controlled whatever the zones (Q7).
- WHEN `observe` cannot record THEN exit 0, one stderr `note:`.
- Risks: a hook timeout still opens (the deadline keeps below it); lockout (canon "Recovery"); one process per write in every project; an older `spec` closes (usage exit 2).

## Acceptance criteria

Scratch repositories (`git init`, `git worktree add`), a scratch `HOME`; CLI `gate.rs`, store `queue_gate.rs`, MCP `plugin_files.rs`, `plugin_skills.rs`; the wrapper run as `sh hooks/gate.sh` with a scratch `PATH`.

- [ ] AC-01 -- no `[gate]`: exit 0, `off`; an unknown `[gate]` key, `mode = "strict"`, `mode = "on"`, `min_files = 3`, an undeclared zone, a glob `src/[`, a zone `Code` -> exit 2 naming `specengine.toml:<line>`, while `spec show <ID>` and MCP `get_node` answer (M: absent read as `selective`; a bad `[gate]` fails `spec show`).
- [ ] AC-02 -- `selective`, a controlled path, the task `draft`, `review`, `changes_requested`, `ready`, `done`, `cancelled` -> 2 `no-claim`; claimed, run open -> 0 `claimed`; after `report` -> 0 (M: `ready` opens; an open run required).
- [ ] AC-03 -- claimed in W1: W2 of the repository closed; W1 after `git switch -c other` closed; a second clone, same slug and branch, closed (M: the repository alone compared; the branch ignored; matched by slug).
- [ ] AC-04 -- outside every zone, under no `specengine.toml`, a project without `[gate]` -> 0 with the data directory `chmod 000` (M: the queue opened before the zone match).
- [ ] AC-05 -- `code = ["src/**"]`, no claim: `src/../src/a.rs`, the absolute path, `link/a.rs` (`link -> src`), absent `src/new/dir/b.rs`, `SRC/a.rs` on a case-insensitive volume (else skipped, noted), `specengine.toml` -> each 2 (M: the raw `file_path` matched).
- [ ] AC-06 -- a bad `[gate]`, `HOME` unset, a schema-6 queue, a corrupt `claim`, the root outside git, bad stdin, the queue under an exclusive lock -> 2 in under 4 s, stderr `specengine gate: no verdict (` and the cause, never `no in_progress task` (M: one exits 0; an unbounded busy wait).
- [ ] AC-07 -- wrapper: a fake `spec` exiting 1, 101 or killed by `SIGKILL` -> 2; one printing on stdout -> stdout empty; no `spec`: 2 with the `PATH` hint under a `[gate]` line at or above `CLAUDE_PROJECT_DIR`, else 0 (M: the code passed through).
- [ ] AC-08 -- `--stdin` on pass, refuse, no verdict: zero stdout bytes; no output of `spec gate` carries `permissionDecision` (M: the JSON printed in hook mode; `allow` on a pass).
- [ ] AC-09 -- a claimed task passes with an open question, a discrepancy, a staged decision, `stale` true, a red `spec check` (M: `stale` read).
- [ ] AC-10 -- `observe`: an unclaimed controlled write -> 0 and one `gate.observed` with the four keys; claimed -> 0, none; `observe`, `off` pass with the data directory unreadable (M: `observe` exits 2; no event).
- [ ] AC-11 -- verdicts identical with every `.md` unreadable; the index database untouched (absent stays absent, else its mtime kept); the median per call over 50 calls on pilot A recorded at shipping (M: the reads' `update` called).
- [ ] AC-12 -- a refusal's stderr and JSON name the root-relative path, zone, mode, `claim_task`, `spec task approve`, task IDs with status; a unique title and goal appear nowhere (M: a title echoed).
- [ ] AC-13 -- `hooks.json`: one event `PreToolUse`, one entry, `type` `command`, the matcher fully matching `Edit`, `Write`, `MultiEdit`, `NotebookEdit`, `timeout` above 3, `sh` on `${CLAUDE_PLUGIN_ROOT}/hooks/gate.sh` (M: `"type": "http"`; `Write` dropped).
- [ ] AC-14 -- a scratch `fixtures/spec-a` under `selective`, hook JSON through the wrapper: closed in `draft`, `ready`; open after `claim`, after `report`; closed after `complete` (M: open after `complete`).
- [ ] AC-15 -- five skills keep the README's rules; `implement` names `claim_task`, `report_run`, `complete_task`; `analyze` `submit_plan`; files = the README's list; 0.2.0; `PINS` appended; `claude plugin validate --strict` (owner); the P2-3 scan (07 s1.2) covers the gate sources and `plugin/**` (M: `claim_task` dropped; `PINS` unchanged; a `"code"` zone in core).
- [ ] AC-16 -- owner, Claude Code version noted: in a `selective` scratch project an unclaimed Edit is refused and its stderr reaches the model, also in a subagent and auto mode; an unrelated project passes; noted: the timeout, `MultiEdit`, `NotebookEdit`, `CLAUDE_PROJECT_DIR`, `/specengine:analyze`, `/specengine:implement` (M: a bare `spec gate --stdin` with `spec` off `PATH` lets the Edit through).

## Out of scope

`spec serve`, the `spec mcp` bridge, auto-start; the other 07 s4 hooks; Bash and other servers' writes (07 s4's limit); `strict`, `min_files`, `contours`; a write checked against the plan; stack-neutral roles, a plugin `/feature` (`plugin-roles`, after the owner checks name resolution, 06 s8); profiles; a UI view of `gate.observed`.

## Open

1. **ADR-0037** rests on working answers Q1-Q3, Q7: the owner reviews it explicitly; another decision supersedes it before shipping.
2. AC-16, the owner's manual check.
3. Working answers: Q1 a new ADR supplementing ADR-0006, `#control` reworded; Q2 only an `in_progress` task claimed in that worktree and branch opens; Q3 no `[gate]` is off; Q4 `spec` missing: the wrapper's 2 with a `PATH` hint under a `[gate]` line at or above `CLAUDE_PROJECT_DIR`, else 0; Q5 an `observe` finding is `gate.observed`, best-effort; Q6 the task prompts are the skills `analyze`, `implement`; Q7 the config file is gated while the gate is on; Q8 roles and `/feature` next.

## Implementation

Not built. Docs at shipping, within caps (crate READMEs pay by moving detail into `gate.md`): `gate.md` drops "Not built yet", takes the CLI contract; 07 s1.4 (two skills; reserved `prepare-task`, `round`), s2 (`spec gate ... | --stdin`, exit 0 or 2), s4 (the built entry, the wrapper; other hooks later), s5 (three modes; `strict`, `min_files`, `contours` later); 06 s3.5 items 2-3; 08 s2, s3 AC-4; root README "Claude Code plugin" (hook, wrapper, two skill rows, "Files" + `hooks/hooks.json`, `hooks/gate.sh`, two `SKILL.md`, "No hooks" dropped, reserved names, 0.2.0, runs wherever enabled); core README (`GateConfig`), CLI README (`spec gate`, JSON on 2), store README (`open_for_gate`, `record_gate_observed`); `tasks.md` "Store", `proposal-queue.md` "States and events" (+ `gate.observed`); http README "Types" (streamed); `ui/README.md` "Contract seam": `QUEUE_EVENT_TYPES` stays 16, no screen shows a finding yet; `CLAUDE.md` State.
