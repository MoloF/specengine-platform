---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval]
ref: RM-0083; v4 step 2a of the owner's re-scope of 2026-11-08 (08 s2 Phase 7, "v4 scope"); Q4 and Q10 are the orchestrator's working answers, awaiting the owner's review
---

# flow-scope: Flow back to three link types, `--stage` that narrows, and the flow reads measured (RM-0083)

## Why

### Requirement

The owner reported two Flow defects on 2026-11-08, and both trace back to the read.

- **Plan items fill the Top read.** The List shows rows like `RM-0023 -> RM-0021 | depends_on | x1 | docs/roadmap/RM-0023.md:9`, and plan items with no `parent:` stand as top boxes under "Not on the walk". The cause: `FLOW_LINK_TYPES` holds `depends_on` (`crates/specengine-model/src/link.rs:96-100`). The unrooted read admits every document holding or receiving a flow link (`docs/canon/spec-flow.md` "The read"), and every plan item holds a `depends_on`.
- **The Stage filter does nothing.** A project's stages sit on steps inside groups. At Top every box is a group with no stage of its own, and where the filter does apply it only dims leaves (`docs/canon/spec-flow-screen.md` "Stage filter"). A filter must narrow the view. The writer studio (2b, RM-0084) needs a read that returns one stage's boxes and nothing else.

Before the studio puts more reads on every scope change, it also needs to know their cost. The flow, `spec trace --summary` and tree reads are whole O(N) reads and have never been timed (`docs/canon/scale-eval.md` "Known limits").

### Decisions

- **D1 Back to ADR-0055's three (Q4, working answer).**
  - ADR-0055's title, `docs/canon/architecture.md#universal` and `docs/canon/flow-links.md` say three types: `follows`, `uses`, `supplies`.
  - `flow-read` (RM-0043) added `depends_on` as a fourth, "requirement" (`docs/features/flow-read.md` "Why", AC-02), and `spec-flow.md` and `spec-flow-layout.md` followed it. The canon has disagreed with itself since then.
  - The split is resolved toward the ADR. A plan's order is `depends_on`'s (`docs/canon/spec-roadmap.md` "Plan"); a flow's is `follows`'s.
  - No new ADR: the decision of record stands, and only code and canon return to it.
- **D2 `--stage` cuts the shown nodes.** It does not change the read.
  - The scope, the walk, the ten gap rows, the fold by depth, `left_out` and `notes` are computed exactly as without it.
  - Of the shown nodes, it keeps those whose own `stage:` is S, compared byte for byte as `spec trace --stage` compares (`docs/canon/spec-trace.md` "The read"). `-` keeps the shown flow steps with no stage.
  - It keeps the edges whose two ends are kept.
  - Each kept node and edge is byte-equal to its object in the read without `--stage`. A stage's box keeps its place in the walk, and the sidebar and the canvas agree.
- **D3 Gap rows never change**, as with `--depth` (`spec-flow.md` "The read", "Depth").
- **D4 Nothing is echoed.** The JSON keeps its 11 keys and the text its lines. `spec trace --stage` echoes nothing either, and the caller knows what it asked. No daemon key, UI type or `fixtures/daemon-keys.json` entry changes.
- **D5 The synthetic corpus gains a flow shape.** With `depends_on` out of the read, today's generator writes no flow link at all, so a timed flow read would read an empty flow. The analysis did not count this work: about 0.5-1 pipeline hour more.

### Assumptions

- **A1** No project relies on `depends_on` edges on Flow for its step order. In the owner's real corpus, checked read-only on 2026-11-08, `depends_on` appears only in plan items, and steps are ordered by `follows`. Restoring three types therefore loses no step order there.
- **A2** The studio (2b) decides which reads it makes on a stage change, for example a stage under the scope's ROOT. This step serves the read and measures its widest form.

## Description and interactions

