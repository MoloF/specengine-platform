---
class: spec
status: in-progress
scope: [crates/specengine-eval, crates/specengine-store, crates/specengine-http, crates/specengine-mcp, crates/specengine-cli, scripts/deploy, docs]
ref: RM-0132; the analyst's breakdown of 2026-11-20; the owner's word of 2026-11-20 ("fix what remains, to the end"), on which the main session decided his open questions and, after the diagnosis, the judge and the fix; his stability rule (ADR-0086)
adrs: [ADR-0086]
---

# write-memory: the daemon's memory under agent writes, soaked, diagnosed and fixed (RM-0132)

## Why

A hub serves up to ten people in parallel for weeks (his word of 2026-11-15), and his rule makes a leak a critical defect (ADR-0086, `docs/canon/soak-runs.md`). Two soaks pointed at the write path:
- **A step.** With 8 MCP writers the soak daemon's RSS went 5 → 96 MB; readers alone stayed at 9.8-13.7 MB (GU16, `guide_soak.rs`).
- **A slope.** 0.96 MiB/min (DL15), 1.68 MiB/min over GU16's second half.
- **The live hub.** 422 → 498 MB after 8 writes (08 s2 Phase 7, "Owner's word, 2026-11-14", before RM-0125's memo).

The slice builds the write soak his rule asks for, uses its variants to tell code that keeps memory from growth with the queue's history and from an allocator plateau, fixes that cause and shows it with a clean re-run. Generic machinery, no project knowledge (ADR-0008); nothing blocks on a breach (ADR-0012).

### Decided, 2026-11-20

The main session decided his open questions on his word "fix what remains, to the end":

| Question | Decision |
|---|---|
| Budget 0.25 MiB/min for the write soaks? | Yes; reads keep DL14's and DP17's 0.5 |
| ADR-0086 records the stability policy now? | Yes: the ADR, a rule in `architecture.md`, one canon page (`soak-runs.md`) |
| If glibc's arenas are the cause, may the image set `MALLOC_ARENA_MAX=2`? | Yes, in `scripts/deploy`; an allocator crate only with its own ADR (out of scope) |
| May the dedup read fewer rows if hits and related stay byte-identical? | Yes, behind an oracle test |
| Linux confirmation before close? | An owner's gate that blocks nothing (WM10) |

After the diagnosis ("Cause"), on the same word:

| Question | Decision |
|---|---|
| What is judged on macOS? | The quiet probe's live-heap slope, Theil-Sen from `SOAK_SECS/3` on. RSS and the footprint are reported beside it. On Linux, RSS (WM10). The budgets are unchanged. |
| WM1's mutation | It keeps at least 1 KiB per stored question (a 2 KiB string). The analyst's summary, about 54 B, keeps about 0.05 MiB/min, so it could never be red. |
| WM4's rule | Each binary is held to its own workspace path dependencies and `Cargo.lock`, and the daemon to `ui/dist` too. Cargo does not relink `spec` after an http-only change, so the old rule refused forever. |
| The fix | The check memo counts the real heap it holds. The root `PathBuf` leaves D's key; the login stays in it. |
| The hub's image | `MALLOC_ARENA_MAX=2` in `scripts/deploy`. |
| The memo's plateau | WM7's test shows it, not a 14 h soak. |
| The dedup | Unchanged: `queue_matches` keeps nothing. |

## Description and interactions

### The harness

`crates/specengine-eval/tests/write_soak.rs`, `#[ignore]`d, on `library_common`, as DL14, DP17 and GU16 are:
- DC-11's tree at `--nodes 10000` with its 10 000 commits;
- the release `specengine-http` as a hub over it (`--public-origin`, `--port 0`, `--slow-ms 0`, a scratch `HOME`), under `perl -e 'alarm N'` with N past the run, killed and reaped on drop;
- logins `writer-1`…`writer-8` and `reader-1`…`reader-10` by the proxy's header, stateless `/mcp/<slug>` calls as DL15's.

One `#[test]`, `write_soak`, runs the variant `WRITE_SOAK_VARIANT` names (default `W`) `WRITE_SOAK_RUNS` times (default 2), each run a fresh daemon and `HOME`, so a fresh queue.

| Variant | Load for `SOAK_SECS` | Judged |
|---|---|---|
| W | 8 writers call `ask_question` back to back, each text fresh syllables (`nonce_words`), so none hits and each is stored; no readers | 0.25 |
| U | 8 writers call `propose_change` `update` back to back on one of `U_TARGETS` ID sections, the text the section's own plus a fresh sentence, against its `span_hash`: each a new check-memo key; nothing is applied (no `[auto]`) | 0.25 |
| R | 10 readers cycle `get_node`, `search` and `GET /api/projects/<slug>/inbox` | 0.5 |
| M | W's writers and R's readers together | 0.25 |
| H | 8 writers ask one question, the same text on the same node, so every call but the first is a hit and stores nothing | reported only |

### Measuring

Every second the harness samples RSS (`ps -o rss=`) and, on macOS, `phys_footprint` (`proc_pid_rusage`'s `ri_phys_footprint`).

On macOS the quiet probe runs every `QUIET_EVERY` (30 s). It holds the clients, waits for every call to answer, waits `QUIET_SETTLE` (1 s), then reads the malloc zones' allocated bytes with `vmmap --summary`. That is the live heap between calls: without a call's working set, and without the pages the allocator keeps.

The judged series is the live heap on macOS and RSS on Linux. Its slope is Theil-Sen over the readings from `SOAK_SECS/3` on; RSS's and the footprint's slopes are reported beside it.

Why the probe and not the others ("Cause"):
- After each write's step, macOS keeps the freed pages resident (98 % fragmentation at the end of a W run), so RSS judged every clean W run red (0.48-0.54 MiB/min).
- The footprint falls for minutes as the allocator returns those pages, which hid a real 2 KiB-per-write leak in 4 of 5 runs.
- The probe read that leak as 2.16 MiB/min and a clean run as 0.0.

### Diagnosis, in order

The test engineer ran it on 2026-11-20, 300 s a run, in this order:
1. the measurement: footprint and RSS beside the quiet probe;
2. H against W: growth with rows stored, not calls;
3. U against W: the memo and the delegated apply;
4. the memo's accounting;
5. the step;
6. the attribution: `WRITE_SOAK_HEAP=1` starts the daemon with `MallocStackLogging=lite` and keeps `leaks -outputGraph` memgraphs and `vmmap --summary` at 60 s and at the end. `heap -diffFrom=<memgraph>` and `malloc_history` read them.

A heap run is reported, never judged: the stack log weighs on memory. The findings, by file:line, are in "Cause".

### The fix

The cause is the check memo's D entries ("Cause"). They are bounded by `BUDGET`, but the budget counts about 2.7 times less than the heap they hold. The fix is one targeted change in `specengine-store` `check_memo.rs` and `check_rows.rs`, inside ADR-0083 ("a miss is slower, never wrong"):
- **Real accounting.** An entry's accounted bytes count the heap it holds: its map node, the key's and the value's capacities, and every string and path it owns. The 32 MiB `BUDGET` is then real bytes.
- **A smaller entry.** The root `PathBuf` leaves D's key, since a daemon's memo is already one per served project (`Project.memo`). The stdio server's process-wide memo, which today keeps the root in every key, keeps entries of different roots apart another way (one memo per root, say), so no lookup crosses roots.
- **The login stays** in D's key: "No findings cross logins" (WM9).
- **The hub's image** sets `MALLOC_ARENA_MAX=2` in the daemon's environment (`scripts/deploy`). It is the glibc arena mitigation for the live hub's step (WM10): unproven here, and it changes no answer.

It changes no answer and no stored row: no `INDEX_FORMAT`, queue schema or backup format step, and no plugin file change. The step itself, each write's transient corpus load, is no leak and is not shrunk here.

Canon in the shipping change:
- `soak-runs.md` "Budgets": the after-fix rows;
- `spec-check-write.md` "Memos": the key without the root, the real accounting and its numbers;
- `hub-ops.md`: `MALLOC_ARENA_MAX=2`;
- `crates/specengine-eval/README.md` gains the harness. It is at its cap, so compress it first.

### WM10, the owner's gate

W is run on Linux in the hub's image (Debian trixie, `scripts/deploy/Dockerfile`). Docker is off on the laptop by his choice, so this needs a Linux host.

The diagnosis could not reproduce glibc here. A transient of 35-75 MB per write, kept in glibc's arenas, fits 422 → 498 MB, but it is unproven.

After the upgrade that carries `MALLOC_ARENA_MAX=2`, the main session records the live hub's RSS under real use only (`docker stats` or `ps` on the host):
- before the first write of a day;
- after about 8 and about 80 real writes;
- 24 h later.

**Baseline**, recorded by the main session before that upgrade: the live hub on plugin 0.49.0, `docker stats` at about 13:40 local on 2026-11-20. `deploy-specengine-1` used 254.4 MiB after 7 h up (started 03:58:37Z), under real use.

There are no synthetic writes on the live hub, which serves real projects. The 422 → 498 MB step is explained or fixed. The gate blocks nothing (ADR-0012).

## Data

### Constants and settings (`write_soak.rs`)

| Constant | Value |
|---|---|
| `NODES` | 10 000, with `library_common::COMMITS` (10 000) |
| `MIN_SOAK_SECS` | 300, the default and the least `SOAK_SECS` |
| `WRITE_BUDGET_MIB_PER_MIN` | 0.25: W, U, M |
| `READ_BUDGET_MIB_PER_MIN` | 0.5: R |
| `WRITERS`, `READERS` | 8, 10 |
| `SAMPLE_EVERY` | 1 s |
| `QUIET_EVERY`, `QUIET_SETTLE` | 30 s, 1 s: the quiet probe (macOS) |
| `HANG` | 30 s, then `sample <pid> 5` |
| `TIMEOUT` | 120 s, the client's own give-up |
| `U_TARGETS`, `SEARCH_WORDS` | 200 ID sections U updates and R reads; 64 words R searches |
| `HEAP_FIRST_AT` | 60 s, heap mode's first snapshot |
| `BUILD` | `cargo build --release -p specengine-cli -p specengine-http --bins` |

| Variable | Meaning |
|---|---|
| `WRITE_SOAK_VARIANT` | `W` (default), `U`, `R`, `M` or `H` |
| `WRITE_SOAK_RUNS` | runs, default 2 |
| `SOAK_SECS` | seconds a run, default and least 300 |
| `WRITE_SOAK_SMOKE=1` | lets `SOAK_SECS` go under 300 and judges no memory: a harness check, never evidence |
| `WRITE_SOAK_HEAP=1` | heap mode (macOS), "Diagnosis, in order" step 6; memory reported, not judged |

Run `BUILD`, then `WRITE_SOAK_VARIANT=W cargo nextest run -p specengine-eval --release --test write_soak --run-ignored only --no-capture`. A variant takes about `WRITE_SOAK_RUNS` × (`SOAK_SECS` + 20 s) plus the tree's seeding (about two minutes). Run one soak at a time, with no edits or builds during it.

### The stale-binary refusal

Before anything is seeded, the harness holds each release binary it runs to its own sources. It compares the binary's mtime with the newest mtime of:
- any file under `crates/<c>/src`, for each workspace crate in that binary's path-dependency closure;
- `Cargo.lock`;
- for `specengine-http` alone, `ui/dist`, which it embeds.

If a binary is older, the test fails with a message naming the binary, the newer file and `BUILD`. An http-only change therefore refuses the daemon, never `spec`, which cargo does not relink.

### The report line

Each run prints one JSON line on stdout. Every key is present, `null` where it does not apply. The values here are illustrative, the `lines`, `heap` and `quiet` objects shortened:

```json
{"test":"write_soak","variant":"W","run":1,"seconds":300,"nodes":10000,"writers":8,"readers":0,
 "smoke":false,"heap_mode":false,
 "calls":3150,"calls_per_min":630.0,"writes":3150,"writes_per_min":630.0,"reads":0,"reads_per_min":0.0,
 "answered":3150,"stored":3150,"hits":0,
 "ms":{"p50":930,"p95":1410,"p99":1720,"worst":2210},"write_ms":{"p50":930,"p95":1410,"p99":1720,"worst":2210},"read_ms":null,
 "samples":{"count":300,"first":{"at_s":1.0,"rss_kib":7712,"footprint_kib":4256},
   "middle":{"at_s":150.0,"rss_kib":92160,"footprint_kib":36864},
   "judged_from":{"at_s":100.0,"rss_kib":91136,"footprint_kib":45056},
   "last":{"at_s":300.0,"rss_kib":93184,"footprint_kib":21264}},
 "quiet":{"count":9,"heap_kib":[[32.0,1750],[62.0,1750],[272.0,1750]],"heap_kib_per_write":0.0,
   "heap_slope_mib_per_min":{"whole":0.0,"judged":0.0,"thirds":[0.0,0.0,0.0]},
   "drain_ms":{"p50":679,"p95":732,"p99":732,"worst":732},"scan_ms":{"p50":403,"p95":433,"p99":433,"worst":433}},
 "judged_on":"live_heap","judged":true,"slope_mib_per_min":0.0,"budget_mib_per_min":0.25,
 "footprint_slope_mib_per_min":{"whole":-6.4,"judged":-1.31,"thirds":[-20.2,-5.7,-0.25]},
 "rss_slope_mib_per_min":{"whole":0.9,"judged":0.465,"thirds":[0.76,0.67,0.04]},
 "kib_per_write":{"footprint":-2.14,"rss":0.76},"kib_per_call":{"footprint":-2.14,"rss":0.76},
 "process":{"start":{"threads":1,"open_fds":9,"children":0},"middle":{"threads":2,"open_fds":22,"children":0},
   "end":{"threads":1,"open_fds":9,"children":0}},
 "binary":{"path":"target.noindex/release/specengine-http","size":37681536,"mtime_unix":1795166195},
 "lines":{"writes":3150,"write_wait_ms":{},"write_run_ms":{},"write_git_ms":{}},
 "hangs":0,"hang_samples":[],"heap":null,"broken":[],"broken_count":0}
```

- `ms`, `write_ms`, `read_ms`: the client's clock, p50, p95, p99 and worst.
- `quiet` (macOS; `null` elsewhere):
  - `heap_kib` holds the quiet readings, `[at_s, KiB]`;
  - `heap_slope_mib_per_min` is their Theil-Sen slope over the whole run, the judged window and each third;
  - `heap_kib_per_write` is the judged slope × 1024 ÷ writes per minute;
  - `drain_ms` is how long the held clients took to drain, and `scan_ms` how long `vmmap` took.
- `judged_on`: `live_heap` on macOS, `rss` elsewhere. `slope_mib_per_min` is that series' judged slope. `judged` is false in smoke and heap modes.
- `footprint_kib` and the footprint slopes are `null` off macOS.
- Each slope object is Theil-Sen over the whole run, over the judged window (from `SOAK_SECS/3` on) and over each third.
- `kib_per_write`: the judged slope of RSS and of the footprint × 1024 ÷ writes per minute, `null` for R. `kib_per_call` is the same per call.
- `process`: threads, open fds and child processes (git among them) at start, middle and end.
- `lines`: the write lines' cost fields (`serve-slow.md` "Lines").
- `hang_samples`: the kept sample files' paths. `broken`: each broken bar as text, at most 20. If any run breaks one, the test fails after every line is printed.

### Why 0.25

At about 500 writes a minute, 0.25 MiB/min allows at most 0.5 KiB kept per write. On the live heap:
- the readers and a clean W read 0.0;
- U's memo read 0.092 (0.54 KiB per update);
- the 2 KiB-per-write mutant read 2.16.

So the bar sits above the clean runs and well below a real leak. The numbers live in `WRITE_BUDGET_MIB_PER_MIN`, `READ_BUDGET_MIB_PER_MIN` and `docs/canon/soak-runs.md` "Budgets", equal (WM6).

## Rules and edge cases

- WHEN a release binary is older than a file under `crates/<c>/src` of a workspace crate it depends on by path, or than `Cargo.lock`, or (the daemon) than `ui/dist`, the harness SHALL refuse before seeding, naming the build command (WM4).
- WHEN `SOAK_SECS` is under 300, the harness SHALL refuse, unless `WRITE_SOAK_SMOKE=1`, which then judges no memory.
- WHEN a request stays open for 30 s, the harness SHALL run `sample <pid> 5` (macOS) into a kept temp file, print its path and fail the run. The run still prints its line.
- Every call SHALL answer as its variant expects: in W, U and M each write is stored (an ID); in H each repeat is a hit and nothing is stored; in R each read is a 200 with no `isError`.
- Memory SHALL be judged on the quiet probe's live heap (macOS) or RSS (Linux), Theil-Sen from `SOAK_SECS/3` on, never on two endpoints or a least-squares line. Fewer than 5 judged quiet readings break the run. Both runs of a judged variant SHALL be within budget.
- A leak grows in proportion to writes and is positive in every third. A plateau's thirds fall as a bounded structure reaches its cap. The memo's cap is reached only after hours, so its bound is shown by WM7's test, not by a soak.
- The fix SHALL:
  - change no answer and no stored row (the goldens);
  - stay inside ADR-0083: bounded, a miss slower and never wrong;
  - keep "No findings cross logins" (`spec-check-write.md` "Memos") and the team of ten's results (WM9);
  - keep roots apart in the stdio server's memo.
- A breach fails a soak, which is a test. It never refuses a user's call (ADR-0012).
- A hub serving P projects holds up to P × 32 MiB of memo, real bytes after the fix, by design. The soak serves one.
- Each soak self-terminates and leaves nothing running.

## Acceptance criteria

WM1 and WM2 judge the harness and the code together. Before the fix, the runs are evidence (WM5); after it, they must pass (WM8).

- [ ] **WM1 [soak]** `crates/specengine-eval/tests/write_soak.rs`, `#[ignore]`d: a release hub over the tree. For `SOAK_SECS` (≥ 300), 8 logins call `ask_question` back to back with fresh text, no readers. Memory is sampled every second, the live heap every 30 s, and judged as WM6.
  - Pass: every call answers; a request open 30 s runs `sample <pid> 5`, prints the file's path and fails the run; two runs; a breach exits non-zero.
  - M: a per-process `Vec<String>` keeping at least 1 KiB (a 2 KiB string) per stored question, never drained: both runs red.
- [ ] **WM2 [variants]**
  - U: 8 writers, `propose_change` update.
  - R: 10 readers of `get_node`, `search`, `GET …/inbox`.
  - M: W and R together.
  - H: one question repeated so it hits; reported, not judged.

  Pass: U, R and M within budget in two runs each.
- [ ] **WM3 [report]** Each run prints one JSON line ("The report line"):
  - the variant, seconds and calls per minute;
  - `ms` p50, p95, p99 and worst;
  - the first, middle and last samples, the slope and the budget;
  - the RSS, footprint and live-heap slopes, and KiB per write;
  - threads, open fds and child processes at start, middle and end;
  - the daemon binary's path, size and mtime.

  Pass: every key is present in both runs.
- [ ] **WM4 [stale binary]** The harness refuses to start when a release `spec` or `specengine-http` is older than its own workspace path dependencies' `src`, `Cargo.lock` or (the daemon) `ui/dist`, naming `cargo build --release -p specengine-cli -p specengine-http --bins`. After an http-only change it refuses the daemon alone.
  - M: build, then `touch crates/specengine-http/src/main.rs`: it refuses.
  - M: the old rule, every crate for every binary: a build after an http-only change still refuses.
- [x] **WM5 [cause]** This document's "Cause" cites the runs of W (three), U (two, plus a quiet run), R (two) and H (one, reported, never judged), and the heap attribution by file:line.
- [ ] **WM6 [budget]** The harness constants and the canon hold the same numbers. A non-ignored test in `write_soak.rs` reads the `write-memory` rows of `docs/canon/soak-runs.md` "Budgets" and compares them with `WRITE_BUDGET_MIB_PER_MIN` and `READ_BUDGET_MIB_PER_MIN`. M: the table's 0.25 changed to 0.3: red.
- [ ] **WM7 [fix]** A store test with a counting global allocator fills the check memo far past its cap with D entries. Then:
  - its accounted bytes stay ≤ the cap;
  - the heap the memo really holds stays ≤ the accounted bytes, and the accounted bytes stay ≤ 1.5 × that heap;
  - its entry count is bounded.

  A second test pins the smaller entry: no root path in D's key, and the login still in it.
  - M: the old `findings_bytes`: the real heap exceeds the accounted bytes, red.
  - M: the root back in D's key: red.

  The counting allocator lives in that test alone; it is not an allocator crate.
- [ ] **WM8 [clean re-run]** After the fix, W, U, M and R each pass two runs under the live-heap judge. The memo's plateau is shown by WM7's test, not by a soak: at U's rate it reaches its cap after hours.
- [ ] **WM9 [team of ten]** `crates/specengine-http/tests/hub_concurrency.rs`, the three `write_cost_golden.rs` tests (cli, http, mcp), `check_memo`'s unit tests and the full run are green. M: key D without the login: `a_login_misses_another_logins_findings` red.
- [ ] **WM10 [Linux, owner's gate]** Run W in the hub's image. If that is not possible, record the live hub's RSS before the first write, after 8 and after 80 writes, and 24 h later. The 422 → 498 MB step is explained or fixed. Baseline: 254.4 MiB on 0.49.0 after 7 h of real use (2026-11-20). The main session records the live hub's RSS after the upgrade that carries `MALLOC_ARENA_MAX=2`, under real use only: no synthetic writes on the live hub, which serves real projects; Docker is off on the laptop by the owner's choice. This gate blocks nothing.

## Out of scope

- A write's latency or cost beyond the fix (RM-0126, in-process git; the POST's durability).
- Concurrent trusted reads (a planned ADR).
- Card, confirm and page writes. The soak drives the agents' `/mcp` queue tools only.
- Shrinking the step: each write's corpus load is transient (no allocation of it survives).
- A global allocator crate (its own ADR). A counting allocator inside a test is not one.
- A soak long enough to show the memo's plateau (about 14 h).
- A narrower dedup: `queue_matches` keeps nothing.
- A watchdog inside the daemon. The harness's hang watchdog serves.
- Moving DL14 and DP17 to the live-heap judge (`soak-runs.md` "Known limits").
- Synthetic writes on the live hub.

## Cause

The test engineer measured this at HEAD on 2026-11-20, 300 s a run, on the laptop (macOS). The report lines are kept with the evidence of the slice.

**W: no leak; the step is transient.**
- Three runs at 572, 605 and 628 writes a minute, each write stored. RSS judged 0.536, 0.503 and 0.482 MiB/min, ending at 94.4, 90.6 and 91.0 MiB; the footprint judged −2.51, −4.65 and −2.25.
- A quiet run at 630 writes a minute (3 150 stored): the live heap read 1 750 KiB at every one of its 9 readings, slope 0.0.
- Heap mode at 362 writes a minute, `heap -diffFrom` from minute 1 to the end:
  - no object allocated after minute 1 was alive at the end;
  - 30 790 objects (9.36 MB) were in flight at minute 1, and all were freed.

The step is each write's transient corpus load:
- cli `intake.rs:788` `indexed_in`;
- → `corpus.rs:52`;
- → store `read.rs:335` and `:537` `indexed_input`;
- → `read.rs:637-646` `decode_all`: rusqlite `Row::get::<String>` 3.9 MB and `sqlite3Malloc` 2.7 MB.

`vmmap` at the end showed malloc 1.8 MB allocated against 66.6 MB dirty: 98 % fragmentation, with "Malloc Small (empty)" 76 MB resident. RSS's plateau is libmalloc keeping freed pages, and the footprint decays after the step.

**U: the only growth, the check memo's D entries.**
- Two runs at 147 and 169 updates a minute: RSS judged 0.105 and 0.124 MiB/min.
- A quiet run at 176 updates a minute: the live heap went 1 885 → 2 277 KiB. Its slope was 0.092 MiB/min, with thirds 0.101, 0.087 and 0.094: level, a bounded structure far from its cap. That is 0.54 KiB per update.

Heap mode named where it goes:
- the `BTreeMap<FindingsKey, Remembered>` at store `check_memo.rs:285`;
- the key's root `PathBuf`, 464 × 160 B, built at `check_rows.rs:120-121` (via cli `create.rs:827` ← `propose.rs:975`);
- `String` clones of the path and the login.

A real entry is about 605 B, but `findings_bytes` (`check_memo.rs:377`) counts about 225 B: a 2.7× undercount. So the 32 MiB counted budget is about 85-120 MiB of real heap, reached after about 14 h of updates.

**R** (two runs, 15 503 and 16 855 reads a minute): the footprint judged −0.005 and 0.0, RSS 0.011 and 0.0.

**H** (one run, reported only, 583 calls a minute): 1 stored and 2 914 hits; RSS 0.215, footprint −6.39.

**Ruled out:**
- `queue_matches` is transient only: the live heap stayed flat at 1 750 KiB while 3 150 questions were stored.
- The delegated apply leaves no survivors.
- Threads were 1-3 in every run.

**The judge.** A real leak was planted: 2 KiB kept per stored question, a mutation in mcp `hub.rs` `hub_ask_question`. Over five runs:
- the footprint judged −0.06, 0.03, 0.43, −2.60 and −2.30, so it was red in only one;
- RSS read 2.94-3.03, but RSS also judged every clean W red (0.48-0.54);
- the quiet probe read 2.157 MiB/min, with thirds 2.19, 2.15 and 2.10 (linear), 3.5 KiB per write, against a clean 0.0.

The analyst's mutation (each question's summary, about 54 B) keeps about 0.05 MiB/min, under the budget.

**The live hub** (glibc) could not be reproduced here (WM10).

## Implementation

Filled in after implementation.
