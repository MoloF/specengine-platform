---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-31
---

# spec trace: what a card records

What a card froze of its targets, which `spec-trace.md` "Suspect" compares with the nodes as they read now, and what its acts log; ADR-0056, ADR-0057, built by `card-trace` (RM-0047), widened by `trace-truth` (RM-0066). Code: core `trace.rs`, store `queue/cards.rs`, model `card.rs`, CLI `revisions.rs`.

## Snapshot

`snapshot` holds `CardFrozen {id, rev, norm_hash, within_hash, also}`: `targets`' entries in order, then any dropped target's. `NULL` on a manual card and on one made before this slice. `norm_hash` is the target's own, as the lock reads it; `within_hash`, `#[serde(default)]`, is `null` when none could be frozen and on an entry stored before it, none migrated. `also` keeps the last 16 hashes vouched for (`ALSO_MAX`). No schema step: the columns hold JSON already.

```json
{"snapshot": [{"id": "SHT-FRONT", "rev": 2, "norm_hash": "b3:...", "within_hash": "b3:...", "also": []}],
 "acceptance": {"by": "desk", "at": "...", "note": null, "verdict": "departs",
  "departs": ["SHT-FRONT"], "discrepancy": "PR-0007"}}
```

| When | Core | The entries |
|---|---|---|
| `new`, a correction | `freeze` | each target as it reads now |
| `edit --target` | `retargeted` | a kept target keeps its entry, a new one is frozen, a dropped one stays after `targets` |
| an accept | `refrozen` | `targets`' alone, frozen anew; one gone, or reading null where its entry compares, keeps its entry |
| a clear | `cleared` | each suspect entry in reach frozen again |
| an editorial apply | `vouched` | the new hash of the entry's kind joins `also` |

**Dropped entries** are inert to marks (`suspect_on` walks `targets`), so neither `edit_card` nor two edits can remove a mark, and a target that returns finds its old hash. A card with all its targets dropped keeps its snapshot. An editorial apply still vouches for them, logging `card.refreshed` and raising `revision`, rightly: a returning target must not mark falsely. An accept writes `targets`' entries alone, and nothing returns after it: a closed card takes no edit. They have no cap, bounded only by the distinct nodes a card ever targeted: a cap would reopen the hole a dropped entry closes.

## Within

**Recipe.** Members are core's `nodes_within`, which `SpecGraph::within` calls, so a read and an apply take the same: the node and the ID sections nested in it. A pair is (name, `norm_hash`), the name the node's ID, else its file's path. Pairs sort by name, then hash for a repeated ID, so a moved section is no change and a new one is. One member: `within_hash` is its `norm_hash`, the same string. Any hash null: `null`. Else `b3_hash` of `<name>\0<norm_hash>\n` per pair: core holds no BLAKE3, so `within_input` returns the bytes and the CLI hashes them. `TargetNow`, per read and never stored, has one constructor, `target_norms`: `TargetNorms`, one target's first node by (path, position), from `SqliteIndex::node_norms`.

**One kind.** A card entry compares one kind of hash and its `also` holds only that kind: an entry with a non-null `within_hash` compares it, an entry with none compares `norm_hash`, never judged by comparing strings. A refresh rewrites the row, so a key stored absent returns as `null`, which reads as none. Every refresh pair carries its kind (`CardRefresh {node, kind, old, new}`, `HashKind::Own` or `HashKind::Within`), and `vouched(entry, kind, old, new)` refuses a pair of the other kind. A document whose two hashes moved still sends two changes, and an entry takes one.

**Over the same members.** A `within_hash` pair goes only when the node's sorted member names are equal in both parses, so `--editorial` on a change that adds or removes an `{#ID}` section vouches nothing for the nodes around it and their cards keep the mark. A section cannot be removed by any proposal, so that direction runs only through a completion. A completion sends a `within_hash` pair only for a node its `(editorial)` trailers name, and also only when, with every such named node taken out, the remaining (name, `norm_hash`) pairs are equal in both blobs. A node with no sections sends both pairs, one string, so an ordinary editorial stays clean on either kind of entry.

**An entry with none** marks on its own text alone, with no false mark, and a note only where the target's two hashes differ: `<C>: <target> froze its own text alone: a change in its sections marks nothing until an accept or a clear`, which write the key. A null now reads not suspect, with `mark_notes`' note.

## Re-estimating

`edit --estimate E --reason T` on a card holding an estimate, `-` for `E` too: `--reason` required (exit 1), no other field beside it (exit 2), `card.reestimated`, `new` null on a clear, so the record keeps `old`. `--reason` beside a first estimate exits 2: that stays `card.edited`. `edit_card` takes `reason?`.

## Events

A departure's discrepancy: `node_ids`, `summary` its flags; `gap_type` `--gap-type` or `contradicts`; `severity` `normal`; `evidence` `[{file: "C-0001", observed, documented}]`; `options` core's two, change the brief and rework the result, each `not priced at acceptance`; `recommendation` 0; author `human`.

`card.accepted` gains `verdict`, `discrepancy`. `card.cleared {id, outcome, nodes: [{id, rev, from, to}], correction, reason, as}`, `outcome` `no_effect`, `needs_correction`, `corrected`; a node's `id` is the target, `from` and `to` what its entry compares, so they differ even when the own hash held, `rev` the target's own, so a `Revision:` join for a nested change walks `within`. `card.reestimated {id, old, new, reason, as}`; `card.refreshed {id, node, proposal}`, one per (card, node): the dedupe in `refresh_snapshots` is the store's rule for a node repeated in a refresh, which a duplicated `{#ID}` can reach. A card row that does not decode is left alone: recording a commit never fails for a card's sake. For `CR-` (RM-0056): consequences from `spec impact --json`, `spec trace --json`; the estimate delta from `card.reestimated` plus each correction's `estimate`.

## Known limits

- `next_card_id` and `card_history` read with `row.get`: bad bytes in a card ID or `events.payload` exit 2.
- An accepted card with no snapshot or a null hash rolls up `confirmed`; only a note differs.
- Accepted: an entry with no `within_hash` can go clean if a sentence moves into a section and the section is deleted by hand.
- Accepted: a section's editorial through a completion marks a holder no trailer names, which a person clears: a false mark, never a false clean.
