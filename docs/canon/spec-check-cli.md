---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-09-30
---

# The spec CLI: check and export index

CLI pass 2a.1 = `spec check` increment 3 part 1 (`docs/features/spec-cli-check.md`). Engine, config, findings, debt, verdict: `docs/canon/spec-check*.md`; discovery, globals, the one-line rule, exit codes: `crates/specengine-cli/README.md`. Both commands parse afresh through the store's check loader (store README): no database, slug or `HOME`. This repository registers nothing until 2b (Q-7): the index keeps `xtask`'s header, hook and CI stay on `xtask`.

## spec check

`spec check [--baseline F] [--debt]` loads the whole config (`ProjectConfig`, `CheckConfig`) and the baseline, walks the working tree, judges with `check::run` and the UTC date taken at start (no clock flag), prints the report and exits with its verdict. Nothing is written, no index refreshed; the mode comes only from `[check] mode`.

- **Baseline**: `<root>/.spec-debt.toml` when an entry of that name exists (a directory or a dangling symlink there cannot be read: cannot check); `--baseline F`, relative to the current directory, replaces it and must exist.
- **Cannot check** (W-2): after discovery every failure is a `cannot` cause of the printed report, exit 2 — the config unreadable (an unreadable `--config` is no discovery failure), not UTF-8 or invalid anywhere (a cause per distinct error, `<config>:<line>`); the baseline missing, unreadable or invalid; the root unreadable (cause `.`); the walk's causes (a directory on the way to a default root that cannot be listed: `cannot  docs: directory cannot be listed…`). A config error stops before the baseline, in mode `enforce` (the mode is read only when all of `CheckConfig` is valid). Only usage and discovery failures leave stdout empty.
- **Names**: the config as in pass 1 (`specengine.toml`, or `--config` as typed), `.spec-debt.toml`, `--baseline` as typed, the root `.`; no output holds an absolute path the caller did not type.
- **Text**: the lines of `Report::lines(--debt)`, one-lined, uncapped (W-4): blocking findings only, and a cut listing would contradict its counts. **JSON** (W-1): `Report::to_json()` + `\n` verbatim, whatever `--debt` (then one `note:`), paths raw — the exception to the CLI's "absent = `null`": an unset `fix`, an absent `debt` are omitted.
- **Exit**: 0 `clean`, `observed`; 1 `blocked`; 2 `cannot-check`, usage, discovery. No `spec:` line beside a report.
- **Determinism** ("one tree and one date"): one tree, config, baseline and date → byte-identical stdout and stderr, whatever the absolute root, walk order or starting directory.

## spec export index

A mode of `spec export` (owner, Q3; its bare form stays Phase 2's queue export, and bare `spec export` prints clap's help, exit 2): `render_index` with the `[[generators]]` entry `index = true` over the same fresh walk, written to `<root>/<[paths] index>` and nowhere else. The header names the registered `command` and `gate` (default `spec check`), never the binary; no default path, command or gate (ADR-0008). The baseline is not read.

- **Refused**, exit 2, empty stdout, nothing written: a config error (one `<config>:<line>: message` per cause); no `[[generators]]` or no `index = true` entry (naming the missing registration); an incomplete walk, `--stdout` included: core's `walk_gap` (§11.5's stop conditions), naming the first by path — an unreadable or unparsable file, an unlistable directory, a missing written root.
- **Writing**: every existing component of `[paths] index` below the root is a non-symlink directory, the file (if present) a non-symlink regular file, the parent exists (never created); else refused, a symlink's target untouched. Equal bytes → `unchanged`, the file not opened for writing (mtime kept). Else the file is opened, checked to be the one inspected (device and inode: a replaced file is refused), truncated and written in place, as `xtask`: no temp file, no guard against a hand-written file (git is the net); a failed write → exit 2.
- **Written anyway, with a `warning:`**: names skipped for not being UTF-8 (one per problem path, with its count; their documents are not listed); `[paths] index` outside the walk (`spec check` then reports `index-missing`).
- **Output**: `wrote docs/index.md: 9182 bytes` or `unchanged …`; JSON `{"path":"docs/index.md","bytes":9182,"written":true}`; `--stdout`: the render byte for byte, nothing written (W-3). `--stdout --json` is a usage error in any argument order, one message (`Usage: spec export index [OPTIONS]`). Read-only projects are unguarded: a pilot takes `--stdout`.

