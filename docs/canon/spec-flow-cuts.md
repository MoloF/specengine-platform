---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http]
owner: owner
reviewed: 2026-11-10
---

# `spec flow`: in the flow, the stage and in-flow cuts, `types`

What a flow read (`spec-flow.md`) marks as part of the flow, and the two cuts that shrink what it shows; `flow-scope`, `flow-scenario`. Domain-free (ADR-0008); served, so no screen computes it (ADR-0033); a cut hides no finding and stops nothing (ADR-0012).

## In the flow

- **The rule.** A document is in the flow when `spec flow` with no ROOT, admitting live documents alone, holds it: it holds or receives a resolved flow link (crossing ones too), holds a flow link whose other end is left out or a `follows` that does not resolve, is selected by a `[flow]` table, or is a live `parent:` holder of any of those. Links, `[flow]` and `parent:` alone: no kind, path, prefix or ID shape, so a record's kind can be in the flow and an ID-less document too, and `depends_on` alone puts nothing in.
- **`in_flow`** on every node of every read, folded as `start` is: a node is in the flow when it, or anything it holds, is. A read with no ROOT serves only `true`.
- **A ROOT** is in the flow only through what folds into it. A Tier 3 ROOT's own document is never a member: unbounded, its node is `false` and its live steps `true`; at depth 0 its node holds them and is `true` by the fold.
- **Cost.** Computed once per read. With no ROOT it is the scope itself. When every ROOT holder is live it reuses the read's links, strays, `[flow]` selection and parent map: one linear pass, no second `flow_read`. A Tier 3 ROOT re-reads the links with live-only admission.

## The cuts

Both run on the shown nodes after the fold, `--stage` first, and move nothing else: the scope, the walk, the ten gap rows, `left_out`, `notes`, `root`, `reason`, `depth` and `FlowRead.shown` are the read's without them, so the `--trace` joins count a card on a dropped document on the node holding it (`counts` and `gaps` are computed before any cut). Each kept node and edge is byte-equal to its uncut object, `group` naming the holder whether kept or not; then the edges whose two ends are kept. The header's counts, the cap, `truncated` and `omitted_*` count what is kept. The read's work does not shrink, only its payload.

- **`--stage S`** (`-` for none) keeps a shown node whose own `stage` is S, byte for byte as `spec trace --stage` compares; `-` a shown flow step with no `stage:`, never a group or a definition. Nothing is echoed. A stage nothing carries: 0 nodes, exit 0. A literal `stage: "-"` cannot be asked (`spec-trace.md` "Known limits").
- **`--in-flow`** (MCP `in_flow: true`, the daemon's `in_flow=true`; another daemon value is 400 `` `in_flow=<v>`: not `true` or `false` ``) drops each shown node with `in_flow` false, but a ROOT holder's node, which stays so the scope keeps its anchor and `counts`; a ROOT node the stage dropped stays dropped. A cut leaving only the ROOT's node: that node, `in_flow: false`, exit 0. Opt-in: CLI and MCP reads stay whole without it; the Flow screen's drawing reads always cut.

## Count and types

- **`not_in_flow`**: under `--in-flow`, the documents the dropped nodes stand for, 1 + `holds` each; `null` without it. With `--stage`, what the in-flow cut drops from what the stage kept. The exit-1 document (a ROOT naming nothing) under the cut carries 0. Text: the header gains `; not in the flow: <d>` after `left out`; a read without the flag prints byte for byte as before.
- **`holds`** still counts dropped documents: a group box can say 144 documents and show 3 inside.
- **`types`**, on every read, the exit-1 document too: the model's `FLOW_LINK_TYPES` names in order, `["follows", "uses", "supplies"]`, never the read's edges, so a screen gives a type one look across pictures without quoting it.

## Code

Core `flow.rs`: `flow_members` (the no-ROOT membership), `FlowNode.in_flow`, `FlowAsk.in_flow_only`, `FlowRead.not_in_flow: Option<usize>`. CLI `flow.rs`: `FlowRequest.in_flow`, the header, `types`. MCP `FlowArgs.in_flow: Option<bool>`, described in the tool's schema alone (`mcp-read.md` "Texts"); the closed mirrors `FlowDocument`, `FlowNode`. Daemon: `args.boolean`.

## Tests

Fixture `fixtures/flow-aside`, an invented pottery workshop: 181 documents, 26 in the flow (10 steps, 4 stages, 14 parts, 142 records, 8 ID-less notes, an ID-less start); a 40x variant built by the tests. CLI `flow_scenario.rs` (the rule over seven fixtures, the cut, 40x, `types`), core `flow_read.rs`, MCP `mcp_flow.rs`, http `flow.rs` (byte-equal to the CLI), `flow_scope.rs` for `--stage`; the genre scans. Unpinned: `counts` taken after the cut is an equivalent mutant, since counts come from the graph subtree and the ROOT holder always stays.
