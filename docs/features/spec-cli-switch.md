---
class: spec
status: shipped
scope: [root, crates/specengine-core]
ref: 08 §2 Phase 1, CLI pass 2b = spec check increment 3 part 3; owner's answers of 2026-09-30 (Q2, Q5, Q6) and 2026-10-01 (1-8, one pipeline)
shipped: 2026-10-01
adrs: [ADR-0029]
---

# The spec CLI, pass 2b: spec check as the gate

## Why

The gate was `xtask`, a second implementation of the convention: every convention change was made twice (ADR-0028), the parity tests raced its cargo subprocess, and the product never judged its own repository. Parity held, so ADR-0013 handed the checks to `spec check`, with W, the one output it lacked, in its summary and JSON (Q5). The second role set went too (ADR-0029). G = `cargo run -q -p specengine-cli -- check`, X = `cargo run -q -p specengine-cli -- export index`, literal (answer 6). W_start (`5d97fd7`) = 115 643 B.

## Acceptance criteria

Each named mutation turns its criterion red. "The library": `specengine_core::check` on the same tree.

- [x] AC-01 ADR-0029 accepted, ≤ 1 536 B, `canon:` resolving, no `supersedes`; ADR-0023 accepted; `#process` and sole `/feature` as ADR-0029. Mutation: `canon:` → a missing anchor.
- [x] AC-02 G from the top: exit 0, `— clean`; `--json` errors, debt, expired, stale 0; `--debt` findings = the pin by (code, path, subject). Mutations: `mode = "observe"` (caught by the mode pin: observe does not un-clean a clean tree); an `[ids]` prefix matching prose.
- [x] AC-03 The walk = a std walk: every `*.md` outside `.`-named, `fixtures`, `target`, `target.noindex`, `node_modules`, `dist` directories, not `_*`. Mutations: `crates` out of `roots`; no `**/_*.md`; a new top-level directory with a README.
- [x] AC-04 X: `unchanged`, mtime kept; `X --stdout` = the file, header naming X and G. Mutation: a byte edited → exit 1, `index-drift` naming X.
- [x] AC-05 Each of the 30 seeds blocks its target with its code (plus the collateral `index-drift`/`ref-dangling` listed in `COLLATERAL`); unseeded clean; nothing written. Mutation: a seeded rule off.
- [x] AC-06 W on a crafted corpus equals the sum from its sizes. Mutations: Tier 3 counted; tier 1 summed; the index pooled; k = 2; generated counted; the failed file dropped.
- [x] AC-07 The summary ends `, worst W <n> B — <verdict>`, `counts.worst_w_bytes` last; n = the library's for plain, fully staged `--staged`, `observe`, both fixtures, this repository; `cannot-check` → 0. Mutations: the key omitted; a `"docs/index.md"` literal in the W code.
- [x] AC-08 Before deletion `cargo xtask docs budget` gave 115 680 B = the library's 115 680 B; the store's W oracle = `counts.worst_w_bytes`.
- [x] AC-09 At shipping G's W = 115 603 B ≤ 115 643 (5 436 + 9 576 + 8 526 + 92 065); `CLAUDE.md` 5 436 ≤ 5 572 B; all caps hold.
- [x] AC-10 Hook in scratch repositories through a logging `cargo` shim, cases (a)–(k): the exact call; a bad `.md` refused, a clean one committed; nothing relevant → no call; `specengine.toml` or `.spec-debt.toml` alone checked; a rename away and a symlink checked; 5 000 names, one bad, refused; shim exit 127 or 2 refused; `commit -a`; `--no-verify`; `100755`.
- [x] AC-11 `docs.yml` runs only `cargo run --locked -q -p specengine-cli -- check --root .`; `.github`, `.githooks` name neither `xtask` nor `sync-saving`. The owner's first push green: pending (owner).
- [x] AC-12 No `xtask` package, lock entry, alias or directory; `.cargo/config.toml` only `target-dir`; eight default members; `scripts/` only `hooks-install.sh` (`no_retired_package_alias_or_script_is_left`).
- [x] AC-13 `dogfood.rs` walks the root config's scope, no cargo subprocess; every file parses under {ADR, width 4}. Mutation: `crates` out of `roots`.
- [x] AC-14 `spec check --root <this repository>` from a scratch directory, scratch `HOME`: stdout = the library's; `git status` unchanged (outside `.claude/`, the owner's); nothing under `HOME`.
- [x] AC-15 `check_genre.rs`, cli `genre.rs` ban `cargo run -q -p specengine-cli`.
- [x] AC-16 `git grep -n xtask` over live documents and sources: ADR-0022, ADR-0029, `docs/specs/specengine-platform/README.md` (the pilots' own), Tier 3 specs, test ban lists and the registry test's fixture text.
- [x] AC-17 The `.claude` commands pass (the edits are in `7d2d093`); the write-area comparison with `CLAUDE.md#process`: the orchestrator's, stage 5.
- [x] AC-18 `docs/canon/architecture.md` names `spec check`; `docs/canon/spec-check.md` "Output" shows W and its pool rule; `docs/canon/spec-check-cli.md` "The gate here"; `docs/README.md` "Enforcement"; `X && G` green.
- [x] AC-19 `cargo nextest run --workspace` green (878 passed, 15 skipped); clippy, fmt clean; `git status` and the data directory unchanged.

## Implementation

One iteration; review accepted (no blocker or major). The truth moved to `docs/README.md` "Enforcement" (root config, roots rule, hook, merges, CI), `docs/canon/spec-check.md` (W), `docs/canon/spec-check-cli.md`, `crates/specengine-core/README.md`.

| Module | What it does |
|---|---|
| core `check/working_set.rs` | `worst_w(&CheckInput, &Paths) -> u64`: canon tier 0 summed + largest tier 1 + index + 3 largest of the pool (every other file not tier 0/1, Tier 3 or generated; failed front-matter in); no path literal |
| core `check/{mod,report,engine}.rs` | re-export; `Counts.worst_w_bytes` last, 0 on cannot-check; summary `, worst W <n> B` |
| `specengine.toml` (new) | roots, excludes, `[ids]`, caps, closed contracts, `enforce`, X with gate G |
| `.githooks/pre-commit` | bash 3.2: names via `--no-renames --diff-filter=ACDMT -z` collected whole; `.md` or top-level TOML → `check --staged --root .`; any non-zero refuses |
| `.github/workflows/docs.yml`, `scripts/hooks-install.sh` | the check alone; installer comment |
| `Cargo.toml`, `Cargo.lock`, `.cargo/config.toml` | `members = ["crates/*"]`, eight default members; `xtask` entry and alias out |
| deleted | `xtask/`, `scripts/sync-saving-agents.sh` |
| tests | store `check_parity.rs`, `parity_config/mod.rs` (root config, std walk, 30 seeds, W oracle); cli `parity.rs`, `worst_w.rs`, `pre_commit_hook.rs`; core `check_worst_w.rs`, `dogfood.rs`, genre bans; eval `build_graph.rs` |

Deliberate: no twin check in the hook. Rules left in canon: a new top-level directory or `.md` file is unwalked until listed in `roots` (the task creating `ui/`, `plugin/` or `AGENTS.md` adds it). Risks: one implementation left (seeds and oracles are the net); the hook builds from the working tree (R11: a broken tree refuses, `--no-verify`); a clean `git merge` runs `pre-merge-commit`, so merged documents are judged by CI alone. The five-digit mention in the `spec-check.md` file-name side-remark went, so this repository has no warning left.
