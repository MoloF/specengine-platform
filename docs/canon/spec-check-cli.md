---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-01
---

# The spec CLI: check and export index

CLI passes 2a.1, 2a.2, 2b and the base = `spec check` increment 3 (`docs/features/spec-cli-check.md`, `spec-cli-staged.md`, `spec-cli-switch.md`, `spec-cli-introduced.md`). Engine, config, findings, debt, verdict: `docs/canon/spec-check*.md`; discovery, globals, the one-line rule, exit codes: `crates/specengine-cli/README.md`. Both commands parse afresh through the store's check loader (store README): no database, slug or `HOME`.

## spec check

`spec check [--staged] [--baseline F] [--debt]` loads the whole config (`ProjectConfig`, `CheckConfig`) and the baseline, walks the working tree, judges with `check::run` and the UTC date taken at start, prints the report and exits with its verdict. Nothing is written, no index refreshed, no git run; the mode is `[check] mode` (`enforce-introduced`: no base, so `enforce` + a `note:`, 2a.2 Q8).

- **Baseline**: `<root>/.spec-debt.toml` when an entry of that name exists (a directory or a dangling symlink there cannot be read: cannot check); `--baseline F`, relative to the current directory, replaces it and must exist.
- **Cannot check** (W-2): after discovery every failure is a `cannot` cause of the printed report, exit 2 — the config unreadable (an unreadable `--config` is no discovery failure), not UTF-8 or invalid anywhere (a cause per distinct error, `<config>:<line>`); the baseline missing, unreadable or invalid; the root unreadable (cause `.`); the walk's causes (an unlistable directory, also on the way to a default root). A config error stops before the baseline, in mode `enforce` (the mode is read only when all of `CheckConfig` is valid). Only usage and discovery failures leave stdout empty.
- **Names**: the config as in pass 1 (`specengine.toml`, or `--config` as typed), `.spec-debt.toml`, `--baseline` as typed, the root `.`; no output holds an absolute path the caller did not type.
- **Text**: the lines of `Report::lines(--debt)`, one-lined, uncapped (W-4): blocking findings only, and a cut listing would contradict its counts. **JSON** (W-1): `Report::to_json()` + `\n` verbatim, whatever `--debt` (then one `note:`), paths raw — the exception to the CLI's "absent = `null`": an unset `fix`, an absent `debt` are omitted.
- **Exit**: 0 `clean`, `observed`; 1 `blocked`; 2 `cannot-check`, usage, discovery. No `spec:` line beside a report.
- **Determinism** ("one tree and one date"): one tree, config, baseline and date → byte-identical stdout and stderr, whatever the absolute root, walk order or starting directory.

### The staged check

`--staged` (2a.2 Q1) judges what `git commit` records: stage-0 regular blobs (`100644`, `100755`) under the root in the index git names (`GIT_INDEX_FILE`: `commit -a`, `-o` too), by `WorkingTree`'s rules; config and baseline staged unless `--config`, `--baseline` (disk). Discovery stays on disk. Against `HEAD` ("The base"), a fully staged tree prints plain's bytes only with the base's fields set aside, under `enforce`, without new debt.

