---
class: canon
tier: 2
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-09
---

# Card tasks: the `T-` tasks a card links

## State

Not built. v4 step 3b `card-tasks` (RM-0092, `docs/features/card-tasks.md`) builds it. Until it ships, a card links no task: `cards` has 24 columns at queue schema 9, `spec card edit` takes no `--task`, and a task shows only on Tasks.

<a id="the-rule"></a>
## The rule

A card links the `T-` tasks that carry its code work: a list of task IDs on the card's own row, written by `spec card edit --task` and its MCP and daemon twins, and nowhere else. No task row, proposal, event payload or file records the link, and a task never names its card.

The link decides nothing. No gate, card move, accept, assign, cancel or clear reads it, and no task's approve, plan, claim, report, complete or cancel reads a card. A task's state is read per request only to be shown beside its card: it refuses no card write, moves no card and proves nothing about the card's work. Completing a task leaves its card where it stands; accepting a card leaves its links as they are. Accept stays a person's act (ADR-0056), and nothing is blocked (ADR-0012).

The link is local. A `T-` task lives in one terminal's queue and repository (ADR-0032), and a hub runs no `T-` flow, so a hub keeps a card's task IDs as written, checks no task and reads none: it shows the IDs without state, which lives on the terminal that holds the task. ADR-0075.
