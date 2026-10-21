---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-store, plugin]
owner: owner
reviewed: 2026-10-21
---

# spec impact: what a change to a node or a code symbol reaches

Before a change: the nodes the impact walk reaches, the code bound to them with each binding's `sync`, the tests to run, this repository's open tasks to warn. Nothing it shows changes an exit, a status, a claim or a verdict (ADR-0046, `architecture-code.md#sync-in-reads`). Built: 7b-1 `docs/features/impact.md`; 7b-2 (`--since`, `delta.symbols`) is `docs/features/impact-since.md`.

## Command

`spec impact [REF] [--symbol QPATH] [--depth N] [--json]`, the globals too.

- **Form**, judged by the library before discovery (no clap group): not exactly one of REF and `--symbol` -> exit 2, stdout empty, `spec: impact takes exactly one of REF and --symbol`. `--depth`: `corpus::depth_of`.
- **REF**: `classify` (`@rev` ignored with a note), one index handle (`open_index`, `indexed_in`), `SpecGraph`, `locate` (exit 1: `show`'s reason), graph's holders warning (`all at distance 0`); the seeds its holders.
- **`--symbol QPATH`**, as `spec symbols QUERY` (`symbol-reads.md` "Lookup"): `[code]` required; `refresh_code`, `require_complete`; one `code_snapshot(SnapshotSymbols::LookupWithFiles {query, name})`: the hits, and every symbol of each file holding a best-tier hit by (path, ord), in the markers' transaction. Matches: the hits at the first hit's tier, in lookup order; none: exit 1, ``--symbol `<QPATH>` names no symbol``. A stored marker of a match's file seeds when `symbols::maps(marker, judged)` gives a place and `unmapped::reached` holds the match there: on the match, on an item or inline mod enclosing it (`parent_ord`), on the file module or the file. It seeds every node its ID names (`resolved`, `alias`, `ambiguous`), any relation. Mentions, dangling, look-alike, queue and other-project IDs, `unanchored`, `cannot_verify` and RON markers seed nothing. No seed: `nodes` `[]`, exit 0.

## Radius

- **Nodes**: `graph::impact_reach(graph, starts, depth) -> Reach {walk, left_out}`, `reach` with the impact table: the walk `spec graph --impact` runs (`graph_with_view` calls `reach`), equal by construction (`spec-cli-graph.md` "spec graph"); `--depth`, `left_out`, Tier 3 as graph's; nodes by (distance, path, ord).
- **Rows**: with `[code]`, one `set_bindings` over the keys of every reached node and every node within it (`SpecGraph::within`), each row once: `tests` the `verifies` rows, `symbols` the rest, verify's order, `BindingRow`s (`code-reads.md` "The binding row"). Without `[code]` (`CodeConfig::from_toml` `Ok(None)`): `symbols`, `tests`, `code` `null`, the code index untouched, no note.
- **`code`**: `SetBindings.code`, the judge's walk counts (`spec refs`' `{files, rs, ron, skipped}`), `null` without `[code]` or under causes (1), (3), (8); then under `--symbol` the lookup walk's. Exit 1: `null`.
- **Tasks**: `roadmap::open_tasks(env, project, slug, git, GitPlace::Unnamed) -> OpenTasks {here, others, notes}`, open by core `PlanTask::is_open`, the queue read as `spec roadmap` reads it (`open_existing`, no schema step). A task of `here` is listed when `node_set::named_set(targets ∪ affected_nodes)`, each resolved now with the sections it holds, meets R: the reached nodes and every node within each. Its `nodes`: the reached N, in `nodes`' order, whose {N} ∪ within(N) meets that set; tasks by number. `others` meeting R: one note, `others_note(k, slug, "spec impact")`. No repository, queue or task: `[]`, no note.

## Notes

