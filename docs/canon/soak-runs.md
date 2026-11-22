---
class: canon
tier: 2
scope: [architecture, crates/specengine-eval, crates/specengine-http, scripts/deploy]
owner: owner
reviewed: 2026-11-22
---

# Soak runs: crashes, hangs and leaks against written budgets

The owner's stability policy (ADR-0086), how a soak measures it in this repository, and every soak's budget. Its rule line is `architecture.md#stability`.

## The rule

- A crash, a hang or a leak is a critical defect, ranked above any logic bug. Work that causes one is not done, and an open finding blocks the acceptance of whatever it touches. That is the pipeline's acceptance, never a user's action: nothing in SpecEngine refuses a call on it (ADR-0012).
- Long-running work, the daemon and a hub, gets a soak: an automated run that drives it for minutes and writes a report. Its budgets are written down here. A breach fails the run with a non-zero exit.
- Memory growth is a robust slope: Theil-Sen, the median of the slopes of every pair of samples, over the last two thirds of the run. It is never two endpoints or a least-squares line: one transient 100 MB dip flipped a least-squares verdict between two runs of the same build. Memory is judged only on runs of 5 minutes or more, since loading can creep past the warm-up.
- A hypothesis is tested with at least two runs per variant. One run catches randomness, and once blamed the wrong cause.
- A watchdog captures a stack when nothing progresses: here, a request open for 30 s runs `sample <pid> 5` (macOS).
- Each finding is a task, a plan item in `docs/roadmap/`, with the report as its evidence. The fix is targeted and is accepted by a clean re-run: the same variant, two runs.
- A change to what a soak covers (the daemon's read or write path, a cache or memo, the turn, the hub's image) runs that soak before it is handed over, and its report goes with the task.

## How a soak runs here

- **Harness.** `specengine-eval`'s `#[ignore]`d tests over `library_common`. Each runs DC-11's tree at `--nodes 10000`, with 10 000 commits, and the release `specengine-http` as a hub over it (`--public-origin`, `--port 0`, `--slow-ms 0`, a scratch `HOME`). The hub lives under `perl -e 'alarm N'`, N past the run, and is killed and reaped on drop, so nothing outlives the test.
- **Runs.** `SOAK_SECS` defaults to 300 and is never less; a smoke setting may go under it, then judges no memory and is never evidence. There are two runs per variant, each with a fresh daemon and `HOME`, and both must pass. In `write_soak.rs`, `WRITE_SOAK_PENDING=N` starts each run on a copy of a `HOME` filled once with N pending questions, and `WRITE_SOAK_READS` picks the readers' reads (`node`, `search`, `inbox`; all by default).
- **Binaries.** Build with `cargo build --release -p specengine-cli -p specengine-http --bins`. `--bin spec` with both packages builds no daemon, and it once left `specengine-http` stale. A harness refuses to start when a release binary it runs is older than its own sources: a file under `crates/<c>/src` of a workspace crate in its path-dependency closure, `Cargo.lock`, and `ui/dist` for the daemon, which embeds it. It names that build command (write-memory WM4). Cargo does not relink `spec` after an http-only change, so a rule over every crate would refuse forever.
- **One at a time.** One soak runs at a time, self-terminating, with no edits or builds during it.
- **Report.** Each run prints one stdout JSON line. It holds the load, the calls and their `ms` p50, p95, p99 and worst, the samples, the slopes and the budgets, the hangs and the breaches; `write_soak.rs`'s also the pending count at both ends, each inbox read's bytes and `ms`, and a `timeline` of RSS, footprint and pending every 10 s.

## Memory

