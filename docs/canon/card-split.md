---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-11
---

# Card split: a task and a subtask per department, in one act

ADR-0078: a piece of work goes to every department it needs in one act. The card: `cards.md`; a hand-off and the new-card rule: `card-flows.md`; departments and their disciplines: `workspaces.md`; a parent's progress: `card-progress.md`.

## State

The rule below holds; nothing of it is built. v5 step 6c `department-split` (RM-0100, `docs/features/department-split.md`) builds it: `[[card.split]]`, the plan, `spec card split` and its `--dry-run`, `split_card`, `POST .../cards/split`, the Split dialog, the preset's `sound` flow and department, plugin 0.37.0. Until it ships, `[card]` holds `flow` alone, and a split is a New task, then a Hand off per department, one write each.

<a id="the-rule"></a>
## The rule

A split hands one piece of work to several departments at once. It makes a parent task on a document, or takes a stored task as the parent, and one subtask per chosen discipline: each a hand-off (`card-flows.md` "The rule"), its `parent` the parent, its `reviewer` the parent's, its flow and first column the new-card rule's for its discipline. It is one write: one transaction, all cards or none, one `card.created` each, the parent first. It writes no stored card: a parent that exists keeps its row, revision and history. So no other card write creates, moves or refuses another, and a split makes no card wait on another (ADR-0074).

A `[[card.split]]` row under `[card]` names a node kind and the disciplines its work usually needs. Rows only pre-select: the disciplines of the rows whose kind a node of the parent's targets, or a node under them, holds; and a subtask's targets, the nodes of its rows' kinds found there, else the parent's targets. A row never refuses, requires or orders a split, and no move, accept, rollup, gate or check reads one (ADR-0012). Kinds and disciplines are the project's words; the core names none (ADR-0008). ADR-0078.
