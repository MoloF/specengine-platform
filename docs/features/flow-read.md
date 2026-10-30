---
class: spec
status: draft
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui, plugin]
ref: RM-0043; Phase 7 step 2 (08 s2); owner's decisions 2026-10-30, pre-approved, review open
adrs: [ADR-0047, ADR-0055]
---

# flow-read: `spec flow`, typed edges, lifted edges and gap rows (RM-0043)

## Why

`spec graph` walks from one REF and knows no groups or gaps. One flowchart is not enough either: sequence, reuse, supply and requirement mean different things, so the read returns typed edges and a screen picks what it draws. Gaps are computed rows; they stop nothing (ADR-0012, ADR-0047).

## Description and interactions

`spec flow [ROOT] [--depth N] [--json]`, MCP `get_flow {root?, depth?}`, daemon `GET .../flow?root=&depth=`. A read, writing only the data directory, as `spec tree`.

- **Scope.** ROOT's holders and every document within them by `parent:`. No ROOT: every live document holding or receiving a flow link or selected by a `[flow]` rule, with its `parent:` ancestors. Documents only: a link to `ID#SECTION` lands on its document. Tier 3 and generated stay out (`left_out`).
- **Typed edges.** Every resolved link of `FLOW_LINK_TYPES` with both ends in scope; containment is `group`. `from` and `to` give the reading direction, so a screen draws an arrow with no branch on a type and picks by `type` what it shows.
- **Walk.** `distance`: breadth-first over `follows` from the starts (marked, else steps no `follows` edge enters), null where unreached. Nodes by (distance, path).
- **Depth** counts from the roots (0), as `spec tree`; default unbounded. A document deeper than N shows as its ancestor at N. An edge with both ends in one shown node joins its `inner`; any other is lifted, merged by (`from`, `to`, `type`, `kind`) with `count`; `when` only at count 1; `path`, `line` the first written. `--depth` never changes a gap row.

### Output

Gap rows first, all ten in table order, so a cut never hides them; a row prints at most 50 items, then `, … and <k> more`, `count` the whole. An item's members join with ` + `.

```text
flow <ROOT or all> | nodes <n>, edges <e>, depth <N or ->[; left out: …]
gap <code> | <state> | <count>[ | <item>, …][ | <cause>]
<distance or -> <name> | <kind or -> | <title or -> | <path>:<line>[ | start][ | end][ | group <name>][ | holds <h>, inner <i>]
<from> -> <to> | <type> | x<count>[ | kind <kind>][ | when <when>] | <path>:<line>
[truncated: <k> of <n> nodes and <j> of <m> edges not shown; give a ROOT or lower --depth]
```

JSON, every key present, absent `null`: `{root, reason, depth, notes, left_out, truncated, omitted_nodes, omitted_edges, gaps, nodes, edges}`; a gap `{code, state, count, items, cause}`, `state` `measured | not_configured | cannot`, `items` a list of name lists; a node `{id, kind, title, path, line, distance, group, start, end, holds, inner}`, `inner` `[{type, count}]`; an edge `{from, to, type, kind, when, count, path, line}`.

**Cap** as `docs/canon/mcp-read.md` "Size": text at most `OUTPUT_CAP_CHARS` plus one tail, cut at a line, JSON the printed items only; the daemon's `View::Browser` uncut. **Exits** as `spec graph`: 0 answered, zero nodes and any gap included; 1 only a ROOT naming nothing, JSON `reason`; 2 usage, a bad `--depth`, a look-alike ID, config outside `[flow]`, `HOME`, `StoreError`.

### Files in scope

