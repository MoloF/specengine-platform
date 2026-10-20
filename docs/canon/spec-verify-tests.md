---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# `spec verify --tests`: bound tests run by the project's own command

`spec verify --tests [--task T] [--json]` (+ `--root`) runs the tests `@verifies` binds to the selected nodes through the project's `[verify] tests` argv template, once per distinct argv, only on the owner's terminal after one `[y/N]`, over a clean committed tree (ADR-0044, `architecture-code.md#tests`). No agent starts it (no MCP tool, route, UI, hook, CI step or role). It writes only the data directory (index, queue, the `verify.tests` event), never `spec.lock` (8b), a task or `runs` row; the project's command may write anything. SpecEngine names no runner, parses no output (ADR-0008). Code: core `verify` (`TestTemplate`, `node_verdict`, `tests_verdict`), store `status_all`, `B3Stream`, `record_verify_tests`, CLI `verify_tests.rs`, `test_runner.rs` (`#[cfg(unix)]`), `node_set.rs`. Texts, text and JSON: `spec-verify-output.md` "`--tests`".

## The `[verify]` keys

```toml
[verify]
tests   = ["scripts/run-test", "{package}", "{target_kind}", "{target}", "{test}"]
timeout = 600   # seconds per invocation, probe included, 1..86400
```

`tests` is argv, never a shell (if wanted: `["sh", "-c", "<script using \"$1\">", "sh", "{test}"]`). Element 0, the program, relative with a `/`: joined to the root; else found on the child's `PATH`. Placeholders, closed: `{file}`, `{qpath}`, `{package}`, `{target_kind}`, `{target}`, `{test}`; `{{`, `}}` a brace; `{test}` or `{qpath}` required. Errors `specengine.toml:<line>: [verify] …`, exit 2; plain verify's run cause 1.

## The node set

CLI `node_set.rs` (`named_task` too), shared with plain `--task` (`spec-verify-scope.md`) and `--node`:

