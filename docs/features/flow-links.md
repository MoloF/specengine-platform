---
class: spec
status: shipped
shipped: 2026-10-31
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli]
ref: RM-0042; Phase 7 step 1 (08 s2); owner's decisions 2026-10-30, pre-approved, review open
adrs: [ADR-0055]
---

# flow-links: `follows`, `uses`, `supplies` and labelled edges (RM-0042)

## Why

A game's flow is made of loops: a failed run restarts, rounds repeat. `depends_on` means "cannot be understood or built without", so a loop in it is a `depends-cycle` finding. A project cannot add its own type (ADR-0031), so the shared table must carry order, reuse and supply (ADR-0055).

## Summary

Three shared link types and a labelled `follows` item, placed in impact and the bundle, with `kind` and `when` in `norm_hash` under `specengine-norm/v1`. No new command, tool, route, table or column. How it works now, with the item rules, the rendering and why it stays v1: `docs/canon/flow-links.md`.

### Outputs

`spec graph` and `spec show --links` lines gain ` | kind <kind>` and ` | when <when>`; `GraphEdge` and `ShownLink` gain `kind` and `when`, present only on a labelled edge. `types` holds 15 `LINK_TYPES`, 7 under `--impact`.

### Index

`INDEX_FORMAT` 13, no column: a stored row holds `unknown-link-type` for a plain `follows`, and an unchanged file is re-parsed only on a new stamp.

## Acceptance criteria

- [x] AC-01 — Loops are no finding. `spec check --root` each fixture: exit 0, `clean`, no finding. `spec graph LEG-RUN --impact --type follows --root fixtures/flow-relay`: exit 0, last line `nodes 5, edges 6`, identical bytes from copies written in opposite file orders. M1: `depends_cycle` also reading `follows` gives `depends-cycle`.
- [x] AC-02 — Bad items. On a copy of flow-relay, each item rule applied alone to `follows[1]` of `leg-run.md`: exit 1, `blocked`, one `frontmatter-type` with that `<problem>` at the item's line, `norm_hash` null; `uses: [{to: KIT-TORCH}]` gives today's message. M2: a mapping accepted under every type turns the `uses` case red. M3: no closed set of kinds turns rule 4 red.
- [x] AC-03 — Labels in the hash. `spec show LEG-RUN --json` gives the golden `norm_hash` of its three rendered links. A changed `when` or `kind` changes it; reordered item keys, block style, a quoted `to`, or `{to: LEG-DRAW}` for `LEG-DRAW` do not. Every earlier golden of `norm_recipe.rs` and `fixtures/spec-lock` passes. M4: `rendered` ignoring `edge` turns the change cases red. M5: `specengine-norm/v2` turns the goldens red.
- [x] AC-04 — Impact. On flow-relay `spec graph KIT-OIL --impact`: `nodes 4, edges 3` (KIT-OIL, KIT-TORCH, LEG-FALL, LEG-RUN); `spec impact KIT-OIL` lists those four. `spec graph LEG-RUN --impact`: `nodes 1, edges 0`. M6: `("follows", Direction::In)` turns the second red. M7: `uses` as `Direction::Out` turns the first red.
- [x] AC-05 — Bundle. `spec bundle LEG-RUN --json`: `neighbours` exactly KIT-TORCH `via uses out`, LEG-DRAW `via follows out`, LEG-FALL `via follows in, follows out`, LEG-SCORE `via follows in`, in that order. KIT-TORCH: exactly KIT-OIL `via supplies in`. KIT-OIL: exactly KIT-TORCH `via supplies out`. M8: no `("follows", In)` loses LEG-SCORE. M9: `("uses", In)` adds two nodes to KIT-TORCH's.
- [x] AC-06 — Labels on reads. `spec graph LEG-RUN --impact --type follows` prints `LEG-RUN --follows--> LEG-FALL | docs/spec/leg-run.md:<line> | kind repeat | when the torch is relit`; `spec show LEG-RUN --links` prints `in follows LEG-SCORE | docs/spec/leg-score.md:<line> | when the runner arrives`; `--json` has those keys, LEG-DRAW's plain edge neither. On `spec-a`, `spec-b`, `spec-c` no edge or link has either key. M10: `kind: null` on every edge turns `daemon_keys.rs` red.
- [x] AC-07 — Genre. The same binary passes AC-01 on both fixtures from `[ids]` alone; `check_genre.rs`, `no_domain.rs` and the source scans of `norm_recipe.rs` and `health.rs` pass with the new words. M11: a literal `"repeat"` in `norm.rs` turns `norm_recipe.rs` red. M12: a literal `"leg"` in http `src` turns `no_domain.rs` red.
- [x] AC-08 — Index stamp. `INDEX_FORMAT` is 13, `format_history.txt` ends with line 13, `SERIALISED` names `edge`, `edge.kind`, `edge.when` under `links[].dst`, and `flow-relay` in `DUMPED` sets them. An index of stamp 12 holding `unknown-link-type` for a plain `follows` loses that row on the next read, the file untouched. M13: stamp 12 turns `format_history_ends_with_the_current_format_and_dump_hash` red.