- **Sampled** every second: RSS (`ps -o rss=`) and, on macOS, `phys_footprint` (`proc_pid_rusage`'s `ri_phys_footprint`, the ledger `footprint -p <pid>` prints).
- **The quiet probe** (macOS) runs every 30 s. It holds the clients, waits for every call to answer, waits 1 s, then reads the malloc zones' allocated bytes with `vmmap --summary`. That is the live heap between calls, without a call's working set or the pages the allocator keeps.
- **Judged**, both, on every judged run:
  - on macOS, the live heap against the soak's bar, and the footprint at the quiet moments (the probe's readings) against 5 MiB/min, RSS reported beside them;
  - on Linux, RSS.

  Each judge sees what the other cannot. After a call's step, libmalloc keeps its freed pages resident (98 % fragmentation at the end of a write run), so RSS judged every clean write run red. The footprint sampled every second falls for minutes as the pages go back, which hid a real 2 KiB-per-write leak in 4 of 5 runs; the live heap read that leak as 2.16 MiB/min and a clean run as 0.0 (write-memory "Cause"). The live heap misses what malloc does not count, the freed large blocks it keeps: under writers and inbox readers together they climbed while the live heap stayed level, and the quiet footprint read that 25.0 and 12.5, a clean run at most 0 (mixed-load-memory).
- **The slope** is in MiB/min, Theil-Sen (`library_common::theil_sen_mib_per_minute`) over the readings from `SOAK_SECS/3` on. The probe gives 6 judged readings in 300 s; fewer than 5 break the run, for either judge.
- **Leak or plateau.** A leak grows in proportion to the load and is positive in every third. A plateau's per-third slopes fall as a bounded structure reaches its cap. A plateau passes only when a run past its cap shows its last third within budget, or, when the cap takes hours to reach, when a test pins the structure's real bytes under its cap (write-memory WM7).
- **The step** is one call's working set, the first samples' rise. It is reported, not judged; heap mode (`MallocStackLogging`, `leaks -outputGraph`, `heap -diffFrom`) shows whether any of it survives.

## Budgets

| Soak | Test | Load | Memory bar, MiB/min | Other bars | Last measured |
|---|---|---|---|---|---|
| DL14, library reads | `library_soak.rs` `dl14_` | 10 readers of `GET …/library`, a commit every 10 s | 0.5, RSS | a read under 10 000 ms and 200; `index=walked` ≤ 1 + ⌊`SOAK_SECS`/300⌋; no hang | 0.0 and 0.062 (2026-11-19) |
| DP17, card practices | `practices_soak.rs` | 10 logins of `get_card {context}`, a page reader | 0.5, RSS | a call under 10 000 ms; one walk per call; the page `walked` ≤ 1 + ⌊`SOAK_SECS`/300⌋; no hang | 0.0 and 0.0 (2026-11-20) |
| DL15, measure | `library_soak.rs` `dl15_` | DL14's readers, 8 `ask_question` writers, the Inbox every 5 s; 120 s | reported only | 200 under 10 000 ms; every call answers; no hang | 0.96 (2026-11-19) |
| GU16, measure | `guide_soak.rs` | 10 guide readers beside 8 `ask_question` writers; 120 s | reported only | a guide read under 1 000 ms with no turn; every write answers; no hang | 0.64 and 0.73 whole, 1.68 second half; readers alone 0.0 (2026-11-20) |
| write-memory W, U, M | `write_soak.rs` | 8 writers of `ask_question` (W) or `propose_change` update (U); W with R's readers (M) | 0.25, live heap (Linux RSS); 5, footprint at the quiet moments (macOS) | every call answers as expected; no hang; a stale binary refused | live heap, then quiet footprint (2026-11-20, mixed-load-memory): W 0.0, 0.0; −6.59, −4.65. U 0.057, 0.058, the check memo to its cap; −2.04, −2.86. M 0.0, 0.0; −4.34, −4.88 (73.3 and 73.1 before) |
| write-memory M over 2 000 pending | `write_soak.rs`, `WRITE_SOAK_PENDING=2000` | M's, the queue filled first | 0.25, live heap (Linux RSS); 5, footprint at the quiet moments (macOS) | as W | 0.0, 0.0; −1.22, −2.13 (25.0 and 12.5 before); RSS 79 → 79 MiB (216 → 443 before) |
| write-memory R | `write_soak.rs` | 10 readers of `get_node`, `search`, `GET …/inbox` | 0.5, live heap (Linux RSS); 5, footprint at the quiet moments (macOS) | as W | 0.0, 0.0; 0.0, 0.016 (2026-11-20) |
| write-memory H | `write_soak.rs` | 8 writers repeating one question, a hit each | reported only | as W | RSS 0.215, one run (2026-11-20) |
| write-memory CM over 2 000 comments | `write_soak.rs`, `WRITE_SOAK_VARIANT=CM` | 10 logins over 2 000 comments on 20 cards: 8 `add_comment` writers over `/mcp` beside readers of `get_card {context}`, `GET …/comments`, `GET …/mentions` | 0.25, live heap (Linux RSS); 5, footprint at the quiet moments (macOS) | as W | live heap, then quiet footprint (2026-11-22, comments CM19): 0.0, 0.0; −1.42, −2.96; RSS 5.2 → 81.1 and 80.2 MiB; 22 295 and 22 031 calls, all answered, 0 hangs. A planted 2 KiB per `add_comment`: live heap 4.40, red; the quiet footprint alone read 1.86, under 5: the live heap is the judge that catches a steady leak |

