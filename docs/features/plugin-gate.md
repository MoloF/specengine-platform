---
class: spec
status: shipped
scope: [plugin, crates/specengine-cli]
ref: plugin-gate analysis 2026-10-08 (HEAD b3da10c), working answers Q1-Q8 for the owner's review; 08 s2 Phase 2, Plugin
shipped: 2026-10-08
adrs: [ADR-0037]
---

# Plugin gate

## Why

The owner's one control point is task approval (ADR-0012), yet nothing stopped an agent writing code before it (08 s3 AC-4), and ADR-0006 closed the hook "when the daemon is unavailable" while no daemon exists: read literally, the gate never opened. ADR-0037 supplements ADR-0006 until `spec serve`: `spec gate` in a `command` `PreToolUse` hook reads the config, git and the queue itself; no verdict is closed. This slice built it and two skills walking an agent through a task, `analyze` and `implement`. Next: `plugin-roles` (stack-neutral roles, a plugin `/feature`; Q8).

## Description and interactions

The owner adds `[zones]` and `[gate]`, `observe` first (findings as `gate.observed`), then `selective`, and approves on a terminal as before. An `implement` agent runs `get_task`, `claim_task` (its role, the top of the git worktree it edits), writes, `report_run`, `complete_task`; at `done` the gate closes again. An unclaimed gated write is refused: the model reads why and the next step, tells the owner, never works around it. An `analyze` agent reads, asks, `submit_plan`s and claims nothing. Without a config or `[gate]`: exit 0 before git or the queue. `spec task approve`, the owner's editor, git and the shell are never gated.

## Data

Canon: `docs/canon/gate.md` ("Config", "Controlled paths", "Verdict", "CLI", "Hook", "Event", "Recovery", "Known limits"). The built signatures (deviation 2):

- core `gate.rs`, pure: `GateConfig::from_toml(&str) -> Result<Option<GateConfig {mode, zones, slug}>, ProjectError>`; `controlled(&self, rel, config_name) -> Option<Control {zone: Option<String>}>`; `GateMode {Off, Observe, Selective}`; `fold(&Path) -> PathBuf`; `opens(&[GateTask {id, status, claim}], branch: Option<&str>, here: impl Fn(&str) -> bool) -> Option<&GateTask>`; `mentions_gate(&str) -> bool`.
- store `queue/gate.rs`: `open_for_gate(db, project, deadline) -> Result<Option<GateQueue>, QueueError>`; `GateQueue::tasks(git_common_dir) -> Result<(Vec<GateTask>, Vec<UnreadableTask>), QueueError>`; `record_gate_observed(db, project, &GateObserved {path, zone, worktree, branch}, now, deadline) -> Result<bool, QueueError>`; `EVENT_GATE_OBSERVED`. `same_dir` public; `WorktreeGit::new_until(dir, &GitEnv, deadline)`, `top_and_common_dir()`, `GitError::Deadline`.
- CLI `gate.rs`: `gate(&Env, &GateRequest {input, session, git: GitEnv, now, deadline, progress}) -> GateOutcome` (never `CliError`); `GateInput::{Path {dir, path}, Hook(HookInput {cwd, file}), Unread {cause, deadline}}`; `read_hook_input(reader, deadline) -> GateInput`; `gate_session(session, cause, &GateProgress) -> GateOutcome`; `GateProgress`, `GateStage {Unknown, Open, Observe, Closed}`; `GATE_DEADLINE` 3 s, `GATE_TASKS_MAX` 8, `LINK_HOPS` 16.
- plugin 0.1.6 -> 0.2.0, `PINS` `6b6fa534...`: `hooks/hooks.json` (canon "Hook"); skills `analyze`, `implement` (root README); descriptions of the five 1 149 B, bodies 2 090 and 2 280 B. `hooks/gate.sh` as built, in outline:

```sh
limit=7; gate_line='<mentions_gate as an ERE>'
exec 4>&2 2>/dev/null; trap '' PIPE               # the wrapper's messages on fd 4
trap 'stop "spec gave no answer within $limit s" watchdog' USR1
trap 'stop "the hook was stopped"' HUP INT TERM   # stop: KILL spec, end the watchdog, rm the temp file, exit 2
command -v spec || by_project "spec is not on PATH" "<PATH hint>"  # [gate] at/above CLAUDE_PROJECT_DIR: 2, else 0
err=$(mktemp "${TMPDIR:-/tmp}/specengine-gate.XXXXXX")
spec gate --stdin <&3 >/dev/null 2>"$err" &      # stdin through fd 3
sh -c '<limit times: sleep 1, the wrapper still its parent?>; kill -USR1 <wrapper>' &
wait "$pid"  # 0: 0; 2 + "unrecognized subcommand": by_project "this spec has no gate" "<reinstall>";
             # 2: 2, spec's stderr replayed; else 2 "spec ended with status N"
```

## Rules and edge cases

