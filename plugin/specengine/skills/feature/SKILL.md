---
name: feature
description: Run a SpecEngine task through the project's stages, from a requirement in words to a plan awaiting approval, or from an approved task to done.
argument-hint: <requirement in words> or <task ID>
disable-model-invocation: true
---

# Running a feature

The project's CLAUDE.md and its roles take precedence over this skill.

The argument: $ARGUMENTS

You are the session that runs the task: hand each stage to its role through the Agent tool, pass the results along, sum up; never do a role's work. The owner's approval of the task is the one control point. A task ID goes to "A task ID" below, anything else is a requirement in words; no argument: ask for one, and end.

## Routing

The stages are `analysis`, `spec`, `implementation`, `review` and `tests`. A `Stage | Role` table in the project's `CLAUDE.md` names a stage's role as it is called (the project's own agent, or a profile's). A stage it does not name, or no table: the generic role, `specengine:analyst`, `specengine:spec-author`, `specengine:implementer`, `specengine:reviewer`, `specengine:tester`, in stage order. Pass each role the task ID and what its stage needs, and tell it it runs under /specengine:feature: the session holds the claim and reports the run.

## A requirement in words

1. The analysis role finds the targets (none: the nearest existing parent document), a title and a goal.
2. Run `spec task new --nodes <targets> --title <title> --goal <goal> --author-role <the analysis role as called>`; it prints the task ID. With no `spec` on `PATH`, say so, print that command for the owner, and end.
3. The analysis role reads the task with `get_task`, raises each gap with the `task_id` and a working answer (`ask_question`, `report_discrepancy`), and calls `submit_plan`.
4. Print the owner's step: read the plan, then on a terminal `spec task approve <task ID>` (or `spec task changes <task ID> --note <text>`), then run `/specengine:feature <task ID>`.

This leg ends here: never wait for the approval.

## A task ID

Read it with `get_task` and go by its status:

- draft, or changes requested: steps three and four above;
- under review: print the owner's step above, and end;
- in progress but claimed in another worktree or with its run already reported, done, or cancelled: say which, give the owner's options, and end;
- ready: call `claim_task` once, with the `task_id`, as `role` the implementation role as called, as `worktree` the top of this worktree (refused: tell the owner its reason, and end); in progress, claimed here with its run open: no claim. Then the steps below; skip a spec or test stage the approved plan does not need, never review.

1. The spec role: the feature's spec and the nodes the task edits, as proposals bound to the `task_id`.
2. The implementation role works in this worktree on the proposed text: the proposals wait for the owner, the work does not. Then the review and test roles in one message, side by side; the tests of that round are outside the review.
3. A blocking finding or a failed test goes back to the implementation role as a list, then review and tests again: at most the rounds the project's `CLAUDE.md` sets, else three.
4. Accepted and green: the spec role again, with the reports, for what was implemented and the rules that changed, as proposals.
5. Call `report_run` with the `task_id`, its outcome, a summary and the changed files; then `complete_task`, last.

Rounds used up with findings open: `report_run` with an outcome other than completed, the open findings in its summary, no `complete_task`; tell the owner what is unresolved.

## Never

- Never wait for the owner's decision or re-read the task until it changes; never approve a task or a proposal, nor ask a role to.
- Never commit, and never write a spec file or the lock: spec changes are proposals the owner approves.
- Never work in another worktree or send a role to one.
- A package marked stale is reported, never a stop. A refused write goes to the owner with the step it names (`specengine:implement`).

## Summary

What was done and the task's status; the changed files as the worktree shows them, not the roles' reports; the proposals and records raised, by ID; the rounds used; open findings; the owner's next step. The edits stay uncommitted: the owner commits.
