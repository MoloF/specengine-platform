---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli]
ref: RM-0042; Phase 7 step 1 (08 s2); owner's decisions 2026-10-30, pre-approved, review open
adrs: [ADR-0055]
---

# flow-links: `follows`, `uses`, `supplies` and labelled edges (RM-0042)

## Why

A game's flow is made of loops: a failed run restarts, rounds repeat. `depends_on` means "cannot be understood or built without", so a loop in it is a `depends-cycle` finding. A project cannot add its own type (ADR-0031), so the shared table must carry order, reuse and supply (ADR-0055).

## Description and interactions

An agent declares these links in front-matter, through the queue as today. No new command, tool, route, table or column.

| Type | Declared on | Means | `spec graph` | `--impact` | Bundle layer 7 | `--links` |
|---|---|---|---|---|---|---|
| `follows` | the later step | it comes after the target | out | not followed | out and in | strong |
| `uses` | the using node | it reuses the target by reference | out | in | out | strong |
| `supplies` | the producer | it produces or feeds the target | out | out | out and in | strong |

- Flow runs against the link: `B follows A` is B -> A, so `spec graph REF --impact --type follows` walks forward (the existing `unwrap_or(Direction::In)`).
- `follows` stays out of impact: one edit would reach every later step and, over a `repeat`, the whole loop.
- Incoming `uses` stays out of the bundle, as incoming `depends_on` does.
- Cycles are no finding: `depends-cycle` and the plan read `depends_on` alone; walks visit a node once.

### Outputs

Only these change. A `spec graph` edge, a `spec show --links` line, a bad item:

```text
<src> --<type>--> <dst> | <path>:<line>[ | kind <kind>][ | when <when>][ | dangling: <reason> | skipped: another project | unchecked]
  <out|in> <type> <name, else written> | <path>:<line>[ | at <section>][ | as <written>][ | kind <kind>][ | when <when>][ | dangling: <reason> | skipped: another project | unchecked]
error  docs/spec/leg-run.md:9: frontmatter-type: front-matter `links`: `follows[1]`: <problem>; kept in `extra`
```

JSON: `GraphEdge` and `ShownLink` gain `kind` and `when`, strings, **present only on a labelled edge**, so `fixtures/daemon-keys.json` and the UI's types stay. `types` holds 15 `LINK_TYPES`, 7 under `--impact`. MCP and the daemon carry the same.

Exits: `spec check` 0 `clean`, flow never judged; 1 `blocked` on `frontmatter-type` or `ref-dangling`; 2 as today. The reads as today: 0 answered, cycles included; 1 an unresolvable REF; 2 usage or config.

### Files in scope

- **Developer:** model `src/link.rs` (`LINK_TYPES` 15, appended; `IMPACT_LINK_TYPES` 7, `uses` in, `supplies` out; `FOLLOWS`), `src/reference.rs`, `src/grammar.rs`, `src/lib.rs`; core `src/yaml.rs` (a fifth `Located` level in `Root`, so `to` keeps its line and span), `src/front_matter.rs`, `src/norm.rs`, `src/check/bundle.rs` (`BUNDLE_LINK_TYPES` 12), `src/check/spec_graph.rs` (`Edge.edge`), `src/lib.rs` and `src/check/resolve.rs` (`edge: None` on literals); store `src/lib.rs`; CLI `src/graph.rs`, `src/links.rs`.
- **Tester:** the two fixtures; new core and CLI `tests/flow_links.rs`; core `tests/norm_recipe.rs`, `tests/spec_graph.rs`, `tests/bundle_layers.rs`; store `tests/format.rs`, `tests/format_history.txt`; CLI `tests/bundle_config.rs`. Green unedited: core `tests/crafted_yaml.rs`, `tests/check_graph.rs`, `tests/check_genre.rs`; http `tests/no_domain.rs`, `tests/daemon_keys.rs`.
- **Untouched:** `src` of http, MCP and import, and `ui/`.
- **Canon at shipping:** model and store READMEs; `node-revision.md`, `spec-cli-graph.md` and `spec-cli-bundle.md` stand within 36 B of their cap, so the detail goes to a new `docs/canon/flow-links.md`.

## Data

```yaml
id: LEG-RUN
links:
  follows:
    - LEG-DRAW
    - {to: LEG-FALL, kind: repeat, when: "the torch is relit"}
  uses: [KIT-TORCH]
```

