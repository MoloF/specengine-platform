---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, docs]
ref: the owner's word of 2026-11-13 ("close the tails and problems so the full release is complete and everything works without errors"); RM-0063; the orchestrator's hardening scan of 2026-11-13 (outside the repository) and its rulings
adrs: []
---

# release-hardening: every known flake fixed, the scan's defects closed, one clean full run

## Why

`v5` ended with `delivery-timeline`, and the owner asked to close the tails so the release is complete and runs without errors. Under load about fifteen tests failed on fixed deadlines or starved threads (RM-0063), and a read-only scan of 2026-11-13 found thirteen defects, twelve taken here. Flakes are fixed in the tests, never in the product, with one exception the owner approved that day: `GATE_DEADLINE` from 3 s to 5 s (R8). Four lanes: T (test-engineer), U (UI), R (Rust), D (canon); one gate, RH-G1, a clean full run on the committed tree. No ADR: no rule of `docs/canon/architecture.md` changes; no plugin bump, queue schema, backup format or served key.

## Acceptance criteria

**[look]** marks a browser look, **[measure]** a load run or timing; every other criterion is a test its named mutation turned red. Code comments cite this heading with an ID (`doc_pointers`). A load run: the test binary built first, then `hw.ncpu` copies of `perl -e 'alarm 120; exec "yes"'`, the named test with `--retries 0` repeatedly within those 120 s.