## Verification

Mutations: 8 run and killed, none survived.

| Mutation | Criterion | Tests red |
|---|---|---|
| M1 | AC-01 | 6 |
| M2 | AC-02 | 2 |
| M4 | AC-03 | 6 |
| M6 | AC-04 | 4, and a compile-time catch by the table pin in core `spec_graph.rs` |
| M9 | AC-05 | 5 |
| M10 | AC-06 | 3 |
| M12 | AC-07 | 2 |
| M13 | AC-08 | 4 |

M3, M5, M7, M8 and M11 were skipped under the cap of 8; every criterion is still covered. Every hand-worked value held: `nodes 5, edges 6`, `nodes 4, edges 3`, `nodes 1, edges 0`, the neighbours, 15 and 7 types. `format_history.txt` line 13 is recorded.

Gates, final run: fmt 0, clippy 0, probes clippy 0; `cargo nextest run --workspace` 4039 run, 4039 passed, 71 skipped; UI lint 0, vitest 2546 passed, build 0; `spec check` 354 documents, 0 errors, 0 warnings; parity after docs 3 of 3; `doc_pointers`, `ui_policy`, `anonymity`, `build_graph` 86 of 86. No `Cargo.toml` or `Cargo.lock` diff.

## Implementation

| Area | What it does |
|---|---|
| Types and impact | `LINK_TYPES` 15, the last three appended as `follows`, `uses`, `supplies`; `FOLLOWS` is a const, and the array spells the word so the daemon's scan reads it. `IMPACT_LINK_TYPES` 7, `BUNDLE_LINK_TYPES` 12 |
| Label | `EdgeKind` and `EdgeLabel` sit in `reference.rs`; `Reference.edge` is skipped when `None`; `{to: X}` gives no label. `yaml.rs` has a fifth `Located` level, so `to` keeps its line and span |
| Reading | `front_matter.rs` walks `links` in written order. `links_fault` gives the first fault: `Shape` is today's message; `Item` is `follows[N]: <problem>` at the item's line, N 0-based; either way the map goes to `extra`, one diagnostic. `edge_item` applies rules 1-8; `when` is judged trimmed, at most 256 bytes |
| Hash | `rendered` appends ` \| <kind or -> [ \| <when>]` to a reference; the word comes from `EdgeKind::as_str`, the recipe stays v1 |
| Reads | `Edge.edge`, then `GraphEdge` and `ShownLink` gain `kind` and `when` as strings, skipped in JSON when absent |
| Index | `INDEX_FORMAT` 13, no column: the label lives in the stored JSON |

**Files.** Developer: model `src/link.rs`, `src/reference.rs`, `src/grammar.rs`, `src/lib.rs`; core `src/yaml.rs`, `src/front_matter.rs`, `src/norm.rs`, `src/check/bundle.rs`, `src/check/spec_graph.rs`, `src/check/resolve.rs`, `src/lib.rs`; store `src/lib.rs`; CLI `src/graph.rs`, `src/links.rs`, and, outside the first file list, `src/main.rs` (its `--impact` help line named the table and would be false). Tester: the two fixtures; core and CLI `tests/flow_links.rs`; core `tests/norm_recipe.rs`, `tests/spec_graph.rs`, `tests/bundle_layers.rs`; store `tests/format.rs`, `tests/format_history.txt`; CLI `tests/graph.rs`, `tests/bundle_config.rs`; http `tests/no_domain.rs`, `tests/graph.rs`; MCP `tests/mcp_read.rs`.

**Divergences from the draft.** `follows: [{[a]: b}]` gives a second `frontmatter-type`; `to` naming no reference is `unparsed-reference`, a warning; MCP `ShowLink` leaves the two keys undeclared, a named exception to exact-keys conformance. Each: `docs/canon/flow-links.md`. The draft's "one document per node" did not stand: each fixture also holds one unlinked ID section, `LEG-START-LINE` in `leg-start.md` and `JRN-SCAN-BAD` in `jrn-scan.md`, because a store test oracle needs a nested section in every fixture corpus.
