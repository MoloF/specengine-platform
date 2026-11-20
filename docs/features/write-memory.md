---
class: spec
status: shipped
shipped: 2026-11-20
scope: [crates/specengine-eval, crates/specengine-store, crates/specengine-http, crates/specengine-mcp, crates/specengine-cli, scripts/deploy, docs]
ref: RM-0132; the analyst's breakdown of 2026-11-20; the owner's word of 2026-11-20 ("fix what remains, to the end"), on which the main session decided his open questions and, after the diagnosis, the judge and the fix; his stability rule (ADR-0086); plugin 0.49.2
adrs: [ADR-0086]
---

# write-memory: the daemon's memory under agent writes, soaked, diagnosed and fixed (RM-0132)

## Why

A hub serves up to ten people in parallel for weeks (his word of 2026-11-15), and his rule makes a leak a critical defect (ADR-0086, `docs/canon/soak-runs.md`). Two soaks pointed at the write path:
- the soak daemon's RSS went 5 → 96 MB with 8 MCP writers, against 9.8-13.7 MB with readers alone (GU16);
- the slope was 0.96 MiB/min (DL15) and 1.68 over GU16's second half;
- the live hub went 422 → 498 MB after 8 writes.

The slice built the write soak his rule asks for, told a leak from the allocator's plateau, fixed the cause and showed it with a clean re-run.

**Decided 2026-11-20**, by the main session on his word:
- the budget is 0.25 MiB/min for writes, and reads keep 0.5;
- ADR-0086 is recorded, with its canon;
- `MALLOC_ARENA_MAX=2` goes in the image;
- the Linux run is an owner's gate (WM10).

After the diagnosis:
- on macOS the quiet probe's live heap is judged;
- WM1's planted leak keeps at least 1 KiB per write;
- each binary is held to its own sources;
- the memo counts its real heap, and the root leaves D's key;
- WM7's test shows the plateau.

## Acceptance criteria

- [x] **WM1 [soak]** `crates/specengine-eval/tests/write_soak.rs`, `#[ignore]`d: 8 logins `ask_question` back to back for `SOAK_SECS` (≥ 300), judged as WM6. Every call answers; a request open 30 s runs `sample <pid> 5`; two runs; a breach exits non-zero. M: a per-process `Vec<String>` keeping 2 KiB per stored question.
  - Both runs were red: the live heap read 1.162 and 1.139 MiB/min, its thirds level at about 1.1, 2.03 KiB per write.
  - The footprint, judged −3.1 and −4.7, would have hidden it.
- [x] **WM2 [variants]** U, R and M were within budget in two runs each; H was reported only (WM8).
- [x] **WM3 [report]** Every key was present in both runs: the variant, seconds, calls per minute, `ms` spreads, samples, slopes (RSS, footprint, live heap) and budget, KiB per write, threads, fds and children, and the binary's path, size and mtime.
- [x] **WM4 [stale binary]** Each binary is held to its own workspace path dependencies' `src`, `Cargo.lock` and, for the daemon, `ui/dist`. An http-only touch refuses the daemon alone. M: `touch crates/specengine-http/src/main.rs` refuses; the old every-crate rule refuses `spec` forever: red.
- [x] **WM5 [cause]** "Cause" cites the runs of W, U, R and H, and the heap attribution by file:line.
- [x] **WM6 [budget]** `WRITE_BUDGET_MIB_PER_MIN` and `READ_BUDGET_MIB_PER_MIN` equal `soak-runs.md` "Budgets"' write-memory rows, compared by `wm6_the_budgets_are_the_canons`. M: 0.3 in the table: red.
- [x] **WM7 [fix]** `crates/specengine-store/tests/check_memo_heap.rs`, with a counting global allocator, fills the memo far past its cap.
  - At HEAD it was red: a D entry held 578 B real against 302 counted (spec-a), and 580 against 319 (plant-diary); a 128 KiB cap held about 245 KiB.
  - After the fix: 358 B real against 372 counted (1.04 by block, 1.18 by size asked), 360 B for both roots. The stdio server's two roots hold 640 → 1 280 B.
  - Mutations red: the old `findings_bytes`; the root back in the key; one memo for every root; the login out of the key (`wm7_d_keeps_the_login`).
