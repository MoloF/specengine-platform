---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-14
---

# `spec verify`'s output: rows, hints, summary, JSON

What `spec verify` prints for the rows, states and run causes of `docs/canon/spec-verify.md`. Deterministic: byte-identical stdout, stderr and JSON from any root, cwd or creation order; no timing, no absolute path; control characters escaped (CLI README "Exit codes and streams").

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
| `spec_ahead same_revision`, `conflict` at R = `rev_L` | "the node's text changed at its locked revision: raise its `rev` by hand, commit, then accept; or `--editorial` for an editorial change (`spec lock accept <ID> --editorial`); accept refuses text changed at a revision" |
| `conflict` at R ≠ `rev_L` | "at a revision the lock does not hold, the marker not raised: check the code against the node, set the marker to its revision, commit, then `spec lock accept <ID>` on a terminal" |
| `cannot_verify` | its category's step: fix the item's parse error or move the marker; make the binding name one symbol; cite one node (`slug/ID`, `ID#SECTION`); write `rev` as a number; fix what nulls the hash; make the file readable UTF-8 under `[code]`; fix the parse error before the symbol |
| `predated`, `bad_marker`, other `spec_ahead` | restore, raise or lower the marker (`spec refs <ID>`) |

`conflict` rows of both kinds give one line, the two steps joined by `; `, R = `rev_L`'s first. A run cause gives no hint. The drift hint (`spec propose drift`) is 4c's (`docs/features/drift-queue.md`).

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
