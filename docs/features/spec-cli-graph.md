---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-model]
ref: 08 §2 Phase 1, CLI pass 3 of 5; 06 §1; 07 §1.2, §2; owner's answers Q1-Q6, 2026-10-01
shipped: 2026-10-01
---

# Graph reads: spec tree, spec graph, show --links

## Why

Without opening files, an agent cannot get a node's ancestors, children, dependencies or an edit's impact (06 §1). The shapes fixed here feed MCP's `get_tree`, `get_node(with: [links])` (07 §1.2) and pass 4's bundles (05 §6).

No ADR: Q1–Q6 and the iteration rulings decide inside ADR-0001/0003, ADR-0008, ADR-0012, ADR-0016, ADR-0026. How it works now: `docs/canon/spec-cli-graph.md`.

## Acceptance criteria

CLI tests unless named; scratch copies of `fixtures/spec-a`, `-b`, own `HOME`; no git writes. M: the mutation that must turn it red.

- [x] AC-01 `crates/specengine-eval/tests/build_graph.rs` unchanged, green. M: `petgraph` in the CLI's `Cargo.toml`.
- [x] AC-02 `tests/tree.rs`: spec-a `spec tree` → exit 0, exactly the canon's example. M: a section's parent taken as its document; every parentless document a root.
- [x] AC-03 spec-b → MOD-CLI 0, CMD-SYNC 1, FLAG-DRY-RUN 2, CMD-STATUS 1; this repository's docs copied with a slug → `nodes 0, roots 0`, the note; `tests/genre.rs` `FORBIDDEN` gains `DOM-`, `MOD-`, `CMD-`. M: a root rule by kind, prefix or path literal.
- [x] AC-04 `spec tree DOM-MOVEMENT --depth 1` → DOM-MOVEMENT, RULE-MOVE-SPEEDS, MEC-SPRINT, MEC-STAMINA; `MEC-SPRINT#RULE-SPRINT-COST` → it, EDGE-SPRINT-EMPTY; `docs/features/stamina-tuning.md` → the path node, AC-07 at 1; `MEC-NOPE` → exit 1, JSON `reason`; `--depth x`, `-1` → exit 2. M: depth from the corpus root.
- [x] AC-05 `--kind edge-case` (string only in the test) → exactly the unfiltered tree's edge-case lines, order, depths, `parent`; `nodes 2, roots 0`. M: filtering before descending; unlisted roots counted.
- [x] AC-06 MEC-SPRINT's `parent:` → an `aliases:` entry of DOM-MOVEMENT → still under it; → `DOM-MOVEMENT#RULE-MOVE-SPEEDS` → under that section; → `project:`-qualified → an unmarked root; → `DOM-NOWHERE` → a root marked dangling, exit 0, `spec check` `ref-dangling` there. M: `parent_id` joined to `nodes.id` as text; the qualifier dropped.
- [x] AC-07 A two-node `parent:` cycle and a self-parent → each node once, marked, one `warning:` each, exit 0; `spec graph MEC-STAMINA` lists MEC-SPRINT once; children killed after 30 s. M: no visited set.
- [x] AC-08 `spec show MEC-STAMINA --links`: out `derived_from` R-12, A-101, `depends_on` MEC-SPRINT, `uses_term` TERM-exhausted; in `depends_on` from MEC-SPRINT, `mentions` from Q-031. `RULE-STAM-REGEN --links`: in `constrains` from MEC-SPRINT, `canon` from DEC-0023 (path, slug anchor). M: path targets unresolved.
- [x] AC-09 spec-b `REQ-002 --links`: in `derived_from` from MOD-CLI `as` its raw Cyrillic legacy form; `mentions` from `docs/features/dry-run.md` lines 6, 12 (by path) and CRIT-01 (18). `MOD-CLI#CMD-SYNC --links`: in `canon` from ADR-0001 (path, Cyrillic slug), `1 archived` left out; `--archive` adds the superseded decision's (reference form), whose `--links` has in `supersedes` ADR-0001 at its line 4; `ADR-0001 --links --archive`: out `supersedes` to it. M: `dst_id` compared as text; `superseded-by` not an edge.
- [x] AC-10 Both fixtures + scratch: an inline `MEC-STAMINA-based` (resolved), a class-less `status: superseded-by DEC-0404`. The (path, line) of every link `--links` marks dangling, out or in, over the live documents == those of `ref-dangling`, `mention-dangling`, `link-dangling`, `canon-file` in `spec check --json` (`parent:` aside). M: no name-shape fallback; outgoing only.
- [x] AC-11 A scratch generated file and a Tier 3 file linking to MEC-STAMINA: not incoming, `left_out {generated: 1, tier3: 1}` (links; `tree`: nodes met), the suffix; `--archive` adds the Tier 3 one; `show REQ-002 --archive` → exit 2. M: no live filter.
- [x] AC-12 Q-032 mentioned in MEC-STAMINA's body → `spec graph MEC-STAMINA` omits it, `--type mentions` lists it at 1. M: `mentions` followed by default.
- [x] AC-13 `spec graph R-12 --impact` → MEC-STAMINA 1, MEC-SPRINT 2, RULE-STAM-REGEN 3; `MEC-SPRINT --impact` → MEC-STAMINA, RULE-STAM-REGEN at 1. M: `derived_from` flipped.
- [x] AC-14 Dangling parents and links, cycles → 0; only an unresolvable REF/ROOT → 1; a look-alike ID → 2 naming the Latin fix; `project:` → 2; no JSON for 2. M: exit 1 on a dangling link.
- [x] AC-15 `tests/bounds.rs`: a tree over 40 000 chars cut at a node line, the tail, the same nodes in text and JSON, `truncated: true`; `show --links` on a long node keeps its links block; a cut inside it → JSON `links` the printed ones, `omitted` the tail's `<k>`. M: links after the text; `omitted` 0 after a cut.
- [x] AC-16 `tests/determinism.rs`: copies written in opposite orders, at different roots → byte-identical `tree`, `graph`, `graph --impact`, `show --links`, text and JSON. M: `HashMap` or rowid order.
- [x] AC-17 `tests/read_only.rs`, `freshness.rs`: copies byte- and path-identical, new files only under `HOME`; a new child document shows without `spec index`. M: `update` skipped; a write under the root.
- [x] AC-18 `INDEX_FORMAT` 6; store `tests/format_history.txt`, the spec-a + spec-b dump unchanged. M: a schema column.
- [x] AC-19 `tests/show.rs`: node keys = pass 1's + `links` (`null` without the flag); `search` keys unchanged. M: `links` omitted when not requested.
- [x] AC-20 The gate clean; every Tier 1 README ≤ 10 240 B; at shipping worst W ≤ 114 506 B + 0 (the spec's line to Tier 3; the canon offset by 05 §8, §3.2, 08). M: the commands' canon appended to the CLI README.
- [x] AC-21 DEC-0023's `canon:` minus `#regeneration` → `spec graph DEC-0023`: MEC-STAMINA at 1, the edge's `reason` set; `spec check`: `canon-form`. M: an anchor-less path `unchecked`.

## Implementation

Canon: `docs/canon/spec-cli-graph.md` (input, live rule, parents and links, the type table, the three commands, exit and cap, API, "Open"); pointers in the CLI, core and model READMEs. Three iterations. 1: the commands, `SpecGraph`, the tables. 2: every used file re-parsed whatever its size (the size heuristic and its warning gone: 71 of 200 same-size races mixed states); `superseded-by` listed on both ends, parity by written place; with `--links` the JSON follows the text's cut and gains `omitted`; tree `roots` counted after `--kind`. 3: doc comments; the anchor-less `canon:` reason split into "lands on the document" (resolved) and "not checked" (no document). Reviewer accepted iteration 2; 1019 of 1019 tests, clippy and fmt clean.

| Module | What it does |
|---|---|
| model `link.rs`, `lib.rs` | `Direction`, `IMPACT_LINK_TYPES`, `graph_direction`, `impact_direction`, `is_weak_link` |
| core `check/spec_graph.rs` (new), `mod.rs` | `SpecGraph` and `NodeAt`, `Standing`, `Parent`, `Endpoint`, `LinkState`, `Edge`, `NodeLinks`, `Walk` |
| core `check/resolve.rs`, `links.rs` | the check's resolution reports the winning holder; `resolve_link_path` shared; behaviour unchanged |
| CLI `corpus.rs` (new) | update, `indexed_input`, re-read and re-parse; `LeftOut`, the live rule, REF location, holders warning |
| CLI `tree.rs`, `graph.rs`, `links.rs` (new) | `tree`, `graph`, the `--links` block and their renderings |
| CLI `show.rs`, `cap.rs`, `lib.rs`, `main.rs` | `ShowRequest {links, archive}`, `ShownNode.links`; the block inside the cap, JSON `links`, `omitted`; `Outcome::{Tree, Graph}`; the subcommands |

Tests: CLI `tree.rs`, `graph.rs`, `links.rs`, `common/graph.rs` (new), `bounds.rs`, `determinism.rs`, `freshness.rs`, `genre.rs`, `read_only.rs`, `show.rs`, `common/mod.rs`; core `spec_graph.rs` (new).

Deviations from the draft, now canon: `graph` lists unchecked edges; default `types` include the corpus's unknown declared types; a second note when documents exist but none is a root; warnings only for reached nodes; the REF/ROOT holders' files always admitted; `--archive` without `--links` refused by the library (exit 2, one line), not clap; `omitted` counts dropped holders' links; the cut node stays in JSON even when its header was not printed.
