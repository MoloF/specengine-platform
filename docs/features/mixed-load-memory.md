---
class: spec
status: shipped
shipped: 2026-11-20
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-eval, crates/specengine-http, docs]
ref: RM-0134; the test-engineer's diagnosis of 2026-11-20 on 0.49.2 (540db8ff) and the analyst's breakdown of that day; the owner's word of 2026-11-20 ("fix what remains, to the end"), on which the main session decided his questions; his stability rule (ADR-0086); plugin 0.49.3
adrs: []
---

# mixed-load-memory: one inbox read holds its answer, not the queue (RM-0134)

## Why

A hub serves up to ten people in parallel for weeks, and his rule makes unbounded memory a critical defect (ADR-0086, `docs/canon/soak-runs.md`). write-memory's M (8 `ask_question` writers beside 10 readers of `get_node`, `search` and `GET …/inbox`) climbed outside the live heap: RSS about 100 → 300 MB, the quiet-moment footprint +73 MiB/min, with the live heap level at 1 750 KiB.

**The diagnosis** (0.49.2, macOS, 300 s a run):
- **No leak.** The live heap was flat in every run. libmalloc kept the freed large blocks ("Malloc Large (empty)", 146 → 268.5 MiB resident).
- **It took writers and inbox readers together.** Each alone was flat: writers with `node` and `search` readers up to 3 100 pending, inbox readers alone over 2 000. M over 2 000 pending climbed RSS 216 → 443 MiB, with the quiet footprint at 25.0 and 12.5 MiB/min.
- **One `spec inbox --json` peaked at about 6.6 KB per pending item**, against a 304 B answer per item. Store `queue.rs` `raw_rows_in` read every column of every row of the project, with no state filter, into one `Vec<RawRow>` before decoding. CLI `inbox.rs` `repository_proposals` then kept every decoded `Proposal`.
- **Each call of a project runs alone in its turn** (http `answer.rs:288-298`), so readers' peaks never stack. The "ten concurrent inbox reads" hypothesis was wrong. What grew was one read's working set, re-sized as the queue grew, because the UI reads the whole inbox again after every proposal event.

**Decided by the main session** on his word:
- the quiet footprint's bar is 5 MiB/min on macOS, for every variant;
- the live heap is kept, and Linux judges RSS;
- the inbox is not paged;
- delivery and the other per-event reads go to RM-0135;
- `queue_matches` streams its rows, with no SQL prefilter;
- no ADR.

## Data

- **Store**: `Readable {Proposal, Unreadable}`; `ProposalQueue::each_readable(filter, visit)` and `QueueSnapshot::each_readable` hand `visit` one decoded row at a time, in `ID_ORDER`, and keep none; `visit` never calls the queue. `list_readable` collects it.
- **The prefilter**, when a filter names states: `AND (status IS NULL OR status NOT IN (<the four>) OR status IN (?3…))`. A NULL or unknown state's row always comes and is noted as unreadable. Every column is selected, so a corrupt row is named by its first failing column. `list` (tests only) reads every row.
- **Soak** (`write_soak.rs`):
  - `FOOTPRINT_BUDGET_MIB_PER_MIN` 5.0 for W, U, M and R; `WRITE_SOAK_PENDING=N`; `WRITE_SOAK_READS` (`node`, `search`, `inbox`).
  - Report keys: `quiet_footprint_slope_mib_per_min`, `quiet_footprint_budget_mib_per_min` (`null` off macOS and for H), `quiet.footprint_kib`, `reads_mix`, `pending {start, end}`, `inbox {count, bytes_first, bytes_last, bytes_max, ms}`, `timeline` (every 10 s).
  - A breach reads `the footprint grows <s> MiB/min (quiet_footprint, from SOAK_SECS/3 on) > the budget 5`.
  - The canon's cell is `0.25, live heap (Linux RSS); 5, footprint at the quiet moments (macOS)`; the row `write-memory M over 2 000 pending` reads as M.
  - `rm0134_inbox_peak_by_pending` is judged at 2.2 KB per item.

## Acceptance criteria