The canon's. Hook stdin as relied on (AC-16 confirms): `{session_id, transcript_path, cwd, permission_mode, hook_event_name, tool_name, tool_input}`; exit 2 refuses and hands stderr to the model; any other code and a timeout proceed; exit 0's stdout is hook JSON (`allow` skips the permission prompt).

## Acceptance criteria

Scratch repositories and `HOME`. New tests: CLI `gate.rs` 20, `gate_wrapper.rs` 8 (sh, dash, bash, zsh); core `gate.rs` 14; store `queue_gate.rs` 10, `worktree_git.rs` +3; mcp `mcp_gate.rs` 1. Updated: mcp `plugin_files.rs`, `plugin_skills.rs`, `mcp_tasks.rs` (the P2-3 scan), `mcp_stage.rs` (version >= 0.1.6); core `check_parity.rs` (five skill files). 47 named mutations red.

- [x] AC-01 -- no `[gate]`: 0 `off`; an unknown key, `mode = "strict"`, `"on"`, `min_files`, an undeclared zone, `src/[`, a zone `Code`, `../x`, a zone not an array -> 2 naming `<root>/specengine.toml:<line>`, while `spec show` answers; not TOML: 2 under a line naming `gate` (`[ gate ]`, `gate.mode =`), else off (CLI; core `mentions_gate`).
- [x] AC-02 -- `selective`: `draft`, `review`, `changes_requested`, `ready`, `done`, `cancelled` -> 2 `no-claim`; claimed -> 0 `claimed`, after `report` too.
- [x] AC-03 -- claimed in W1: W2 closed, also switched onto `main`; W1 on another branch closed, back open; a same-slug clone closed; detached closed, `branch` null.
- [x] AC-04 -- uncontrolled, no config, no `[gate]` -> 0 with the data directory `chmod 000`.
- [x] AC-05 -- `src/../src/a.rs`, absolute, `link/a.rs`, `./src/./a.rs`, absent `src/new/dir/b.rs`, `SRC/a.rs` (case-insensitive volume) -> `src/...`, 2; any `specengine.toml` in any case and depth (`src/x/SpecEngine.toml`, `crates/foo/specengine.toml` with zone `crates/*/src/**`) -> 2, zone null, "would be a project config"; symlinks: dangling into a zone 2, a zone file linking out 2, claimed 0, a dangling loop 2 gated, 0 ungated, 0 + note `observe`; `~/` against `HOME` (M: the raw path matched; only the canonical candidate judged; `rel == "specengine.toml"`).
- [x] AC-06 -- a bad `[gate]`, `HOME` unset, schema 6 (reinstall hint), a corrupt claim (`spec task show T` hint), outside git, the queue locked -> 2 under 4 s, `specengine gate: no verdict (`, never `no in_progress task`. Bad, absent or held-open stdin decided by the session's project (deviation 13): none, no `[gate]`, `off` 0; `observe` 0 + note; `selective`, broken 2; a 1.1 MB `Write` ungated 0; 30 MB gated claimed 0 in 1.28 s (M: a capped reader).
- [x] AC-07 -- wrapper (`gate_wrapper.rs`; sh, dash, bash, zsh): a fake `spec` exiting 1, 101, killed -> 2; printing `allow` -> stdout empty; slow -> 2 within 7 s + 0.5 s; HUP, INT, TERM -> 2, no temp file, no process after 1.1 s; a `SIGKILL`ed wrapper -> no `USR1` sent, watchdog gone in 1.2 s; 60 fast runs leave no `sleep`; no `spec` or one without `gate`: 2 under a `gate` line at or above `CLAUDE_PROJECT_DIR`, `[gateway]` 0, no `[gate]` 0.
- [x] AC-08 -- `--stdin`: zero stdout bytes on every outcome; no output names `permissionDecision`; usage errors exit 2, one line.
- [x] AC-09 -- a claimed task passes with an open question, a discrepancy bound, `stale`, a red `spec check` (a staged decision: no proposal row is read).
- [x] AC-10 -- `observe`: unclaimed -> 0 + one `gate.observed` with the four keys in order; a config: zone null; detached: branch null; claimed: none; data directory unreadable: 0 + note; export keeps the events.
- [x] AC-11 -- verdicts identical with every `.md` unreadable; the `.db` mtime and size kept over 10 calls, a fresh slug's `.db` absent; fixture medians (debug) claimed 31.0 ms, refused 31.4, not controlled 6.2, no config 6.2; the wrapper about +7 ms. Not met: the pilot-A median (Open 3).
- [x] AC-12 -- a refusal's stderr and JSON: root-relative path, zone, mode, `claim_task`, `spec task approve T`, IDs, status, claim place; title and goal never; nine `ready` -> 8 + `and 1 more`.
- [x] AC-13 -- `hooks.json` as canon "Hook" (`plugin_files.rs`; M: `"type": "http"`, `Write` dropped).
- [x] AC-14 -- `fixtures/spec-a` copy, `selective`, through the wrapper: `draft` 2, `ready` 2, claimed 0, reported 0, completed 2.
- [x] AC-15 -- five skills under the README's rules; `implement` names `claim_task`, `report_run`, `complete_task`, `analyze` `submit_plan`; the ten files; 0.2.0, `PINS` appended; the P2-3 scan over the gate sources and `plugin/**`, no `"code"` in core (`plugin_files.rs` `ac03_the_plugin_tree_is_data_s_ten_files`, `plugin_skills.rs`; M: `claim_task` dropped). `claude plugin validate --strict`: the owner's.
- [ ] AC-16 -- owner: see "Owner's manual check".

