---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-15
---

# `spec verify`'s output: rows, hints, summary, JSON

What `spec verify` prints for the rows, states and run causes of `docs/canon/spec-verify.md`, and `--tests` for `spec-verify-tests.md` ("`--tests`"). Deterministic: byte-identical stdout, stderr and JSON from any root, cwd or creation order; no timing, no absolute path; control characters escaped (CLI README "Exit codes and streams").

## Text

stdout, in order: each non-`ok` row in key order; the `cannot:` lines of the run causes; the hints; the summary. `note:` lines go to stderr.

```
fix | conflict | R-12 | implements | crates/game/src/stamina.rs | item game::stamina::regen_system function_item
pre-existing check | unlocked | R-20 | configures | assets/data/movement.ron | ron root.stamina.regen_per_second
verify: blocked [enforce-introduced]: 1 fix, 1 check (1 pre-existing), 0 cannot, 11 ok; 9 by blob, 3 by ast; 1 skipped
```

A row: `<class> | <state>[ <category>][ <detail>] | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]`, class `fix`, `check`, `cannot`, prefixed `pre-existing `; a keyless row's binding is `unanchored` or `cannot_verify` and its category, as accept prints it (`spec-lock-accept.md` "Steps" 8). The summary's `[<mode>]` is the mode the verdict used, also with a run cause (plain `enforce-introduced`: `[enforce]`), absent when `[verify]` cannot be read; `(<n> pre-existing)` only with a base; `by blob`, `by ast` count rows by `compared`; `skipped` the `queue` and `other_project` markers.

## Hints

One `hint: <state>[ <category>]: <step>` per (state, category) present, in state order; equal texts stay apart.

| Rows | Step |
|---|---|
| `code_ahead`, `unlocked`, `broken_binding` but `node_gone` | commit, then `spec lock accept <ID>` on a terminal |
| `broken_binding node_gone` | commit first (the node is gone only when `HEAD` agrees), then `spec lock accept <old ID>` on a terminal drops its entries |
| `unlocked key_problem` | names each key's problem; `spec lock accept` cannot lock it, rename what gives it |
| `spec_ahead same_revision`, `conflict` at R = `rev_L` | "the node's text changed at its locked revision: raise its `rev` by hand (`spec bump` prints the line while the edit is uncommitted), commit, then accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`); accept refuses text changed at a revision" |
| `conflict` at R ≠ `rev_L` | "at a revision the lock does not hold, the marker not raised: check the code against the node, set the marker to its revision, commit, then `spec lock accept <ID>` on a terminal" |
| `cannot_verify` | its category's step: fix the item's parse error or move the marker; make the binding name one symbol; cite one node (`slug/ID`, `ID#SECTION`); write `rev` as a number; fix what nulls the hash; make the file readable UTF-8 under `[code]`; fix the parse error before the symbol |
| `predated`, `bad_marker`, other `spec_ahead` | restore, raise or lower the marker (`spec refs <ID>`) |

