---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# Card mandate: a subtask is binding, or its department decides

ADR-0079 says who decides what a subtask's brief leaves open. Related pages: the card, `cards.md`; a hand-off, `card-flows.md`; a split, `card-split.md`; a card's questions and decisions, `card-questions.md`; a parent's subtasks, `card-progress.md`.

## State

The rule below holds, but none of it is built yet. v5 step 6d `task-mandate` (RM-0104, `docs/features/task-mandate.md`) builds it, with plugin 0.38.0:
- the `mandate` column at queue schema 12, and backup format 6;
- `--mandate` on `spec card new`, `edit` and `split`, in the daemon's three card bodies, and on `create_card`, `edit_card` and `split_card`;
- the choice in the Hand off and Split dialogs;
- the work item's framing by mandate;
- the parent's "Decisions from subtasks".

Until it ships, no card holds a mandate, and a subtask's department reads its brief as it chooses.

<a id="the-rule"></a>
## The rule

A card may hold a mandate: `binding` or `open`, shown as "Binding" and "Department decides".
- **Who names it.** The asking side, when it makes a card with a reviewer: a hand-off, or each subtask of a split. A card made without one holds none, as does every card made before.
- **Who changes it.** Only the card's reviewer, by an edit that is one `card.edited`. The reviewer is compared as an accept compares one: on a terminal, a name nothing verifies; on a hub, the login.

**Binding.** The brief stands. Departing from it goes back up as a return with a question, which the work item offers first.

**Open.** The department decides. It records each decision on the task and says why.

**Either way:**
- A decision on the task can still be recorded (ADR-0012).
- The asking side sees its subtasks' decisions on the parent, a departure from a binding task marked as such. It may object with a question.
- Confirming an answer stays where it is (ADR-0035, ADR-0061).

**A mandate decides nothing.** None of these reads it:
- no move, accept, assign, cancel, `allowed`, rollup or Ready to accept;
- no stage, approve, reject or confirm;
- no check and no gate.

On the page it only orders the acts and frames the dialogs. The store keeps a decision's `why` optional on any card. ADR-0079.
