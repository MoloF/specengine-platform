---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, fixtures]
ref: Phase 3 plan 2026-10-10, slice 7b of impact-bundle (WA-6, WA-10, WA-11, WA-14; Q1, Q7-Q9, Q13 decided under the owner's pre-approval, review open); 05 s5.3; 06 s1; 07 s1.2, s2; 08 s2
adrs: [ADR-0046]
---

# impact: the blast radius of a node, a symbol or a revision

## Why

08:131 promises the blast radius through `get_impact` (07 s1.2: `node_id | qpath | since: <commit>` -> nodes, symbols, tests, tasks); only `spec graph --impact` exists, nodes only, from one node. An agent about to change code, or handing in a branch, has no single call for what its change reaches, and reads graphs and files instead.

Delivered: `spec impact` and MCP `get_impact` over 7a's rows and sync (`docs/canon/code-reads.md`), and `spec verify`'s `delta.symbols`. Slice 7 split (Q1): 7a first, 7b this. ADR-0046 (`architecture.md#control`, `architecture-code.md#sync-in-reads`).

**In scope:** `spec impact REF | --symbol QPATH | --since REV`; `get_impact`; the tasks in the radius; `delta.symbols`; the `INSTRUCTIONS` line; `mcp_door` coverage; fixtures.

## Description and interactions

- **An agent** asks what a node, a symbol or its branch's code change reaches: nodes, bound symbols with their `sync`, the tests to run, the open tasks to warn. **The owner** runs it on a branch (`--since main`).
- **Crates**: core the pure seeds and order, the walk reused; store the revision reader (4a's `HEAD` base reader at any commit: `rev-parse`, `ls-tree -r -z`, one `cat-file --batch`, one `hash-object --no-filters --stdin-paths`, under `GitEnv::child_vars`) and tasks by node; CLI `impact`, verify's delta; MCP `get_impact`. No new crate or dependency; MCP's stay exactly 7 (`build_graph.rs`), no git library.
- **Reused, unchanged**: `spec graph --impact`'s walk, table and JSON (`spec-cli-graph.md`); 6a-3's lookup and `unmapped`'s containment (`docs/canon/symbol-reads.md` "Lookup", "spec unmapped"; `lookup_symbols`, `unmapped::reached`, `Seeds`, `symbols::place`: its "Crates and seams"); 7a's row, order, sync and refresh.
- **Unchanged**: `spec check`, gate, hook, CI, `INDEX_FORMAT`, queue schema, every other read. **Writes**: the data directory only; the queue is read, never written.
- **Overridden lines**, fixed at shipping (SW-2): 05:160 (`delta.symbols`), 07:31 (`node_id | qpath` -> `id | symbol`, 2026-10-10), 07:82-83, 08:44, 08:131.

## Data

### Command

Q8, WA-10. `spec impact REF | --symbol QPATH | --since REV [--depth N] [--json]` (+ `--root`, `--config`): exactly one input, else exit 2 ``spec: impact takes exactly one of REF, --symbol and --since``. MCP `get_impact {id?, symbol?, since?, depth?}`, each nullable, no root `oneOf` (as `get_task`, `task-package.md` "MCP"): not exactly one -> error result ``spec: get_impact takes exactly one of `id`, `symbol` and `since` ``. Argument names are the CLI's flags, the positional REF `id` as in `get_node`, `refs` (WA-17 mirror rule, `code-tools.md` "Tools"; 2026-10-10: overrides 07 s1.2's `node_id | qpath`, fixed at shipping). The read tools' annotations; `content` and `structuredContent` as every read (`mcp-read.md` "Parity").

### Seeds and walk

Seeds at distance 0, then `spec graph --impact`'s breadth-first walk (`depends_on`, `derived_from`, `verifies`, `uses_term` in; `constrains` out), each node once, `--depth N` as graph's.

- **REF**: its holders; `nodes` equal `spec graph REF --impact --json`'s.
- **`--symbol QPATH`**: 6a's lookup at its best non-empty tier (`exact`, `suffix`, `name`), every symbol there; each reaches the nodes bound by containment (a marker on it, an enclosing item or inline mod, its file module; `docs/canon/symbol-reads.md` "spec unmapped"), any relation. None: exit 1 ``spec: --symbol `<q>` names no symbol``.
- **`--since REV`**: "Since"; a changed or removed symbol reaches the nodes REV's markers bound by containment, a changed or added one those the disk's markers bind.

### Since

Q7, WA-11.

- **REV**: a leading `-` or a control character: exit 2 before any git call. Then `git rev-parse --verify --end-of-options <REV>^{commit}`; not a commit: exit 1 ``spec: --since `<REV>` is not a commit``.
- **Compared**: REV's tree with the working tree on disk under the `[code]` roots and excludes, untracked files included, `.gitignore` and the git index never read (as `spec check --changed`, `spec-check-git.md` "The changed check"). `spec.lock` is not involved: drift since verification is `spec verify`'s question.
- **Reads**: REV's `ls-tree -r -z` once; disk OIDs from one `hash-object --no-filters --stdin-paths`. Equal OID: unchanged. A differing path: REV's blob from one `cat-file --batch`, parsed; each item's 5a level digests against the index's (6a's columns; it stores no `ast_hash`: `symbol-index.md` Q11, 2026-10-13); a RON file whole.
- **Delta**: `added` (disk only), `removed` (REV only), `changed` (both, the hash differs), each `{file, symbol, item_kind, line}` (`line` the disk's, REV's for removed), by file bytes, line, symbol; an equal-hash edit (a comment, formatting) is no change; `renamed` null until 7b's follow-up builds it on 5c's core pairing (`anchor-states.md` Q13, Q14: 5c fills `bindings.moved` only).

### Output

Text: seeds, the four lists, the summary; a row is 7a's line (`docs/canon/code-reads.md` "The binding row"):

```
since a0cea48: 1 file, 1 changed, 1 added, 0 removed
  changed | crates/tally/src/counter.rs:22-30 | crates/tally::counter::impl Tally #1 impl_item
  added | crates/tally/src/counter.rs:41-44 | <qpath> function_item
node LAW-BOUND | law | Bound | docs/spec/counter.md:13 | distance 0
node cap-check/PASS-01 | pass-mark | Stepping past the cap keeps the cap | docs/features/cap-check.md:13 | distance 1
symbol LAW-BOUND | implements | crates/tally/src/counter.rs:22-30 | item crates/tally::counter::impl Tally #1 impl_item | layer a | sync code_ahead code_changed
  sig impl Tally
test cap-check/PASS-01 | verifies | crates/tally/tests/bounds.rs:16-22 | item <qpath> function_item | layer a | sync unlocked
task T-0003 | in_progress | Raise the cap | LAW-BOUND
impact: nodes 2, symbols 1, tests 1, tasks 1
```

A REF seed prints no seed line; `--symbol` prints `symbol <q>: <n> at <tier>` and each match `  <qpath> | <item_kind> | <path>:<start>-<end>`.

- `nodes`: graph's node object `{id, kind, title, path, line, distance, archived}` by (distance, path, ord).
- `symbols`: the rows (relation ≠ `verifies`) of every node reached; `tests`: their `verifies` rows (criteria verifying a reached node are reached: `verifies` is walked in); 7a's order and sync.
- `tasks`: this repository's (`git_common_dir`) tasks whose `targets` or `affected_nodes` resolve to a reached node, status not `done`, `accepted` or `cancelled` (`tasks.md` "Transitions"): `{id, status, title, nodes}` (`nodes` the reached ones it names, in radius order), by ID.
- **JSON**, every key present, absent null: `{input: {ref, symbol, since}, revision, depth, seeds: {nodes, symbols}, delta: {added, removed, changed, renamed}, nodes, symbols, tests, tasks, notes, truncated, code}`; `revision` the resolved commit; `delta` null but for `--since`; `seeds.symbols` `{qpath, item_kind, path, start_line, end_line, tier}`; `truncated` `{nodes, symbols, tests, tasks}` not shown; `code` as `spec refs`' `{files, rs, ron, skipped}`.
- **Cut** at `OUTPUT_CAP_CHARS` (40 000): lists filled in order, the rest counted, then ``[truncated: <n> nodes, <s> symbols, <t> tests, <k> tasks not shown; narrow with --depth N, a REF or a narrower --symbol]``; JSON holds the printed items only.

### Exits and degrade

| Exit | When |
|---|---|
| 0 | answered, zero nodes or rows too |
| 1 | REF names nothing (`show`'s reason); `--symbol` names no symbol; REV is not a commit |
| 2, stdout empty | usage (inputs, `--depth`, an option-like or control-character REV); discovery; config; a look-alike ID; `--symbol` or `--since` without `[code]` (`code-refs.md`'s sentence), with a bad `[code]` or a walk gap; `--since` without git or on a git failure |

The REF form degrades as 7a's reads (`docs/canon/code-reads.md` "Degrade"): no `[code]`: `symbols`, `tests` null, silent; a code-layer cause: nodes and tasks unchanged, rows per 7a's table, one note. `--symbol` and `--since` take code as their input, so its failures exit 2 as `spec symbols`; a lock cause still only marks rows `sync -` with one note (ADR-0046).

### delta.symbols

Q13. `spec verify --changed --json`: "Since"'s `{added, removed, changed, renamed}` with REV = `HEAD`, the same reader and order, over all code, bound or not; unborn `HEAD`: every item `added`. Plain: null.

### INSTRUCTIONS

WA-14. The tools line names `get_impact` within the 2 048 B compile-time asserts (`server.rs`), the `probes` build included: the reads lines reworded, none dropped. 2026-10-21: 70 B of room after 6b-2b (1 978 B; 2 037 with `probes`, 11 B left: `mcp-read.md` **Texts**), 7a about 33 B of it (the `probes` build about 22 B over: 7a cuts); in the reads line, `; get_impact(id|symbol|since, depth?) = impact` (46 B): a cut elsewhere, never a rule, re-measured after 7a.

## Rules and edge cases

- WHEN run from two roots, another cwd, in reverse creation order or on CRLF copies, text and JSON SHALL be byte-identical.
- WHEN `since` comes from an agent, it SHALL be refused before any git call if option-like or holding a control character, and SHALL reach git after `--end-of-options`.
- Door: git only `rev-parse`, `ls-tree`, `cat-file`, `hash-object` without `-w`; never `status` or `diff` (they refresh `.git/index`). Genre: no stack, kind or fixture literal; `item_kind`, qpaths as data; the terminal task statuses are model's `TaskStatus`, not literals.
- Cost grows with the diff (every REV blob under the code roots listed, differing ones parsed); the cut bounds the answer.

## Acceptance criteria

rust-developer, M: mutation -> the test turning red:

- [ ] RD-1 -- M: outgoing `depends_on` followed -> `impact_nodes_equal_graph_impact`.
- [ ] RD-2 -- M: two inputs accepted -> `impact_takes_exactly_one_input` (CLI exit 2; MCP error result); `node_id` or `qpath` as argument names -> `get_impact_arguments_are_the_flag_names` (`{id}`, `{symbol}` answer; `{node_id}` refused by `additionalProperties: false`).
- [ ] RD-3 -- M: `-o…` or a control character passed through -> `option_like_since_is_refused_before_git`.
- [ ] RD-4 -- since. M: `HEAD` used instead of REV -> `since_compares_the_given_revision`; untracked files ignored -> `since_counts_untracked_code`; an equal-hash edit counted -> `since_ignores_unchanged_hashes`.
- [ ] RD-5 -- M: the exact item only -> `symbol_impact_reaches_the_enclosing_binding`.
- [ ] RD-6 -- delta. M: filled in plain mode -> `plain_verify_has_no_symbol_delta`; `renamed` set -> `renamed_stays_null`.
- [ ] RD-7 -- M: no tail -> `impact_cut_names_narrowing`; a terminal task listed -> `impact_lists_open_tasks_only`.
- [ ] RD-8 -- M: `mcp_door` without `get_impact {since}` -> `mcp_door.rs`'s tool coverage; `INSTRUCTIONS` over 2 048 B, `probes` included -> `server.rs`'s compile-time asserts; a git library in MCP -> `build_graph.rs`'s dependency count (7); `git status` -> `reads_leave_git_index_and_objects_untouched` (new, the form of 7a-1's `show_bindings_writes_nothing_under_the_root`).

test-engineer:

- [ ] TE-1 -- 7a's repository plus commits: an item changed, one added, one removed, a comment-only edit, an untracked code file, a RON edit; `--symbol` on a nested item; a task in each status; `expected.json` with each form's lists.
- [ ] TE-2 -- two roots, another cwd, reverse creation order, CRLF: identical text and JSON for the three forms.
- [ ] TE-3 -- cost: `--since` over 1k changed files, latency recorded; a 10k-node radius cut with its tail.
- [ ] TE-4 -- genre: `spec-c`: `impact REF` exit 0, code lists null; `--since` exit 2 naming `[code]`; the source scans.
- [ ] TE-5 -- MCP: parity with `--json`, the schema walker, `mcp_size`'s maximum for `get_impact`, `mcp_door` with `{since}`.
- [ ] TE-6 -- door: after each scenario `git status --porcelain=v1 -z --untracked-files=all`, `.git/index` mtime, the object count, `HEAD` unchanged.
- [ ] TE-7 -- pilots, manual (`#[ignore]`, release, `--timeout`, foreground, scratch `HOME`): `impact` latency by form, `--since` over a real branch; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec, with `bundle-bindings.md`'s SW-1 (ADR-0046, the canon, 08, the amended drafts).
- [ ] SW-2 -- at shipping: Tier 2 `docs/canon/impact.md`; `mcp-read.md` tools row and `INSTRUCTIONS` figures; the overridden lines; CLI, store, MCP READMEs; `CLAUDE.md` State.

## Risks

`INSTRUCTIONS` has 70 B left after 6b-2b, 11 B in the `probes` build, about 37 B and none after 7a: `get_impact`'s 46 B need a cut of about 9 B, 46 B in the `probes` build, re-measured after 7a. Filling `delta.symbols` changes `check_binding`'s `outputSchema` (`mcp-code.md` "Mirrors", `{"type": "null"}` today): its mirror, a plugin MINOR and a `SCHEMA_PINS` entry. An agent's `since` is untrusted input to git ("Rules"). `--since` cost grows with the diff, `--symbol`'s `name` tier can match many symbols: both bounded by the cut, not by a cap on work. A task's nodes resolve at the reading root, as 7a's package rows. 7b generalises 4a's reader (a draft): a change there reopens "Since".

## Out of scope

filling `renamed` (7b's follow-up on 5c's core pairing, `anchor-states.md` Q14); `bindings.moved` (5c); a call-graph radius (5a `deps`, LSP); documents changed since REV as seeds (`spec check --changed` shows them; `spec graph --impact` per node); `--archive`; an HTTP route and the graph's impact screen (Phase 4); `--format dot|mermaid`; tasks of other repositories; a cap on `--since` work.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 split | 7a `bundle-bindings`, then 7b this | 7a's value waits for 7b's git work |
| Q7 `since` | a git revision vs the working tree, untracked included; the lock not involved | lock state is a second drift judge; `HEAD` only gives no branch radius |
| Q8 `get_impact` | a new read tool, twin of `spec impact`; graph unchanged | `graph --impact` changes shipped JSON, and MCP has no graph tool |
| Names (2026-10-10) | `{id, symbol, since, depth}`: the CLI's flags, REF `id` (WA-17, as 6b) | 07's `node_id`, `qpath`: two names for one argument |
| Q9 ADR | ADR-0046 (shared with 7a) | the rule lives only in feature canon |
| Q13 `delta.symbols` | `--changed` only, `HEAD` vs disk, all code; plain null | against the lock duplicates `bindings` |

WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Working answers

[P] checked, [A] assumed. WA-6 [P/A] the row key is the lock key; 7a's row serves `symbols`, `tests`. WA-10 [P] `get_impact` is `node_id | qpath | since: <commit>` (named `id`, `symbol`, `since` from 2026-10-10) returning nodes, symbols, tests, tasks (07 s1.2, 06 s1); the walk and its directions exist (`spec-cli-graph.md`). WA-11 [P/A] `since` reuses `--changed`'s git calls at a given revision; git runs only as a subprocess (`build_graph.rs`). WA-12 [P] slice 7 owns `delta.symbols` (`spec-verify.md`, `symbol-index.md` Q6). WA-14 [P] `INSTRUCTIONS` is 1 885 B, 2 042 with `probes`, against 2 048, compile-time asserted; 6b adds four tools first. WA-15 [A] the MCP canon is near 12 288 B: detail in a new `impact.md` canon at shipping.

## Dependencies and ordering

- **Needs**: 7a (rows, sync, refresh, degrade) and through it 1, 3, 4a, 6a; 4a's `HEAD` base reader, generalised here.
- **Order**: 7a -> 7b; 6b-1 and 6b-2 ship before 7a (08 s2, 2026-10-16) and take their `INSTRUCTIONS` shares first. **Independent**: 4b, 4c, 8a, 8b. **Later**: `renamed` on 5c's core pairing (`anchor-states.md` Q14: 5c ships first and leaves it null), Phase 4 (the graph's impact mode).

## Implementation

Not started. To record: modules, TE-3's and TE-7's figures, deviations with reasons.
