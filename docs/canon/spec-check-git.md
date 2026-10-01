---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-01
---

# spec check against HEAD

`spec check --staged`, `--changed` and their base, `HEAD`: CLI pass 2a.2, `spec check` increment 3, the `--changed` pass (`docs/features/spec-cli-staged.md`, `spec-cli-introduced.md`, `spec-cli-changed.md`). Plain `spec check` (baseline, causes, names, output, exit, determinism) and the gate: `docs/canon/spec-check-cli.md`; introduced, new debt, verdict: `docs/canon/spec-check.md`; the code: the store's `git.rs`, `source.rs` (`GitIndex`), `base.rs`, `check.rs` (store README). The two flags exclude each other: usage error, exit 2, empty stdout.

## The staged check

`--staged` (2a.2 Q1) judges what `git commit` records: stage-0 regular blobs (`100644`, `100755`) under the root in the index git names (`GIT_INDEX_FILE`: `commit -a`, `-o` too), by `WorkingTree`'s rules; config and baseline staged unless `--config`, `--baseline` (disk). Discovery stays on disk. Against `HEAD` ("The base"), a fully staged tree prints plain's bytes only with the base's fields set aside, under `enforce`, without new debt.

- **Git** (`std::process`, 05 §9) in the root, read-only: `rev-parse`, `ls-files -s -z`; `rev-parse --verify -q HEAD` (born, unborn, else a failure); a born `HEAD`'s `ls-tree -r -z <oid>` (the root's subtree, paths as `ls-files`'s); one lockstep `cat-file --batch`, each OID once, in order: staged config and baseline, `HEAD`'s, the listed blobs, `HEAD`'s blobs whose (path, OID) the index lacks; `diff-files --diff-filter=A --ita-invisible-in-index` dropping intent-to-add entries (only when an empty blob under the root is a `.md`, config or baseline). Every child: `-c core.fsmonitor=false`, `GIT_OPTIONAL_LOCKS=0`, `GIT_NO_LAZY_FETCH=1`, `GIT_NO_REPLACE_OBJECTS=1`, `GIT_TERMINAL_PROMPT=0`; no filter or refresh; git's text never shown.
- **Environment**: `GIT_*` inherited, resolved as git does from the caller's directory: relative `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` by `rev-parse --git-path index`, `--git-path objects`, `--git-common-dir`; `GIT_DIR`, `GIT_WORK_TREE` against it; `GIT_DIR` alone → `GIT_WORK_TREE` = it (git's rule, `core.worktree` set aside).
- **Guard**, `GIT_DIR` without `GIT_WORK_TREE` (linked worktree hooks): `rev-parse --show-toplevel`, then `--absolute-git-dir`, minus `GIT_DIR`, `GIT_CEILING_DIRECTORIES`, across file systems; goes on when no top is found, or the top is the caller's directory and its git dir `GIT_DIR`, or `GIT_DIR` its `.git` gitfile (same: canonical paths or device and inode); else cannot check, `GIT_DIR is set without GIT_WORK_TREE below the working tree's top: pass --root from the top` (also a top with a foreign or missing `GIT_DIR`). A `GIT_DIR`-only tree without `.git` in a repository hidden by `GIT_CEILING_DIRECTORIES` is refused: set `GIT_WORK_TREE`.
- **Hooks** run from the top with `--root`, never `cd` (R11): the guard is complete only for layouts git creates (init, clone, `worktree add`, submodule, `--separate-git-dir`).
- **Cannot check**, beyond plain's causes: fixed one-line causes at `.` unless named — no repository or `git`, a git failure, `GIT_INDEX_FILE` empty or naming no file, an unresolvable relative variable (`GIT_OBJECT_DIRECTORY`, `GIT_COMMON_DIR` below the top), the guard; an unmerged path, a missing blob; the config unstaged or not a regular blob, nor the baseline; a partial base ("The base"). Before the config is read: mode `enforce` or `--config`'s; never with notes.
- **Unlike plain**, by design: untracked, ignored, skip-worktree files; submodule contents; filters; symlinks, gitlinks skipped (a root of only them is missing; a non-UTF-8 gitlink uncounted); NFD names vs `core.precomposeUnicode`; an intent-to-add `.md` deleted on disk is walked empty, one neither `.md` nor TOML stays an empty file unless the detector runs.

## The changed check

`--changed` (owner's Q1, 2026-10-01) gives the hook's verdict before `git add`: plain's tree judged against `HEAD`. The tree is plain's disk walk exactly (`WorkingTree`, store README "Walk"): untracked and ignored files included (introduced: `HEAD` lacks them), `.gitignore` and the git index never read; config and baseline from disk as plain (`--config`, `--baseline`, else `specengine.toml`, `default_baseline`). An untracked or ignored `.md` that must not count: `[paths] exclude`, set at a pilot's onboarding. With the base's fields set aside, under `enforce` and without new debt, it prints plain's bytes.

- **Base**: `--staged`'s ("The base"), with five differences. (1) Config and baseline from disk; `HEAD`'s found by the rule for given files ("Notes"), the root's own at `specengine.toml`, `.spec-debt.toml`. (2) No git index: no `ls-files`, unmerged refusal or intent-to-add detector. (3) Sharing by bytes: a `HEAD` file reuses the checked parse only at the same path, its disk file read without error and byte-equal to `HEAD`'s blob; else `HEAD`'s blob is parsed, never a parse across paths. (4) Every object `ls-tree` lists under the root is read and checked before any sharing or parse. (5) No shared-OID exception: each such object not given as a blob is a base cause.
- **Git**: `GitEnv::child_vars` as for `--staged` (environment, guard, forced variables), then only `rev-parse --is-inside-work-tree`, `rev-parse --verify -q HEAD`, a born `HEAD`'s `ls-tree -r -z <oid>` once, one lockstep `cat-file --batch`, each OID once, in order `HEAD`'s config, baseline, the listed blobs. Unborn: the two `rev-parse` calls, every finding introduced.
- **Order**: the disk config and baseline are validated first, as plain, with no git run, an error giving plain's causes (an invalid baseline under `enforce-introduced` keeps `[enforce-introduced]` and no note, where plain prints `[enforce]` and its fallback note); then a git failure, no repository or no `git` → one fixed cause at `.` in the disk config's mode, no notes, never a fallback to plain; a partial base → its causes and the checked run's, in the checked mode.
- **Unlike `--staged`**, by design: every listed `HEAD` blob is read on each run (cost grows with the corpus: revisit with the Phase 2 daemon); smudged or eol-converted bytes never equal clean blobs (no sharing; a size-dependent finding may read introduced); an NFD name on disk over `HEAD`'s NFC, a case-only rename on a case-insensitive file system re-introduce a document; submodule contents read as introduced, sparse or skip-worktree documents as deleted.

## The base

`--staged` and `--changed` are judged against `HEAD` in every mode, a plain run never (owner's Q4 of 2026-09-30; 2a.2 Q4–Q9, Q1–Q4 of 2026-10-01: `docs/features/spec-cli-introduced.md`; no ADR, Q9). Introduced, new debt, verdict: `docs/canon/spec-check.md`.

- **Base**: `HEAD`'s tree under the root, judged by `check::run` with the checked scheme, `[paths]`, `CheckConfig`, no baseline and the same date; causes dropped, findings kept (a root absent at `HEAD`: all introduced); unborn → empty; during a merge or rebase, still `HEAD`. Under `--staged` a path at the index's OID reuses the checked parse (bytes shared by OID; `--changed`: by bytes), never a parse across paths. No rename detection: a `git mv` or `mv`, an NFD name over a precomposed one re-introduce; a deleted document's old path is parsed from `HEAD`, so a citation of its ID reads introduced.
- **Mode**: the stricter of the checked config's and `HEAD`'s (2a.2 Q6), its only use of `HEAD`'s config. **New debt** against `HEAD`'s baseline (absent → empty): here a new `.spec-debt.toml` entry or a later `expires` takes the owner's `--no-verify` (2a.2 Q5, Q7). Pilots: `enforce-introduced`, agents' `--changed`, the same hook, CI plain = `enforce` (Q2: no `--base REF`).
- **Notes** (stderr `note:`, never JSON), in order: `HEAD`'s config unknown — a symlink, gitlink, missing blob, not UTF-8 or invalid (`HEAD's specengine.toml is not a valid config (line N): …`), `--config` outside the root — so the checked mode; `HEAD`'s stricter mode; `HEAD`'s baseline unknown likewise or `--baseline` outside the root: new debt unjudged (Q3, Q4). Outside: the canonical path not below the canonical root; else the root-relative path names `HEAD`'s file; a given file is named as typed.
- **A partial base fails closed**: a listed `HEAD` document not given as a blob → a cause at its path, `HEAD's blob is missing from the git object database`, `HEAD's object is not a blob` or `HEAD's blob was not read`; cannot-check in the checked mode with the checked run's causes, no findings, no notes; under `--staged` only, a (path, OID) the index shares is the checked run's own cause.

