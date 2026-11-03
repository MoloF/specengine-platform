---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http]
owner: owner
reviewed: 2026-10-31
---

# `spec flow`: the walk, typed edges, lifted edges and gap rows

A read of the flow links (`flow-links.md`): `spec flow [ROOT] [--depth N] [--trace] [--json]`, MCP `get_flow {root?, depth?, trace?}`, daemon `GET .../flow?root=&depth=&trace=`. It writes only the data directory, as `spec tree`. Gap rows and rollups are computed per read; they change no exit, finding, status or gate (ADR-0012, ADR-0047).

## The read

- **Scope.** ROOT's holders and every document within them by `parent:`, flow links or not. No ROOT: every live document holding or receiving a flow link or selected by a `[flow]` rule, with its `parent:` ancestors. So a rooted read can hold a document the whole read lacks: `AREA-HIRE` in `spec-c`, whose whole read has no node. Documents only: a link to `ID#SECTION` lands on its document. Tier 3 and generated stay out (`left_out`).
- **Typed edges.** Every resolved link of `FLOW_LINK_TYPES` with both ends in scope; containment is `group`. `from` and `to` give the reading direction, so a screen draws an arrow with no branch on a type and picks by `type` what it shows.
- **Walk.** `distance`: breadth-first over `follows` from the starts (marked, else steps no `follows` edge enters), null where unreached. It is the scope's walk: under a ROOT it restarts at that scope's starts. Nodes by (distance, unreached last, path), edges by (`from`, `to`, type, kind).
- **Depth** counts from the roots (0), as `spec tree`; default unbounded. A document deeper than N shows as its ancestor at N. A shown node's `distance` is the least of what it holds, itself included, null only when nothing it holds is reached; `start` and `end` are true when it or anything it holds is one, folded over the same members, so the overview marks where a flow begins and ends. An edge with both ends in one shown node joins its `inner`; any other is lifted, merged by (`from`, `to`, `type`, `kind`) with `count`; `when` only at count 1; `path`, `line` the first written. `--depth` never changes a gap row. A node's `gaps` counts the documents this read's measured rows name among what it holds, itself included; the set is taken before any fold or cap, so a depth moves no document's count, and `no-start`'s scope is no document (`FlowGap.files` carries each row's files, not printed).
- **`stage`.** A document's front-matter `stage:`, typed `Text`, in any script: project vocabulary nothing resolves or branches on. Counted by no hash (`node-revision.md` "Never counted"), so re-cutting stages raises no revision and marks no card, and it cuts across groups, which `parent:` cannot. A shown node's is its own: a fold takes nothing from what it holds.
- **`--trace`** adds each shown node's `rollup` and `counts`, `spec trace`'s (`spec-trace.md` "The read"); without it both are `null` and no queue is read. The CLI joins them by file after core's read, so core reads no queue. `rollup` covers the node's own cards, `counts` it and what it holds by `parent:`. It never fails a flow: a queue that cannot be read, a newer one included, leaves both `null` with a note `rollups not shown: <why>`, never `no card`; one below this build's is read as it stands and named. Notes on marks: every card's with no ROOT, as `spec trace`; under a ROOT, what the shown nodes hold.

**Reuse is not here:** `pack:` and overrides are RM-0058's.

## Output

Gap rows first, all ten in table order, so a cut never hides them; a row prints at most `GAP_ITEMS_SHOWN` (50) items, then `, … and <k> more`, `count` the whole. An item's members join with ` + `.

```text
flow <ROOT or all> | nodes <n>, edges <e>, depth <N or ->[; left out: …]
gap <code> | <state> | <count>[ | <item>, …][ | <cause>]
<distance or -> <name> | <kind or -> | <title or -> | <path>:<line>[ | start][ | end][ | group <name>][ | stage <s>][ | holds <h>, inner <i>][ | gaps <g>][ | rollup <state>]
<from> -> <to> | <type> | x<count>[ | kind <kind>][ | when <when>] | <path>:<line>
[truncated: <k> of <n> nodes and <j> of <m> edges not shown; give a ROOT or lower --depth]
```

