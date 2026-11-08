---
class: spec
status: shipped
shipped: 2026-11-09
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval]
ref: RM-0083; v4 step 2a of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"); Q4 and Q10 are the orchestrator's working answers, awaiting the owner's review
---

# flow-scope: Flow back to three link types, `--stage` that narrows, and the flow reads measured (RM-0083)

## Why

The owner reported two Flow defects on 2026-11-08, both in the read:
- **Plan items filled the Top read.** `FLOW_LINK_TYPES` held `depends_on`, which `flow-read` (RM-0043) had added as a fourth type against ADR-0055's three, so every plan item entered the unrooted read and plan items with no `parent:` stood as top boxes.
- **The Stage filter did nothing.** Stages sit on steps inside groups: at Top every box is a group with no stage, and the filter only dimmed leaves. The writer studio (2b, RM-0084) needs a read that returns one stage's boxes and nothing else.

The flow, `spec trace --summary` and tree reads were whole O(N) reads never timed, and the studio puts more of them on every scope change.

Decided (the orchestrator's working answers, awaiting the owner's review):
- **D1** Back to ADR-0055's three (Q4). A plan's order is `depends_on`'s, a flow's `follows`'s. No new ADR: the decision of record stands, code and canon return to it.
- **D2** `--stage` cuts the shown nodes after the fold and changes nothing else; each kept node and edge is byte-equal to its object without it. **D3** Gap rows never change. **D4** Nothing is echoed: no key, UI type or `fixtures/daemon-keys.json` entry changes.
- **D5** The synthetic corpus gains a flow shape: without `depends_on` it held no flow link.

## Description and interactions

How it works now: `docs/canon/spec-flow.md`, `docs/canon/spec-flow-readings.md`, `docs/canon/scale-eval.md`. The other `depends_on` readers (`graph`, `--links`, the bundle, impact, `roadmap`, `depends-cycle`), `spec trace` and the UI are unchanged: the Stage filter, the mocks' `depends_on` edges and `FLOW_DASHES` are 2b's (RM-0084).

## Data

`spec flow [ROOT] [--depth N] [--stage S] [--trace] [--json]`, `get_flow {stage?}`, `GET .../flow?stage=`: `spec-flow.md` "Code", `mcp-read.md` "Texts". No format or key changes; plugin 0.29.1 -> 0.30.0.

### The synthetic corpus

`docs/canon/scale-eval.md` "Corpus", "Flow".

### The timed reads

`docs/canon/scale-eval.md` "Reads and steps", "Output", "Baseline".

## Rules and edge cases

**F1** the three, in order; **F2**-**F5** the cut as `spec-flow.md` "The read" states it, edge cases too, **F3** the rows, `left_out` and `notes` unchanged; **F6** one document per stage on every surface; **F7** no stage, kind or fixture word in a source.

## Acceptance criteria

Scratch copies of the named fixtures; no new committed fixture. Every named mutation, M1-M15, was killed. M5 is caught only through `--stage bind` on trace-almanac, since a `follows` edge crossing a scope counts as an entry or a way out.

