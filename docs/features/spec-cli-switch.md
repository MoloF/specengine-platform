---
class: spec
status: in-progress
scope: [root, crates/specengine-core]
ref: 08 §2 Phase 1, CLI pass 2b = spec check increment 3 part 3; owner's answers of 2026-09-30 (Q2, Q5, Q6) and 2026-10-01 (1-8, one pipeline)
adrs: [ADR-0029]
---

# The spec CLI, pass 2b: spec check as the gate

## Why

The gate is `xtask`, a second implementation of the convention: every convention change is made twice (ADR-0028), the parity tests race its cargo subprocess, and the product never judges its own repository. Parity holds, so ADR-0013 hands the checks to `spec check`, with W, the one output it lacks, in its summary and JSON (Q5). Answers: `docs/canon/spec-check-cli.md` "Next: 2b" (2026-09-30); the analysis's eight recommendations, "answer 1–8" (2026-10-01). One commit, by the owner; the only product change is W in the core. The second role set goes too (ADR-0029).

## Description and interactions

G = `cargo run -q -p specengine-cli -- check`, X = `cargo run -q -p specengine-cli -- export index`: literal, no alias (answer 6).

### Order of work

1. **2a** (old prompts): ADR-0029, `CLAUDE.md#process`, this spec; green on `xtask`. W_start (`5d97fd7`) = 5 572 + 9 576 + 8 427 + 92 068 = 115 643 B.
2. **Owner checkpoint**, the only pause (answer 1): apply the `.claude` edit list, run its commands, restart Claude Code (the role registry reloads). An old prompt would rerun `xtask` and undo the switch.
3. **2b** (spec-writer, new prompt): the root `specengine.toml`; `xtask/README.md` deleted; in `CLAUDE.md` the command becomes `X && G`, the `xtask/` layout line goes (5 411 B); `X && G` → `clean`. From here **no role runs `xtask`**, but AC-08's `cargo xtask docs budget`.
4. **3** (≤ 3 iterations). Developer: W in core `src/check/` (`Counts`, `Report::lines`, `to_json`; no CLI change); `xtask/` deleted, out of the workspace and `Cargo.lock` (ADR-0029); the `.cargo/config.toml` alias out; hook, installer comment, CI; `scripts/sync-saving-agents.sh` deleted; the comment at `crates/specengine-import/src/frontmatter.rs:4`. Test-engineer, iteration 1: the migrations. Until then `parity_config::skip_dirs()` (it reads `xtask`'s source) and the parity tests are red, those comparing the index from 2b on: expected, not developer defects.
5. **4–5**: review; the spec-writer removes every live `xtask` mention (AC-16: `CLAUDE.md`, `docs/README.md`, `README.md`, `docs/canon/*`, four crate READMEs, 08 §2; the pinned five-digit mention goes with its pin), adds W to `spec-check.md` "Output", ships; the orchestrator runs G and AC-17 itself (its `/feature` text may still say `xtask`).
6. The owner commits once, `git add -A` (three new files).

`xtask` → after 2b: `docs check` → G; `docs index` / `--write` → `X --stdout` / X; `docs budget` → W in G (its size table dropped); `GENERATORS` → `[[generators]]`; `SKIP_DIRS` → `roots`, `exclude`.

Migrations: store `check_parity.rs`, `parity_config/mod.rs` read the root config (AC-02–AC-05, AC-08; the `xtask/README.md` seeds move to another Tier 1 README, the `generator-path` seed uses X); cli `parity.rs` copies it (AC-04, AC-14 without `--config`); AC-07, AC-12, AC-13, AC-15 name the rest; `check_config.rs` `PLAIN` gains X and G.

### Hook and CI

- Hook: `cargo run -q -p specengine-cli -- check --staged --root .` (answer 4) WHEN the staged names (`--no-renames --diff-filter=ACDMT`; T beyond answer 3: an `.md` made a symlink leaves the walk) include a `.md` file or the top-level `specengine.toml` or `.spec-debt.toml`; otherwise no cargo call. Any non-zero exit SHALL refuse (toolchain missing, build failure, 1, 2), naming X; `:15–20` go, `100755` stays. Fix the trigger: `grep -q` on a `pipefail` pipeline (`:5, :7`) returns 141 on a long list and skips the check; rename detection (`:7`) hid `.md` → non-`.md`.
- R11: `cargo run` builds from the working tree, not the staged one; warm 0.3–1 s, cold 1–3 min. An unstaged `specengine.toml` → exit 2, so no test runs `--staged` here.
- Installer: the comment. CI (answer 5): `cargo run --locked -q -p specengine-cli -- check --root .` on `HEAD`, its only check (`:12–13` replaced), no cache.
- Fail closed fits ADR-0006, ADR-0012 (content, the task gate): only form errors the convention makes fatal block (ADR-0022); `--no-verify` stays.

### The .claude edit list (owner, at the checkpoint)

Under `.claude/`, lines as of `5d97fd7`, verified 2026-10-01: 30 lines in 8 files, 6 deleted; `-` is the exact fragment, `+` its replacement.

```text
Rule 1  `cargo xtask docs check` -> `cargo run -q -p specengine-cli -- check`: agents/spec-writer.md 16 99,
        test-engineer.md 16, code-reviewer.md 45; commands/feature.md 51 86 90 97.
        Unquoted, cargo xtask docs check -> spec check: spec-writer.md 3, test-engineer.md 3, feature.md 2.
agents/spec-writer.md
 28 - `docs/`, `xtask/`,        + `docs/`,
 78 whole line: cargo run -q -p specengine-cli -- export index && cargo run -q -p specengine-cli -- check
 81 - commands, `cargo xtask docs budget` and `git diff`/`git status`.
    + commands and `git diff`/`git status`; the worst W is on the check's summary line.
 84 - `CLAUDE.md` and `*/README.md`.** Code, tests and configs
    + `CLAUDE.md`, `*/README.md`, `specengine.toml` and `.spec-debt.toml`; `.claude/` is the owner's: put its text in the spec.** Code, tests and other configs
 99 after Rule 1, before the final ".": + " (the summary, with the worst W)"
agents/rust-developer.md
  3 - xtask/src, Cargo manifests.
    + Cargo manifests, and the documentation gate's hook, CI, scripts and cargo config.
 47 - `xtask/src/`, `plugin/`.**
    + `plugin/`, `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/`.** `.claude/` is the owner's.
 48 - documentation in `docs/`, `CLAUDE.md` or `*/README.md`:
    + documentation or its gate's config (`docs/`, `CLAUDE.md`, `*/README.md`, `specengine.toml`, `.spec-debt.toml`):
 50 - `xtask/tests/`, `fixtures/`  + `fixtures/`
agents/test-engineer.md
  3 - crates/*/tests and xtask/tests for  + crates/*/tests for
 49 - over `crates/*/src` is
    + over the developer's area (`crates/*/src`, `.githooks/`, `scripts/`, `.github/workflows/`, `.cargo/`) is
 52 - `xtask/tests/`, `fixtures/`  + `fixtures/`
agents/code-reviewer.md
 44 - keep out of `docs/`? Run
    + keep out of `docs/` and `specengine.toml`, and every role out of `.claude/`? Does a hook, CI, `scripts/` or `.cargo/` change weaken the gate, or a `.spec-debt.toml` entry hide a fixable error? Run
agents/requirement-analyst.md
 24 - `crates/`, `xtask/`.
    + `crates/`; the documentation gate — `specengine.toml`, `.githooks/`, `.github/workflows/`.
 60 - `xtask`, `ui`                + `ui`, the gate (hook, CI)
agents/ui-developer.md
 45 - `crates/**`, `xtask/**`,     + `crates/**`,
commands/feature.md 24
    - `xtask/`, `plugin/`, Cargo manifests
    + `plugin/`, Cargo manifests, `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/`
commands/feature.md 97
    after Rule 1, before the final ";": + " (its summary, with the worst W)"
settings.json
  8 - "Bash(cargo xtask docs *)",
    + "Bash(cargo run -q -p specengine-cli -- check)",
      "Bash(cargo run -q -p specengine-cli -- check *)",
      "Bash(cargo run -q -p specengine-cli -- export index)",
      "Bash(cargo run -q -p specengine-cli -- export index *)",
 12 15 deleted: "Bash(./scripts/sync-saving-agents.sh --check *)", "Bash(./scripts/sync-saving-agents.sh --check)"
 14 - "Bash(git diff)",  + "Bash(git diff)"
Deleted: agents/*-saving.md (5), commands/feature-saving.md
```

Then, from the top:

```bash
git grep -n xtask -- .claude  # no output
git grep -n -i saving -- .claude  # no output
ls .claude/agents/*-saving.md .claude/commands/feature-saving.md  # none: an error
python3 -m json.tool .claude/settings.json >/dev/null  # parses
git diff --stat -- .claude  # 14 files: 8 modified, 6 deleted
```

## Data

### The root specengine.toml

```toml
# This repository's documentation gate: spec check (ADR-0013, ADR-0022; docs/canon/spec-check-cli.md).
[paths]
roots      = ["CLAUDE.md", "README.md", "crates", "docs"]
records    = "docs/decisions"
tier0      = "CLAUDE.md"
tier1_name = "README.md"
index      = "docs/index.md"
exclude    = ["**/_*.md", "**/fixtures/**", "**/target/**", "**/target.noindex/**", "**/node_modules/**", "**/dist/**"]

[ids]
ADR = { kind = "decision", width = 4 }

[budgets]
tier0_bytes    = 16384
tier1_bytes    = 10240
index_bytes    = 10240
decision_bytes = 1536
canon_bytes    = 12288

[classes]
canon     = { required = ["class", "tier", "scope", "owner", "reviewed"], closed = true }
decision  = { required = ["class", "id", "title", "status", "date", "scope"], optional = ["canon", "supersedes", "ref"], closed = true }
spec      = { required = ["class", "status", "scope"], optional = ["ref", "shipped", "adrs"], closed = true }
generated = { required = ["class", "generator", "source"], closed = true }

[check]
mode = "enforce"

[[generators]]
command = "cargo run -q -p specengine-cli -- export index"
writes  = ["docs/index.md"]
index   = true
gate    = "cargo run -q -p specengine-cli -- check"
```

Checked against the core's readers: no correction. Versus `parity_config/mod.rs`: no `scripts` (no `.md`) or `xtask` root, no dot-directory excludes (never walked below a root), X and G registered; `roots` cannot hold `.`. Expect `clean`, one warning (the five-digit mention in `docs/canon/spec-check.md` "Rules").

### Worst W

Over the walked files as read (staged blobs under `--staged`; documentation-system §3): W = Σ canon `tier: 0` + the largest canon `tier: 1` + the `[paths] index` file (0 if not walked) + the three largest other files that are neither Tier 3 (the index render's predicate) nor `class: generated` (answer 2), failed front-matter included. k = 3, no key; `cannot-check` → 0; a pure public function, no path literal (ADR-0008). Answer 8 (numbers illustrative):

    spec check [enforce]: 131 documents, 0 errors, 1 warnings, 0 debt, 0 expired, 0 stale, worst W 115643 B — clean
    "counts":{"documents":131,…,"stale":0,"worst_w_bytes":115643}

Not a check (answer 7): no cap, no warning.

## Rules and edge cases

- WHEN a new top-level directory holds documents, it SHALL stay unwalked until listed in `roots` (AC-03).
- Assumption: `.claude/**` is checked by commands, not by a test.
- Risks: one implementation left (seeds, oracles); R11 on a broken working tree (`--no-verify`); old prompts; a half-switched tree (revert, delete the three new files); build cost; moving pins; ADR-0029's cost.

## Acceptance criteria

Each named mutation turns its criterion red. "The library": `specengine_core::check` on the same tree.

- [ ] AC-01 ADR-0029 accepted, ≤ 1 536 B, `canon:` resolving, no `supersedes`; ADR-0023 accepted; the `#process` cells and sole `/feature` as ADR-0029. Mutation: `canon:` → a missing anchor.
- [ ] AC-02 G from the top: exit 0, `— clean`; `--json` errors, debt, expired, stale 0; `--debt` findings = the pin by (code, path, subject). Mutations: `mode = "observe"`; an `[ids]` prefix matching prose.
- [ ] AC-03 The walk = a std walk: every `*.md` outside `.`-named, `fixtures`, `target`, `target.noindex`, `node_modules`, `dist` directories, not `_*`. Mutations: `crates` out of `roots`; no `**/_*.md`; a new top-level directory with a README (scratch).
- [ ] AC-04 X: `unchanged docs/index.md: <n> bytes`, mtime kept; `X --stdout` = the file, header naming X and G; `git diff HEAD -- docs/index.md`: the header, 2b's and re-scoped lines. Mutation: a byte edited (scratch) → exit 1, `index-drift` naming X.
- [ ] AC-05 Each of the 30 seeds (scratch) blocks exactly its target with its code; unseeded clean; nothing written. Mutation: a seeded rule off.
- [ ] AC-06 W on a crafted corpus, expected from sizes: a canon tier 0; two canon tier 1; an index larger than the third live Tier 2; a shipped spec and a superseded decision larger than any live file; a generated non-index file; the largest live file with failed front-matter. Mutations: Tier 3 counted; tier 1 summed; the index pooled; k = 2; generated counted; the failed file dropped.
- [ ] AC-07 The summary ends `, worst W <n> B — <verdict>`, `counts` with `worst_w_bytes`; n = the library's for plain, fully staged `--staged`, `observe`, both fixtures, this repository; `cannot-check` → 0. Mutations: the key omitted; a `"docs/index.md"` literal in the W code.
- [ ] AC-08 Before `xtask/` goes, `cargo xtask docs budget`'s W = the library's, both in "Implementation"; the store's W oracle = `counts.worst_w_bytes`.
- [ ] AC-09 At shipping G's W ≤ 115 643 B, `CLAUDE.md` ≤ 5 572 B, all caps hold. Mutation: `CLAUDE.md` past its margin.
- [ ] AC-10 Hook in scratch repositories (scratch `HOME`, `GIT_CONFIG_GLOBAL=/dev/null`, the real hook with its mode, a logging `cargo` shim first on `PATH` exec-ing `CARGO_BIN_EXE_spec` with the arguments after `--`); mutations in brackets: (a) the shim gets `run -q -p specengine-cli -- check --staged --root .` [no `--root .`]; (b) a staged bad `.md` refused, `HEAD` kept, a clean one committed; (c) nothing relevant staged → no call [unconditional]; (d) only `specengine.toml` staged with a cap below a document, or a bad `.spec-debt.toml` → refused [`.md` only]; (e) `git mv a.md a.txt` checks [renames on]; (f) 5 000 staged `.md`, one bad → refused [`grep -q` on a `pipefail` pipeline]; (g) shim exit 127 or 2 → refused [`|| true`]; (h) `commit -a`, the error only on disk → refused; (i) `--no-verify` commits; (j) `100755` in tree and index [`chmod -x` reddens (b)]; (k) `a.md` made a symlink checks [`--diff-filter=ACMD`].
- [ ] AC-11 `docs.yml` runs only `cargo run --locked -q -p specengine-cli -- check --root .`: `git grep -n "xtask\|sync-saving" -- .github .githooks` is empty; the owner's first push is green.
- [ ] AC-12 No `xtask/`, no `xtask` in `cargo metadata --no-deps` or `Cargo.lock`; `.cargo/config.toml` keeps only `target-dir`; `cargo xtask` fails; `build_graph.rs` expects eight default members (mutation: `xtask` back). `scripts/` holds only `hooks-install.sh`; `git grep -n -i "feature-saving\|sync-saving"`: only the records ADR-0023, ADR-0029 and Tier 3 documents (mutation: the script back).
- [ ] AC-13 `dogfood.rs` walks the root config's scope, no cargo subprocess, no `xtask`; every walked file parses under {ADR, width 4} without front-matter diagnostics. Mutation: `crates` out of `roots` fails its count bound.
- [ ] AC-14 `spec check --root <this repository>` from a scratch directory, scratch `HOME`: exit 0, stdout = the library's; `git status` (no optional locks, untracked files listed) unchanged; nothing under `HOME`.
- [ ] AC-15 `check_genre.rs`, cli `genre.rs` ban `cargo run -q -p specengine-cli`. Mutation: a default generator with it in core or CLI.
- [ ] AC-16 At shipping `git grep -n xtask` over live documents and sources: only ADR-0022, ADR-0029, `docs/specs/specengine-platform/README.md` (the pilots' `xtask`), test ban lists.
- [ ] AC-17 (owner; run in stage 5) The `.claude` commands above pass; the role prompts' write areas match `CLAUDE.md#process`.
- [ ] AC-18 `docs/canon/architecture.md` names `spec check` as the enforcement; `docs/canon/spec-check.md` "Output" shows W; `docs/canon/spec-check-cli.md` describes what shipped; `X && G` green.
- [ ] AC-19 `cargo nextest run --workspace` once, green; clippy, fmt clean; the run leaves this repository's `git status` and `$HOME/Library/Application Support/specengine` unchanged.

## Out of scope

`--base REF` (`spec-cli-introduced`); a W cap, the size table; `[project]`, a CI cache; pilots; generator drift (ADR-0013); editing `.claude/**`; rewriting Tier 3 or ADR-0022.

## Implementation

Filled in after implementation.