- [x] **MX1 [identical answers]** The goldens captured at 540db8ff are byte-identical: `spec inbox` text, `--json` and `--all --json`, `approve --open`'s notes, and the hub's `GET …/inbox`. The seeded queue holds every kind and state, staged and recorded items, another repository, 12 gone rows, corrupt `author`, `branch` and `base_commit`, and NULL and unknown states (`inbox_rows.rs`). Mutations, red: a bare `status IN (…)`; a corrupt `author` read as its default, standing in for selecting only the inbox's columns.
- [x] **MX2 [every list]** `list_readable` on the queue and on a snapshot equals HEAD's `readable(raw_rows_in(…))`, kept as a `#[cfg(test)]` oracle, for 80 filter combinations (16 state subsets × 5 repository filters). Mutation, red: a bare `status IN (…)`.
- [x] **MX3 [peak]** The counting allocator measures 1 097.05 B per item, against HEAD's 3 652.35 and a bound of 1 217.45; the assertion runs on macOS only. The answers are 306 917 and 1 230 917 B, as at HEAD. `rm0134_inbox_peak_by_pending` reads 1 676.6 B per pending item after the harness's last fixes (1 611 before them; HEAD 6 641; bar 2 200), each level's answer bytes asserted equal to HEAD's. Mutation, red: collecting rows before decoding (HEAD's read), here and in MX4.
- [x] **MX4 [closed rows not fetched]** 4 000 rejected rows add 0 allocations to a 10-item read (HEAD: +96 008).
- [x] **MX5 [dedup oracle]** 54 intakes over 2 000 rows give hits and related equal to HEAD's `queue_matches`. Mutations, each red: a state filter, a text-only target match, the Rust target check dropped.
- [x] **MX6 [the judge]** `wm6_the_budgets_are_the_canons` reads both bars from `soak-runs.md` "Budgets". `mx6_the_quiet_footprint_is_judged_on_macos` breaks a climbing quiet footprint, passes a level one, and judges none off macOS. Mutation, red: 6 as the canon's footprint bar.
- [x] **MX7 [clean re-run]** 300 s a run, two runs each, both judges green. Columns: live heap; quiet footprint; RSS slope (all MiB/min); RSS mid → end (MiB).
  - M with `WRITE_SOAK_PENDING=2000`: 0; −1.222; 0.151; 79 → 79, and 0; −2.131; 0.145; 79 → 79. Pending 2 000 → 4 301; the inbox 616 234 → 1 307 834 B, `ms` p50/p99/worst 1042/1380/1458.
  - M with no prefill: 0; −4.337; 1.401; 92 → 96, twice.
  - Before the fix: 216 and 199 → 443 and 420 MiB RSS; quiet footprint 25.0 and 12.5; WM8's M 73.
  - Every call answered; 0 hangs.
- [x] **MX8 [the other variants]** All green:
  - W: 0; −6.594; 0.44; 90 → 91, and 0; −4.648; 0.438; 92 → 92.
  - U: 0.057; −2.042; 0.052; 157 → 157, and 0.058; −2.861; 0.041; 159 → 159.
  - R: 0; 0; 0; 81 → 81, and 0; 0.016; 0.011; 81 → 81.
- [x] **MX9 [regressions]** The named suites, within the full run ("Release").
- [ ] **MX10 [owner's gate]** WM10 stays open. After the upgrade to 0.49.3 the main session records the live hub's passive RSS beside 0.49.2's 36.8, 141 and 119.6 MiB. M with no prefill still shows 1.4 MiB/min of RSS on macOS, reported only; a Linux run judges RSS. It blocks nothing.

## Out of scope

Paging the inbox; the inbox's time in the turn (the concurrent-trusted-reads ADR); a SQL prefilter for `queue_matches`; any schema step; `spec delivery` and the other per-event reads (RM-0135); the write's fixed corpus load; an allocator crate.

## Implementation

| Module | What it does |
|---|---|
| store `queue.rs` | (i): `Readable`; `each_raw_row_in` with `States::Filtered` (the prefilter: the wanted states bound as `?3…`, the four written from `ProposalStatus::ALL`) or `States::Every` (`list`); `each_readable_in`; `ProposalQueue::each_readable`, with `list_readable` a provided method collecting it; `raw_rows`, `raw_rows_in` and `readable` removed. (ii): `queue_matches` steps rows through `MatchRow::read`, its SQL and Rust checks unchanged |
| store `queue/cards.rs`, `lib.rs` | `QueueSnapshot::each_readable`; `Readable` exported |
| cli `inbox.rs`, `apply.rs` | `repository_proposals<T>(context, statuses, keep)` projects each row as it comes; `approve --open` keeps the `Proposal` |
| eval `write_soak.rs` | `judge_footprint`, the knobs, the report keys, the cell parser, MX3's peak bar |
| tests | cli `inbox_rows.rs` (MX1, its goldens `inbox_rows_before.json`), `inbox_heap.rs` (MX3, MX4); store `queue/mixed_load_oracle.rs` (MX2, MX5; `head.rs` HEAD's code verbatim; `queue_rows.rs` the shared seed, `#[cfg(test)]`, which `inbox_rows.rs` includes by `#[path]`; its kinds come from `ProposalKind::ALL`, so the source guards stand) |

**Canon:** `soak-runs.md` ("Memory", "Budgets", "Known limits", "Tests"), `proposal-review.md` "Inbox", `proposal-queue.md` "Store", `plugin-files.md`.

**Deviations:**
1. (iv), the answer's buffers, was not done: MX3 passed without it.
2. The inbox's repository checks (`repository_gone`'s stats) run inside the read's deferred transaction, which blocks no writer under WAL.
3. MX3's allocator bound is macOS's.

### Release

Plugin 0.49.3, a version-only PATCH, tag `v0.49.3`; `INDEX_FORMAT` unchanged at 21; a hub's upgrade is the image alone.

The full run: 6 577 tests, all passed (125 skipped). `cargo clippy --workspace --all-targets -D warnings` and `spec check` are clean. A first run failed 4 tests: the oracle's seed in `src` tripped the source guards (`code_refs_walk`: a `tests/` path; `proposal_genre`: kind literals). The seed moved to `queue/mixed_load_oracle/queue_rows.rs` with its kinds taken from `ProposalKind`; no guard changed, and `head.rs` is untouched.
