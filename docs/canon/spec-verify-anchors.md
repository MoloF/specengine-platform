---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# `spec verify`'s anchors: a renamed or moved binding followed

In verify's judge (CLI `judge_side`), per side (disk, `HEAD`, index, commit): an entry no marker gives any more, paired with a marker binding of its group without an entry, is one row with it. Read-only, no new state: `spec lock accept <ID>` re-keys (`spec-lock-accept.md` "Steps" 6); a rebase never pairs.

## The pairing

**E**: entry-only `item` or `module` rows judged `broken_binding file_gone|symbol_gone`. **B**: rows with no entry bound to an item, an inline `mod` or a file module, code-side lockable (no `bad_levels`, `key_problem`), in a group holding a non-trivial E, hashed in the cascade's one pass; refused or in a file with an `ERROR` or `MISSING` node (ADR-0042): no candidate. A trivial entry selects none; no non-trivial E, nothing more hashed. Group: (node, relation, kind, item_kind), opaque to core `anchor::pair` (pure; 7a, 7b too). A body under `TRIVIAL_BODY_BYTES` (`code-hash.md` "Triviality") on either side never pairs. Per group:

1. C0(e): every b with an equal `body`; none: every b in e's file with an equal `skeleton`.
2. C(e): C0(e); when it holds several, those with an equal `quote`, only when exactly one is left.
3. R(b): the e whose C(e) holds b; when one matched b by `body`, only the `body` matches stay; then step 2's quote rule.
4. MOVED iff C(e) = {b} and R(b) = {e}; else AMBIGUOUS when C(e) ≠ ∅, else LOST.

Over sets, order-free. An ordinal shift (`marker_gone`) never pairs.

## Anchors per row

| `anchor.state` | Row | `from`; `candidates` |
|---|---|---|
| `exact` | an entry and a marker at one key (a dangling one on its entry's row too) | null; null |
| `moved` | the merged row | e's `file`, `kind`, `symbol`, `item_kind`; null |
| `ambiguous` | e's row, `broken_binding` | null; C(e), the same four each, key order |
| `lost` | every other entry row with no binding at its key | null; null |

Null without an entry (`unlocked`, keyless, an unpaired or contested B) or `cannot_verify`.

## The merged row

At b's key with b's markers, against e's entry: b's digests over C (`spec-verify.md` "Cascade" step 4), no blob step, `compared` `ast`; spec-side states stand. `path` changed iff L_lock ∪ L_mark holds it, first in `changed` and `levels.changed`: a moved `[path]` binding reads `code_ahead code_changed path`, the one exception to "Cascade" step 3. An `ok` one's `levels.changed`: `[]`. e's row is dropped. Until accepted each run pairs from the old entry, so a later edit it cannot follow reads `lost`. The `code` fact: digest levels only (`path` alone: null).

## Drift evidence

`drift_hash` place 18 (`proposal-drift-item.md` "The item"): a moved row's `from` as `[file, kind, symbol, item_kind]` (core `HashedRow.moved_from`), else null; `lock_*` e's. Stored rows keep `anchor`; core `DriftRow` never reads it (no move in `spec review`'s text or goals). An open item over a now-followed rename re-keys once.

## Output

Forms: `spec-verify-output.md`. `anchor`, the 17th row key, after `levels`. `delta`, rows not Pre-existing: `bindings.moved` each moved row's entry key and own (`ok` is never Pre-existing: listed until accepted); `ambiguous` adds the `ambiguous` anchors, `lost` the other `broken_binding` rows; a moved `code_ahead` or `conflict` is in `stale` too. Text: `moved from` under a non-`ok` moved row, a `candidate` line per candidate; the summary counts `ok` moves; any move, `ok` too, gives the `moved` hint once.
