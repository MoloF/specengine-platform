---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-11
---

# Card questions: the items a card names, and its Activity

ADR-0077: a card asks, and the answer is a recorded decision. The card: `cards.md`; the items: `agent-intake.md`, `proposal-decision.md`; a record: `decision-record.md`; a stage: `decision-staging.md`.

## State

The rule below holds; nothing of it is built. v5 step 5 `card-questions` (RM-0097, `docs/features/card-questions.md`) builds it: queue schema 11, backup format 5, `--card` and `card_id`, a decision's `why` and its `{{why}}` slot, a card's `items`, the card door's raise, the stage's `as`, Activity. Until it ships, no proposal names a card, the queue stands at schema 10 and backup format 4, and the daemon raises no proposal outside a hub's `/mcp` and a departing accept.

<a id="the-rule"></a>
## The rule

A question, a discrepancy or a decision may name one card, its `card_id`, set when it is raised and never changed. The card is one of the same queue, in any state; no other kind names one, and an item names at most one. A decision may also say why it was taken. Its record names no card: the card reads its items back, each with its state and, once approved, its record.

The page raises a question or a decision on a card through the card door: one write under the acting name, the typed one locally (ADR-0057), the login on a hub, where it is a plain write any login makes. A return that asks is two writes, the move first. An answer is a stage on the item, confirmed as any stage is: on a terminal, or on a hub by a Confirm its `[team]` allows (ADR-0035, ADR-0061).

The link decides nothing. No card move, accept, assign, cancel or clear, no `allowed`, rollup or gate reads a card's items, and no stage, approve, reject or confirm reads an item's card. A card with an open question moves and is accepted as any other; an answered question moves no card (ADR-0012). ADR-0077.