## Owner's and working answers

- Q3 (2026-09-30): the index writer is `spec export index`; class `generated` documents are written only by their registered generator (`docs/canon/architecture.md#apply`).
- Q4 (2026-09-30): "introduced" is relative to `HEAD` (2a.2 below); it answers `docs/canon/spec-check.md` Q-5.
- Working answers (the code) → the other answer's cost: W-1 JSON verbatim → a second serialiser, or a core change breaking `check_output.rs`; W-2 cannot-check prints its report → no JSON for the failure that matters most; W-3 `--stdout` → previewing means writing, forbidden in pilots; W-4 uncapped → `search`'s cap.

## Next: 2a.2 spec-cli-staged

Fixed now (Q4):

- `--staged` checks the root's git index: stage-0 regular blobs of `git ls-files -s -z`, read by one `git cat-file --batch`, under `WorkingTree`'s walk rules (symlinks `120000`, gitlinks `160000` skipped; a written root without tracked entries is missing); config and baseline from the index unless `--config`, `--baseline`. `--changed` checks the working tree.
- Base: `HEAD`'s tree (`git ls-tree -r -z HEAD`), checked with the checked tree's config, parses shared by (path, blob OID); unborn `HEAD` → empty. **Introduced**, for both flags: (code, path, subject) absent from the base's findings; a rename re-introduces, a second occurrence of a key does not.
- `mode = "enforce-introduced"` (observe → enforce-introduced → enforce, 04 §1.6) blocks on introduced errors not in live debt and on a baseline entry whose triple `HEAD`'s baseline lacks at that path: new debt needs the owner's `--no-verify`; a baseline outside the repository turns that off with a `note:`. `Mode`: a kebab-case rename.
- Git by `std::process` (05 §9), inheriting `GIT_INDEX_FILE`, `GIT_DIR` (hooks, `git commit -o`); never fetches; a missing blob, unmerged entries, no repository or `git` → cannot-check; the root may be a worktree subdirectory. JSON findings and counts gain `introduced`; `--debt` labels pre-existing errors.
- Open, the analyst's recommendations: expired debt on a pre-existing error blocks; a triple whose `expires` moved later is new; the stricter of the staged and `HEAD` mode applies; "no new baseline entries" also under `enforce` with a base; `enforce-introduced` without either flag → `enforce` + a `note:`. Too big → `--staged` first (all 2b needs). Test risks: the owner's git config, a `cat-file` pipe deadlock, intent-to-add, clean filters, submodules.

## Then: 2b spec-cli-switch

Owner's answers (2026-09-30):

- Q2: an ADR amending ADR-0023's table — `spec-writer` also `specengine.toml`, `.spec-debt.toml`; `rust-developer` also `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/`. `.claude/**` stays the owner's, who applies by hand the text 2b's spec-writer prepares: the two role prompts, the `*-saving` twins, ~43 `xtask` mentions in 14 files, the `Bash(cargo xtask docs *)` allow entry in `.claude/settings.json`.
- Q5: `spec check` reports the worst W in its summary and JSON counts (tests compare with the library, not literals). Q6: the hook runs `cargo run -q -p specengine-cli -- check --staged`.
- One commit: the root config registers `cargo run -q -p specengine-cli -- export index` and its gate; the index regenerated; hook and CI switched; `xtask` removed. CI (`enforce`, a `HEAD` checkout) runs the full check; `--base REF` (05 §5.2) later.

Elsewhere: the queue export, `--state` (Phase 2); applying `fix` data (Q-3); an MCP check tool; path arguments; a clock flag; the `rev` pre-commit rule (05 §3.5, Phase 3); project generators' drift (ADR-0013).

## Open

- The store walker (`update_paths` silently drops the rows under an unreadable directory, a Phase 2 item; `resolve`'s race): store README "Open minors".
- A FIFO swapped in for the index before the open would block the writer (`O_NONBLOCK` later).
- Untested: the replaced-file refusal (needs a seam); the non-UTF-8 name warning on APFS, which refuses such names.
- Causes are sorted as strings (`:22` before `:6`; cosmetic, core); `ProjectConfig` stops at its first error, so a config's causes may be incomplete.
