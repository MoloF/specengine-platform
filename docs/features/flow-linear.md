---
class: spec
status: shipped
shipped: 2026-11-04
scope: [crates/specengine-core, crates/specengine-cli]
ref: RM-0076; Phase 7 step 12a (08 s2 "Step 12 in five"), split from step 12 `scale-reads` (RM-0054) by the orchestrator on 2026-11-03, the split awaiting the owner's review; two iterations
adrs: []
---

# flow-linear: a flow read linear in `parent:` depth, a bundle fit that estimates each part once (RM-0076)

## Why

`spec flow` and `spec trace` found a member's parent document by building its whole `parent:` chain, in the scope's walk, in `parented` and in `tree_of`, and the depth fold climbed the tree again on every call. A read cost O(members × depth): chains of 1 000, 2 000 and 4 000 took 0.58, 2.39 and 9.79 s in debug, and the core test capped its chain at 1 200. `spec bundle`'s `fits` estimated the whole body again for every candidate: 3 000 candidates took 1.13 s in debug. `tokens_est` adds a weight per character in thousandths and rounds up once, so running sums make the concatenation's decision exactly. This is the first of step 12's five slices, with no store, index or format change and no ADR; every output stays byte for byte.

Decided by this spec, each awaiting the owner's review:
- L1, one parent map per read;
- L2, the walk stops at an ancestor already walked;
- L3, the fold memoised too (the analysis had missed it);
- L4, core gives the estimator's two halves, so the CLI copies no weight;
- L5, target headers stay measured per target;
- L6, a golden recorded before any `src/` change;
- L7, the `--trace` counts stay (RM-0070).

## Description and interactions

Canon: `docs/canon/spec-flow.md` "The read", "Code" and "Known limits"; `docs/canon/spec-cli-bundle.md` "Fitting"; the core README "Token estimator". The surfaces, all byte-equal: `spec flow` (text, `--json`, `--trace`), `get_flow`, the daemon's flow route, `spec trace`'s lanes; `spec bundle`, the task package, `get_context_bundle`, the daemon's bundle route and eval's W.

## Data

No file format, table, JSON key, flag, config key, route or fixture changes; `INDEX_FORMAT` stays 15. In code: core `tokens::thousandths(&str) -> u64` and `tokens_of(u64) -> u32`, with `tokens_est(t) == tokens_of(thousandths(t))`; CLI `Measure {chars, thousandths}`.

The decision, worked (budget 2): body `abc` is 3 characters and 1 050 thousandths, part ` d` is 2 and 500. Together they make 1 550, which is 2 tokens and fits. Rounded per part they would make 2 + 1 = 3 and be refused. Characters are counted, not bytes: `\u{0434}` is 1 character and 2 bytes.

The golden `crates/specengine-cli/tests/flow_linear_before.json` (661 KB) was recorded with `e863186`, and re-recorded by `flow-scope` (RM-0083, FS-8) at 659 KB: 14 flow-gaps rows lost only their `depends_on` edge, its header count and `inner` count. It holds 89 `flow` and `trace` rows as bytes and 476 `bundle` rows as BLAKE3 digests of text, JSON, stderr and exit codes.

## Rules and edge cases

- A document's parent document is the file of the first node on its chain outside its own file, computed once per read. A `parent:` cycle's cut document has none.
- The scope's walk stops at the top, at a non-admitted ancestor (which joins `left_out`), or at an ancestor already walked.
- A folded member shows as the first node on its `up` chain whose level is at most N, or as the chain's top.
- The fit counts characters, rounds once over the whole sum and saturates as `tokens_est`. It decides what today's concatenation decided for every input.

## Acceptance criteria

M names the mutations that turned each red, all 10 named.