**Lane T** (test-engineer's final report, 2026-11-13)

- [x] RH-T1 — **show_bindings.** `sleep(PAST_RACY)` (2.1 s) and one warm `spec show` before the loop; load 10/10.
- [x] RH-T2 — **gate_wrapper.** The stalled ceiling LIMIT + 2.5 s (9.5 s, under the hook's 10 s); the stopped hook: `spec` and the watchdog 1.1 s, its orphaned `sleep 1`s 3 s; load 5/5. Mutation `gate.sh`'s limit 10 s red. **Survives:** "the watchdog left alive", because the watchdog's own ppid check ends it within 1 s; red only with that check removed too (known limit).
- [x] RH-T3 — **No verdict by deadline, asked again.** `tests/common/gate.rs` (`VERDICT_TRIES` 5, `starved`, `until_verdict`) and `rh_t3_only_a_no_verdict_by_deadline_is_asked_again` (3 calls, 1 call, 5 calls); 13 users plus 2 RH-R8 tests; `ac01`, `ac06` never retry; load 5/5. Mutations red: `starved` on any exit 2; `VERDICT_TRIES` 1.
- [x] RH-T4 — **Push deadline.** http `spec_ids_hub.rs` `PUSH` 30; `ac1_*`, `ac2_*` load 5/5.
- [x] RH-T5 — **Tail.** http `tail.rs` `LIVE` 3 s; load 5/5. Mutation red: no poll after the first read.
- [x] RH-T6 — **FIFO.** `round.rs` loops until both outcomes are seen or 60 s pass, a 20 s hang check per round; load 5/5. Mutation red: no `O_NONBLOCK`.
- [x] RH-T7 — **ac6g.** A failed premise runs the scenario once more from a fresh place (`ac6g_a_failed_premise_runs_once_more_from_a_fresh_place`); a state assertion never retries. Mutation red: `IndexTrust` walking on every read.
- [x] RH-T8 — **RM-0063's rest.** `failing_test_removes_evidence` `[verify] timeout` 5; load 5/5. Mutation red: a timed-out node keeping its block. `drift_dedup` `concurrent_raises_store_one_item` (the store's 5 s busy timeout, a product limit), `worktree_git` `new_until_kills_a_slow_git_at_the_deadline` and `serve.rs` `d4_…` (never reproduced) unchanged. No `.config/nextest.toml` (no role owns it).

**Lane U**

- [x] RH-U1 — **Quotes keep their line feeds:** `.evidence .pairs dd { white-space: pre-wrap; overflow-wrap: anywhere; }`, quotes only. Mutation red: the rule removed.
- [x] RH-U2 — **An option-less gist question** shows its working answer and price after "Why it matters", outside the fold (`AnswerSections` with a nulled copy); "No options attached." gone; PQ12's looks unchanged. Mutations red: the answer only in the fold; in both.
- [x] RH-U3 — **A fileless pair side** (`path: null`) has no "No evidence cites this file"; a side with a path and no position keeps it.
- [x] RH-U4 — **A gist as the lead draws it, everywhere:** `inbox/ProposalSummary.tsx` on six surfaces, a gist in `<bdi>` through `shownSlot`. Mutation red: any surface on raw `summaryOf`.
- [x] RH-U5 — **Opened by an alias:** `trace/holderRow.ts` matches the row by the holder's ID, an ID-less one by path; `DocMeta` and `CardsBlock` read it. Mutation red: `node.id === docRef`.
- [x] RH-U6 — **The MVP cut's label in its own place:** a node above the lane heads, outside `[0, LANE_HEAD)` (`RoadmapChain.test.tsx`). Mutation red: the label at `top: 0`.
- [x] RH-U7 — **UI gates.** Lint, `tsc`, `pnpm test`, `pnpm build`; no catalog key; first load +251 B over 93ce8555 (≤ 0.3 kB).

**Lane R**

- [x] RH-R1 — **Refused at intake.** `text` `"a\r\nb"` exits 1 `text: holds U+000D: a decision record never carries it`, nothing stored; alike `options[1].label` U+202E, `evidence[0].observed` U+0007, a decision's `why` U+000D, a departing accept's `--summary` U+000D; stdio and a hub's `/mcp` the same line; an LF-only item applies. Mutations red: `observed` unchecked; the predicate `== '\r'`.
- [x] RH-R2 — **`--login` takes no flag.** `spec workspace --login --json` exits 2 naming `--login`, stdout empty; `--login=-x` asks for `-x`. Mutation red: `allow_hyphen_values` back.
- [x] RH-R3 — **An ID-less document refreshes its cards.** An `--editorial` approve on `docs/notes/plain.md` writes one `card.refreshed` naming the path and leaves `C-0001` clean; a meaning approve writes no `rev:` and leaves it suspect. Mutations red: pairing by ID only; the path pair passed to the raise.
- [x] RH-R4 — **A bad split row: a finding, splitting off, nothing else.** One `card-config` line (`` error  specengine.toml:63: card-config: unknown field `disciplnes`, expected `kind` or `disciplines` ``); `CHECK_CODES` 38; every other card surface, the gate, `spec workspace`, `spec delivery`, `spec card counts` byte for byte as without the row; `spec card split` (dry run too) exit 2, the route 503, `split_card` an error result; `spec init --pack` refuses. Mutations red: split errors joined to `[card]`'s; the finding dropped; `init` not judging the split.
- [ ] RH-R5 — **Delivery reads one snapshot.** Store `queue_snapshot.rs`: a second connection's `card.accepted` and question stay unseen by the handle's reads and seen by a new handle; DA-10's seam counts unchanged. Mutation red: each read on its own transaction. **Owed:** the DT3 re-measure (release); the CLI call site is caught only by the store test.
- [ ] RH-R6 — **Collision names count every holder.** `R-01` (also Tier 3) and `R-02` (also generated) are W1; `feat/R-97` named; each printed name shows one node. Mutation red: `outside_ids` not counted. **Owed:** the eval `reads.collisions` medians at 1k, 10k, 100k (release).
- [ ] RH-R7 — **Rust gates.** Clippy clean; `plugin.json` and `PINS` as `usage-orphans` leaves them (0.46.0); `fixtures/daemon-keys.json` unchanged by this release. The full run under load (load average 24-32): 6 037 run, 10 failed, none this release's (see Implementation). The clean run is RH-G1.
- [x] RH-R8 — **The gate's deadline is 5 s.** `GATE_DEADLINE` 5 s; `spec gate --stdin` with stdin open exits 2 `not read to its end within 5 s` in 5.0-5.5 s; `tests/gate.rs` words its lines through `GATE_DEADLINE.as_secs()`; a test reads `limit=` 7 from `gate.sh` and `timeout` 10 from `hooks.json` and asserts deadline + 400 ms < limit < timeout (`plugin_files.rs` `GATE_DEADLINE_SECONDS` 5). Mutations red: 3 s; 7 s.

**Lane D**

- [x] RH-D1 — **Versions.** `hub-deploy.md` "Image" names 0.46.0; `card-mandate.md` "Known limits" and `CLAUDE.md` say the hub runs 0.45.0 (schema 14, format 7; upgraded 2026-11-13, a dump first) and its next upgrade, to 0.46.0, has no schema or format step, pointing at `hub-ops.md` "Upgrade"; no "to 0.38" in `CLAUDE.md` or `docs/canon/`.
- [x] RH-D2 — **Closed limits leave the canon:** every row of the plan's canon table done; none of the ten closed-limit phrases is found.
- [x] RH-D3 — **RM-0063 closed** with each entry's disposition.
- [x] RH-D4 — **The check:** `export index`, then `check`, 0 errors; every edited canon file within its cap.

**Gate**

- [ ] RH-G1 — **One clean full run**, on the owner's commit holding every lane and the canon, nothing else running: `cargo nextest run --workspace --retries 0 --no-fail-fast` (0 failed, skipped equal to the `#[ignore]` count), UI lint and `tsc`, `pnpm test` (no `.skip(`, `.only(`, `.todo(`), `pnpm build`, `export index` and `check`. Runs after the commit, on an idle machine; a failure goes to its lane.

## Out of scope

The pair side's found flag (2a.7, a served field); the collision-pass flood and its cap of 200; the hub's upgrade (the owner's, done to 0.45.0 on 2026-11-13), the tags and the push of `main`; gate 11b-3 and the pipeline trial; RM-0039, RM-0001; the Delivery tab without `[roadmap]`; the store's 5 s busy timeout; the scan's minor items; `stash@{0}` and the never-run 10k differential; Vite's chunk-size warning.

## Implementation

| Module | What it does |
|---|---|
| core `intake.rs` | R1: every field a record carries judged with `record::refused_char` after its cap, `lines` before its grammar |
| CLI `main.rs` | R2: `--login` without `allow_hyphen_values` |
| core `raise.rs`, CLI `revisions.rs` | R3: `idless_documents`, `Revisions.document`: the path pair for the editorial refresh |
| core `card_toml.rs`, `check/mod.rs`, `project_toml.rs`; store `check.rs`; CLI `card.rs`, `init.rs` | R4: `CardConfig.split_problem`; `CheckSetup.card`, `card-config` the 38th code; the split and `init --pack` refuse |
| store `queue/cards.rs`, `queue.rs`, `lib.rs`; CLI `delivery.rs` | R5: `SqliteQueue::snapshot` -> `QueueSnapshot`, one deferred read |
| store `read.rs`, core `collisions.rs` | R6: `CollisionRows.outside_ids` in `collision_input`'s transaction |
| CLI `gate.rs` | R8: `GATE_DEADLINE` 5 s; the backstop 5.4 s follows |
| UI `styles/app.css`, `inbox/{ProposalCard,PairView,ProposalSummary,ProposalList}.tsx`, `inbox/summary.ts`, `tree/{ProposalsPanel,NodePane}.tsx`, `health/LeftRegion.tsx`, `palette/Palette.tsx`, `doc/DocBody.tsx`, `trace/{CardsBlock.tsx,holderRow.ts}`, `roadmap/{RoadmapChain.tsx,chain.ts}` | U1-U6 |
| tests | lane T's files and `common/gate.rs`; RH-R tests in core, store (`queue_snapshot.rs`), CLI, MCP, http |

**Departures**, accepted at review: a split row whose keys already err names no missing key; no editorial note when an ID-less document's own hash moved, its cards refreshing; R1's check runs before `lines`' grammar; R6's mark wording unchanged; `tree/NodePane.tsx` (outside the plan's files) passes `holders[0]` for RH-U5.

**The full run under load**, 10 failures, none this release's: two `usage-orphans` pins (`kind_amendment` 151, `.env.example` 0.46.0) and MCP `review_form`, re-pinned; three http `ui` t3, `ui/dist` rebuilt mid-run; four wall-clock bounds outside RM-0063, each passing on rerun and watched in RH-G1: core `cost` ×2 (2 s), code `hash_levels_depth` (5 s), http `live` (17 s).

**Known limits:** the T2 survivor; R5's CLI call site covered only by the store test; `Names::end` counts universe holders alone; the Inbox's `DecisionDialog` and `ConfirmDialog` draw their summary raw; an item stored before R1 still stages and never applies (`reject:`).

**Canon:** `gate.md` "Verdict"; `agent-intake.md` "Rules"; `round.md`, `spec-trace.md`, `spec-delivery.md`, `collision-pass.md`, `plain-questions.md`, `doc-view.md`, `spec-roadmap-screen.md` (their closed limits); `card-split.md` and `card-flows.md` "Config"; `spec-check.md`, `spec-check-process.md`, `hub-team.md` "Check"; `proposal-revisions.md` "The raise"; `agent-intake-documents.md` "Pair"; `hub-department.md`; `hub-deploy.md`, `card-mandate.md`; the store README.
