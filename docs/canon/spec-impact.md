---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-mcp, crates/specengine-store, crates/specengine-core, plugin]
owner: owner
reviewed: 2026-10-21
---

# spec impact: what a change to a node, a code symbol or the code since a revision reaches

Before a change or hand-in: the nodes the impact walk reaches, the code bound to them with each binding's `sync`, the tests to run, this repository's open tasks to warn. Nothing it shows changes an exit, a status, a claim or a verdict (ADR-0046, `architecture-code.md#sync-in-reads`). Built: 7b-1 `docs/features/impact.md`, 7b-2 `docs/features/impact-since.md`.

## Command

`spec impact [REF] [--symbol QPATH] [--since REV] [--depth N] [--json]`, the globals too.

- **Form**, by the library before discovery (no clap group): not exactly one of REF, `--symbol` and `--since` -> exit 2, stdout empty, `spec: impact takes exactly one of REF, --symbol and --since`. `--depth`: `corpus::depth_of`.
- **REF**: `classify` (`@rev` ignored with a note), one index handle (`open_index`, `indexed_in`), `SpecGraph`, `locate` (exit 1: `show`'s reason), graph's holders warning (`all at distance 0`); the seeds its holders.
- **`--symbol QPATH`**, as `spec symbols QUERY` (`symbol-reads.md` "Lookup"): `[code]` required; `refresh_code`, `require_complete`; one `code_snapshot(SnapshotSymbols::LookupWithFiles {query, name})`: the hits, and every symbol of each file holding a best-tier hit by (path, ord), in the markers' transaction. Matches: the hits at the first hit's tier, in lookup order; none: exit 1, ``--symbol `<QPATH>` names no symbol``. A stored marker of a match's file seeds when `symbols::maps(marker, judged)` gives a place and `unmapped::reached` holds the match there: on the match, on an item or inline mod enclosing it (`parent_ord`), on the file module or the file. It seeds every node its ID names (`resolved`, `alias`, `ambiguous`), any relation. Mentions, dangling, look-alike, queue and other-project IDs, `unanchored`, `cannot_verify` and RON markers seed nothing. No seed: `nodes` `[]`, exit 0.

## Since

- **Guard**, before discovery, no git child: REV starting with `-` or holding U+0000-U+001F, U+007F -> exit 2, stdout empty, ``spec: --since `<REV>` is refused: a revision never starts with `-` or holds a control character`` (controls `\u{XX}`).
- **Read**, one `VerifyGit` session, a failure exit 2: `[code]`; `open`; `list_revision(REV)`, `None` exit 1 ``--since `<REV>` is not a commit`` (unknown, a tree, an unborn `HEAD`, a range, `^REV`: `RevAnswer::Named` takes `rev-parse --verify -q`'s exit 1, whatever printed, and an exit 0 not one OID as none; `list_commit`, `Strict`, fails on a misbehaving git); the disk walk (`read_tree`: untracked in; `.gitignore`, git index unread); `read_code`, REV's code files and manifests under today's `[code]` by `cat-file --batch` (documents, lock, config unread), `CommitCode::tree`; `finish()`. A tag is peeled; `revision` the full OID.
- **Seeds** (`code_delta.rs`): an item `removed` or `changed` reaches the nodes REV's markers bind it to, `added` or `changed` the disk's: on it, an item enclosing it (the code index's parent helper) or the file's module; in a RON file every value-anchored marker. IDs as `--symbol`'s; distance 0.

## Delta

One record builder (`code_delta`) and comparer (core `symbol_delta::compare`, pure): an uncut `--since HEAD`'s `delta` equals `verify --changed --json`'s `delta.symbols`.

- **Files**: `.rs`, `.ron` paths one side alone holds, or both with other bytes; non-UTF-8 skipped on its side.
- **Records**, afresh from each side's bytes (index digests unread): each Rust item `{file, qpath, item_kind, line, sig, body}` as its side's walk names it, digests by `item_levels` (`None`: unhashable, a re-parse failed); a RON file one (`qpath` its path, `item_kind` `file`, line 1, digests the `b3_hash` of its bytes).
- **Compare**: key `(file, qpath, item_kind)`, twins by (line, order), the surplus added or removed; `changed`: `sig` or `body` differs, or a `None`; lists by (file, line, qpath, item_kind).
- Comments, markers, formatting, CRLF change no Rust item (a RON CRLF copy is `changed`); a move: removed plus added; a `mod`, `#[path]` or package change over equal bytes: `bindings.moved` only.
- `verify --changed` (`--task` too) computes it under `--json` only (`VerifyFormat`, `verify_as`; the library `verify()` is `Json`): `HEAD`'s code (`HeadSide::code`; unborn: all `added`) against the walk, uncapped (`spec-verify-output.md` "JSON").

## Radius

- **Nodes**: `graph::impact_reach(graph, starts, depth) -> Reach {walk, left_out}`: `spec graph --impact`'s walk (both call `reach`), equal by construction (`spec-cli-graph.md` "spec graph"); `--depth`, `left_out`, Tier 3 as graph's; nodes by (distance, path, ord).
- **Rows**: with `[code]`, one `set_bindings` over the keys of every reached node and every node within it (`SpecGraph::within`), each row once: `tests` the `verifies` rows, `symbols` the rest, verify's order, `BindingRow`s (`code-reads.md` "The binding row"). Without `[code]` (`CodeConfig::from_toml` `Ok(None)`): `symbols`, `tests`, `code` `null`, the code index untouched, no note.
- **`code`**: `SetBindings.code`, the judge's walk counts (`spec refs`' `{files, rs, ron, skipped}`), `null` without `[code]` or under causes (1), (3), (8); then the lookup's or disk walk's. Exit 1: `null`.
- **Tasks**: `roadmap::open_tasks(env, project, slug, git, GitPlace::Unnamed) -> OpenTasks {here, others, notes}`, open by core `PlanTask::is_open`, the queue read as `spec roadmap` reads it (`open_existing`, no schema step). A task of `here` is listed when `node_set::named_set(targets ∪ affected_nodes)`, each resolved now with the sections it holds, meets R: the reached nodes and every node within each. Its `nodes`: the reached N, in `nodes`' order, whose {N} ∪ within(N) meets that set; tasks by number. `others` meeting R: one note, `others_note(k, slug, "spec impact")`. No repository, queue or task: `[]`, no note.

## Notes

stderr and JSON `notes`, in order: the md refresh's; the holders warning (`--symbol`: the lookup refresh's; `--since`: the disk walk's); the queue's; last 7a's one `bindings: <cause>` (`set_bindings`' refresh notes unprinted). Under `GitPlace::Unnamed` no note carries a path, the root never string-replaced:

- the data directory or the queue unreadable: `tasks not shown: the queue cannot be read (spec roadmap names the cause)`;
- git failing in the root: `tasks not shown: git in the project root failed (spec roadmap names the cause)`;
- an `[ids]` prefix taking `PR` or `T`: `tasks not shown: <config>: <clash>`; a row not decoded: `<row>; not shown`; ``<k> task(s) of another repository of the project `<slug>` not shown: `spec impact` shows the current repository's``.

`spec roadmap` (`GitPlace::Root`) keeps its texts (`spec-roadmap.md` "Notes").

## Output

```text
seed symbol <QPATH>: <n> at <tier>
since <7 hex>: <f> files, <c> changed, <a> added, <r> removed
impact: nodes <N>, symbols <S|->, tests <T|->, tasks <K>[; code files <f> (.rs <r>, .ron <o>), skipped <s>][; left out: <g> generated, <a> archived]
node <graph's node line>
tested | <state>[ <category>] | <node>
symbol <row line>
  sig <signature>
test <row line>
task <id> | <status> | <title or -> | <names>
seed <qpath> | <item_kind> | <path>:<start>-<end>
<changed|added|removed> | <file>:<line> | <symbol> <item_kind>
[truncated: <parts> not shown; lower --depth N or narrow the REF or --symbol]
```

A header under `--symbol` or `--since` only (`<7 hex>` `revision`'s first seven, never git's abbreviation; totals before the cut); the summary's totals, the code part when `code` is set, zero `left out` parts omitted; rows `cap.rs` `row_line`, then `row_lines`' `sig`, `moved from`, `candidate` lines; a task's names ID, else path; delta lines last, `changed`, `added`, `removed`, each in the comparer's order. `--since`'s tail ends `lower --depth N or name a later --since REV`.

JSON, every key present, absent `null`: `{input {ref, symbol, since}, reason, revision, depth, seeds {nodes, symbols}, delta, left_out {generated, tier3}, nodes, tested, symbols, tests, tasks, notes, truncated, code}`; `tested`: `spec-lock-tested.md` "Reads". `input` as given; `revision` `--since`'s OID. `seeds.nodes` the printed distance-0 names; `seeds.symbols` the printed matches `{qpath, item_kind, path, start_line, end_line, tier}`, `null` for REF and `--since`. `delta` under `--since`: `{added, removed, changed: [{file, symbol, item_kind, line}], renamed: null}`, the printed items (`symbol` the qpath, a RON file's path; `line` a removed one's old). A node graph's object; `tasks` `{id, status, title, nodes}`. `truncated`: `null` uncut, else `{nodes, tested, symbols, tests, tasks, seeds, delta}` not shown (0 for a list not cut, `null` for a `null` list). Exit 1: `input`, `depth`, `reason`, `notes` set, `left_out` zeros, `nodes`, `tasks`, `seeds.nodes` `[]`, `seeds.symbols` `[]` under `--symbol`, the rest `null`.

## Cut

`OUTPUT_CAP_CHARS` 40 000: the header and summary always print; then nodes, tested lines, symbol rows (each with its lines), tasks, seed matches, delta items, each whole, while the text stays within the cap (`lines_within`, the first item whole); the first item that does not fit and all later ones are cut. The tail's parts `<n> nodes`, `<e> tested`, `<s> symbols`, `<t> tests`, `<k> tasks`, `<m> seed matches`, `<d> delta items`, zero parts omitted. The JSON holds exactly the printed items.

## Exits and degrade

0 answered (zero items too). 1 REF names nothing (`show`'s reasons), `--symbol` no symbol or `--since` no commit: stdout empty, `spec: <reason>` after the notes, the JSON document. 2, stdout empty: the form; graph's (`--depth`, discovery, config, `HOME`, `StoreError`, a look-alike ID, a `project:` REF); under `--symbol` also `spec symbols QUERY`'s (no `[code]`, a `[code]` error, a walk gap, a failed refresh); under `--since` the guard and "Since"'s. The REF form degrades as 7a's reads (`code-reads.md` "Degrade": (1), (3), (8) rows `null`; (4)-(6) `sync -`; one note); `nodes`, `tasks` and the exit never change with code. Under `--symbol`, `--since`, (4)-(6) only mark rows; a gap `set_bindings`' own refresh meets is its note, exit 0.

Text and JSON byte-identical across roots, clones, cwd, creation order and CRLF Rust: no root, `HOME` or date. Writes the data directory only; git as 7a's reads (`rev-parse`, `hash-object` without `-w`) plus the tasks' `WorktreeGit` probe, `--since` also `ls-tree -r -z`, `cat-file --batch`; `GIT_OPTIONAL_LOCKS=0`, never `status`, `diff`, `ls-files`.

## MCP get_impact

`{id?, symbol?, since?, depth?}` (`ImpactArgs`, unknown fields refused), each nullable; no root `oneOf`. Not exactly one non-null of `id`, `symbol`, `since`, before discovery: an error result ``spec: get_impact takes exactly one of `id`, `symbol` and `since` `` + LF, no `structuredContent`. Then one `call`: content the output, `structuredContent` the document; read annotations. `IMPACT_DESCRIPTION` 2 043 B, ASCII, its figures asserted. Mirror `ImpactDocument` (`Impact{Input,Seeds,SeedSymbol,Node,Task,Truncated,Delta,DeltaItem}`, `BindingRowShape`, `CodeCount`): a key the JSON may set `null` nullable, `delta.renamed` `AlwaysNull`. `INSTRUCTIONS`' reads line ends `get_impact(id|symbol|since, depth?) = impact.` (`mcp-read.md` "Tools"). Plugin: `plugin.md` "Skills".

## Tests

CLI `impact.rs` (`fixtures/impact/`), `impact_since.rs`, `verify_delta.rs` (scripted histories), `impact_{cost,pilot}.rs` (`#[ignore]`); core `symbol_delta.rs`. MCP `mcp_impact.rs`, `mcp_door`, `mcp_size`, `mcp_genre`, `plugin_{skills,files}`.

## Known limits

- `LookupWithFiles` reads each best-tier hit's file whole.
- The REF form costs about 3x `show --bindings` (771 against 259 ms, `code-a`): it judges it all.
- `--symbol` refreshes code twice (lookup, `set_bindings`); a name tier can match many items. `--since` reads every REV code blob, binds REV's tree, parses each compared Rust file twice per side (bind, `item_levels`), then `set_bindings` refreshes. The cut bounds the answer, not the work.
- A nested repository or submodule under a code root reads as `added` on every run; `renamed` is not filled (RM-0031).