- [x] **FS-1 Three types.** The pin `crates/specengine-core/tests/flow_read.rs:236` asserts F1's array. M1: `("depends_on", Direction::Out)` restored.
- [x] **FS-2 `depends_on` alone is no flow.** (a) A scratch flow-gaps gains WING-ANNEX (`class: canon`, no `parent:`, `depends_on: [WING-LEND]`): no node or edge of the unrooted `spec flow --json` names it; `spec graph WING-ANNEX` still lists that edge; the edges hold only `follows`, `uses`, `supplies`, no `inner` names `depends_on`; `LND-HAND -> PLT-MARK | depends_on` is gone, PLT-MARK still a node by `[[flow.definition]]`. (b) On `roadmap-a`: the unrooted `spec flow` has no node; `spec roadmap` is byte-equal to `expected.txt`; `spec check` still reports `depends-cycle` for RM-0020 and RM-0021. M1; M2: the scope admitting by any `LINK_TYPES` entry.
- [x] **FS-3 A stage across groups** (trace-almanac). `spec flow --stage gather --json` holds exactly STEP-CALL (`group` PART-GATHER, `distance` 0, `start`) and STEP-SORT (`group` PART-PRINT, `distance` 1), the edge `STEP-CALL -> STEP-SORT` `follows`, no group node; the text header reads `nodes 2, edges 1`. M3: groups kept. M4: `group` over the kept nodes.
- [x] **FS-4 The rows stand.** For `--stage gather`, `print`, `-` and `bind`, the text `gap` lines and the JSON `gaps`, `left_out` and `notes` are byte-equal to the read's without `--stage`. M5: rows over the narrowed scope.
- [x] **FS-5 None, and an unused stage.** `--stage -` gives STEP-READ alone, no edge; `--stage bind` 0 nodes, 0 edges, the ten rows, exit 0; `spec flow NOPE --stage gather` exits 1 with `reason`. M6: `-` read as a literal. M7: `-` keeping non-steps.
- [x] **FS-6 The surfaces agree.** On trace-almanac: `GET .../flow?stage=gather` and `?stage=-` are byte-equal to the CLI's `--json`; `get_flow {stage: "gather"}`'s `structuredContent` equals that JSON and its content the text; `?stage=gather&stage=print` gets the 400. M8: the route dropping `stage`. M9: `FlowArgs.stage` dropped. The plugin's MINOR rides here.
- [x] **FS-7 The flow reads are measured.** SE-2's shape gains the heads, `parent:`, `follows` and `stage:`, and a stage spans at least two directories; SE-5's 1k run prints `reads` and `growth` for the five keys, Top's and the stage's answers each holding a node; the ignored `scale_baseline` printed all five at 1k, 10k and 100k. No test asserts a time. M10: a key missing. M11: no `follows`. M12: a stage per directory.
- [x] **FS-8 The golden, re-pinned.** `crates/specengine-cli/tests/flow_linear_before.json`, re-recorded: the gap rows byte-equal; every kept node with the same `distance`, `start`, `end`, `stage`; every `follows`, `uses` and `supplies` edge of an uncut row kept; every removed item a `depends_on` edge or `inner` entry, or a node only those admitted; every `trace` and `bundle` row byte-equal. M13: a re-record dropping a `follows` edge.
- [x] **FS-9 The fold comes first** (trace-almanac). Under `--stage print` and `--stage gather --trace`, each kept node and edge is byte-equal to its object without `--stage` (STEP-SET `distance` 3, STEP-BIND 4 and `end`); `--depth 1 --stage gather` gives 0 nodes; `spec flow PART-PRINT --depth 1 --stage gather` STEP-SORT alone. M14: the walk over the stage's documents. M15: the cut before the fold.
- [x] **F-form.** Clippy `-D warnings` and fmt clean, no dependency added; the default `cargo nextest run`, `pnpm --dir ui test` (unchanged) and `spec check` (0 errors, 0 warnings) pass.

## Out of scope

`[[flow.exclude]]`; `--trace`'s quadratic counts (RM-0070); the UI (RM-0084); a stage echo, new `stage:` semantics, a group's stage taken from what it holds; a trusted twin, step replay or bar for the five reads.

## Implementation

Review: ACCEPT; nits fixed (`NO_STAGE` shared from `trace.rs`, the CLI `lib.rs` synopsis, the eval doc comments). Deliberate departures:
- **Plugin 0.30.0**, not "the version stays": `get_flow` takes `stage`, a MINOR by `plugin-files.md` "Changing it" (precedent 0.25.0, `get_flow`'s `trace`); `.env.example` moved with it for `tag-check.sh`. `v0.29.1` stays `v3`'s tag, the next is `v0.30.0`; `scripts/deploy/README.md` clones `v0.29.1` until `v4`'s first release. Pin renamed `ac8d_the_release_tag_is_the_plugin_version`.
- **`FlowRead.shown`**: `--trace` joins over the uncut list, so F3's notes hold.
- **`FLOW_DESCRIPTION`** reads "follows, uses or supplies", synopsis `[--stage S]`; `stage` in the schema alone, the description at 2 045 B of 2 048.
- **Scale**: drawn links counted, `calls` keys, the stage words, Tier 3 heads (`scale-eval.md` "Corpus", "Output").
- **FS-8**: 661 235 -> 659 231 B, 14 flow-gaps rows, each less its `depends_on` edge, header count and `inner` count; the `#[ignore]` recorder `fs8_record_the_golden` writes only outside the repository.

**Baseline** (2026-11-08, `scale-eval.md` "Baseline"): at 100k the flow reads 980-1 041 ms, trace summary 544, tree 956; the four targeted reads 52-58 ms walking, trusted 0.61-2.97, `search` 6.31; RM-0054's bars hold. Known limit for `writer-studio` (RM-0084): flow, trace and tree stay whole O(N) reads.

| Module | What it does |
|---|---|
| model `link.rs`; core `flow.rs` | three types; `FlowStage`, the cut, `FlowRead.shown` |
| CLI `flow.rs`, `trace.rs`, `main.rs` | `--stage`, `NO_STAGE`, joins over `shown` |
| mcp `read.rs`; http `app.rs` | `FlowArgs.stage`; the route |
| eval `scale/` | the flow shape, the five reads |
| tests | CLI `flow_scope.rs` (new), `flow_linear.rs`; `scale_cli.rs`; the pins |