## Next

The rest of Phase 1 (08 §2): CLI passes 3 (graph) and 4 (bundle), MCP stdio reads, check increment 4 (`spec-check-process`); then the pilots.

## Open

- `GitIndex`'s minors: store README "Open minors".
- Guard residues, false passes of a `cd` hook (fixes: `docs/features/spec-cli-staged.md`): (a) `core.worktree`, an outside git dir, `git --git-dir=… commit`; (b) refused discovery (dubious ownership, outer `--git-dir`, a broken `.git` gitfile) read as no repository; (c) a caller inside a git dir accepted. Nits: a link to the top's `.git` passes the gitfile clause; the message misleads for a top's foreign or missing `GIT_DIR`.
- git < 2.44 partial clones fetch lazily; `--ita-invisible-in-index` is experimental (gone → exit 2).
- Base residues: a corrupt `HEAD` ref reads as unborn; a `--config` in the root at a path `HEAD` lacks: checked mode, no note; a `HEAD` document panicking the parser is dropped from the base with its cause, so what cites it reads pre-existing (the owner may rule it partial). A `--no-verify` commit leaves its errors pre-existing.
- `--changed`: `child_vars` is shared, so a `GIT_INDEX_FILE` naming no file is refused though the index is unused; untested: a case-only rename on a case-insensitive file system.