## Out of scope

`spec serve`, the `spec mcp` bridge; the other 07 s4 hooks; Bash and other servers' writes; `strict`, `min_files`, `contours`; a write checked against the plan; `plugin-roles`; profiles; a UI view of `gate.observed`.

## Open

1. **ADR-0037 stays accepted; the owner's explicit review is still open.** It rests on working answers Q1 (a new ADR supplementing ADR-0006, `#control` reworded), Q2 (only an `in_progress` task claimed in that worktree and branch opens), Q3 (no `[gate]` is off), Q7 (the config gated while on). Another answer is a new ADR superseding it.
2. AC-16, the owner's manual check.
3. AC-11's pilot-A median: not measured in this slice.

## Owner's manual check

Record the Claude Code version. In a `selective` scratch project with `spec` on `PATH`: an unclaimed Edit refused, its stderr reaching the model, also in a subagent and auto mode; an unrelated project passes; note the timeout, `MultiEdit`, `NotebookEdit`, `CLAUDE_PROJECT_DIR`, `/specengine:analyze`, `/specengine:implement`; `spec` off `PATH` -> the wrapper refuses; `claude plugin validate --strict` on `plugin`, `plugin/specengine`.

## Implementation

Canon: `gate.md` (built), `architecture.md#control`, `tasks.md` "Store", `proposal-queue.md` "States and events"; core, CLI, store, http READMEs, `ui/README.md`, root README "Claude Code plugin", `CLAUDE.md` State; 06 s3.5, 07 s1.4, s2, s4, s5, 08 s2, s3 AC-4. Iterations: Rust 3; review 1 changes needed (two majors: input failures refused every project; a nested config bypassed the gate), all taken in iteration 2; iteration 3: CLI comments cite canon `gate` without its path or ADR number (`genre.rs`). Workspace 2 012 passed, 20 skipped; fmt, clippy (+ probes) clean; UI 66 files green. A store `#[cfg(test)]` oracle (`rows.rs`) now covers every fixture corpus, `spec-c` included.

| Module | What it does |
|---|---|
| core `gate.rs` (new), `lib.rs`, `project_toml.rs` | config, modes, globs, `controlled`, `fold`, `opens`, `mentions_gate` |
| store `queue/gate.rs` (new), `queue.rs`, `lib.rs`, `git.rs`, `worktree.rs` | read-only open, tasks, `gate.observed`; `same_dir`; `new_until` kills git at the deadline |
| CLI `gate.rs` (new), `lib.rs`, `main.rs` | input, candidates, verdict, messages, JSON; `gate` runs apart; usage, panic, backstop |
| `plugin/` | `hooks/hooks.json`, `hooks/gate.sh`, two skills, 0.2.0, descriptions |

Deviations, accepted: (1) the wrapper as Data, not the draft's; (2) the signatures above; (3) `[ ] { } \` and control characters refused in globs; (4) `off` reads no zone; (5) `[project] slug` checked unless `off`; (6) corrupt rows as canon step 6, only when nothing opens; (7) config causes name the absolute path, hint `Fix <path>:<line>.`; (8) JSON `mode` `off` with no table; `observe` no verdict: pass `observed`, `cause` set, a note; (9) the detached wording; (10) `file_path`, else `notebook_path`; `tool_name` unread; `cwd` only for a relative path; (11) usage errors print the no-verdict line, a panic exits 2; (12) manifest descriptions name tasks and the gate; (13) iteration 2: no input cap, input failures by the session's project (none: pass), every `specengine.toml` controlled, two candidates, dangling links (16 hops), `~`, wrapper limit 7 s (draft: none), per-second watchdog, HUP/INT/TERM traps, version skew by the project, backstop by stage.

Residual risks: canon "Known limits" (timeout fail-open; the first-run macOS scan; `-wal`, `-shm`; cost) and "Recovery" (lockout; a `SIGKILL`ed wrapper's temp file); Claude Code behaviour unverified until AC-16.

Incident: during iteration 2's watchdog debugging the developer ran `pkill -x sleep` once, ending every `sleep` process of the user, possibly the owner's or other agents' waits. Owner's text, if wanted, for `.claude/agents/rust-developer.md` and `test-engineer.md`: "Never signal processes by name (`pkill`, `killall`): only PIDs your run started."
