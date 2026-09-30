---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-01
---

# The spec CLI: check and export index

CLI passes 2a.1, 2a.2 and 2b = `spec check` increment 3 (`docs/features/spec-cli-check.md`, `spec-cli-staged.md`, `spec-cli-switch.md`). Engine, config, findings, debt, verdict: `docs/canon/spec-check*.md`; discovery, globals, the one-line rule, exit codes: `crates/specengine-cli/README.md`. Both commands parse afresh through the store's check loader (store README): no database, slug or `HOME`. They are this repository's documentation gate ("The gate here").

## spec check

`spec check [--staged] [--baseline F] [--debt]` loads the whole config (`ProjectConfig`, `CheckConfig`) and the baseline, walks the working tree, judges with `check::run` and the UTC date taken at start (no clock flag), prints the report and exits with its verdict. Nothing is written, no index refreshed; the mode comes only from `[check] mode`.

- **Baseline**: `<root>/.spec-debt.toml` when an entry of that name exists (a directory or a dangling symlink there cannot be read: cannot check); `--baseline F`, relative to the current directory, replaces it and must exist.
- **Cannot check** (W-2): after discovery every failure is a `cannot` cause of the printed report, exit 2 — the config unreadable (an unreadable `--config` is no discovery failure), not UTF-8 or invalid anywhere (a cause per distinct error, `<config>:<line>`); the baseline missing, unreadable or invalid; the root unreadable (cause `.`); the walk's causes (an unlistable directory, also on the way to a default root). A config error stops before the baseline, in mode `enforce` (the mode is read only when all of `CheckConfig` is valid). Only usage and discovery failures leave stdout empty.
- **Names**: the config as in pass 1 (`specengine.toml`, or `--config` as typed), `.spec-debt.toml`, `--baseline` as typed, the root `.`; no output holds an absolute path the caller did not type.
- **Text**: the lines of `Report::lines(--debt)`, one-lined, uncapped (W-4): blocking findings only, and a cut listing would contradict its counts. **JSON** (W-1): `Report::to_json()` + `\n` verbatim, whatever `--debt` (then one `note:`), paths raw — the exception to the CLI's "absent = `null`": an unset `fix`, an absent `debt` are omitted.
- **Exit**: 0 `clean`, `observed`; 1 `blocked`; 2 `cannot-check`, usage, discovery. No `spec:` line beside a report.
- **Determinism** ("one tree and one date"): one tree, config, baseline and date → byte-identical stdout and stderr, whatever the absolute root, walk order or starting directory.

### The staged check

`--staged` (2a.2 Q1) judges what `git commit` records: stage-0 regular blobs (`100644`, `100755`) under the root in the index git names (`GIT_INDEX_FILE`: `commit -a`, `-o` too), by `WorkingTree`'s rules; config and baseline staged unless `--config`, `--baseline` (disk). Discovery stays on disk. A fully staged tree prints plain's bytes. A commit setting `mode = "observe"` is judged in `observe` (Q3).