- [x] AC-10 [default, core] Linear flow.
  - The 400 seeded flows and every fixture test of flow, trace, MCP and http pass unchanged.
  - **The forest differential.** Today's `scope_of`, `parent_of`, `tree_of` and `shown_as` are kept as a `#[cfg(test)]` reference. On 300 seeded forests (sections as parents, Tier 3 and generated ancestors, planted cycles), scope, tree and folds equal the reference's, whole and under each ROOT.
  - **The ratio.** A chain of 8 000 reads in at most 8x the time of 2 000, unbounded and at `--depth 0`, best of three, the floor 50 ms.
  - **The deep test** walks a chain of 20 000.
  - **The golden.** Every `flow` and `trace` row is byte-equal.
  - M: a per-member `ancestors` in `scope_of`, `parented` or `tree_of`; `shown_as` climbing per call; the walk stopping after one parent; the walk passing a non-admitted ancestor.
- [x] AC-11 [default, cli] Bundle counts.
  - Every `bundle` row of the golden is equal by digest, `bundle_hash` included, and the bundle, package, MCP and W tests pass unchanged.
  - **The differential.** `Measure` decides as the concatenating `fits`, on seeded ASCII, whitespace, Cyrillic, CJK and emoji parts, at the token bound and at the character bound, each bound deciding both ways at least 100 times.
  - **The halves.** `tokens_est` equals its two halves, thousandths add over any split, and `tokens_of` rounds up once and saturates.
  - **The counter.** `assemble` over 12 000 questions measures at least the candidates' characters and at most twice the parts offered.
  - M: bytes for characters; rounding per part; the concatenating `fits` through `Measure::of` or through `tokens_est`.
- [x] F1 — form: clippy and fmt are clean, the implementation touches no `docs/`, fixture or `Cargo.toml`, and `export index` and `check` give 0 errors and 0 warnings.

## Out of scope

The edge table (RM-0078), reads from the index (RM-0079), read freshness (RM-0054), the synthetic corpus and the 100 000-node bars (RM-0077, RM-0054), the `--trace` counts (RM-0070), `SpecGraph::ancestors` and the bundle's ancestors layer, uncut gap rows, and the token weights.

## Implementation

Accepted on the first iteration; the second fixed a genre line and took the reviewer's two optional nits. Tests: 4909 of 4909 in the workspace; clippy, fmt and `spec check` are clean. Mutations: all 10 named turned red. The `ancestors` ones ran 19.5 s or more against about 1.1-1.6 s, and `shown_as` ran 47 s against 2.5 s at depth 0.

The developer's choices:
- the parent map is built up front for every file, O(files);
- `parent_of` is removed;
- `rest` gets no suffix sum (L5), so `rest` and the title are measured twice, and each candidate once.

Not covered:
- the golden skips `bundle_fit.rs`'s target sets over modified copies;
- the ratio times the whole read only, never a ROOT read.

| Module | What it does |
|---|---|
| core `src/tokens.rs` | `thousandths` (`:42`), `tokens_of` (`:51`); `tokens_est` (`:36`) through them |
| core `src/flow.rs` | `parents_of` (`:550`) fills each file's parent document through `listed_outside` (`:560`), which reads `listed_under` only up to the first node outside the file. `scope_of`'s walk (`:511`) keeps a `Vec<bool>` `walked`. `tree_of(&parents, &scope)` (`:581`). `folded` (`:1189`) fills shown nodes in level order and returns an empty map with no depth. The forest differential (`:1236`) holds the `#[cfg(test)]` reference |
| cli `src/bundle.rs` | `Measure` (`:718`) with `of`, `plus` and `fits`; `assemble`'s running body measure; `fit_differential` (`:1479`): the differentials, the worked decision and the two-sided counter |
| core `tests/flow_read.rs` | the deep chain of 20 000 (`:1062`); the ratio (`:1170`) |
| core `tests/tokens.rs` | the halves (`:310`, `:324`, `:367`) |
| cli `tests/flow_linear.rs`, `tests/flow_linear_before.json` | the golden's cases (`:410`) and its four replays |
