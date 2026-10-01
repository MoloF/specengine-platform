---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
ref: 08 §2 Phase 1, spec check increment 3 part 4
shipped: 2026-10-01
---

# spec check: enforce-introduced

## Why

A backlogged pilot (08 §4.3, §4.2 item 3): `enforce` refuses each documentation commit until fixed or baselined, `observe` lets agents add errors. `enforce-introduced` (04 §1.6) blocks only what a commit adds, with new debt and the stricter mode, else a baseline entry or `observe` silences an error.

Owner's answers: 2a.2 Q4–Q9 (2026-09-30; Q9 no ADR); Q1–Q4 (2026-10-01): Q1 `--changed` is the next task, `spec-cli-changed`, both before pilot agents use `enforce-introduced`; Q2 no `--base REF`; Q3 a `--baseline`, `--config` outside the root counts as outside the repository; Q4 `HEAD`'s unreadable baseline lifts the new-debt rule. How it works now: `docs/canon/spec-check-git.md` "The base", `docs/canon/spec-check.md`.

## Acceptance criteria

Cli tests unless core or store is named; 2a.2's scratch repositories (isolated git config, HEAD committed with `--no-verify`); copied `spec-a`, `spec-b` as a backlog; `--staged`, `enforce-introduced` unless stated; never git here. M: the mutation turning it red; (m): where the mutation run needed more.

