---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli]
ref: 08 §2 Phase 1, CLI pass after spec-cli-introduced (owner's Q1, 2026-10-01)
shipped: 2026-10-01
---

# spec check --changed

## Why

Under `enforce-introduced` a plain `spec check` has no base: it falls back to `enforce` with a note and fails on a pilot's backlog. Only `--staged` had a base, so an agent had to `git add` to learn what it introduced. `--changed` gives the hook's verdict on the working tree: what plain sees, judged against `HEAD` by `--staged`'s rules. Owner's Q1 (2026-10-01): it ships before the first pilot's agents work under `enforce-introduced`.

No ADR: the owner's answers 2a.2 Q4–Q9 (Q9) and Q1–Q4 of 2026-10-01 (`docs/features/spec-cli-introduced.md`); ADR-0001, ADR-0008. Plain stays without a base (2a.2 Q8), CI plain, no `--base REF` (Q2); A1 (the checked tree is plain's disk walk) owner-agreed. How it works now: `docs/canon/spec-check-git.md` "The changed check", "The base"; `docs/canon/spec-check-cli.md` (synopsis, plain's note).

## Acceptance criteria

CLI tests unless store is named; 2a.2's scratch repositories (isolated git config and `HOME`, `HEAD` committed with `--no-verify`); copied `spec-a`, `spec-b` as the backlog; changes unstaged; `--changed` under `enforce-introduced` unless stated; never git in this repository. M: the mutation that must turn it red; (m): where the mutation run needed more.

- [x] AC-01 Parity: `--changed` == plain with the base's fields set aside (`common/staged.rs` `base_aside`) in text, `--debt`, `--json`, `--json --debt`; verdicts clean, blocked, observed, cannot-check; the unstaged tree holds a modified document, an untracked one, a `.gitignore`d one, one `git rm --cached` but kept, a dot-directory, an excluded `.md`, a symlinked `.md` under a root. M: the checked side read from `GitIndex`; `.gitignore` honoured.
- [x] AC-02 The index is not read: (a) an error staged, fixed only on disk → `--changed` 0, `--staged` 1; (b) the reverse → 1 at the disk line; (c) an unmerged document → no unmerged cause, the disk file judged; (d) `HEAD` and the index `observe`, the disk `enforce` → `[enforce]`; (e) under `enforce`, a baseline at `HEAD` and in the index covering E1, deleted on disk → E1 blocks. M: `ls-files` and the unmerged refusal reused; config or baseline from the index.
- [x] AC-03 Introduced, by bytes: `HEAD` has E1; the disk has E1, E1's key on another line and E2 → exit 1, one `error` line (E2), `introduced` only on E2, `counts.introduced` 1; disk == `HEAD` → exit 0, `observed`. M: sharing by path without comparing bytes.
- [x] AC-04 Paths: a plain `mv` of a document with an error → exit 1 at the new path; a deleted document cited by an unchanged one → the dangling citation introduced; an untracked document's errors introduced; an NFD name over `HEAD`'s precomposed one → introduced. M: equal bytes shared across paths; `HEAD` files missing on disk left out.
- [x] AC-05 (store, `BASE_PARSES`) N unchanged, 1 changed, 1 moved, 1 deleted, 1 untracked → the base parses exactly 3; nothing changed → 0. M: the base parsed afresh; bytes compared across paths.
- [x] AC-06 A partial base fails closed: `HEAD`'s blob of a changed or unchanged document deleted → exit 2 in the checked mode, one cause `HEAD's blob is missing from the git object database`, the checked run's causes merged, no findings, no notes; a non-blob OID → `HEAD's object is not a blob`; `HEAD`'s tree deleted → one fixed cause at `.` naming `git ls-tree`; no git text, no absolute path. M: a missing blob dropped (exit 0); an `ls-tree` failure read as an empty base.
- [x] AC-07 Mode, notes, new debt: introduced AC-09, AC-10, AC-11, AC-11b re-run with `--changed`. M: the disk mode alone; the baseline from the index; `HEAD`'s file looked up at the root for a given file at another path.
- [x] AC-08 Plain's note: plain → `[enforce]`, exit 1, one note naming both flags; `--changed` → `[enforce-introduced]`, no fallback note. M: the fallback note under `--changed`.
- [x] AC-09 Unborn `HEAD` and roots: introduced AC-06 re-run — no commit → no `ls-tree` or `cat-file`, blocking lines == `enforce`'s; an untracked `--root proj` → the same; `proj` committed with only pre-existing errors → exit 0. M: unborn read as a git failure; `HEAD:<prefix>`; `--full-tree`.
- [x] AC-10 Flags and failures: `--changed --staged` in either order → exit 2, usage error, empty stdout; no repository or no `git` → exit 2, one cause at `.`, no `fatal:`; an invalid disk config → plain's causes, no git call logged. M: no conflict; a fallback to plain; git before the config.
- [x] AC-11 Git use (`GitLog`): subcommands ⊆ {`rev-parse`, `ls-tree`, `cat-file`}; `ls-tree -r -z <hex>` once, only when born; one `cat-file --batch`; every OID once, equal contents once; `-c core.fsmonitor=false` and the four variables on every call. M: the staged session reused; a second session; the OID cache bypassed.
- [x] AC-12 Read-only, environment: `.git/index` bytes and mtime kept, no `index.lock`; objects, refs, `HEAD`, the tree unchanged; nothing under scratch `HOME`; a linked worktree → its own `HEAD`; the guard as for `--staged`; TAB and LF names, sha256 work. M: `update-index --refresh`; `GIT_DIR` dropped; `ls-tree` without `-z`.
- [x] AC-13 Load: ≥ 3 000 committed documents (≥ 3 MiB) and one of 1 MiB, 10 changed on disk → the expected report within 60 s (0.9 s measured). M: every request written before any reply is read.
- [x] AC-14 Determinism: two repositories at different absolute paths, files created in opposite orders → identical stdout, stderr, text and JSON. M: an absolute path in a cause.
- [x] AC-15 `--staged` unchanged: the staged and introduced suites pass with only AC-17's changes. M: the byte rule applied to `--staged`.
- [x] AC-16 Genre covers the new code: met by the new store `changed_genre.rs`, which scans `check.rs`, `git.rs`, `base.rs` and turns red for a `"docs/"` literal in each (the existing store `genre.rs` checks project names only in `base.rs`, so the literal in `check.rs` or `git.rs` stayed green there).
- [x] AC-17 Changed existing tests: `introduced.rs:182` (the note text), `common/staged.rs` `is_staged` (`--changed` counts as a run with a base); store `base.rs` gains `mod base_parses_changed`; no other test file, `.githooks/`, `.github/workflows/` unchanged.
- [x] AC-18 Docs: the gate passes; at shipping W = 115 743 B ≤ 115 675 + 76 B (this spec's Tier 3 index line); caps hold; net growth ≤ 0 for `CLAUDE.md`, 04, 05, 07, 08, `docs/canon/spec-check.md`, the store and CLI READMEs; `--changed` is canon in `docs/canon/spec-check-git.md` and in `spec-check-cli.md`'s synopsis and note; no "Next" lists `spec-cli-changed`.
- [x] AC-19 `cargo nextest run --workspace` once: 954/954; clippy, fmt clean; `build_graph.rs`: no new normal dependency (m: the `sha1` mutation not applied).

## Implementation

Two iterations; review accepted iteration 2 (two reviewer nits: the CLI picks the store function in one outer match via a private `AgainstHead` fn-pointer alias; `HeadWalk::read`'s doc covers both callers). The truth moved to `docs/canon/spec-check-git.md` ("The changed check", "The base", "Next", "Open"), `spec-check-cli.md`, the store and CLI READMEs.

| Module | What it does |
|---|---|
| store `git.rs` | `Staged::head_only(root, &GitEnv)`: a session without `ls-files`, unmerged refusal or intent-to-add detector; `child_vars`, `check_work_tree` unchanged |
| store `base.rs` | `Head::walk_by(&WalkScope)` (`Head::walk(&IndexWalk)` delegates); `HeadWalk::findings_by_bytes`: every listed object pre-scanned, no shared exception, reuse only at the same path without a read error and with equal bytes; private `run_base`, the parse-and-run tail shared with `--staged`, unchanged |
| store `check.rs`, `lib.rs` | `check_changed_with_notes(root, Option<GivenFile>, Option<GivenFile>, &GitEnv, today) -> StagedCheck` (config `None` → the root's `CONFIG_FILE` from disk, baseline `None` → `default_baseline`); `place()`, `read_head()` shared with `check_staged_with_notes` |
| cli `check.rs`, `lib.rs`, `main.rs` | `CheckedTree {WorkingTree (default), Staged(GitEnv), Changed(GitEnv)}`; `CheckRequest.tree` replaces `.staged`; `--changed` with `conflicts_with = "staged"`; plain's note names both flags |
| tests | cli `changed.rs` (12), `changed_base.rs` (5), `changed_git.rs` (10), `changed_load.rs` (1); store `changed_genre.rs` (2), `base.rs` `mod base_parses_changed` |

Deliberate: `CheckRequest.staged` → `tree: CheckedTree`, a public field change (only `main` builds it) that makes both flags unrepresentable in the library. An invalid disk baseline under `enforce-introduced`: plain prints `[enforce]` and its fallback note, `--changed` `[enforce-introduced]` without one, causes identical. A `--config` inside the root is named in notes as typed, as under `--staged`.

Residue (canon "Open"): A4, `child_vars` shared, so a `GIT_INDEX_FILE` naming no file is refused though unused; untested: R2's case-only rename on a case-insensitive file system.