- [x] **WM8 [clean re-run]** 300 s a run, the live-heap judge, MiB/min:
  - W: 0.0 and 0.0, at 593 and 627 writes a minute.
  - U: 0.056 [0.063, 0.056, 0.057] and 0.058, at 180 updates a minute: 0.32 KiB per update, the memo itself, until its cap (0.54 before).
  - M: 0.0 and 0.0, at 1 206 calls a minute (535 writes).
  - R: 0.0 and 0.0, at 17 276 and 17 532 reads a minute.
  - Threads were 1-3. The plateau is WM7's, not a 14 h soak.
  - M's RSS (53.5 and 51.4) and footprint (45.7 and 43.7) grew in its last third outside the live heap: RM-0134.
- [x] **WM9 [team of ten]** `hub_concurrency.rs`, the three `write_cost_golden.rs`, `check_memo`'s units and `check_memo_heap.rs` are green, 102 tests. M: D keyed without the login: `a_login_misses_another_logins_findings` red.
- [ ] **WM10 [Linux, owner's gate]** W in the hub's image, or the live hub's RSS under real use only, with no synthetic writes. The baseline is 254.4 MiB on 0.49.0 after 7 h (`docker stats`, 2026-11-20, started 03:58:37Z); the main session records it again after the 0.49.2 upgrade. Docker is off on the laptop by his choice. It blocks nothing.

## Out of scope

Write latency beyond the fix (RM-0126), concurrent trusted reads, card and page writes, shrinking the per-write step (transient), an allocator crate (a counting allocator inside a test is not one), a 14 h plateau soak, a narrower dedup, a daemon watchdog, and synthetic writes on the live hub.

## Cause

Measured at HEAD on 2026-11-20, 300 s a run, macOS.

- **W keeps nothing.** Over three runs, RSS judged 0.48-0.54, while the live heap stayed at 1 750 KiB at every reading over 3 150 stored. `heap -diffFrom` from minute 1 to the end found 0 survivors, and all 30 790 objects in flight were freed.
  - The step is each write's corpus load: cli `intake.rs:788` → `corpus.rs:52` → store `read.rs:335`/`:537` `indexed_input` → `read.rs:637-646` `decode_all`.
  - libmalloc keeps its pages: 1.8 MB allocated against 66.6 MB dirty.
- **U: the memo's D entries.** The live heap grew 0.092 MiB/min, level by third, 0.54 KiB per update. The growth was in `check_memo.rs:285`'s map: the key's root `PathBuf` (`check_rows.rs:120-121`, via `create.rs:827` ← `propose.rs:975`) and clones of the path and login. `findings_bytes` (`:377`) counted 2.7 times less, so the budget meant 85-120 MiB real.
- **R and H** were flat. `queue_matches` and the delegated apply left no survivors.
- **The judge.** A planted 2 KiB-per-write leak read 2.16 MiB/min on the live heap, but the footprint was red in only 1 of 5 runs, and RSS judged clean runs red.

## Implementation

### The fix

| Module | What it does |
|---|---|
| store `check_memo.rs` | the real heap by capacity, size classes (macOS; glibc's on `linux-gnu`), a node share, `D_ROOT`; boxed entries; D keyed without a root; `PerRoot`; `keep_findings` refuses past the budget |
| store `check_rows.rs`; mcp `read.rs` | D's key without a root; the stdio server's `PerRoot` |
| `scripts/deploy/Dockerfile` | `ENV MALLOC_ARENA_MAX=2` in the runtime stage |
| eval `write_soak.rs`, `library_common` | the soak, the quiet probe, the per-binary refusal, WM6 |
| tests | store `check_memo_heap.rs`; http `deploy.rs` (the arena pin); dead core `AsIs::bytes`, `found_bytes` removed |

**Canon:** `spec-check-write.md` "Memos"; `hub-ops.md` "Health"; `soak-runs.md` (the judge, the after-fix numbers, M's finding); the eval README; `plugin-files.md`.

**Deviations:**
1. Each stdio root has its own budget, so R roots hold up to R × 32 MiB.
2. A doubt in a stdio scope empties every root's D.
3. `warm` and `by_login` are not counted.
4. glibc's rounding is modelled, not measured (WM10).

### Release

Plugin 0.49.2, a version-only PATCH, tag `v0.49.2`; `INDEX_FORMAT` unchanged at 21.

The full run: 6 567 tests, all passed (123 skipped). `cargo clippy --workspace --all-targets -D warnings` and `spec check` are clean.

The review's five minors were applied:
- glibc's chunk model, `max(32, round16(size + 8))`, under `linux-gnu`;
- the dead core `AsIs::bytes` and `found_bytes` removed;
- full destructuring;
- a replaced entry counted by its stored key;
- notes.

WM7 after them: 358 B real against 372 counted on macOS.
