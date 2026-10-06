---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-core, crates/specengine-model]
owner: owner
reviewed: 2026-10-06
---

# spec tree, spec graph, show --links

CLI pass 3 (`docs/features/spec-cli-graph.md`): a node's ancestors, children, links and an edit's reach without opening files; the shapes feed MCP `get_tree`, `get_node(with: [links])` (`docs/canon/mcp-read.md`) and pass 4's bundle (05 §6). Read-only; each command is one library function returning a struct (`render_text`, `render_json`). As `spec show` (`crates/specengine-cli/README.md`): discovery, `--json`, freshness, REF forms and resolution (several holders → all by (path, ord), one `warning:`), streams, the one-line rule, writes only in the data directory. A node is named by its ID, else its path.

## Input

`update`, `SpecIndex::indexed_input()`, then every parsed file re-read from the working tree and re-parsed by the check's parser (`CheckFile::parsed`) whatever its size: the source of the graph, `path:line` and written forms, so a call sees each file in one state. A file that cannot be read or panics the parser keeps its indexed parse, its lines 1, one `warning:`. `check::SpecGraph` resolves parents and links once per call as `spec check` does. No table, column, query, SQL recursion or `INDEX_FORMAT` change. `parent:` is read from the bytes, so `#SECTION`, `slug/` and `project:` are kept.

## Live sources

Live = neither `class: generated` nor Tier 3 (`check::is_tier3_file`). Tree nodes (a left-out one takes its subtree), incoming links and `graph` edges come only from live files; `--archive` admits Tier 3, never generated; the REF or ROOT holders' files are always admitted (` | archived`). Left out: `left_out {generated, tier3}` (links; `tree`: each left-out node met, not its subtree) and the summary suffix `; left out: <g> generated, <t> archived (--archive)`, zero parts omitted. `--archive` admits every link written in a Tier 3 file, the `out supersedes` on X that a Tier 3 D's `status: superseded-by X` gives included.

## Parents and links

- **Parent.** A document's: its `parent:` resolved from its file; several holders → the first in (path, ord), one `warning:`; dangling → a root marked ` | parent <written> dangling` (`spec check`: `ref-dangling`); `project:` → an unmarked root. A section's: the nearest enclosing ID section, else its document (an ID-less feature holds its `{#AC-..}` sections). A cycle (a self-parent too) is broken at its member first in (path, ord): a root marked ` | parent cycle`, each node once, one `warning:` naming the members (`spec check`: `parent-cycle`). Warnings only for nodes a walk reaches. Nothing comes from directories or kinds (ADR-0008); ROOT reaches beyond `[paths] spec`.
- **Source**: the innermost ID section around the link, else the document, else the file's path.
- **Target**: an ID → its node, `ID#SECTION` → that section; a path → its document, an anchor (slug, `attr`, `html`, ID) → the innermost ID section holding it; an anchor naming nothing → the document, `reason` set (`canon-anchor`, `link-anchor`). A `canon:` path without `#anchor` → the document, resolved and followed, reason `` `canon:` names no #anchor; lands on the document ``; no document there → `unchecked`, `` …; not checked `` (the check: `canon-form`); a `canon:` at a non-canon file → dangling (`canon-file`). Never-checked paths (`LICENSE`, `x.rs`) → `unchecked`, `project:` → `skipped`: neither followed. Inline mentions keep the check's name fallback (`MEC-STAMINA-based` → MEC-STAMINA).
- **`superseded-by`**: in D, `status: superseded-by X` is the edge X `--supersedes-->` D written at D's `status:` line, live as D's file: on D `in supersedes X`, on X `out supersedes D`.
- **Parity** with the check by written place: the (path, line) of every link `--links` marks dangling over the live documents, out or in, equal those of `ref-dangling`, `mention-dangling`, `link-dangling`, `canon-file` (`parent:` aside).

## Link types

One built-in table in `specengine-model`, no `specengine.toml` key:

| Type | `graph` | `--impact` | `--links` |
|---|---|---|---|
| `depends_on`, `derived_from`, `verifies`, `uses_term` | out | in | strong |
| `constrains` | out | out | strong |
| `supersedes`, `revises`, `amends`, `answers`, `working_answer`, `canon`, `adopts`, an unknown declared type | out | — | strong |
| `mentions` | — | — | weak |