In scope:
- `FLOW_LINK_TYPES` back to three;
- `spec flow --stage S` (`-` for none), MCP `get_flow {stage}`, and the daemon's `GET .../flow?stage=`;
- five timed reads in `specengine-eval scale`, over a corpus with a flow shape.

What changes for a reader:
- An unrooted `spec flow` no longer admits a document whose only flow-like link is `depends_on`.
- No read shows a `depends_on` edge or an `inner` entry of that type.
- `spec graph`, `spec show --links`, the bundle, impact, `spec roadmap` and `depends-cycle` read `depends_on` exactly as before: each uses its own table (`LINK_TYPES`, `BUNDLE_LINK_TYPES`, `IMPACT_LINK_TYPES`), not `FLOW_LINK_TYPES`.
- `spec trace` is unchanged: its scope and lanes read no flow-link table.

The UI does not change in this step. `HttpClient.getFlow` sends no `stage` until 2b. The UI mocks keep two `depends_on` flow edges (`ui/src/mocks/flow.ts`), which the daemon no longer serves; 2b reworks the mocks and drops them. `FLOW_DASHES` keeps four dashes by a type's place, of which a read now fills three. That is no defect: `labels.ts` spells no type.

## Data

### The model

```rust
pub const FLOW_LINK_TYPES: [(&str, Direction); 3] = [
    (FOLLOWS, Direction::In),
    (USES, Direction::Out),
    (SUPPLIES, Direction::Out),
];
```

### Core

`FlowAsk` gains a `stage` field, for example `stage: Option<FlowStage<'r>>` with `enum FlowStage<'r> { Named(&'r str), Unstaged }`. The names are the developer's to choose.

`flow_read` applies it after `shown(...)`, which is where the fold is done:
- `Named(s)` keeps a node whose own `stage` is `Some(s)`;
- `Unstaged` keeps a node whose own stage is `None` and whose document is in `steps`;
- an edge stays when both its ends are kept nodes.

Neither `flow_walk` nor `FlowWalk` changes.

### CLI

```text
spec flow [ROOT] [--depth N] [--stage S] [--trace] [--json]
```

- `--stage S`: "Keep the nodes of this stage, `-` the flow steps with none; the gap rows stay whole."
- `FlowRequest` gains `stage: Option<String>`, and the CLI maps `-` to `Unstaged`.
- S is any text, in any script, compared byte for byte with no trim, case fold or NFC.
- The text lines are unchanged: the header's `nodes <n>, edges <e>` and the truncation line count what is kept, and the cut at `OUTPUT_CAP_CHARS` applies to what is kept. The JSON's keys are unchanged.
- The exits are `spec-flow.md`'s. A ROOT naming nothing is exit 1 whatever the stage, and a stage nothing carries is 0 nodes with exit 0.

### MCP and the daemon

`FlowArgs` gains `stage: Option<String>`, documented as "Keep the nodes of this stage; `-`: the flow steps with none. The gap rows stay whole." The output schema is still `FlowDocument`.

`INSTRUCTIONS` stays byte for byte. It is at its 2 048 B cap, and `get_trace`'s `stage?` and `summary?` already live in its schema alone (`docs/canon/mcp-read.md` "Texts"). The hub's `/mcp` serves the same tool, so it gains the argument with no code of its own.

The daemon's route becomes `args("flow", &["root", "depth", "stage", "trace"])` with `stage: args.text("stage")?`:
- `?stage=gather` answers `spec flow --stage gather --json` in the browser view;
- `?stage=-` answers `-`;
- a repeated `stage` gets 400, "`stage` is given 2 times: it takes one value".

### Example (`fixtures/trace-almanac`)

The fixture is the one at its own `docs/spec/`:
- BOOK-ALMANAC holds PART-GATHER and PART-PRINT;
- STEP-CALL (`gather`) and STEP-SET (`print`) sit under PART-GATHER;
- STEP-SORT (`gather`), STEP-READ (no stage) and STEP-BIND (`print`) sit under PART-PRINT;
- the walk is STEP-CALL 0, STEP-SORT 1, STEP-READ 2, STEP-SET 3, STEP-BIND 4 (end).

