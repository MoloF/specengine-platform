---
class: spec
status: shipped
shipped: 2026-11-15
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, crates/specengine-code, crates/specengine-eval]
ref: RM-0125; the owner's words of 2026-11-15 ("do all the points, 1, 2, 3 at once"; "include b in the release"; "a team of up to 10 people in parallel"); the analyst's breakdown and supplement; patch release 0.47.1
adrs: [ADR-0083]
---

# write-cost: a write's check pays for what it touches, and the confirm's POST reuses its GET's findings (RM-0125)

## Why

On the hosted hub (2 vCPU, 4 GB, 1 841 documents), the Inbox's Confirm was slow and grew with the corpus. A question's GET held the turn 3.2-3.3 s and its POST 4.4-4.6 s. Three causes:
- every write read and parsed every file and checked the whole tree twice, though the call had just decoded the index;
- the POST repeated the GET's rehearsal;
- about 25 and 44 git processes ran.

His words:
1. Check only what a change touches; the POST does not recompute when nothing moved; the as-is check is cached until the tree changes.
2. Include option (b), the walked rows.
3. Do not break the project: a team of up to 10 people works in parallel.

His goal was about 1 s. The 0.47.0 gate's three red timing tests failed only under load.

## Acceptance criteria

- [x] **WC1** E's oracle (`crates/specengine-core/tests/introduced_oracle.rs`): equal to `PatchCheck::introduced` on 7 167 fixture cases and 16 cycle cases. The repository's 960-case sample passes but is `#[ignore]`d. The mutations are red, except two shown equivalent.
- [x] **WC2** Generated corpora: 697 cases in 9.7 s.
- WC3 and WC4: retired, with A and B.
- [x] **WC5** POST after GET: 0 runs and 0 parses; the 409 cases; another login's commit between and a double confirm both equal 0.47.0's outcome.
- [x] **WC6** A 32 MiB byte budget with D evicted by LRU. Budget 0 gives byte-identical answers. D is keyed by login. A count bound of 8 turns it red. See deviation 3.
- [x] **WC7** Git spawns. A question's GET/POST went from 25/45 to 11/25; an update's from 29/46 to 15/25.
- [x] **WC8** `--local-env-vars` runs once per `PATH`.
- [x] **WC9** Byte-identical to the 0.47.0 goldens on 31 corpora: cli, http and mcp (deviation 1).
- [x] **WC10** No format step and no table or column: `INDEX_FORMAT` 17, queue 14, backup 7.
- WC11: replaced by WC25.
- [x] **WC12** `cost.rs` is relative and green under 28 `yes` processes on 14 cores.
- [x] **WC13** The level walks are relative.
- [x] **WC14** T(100 000)/T(25 000) is 4.01 alone and 4.10 under load: linear, so `levels.rs` is unchanged.
- [x] **WC15** The five fields (deviation 2).
- [ ] **WC16 [owner]** The live hub on 0.47.1, read with `SPECENGINE_SLOW_MS=0` for the session, then removed. The median of three confirms at about 1 900 documents, `index=walked`, must show:
  - GET ≤ 1 000 ms, POST ≤ 2 000 ms and MCP `propose_change` ≤ 1 500 ms of `run_ms`;
  - `check_ms` ≤ 150 on a GET after another login's commit, and ≤ 30 on a POST after its GET;
  - at most 60 ms more per 1 000 documents, once the corpus has grown by 500;
  - the Stage route no slower;
  - RSS recorded.

  The POST's ≈1 s is a later durability slice.
- [x] **WC17** fmt, clippy, the full nextest run, `export index`, `check`; plugin 0.47.1; `tag-check.sh v0.47.1`.
- [x] **WC18** ADR-0083 (1 341 B), with its canon `docs/canon/spec-check-write.md`.
- [x] **WC19** (b) equals the disk check: the store's fixture oracle has 3 254 cases, and `specengine-eval write-cost` shows 0 differ.
- [x] **WC20** One snapshot. **WC21** The hash net (deviation 4). **WC22** The racy window. **WC23** The hole, pinned.
- [x] **WC24** The ID preview and the holder lookup run without a decode.
- [x] **WC25** The GET path grows +30.7 and +32.4 ms from 1 000 to 4 000 nodes, against a bound of 1.5 × the walk (41-42 ms); deviation 5.
- [x] **WC26** The memo lives only in the turn. **WC27** No D hit crosses logins. **WC28** Ten logins on two seeds, replayed one call at a time: identical. **WC29** Suites unchanged and green. **WC30** A doubt empties D and C re-validates.
- WC31-WC34: they belong to (c), deferred to RM-0126.
- [x] **WC35** `SPECENGINE_SLOW_MS` in the hub's entrypoint (`deploy.rs` `wc35_*`).