stderr and JSON `notes`, in order: the md refresh's; the holders warning (`--symbol`: the lookup refresh's); the queue's; last 7a's one `bindings: <cause>` (`set_bindings`' refresh notes unprinted). Under `GitPlace::Unnamed` no note carries a path, the root never string-replaced:

- the data directory or the queue unreadable (a newer build's, a database error): `tasks not shown: the queue cannot be read (spec roadmap names the cause)`;
- git failing in the root: `tasks not shown: git in the project root failed (spec roadmap names the cause)`;
- an `[ids]` prefix taking `PR` or `T`: `tasks not shown: <config>: <clash>`; a row not decoded: `<row>; not shown`; ``<k> task(s) of another repository of the project `<slug>` not shown: `spec impact` shows the current repository's``.

`spec roadmap` (`GitPlace::Root`) keeps its texts (`spec-roadmap.md` "Notes").

## Output

```text
seed symbol <QPATH>: <n> at <tier>
impact: nodes <N>, symbols <S|->, tests <T|->, tasks <K>[; code files <f> (.rs <r>, .ron <o>), skipped <s>][; left out: <g> generated, <a> archived]
node <graph's node line>
symbol <row line>
  sig <signature>
test <row line>
task <id> | <status> | <title or -> | <names>
seed <qpath> | <item_kind> | <path>:<start>-<end>
[truncated: <parts> not shown; lower --depth N or narrow the REF or --symbol]
```

The header under `--symbol` only; the summary's totals, the code part when `code` is set, zero `left out` parts omitted; rows `cap.rs` `row_line`, then `row_lines`' `sig`, `moved from`, `candidate` lines; a task's names ID, else path.

JSON, every key present, absent `null`: `{input {ref, symbol, since}, reason, revision, depth, seeds {nodes, symbols}, delta, left_out {generated, tier3}, nodes, symbols, tests, tasks, notes, truncated, code}`. `input` as given; `since`, `revision`, `delta` `null` (7b-2's). `seeds.nodes` the printed distance-0 names; `seeds.symbols` the printed matches `{qpath, item_kind, path, start_line, end_line, tier}`, `null` for REF. A node graph's object; `tasks` `{id, status, title, nodes}`. `truncated`: `null` uncut, else `{nodes, symbols, tests, tasks, seeds, delta}` not shown (0 for a list not cut, `null` for a `null` list, `delta` `null`). Exit 1: `input`, `depth`, `reason`, `notes` set, `left_out` zeros, `nodes`, `tasks`, `seeds.nodes` `[]`, `seeds.symbols` `[]` under `--symbol`, the rest `null`.

## Cut

`OUTPUT_CAP_CHARS` 40 000: the header and summary always print; then nodes, symbol rows, test rows (each with its lines), tasks, seed matches, each whole, while the text stays within the cap (`lines_within`, the first item whole); the first item that does not fit and all later ones are not shown. The tail's parts `<n> nodes`, `<s> symbols`, `<t> tests`, `<k> tasks`, `<m> seed matches`, zero parts omitted. The JSON holds exactly the printed items.

## Exits and degrade

0 answered (zero nodes, rows or tasks too; matches no marker seeds). 1 REF names nothing (`show`'s reasons) or `--symbol` names no symbol: stdout empty, `spec: <reason>` after the notes, the JSON document. 2, stdout empty: the form; graph's (`--depth`, discovery, config, `HOME`, `StoreError`, a look-alike ID, a `project:` REF); under `--symbol` also `spec symbols QUERY`'s (no `[code]`, a `[code]` error, a walk gap, a failed refresh). The REF form degrades as 7a's reads (`code-reads.md` "Degrade": (1), (3), (8) rows `null`; (4)-(6) `sync -`; one note); `nodes`, `tasks` and the exit never change with code. Under `--symbol`, (4)-(6) only mark rows; a gap `set_bindings`' own refresh meets is its note, exit 0.

Text and JSON byte-identical across roots, cwd, creation order and CRLF: no root, `HOME` or date. Writes the data directory only; git as 7a's reads (`rev-parse`, `hash-object` without `-w`) plus the tasks' `WorktreeGit` probe, never `status` or `diff`.

## MCP get_impact

`{id?, symbol?, depth?}` (`ImpactArgs`, unknown fields refused), each nullable; no root `oneOf`. Not exactly one non-null of `id`, `symbol`, before discovery: an error result ``spec: get_impact takes exactly one of `id` and `symbol` `` + LF, no `structuredContent`. Then one `call`: content the output, `structuredContent` the document; read annotations. `IMPACT_DESCRIPTION` 1 898 B, ASCII, its figures asserted. Mirror `ImpactDocument` (`Impact{Input,Seeds,SeedSymbol,Node,Task,Truncated}`, `BindingRowShape`, `CodeCount`): `truncated` and its `symbols`, `tests`, `seeds` nullable; `input.since`, `revision`, `delta`, `truncated.delta` `AlwaysNull`. `INSTRUCTIONS`' reads line ends `get_impact(id|symbol, depth?) = impact.` (`mcp-read.md` "Tools"). Plugin 0.15.0: `implement` step 1 calls it per target before the first edit (`plugin.md` "Skills").

## Tests

CLI `impact.rs` over `fixtures/impact/` and copies of `spec-a`, `spec-c`, `code-a`, `anchor-states`, `verify-tests`, `verify-ecs`; `impact_{cost,pilot}.rs` (`#[ignore]`). MCP `mcp_impact.rs`, `mcp_door`, `mcp_size`, `mcp_genre`, `plugin_{skills,files}`.

## Known limits

- `LookupWithFiles` reads every symbol of each file holding a best-tier hit: one match in a large file reads it whole.
- The REF form costs about 3x `show --bindings` (771 against 259 ms on `code-a`): it judges the whole radius.
- `--symbol` refreshes code twice (the lookup's, `set_bindings`'); a name tier can match many items, bounded by the cut, not by work.
- Another repository's tasks are counted, never listed; `renamed` is not filled.