A `follows` item is a string (one reference, as any `links` item) or a mapping of `to` (required), `kind`, `when`. Other types stay strings. `kind` is `alternative`, `parallel` or `repeat`. `when` is prose, never evaluated, stored trimmed. `{to: X}` alone equals the plain `X`. A `to` naming no reference is `unparsed-reference`. Item rules, the first failing giving `<problem>` verbatim:

1. `` unknown key `by`: an item takes `to`, `when`, `kind` ``
2. `` no `to` ``
3. `` `to` is not a string ``
4. `` `kind` `loop` is none of `alternative`, `parallel`, `repeat` ``
5. `` `when` is not a string ``
6. `` `when` is blank ``
7. `` `when` is over 256 bytes ``
8. `` `when` holds a control character ``

```rust
pub enum EdgeKind { Alternative, Parallel, Repeat }   // serde lowercase; ALL, as_str, parse
pub struct EdgeLabel { pub kind: Option<EdgeKind>, pub when: Option<String> }
// Reference gains, skipped when None: pub edge: Option<EdgeLabel>
```

The label sits on `Reference`, not `Link`: `SpecGraph` builds edges from `Fields.links` through `declared_references`, so a label on `Link` would reach neither `spec graph` nor the bundle. `Fields.links` keeps its type.

**Index.** `INDEX_FORMAT` 13, no column: a stored row holds `unknown-link-type` for a plain `follows`, and an unchanged file is re-parsed only on a new stamp.

### `norm_hash`: settled, the labels count and the recipe stays v1

**`kind` and `when` count, under `specengine-norm/v1`.** A changed branch condition is a changed meaning: it must raise a revision and later mark cards suspect. A labelled item renders after its target; no earlier rendering changes:

```text
<rendering of the target>[ | <kind, or -> [ | <when>]]
follows ref:LEG-FALL | repeat | the torch is relit
follows ref:LEG-SCORE | - | the runner arrives
follows ref:JRN-UNLOCK | parallel
```

The literals ` | ` and `-` hold no letter, so `offending_literals` stays; the word comes from `EdgeKind::as_str`. `when` runs to the frame's end, so fields cannot be confused.

- **Taken, additive under v1:** a labelled item is `frontmatter-type` today, its document's hash null, and `spec lock accept` refuses a null hash, so no hash or lock entry changes. A plain item already renders as an unknown type does.
- **Refused, not counted:** a rewritten condition passes unseen.
- **Refused, v2:** every `spec.lock` hash would move and need ADR-0050's rebase, for no document holding a label today.

At shipping `node-revision.md` gains that a rendering for an input that was null changes no hash, so it is no v2.

### Fixtures

One document per node at `docs/spec/<id, lowercase>.md`, header as `fixtures/spec-c/docs/spec/hire.md`; `[ids]` `shape = "name"`. No kind here occurs in a literal the word scans read; a common word such as `step` may.

`fixtures/flow-relay`, an invented game, `LEG` (`leg`), `KIT` (`kit`). Retry loop RUN, FALL, RUN; round loop DRAW, RUN, SCORE, DRAW:

```text
LEG-START, KIT-TORCH  no links
LEG-DRAW   follows LEG-START, {to: LEG-SCORE, kind: repeat, when: "rounds remain"}
LEG-RUN    follows LEG-DRAW, {to: LEG-FALL, kind: repeat, when: "the torch is relit"}; uses KIT-TORCH
LEG-FALL   follows {to: LEG-RUN, kind: alternative, when: "the torch goes out"}; uses KIT-TORCH
LEG-SCORE  follows {to: LEG-RUN, when: "the runner arrives"}
LEG-END    follows {to: LEG-SCORE, kind: alternative, when: "no round remains"}
KIT-OIL    supplies KIT-TORCH
```

`fixtures/flow-hire`, beside `fixtures/spec-c`, no game word, `JRN` (`journey`), `FEE` (`tariff`):

```text
JRN-SCAN, FEE-RIDE  no links
JRN-UNLOCK      uses JRN-SCAN
JRN-RIDE        follows {to: JRN-UNLOCK, kind: alternative, when: "released"}
JRN-NO-RELEASE  follows {to: JRN-UNLOCK, kind: alternative, when: "not released"}
JRN-METER       follows {to: JRN-UNLOCK, kind: parallel}
JRN-RETURN      follows JRN-RIDE, {to: JRN-NEAREST, kind: repeat, when: "a slot is near"}; uses JRN-SCAN; supplies FEE-RIDE
JRN-NEAREST     follows {to: JRN-RETURN, kind: alternative, when: "dock full"}
```

## Rules and edge cases

