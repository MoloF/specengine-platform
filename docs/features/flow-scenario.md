---
class: spec
status: shipped
shipped: 2026-11-10
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: RM-0094; v5 step 2 (08 s2 Phase 7, "v5 scope"); Flow v2 step 3c and the owner's path request, both of 2026-11-10; Flow v2 Q1-Q4 and v5 Q14 at their recommendations; FS-1 to FS-24 merge the analysts' FS-1 to FS-14 and FL-1 to FL-15; W1-W7 working answers, the owner's review open
---

# flow-scenario: Flow draws the scenario, and its left pane is a path (RM-0094)

## Why

The owner looked at Flow on his own project on 2026-11-10 and called the design bad. The canvas was noisy: records and ID-less notes stood as a wall of tiny boxes (one part drew 150, none on the walk), dashed `uses` bundles carried `x13` badges, the fit shrank text to about 8 px, and the walk ran off to the right. The panel led with kind, distance, `path:line` and English gap codes: "this is information for the AI". The left pane showed a scope and an Up link guessed from held reads, not "a path that goes deeper and deeper". His answers: the canvas shows the scenario only; records go to a "Records for the AI" list and ID-less documents to the Spec tree; technical fields fold. The rule for "in the flow" had to be domain-free (ADR-0008): his records share a directory and kinds the core must never name, and what separates them is that no flow link reaches them. So the daemon serves `in_flow` and the path, and the UI only reads them (ADR-0033). The document modal and the minimal panel are RM-0095's.

How it works now: `docs/canon/spec-flow-cuts.md`, `spec-flow-bar.md`, `spec-flow-screen.md`, `spec-flow-layout.md`, `spec-flow-place.md`; `ancestors` in `spec-cli-read.md` "Output" and `spec-read-subset.md` "The subset".

## Description and interactions

Each decision is canon now; the numbers stay because code comments cite them.

- **D1** In the flow, folded like `start`: `spec-flow-cuts.md` "In the flow". **D2** The opt-in cut: "The cuts". **D3** `types`: "Count and types".
- **D4** `ancestors`, not `path`, a node's `path` being its file: `spec-cli-read.md` "Output". **D5** Built from the subset's `PARENT` rows; the walk the oracle: `spec-read-subset.md` "The subset", "The walk".
- **D6** What Flow reads, every drawing read cut, Records on demand: `spec-flow-screen.md` "Reads". **D7** Top to bottom, dividers (Q14), **D8** the 0.92 and 0.5 floors, **D9** off-walk edges and labels: `spec-flow-layout.md`.
- **D10** Gaps in the bar: `spec-flow-bar.md`. **D11** The path pane: `spec-flow-place.md` "Places". **D12** Titles, technical details folded: `spec-flow-screen.md` "Panel".
- **D13** No ADR: ADR-0008, ADR-0033, ADR-0055's three types and ADR-0012 hold. **D14** `INSTRUCTIONS` unchanged; `in_flow` in the tool schema: `mcp-read.md` "Texts".

## Data

Flow JSON (13 keys, a node 16): `spec-flow.md` "Output". Show JSON (a node 20, `ancestors` before `links`): `spec-cli-read.md` "Output". `fixtures/daemon-keys.json`: 115 sets, `ShownAncestor` `["id", "kind", "title", "path"]` last. UI forms: `spec-flow-screen.md` "Reads"; keys in `ui/src/i18n/en.ts`.

**Fixture `flow-aside`**, an invented pottery workshop with no game or pilot word: what is in the flow is told by links, `[flow]` and `parent:` alone. 181 documents, 26 in the flow; 155 out (141 records, 8 ID-less notes, 4 parts, 2 desk papers). `ATL-POTTERY` holds `KRUN-FIRING` (10 `POT-*` steps in four stages, 11 `follows` edges with an alternative and a repeat, 15 `uses`) and 14 `WBAY-*` parts; `WBAY-KILN` holds two `CONE-*` rules and the 142 records, `PUZZLE-0003` linked only by `depends_on`; the ID-less "Open day" is a `[flow]` start and end. `[paths] roots = ["docs"]`, since nothing lies in the default role directories. The 40x variant is built by the tests, never committed.

## Acceptance criteria

Each named mutation turned its test red unless said otherwise. UI criteria FS-16 to FS-21 run over `ui/src/test/asideStub.ts`, checked against `spec flow --in-flow --json` on the fixture.