`conflict` rows of both kinds give one line, the two steps joined by `; `, R = `rev_L`'s first. A run cause gives no hint. Then once, last, when a row would raise a drift item in a sweep (core `drift::raises(state, detail, false)`: `code_ahead`, `spec_ahead`, `conflict`, `broken_binding`): `hint: drift: commit first; spec propose drift queues HEAD's drift for the owner` (`proposal-drift.md`).

## JSON

`--json`: one compact document after discovery, for exit 0, 1 and 2 alike; every key present, absent `null`, uncapped:

```json
{"mode":"enforce-introduced","base":"HEAD","verdict":"blocked",
 "counts":{"rows":13,"ok":11,"fix":1,"check":1,"cannot":0,"pre_existing":1,"skipped":1},
 "cascade":{"by_blob":9,"by_ast":3},
 "rows":[{"node":"R-12","relation":"implements","file":"crates/game/src/stamina.rs","kind":"item",
   "symbol":"game::stamina::regen_system","item_kind":"function_item","state":"conflict","category":null,
   "detail":null,"class":"fix","pre_existing":false,"markers":[{"line":41,"rev":2}],"node_rev":3,
   "lock":{"rev":2,"commit":"a0cea48<33 hex>","verified_at":"2026-10-10"},"compared":"ast"}],
 "delta":{"symbols":null,"bindings":{"moved":null,"ambiguous":[],"lost":[],"stale":[<R-12's six key fields>]},
   "affected_nodes":["R-12"],"tests_reaching":null},
 "cannot":[],"notes":[]}
```

- `mode`: the summary's, `null` when `[verify]` cannot be read; `base`: `null` (plain), `"HEAD"`, `"unborn"`; `verdict`: `clean`, `observed`, `blocked`, `cannot-verify`.
- `rows`: every row, `ok` too, in key order; a keyless row's `kind`, `symbol`, `item_kind` `null`; `markers` `[]` for a lock-only row, `rev` `null` when unversioned or malformed; `node_rev` `null` when unknown or gone; `lock` `null` without an entry; `class` `null` for `ok`; `compared` `blob`, `ast` or `null`. A run cause: `rows` `[]`.
- `delta`: 05 s5.2's reply without `ms`, over the rows not Pre-existing: `bindings.lost` the `broken_binding` rows, `stale` `code_ahead` and `conflict`, `ambiguous` `cannot_verify ambiguous_*`, each a list of row keys (`node`, `relation`, `file`, `kind`, `symbol`, `item_kind`); `affected_nodes` the sorted distinct resolved nodes of non-`ok` rows; `bindings.moved` `null` until 5c, `tests_reaching` until 7a (`bundle-bindings`), `symbols` until 7b (`impact`).
- `cannot`: the run causes' lines, in order; `notes`: the `note:` texts.

## `--tests`

stderr: the question, the runner's output live, `note:`, `warning:`; stdout: the report.

- **Config** (`specengine.toml:<line>: [verify] …`): ``tests must be an argv list, e.g. ["scripts/run-test", "{test}"]``, `tests[<i>] is a <type>, not a string: <that>`, `tests is empty: <that>`, `tests[0], the program, is empty`, ``tests[<i>] `<elem>`: `{x}` is no placeholder: {file}, {qpath}, {package}, {target_kind}, {target}, {test}; {{ and }} write a brace`` (``… a lone `{`: {{ writes a brace``, `}` alike), `tests uses neither {test} nor {qpath}: the command cannot tell which test to run`, `timeout is a <type>, not whole seconds from 1 to 86400`, `timeout <n> is not from 1 to 86400 seconds`; no key: `[verify] tests is not set in specengine.toml`.
- **Question**: per test `run | <file> | <kind> <symbol> | <nodes>` or `not run | … | <reason>`, per node `node <ID>: not verifiable <state>`, last `run <n> bound tests of <k> nodes as ["e0", …] (timeout <t> s each) in <worktree> on <branch> at <commit>? [y/N]`, elements JSON-quoted, control and bidi characters escaped.
- **Reasons** (the summary's tail, JSON `reason`): `dirty` `the worktree differs from HEAD: <XY path>, …; commit them first`; `empty` `no @verifies binding names a node of the set` or `none of the <n> bound tests can run (each says why)`; `declined` `the owner declined; nothing ran`; `refused` `the [verify] tests command passed a test that does not exist; make it fail when no test matches`, `the [verify] tests command timed out after <t> s on a test that does not exist (the probe); make it end when no test matches`, ``the [verify] tests program `<elem0>` cannot start: <error>``; `interrupted` `a signal to spec stopped the run; the runner's processes were stopped`; `changed`, joined `; `: `HEAD moved from <a> to <b> during the run`, `the branch changed from <a> to <b> during the run`, `the worktree changed during the run: <XY path>, …; commit them first`, a git error. `cannot:` lines include `specengine.toml is not a file of HEAD: [verify] tests runs only a committed template` and ``the [verify] tests program `<program>` is not a file of HEAD: commit it first``.
- **Text**: per test by (`file`, `symbol`) bytes `<passed|failed|timed out|not run> | <file> | <kind> <symbol> | <nodes>[ | <detail>]`, detail `exit <n>`, `signal <n>`, `after <t> s` or not run's reason (RON: `ron <field path>`; nodes by ID bytes, `, `); `node <ID>: <verdict>[ <state>]` by ID bytes; `cannot: <cause>`; the summary.

```
passed | crates/game/src/stamina.rs | item crates/game::stamina::tests::regen_stops_at_zero | R-12
failed | crates/game/tests/stamina.rs | item crates/game::test:stamina::drain_is_clamped | R-12, R-13 | exit 101
not run | crates/game/src/stamina.rs | item crates/game::stamina::Regen::check | R-14 | member
node R-12: failed
node R-13: failed
node R-14: no runnable test
verify --tests: unverified: 3 nodes (0 verified, 2 failed, 0 not verifiable, 1 no runnable test); 3 tests (1 passed, 1 failed, 0 timed out, 1 not run)
```

A summary with a reason: `verify --tests[ --task <T>]: <verdict>: <reason>`.

- **JSON** (exits 0-2 after discovery), every key, no timing, hash, tail or worktree: `task`, `branch`, `commit`, `command`, `timeout`, `verdict`, `reason` (null: `verified`, `unverified`, `cannot`), `counts` (`nodes`: `verified`, `failed`, `not_verifiable`, `no_runnable_test`; `tests`: `passed`, `failed`, `timed_out`, `not_run`; `probes`, the planned count), `tests` (`file`, `kind`, `symbol`, `nodes`, `argv` (element 0 as written) or null, `outcome` `passed|failed|timed_out|not_run`, `exit`, `signal`, `reason`), `nodes` (`id`, `verdict` `verified|failed|not_verifiable|no_runnable_test`, `state`), `lock` (null until 8b), `cannot`, `notes`.