- WHEN a mapping item stands under any type but `follows`, the system SHALL report today's `frontmatter-type` message.
- WHEN one item is malformed, the whole `links` map goes to `extra` and `norm_hash` is null, as for any mistyped `links`.
- WHEN `to` names nothing, the system SHALL report `ref-dangling` at the `to` value's line.
- A step may follow itself; `follows` beside `depends_on` on one pair is no finding; two items on one target are two edges.
- Links are a document's only, so a step is a document; `to` may name `ID#SECTION`.

## Acceptance criteria

- [ ] AC-01 — Loops are no finding. `spec check --root` each fixture: exit 0, `clean`, no finding. `spec graph LEG-RUN --impact --type follows --root fixtures/flow-relay`: exit 0, last line `nodes 5, edges 6`, identical bytes from copies written in opposite file orders. M1: `depends_cycle` also reading `follows` gives `depends-cycle`.
- [ ] AC-02 — Bad items. On a copy of flow-relay, each item rule applied alone to `follows[1]` of `leg-run.md`: exit 1, `blocked`, one `frontmatter-type` with that `<problem>` at the item's line, `norm_hash` null; `uses: [{to: KIT-TORCH}]` gives today's message. M2: a mapping accepted under every type turns the `uses` case red. M3: no closed set of kinds turns rule 4 red.
- [ ] AC-03 — Labels in the hash. `spec show LEG-RUN --json` gives the golden `norm_hash` of its three rendered links. A changed `when` or `kind` changes it; reordered item keys, block style, a quoted `to`, or `{to: LEG-DRAW}` for `LEG-DRAW` do not. Every earlier golden of `norm_recipe.rs` and `fixtures/spec-lock` passes. M4: `rendered` ignoring `edge` turns the change cases red. M5: `specengine-norm/v2` turns the goldens red.
- [ ] AC-04 — Impact. On flow-relay `spec graph KIT-OIL --impact`: `nodes 4, edges 3` (KIT-OIL, KIT-TORCH, LEG-FALL, LEG-RUN); `spec impact KIT-OIL` lists those four. `spec graph LEG-RUN --impact`: `nodes 1, edges 0`. M6: `("follows", Direction::In)` turns the second red. M7: `uses` as `Direction::Out` turns the first red.
- [ ] AC-05 — Bundle. `spec bundle LEG-RUN --json`: `neighbours` exactly KIT-TORCH `via uses out`, LEG-DRAW `via follows out`, LEG-FALL `via follows in, follows out`, LEG-SCORE `via follows in`, in that order. KIT-TORCH: exactly KIT-OIL `via supplies in`. KIT-OIL: exactly KIT-TORCH `via supplies out`. M8: no `("follows", In)` loses LEG-SCORE. M9: `("uses", In)` adds two nodes to KIT-TORCH's.
- [ ] AC-06 — Labels on reads. `spec graph LEG-RUN --impact --type follows` prints `LEG-RUN --follows--> LEG-FALL | docs/spec/leg-run.md:<line> | kind repeat | when the torch is relit`; `spec show LEG-RUN --links` prints `in follows LEG-SCORE | docs/spec/leg-score.md:<line> | when the runner arrives`; `--json` has those keys, LEG-DRAW's plain edge neither. On `spec-a`, `spec-b`, `spec-c` no edge or link has either key. M10: `kind: null` on every edge turns `daemon_keys.rs` red.
- [ ] AC-07 — Genre. The same binary passes AC-01 on both fixtures from `[ids]` alone; `check_genre.rs`, `no_domain.rs` and the source scans of `norm_recipe.rs` and `health.rs` pass with the new words. M11: a literal `"repeat"` in `norm.rs` turns `norm_recipe.rs` red. M12: a literal `"leg"` in http `src` turns `no_domain.rs` red.
- [ ] AC-08 — Index stamp. `INDEX_FORMAT` is 13, `format_history.txt` ends with line 13, `SERIALISED` names `edge`, `edge.kind`, `edge.when` under `links[].dst`, and `flow-relay` in `DUMPED` sets them. An index of stamp 12 holding `unknown-link-type` for a plain `follows` loses that row on the next read, the file untouched. M13: stamp 12 turns `format_history_ends_with_the_current_format_and_dump_hash` red.

## Out of scope

`spec flow`, gap rows, lifted edges, marking a cycle meant to run forever (RM-0043); the canvas and `ui/src/policy.test.ts`'s three names (RM-0044); `stage:`, `pack:`, overrides (RM-0058); labels on `uses` or `supplies`; a project's own edge kinds; evaluating `when`.

## Implementation

Left for the developer.
