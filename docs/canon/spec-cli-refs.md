---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-15
---

# `spec refs` output: rows, JSON, exits

What `spec refs REF [--json]` prints, for a reader of its output. The command, REF, the order of rows and the cap: `docs/canon/code-bindings.md` "`spec refs`"; the states and bindings a row names: its "ID state", "Revision state", "Rust binding", "Mentions". Code: CLI `refs.rs`.

## Rows

```
LAW-STEP | law | Step | docs/spec/counter.md:17 | rev 1 (none written)
implements | crates/tally/src/counter.rs:23:8 | LAW-STEP@1 | resolved | current | default | item crates/tally::counter::Tally::step function_item
configures | data/limits.ron:6:17 | LAW-STEP | resolved | unversioned | default | ron data/limits.ron#root.step ambiguous duplicate
mention | crates/tally/src/counter.rs:4:28 | LAW-STEP | resolved | file
refs: markers 2, mentions 1; code files 8 (.rs 7, .ron 1), skipped 0
```

- Header: search's first four fields + ` | rev <R>` (`rev 1 (none written)`, `rev unknown`); blocks apart by an empty line.
- Marker: `<relation> | <path>:<line>:<column> | <token> | <id_state>[ <fix>] | <rev_state> | <levels> | <binding>[ | <note>]`, 1-based, the column in bytes at the `@`; levels `default`, `[a, b]`, `invalid:<category>`. Mention: `mention | <path>:<line>:<column> | <reference as written> | <id_state>[ <fix>] | <binding>`.
- Binding: `item <qpath> <kind>[ ambiguous <reason>]`, `module <qpath>[ ambiguous <reason>]`, `ron <path>#<field path>[ ambiguous duplicate]`, `unanchored <category>`, `cannot_verify <category>`, `file` (mentions only).

## JSON

`--json`, the printed rows, every key present (absent `null`), in this order: `{ref, reason, notes, nodes: [{id, kind, title, path, line, rev, rev_written, markers: [{relation, path, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, binding}], mentions: [{path, line, column, written, id_state, fix, binding}]}], code: {files, rs, ron, skipped}, omitted: {markers, mentions}}`; `binding` `{kind, qpath, item_kind, line, ambiguity, category}`, `kind` a binding word above, `qpath` a RON anchor's `<path>#<field path>` too, `line` an item's only. Marker `id` is the token's ID part as written, `rev` its 1-9 digits; node `rev` `null` when unreadable; `levels` (a list), `levels_state` as the eval's `markers.json` (`code-identity.md` "Eval outputs").

## Exit

0 answered, zero rows too. 1 REF names nothing (show's `spec: <reason>`; JSON `nodes` `[]`, `code`, `omitted` `null`). 2, stdout empty: usage, discovery, a config error (`[code]` included), no table, a refused REF, a walk gap.