JSON, every key present, absent `null`: `{root, reason, depth, notes, left_out, truncated, omitted_nodes, omitted_edges, gaps, nodes, edges}`; a gap `{code, state, count, items, cause}`, `state` `measured | not_configured | cannot`, `items` a list of name lists, `cause` one line, `null` at 0; a node, 15 keys, `{id, kind, title, path, line, distance, group, stage, start, end, holds, inner, gaps, rollup, counts}`, `inner` `[{type, count}]`, `counts` `{no_card, in_work, suspect, departed, confirmed}`; an edge `{from, to, type, kind, when, count, path, line}`.

**Cap** as `mcp-read.md` "Size": text at most `OUTPUT_CAP_CHARS` plus one tail, cut at a line, JSON the printed items only; the daemon's `View::Browser` uncut. **Exits** as `spec graph`: 0 answered, zero nodes and any gap included; 1 only a ROOT naming nothing, JSON `reason` (the daemon's 404); 2 usage, a bad `--depth`, a look-alike ID, config outside `[flow]`, `HOME`, `StoreError`.

## `[flow]`

Read only by `spec flow`, in the manner of `[[health.bound]]`: `ProjectConfig` only name-checks it, and it is never a finding or a gate.

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

Each table lists rules, a rule's selectors `kinds`, `classes`, `paths`, `when` in `[[check.rules]]`' grammar, at least one, ANDed; rules ORed; documents only. Steps: `[[flow.step]]`'s, every document holding or receiving `follows`, the starts and ends. A live document's `follows` that does not resolve (dangling, another project's) gives no edge and keeps the document a step. The core names no kind or key. A project's own key in `when` draws `unknown-key`, a warning stopping nothing; `kinds` and `paths` need none.

## Gap rows

| Code | Needs | An item | The rule | `cause` above 0 |
|---|---|---|---|---|
| `no-start` | `start` | the scope | no start, and no `follows` edge enters from outside; a scope with no step fires it | `no step here is a start: match the flow's first step in [[flow.start]], or, reading a part, add a follows link into it from outside` |
| `two-starts` | `start` | all the starts | two or more starts | `more than one start: narrow [[flow.start]] to one` |
| `unreachable` | `start` | a step | no path from a start or entering edge; `cannot` under `no-start` | `no path from a start reaches it: add a follows link on it, naming a step before it` |
| `no-way-to-end` | `end` | a step | no path to an end or leaving edge, in no trap | `no path from it reaches an end: have a later step follow it, or match it in [[flow.end]]` |
| `trap-cycle` | `end` | its members | a `follows` cycle no member of which has such a path | `a loop with no way out: have a step outside it follow a member, or match a member in [[flow.end]]` |
| `unjoined-parallel` | - | the split step | one `parallel` branch, or no later step common to all branches | `a parallel fork with one branch, or branches that never meet: add a branch, or a later step following them all` |
| `cycle-no-repeat` | - | its members | a cycle left once `repeat` edges are removed | `a loop with no repeat edge: write the edge closing it as {to: ID, kind: repeat}` |
| `definition-unused` | `definition` | a definition | no live document `uses` it | `no document uses it: add a uses link to it, or drop it from [[flow.definition]]` |
| `definition-once` | `definition` | a definition | one does | `only one document uses it: add a uses link from a second document, or fold it into that document` |
| `supply-unfed` | - | its members | a `supplies` cycle no node outside it feeds | `these only supply each other: add a supplies link into them from a node outside` |

A cycle is Tarjan's component of two or more nodes or with a self-edge. Items sort by name. A `cause` is one line, why plus how, in text, JSON and the Flow screen; no key was added. Core `measured()` sets `GapCause::Found(code)` above 0, on the headless `no-start` row too; at 0 `null`. A row needing an absent table: `not_configured`, never 0, its cause `` no [[flow.<table>]] rule: add one naming `` the first step by paths, the last step by paths, or what steps share, by paths or kinds. A bad table: the first error by line, `<config>:<line>: <message>`; the rows needing it `cannot`, exit 0. An unknown key in `[flow]` marks all four tables.

**`endless`: no new word.** ADR-0055's three kinds stand. An end step may carry a `repeat` edge to itself, and a loop that can reach an end is no trap, so a flow with no finish marks its resting step as an end and lists nothing.

## Readings

- **A self-edge** stays an edge while its document is shown and joins `inner` when folded.
- **`start`** means the walk's source, derived starts included, so `no-start 1` and a node marked `start` can stand together; neither it nor `end` stays a shown node's own: both fold.
- **Only an unresolved `follows` keeps its document** a step; an unresolved `uses`, `supplies` or `depends_on` does not. A step whose only `follows` is unresolved, with no marked start, is a derived start with distance 0.
- **Several holders of an ID:** a link uses its first admitted holder; under a ROOT, a first holder outside makes it a crossing edge.
- **A left-out source:** a live document receiving `follows` from a left-out document is neither step nor member.
- **Crossing edges.** Under a ROOT, a `follows` edge crossing the scope is an entry or a way out, so a sub-flow is not called broken. It is no edge of that read.
- **`unjoined-parallel`** walks only splits of two or more branches, breadth-first, one step per branch in turn, stopping at the first step all branches reached; a one-branch split is listed with no walk; a split with no join still walks each branch to its end. The walk covers every admitted document, beyond the scope too, so a sub-flow is not called broken.
- **`gaps`.** A row can name a document outside the read, which no node counts; two holders of one ID count as two.
- **`definition-*` and `supply-unfed`** list what is in scope and read links over every live document: domain-free, by links alone.

## Code

- Model: `FLOW_LINK_TYPES`, with `USES` and `SUPPLIES` consts; the `Direction` reads the link, `In` running target to source.
- Core `flow_toml.rs` reads `[flow]` table by table and never fails: a table's first error stays on it. Selectors share `health_toml.rs`' reader and `check/health.rs`' judge (`documents_selected`).
- Core `flow.rs` `flow_read` takes resolved links from `SpecGraph::edges`, builds the scope and steps, computes the ten rows before any fold, then folds by depth. Tarjan is iterative. `flow_walk` -> `FlowWalk {steps, distance}` is that walk with no row computed, for `spec trace`'s lanes; `stage_of` reads a file's `stage:`; `FlowNode.file` is what a read joins by.
- CLI `flow.rs` resolves ROOT, caps and renders, owns the causes' wording (`cause_text`, `remedy`), and joins `TraceBadge` by file (`trace.rs` `flow_badges`). MCP `get_flow` with the closed `FlowDocument` mirror. Daemon route `View::Browser`.

## Known limits

- Gap rows are never cut, so one `cycle-no-repeat` or `two-starts` item can pass `OUTPUT_CAP_CHARS`: a 4 000-member ring measured 48 034 characters on one row. The tail's "lower --depth" cannot shrink a row.
- The read is quadratic in `parent:` depth: a chain of 1 000, 2 000, 4 000 took 0.58, 2.39, 9.79 s in debug (RM-0076).
- A read names no exit step: `Steps.leaving` is computed for rows, not served.

## Tests

Fixtures `fixtures/flow-gaps` (a library's lending, no game word), `flow-relay`, `flow-hire`, `trace-almanac`, `spec-c`. Core and CLI `tests/flow_read.rs`, CLI `trace_views.rs`, MCP `tests/mcp_flow.rs`, http `tests/flow.rs`; the lines: CLI `flow_authoring.rs`, MCP `mcp_flow_authoring.rs`; `check_genre.rs`, `no_domain.rs` and the CLI and MCP genre scans cover the sources. Unpinned: three or more `parallel` branches.