- [x] FS-1 — **The fixture.** CLI `flow_scenario.rs` `fs1_the_fixture_is_clean`, `fs1_the_fixture_parses_as_expected` (0 errors, 0 warnings, `expected.json`); `anonymity.rs`; core `check_genre.rs`, http `no_domain.rs`, MCP `mcp_genre.rs`, CLI `genre.rs` (its `FORBIDDEN` gains the fixture's words). M: a pilot's name in a note; `hunch` quoted in core `flow.rs`.
- [x] FS-2 — **The rule.** `fs2_in_the_flow_is_what_the_read_with_no_root_holds` (seven fixtures, depths 0, 1, unbounded), `fs2_the_fixture_holds_twenty_six_in_the_flow` (`ATL-POTTERY --depth 1`: 24 nodes, 12 in); core `fs2_a_tier_3_root_holding_a_live_step_is_in_the_flow_by_the_fold`. Amended: a ROOT is in the flow only through what folds into it; a Tier 3 ROOT's own node is `false` unbounded, `true` at depth 0. M: membership from the rooted scope; `parent:` holders left out; `[flow]` selection left out; the fold dropped.
- [x] FS-3 — **Domain-free.** `fs3_a_records_kind_and_an_id_less_document_are_in_by_links_alone`. M: `depends_on` as a flow link; an ID-less node taken as out.
- [x] FS-4 — **The cut.** `fs4_the_cut_keeps_the_flow_and_counts_documents` (`ATL-POTTERY` 12 nodes, 14; `WBAY-KILN` 4, 141; `WBAY-OFFICE` 1, 2; `KRUN-FIRING` 11, 0; kept objects byte-equal), `fs4_with_no_root_the_cut_drops_nothing`, `fs4_the_text_header_counts_what_the_cut_left_out`, `fs4_the_stage_cuts_first` (2 nodes, 0), `fs4_a_card_on_a_dropped_record_still_counts`. M: the cut before the fold; nodes counted, not documents; the ROOT dropped; 0 without the flag. "`counts` after the cut" survives as an equivalent mutant: counts come from the graph subtree and the ROOT holder always stays.
- [x] FS-5 — **Default reads byte-equal.** `fs5_a_read_without_the_flag_prints_as_before`, `the_key_scan_keeps_every_other_byte`; goldens of `flow_linear.rs`, `trace_views.rs`, `store_reads.rs` with only the four keys stripped, per command (`tests/common/keys.rs`: `bundle` has an `ancestors` layer of its own). M: ` | in flow` on a text node line.
- [x] FS-6 — **At 40x.** `fs6_the_cut_reads_the_flow_out_of_forty_times_the_records` (4 nodes, 5 679, `truncated` false). M: the flag ignored.
- [x] FS-7 — **Types.** `fs7_every_read_serves_the_models_types`. M: `types` from the read's edges.
- [x] FS-8 — **The served path.** CLI `show_ancestors.rs`, nine tests (the table, every document, text unchanged, `--bindings` on `code-a`, `parent-ref`, dangling, two holders, a cycle once, an ID-less document by path). M: nearest first; the last holder; a cycle member twice; `ancestors` in the text.
- [x] FS-9 — **From the subset.** `src/store_reads_tests.rs` `fs9_a_show_reads_its_chain_from_the_subset` (C(REF) plus two chain files; whole decodes, graphs and walks 0); `store_reads.rs`' differential over every configured fixture. M: the chain from a whole decode; the chain left out of the plan.
- [x] FS-10 — **The twins.** MCP `fs10_get_flow_takes_in_flow`, `fs10_get_node_serves_the_ancestors` (the strict walker closes the schema); http `fs10_in_flow_is_the_clis_document_byte_for_byte` (400 on `in_flow=yes`). M: the route dropping `in_flow` (145 nodes); `ancestors` missing from `ShowNode`.
- [x] FS-11 — **Key sets and plugin.** http `daemon_keys.rs` (115); UI `daemonKeys.test.ts`, `AHEAD_OF_FIXTURE` empty; `plugin_files.rs` 0.33.0 `PINS`, `SCHEMA_PINS`; `plugin_skills.rs`; `mcp_decision.rs`, `mcp_path.rs` pins kept. M: the schema changed at 0.32.0; a key missing from the fixture.
- [x] FS-12 — **[measure] Cost.** Below: every p50 within 10 %. M: membership from a second `flow_read`.
- [x] FS-13 — **Flow's reads.** `FlowView.test.tsx` "the reads (FS-13)": three forms with `in_flow`; a cold mount at `scope=CONE-GLOST` makes exactly the scope `getFlow`, `getNode`, `getTrace`; `aside40`'s `WBAY-KILN` 3 boxes. M: `in_flow` dropped from `flowForm`; titles from one `getNode` per ancestor.
- [x] FS-14 — **Readable boxes.** `geometry.test.ts` (0.92), `styles.test.ts` "readable boxes (FS-14)" (the 14 px title, `minZoom` 0.5, Graph's 0.6 and 0.1); no `SCALE_EXCEPTIONS` names RM-0094. M: Graph's floor; `--font-size-small`; a raw `font-size` in `.flow-box`.
- [x] FS-15 — **Top to bottom.** `layout.test.ts` "rows (AC-04, FS-15)", "edges (AC-05, FS-15)", "keys' moves (FS-15)". M: `x` by distance; a divider per row; a divider in a stage view.
- [x] FS-16 — **Edges.** "the canvas (FS-16)": none of 9 lifted edges unselected, 9 for `KRUN-FIRING`, 1 for `WBAY-KILN`, 0 after Esc; 11 in `KRUN-FIRING`; no canvas count; the List row ends `×5`; `labels.test.ts`. M: off-walk edges unselected; the count back; `typesOf` from the read.
- [x] FS-17 — **Gaps.** "the Gaps control (FS-17)": Legend then Gaps, "1 found"; found, not measured, "7 rows find nothing"; causes folded, `lang="en"`; "Wedging + Open day"; one `createCard`; one overlay at a time; Esc to the toggle; Tab order. Amended: activation replaces when the member is drawn; at the lens root it is folded, so it seeks and the fix pushes. M: the cause unfolded; a row in the panel; an item by ID.
- [x] FS-18 — **Records for the AI.** "Records for the AI (FS-18)", `mocks/flow.smoke.test.tsx`: 141, one uncut read on opening, `edict` 32, `hunch` 59, `puzzle` 50; `WBAY-OFFICE` "2 documents"; "Show all 5679"; focus back; none on Top, at N = 0, or on an uncut read. M: the read on mount; `in_flow: true` listed; ID-less listed; the scope's own node listed.
- [x] FS-19 — **The path pane.** "the path pane (FS-19)": "The pottery", "Kiln bay", "Glost cone", `flowHash` links, `aria-current`; the groups; a module rooted at `WBAY-KILN` (the stub's; the mock's `aside` serves none); failure and Retry; no Up anywhere. M: `aria-current` dropped; the path from held reads; Up restored.
- [x] FS-20 — **Titles.** "titles, technical details folded (FS-20)". M: `flow-places-name` restored; Technical details open.
- [x] FS-21 — **Empty states.** "empty states (FS-21)": `WBAY-OFFICE`'s `EmptyState`; an in-flow scope with all its documents cut shows `flow.emptyScope` with Records secondary. M: a blank canvas.
- [x] FS-22 — **The chevron.** `styles.test.ts` "the path pane (FS-22)", `align-items: flex-start` added. M: `flex-wrap: wrap`. **[look]** 2026-11-10, 1100x800, English and Russian, the mock's `normal` at `module=`: four group rows, each chevron 2 px below the top of its words, on the first line; no page overflow.
- [x] FS-23 — **Catalogs, gates, size, churn.** `screens.language.test.tsx` "Flow's scenario in Russian (FS-23 of flow-scenario)"; `pnpm lint` 0, `tsc`, `build`, `test` (4 748); eval `ui_policy`, `anonymity`, `doc_pointers` (58); first-load JS under 845 kB (below); churn below. M: a `ru.ts` key removed; `"follows"` in `layout.ts`.
- [x] FS-24 — **[look] The owner's words.** Run 2026-11-10 (below): the mock's `aside` at `KRUN-FIRING`, 1920x1080 and 1100x800, light, English; 1440x900 in Russian, light and dark, over the fixture and a scratch copy of the owner's project: readable, the scenario only.

## Out of scope

RM-0095: the document modal and page, link ends' titles, the minimal panel, the breadcrumb on `ancestors`. RM-0099: Graph's look. RM-0096: the owner's words. Not done: a text line for `ancestors`, an `INSTRUCTIONS` change, a kind or path filter in the core, a layout library, keyboard zoom, a record count inside a group's box, a cut that changes `holds`, the Records dialog in the hash.

## Implementation

Rust and UI reviews: ACCEPT with minors, fixed (the UI in iteration 2); every departure accepted.

**Departures.** Rust: (1) `read-spec`'s two lines, reworded to keep the body under 4 096 B (4 093): line 17 "one node's current text and `ancestors`, its path from the top: `get_node` with its `id`; add `with: ["links"]` for its links both ways, `with: ["bindings"]` for the code bound to it and whether it still matches the spec (each binding's `sync`);", line 20 "the ordered flow, its typed edges and gap rows, which stop nothing: `get_flow`; if too large, from a `root`, to a `depth`, or `in_flow: true` to keep only what is in the flow;". (2) eval `scale/mod.rs` gains `in_flow: false`. (3) The help reads "Leave out ...". (4) The exit-1 document under the cut: `not_in_flow` 0, `types` filled. (5) The walk always builds the whole graph; `show --bindings` on `[code]` pays one build more, unmeasured. (6) `FLOW_DESCRIPTION` unchanged, at 2 048 B. UI: (7) the List keeps `×<count>`. (8) `ui.close`; `app.shortcuts.flow.move`, `.across` reworded for rows; `flow.edgeKind.*` words the closed kinds, an unknown one raw. (9) Boxes 160 px wide. (10) Gaps on the List tab too, after the bar in the DOM. (11) No Records button on an uncut read. (12) An untitled group shows its ID. (13) An unknown gap code raw, `lang="en"`; `flow.gapsClear`, `flow.gapsHeading`, `flow.fact.title` gone. (14) Gaps reads the tree once for members no read in hand titles: the contract lacks their titles (RM-0101). (15) The minimap bottom-left.

**Test churn beyond the plan**: CLI `bundle`, `norm_show`, `show_bindings`, `read_trust`, `tested_reads`, `genre`, `kind_amendment`, `common/{mod,keys}`, `src/store_reads_tests`; MCP `common/read`, `plugin_skills`, `plugin_roles`; http `spec_ids_hub`; store `projection`, `store_reads` (21 fixtures); eval `import_support`; UI `api/{http,live,provisional,queries}`, `App`, `mocks/{flow,workspaces.smoke}`, `test/{builders,flowStub}`.

**Numbers.** FS-12 (release, `scale` 10 000, four alternating runs, median p50 against the parent): `get_flow_top` +0.3 %, `get_flow_root` +0.4 %, `get_flow_stage` +1.5 %, `get_node` +7.6 % (min to min +5.2 %), `get_node_links` +6.0 %, `get_context_bundle` +4.2 %. Workspace 5 332 tests, four red on the UI lane's in-flight edits or load, each green alone or once fixed. UI 4 748 tests, 32 mutations red. First-load JS 815.13 kB raw (806.03 before, bar 845); `ru` 141.46 kB.

**Looks**, 2026-11-10, the orchestrator, `vite preview` on a scratch build, DevTools emulation. The mock's `aside`, `KRUN-FIRING`, light, English: at 1920x1080 zoom 1, titles 14 px, 10 boxes inside the canvas, dividers wheelwork, bisque, glazing, crating, the path "The pottery", "Firing run"; at 1100x800 zoom 0.986, titles 13.8 px, header 48 px; no page overflow at either. The chevron (FS-22) above. Over a debug daemon on scratch copies, 1440x900, Russian: the owner's root: 14 boxes, the flow and 13 parts in the band, no ID-less note or record, the path by titles. The part that drew 150 boxes: none, `flow.emptyAside`, Records (150). A flow scope: 10 boxes top to bottom, four dividers, fit 0.92, titles 12.9 px, 15 edges, no counts, no overflow, branches side by side, repeat arcs right; edge kinds in Russian. Dark right after reload and a live switch.

| Module | What it does |
|---|---|
| core `flow.rs`; CLI `flow.rs`, `main.rs` | membership, `in_flow`, the cut, `not_in_flow`, `types`, `--in-flow` |
| CLI `show.rs`, `subset.rs` (`Gather::chain`), `bundle.rs`, `cap.rs`, `lib.rs` | `ancestors` from the subset's `PARENT` rows |
| MCP `read.rs`, `mirror.rs`; http `app.rs`; `plugin/` | `in_flow`, the mirrors; 0.33.0 |
| UI `api/`, `flow/` (`FlowRecords.tsx`), `mocks/` (`harbor-sim/aside.ts`), `styles/`, `i18n/` | forms, the path, Gaps, Records, rows, CSS on the scale |