```text
$ spec flow --stage gather
flow all | nodes 2, edges 1, depth -
gap ... (the ten rows of `spec flow`, byte for byte)
0 STEP-CALL | ... | docs/spec/step-call.md:<line> | start | group PART-GATHER | stage gather
1 STEP-SORT | ... | docs/spec/step-sort.md:<line> | group PART-PRINT | stage gather
STEP-CALL -> STEP-SORT | follows | x1 | docs/spec/step-sort.md:<line>
```

Under `--stage`, `group` names the holder as the read without it shows it. That holder may not be among the kept nodes: here PART-GATHER and PART-PRINT are not.

### The synthetic corpus (`crates/specengine-eval/src/scale/generate.rs`)

Added to every part file, next to what `docs/canon/scale-eval.md` "Corpus" lists:
- **Heads.** In each directory `areas/aNN/gMMMM/`, the part file with the smallest k is the head.
- **`parent:`** Every other part document names its head's document ID.
- **`follows`.** Every other part document follows the document of the part file before it in its directory, in `links` next to its `depends_on`. Each directory is therefore one chain under its head.
- **`stage:`** Every part document takes word `floor(k / 16) mod 8` of eight two-syllable words. The words come from the seed alone, like the READMEs, so they are equal at every size, and a stage cuts across every directory and area.

The vocabulary rule holds: the words are syllable words, and `parent`, `follows` and `stage` are front-matter keys. `spec check` stays clean (SE-3). Only `ref-superseded` judges a target's status, so a Tier 3 head or predecessor draws no finding. The flow read leaves such a document out, as it leaves out any Tier 3 document (`left_out`).

### The timed reads

Five reads are timed as the four are: called through the CLI library as MCP calls them (`Capped`), warmed once, then the best of three.

| Key | Call | Stands for |
|---|---|---|
| `get_flow_top` | `flow`, no ROOT, `depth: 0`, `trace: true` | the Flow screen's Top read |
| `get_flow_root` | `flow`, ROOT the first directory's head, `depth: 1`, `trace: true` | a group's read |
| `get_flow_stage` | `flow`, no ROOT, `stage` the first word, `trace: true` | the widest stage read |
| `get_trace_summary` | `trace`, `summary: true` | the lanes |
| `get_tree` | `tree`, no ROOT, `depth: 1` | a module bar's read |