## Data

### Goldens and the git shim

`fixtures/write-cost-golden/<corpus>.json` holds one file per corpus, 31 in all, plus `git-counts.json`. They were captured at `d38e70d4` (`v0.47.0`) before the first source edit, and are normalized as deviation 1 says. The shim is an executable `git` first on the spawned `PATH`: it logs its arguments one line per call, then `exec`s the real git.

### Sections the code cites

These are kept as pointers.

#### E: the affected set

`docs/canon/spec-check-write.md` "The affected set".

#### C and D

`docs/canon/spec-check-write.md` "Memos".

#### Attribution: five fields

`docs/canon/serve-slow.md` "Lines"; `docs/canon/spec-check-write.md` "Cost fields".

#### Test hygiene

`cost.rs` and `hash_levels_depth.rs` compare T(full) ≤ 30 × T(tenth) + 250 ms, interleaved, best of three, with a 60 s guard.

#### The hub's slow-line threshold

`docs/canon/hub-deploy.md` "Entrypoint"; `docs/canon/serve-slow.md` "Threshold".

## Implementation

**Canon:**
- `docs/canon/spec-check-write.md` (ADR-0083) holds the detail;
- `serve-slow.md` and `hub-deploy.md` "Entrypoint" carry the fields and `SPECENGINE_SLOW_MS`;
- eight canon files and six crate READMEs point to them.

(c), in-process git, is RM-0126, pending the owner's review.

| Module | What it does |
|---|---|
| core `check/affected.rs`, `check/counts.rs`, `engine.rs` `run_over`, `patch.rs` `introduced_{over,in}` | the set, the region, the runs over them; `RunCounts` |
| store `check_rows.rs`, `check_memo.rs`, `cost.rs`, `read.rs`, `worktree.rs` | `introduced_from_rows`; C and D; the five fields; `highest_record_number` by query; G1, G2 |
| cli `decide`, `propose`, `create`, `amend`, `interpret`, `intake`, `preflight` (`target_closure`), `confirm`, `apply` | the call sites; G3-G5 and one more cut |
| http `turn.rs` (`within`), `answer.rs`, `mcp.rs`, `auto.rs`, `slow.rs`; mcp `read.rs` | the memo inside the turn; the fields; stdio's memo |
| eval `write_cost.rs`; `scripts/deploy` | `specengine-eval write-cost`; `SPECENGINE_SLOW_MS` |

**Measured, release builds:**
- The laptop's approve to its prompt dropped from 0.16-0.18 s to 0.08-0.09 s, and with its commit from 0.32 s to 0.21 s.
- `write-cost` per case (whole check / rows / D hit), with 0 differ everywhere:

  | Corpus | Whole | Rows | D hit |
  |---|---|---|---|
  | this repository | 79.2 ms | 9.1 ms | 0.20 ms |
  | 4 022 generated files | 240.2 ms | 59.4 ms | 1.16 ms |
  | a real corpus of about 1 860 documents, local | 315 ms | 35 ms | 0.6 ms |

**Deviations:**
1. WC9's normalizations:
   - `<root>`;
   - `<sha>`, http only;
   - `<now>` and `<today>`, http and mcp, which read the wall clock (the CLI's clock is injected);
   - `git show`'s `index` lines dropped;
   - the goldens are ASCII JSON, compared parsed.
2. A proposal's own trusting read spawns `git diff --no-index`, so it shows `git=1`.
3. Ten GET→POST pairs cannot all hit, because a committing POST moves every digest. The test instead reads ten logins' reads again, then checks a POST hit.
4. A D hit returns before the hash net. This is ADR-0083's hole: D's key holds the walked digest.
5. Two WC25 cases are printed, not asserted, and are next-slice work: an index render (+107 ms, it decodes every row) and a root file with many readers (+210 ms).
6. WC1's mutations in code that both sides share pass the oracle, so the cycle cases assert explicit tuples. "No render on a title change" is caught only by the `#[ignore]`d repository sample.
7. Cycles run over a forward-closed region instead of the edge rows. C is replaced per digest. The roadmap render trigger was dropped.
8. Small ones:
   - one extra git cut, for the confirm's top and common dir;
   - `DoubtOnPanic` leaves D alone;
   - the `--push` merge enters no memo scope.

**Next:**
- the POST's write side (durability, `write_ms`);
- the index render's decode;
- the trailer lookup's growth;
- `place_unchanged` and the other git merges;
- (c).