Why 0.25 for writes: at about 500 writes a minute it allows at most 0.5 KiB kept per write. On the live heap the readers and a clean writer read 0.0 and a 2 KiB-per-write leak 2.16, so the bar sits above the clean runs and below a real leak. Why 5 for the quiet footprint: clean runs read at most 0, and M over 2 000 pending read 25.0 and 12.5 before its fix, so the bar sits 2.5 times under the lowest red run: noisy, so read the thirds too. `write_soak.rs`'s `WRITE_BUDGET_MIB_PER_MIN`, `READ_BUDGET_MIB_PER_MIN` and `FOOTPRINT_BUDGET_MIB_PER_MIN` hold the write-memory rows' numbers, and a test compares them (write-memory WM6, mixed-load-memory MX6).

## Known limits

- DL14 and DP17 judge RSS, and predate the live-heap judge and the stale-binary refusal; their rows keep their history. A slice that touches them moves them to both.
- DL15 and GU16 run for 120 s, under the 5-minute floor, so they judge no memory.
- The quiet probe is macOS's (`vmmap`). It pauses the load every 30 s, 0.7-2.5 s each time, so a run's calls per minute include the pauses.
- On Linux, RSS keeps glibc's freed arenas as well, so read a Linux run's thirds beside its slope.
- **Memory malloc does not count** escaped the live heap until RM-0134: under writers and inbox readers together, each inbox read re-sized buffers of about 6.6 KB per pending item and libmalloc kept the freed large blocks, RSS 216 → 443 MiB in 300 s over 2 000 pending. One read now holds its answer (`proposal-review.md` "Inbox"). M with no prefill still shows RSS 1.4 MiB/min on macOS, reported only; a Linux run judges RSS, so read it there (write-memory WM10).
- The daemon has no watchdog thread of its own: a soak's request watchdog is the only one. The owner's frame watchdog is for his games.
- macOS's allocator is not the hub's glibc (Debian trixie). A Linux run in the hub's image is an owner's gate (write-memory WM10).

## Tests

eval `library_soak.rs`, `practices_soak.rs`, `guide_soak.rs`, `write_soak.rs` (all `#[ignore]`d, release, but `write_soak.rs`'s tests of this page's "Budgets" and of the judges, WM6 and MX6), and `library_common/` (the hub, the sampler, the slope). `write_soak.rs`'s `#[ignore]`d `rm0134_inbox_peak_by_pending` holds one `spec inbox --json` to 2.2 KB of footprint per pending item; cli `inbox_heap.rs` counts one inbox read's peak and allocations (mixed-load-memory MX3, MX4).
