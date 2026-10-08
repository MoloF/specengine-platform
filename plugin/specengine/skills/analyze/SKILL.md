---
name: analyze
description: Analyse or plan a SpecEngine task before the owner approves it. Read its package and the spec it targets, raise each gap as a record with a working answer, submit a plan. Never claim the task or edit code.
---

# Analysing a task

The project's CLAUDE.md and its roles take precedence over this skill.

Use this when you are asked to analyse or plan a task. A task is the owner's unit of work in the SpecEngine queue: a goal, the spec nodes it targets, its criteria. The plan comes before the owner's approval; the work comes after it (the implement skill).

## Steps

1. **Read the task.** Call `get_task` with its `task_id`. The package holds the goal, targets, criteria, assumptions, the open questions bound to it, the owner's notes, the last plan, and whether the spec changed since approval (`stale`).
2. **Read its spec.** Call `get_context_bundle` with the task's targets in `node_ids`; narrow with `get_node`, `search` or `get_tree` only where the bundle is not enough (the read-spec skill).
3. **Raise each gap as a record.** Where the spec is silent or ambiguous, or the code disagrees with it, call `ask_question` or `report_discrepancy` with the task's `task_id` and your `working_answer`, and keep going on that answer (the ask-owner skill). Nothing waits for the owner's reply.
4. **Submit the plan.** Call `submit_plan` with the `task_id`, `plan_md` (Markdown: what changes where, in what order, and how it is checked), `criteria` (references or short texts) and `affected_nodes` (the other spec nodes the work touches). A new plan replaces the last one. A plan is taken only before the owner approves the task.
5. **Stop there.** The owner reads the plan and approves the task on a terminal, or asks for changes: their notes come back in the package; read them with `get_task` and submit again.

## Never

- Never call `claim_task` and never edit code while analysing: the task is not approved yet. Where the project turns the task gate on, a write to a controlled path is refused until an approved task is claimed in that worktree, so an analysing agent's write is refused by design.
- Never approve a task, or ask another agent to: approval is the owner's alone.
- Never edit spec files: a change to the spec text is a proposal (the propose-spec-change skill) or part of the plan.