- [x] AC-01 (core) `"enforce-introduced"` ↔ `EnforceIntroduced` (TOML, JSON); `Observe < EnforceIntroduced < Enforce`; default `enforce`; an unknown mode → a cause at its line naming all three. M: `rename_all = "lowercase"`; declared `Observe, Enforce, EnforceIntroduced`.
- [x] AC-02 No base → today's bytes: `check_output.rs`, cli `check.rs`, `worst_w.rs`'s plain tests, `parity.rs`, `exit.rs`, `determinism.rs` pass unmodified; plain JSON lacks `introduced`, `new_debt`. M: `introduced` serialised as `null` (m: caught by the new plain-output test, not by the unmodified list: CLI and library print it alike).
- [x] AC-03 (2a.2 Q8) Plain `enforce-introduced`, a blocked fixture → `[enforce]`, exit 1, enforce's lines, one `note:`, also without `git` on `PATH`; core `verdict_in(EnforceIntroduced)` without a base == `verdict_in(Enforce)`. M: `introduced.unwrap_or(false)`; plain running git.
- [x] AC-04 HEAD: E1 in `a.md`; staged: E1, E1's key again on another line, a new E2 → exit 1, one `error` line (E2); `introduced` `true` on E2, `false` on both E1-keyed, `counts.introduced` 1. M: `line` in the key; `subject` dropped; matching by count per key.
- [x] AC-05 `git mv` of a document with a pre-existing error → exit 1 at the new path; an NFD index entry (`update-index --index-info`) over HEAD's precomposed name, a feature document moved between features → introduced. M: path out of the key; parses shared by OID alone.
- [x] AC-06 No commit → blocking lines == `enforce`'s, exit 1, no cause or note; `--root proj` new in a born HEAD's index → the same; `proj` committed with only pre-existing errors → exit 0. M: `HEAD:<prefix>` (exit 2); unborn read as a git failure; `--full-tree` or `--full-name` (all introduced).
- [x] AC-07 (a) A written root absent at HEAD → no cause, its findings introduced; (a′) a partial base fails closed: a listed HEAD document whose blob is missing (HEAD's loose blob of a changed `a.md` deleted, staged `a.md` dropping an ID `b.md` cites), whose OID names a non-blob, or whose blob was not read → exit 2 in the checked mode, one cause at its path: `HEAD's blob is missing from the git object database`, `HEAD's object is not a blob`, `HEAD's blob was not read`; the checked run's causes merged in, no findings, no notes; a (path, OID) the index shares is the checked run's own cause, reported once; (b) HEAD's tree object deleted → exit 2, one fixed cause at `.` naming `git ls-tree`; no git text or absolute path. M: base causes merged into a judged report; the old (a′) rule, the blob read as unreadable: `a.md`'s own findings introduced while `b.md`'s dependent finding became pre-existing (exit 0); an `ls-tree` failure read as an empty base (m: the unreachable `None` arm turns red only with the pre-scan reverted: a backstop).
- [x] AC-08 A pre-existing error without debt: no block, listed only with `--debt`, ends ` (pre-existing)`; with live debt: debt; with expired debt: blocks (2a.2 Q4); an introduced error under HEAD's existing entry: no block. M: `blocks` as `mode >= EnforceIntroduced && blocks_when_enforced()`; expired debt over a pre-existing error passing.
- [x] AC-09 Against HEAD's baseline: an added triple → exit 1 under `enforce-introduced` and `enforce`, one `new` line naming it; `expires` moved later blocks; moved earlier, `reason` changed, an entry removed do not; unborn HEAD → every staged entry new. M: `>=` for "later"; `reason` compared; the rule only under `enforce-introduced`.
- [x] AC-10 (2a.2 Q6) HEAD/staged `enforce`/`observe` with an error → `[enforce]`, exit 1, one note; `enforce-introduced`/`observe` → `[enforce-introduced]`; `observe`/`enforce` → no note; HEAD's config invalid TOML, a symlink, a missing blob → staged mode, one note; none → no note. M: staged mode alone (2a.2 Q3); an invalid HEAD config → exit 2.
- [x] AC-11 (Q3) `--baseline` outside the repository, or inside but outside `--root`, holding entries HEAD lacks → no block, one note; under the root → compared with HEAD's blob at its path; `--config` outside the root, HEAD `enforce`, given `observe` → `[observe]`, one note. M: outside files compared with HEAD's root ones.
- [x] AC-11b (Q4) HEAD's `.spec-debt.toml` a symlink, a missing blob, invalid TOML → rule lifted, one note, no block from new entries, no `new_debt` key. M: strict (every entry new).
- [x] AC-12 A logging `git` wrapper: only the five subcommands; `ls-tree -r -z <hex>` once in the root iff HEAD is born; one `cat-file --batch`; `-c core.fsmonitor=false` and the four variables on every call; each OID requested once, none for an unchanged path's HEAD blob. M: a second session for HEAD; HEAD blobs requested for unchanged paths (m: an equivalent mutant while the OID cache dedupes; red with the cache bypassed).
- [x] AC-13 2a.2's AC-07, AC-08, AC-12 with errors at HEAD: a linked worktree → its HEAD; the guard refuses as before; `.git/index` bytes, mtime kept, no lock; objects, refs, `HEAD` unchanged; nothing under scratch `HOME`; TAB/LF names, sha256 work. M: `GIT_DIR` dropped for the base calls (m: caught with a separate git dir, not in a linked worktree); `ls-tree` without `-z`.
- [x] AC-14 (store, parse counter) N unchanged, 1 changed, 1 renamed file → the base parses exactly 2. M: the base parsed afresh.
- [x] AC-15 ≥ 3 000 committed documents (≥ 3 MiB) and one of 1 MiB, 10 changed and staged → exit within 60 s under a watchdog, the expected report. M: HEAD requests written before any reply is read (m: red only with every document changed; 10 changed fit in the pipe).
- [x] AC-16 (determinism: same trees, config, baseline, date) With a base `counts.introduced` precedes `worst_w_bytes`, the summary ends `, worst W <n> B — <verdict>`; two repositories at different absolute paths, files added in opposite orders → identical stdout, stderr. M: `introduced` after `worst_w_bytes`.
- [x] AC-17 (hook, 2b AC-10 harness) A backlog at HEAD: a clean edit commits; an introduced error, a `git mv` of a document with errors, a new baseline entry (also under `enforce`) are refused; `--no-verify` commits, then a clean `--amend` commits (pins R1); this repository stays `enforce`, the hook command unchanged. M: the hook command changed.
- [x] AC-18 Store and cli `genre.rs` cover the new code. M: a `"docs/"` literal in the base code.
- [x] AC-19 A1, the changed existing tests by answer: 2a.2 Q4, Q6 and the base fields — cli `common/staged.rs` `assert_parity`, `assert_same` (base fields set aside when exactly one run is staged), `library_staged` (`check_staged` == `check_staged_with_notes(…).report`, notes taken), and with them `staged.rs` 3 tests, `staged_git.rs` 9, `staged_load.rs` 1, `worst_w.rs` `staged_reports_the_library_s_w_of_the_staged_blobs`; Q7 new debt — `staged.rs` `clean_repo` and 3 tests, `staged_git.rs` 3 (`nothing_is_written_for_any_verdict` also commits its `observe` config, Q6), `pre_commit_hook.rs` `c_d_…` (a good `.spec-debt.toml` alone refused); Q4–Q9 — `enforce_introduced_is_a_cause_at_its_line` (now valid); AC-18 — store `genre.rs` `the_literal_scan_covers_the_git_module`. `git diff --stat crates/*/tests` shows no other file.
- [x] AC-20 The gate passes; at shipping W = 115 675 B ≤ 115 797 B; caps hold; this spec < 15 732 B; net growth ≤ 0 for `CLAUDE.md`, 04, 05, 07, 08, `docs/canon/spec-check.md`, the three READMEs; no canon line keeps "no `HEAD`", 2a.2 Q3's observe rule or unqualified `--staged` byte parity.
- [x] AC-21 `cargo nextest run --workspace` once: 923/923; clippy, fmt clean; `build_graph.rs`: no new normal dependency. M: `sha1` in the store (m: an extra workspace dependency stood in; `sha1` needs the network).

## Implementation

Three iterations; review accepted iteration 3, nothing blocking. The truth moved to `docs/canon/spec-check-git.md` ("The staged check", "The base", "Next"), `docs/canon/spec-check.md` (types, verdict, output), the three crate READMEs, `docs/README.md` "Enforcement".

| Module | What it does |
|---|---|
| core `check/config.rs` | `Mode` kebab-case, declared `Observe, EnforceIntroduced, Enforce` (= `Ord`), `Mode::ALL`; an unknown mode names all three |
| core `check/report.rs` | `Finding.introduced`, `Counts.{introduced, new_debt}`, `Report.new_debt` (`NewDebt` = entry + `head_expires`), omitted when `None`; `Finding::blocks_in`, `Report::verdict_in` (blocked → observed → clean), `without_base`; ` (pre-existing)`, `new` lines, the summary's base counts |
| core `check/base.rs` (new) | `Base {findings, baseline?, mode?}`; pure `judge`: key membership, new debt, the stricter mode |
| core `engine.rs`, `generated.rs`, `graph.rs`, `mod.rs` | `introduced: None`; re-exports |
| store `git.rs` | `Git::head` (`rev-parse --verify -q HEAD`), `Git::ls_tree`, `parse_ls_tree`; `Staged.objects` caches every object read: no OID requested twice |
| store `source.rs` | `IndexWalk`, `GitIndex::from_walk`, `take_object` |
| store `base.rs` (new) | `Placed` (inside the root by canonical path, or outside); `Head::read` (HEAD's mode, baseline), `notes`; `HeadWalk::read` (every listed OID), `findings` (pre-scan for a partial base before any parse; parses shared by (path, OID)) |
| store `check.rs`, `lib.rs` | `GivenFile`, `StagedCheck`, `check_staged_with_notes`; `check_staged` report-only, given files of unknown location |
| cli `check.rs`, `main.rs` | given files with their paths; notes as `note:`; plain `enforce-introduced` → one note + `without_base()`; help |
| tests | core `check_introduced.rs`; cli `introduced{,_git,_load,_hook}.rs`; store `base.rs` `mod base_parses`, `mod base_partial`; A1 |

Deliberate, beyond the draft: a partial base fails closed for a non-blob or unread object too (the orchestrator's ruling after the review found a false clean), HEAD's config and baseline staying notes; notes name a given file by its flag as typed and an invalid HEAD file by its line only; `new` lines sit after `stale`; ` (pre-existing)` also on blocking lines; HEAD's baseline is read even without a checked one; public extras `Mode::ALL`, `Report::new_debt_blocks`, `Finding::is_pre_existing`.

Residue (canon "Open"): a corrupt HEAD ref reads as unborn; a `--config` inside the root at a path HEAD lacks: checked mode, no note; a HEAD document that makes the parser panic is dropped from the base with its cause, the old (a′) mechanism, needing a parser bug and a commit past the gate (the owner may rule it a partial base).
