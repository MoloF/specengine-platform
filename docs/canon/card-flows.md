---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-09
---

# Card flows: config, hand-off and return

## State

Not built. v4 step 3a `board-handoff` (RM-0085, `docs/features/board-handoff.md`) builds it. Until it ships, `[card]` is `cards.md` "Config", with no `return` key; a child card is a `parent` link that no rule reads; a move takes no note; `spec card new` names no reviewer, so a reviewer is `assign`'s.

<a id="the-rule"></a>
## The rule

A hand-off is how one trade asks another for work: a child card, its `parent` the asking card, in the receiving trade's flow, which its discipline picks as for any new card (`cards.md` "Config"). Its `reviewer`, the asking side, is named when the card is created. A hand-off never moves a card across flows, which stays refused. The receiver takes the card by moving it out of its flow's first column: there is no "take" act. "Accept" stays the asking side's word and act: the child's reviewer accepts its result, never its assignee (ADR-0056).

A return sends work back. It is a move into the one column a flow names as `return`, and it needs a reason, a note that the move's `card.moved` event keeps. Only the child moves. The asking card stays where it is and shows its children as handed off, taken, returned or accepted.

No card write moves, creates or refuses another card. A parent waits on no child and a child on no parent: each moves, is accepted and is cancelled on its own. A required reason is a transition rule met in the same act, so nothing is blocked (ADR-0012). The core knows a column whose entry needs a note, never its name (ADR-0008). ADR-0074.
