---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-01
---

# spec check and export index

CLI passes 2a.1 and 2b of `spec check` increment 3 (`docs/features/spec-cli-check.md`, `spec-cli-switch.md`). `--staged`, `--changed` and their base, `HEAD` (2a.2, `enforce-introduced`): `docs/canon/spec-check-git.md`. Engine, config, findings, debt, verdict: `docs/canon/spec-check*.md`; discovery, globals, the one-line rule, exit codes: `crates/specengine-cli/README.md`. Both commands parse afresh through the store's check loader (store README): no database, slug or `HOME`.

## spec check

`spec check [--staged | --changed] [--baseline F] [--debt]` loads the whole config (`ProjectConfig`, `CheckConfig`) and the baseline, walks the working tree, judges with `check::run` and the UTC date taken at start, prints the report and exits with its verdict. Nothing is written, no index refreshed, no git run; the mode is `[check] mode` (`enforce-introduced`: no base, so `enforce` + ``note: mode `enforce-introduced` has no base without --staged or --changed: judged as `enforce` ``, 2a.2 Q8). `--staged` reads the git index instead, `--changed` this tree, both adding the base and running git (`docs/canon/spec-check-git.md`); names, output, exit and determinism below hold for them too.

- **Baseline**: `<root>/.spec-debt.toml` when an entry of that name exists (a directory or a dangling symlink there cannot be read: cannot check); `--baseline F`, relative to the current directory, replaces it and must exist.
- **Cannot check** (W-2): after discovery every failure is a `cannot` cause of the printed report, exit 2 — the config unreadable (an unreadable `--config` is no discovery failure), not UTF-8 or invalid anywhere (a cause per distinct error, `<config>:<line>`); the baseline missing, unreadable or invalid; the root unreadable (cause `.`); the walk's causes (an unlistable directory, also on the way to a default root). A config error stops before the baseline, in mode `enforce` (the mode is read only when all of `CheckConfig` is valid). Only usage and discovery failures leave stdout empty.
- **Names**: the config as in pass 1 (`specengine.toml`, or `--config` as typed), `.spec-debt.toml`, `--baseline` as typed, the root `.`; no output holds an absolute path the caller did not type.
- **Text**: the lines of `Report::lines(--debt)`, one-lined, uncapped (W-4): blocking findings only, and a cut listing would contradict its counts. **JSON** (W-1): `Report::to_json()` + `\n` verbatim, whatever `--debt` (then one `note:`), paths raw — the exception to the CLI's "absent = `null`": an unset `fix`, an absent `debt` are omitted.
- **Exit**: 0 `clean`, `observed`; 1 `blocked`; 2 `cannot-check`, usage, discovery. No `spec:` line beside a report.
- **Determinism** ("one tree and one date"): one tree, config, baseline and date → byte-identical stdout and stderr, whatever the absolute root, walk order or starting directory.

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

Index sharding (`docs/features/roadmap.md` Q-8) before Phase 2 and a pilot's `spec export index`; then the pilots.

## Open

- The store walker's minors: store README "Open minors".
- A FIFO swapped in for the index before the open would block the writer (`O_NONBLOCK` later).
- Untested: the replaced-file refusal (needs a seam); the non-UTF-8 name warning on APFS, which refuses such names.
- Causes are sorted as strings (`:22` before `:6`; cosmetic, core); `ProjectConfig` stops at its first error, so a config's causes may be incomplete.