- **Git** (`std::process`, 05 §9) in the root, read-only: `rev-parse`, `ls-files -s -z`, one lockstep `cat-file --batch` (config and baseline, then the listed blobs), `diff-files --diff-filter=A --ita-invisible-in-index` dropping intent-to-add entries (only when an empty blob under the root is a `.md`, config or baseline). Every child: `-c core.fsmonitor=false`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, `GIT_NO_REPLACE_OBJECTS=1`, `GIT_TERMINAL_PROMPT=0`; no `HEAD`, filter or refresh; git's text never shown.
- **Environment**: `GIT_*` inherited, resolved as git does from the caller's directory: relative `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` by `rev-parse --git-path index`, `--git-path objects`, `--git-common-dir`; `GIT_DIR`, `GIT_WORK_TREE` against it; `GIT_DIR` alone → `GIT_WORK_TREE` = it (git's rule, `core.worktree` set aside).
- **Guard**, `GIT_DIR` without `GIT_WORK_TREE` (linked worktree hooks): `rev-parse --show-toplevel`, then `--absolute-git-dir`, minus `GIT_DIR`, `GIT_CEILING_DIRECTORIES`, across file systems; goes on when no top is found, or the top is the caller's directory and its git dir `GIT_DIR`, or `GIT_DIR` its `.git` gitfile (same: canonical paths or device and inode); else cannot check, `GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top` (also a top with a foreign or missing `GIT_DIR`). A `GIT_DIR`-only tree without `.git` in a repository hidden by `GIT_CEILING_DIRECTORIES` is refused: set `GIT_WORK_TREE`.
- **Hooks** run from the top with `--root`, never `cd` (R11): the guard is complete only for layouts git creates (init, clone, `worktree add`, submodule, `--separate-git-dir`).
- **Cannot check** also, fixed one-line causes at `.` unless named: no repository or `git`, a git failure, `GIT_INDEX_FILE` empty or naming no file, an unresolvable relative variable (`GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` below the top), the guard; an unmerged path, a missing blob; the config unstaged or not a regular blob, nor the baseline. Before the config is read: mode `enforce` or `--config`'s.
- **Unlike plain**, by design: untracked, ignored, skip-worktree files; submodule contents; filters; symlinks, gitlinks skipped (a root of only them is missing; a non-UTF-8 gitlink uncounted); NFD names vs `core.precomposeUnicode`; an intent-to-add `.md` deleted on disk is walked empty, one neither `.md` nor TOML stays an empty file unless the detector runs.

## spec export index

A mode of `spec export` (owner, Q3; its bare form stays Phase 2's queue export, and bare `spec export` prints clap's help, exit 2): `render_index` with the `[[generators]]` entry `index = true` over the same fresh walk, written to `<root>/<[paths] index>` and nowhere else. The header names the registered `command` and `gate` (default `spec check`), never the binary; no default path, command or gate (ADR-0008). The baseline is not read.

- **Refused**, exit 2, empty stdout, nothing written: a config error (one `<config>:<line>: message` per cause); no `[[generators]]` or no `index = true` entry (naming the missing registration); an incomplete walk, `--stdout` included: core's `walk_gap` (§11.5's stop conditions), naming the first by path — an unreadable or unparsable file, an unlistable directory, a missing written root.
- **Writing**: every existing component of `[paths] index` below the root is a non-symlink directory, the file (if present) a non-symlink regular file, the parent exists (never created); else refused, a symlink's target untouched. Equal bytes → `unchanged`, the file not opened for writing (mtime kept). Else the file is opened, checked to be the one inspected (device and inode: a replaced file is refused), truncated and written in place: no temp file, no guard against a hand-written file (git is the net); a failed write → exit 2.
- **Written anyway, with a `warning:`**: names skipped for not being UTF-8 (one per problem path, with its count; their documents are not listed); `[paths] index` outside the walk (`spec check` then reports `index-missing`).
- **Output**: `wrote docs/index.md: 9182 bytes` or `unchanged …`; JSON `{"path":"docs/index.md","bytes":9182,"written":true}`; `--stdout`: the render byte for byte, nothing written (W-3). `--stdout --json` is a usage error in any argument order, one message (`Usage: spec export index [OPTIONS]`). Read-only projects are unguarded: a pilot takes `--stdout`.

## Owner's and working answers

- Q3 (2026-09-30): the index writer is `spec export index`; class `generated` documents are written only by their registered generator (`docs/canon/architecture.md#apply`).
- Q4 (2026-09-30): "introduced" is relative to `HEAD` (spec-cli-introduced below); it answers `docs/canon/spec-check.md` Q-5.
- Working answers (the code) → the other answer's cost: W-1 JSON verbatim → a second serialiser, or a core change breaking `check_output.rs`; W-2 cannot-check prints its report → no JSON for the failure that matters most; W-3 `--stdout` → previewing means writing, forbidden in pilots; W-4 uncapped → `search`'s cap.