- **Developer:** model `src/link.rs`, `src/lib.rs`; core `src/flow.rs`, `src/flow_toml.rs` (new), `src/check/health.rs` (the selection shared with `bound_candidates`), `src/project_toml.rs`, `src/lib.rs`; CLI `src/flow.rs` (new), `src/lib.rs`, `src/main.rs`; MCP `src/read.rs`, `src/mirror.rs`, `src/server.rs`; http `src/app.rs`; `plugin/specengine/.claude-plugin/plugin.json` 0.21.0.
- **UI developer (ADR-0033's exception, no screen):** `ui/src/api/provisional.ts` (`FlowView`, `FlowGap`, `FlowNode`, `FlowEdge`, `FlowInner`), `ui/src/api/daemonKeys.test.ts` (86 sets).
- **Tester:** `fixtures/flow-gaps`, `fixtures/daemon-keys.json`; new core and CLI `tests/flow_read.rs`, MCP `tests/mcp_flow.rs`, http `tests/flow.rs`; core `tests/check_genre.rs`; CLI `tests/genre.rs`, `tests/read_only.rs`; MCP `tests/common/read.rs` (`TOOLS` 23), `tests/mcp_stdio.rs`, `mcp_size.rs`, `mcp_door.rs`, `mcp_genre.rs`, `plugin_files.rs`, `mcp_decision.rs`, `mcp_path.rs`; http `tests/daemon_keys.rs`, `methods.rs`, `parity.rs`, `projects.rs`.
- **`INSTRUCTIONS`:** the reads line gains `get_flow(root?, depth?) = flow`, 32 B against 8 left: shorten "Hints use flags", at most 2 048 B.
- **Canon at shipping:** new `docs/canon/spec-flow.md`; pointers in `mcp-read.md`, `plugin-files.md`, the http README, the first and last within 9 B of their cap.

## Data

```rust
pub const FLOW_LINK_TYPES: [(&str, Direction); 4] = [
    (FOLLOWS, Direction::In), ("uses", Direction::Out), ("supplies", Direction::Out), ("depends_on", Direction::Out),
];  // the Direction reads the link: `In` runs target to source
```

`[flow]`, read only by `spec flow`, in the manner of `[[health.bound]]`: `ProjectConfig` only name-checks it, and it is never a finding or a gate.

```toml
[[flow.step]]        # optional
kinds = ["lending"]
[[flow.start]]
paths = ["docs/spec/lnd-ask.md"]
[[flow.end]]
when = { phase = "last" }
[[flow.definition]]
kinds = ["bookplate"]
```

Each table lists rules, a rule's selectors `kinds`, `classes`, `paths`, `when` in `[[check.rules]]`' grammar, at least one, ANDed; rules ORed; documents only. Steps: `[[flow.step]]`'s, every document holding or receiving `follows`, the starts and ends. The core names no kind or key. A project's own key in `when` draws `unknown-key`, a warning blocking nothing; `kinds` and `paths` need none.

| Code | Needs | An item | The rule |
|---|---|---|---|
| `no-start` | `start` | the scope | no start, and no `follows` edge enters from outside |
| `two-starts` | `start` | all the starts | two or more starts |
| `unreachable` | `start` | a step | no path from a start or entering edge; `cannot` under `no-start` |
| `no-way-to-end` | `end` | a step | no path to an end or leaving edge, in no trap |
| `trap-cycle` | `end` | its members | a `follows` cycle no member of which has such a path |
| `unjoined-parallel` | - | the split step | one `parallel` branch, or branches sharing no later step, the walk stopping at the split |
| `cycle-no-repeat` | - | its members | a cycle left once `repeat` edges are removed |
| `definition-unused` | `definition` | a definition | no live document `uses` it |
| `definition-once` | `definition` | a definition | one does |
| `supply-unfed` | - | its members | a `supplies` cycle no node outside it feeds |

A cycle is Tarjan's component of two or more nodes or with a self-edge. Items sort by name. A row needing an absent table: `not_configured`, cause `` no [[flow.start]] rule: what starts a flow is the project's to declare ``, never 0. A bad table: the first error by line, `<config>:<line>: <message>`; the rows needing it `cannot`, exit 0.

**`endless`, settled: no new word.** ADR-0055's three kinds stand. An end step may carry a `repeat` edge to itself, and a loop that can reach an end is no trap, so a flow with no finish marks its resting step as an end and lists nothing.

**Scope edges.** Under a ROOT, a `follows` edge crossing the scope is an entry or a way out, so a sub-flow is not called broken. `definition-*` and `supply-unfed`, domain-free by reading links alone, read every live document.

### Fixture

`fixtures/flow-gaps`, a public library's lending, no game word. `[ids]` `WING` (`wing`), `LND` (`lending`), `PLT` (`bookplate`), `shape = "name"`; `docs/spec/<id, lowercase>.md`. LND-ASK also holds one unlinked ID section, LND-ASK-LINE: a store test oracle needs a nested section in every fixture corpus. `[flow]`: step `kinds = ["lending"]`; start `paths` of LND-ASK, LND-A, LND-B; end `paths` of LND-OPEN, LND-END, LND-END-TWO; definition `kinds = ["bookplate"]`. WING-LIBRARY is the parent of WING-LEND, WING-BROKEN, WING-HEADLESS.

```text
parent WING-LEND; LND-ASK, PLT-MARK no links
LND-CHECK    follows LND-ASK, {to: LND-HAND, kind: repeat}
LND-STAMP, LND-NOTE  follows {to: LND-CHECK, kind: parallel}; uses PLT-MARK
LND-HAND     follows LND-STAMP, LND-NOTE; depends_on PLT-MARK
LND-OPEN     follows LND-HAND, {to: LND-OPEN, kind: repeat}
parent WING-BROKEN; LND-STRAY, PLT-ONCE, PLT-UNUSED no links
LND-A, LND-B  follows {to: LND-OPEN, kind: alternative}
LND-END      follows LND-A, LND-B, LND-FORK-A, LND-SPIN-B
LND-END-TWO  follows LND-FORK-B
LND-DEAD     follows LND-A; uses PLT-ONCE
LND-TRAP-A   follows LND-B, {to: LND-TRAP-B, kind: repeat}
LND-TRAP-B   follows LND-TRAP-A
LND-FORK-A, LND-FORK-B  follows {to: LND-B, kind: parallel}
LND-SPIN-A   follows LND-A, LND-SPIN-B
LND-SPIN-B   follows LND-SPIN-A
PLT-FEED-A   supplies PLT-FEED-B
PLT-FEED-B   supplies PLT-FEED-A
parent WING-HEADLESS; LND-Y no links
LND-X        follows LND-Y
```

## Rules and edge cases

- WHEN a gap row counts anything, the system SHALL change no exit, finding, status or gate.

## Acceptance criteria

- [ ] AC-01 — The walk. `spec flow --root fixtures/flow-relay`: exit 0, first line `flow all | nodes 8, edges 10, depth -`; LEG-START, LEG-DRAW, LEG-RUN, LEG-FALL, LEG-SCORE, LEG-END at distance 0 to 4, then KIT-OIL, KIT-TORCH at `-`; seven rows `not_configured`, three `measured` at 0; identical bytes from copies in opposite file orders. M1: nodes in input order turn the copies red.
- [ ] AC-02 — Typed edges. `--json` on flow-gaps holds all four types; a `follows` edge has `from` its target, any other `from` its source. M2: `from` always the link's source turns `LEG-START -> LEG-DRAW` red.
- [ ] AC-03 — Planted gaps. `spec flow WING-BROKEN --root fixtures/flow-gaps`: exit 0, rows in order at `0, 1, 1, 2, 1, 1, 1, 3, 1, 1`: `two-starts` LND-A + LND-B; `unreachable` LND-STRAY; `no-way-to-end` LND-DEAD, LND-STRAY; `trap-cycle` LND-TRAP-A + LND-TRAP-B; `unjoined-parallel` LND-B; `cycle-no-repeat` LND-SPIN-A + LND-SPIN-B; `definition-unused` PLT-FEED-A, PLT-FEED-B, PLT-UNUSED; `definition-once` PLT-ONCE; `supply-unfed` PLT-FEED-A + PLT-FEED-B. `spec flow WING-HEADLESS`: `no-start` names it, `unreachable` `cannot`.
- [ ] AC-04 — Loops are not gaps. `spec flow WING-LEND`: ten rows `measured` at 0, LND-OPEN `end` with its `repeat` edge listed; flow-relay with a scratch `[flow]` by `paths`: the same. M3: keeping `repeat` edges in `cycle-no-repeat` turns both red. M4: calling every cycle a trap turns WING-LEND red.
- [ ] AC-05 — Lifted edges. `spec flow WING-LIBRARY --depth 1`: `nodes 4`, one edge `WING-LEND -> WING-BROKEN | follows | x2`. At depths 0, 1, 2 and unbounded, per type the edges' `count` plus the nodes' `inner` is equal, and the gap rows are byte-equal. M5: counting a merged pair once gives `x1`.
- [ ] AC-06 — Genre. The same binary answers on flow-relay, flow-hire, flow-gaps and `spec-c`, the last `nodes 0`, exit 0; `check_genre.rs`, `no_domain.rs` and the CLI and MCP genre scans pass over the new sources. M6: a literal kind in core `flow.rs` turns the scan red.
- [ ] AC-07 — Surfaces. `get_flow` equals the CLI, meeting its schema; `mcp_door` and `read_only.rs` pass; the daemon's route is uncut. A generated corpus past 40 000 characters: text cut at a line with the tail, every gap row printed, JSON the printed items. M7: JSON keeping edges past the cut turns it red.
- [ ] AC-08 — Config. No `[[flow.end]]`: its two rows `not_configured` with the cause. A rule with no selector or an unknown key: those rows `cannot`, naming `<config>:<line>`, exit 0, `spec check` and `spec tree` unchanged. A `when` on a scratch key selects its document. M8: `not_configured` printing 0 `measured` turns it red.

## Out of scope

The canvas (RM-0044); stages, `pack:`, overrides (RM-0058); `--type`, `--archive`; silencing `unknown-key`; a resolved-edge table (RM-0054): the read stays O(n).

## Implementation

Left for the developer.