- **Git** (`std::process`, 05 §9) in the root, read-only: `rev-parse`, `ls-files -s -z`; `rev-parse --verify -q HEAD` (born, unborn, else a failure); a born `HEAD`'s `ls-tree -r -z <oid>` (the root's subtree, paths as `ls-files`'s); one lockstep `cat-file --batch`, each OID once, in order: staged config and baseline, `HEAD`'s, the listed blobs, `HEAD`'s blobs whose (path, OID) the index lacks; `diff-files --diff-filter=A --ita-invisible-in-index` dropping intent-to-add entries (only when an empty blob under the root is a `.md`, config or baseline). Every child: `-c core.fsmonitor=false`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, `GIT_NO_REPLACE_OBJECTS=1`, `GIT_TERMINAL_PROMPT=0`; no filter or refresh; git's text never shown.
- **Environment**: `GIT_*` inherited, resolved as git does from the caller's directory: relative `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` by `rev-parse --git-path index`, `--git-path objects`, `--git-common-dir`; `GIT_DIR`, `GIT_WORK_TREE` against it; `GIT_DIR` alone → `GIT_WORK_TREE` = it (git's rule, `core.worktree` set aside).
- **Guard**, `GIT_DIR` without `GIT_WORK_TREE` (linked worktree hooks): `rev-parse --show-toplevel`, then `--absolute-git-dir`, minus `GIT_DIR`, `GIT_CEILING_DIRECTORIES`, across file systems; goes on when no top is found, or the top is the caller's directory and its git dir `GIT_DIR`, or `GIT_DIR` its `.git` gitfile (same: canonical paths or device and inode); else cannot check, `GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top` (also a top with a foreign or missing `GIT_DIR`). A `GIT_DIR`-only tree without `.git` in a repository hidden by `GIT_CEILING_DIRECTORIES` is refused: set `GIT_WORK_TREE`.
- **Hooks** run from the top with `--root`, never `cd` (R11): the guard is complete only for layouts git creates (init, clone, `worktree add`, submodule, `--separate-git-dir`).
- **Cannot check** also, fixed one-line causes at `.` unless named: no repository or `git`, a git failure, `GIT_INDEX_FILE` empty or naming no file, an unresolvable relative variable (`GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` below the top), the guard; an unmerged path, a missing blob; the config unstaged or not a regular blob, nor the baseline; a partial base ("The base"). Before the config is read: mode `enforce` or `--config`'s; never with notes.
- **Unlike plain**, by design: untracked, ignored, skip-worktree files; submodule contents; filters; symlinks, gitlinks skipped (a root of only them is missing; a non-UTF-8 gitlink uncounted); NFD names vs `core.precomposeUnicode`; an intent-to-add `.md` deleted on disk is walked empty, one neither `.md` nor TOML stays an empty file unless the detector runs.

### The base