## The gate here

2b (owner, 2026-09-30, 2026-10-01; ADR-0029): the root `specengine.toml` registers X = `cargo run -q -p specengine-cli -- export index`, gate G = `cargo run -q -p specengine-cli -- check`, both literal. Roles run `X && G`; the pre-commit hook runs `check --staged --root .`, CI `check --root .` on `HEAD`; G's summary carries the worst W. Roots, hook triggers, merges: `docs/README.md` "Enforcement".

Elsewhere: queue export, `--state` (Phase 2); applying `fix` (Q-3); an MCP check tool; path arguments; a clock flag; the `rev` rule (05 §3.5, Phase 3); generators' drift (ADR-0013).

## Next: spec-cli-introduced

Before the first pilot with a backlog (2a.2 Q2); then index sharding (`docs/features/roadmap.md` Q-8), before Phase 2 and a pilot's `spec export index`. Decided by 2a.2 Q4–Q9:

- Base: `HEAD`'s tree by `git ls-tree -r -z HEAD` in the root (not `HEAD:<prefix>`, failing for a new root), checked with the checked tree's config, parses shared by (path, blob OID); unborn `HEAD` → empty; its causes ignored, findings kept; git failing on it → cannot-check. **Introduced**, for `--staged` and `--changed` (the working tree): (code, path, subject) absent from the base's findings; a rename (or NFD vs precomposed name) re-introduces, a key's second occurrence does not.
- `mode = "enforce-introduced"` (observe → enforce-introduced → enforce, 04 §1.6) blocks on introduced errors not in live debt, on expired debt even over a pre-existing error (Q4), and on new debt: a triple `HEAD`'s baseline lacks at that path, or whose `expires` moved later, not earlier nor a changed `reason` (Q5); under `enforce` with a base too (Q7). New debt needs the owner's `--no-verify`; a `--baseline` outside the repository lifts that with a `note:`. No ADR (Q9): ADR-0022 enforcement a project opts into by its config.
- Mode: the stricter of the staged and `HEAD` configs' (Q6; `HEAD`'s unreadable or invalid → staged + a `note:`); `enforce-introduced` without `--staged`, `--changed` → `enforce` + a `note:` (Q8). Output: without a base 2b's bytes; with one an optional `introduced` in findings and counts, omitted when absent like `fix` (`check_output.rs` pins the keys); `--debt` labels pre-existing errors.
- Impact: `Mode` kebab-case, declared `Observe, EnforceIntroduced, Enforce` so derived `Ord` is the ladder (`blocks`' `==` breaks); new debt a 30th code in `CHECK_CODES` or a section beside `stale`. Risks: an error committed with `--no-verify` stays pre-existing; agents may pass `--no-verify` unless denied.

## Open

- The store walker's and `GitIndex`'s minors: store README "Open minors".
- A FIFO swapped in for the index before the open would block the writer (`O_NONBLOCK` later).
- Untested: the replaced-file refusal (needs a seam); the non-UTF-8 name warning on APFS, which refuses such names.
- Causes are sorted as strings (`:22` before `:6`; cosmetic, core); `ProjectConfig` stops at its first error, so a config's causes may be incomplete.
- Guard residues, false passes of a `cd` hook (fixes: `docs/features/spec-cli-staged.md`): (a) `core.worktree`, the git dir outside, `git --git-dir=… commit`; (b) discovery refused (dubious ownership, outer `--git-dir`; a broken `.git` gitfile) read as no repository; (c) the caller inside a git dir: accepted. Nits: a link elsewhere to the top's `.git` passes the gitfile clause; the message misleads for a top's foreign or missing `GIT_DIR`.
- git < 2.44 partial clones fetch lazily; `--ita-invisible-in-index` is experimental (gone → exit 2).