## spec tree

`spec tree [ROOT] [--depth N] [--kind K]… [--archive]`, containment only. Roots: ROOT's holders; none given → every live document under `[paths] spec` with no resolving parent, by path; none there → `nodes 0, roots 0`, exit 0, `note: no document under [paths] spec "<value>"; give a ROOT` (documents but no root: `… is a root; give a ROOT`). Pre-order, a node's nested sections (by ord) before its child documents (by path). `--depth N` counts from the roots (0). `--kind K` (free, repeatable) filters lines after the walk; `depth`, `parent` stay the whole tree's. `nodes` counts the lines listed, `roots` those at depth 0. JSON `{ref, reason, notes, depth, kinds, archive, left_out, truncated, nodes}`, flat pre-order; a node `{id, kind, title, path, line, depth, parent, mark, status, rev, tokens_est, archived}`, `parent` the name listed under, `mark` `null | "dangling-parent" | "parent-cycle"`. On `fixtures/spec-a`:

```text
DOM-GAME | domain | Lantern Keep | docs/spec/game.md:1 | status accepted
  RULE-CORE-LOOP | rule | Core loop | docs/spec/game.md:21
  DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1 | status accepted
    RULE-MOVE-SPEEDS | rule | Speeds | docs/spec/movement/README.md:16
    MEC-SPRINT | mechanic | Sprint | docs/spec/movement/sprint.md:1
      RULE-SPRINT-COST | rule | Cost | docs/spec/movement/sprint.md:18
        EDGE-SPRINT-EMPTY | edge-case | Empty tank | docs/spec/movement/sprint.md:22
    MEC-STAMINA | mechanic | Stamina | docs/spec/movement/stamina.md:1 | status accepted
      RULE-STAM-REGEN | rule | Regeneration | docs/spec/movement/stamina.md:21
      EDGE-STAM-ZERO | edge-case | Depletion | docs/spec/movement/stamina.md:25
nodes 10, roots 1
```

A line's tail: ` | status <s>`, ` | archived`, then ` | parent <written> dangling` or ` | parent cycle`.

## spec graph

`spec graph REF [--impact] [--type T]… [--depth N] [--archive]`: breadth-first from REF's holders (distance 0), each node visited once, a visited node's edge still listed. A node's links include its nested sections'; a nested section reached by itself is a separate node. Default: every strong type outgoing; `--impact`: the impact column; `--type T` (free, repeatable) replaces the set: outgoing, under `--impact` in its table direction, else incoming. `--depth N` stops at distance N (not expanded); default unbounded. A dangling, `project:` or unchecked edge of a followed type from a reached node is listed, its end as written plus the `--links` state suffix, never followed.

```text
<distance> <name> | <kind or -> | <title or -> | <path>:<line>[ | archived]
<src> --<type>--> <dst> | <path>:<line>[ | dangling: <reason> | skipped: another project | unchecked]
nodes <n>, edges <e>[; left out: …]
```

Nodes by (distance, path, ord), then edges by (type, path, line, column) in link direction, `path:line` where written. JSON `{ref, reason, impact, types, depth, archive, notes, left_out, truncated, nodes, edges}`; `types` `[{type, direction}]` as followed, in order: the `--type`s given (a repeat once); else `IMPACT_LINK_TYPES`; else the 12 `LINK_TYPES`, then every unknown declared type of the corpus by name; a node `{id, kind, title, path, line, distance, archived}`; an edge `{src, type, dst, written, path, line, state, reason}`.

## spec show --links