- **A task's set** (`task_set`): its `targets`, `criteria` refs and `affected_nodes` columns now, any state, never `snapshot`, resolved as `spec show` resolves a REF, plus contained sections; a ref naming nothing: `note: verify: <ref> names nothing now`. `T` in `[ids]`, a look-alike, unknown or another repository's: exit 2, stdout empty, `tasks.md` "Commands"' texts.
- **A row in a set** (`NodeSet::row_nodes`): a marker `spec refs` lists under a set node (`resolved`, `alias`, each `ambiguous`, `homoglyph` under its fix's node), or a lock entry whose `node` is one. `--tests` uses the marker half, plain `--task` both.
- **Selection**: the task's set, else every node a `@verifies` binding names; a test row per (`file`, `symbol`), a keyless binding's per marker comment (by its offset), with its set nodes; a node is reported when a `@verifies` binding names it (`--task`: `note: verify --tests: <n> nodes of <T> have no @verifies binding`).

## Placeholders

From the structured name (`placed_name_file`), never a split qpath: `{file}` root-relative; `{qpath}` `Name::undiscriminated()`; `{package}` `[package] name` (none on the directory fallback); `{target_kind}`, `{target}` the unit's, a primary target's from the layout (`[lib] name`, `[[test]] path` ignored: a wrong `{target}` fails, never passes); `{test}` libtest's, `QPath` module segments and name by `::`. `mod tests` in `crates/game/src/stamina.rs`: `stamina::tests::regen_stops_at_zero`, `lib`, `game`; `crates/game/tests/stamina.rs`: `drain_is_clamped`, `test`, `stamina`, `{qpath}` `crates/game::test:stamina::drain_is_clamped`. A value is bytes in its element, never split or joined. Equal argv (`cfg` twins) run once, sharing the outcome.

## Runnable and verifiable

**Runnable**: a free `function_item` of a rooted, non-`shared:` unit in a file `HEAD` holds, each used value free of U+0000-U+001F, U+007F. Else `not run`, the first of: `not committed`, `module`, `ron`, `unrooted`, `shared unit`, `member`, `not a function` (a keyless binding too: kind its state, symbol its category), `control character`, `no value {<name>}`, `node not verifiable`. Core never looks for `#[test]`.

**Verifiable**: accept's predicate, CLI `lock.rs` `committed_bindings` over the marker files naming the node, `HEAD`'s bytes, `node_at_head`'s `rev` and an empty old lock, gives no `unlockable` or `keyless` entry. Else `not verifiable <state>`, its first refusal (keyed, then keyless) in its words; the node's file or a marker file not in `HEAD`: `not committed`; `node_at_head` refusing: `rev_unknown` if the corpus cannot read the revision, else `hash_null`. A predicate error (a `HEAD` blob not UTF-8, `HEAD` and disk giving other items): `cannot`.

## Order

1. Usage, exit 2: `--tests` with `--changed` or `--staged`. Then not Unix, `require_terminal`, the clock: exit 2, stdout empty.
2. Discovery (exit 2); `[verify]` (`tests` required), `[code]`: else `cannot`.
3. A worktree, `HEAD` born, a branch, no operation in progress: else `cannot`.
4. `status_all`, no pathspec: an entry -> `dirty`. The root's `specengine.toml`, whatever `--config` names (another `--config`: `cannot` at 5), not a blob of `HEAD` (ignored) -> `cannot`; a git error: `git: <error>`.
5. `committer_ident`; the queue as `spec task` opens it (a newer schema, another `--config`: `cannot`); the task (dirty wins).
6. The corpus, the code walk (a gap: `cannot`), the set, runnable, verifiable. A distinct program of the planned argv, probes too, relative with a `/` and not a file of `HEAD` -> `cannot`. Element 0 is read from the worktree top through the root's path: `.` and empty segments dropped; a `..` only before the first written name, popping a root segment; a `..` after a name or past the top: not a file of `HEAD` (`../tools/run-test` from `sub/` runs; `scripts/../scripts/run-test`, `target/x/../../scripts/run-test` are `cannot`). A committed symlink is a file of `HEAD`; its target is what it calls. Nothing to run -> `empty`.
7. The listing and the question on stderr: the counts, the template run, the timeout, worktree, branch, commit. Only `y`, `yes`, no `--yes`; else `declined`. Ctrl-C here ends `spec`.
8. Probes, one per distinct probe argv (`{test}` `specengine_verify_probe_no_such_test`, `{qpath}` the test's with it last), in order of first use, stopping at the first refusal. One passing or timing out -> `refused`.
9. Each distinct argv once, by its first (`file`, `symbol`). A program that cannot start (8 too) -> `refused`.
10. After a completed 9: `HEAD`, branch, `status_all` again; a difference or a git failure -> `changed`.
11. The event, then stdout.

## Runner

One at a time: `process_group(0)`, cwd the root, stdin null, `env_clear` + `WorktreeGit::vars()` (the owner's environment minus git's local variables, `GIT_TERMINAL_PROMPT=0`). stdout and stderr share one pipe, copied live to `spec`'s stderr, hashed whole; `failed`, `timed out` keep its last 4 096 bytes. Nothing reaches stdout or a file.

- Exit seen unreaped (`waitid` `WNOWAIT`, every 10 ms), the group `SIGKILL`ed, then the reap: a signalled group ID is never reused; the pipe read to EOF or 5 s. Overrun from spawn: `SIGTERM` to the group, 5 s, `SIGKILL`.
- Steps 8-9 only: `SIGINT`, `SIGTERM`, `SIGHUP` (a `sigaction` flag) go to the group as that escalation: `interrupted`.
- A leader that cannot be waited on (`waitid` and `try_wait` failing): `cannot start`, unless a stop is under way (its cause wins); its pid is never signalled.
- `libc` `killpg`, `sigaction`, `waitid`: the CLI's only `unsafe`, a `// SAFETY:` each, in `group.rs`, the core `spec resolve` shares (`layer-c.md`; its wait adds the caller's stop as a third trigger).

## Verdicts and event

A test: `passed` (exit 0 in time), `failed` (`exit <n>`, `signal <n>`), `timed out` (`after <t> s`, exit and signal null), `not run` (its reason; unstarted: the run verdict; stopped by a forwarded signal: `interrupted`, exit, signal, hash, tail null). A node: `verified` (runnable tests, all passed), `failed` (a time-out too), `not verifiable <state>`, `no runnable test`.

| Run verdict | Exit | When |
|---|---|---|
| `verified` | 0 | every reported node verified |
| `unverified` | 1 | a reported node not verified |
| `dirty`, `empty`, `declined`, `changed` | 1 | steps 4, 6, 7, 10 |
| `refused`, `interrupted` | 2 | steps 8-9 |
| `cannot` | 2 | steps 2-6 |

Exit 1 is "not verified", outside ADR-0042's judge, never a Fix. `nodes` is `[]` but for `verified`, `unverified`, `empty`.

**`verify.tests`** for `verified`, `unverified`, `changed`, `refused`, `interrupted`, last, by `record_verify_tests` (one `Immediate` transaction, this build's schema); failing: `warning: verify.tests not recorded: <error>`, exit kept. Keys: `task`, `worktree`, `branch`, `commit`, `command`, `timeout`, `verdict`, `probes` (started: `argv`, `exit`, `signal`, `outcome`, `output_hash`), `tests` (`file`, `symbol`, `nodes`, `argv`, `exit`, `signal`, `outcome`, `output_hash` `b3:` or null, `tail` of `failed`, `timed out`), `nodes` (`id`, `verdict`, `state`), `lock_commit` (null until 8b), `by` (committer, no date).

## Known limits

- Provenance: a clean tree, `specengine.toml` and a root-relative program in `HEAD`; a `skip-worktree` file, an absolute or `PATH` program or what it calls is not traced, and an agent's commit can change what runs. The owner's `y` on the shown template is the guard: review `[verify]` edits as code.
- The owner's rights and environment (`env -i …` for less); no overall cap; `setsid`, a double fork or a `SIGKILL`ed `spec` escapes the group. A `spec` inheriting `SIGCHLD` `SIG_IGN` has its children reaped unseen: the first probe or test reads `cannot start` (`No child processes`), `refused`.
- The probe catches "nothing matched", not a substring filter; a skipped `#[ignore]` passes unless wrapped; a faked terminal passes (ADR-0034); per-test runs match the suite only for order-independent tests; a test writing the tree ends `changed`.