`--staged` is judged against `HEAD` in every mode, a plain run never (owner's Q4 of 2026-09-30; 2a.2 Q4–Q9, Q1–Q4 of 2026-10-01: `docs/features/spec-cli-introduced.md`; no ADR, Q9). Introduced, new debt, verdict: `docs/canon/spec-check.md`.

- **Base**: `HEAD`'s tree under the root, judged by `check::run` with the checked scheme, `[paths]`, `CheckConfig`, no baseline and the same date; causes dropped, findings kept (a root absent at `HEAD`: all introduced); unborn → empty. A path at the index's OID reuses the checked parse; bytes shared by OID, never a parse across paths. No rename detection: a `git mv`, an NFD name over a precomposed one re-introduce.
- **Mode**: the stricter of the checked config's and `HEAD`'s (2a.2 Q6), its only use of `HEAD`'s config. **New debt** against `HEAD`'s baseline (absent → empty): here a new `.spec-debt.toml` entry or a later `expires` takes the owner's `--no-verify` (2a.2 Q5, Q7). Pilots: `enforce-introduced`, the same hook, CI plain = `enforce` (Q2: no `--base REF`).
- **Notes** (stderr `note:`, never JSON), in order: `HEAD`'s config unknown — a symlink, gitlink, missing blob, not UTF-8 or invalid (`HEAD's specengine.toml is not a valid config (line N): …`), `--config` outside the root — so the checked mode; `HEAD`'s stricter mode; `HEAD`'s baseline unknown likewise or `--baseline` outside the root: new debt unjudged (Q3, Q4). Outside: the canonical path not below the canonical root; else the root-relative path names `HEAD`'s file.
- **A partial base fails closed**: a listed `HEAD` document not given as a blob → a cause at its path, `HEAD's blob is missing from the git object database`, `HEAD's object is not a blob` or `HEAD's blob was not read`; cannot-check in the checked mode with the checked run's causes, no findings, no notes; a (path, OID) the index shares is the checked run's own cause.

## spec export index

A mode of `spec export` (owner's Q3, 2026-09-30, `docs/canon/architecture.md#apply`; bare `spec export`, Phase 2's queue export, prints clap's help, exit 2): `render_index` with the `[[generators]]` entry `index = true` over the same fresh walk, written to `<root>/<[paths] index>` and nowhere else. The header names the registered `command` and `gate` (default `spec check`), never the binary; no default path, command or gate (ADR-0008). The baseline is not read.

- **Refused**, exit 2, empty stdout, nothing written: a config error (one `<config>:<line>: message` per cause); no `[[generators]]` or no `index = true` entry (naming the missing registration); an incomplete walk, `--stdout` included: core's `walk_gap` (§11.5's stop conditions), naming the first by path — an unreadable or unparsable file, an unlistable directory, a missing written root.
- **Writing**: every existing component of `[paths] index` below the root a non-symlink directory, the file (if any) a non-symlink regular file, the parent existing (never created); else refused, a symlink's target untouched. Equal bytes → `unchanged`, not opened for writing (mtime kept). Else opened, checked to be the file inspected (device and inode), truncated and written in place: no temp file, no guard against a hand-written file (git is the net); a failed write → exit 2.
- **Written anyway, with a `warning:`**: names skipped for not being UTF-8 (one per problem path, with its count; their documents are not listed); `[paths] index` outside the walk (`spec check` then reports `index-missing`).
- **Output**: `wrote docs/index.md: 9182 bytes` or `unchanged …`; JSON `{"path":"docs/index.md","bytes":9182,"written":true}`; `--stdout`: the render byte for byte, nothing written (W-3). `--stdout --json` is a usage error in any argument order, one message (`Usage: spec export index [OPTIONS]`). Read-only projects are unguarded: a pilot takes `--stdout`.

## Working answers

The code → the other answer's cost: W-1 JSON verbatim → a second serialiser, or a core change breaking `check_output.rs`; W-2 cannot-check prints its report → no JSON for the failure that matters most; W-3 `--stdout` → previewing means writing, forbidden in pilots; W-4 uncapped → `search`'s cap.

## The gate here

2b (owner, 2026-09-30, 2026-10-01; ADR-0029): the root `specengine.toml` registers X = `cargo run -q -p specengine-cli -- export index`, gate G = `cargo run -q -p specengine-cli -- check`, both literal. Roles run `X && G`; the pre-commit hook runs `check --staged --root .`, CI `check --root .` on `HEAD`; G's summary carries the worst W. Roots, hook triggers, merges: `docs/README.md` "Enforcement".

Elsewhere: queue export, `--state` (Phase 2); applying `fix` (Q-3); an MCP check tool; path arguments; a clock flag; the `rev` rule (Phase 3); generators' drift (ADR-0013).

## Next

`spec-cli-changed` (owner's Q1, 2026-10-01): `--changed`, the working tree against `HEAD`, parses shared by bytes; both before pilot agents use `enforce-introduced`. Then index sharding (`docs/features/roadmap.md` Q-8) before Phase 2 and a pilot's `spec export index`; then the pilots.

## Open

- The store walker's and `GitIndex`'s minors: store README "Open minors".
- A FIFO swapped in for the index before the open would block the writer (`O_NONBLOCK` later).
- Untested: the replaced-file refusal (needs a seam); the non-UTF-8 name warning on APFS, which refuses such names.
- Causes are sorted as strings (`:22` before `:6`; cosmetic, core); `ProjectConfig` stops at its first error, so a config's causes may be incomplete.
- Guard residues, false passes of a `cd` hook (fixes: `docs/features/spec-cli-staged.md`): (a) `core.worktree`, an outside git dir, `git --git-dir=… commit`; (b) refused discovery (dubious ownership, outer `--git-dir`, a broken `.git` gitfile) read as no repository; (c) a caller inside a git dir accepted. Nits: a link to the top's `.git` passes the gitfile clause; the message misleads for a top's foreign or missing `GIT_DIR`.
- git < 2.44 partial clones fetch lazily; `--ita-invisible-in-index` is experimental (gone → exit 2).
- Base residues: a corrupt `HEAD` ref reads as unborn; a `--config` in the root at a path `HEAD` lacks: checked mode, no note; a `HEAD` document panicking the parser is dropped from the base with its cause, so what cites it reads pre-existing (the owner may rule it partial). A `--no-verify` commit leaves its errors pre-existing.