Where they appear in the output:
- each is in `reads.<key> {median_us, max_us}` (one target, so both are that target's best) and in `growth.<key>`;
- `calls` holds their timings and their answers' BLAKE3;
- `nondeterministic` covers them.

They get no `steps` replay, no `other_us`, no `reads_trusted` twin and no bar. On a hub, `get_flow` and `get_trace` walk (`docs/canon/spec-read-freshness.md` "Routes and tools"). stdout still holds numbers only, and the stderr summary line is unchanged.

## Rules and edge cases

- **F1** `FLOW_LINK_TYPES` SHALL hold exactly `follows` (In), `uses` (Out) and `supplies` (Out), in that order.
- **F2** WHEN `--stage S` is given, the read SHALL answer the document of the same read without it, with `nodes` and `edges` kept as D2 says, and the header's counts, `truncated`, `omitted_nodes` and `omitted_edges` computed over what is kept.
- **F3** The ten gap rows, `left_out`, `notes`, `root`, `reason` and `depth` SHALL be byte-equal to the read's without `--stage`.
- **F4** Each kept node SHALL keep the same `distance`, `group`, `start`, `end`, `holds`, `inner`, `gaps`, `rollup` and `counts`.
- **F5** `-` SHALL keep only shown nodes whose own document is a flow step with no `stage:`. A group or a definition with no stage is not kept.
- **F6** The CLI, `get_flow` and the daemon's route SHALL answer one document for one stage.
- **F7** No source SHALL name a stage, kind or fixture word (`check_genre.rs`, `no_domain.rs`, the CLI and MCP genre scans).

Edge cases:
- **A literal `stage: "-"`** cannot be asked, since `-` asks for the steps with none. This is the same limit as `spec trace`'s (`spec-trace.md` "Known limits").
- **`--depth N` with `--stage`:** the fold comes first, so only shown nodes at depth N or less are judged, each by its own stage. On trace-almanac, `--depth 1 --stage gather` with no ROOT keeps nothing: BOOK and the two PARTs carry no stage. `spec flow PART-PRINT --depth 1 --stage gather` keeps STEP-SORT.
- **A `[flow]` table with an error** marks its rows `cannot`, as without `--stage`; the cut still applies.
- **The cost.** The read's work does not shrink: only the payload is bounded, by the stage's size. On a hub, every read takes the project's one turn (`docs/canon/serve.md` "Concurrency"). `get_flow_stage` measures the widest form.

## Acceptance criteria

The CLI, MCP and http criteria run on scratch copies of the named fixtures. M names the mutations that must turn each criterion red.

- [ ] **FS-1 Three types.** The pin `crates/specengine-core/tests/flow_read.rs:236` asserts F1's array. M1: `("depends_on", Direction::Out)` restored.
- [ ] **FS-2 `depends_on` alone is no flow.**
  - (a) A scratch copy of `fixtures/flow-gaps` gains `docs/spec/wing-annex.md`: `id: WING-ANNEX`, `class: canon`, no `parent:`, `links: depends_on: [WING-LEND]`. There:
    - WING-ANNEX is no node of the unrooted `spec flow --json`, and no edge names it;
    - `spec graph WING-ANNEX` still lists its `depends_on` edge to WING-LEND;
    - the read's edges hold exactly the types `follows`, `uses` and `supplies`, and no node's `inner` names `depends_on`;
    - `LND-HAND -> PLT-MARK | depends_on` is gone, while PLT-MARK stays a node, selected by `[[flow.definition]]`.
  - (b) On `fixtures/roadmap-a`:
    - the unrooted `spec flow` has no node: its plan items hold `depends_on` alone;
    - `spec roadmap` is byte-equal to `expected.txt`;
    - `spec check` still reports `depends-cycle` for RM-0020 and RM-0021.
  - M1, and M2: the scope admitting a document by any `LINK_TYPES` entry.
- [ ] **FS-3 A stage across groups** (trace-almanac). `spec flow --stage gather --json`:
  - holds exactly STEP-CALL (`group` PART-GATHER, `distance` 0, `start` true) and STEP-SORT (`group` PART-PRINT, `distance` 1);
  - holds exactly the edge `STEP-CALL -> STEP-SORT` `follows`, and no group node;
  - its text header reads `nodes 2, edges 1`.
  - M3: groups kept although they have no stage of their own. M4: `group` computed over the kept nodes (null).
- [ ] **FS-4 The rows stand.** For `--stage gather`, `--stage print`, `--stage -` and `--stage bind`, both the text `gap` lines and the JSON `gaps`, `left_out` and `notes` are byte-equal to the read's without `--stage`. M5: rows computed over the narrowed scope.
- [ ] **FS-5 None, and an unused stage.**
  - `--stage -` gives STEP-READ alone, with no edge.
  - `--stage bind` gives 0 nodes and 0 edges, the ten rows, and exit 0.
  - `spec flow NOPE --stage gather` exits 1 with `reason`, as without `--stage`.
  - M6: `-` read as a literal stage (0 nodes). M7: `-` keeping non-steps (BOOK-ALMANAC and the PARTs listed).
- [ ] **FS-6 The surfaces agree.** On trace-almanac served as its own slug:
  - `GET .../flow?stage=gather` and `?stage=-` are byte-equal to `spec flow --stage gather --json` and `--stage - --json`;
  - `get_flow {stage: "gather"}`'s `structuredContent` equals that JSON, and its content equals the text;
  - `?stage=gather&stage=print` gets 400, "`stage` is given 2 times: it takes one value".
  - M8: the route parsing `stage` but not passing it on (the whole read is answered). M9: `FlowArgs.stage` dropped.
- [ ] **FS-7 The flow reads are measured.**
  - SE-2's shape gains the heads, `parent:`, `follows` and `stage:` above. It also asserts that a stage holds documents from at least two directories.
  - SE-5's debug run at 1k prints `reads` and `growth` entries for the five keys, and `get_flow_top`'s and `get_flow_stage`'s answers each hold at least one node.
  - The ignored `scale_baseline` prints all five at 1k, 10k and 100k. No test asserts a time.
  - M10: one key missing from the table. M11: the generator writing no `follows` (Top answers no node). M12: `stage` chosen per directory (one directory per stage).
- [ ] **FS-8 The golden, re-pinned.** `crates/specengine-cli/tests/flow_linear_before.json` is re-recorded at this slice's head.
  - A test compares each `flow` row's old JSON and new JSON:
    - the gap rows are byte-equal;
    - every node kept has the same `distance`, `start`, `end` and `stage`;
    - every `follows`, `uses` and `supplies` edge of an uncut old row is in the new one;
    - every removed item is a `depends_on` edge or `inner` entry, or a node that only such edges admitted.
  - Every `trace` and `bundle` row stays byte-equal.
  - M13: a re-record that also drops a `follows` edge.
- [ ] **FS-9 The fold comes first** (trace-almanac).
  - Under `--stage print` and under `--stage gather --trace`, each kept node's and edge's JSON object is byte-equal to its object in the same read without `--stage`. Under `print`, STEP-SET keeps `distance` 3 and STEP-BIND keeps `distance` 4 and `end` true.
  - `--depth 1 --stage gather` with no ROOT gives 0 nodes; `spec flow PART-PRINT --depth 1 --stage gather` gives STEP-SORT alone.
  - M14: the walk recomputed over the stage's documents (STEP-SET reads 0). M15: the cut made before the fold (`--depth 1 --stage gather` lists STEP-CALL and STEP-SORT).
- [ ] **F-form.** `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` are clean, and no dependency is added. The default `cargo nextest run` passes, and so do `pnpm --dir ui test` (unchanged) and `spec check` (0 errors, 0 warnings).

### Fixtures and tests to add (test-engineer)

Fixtures:
- `fixtures/trace-almanac`: FS-3 to FS-6 and FS-9;
- `fixtures/flow-gaps`, plus the scratch document WING-ANNEX of FS-2 (a): it is written into the copy, never committed, so no golden row or `expected.json` moves;
- `fixtures/roadmap-a`: FS-2 (b);
- the synthetic corpus: FS-7.

There is no new committed fixture.

Tests:
- core `tests/flow_read.rs`: FS-1's pin. The CLI test holding flow-read's AC-02 ("all four types") moves to FS-2 (a)'s three.
- CLI `tests/flow_scope.rs` (new): FS-2 to FS-5 and FS-9.
- MCP `tests/mcp_flow.rs` and http `tests/flow.rs`: FS-6. A pinned tool schema, if any, moves with `stage`.
- CLI `tests/flow_linear.rs` and its golden: FS-8.
- eval `tests/scale_cli.rs`: SE-2, SE-5 and `scale_baseline` for FS-7. `scale_edges.rs`, `store_reads_scale.rs` and `read_trust_scale.rs` are re-run unchanged on the new corpus.

## Out of scope

- `[[flow.exclude]]`, or any way to drop a kind or type from a read.
- `--trace`'s quadratic counts: they are measured here, not fixed (RM-0070, `spec-flow.md` "Known limits").
- The UI: the Stage filter, the mocks' `depends_on` edges and `FLOW_DASHES` all go to 2b (RM-0084).
- A stage echo in the JSON; new `stage:` semantics; a stage on a group taken from what it holds.
- A trusted twin, a step replay or a bar for the five new reads.
- The skills' texts, so the plugin version stays.

### Risks

- **R3 Scale.** Flow, trace and tree are whole O(N) reads, and only `show`, `--links` and `bundle` read a subset (`docs/canon/spec-read-subset.md`). A bounded payload is not bounded daemon work, and the screen asks for `trace` on every read. FS-7 gives 2b the numbers before it multiplies the reads.
- **R9 Output moves.** Dropping `depends_on` changes `get_flow`'s output, the 661 KB golden, flow-read's AC-02 test, and the daemon-key run's values (not its keys). FS-8 proves every change is a `depends_on` one.
- **R-corpus.** The flow shape changes the synthetic corpus, its digest and every read's figures. `follows` neighbours and `parent:` ancestors enlarge bundles, so W_after moves and RM-0054's bars are read again from the new run. The 12b baseline stays the record of its own corpus.
- **R-cap.** `spec-flow.md` has 12 B free under its 12 288 B cap, `scale-eval.md` 11 B, `mcp-read.md` 17 B, and `crates/specengine-http/README.md` 6 B. "Canon at ship" names where the room comes from.

## Canon at ship

Applied by the spec-writer when the slice ships, with `reviewed:` moved on each file touched.

- **`docs/canon/spec-flow.md`**:
  - The first line takes `[--stage S]`, `get_flow {root?, depth?, stage?, trace?}` and `GET .../flow?root=&depth=&stage=&trace=`.
  - "Typed edges" becomes: "Every resolved link of `FLOW_LINK_TYPES`, ADR-0055's three (`follows`, `uses`, `supplies`), with both ends in scope; `depends_on` orders a plan, never a flow".
  - "Readings": "an unresolved `uses` or `supplies` does not".
  - A new bullet after "`stage`" states D2, F2 to F5, the `group` reading and the literal `"-"`.
  - "Code" names `FlowAsk`'s stage and the cut after the fold. "Tests" adds `flow_scope.rs` and `roadmap-a`.
  - The room, about 600 B, comes from moving `## Readings` whole to a new Tier 2 `docs/canon/spec-flow-readings.md`, which `docs/index-canon-spec.md`'s glob claims, and from fixing its citations.
- **`docs/canon/spec-flow-layout.md` "Forms"**: "four by type" becomes "four by a type's place, of which a read fills three".
- **`docs/canon/mcp-read.md`**:
  - line 15 takes `get_flow {root?, depth?, stage?, trace?}`;
  - "Texts" names `get_flow`'s `stage?` beside `get_trace`'s as living in its schema alone;
  - the paragraph is shortened by the bytes it lacks.
- **`crates/specengine-http/README.md` "Endpoints"**:
  - the flow row's query becomes `root`, `depth`, `stage`, `trace` (+9 B);
  - "Live tail"'s last line merges into its paragraph, "...after 300 s; types and polls: `serve.md` "Live tail"." (−26 B).
- **`docs/canon/scale-eval.md`**:
  - "Corpus" takes the flow shape;
  - "Reads and steps" takes the five keys;
  - "Output" and "Known limits" are updated: "Not timed" loses flow, trace and tree;
  - "Baseline" takes the new run at this slice's head, all nine reads at 1k, 10k and 100k. The 12b tables move to `docs/features/scale-eval.md`, the spec that measured them.
- **Documents brought in line**:
  - `docs/features/flow-read.md`: "Why" drops "and requirement", and AC-02 reads three types, naming this slice;
  - `docs/features/flow-linear.md`: the golden's line names its re-record.
- **No change**: `docs/canon/flow-links.md` and `docs/canon/architecture.md`, which already say three, and `docs/canon/spec-trace.md`.

## Implementation

Filled in after implementation.
