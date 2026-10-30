---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp]
owner: owner
reviewed: 2026-10-31
---

# Flow links: `follows`, `uses`, `supplies` and labelled edges

Three shared, domain-free link types carry order, reuse and supply, and a `follows` item may carry a label (ADR-0055, `architecture.md#universal`). They are written in front-matter as any `links` item, with no command, tool, route, table or column of their own. `spec check` gives flow no rule, so nothing here blocks work (ADR-0012).

## Types

| Type | Declared on | Means | `spec graph` | `--impact` | Bundle layer 7 | `--links` |
|---|---|---|---|---|---|---|
| `follows` | the later step | it comes after the target | out | not followed | out and in | strong |
| `uses` | the using node | it reuses the target by reference | out | in | out | strong |
| `supplies` | the producer | it produces or feeds the target | out | out | out and in | strong |

- **The tables.** `LINK_TYPES` 15, these three last; `FOLLOWS` is a const, and the array spells the word so the daemon's scan (`no_domain.rs`) reads it. `IMPACT_LINK_TYPES` 7: `depends_on`, `derived_from`, `verifies`, `uses_term`, `uses` in; `constrains`, `supplies` out. `BUNDLE_LINK_TYPES` 12; layer 7's rows: `depends_on` out, `constrains` out and in, `derived_from` out, `follows` out and in, `uses` out, `supplies` out and in.
- **Flow runs against the link.** `B follows A` is the link B -> A, so `spec graph REF --impact --type follows` walks the flow forward (`impact_direction(..).unwrap_or(Direction::In)`).
- **`follows` stays out of impact.** One edit would reach every later step and, over a `repeat`, the whole loop.
- **Incoming `uses` stays out of the bundle**, as incoming `depends_on` does: impact, not context.
- **Cycles** in the three are no finding: `depends-cycle` and the plan read `depends_on` alone, and a walk visits a node once.
- **`supplies` reads as written**: a pack adding a consumer edits the producer or declares `uses`.

## Items

```yaml
links:
  follows:
    - LEG-DRAW
    - {to: LEG-FALL, kind: repeat, when: "the torch is relit"}
  uses: [KIT-TORCH]
```

A `follows` item is a string (one reference, as any `links` item) or a mapping of `to` (required), `kind`, `when`; every other type stays strings. `kind` is `alternative`, `parallel` or `repeat`. `when` is prose in the project's language, never evaluated, stored trimmed. `{to: X}` alone gives no label and equals the plain `X`. Links are a document's only, so a step is a document; `to` may name `ID#SECTION`. A step may follow itself; `follows` beside `depends_on` on one pair is no finding; two items on one target are two edges.

Item rules, the first failing giving `<problem>`:

1. `` unknown key `by`: an item takes `to`, `when`, `kind` ``
2. `` no `to` ``
3. `` `to` is not a string ``
4. `` `kind` `loop` is none of `alternative`, `parallel`, `repeat` ``
5. `` `when` is not a string ``
6. `` `when` is blank ``
7. `` `when` is over 256 bytes ``, judged trimmed
8. `` `when` holds a control character ``

```text
error  docs/spec/leg-run.md:10: frontmatter-type: front-matter `links`: `follows[1]`: <problem>; kept in `extra`
```

## Reading

`yaml.rs` `Root` has a fifth `Located` level, so `to` keeps its line and span. `front_matter.rs` walks `links` in written order. `links_fault` gives the first fault: `Shape` is the message every mistyped `links` gets (a mapping item under any type but `follows` too), `Item` is ``follows[N]: <problem>`` at the item's line, N 0-based. Either way the whole map goes to `extra`, with one diagnostic, and the document's `norm_hash` is null. `edge_item` applies rules 1 to 8.

A dangling `to` is `ref-dangling` at the value's line; `ref-superseded` and `project:` behave as for any `links` item.

Model (`reference.rs`): `EdgeKind {Alternative, Parallel, Repeat}` (`ALL`, `as_str`, `parse`, serde lowercase), `EdgeLabel {kind?, when?}`, `Reference.edge`, skipped when `None`. The label sits on `Reference`, not `Link`: `SpecGraph` builds edges from `Fields.links` through `declared_references`, so a label on `Link` would reach neither `spec graph` nor the bundle. `Fields.links` keeps its type. `Edge.edge` carries it into the graph.

## Hash

`kind` and `when` count in `norm_hash`, under `specengine-norm/v1`: a changed branch condition is a changed meaning. `rendered` appends to a labelled item's reference:

```text
<rendering of the target> | <kind, or -> [ | <when>]
follows ref:LEG-FALL | repeat | the torch is relit
follows ref:LEG-RUN | - | the runner arrives
follows ref:JRN-UNLOCK | parallel
```

The literals ` | ` and `-` hold no letter, so `norm_recipe.rs`' source scan needs no exception; the word comes from `EdgeKind::as_str`. `when` runs to the frame's end, so no two labels read alike. Key order, flow or block style and YAML quoting are not counted: the rendering reads parsed values.

**This additive case stays v1.** A labelled item was `frontmatter-type`, its document's hash null, and `spec lock accept` refuses a null hash, so no hash and no lock entry changes. A plain item of the three already rendered as an unknown type does. A v2 would move every `spec.lock` hash and need ADR-0050's rebase, for no document holding a label.

## Outputs

`spec graph` and `spec show --links` lines end, before the state suffix, ` | kind <kind>` and ` | when <when>`, each when set (`docs/canon/spec-cli-graph.md`). JSON: `GraphEdge` and `ShownLink` gain `kind` and `when`, strings, skipped when absent. That is the one exception to "every key present": a corpus without labels keeps its documents, so `fixtures/daemon-keys.json` and the UI's types do not move. `ShownLink`'s JSON is `spec health`'s too. MCP `get_node` with `links`, `get_impact`, `get_context_bundle` and the daemon's `graph` carry the same.

## Index

`INDEX_FORMAT` 13, no column: the label lives in the stored JSON of `links` and `fields.links`. The stamp moved because a stored row held `unknown-link-type` for a plain `follows`, and an unchanged file is re-parsed only on a new stamp.

## MCP mirror

`kind` and `when` are undeclared in `mirror.rs` `ShowLink`, because `require_all` cannot express an optional key, so the mirror's "every key `required`" (`mcp-read.md` "Tools") does not cover these two. The document meets the published schema, which has no `additionalProperties`, but it is a named exception to the project's exact-keys conformance. `mcp_read.rs` `flow_labels_reach_get_node_within_its_schema` pins that the document minus exactly those two keys passes strictly, the full document fails strictly, and plain links stay strict.

## Known limits

- One exception to one diagnostic per map: `follows: [{[a]: b}]` gives a second `frontmatter-type`, rule 1's and the one `extra` gives a collection used as a key.
- A `to` naming no reference is `unparsed-reference`, a warning: the item is dropped, the rest of the map kept, and `norm_hash` null.
- `ui/src/policy.test.ts` lists link types by hand and does not hold these three yet (RM-0044).

## Tests

Core `tests/flow_links.rs`, `tests/norm_recipe.rs`, `tests/spec_graph.rs`, `tests/bundle_layers.rs`; CLI `tests/flow_links.rs`, `tests/graph.rs`; store `tests/format.rs`; over `fixtures/flow-relay` and `fixtures/flow-hire`. Each fixture holds one unlinked ID section (`LEG-START-LINE`, `JRN-SCAN-BAD`): a store test oracle needs a nested section in every fixture corpus.