`spec show REF --links [--archive]`: `show` unchanged, each shown node's links block between its header line and its bytes; `--archive` without `--links` → exit 2, one line. Out: the links written in the node's span (a document: front-matter and every nested section), every state. In: the links resolving to the node or a nested section from admitted files (the shown node's own file always). Strong before `mentions`, then (type, path, line, column).

```text
  <out|in> <type> <name, else written> | <path>:<line>[ | at <section>][ | as <written>][ | dangling: <reason> | skipped: another project | unchecked]
  links <o> out, <i> in[; left out: …]
```

`at`: the nested section holding the link or landed on. `as`: the written form when it is not the name of the node it lands on (for `superseded-by`'s out link: the shown node); `[[X]]` prints `as [[X]]`. JSON: a node's `links` (`null` without `--links`) `{outgoing, incoming, left_out, omitted}`; a link `{type, origin, at, name, written, path, line, state, reason}`, `state` `resolved | dangling | skipped | unchecked`.

## Exit, cap, determinism

Exit 0 answered: zero nodes, dangling parents or links, cycles. 1 only an unresolvable REF or ROOT (`show`'s reasons, JSON `reason`). 2 as `show`: usage, `--depth` not an integer ≥ 0, a look-alike or mixed-script ID (naming the Latin fix), `project:`, config, `HOME`, `StoreError`; JSON for 0 and 1 only.

`OUTPUT_CAP_CHARS` cuts at a node, edge or link line, the first item whole. Tails: `[truncated: <k> of <n> nodes not shown; give a ROOT, lower --depth or add --kind]`, `[truncated: <k> of <n> nodes and <j> of <m> edges not shown; lower --depth or add --type]`, `show`'s (bounded: CLI README "Output and the cap") plus `; links not shown: <k>`. JSON holds exactly the printed items, `truncated: true`. `show --links`: the block counts toward the cap; a cut in the block falls at a line end and hides the node's text. Its JSON follows the text's cut (the same nodes, link lines, text bytes): the cut node's `omitted` is the tail's `<k>` (its unprinted links and those of the nodes and holders after it), 0 elsewhere; the cut node stays in JSON as the carrier of `truncated` and `omitted` even when its header was not printed.

Determinism: `BTreeMap`s and the orders above, never rowid, insertion or the absolute root; copies written in opposite orders at different roots give byte-identical text and JSON.

## API

- model: `Direction {Out, In}` (`as_str`), `IMPACT_LINK_TYPES: [(&str, Direction); 5]`, `graph_direction`, `impact_direction`, `is_weak_link` (`&str -> …`).
- core `check`: `SpecGraph::new(&CheckInput, &IdScheme, &Paths)`, reads no file; lookups `paths`, `file_of`, `standing`, `is_tier3`, `nodes`, `node`, `document`, `documents`, `name`, `line`, `locate(&Reference, written) -> Endpoint`; tree `parent -> Option<Parent>`, `listed_under`, `children`, `cycles`, `ancestors` (pass 4), `within`; links `edges`, `links(at, admit) -> NodeLinks`, `walk(starts, follow: Fn(&str) -> Option<Direction>, depth, admit) -> Walk`. Types `NodeAt {file, ord}`, `Standing {Live, Generated, Tier3}`, `Parent {None, Node {node, others}, Dangling {written, reason}, Skipped {written}, Cycle {members}}`, `Endpoint {Nodes, Dangling, Skipped, Unchecked}`, `LinkState` (`as_str`), `Edge` (`state`, `reason`, `written_end`), `NodeLinks`, `Walk`.
- CLI: `tree(&Env, &Globals, &TreeRequest) -> TreeOutcome` (`TreeNode`, `TreeMark`), `graph(…, &GraphRequest) -> GraphOutcome` (`GraphNode`, `GraphEdge`, `FollowedType`), `ShowRequest {reference, links, archive}`, `ShownNode.links` (`ShownLinks`, `ShownLink`), `LeftOut`, `Outcome::{Tree, Graph}`; `corpus.rs` the input, live rule and REF location. Dependencies unchanged; `petgraph` only in core.

Tests: CLI `tree.rs`, `graph.rs`, `links.rs`, `bounds.rs`, `determinism.rs`, `read_only.rs`, `freshness.rs`, `show.rs`, `genre.rs` over scratch copies of `fixtures/spec-a`, `-b` with their own `HOME`; core `spec_graph.rs`; eval `build_graph.rs` unchanged.

## Open

- Every call re-reads and re-parses every used file; measured through MCP (`docs/canon/mcp-read.md` "Latency"): a lighter resolver input, or the indexed parse when the bytes match, before large pilots.
- The same-size edit race (`freshness.rs`) is probabilistic on its red side: no seam between the index update and the re-read.
- The unreadable and parser-panic fallback (lines 1, `warning:`) is untested (fix: a `&dyn Source`, as `show`'s).
