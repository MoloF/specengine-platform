---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-31
---

# spec trace: what a card records

What a card froze of its targets, which `spec-trace.md` "Suspect" compares with the nodes as they read now, and what its acts log; ADR-0056, ADR-0057, built by `card-trace` (RM-0047). Code: core `trace.rs`, store `queue/cards.rs`, model `card.rs`.

## Snapshot

`snapshot` holds `CardFrozen {id, rev, norm_hash, also}`: `targets`' entries in order, then any dropped target's. `NULL` on a manual card and on one made before this slice. `also` keeps the last 16 hashes vouched for (`ALSO_MAX`). No schema step: the columns hold JSON already.

```json
{"snapshot": [{"id": "SHT-FRONT", "rev": 2, "norm_hash": "b3:...", "also": []}],
 "acceptance": {"by": "desk", "at": "...", "note": null, "verdict": "departs",
  "departs": ["SHT-FRONT"], "discrepancy": "PR-0007"}}
```

| When | Core | The entries |
|---|---|---|
| `new`, a correction | `freeze` | each target as it reads now |
| `edit --target` | `retargeted` | a kept target keeps its entry, a new one is frozen, a dropped one stays after `targets` |
| an accept | `refrozen` | `targets`' alone, frozen anew; one gone or reading null keeps its entry |
| a clear | `cleared` | each suspect entry in reach frozen again |
| an editorial apply | `vouched` | the new hash joins `also` |

**Dropped entries** are inert to marks (`suspect_on` walks `targets`), so neither `edit_card` nor two edits can remove a mark, and a target that returns finds its old hash. A card with all its targets dropped keeps its snapshot. An editorial apply still vouches for them, logging `card.refreshed` and raising `revision`, rightly: a returning target must not mark falsely. An accept writes `targets`' entries alone, and nothing returns after it: a closed card takes no edit.

## Re-estimating

`edit --estimate E --reason T` on a card holding an estimate, `-` for `E` too: `--reason` required (exit 1), no other field beside it (exit 2), `card.reestimated`, `new` null on a clear, so the record keeps `old`. `--reason` beside a first estimate exits 2: that stays `card.edited`. `edit_card` takes `reason?`.

## Events

`card.accepted` gains `verdict`, `discrepancy`. `card.cleared {id, outcome, nodes: [{id, rev, from, to}], correction, reason, as}`, `outcome` `no_effect`, `needs_correction`, `corrected`; `card.reestimated {id, old, new, reason, as}`; `card.refreshed {id, node, proposal}`. For `CR-` (RM-0056): consequences from `spec impact --json`, `spec trace --json`; the estimate delta from `card.reestimated` plus each correction's `estimate`.
