---
name: implement
description: Implement a SpecEngine task the owner approved. Claim it in the worktree you edit, work there on its branch, report the run and complete it. A refused write is reported to the owner, never worked around.
---

# Implementing a task

The project's CLAUDE.md and its roles take precedence over this skill.

Use this when you are asked to implement a task. The owner's approval is the one control point: where the project turns the task gate on, a write to a controlled path passes only while a task in progress is claimed in that file's worktree, on its branch.

## Steps

1. **Read the task.** Call `get_task` with its `task_id`. Go on only when it is approved and ready, or already claimed in this worktree for your work; anything else is the owner's to decide. Read the goal, targets, criteria, plan and owner notes; where the spec changed since approval (`stale`), read the current nodes with `get_context_bundle` first. Before the first edit, call `get_impact` once per target, its `id` (else its `path`) as `id`, or with the `symbol` you are about to change: it lists the nodes the change reaches, the code bound to them with each binding's `sync`, the tests to run and the open tasks on them; name those tasks in your report.
2. **Claim it once.** Before the first edit, call `claim_task` with the `task_id`, your `role` as the project names it (its CLAUDE.md or roles) and, as `worktree`, the top of the git worktree you edit. A task claimed elsewhere is worked on there, not here.
3. **Work only there.** Edit only inside that worktree, on the branch it was claimed on; do not switch branches. A subagent editing in another worktree is refused: keep the edits here.
4. **Keep gaps as records.** A question or a discrepancy found while working goes through `ask_question` or `report_discrepancy` with the `task_id`; keep working on your `working_answer` (the ask-owner skill). A change to the spec text goes through `propose_change` where the project routes spec edits through the queue.
5. **Check the bindings.** Before the report, call `check_binding` once per target, its `id` (else its `path`) as `node`; it judges this session's worktree. Mend what your change broke: a binding that no longer reaches its code, spec and code that both changed, a marker naming a revision its node has not reached, or one it cannot verify; else raise it with `report_discrepancy` and keep working on your `working_answer`. The owner settles code that moved ahead of its spec, a spec ahead of its code and a binding not yet locked: name them in the report, never hide them. A node's `tested` (in `get_task`, `get_impact` and `check_binding`) is the owner's own test evidence: an edit to the node or its bound code can void it, so name each such node in the report for the owner to run its tests again, and never claim that evidence yourself. An error result with a report is a finding, not a failed call; where the project binds no code, the report says so: go on.
6. **Report the run.** First call `get_impact` once with `since` the commit your work started from: run the tests it lists and name its open tasks. Then call `report_run` with the `task_id`, its `outcome`, a `summary` of what the run did and the `changed_files`. The task stays in progress after the report, and its writes still pass.
7. **Complete it.** Call `complete_task` with the `task_id` once the work is done and reported. Its writes are gated again: further work needs another approved task.

## A refused write

A refusal names the path, its zone, the mode and the next step; a "no verdict" names why the gate could not decide. Stop and tell the owner: quote the refusal and the step it names (claim a ready task in this worktree, or the owner's approval on a terminal). Never work around it: no write through the shell, no other worktree or branch, no edit of the project's config, no other file name for the same change.
