---
description: Runs a SpecEngine requirement through the roles — analysis, specification (with an ADR and a canon diff if a decision was taken), implementation, review and tests, documentation update — with a rework loop on blocking findings and a spec check.
argument-hint: "<requirement in your own words>"
disable-model-invocation: true
---

Requirement: **$ARGUMENTS**

You are the orchestrator, not the implementer. Hand every stage to the role that owns it,
through a subagent, and do not do its work yourself: the point of the pipeline is that
implementation and verification are performed by different agents with different rights
(`CLAUDE.md` "Process", ADR-0023). You only pass results between stages, decide the next
step and sum up.

Before you start, check `git status`. If the working tree is not clean, warn that the roles'
edits will mix with the current ones, and ask whether to continue.

## 0. Who implements

Decided before the analysis, by where the code will live:

| Place of work | Implementer |
| --- | --- |
| `crates/`, `plugin/`, Cargo manifests, `.githooks/`, `.github/workflows/`, `scripts/`, `.cargo/` | `rust-developer` |
| `ui/` — the web interface (Phase 4+) | `ui-developer` |
| both | both roles, front end after the core: the front end needs finished API types |
| documents only (canon, a decision, a spec without code) | there is no stage 3: after stage 2 go straight to stage 5 |

The analysis, specification, review and test roles are the same in every case.

## 1. Analysis

Launch `requirement-analyst` with the requirement as it is.

Once you have the breakdown, show the human the **questions for the owner, the assumptions
and the decisions needed** — this is the only place where his opinion changes everything
that follows. Do not wait for an answer: drift and questions block nothing (ADR-0012), the
work proceeds on the analyst's assumptions, and the questions exist so that the owner can
step in before it is too late.

If the analyst writes that the requirement **contradicts an accepted ADR**, stop before the
specification: changing an accepted decision is the owner's decision, not a role's. Show the
contradiction and the options with their cost, and ask whether to create a new ADR
superseding the old one.

## 2. Specification

Launch `spec-writer`, passing the breakdown in full. It will create or extend
`docs/features/<slug>.md` from the template. If a decision was taken in the work, also an
ADR with a canon diff in the same change. At the end it will rebuild the index and pass
`cargo run -q -p specengine-cli -- check`.

Read the summary and make sure the acceptance criteria are verifiable by an action. If a
criterion is phrased as an opinion ("works well"), send it back to the writer to sharpen
before implementation starts: the work will be accepted against these criteria. A red
documentation check is also a return to the writer.

## 3. Implementation and verification — a loop, up to 3 iterations

**An iteration:**

1. Launch the implementer from stage 0. On the first iteration pass the path to the spec and
   a short digest; on later ones, also the previous iteration's findings as a list. If the
   work covers both the core and the front end, `rust-developer` first, then `ui-developer`.
2. Once you have the report, launch `code-reviewer` and `test-engineer` **in parallel, in
   one message with two calls** — they are independent, and launching them in sequence
   doubles the wait. Pass each the path to the spec and the implementer's report.
3. Collect the blocking items from both reports: review findings at level `blocker` and
   `major` plus everything that failed for the tester. Do not send `minor` and `nit`
   findings back into the loop — list them in the summary and let the human decide.

**If there is nothing blocking**, go to stage 4.

**If there is and iterations remain**, start the next one, passing the implementer the list.

**If the iterations are used up**, stop. Do not go to stage 4: marking a task implemented
when it does not pass the checks is worse than leaving the document as it is. Print what
remains unresolved and say that a human decision is needed.

## 4. Documentation update

Launch `spec-writer` once more — with the reports of the implementer, the reviewer and the
tester. It will tick off the criteria that are met and fill in "Implementation". Then it
will move the truth into the canon: the Tier 1 README files of the crates and the sections
of `docs/canon/architecture.md` if a rule changed. If the work is finished, it will ship the
spec (`status: shipped`, compression to ≤ 3 KB) and pass `cargo run -q -p specengine-cli -- check` again.

## 5. Summary

Run `cargo run -q -p specengine-cli -- check` and `git status` yourself. Then a short summary in ordinary
sentences:

- what was done and what status the task is in;
- the changed files (take them from `git status`, not from the roles' reports — they can
  diverge);
- new ADRs and changed canon sections;
- the result of `cargo run -q -p specengine-cli -- check` (its summary, with the worst W);
- how many iterations it took;
- questions for the owner left unanswered;
- open findings at level `minor` and `nit`;
- if you stopped because the iterations ran out — exactly what is unresolved.

Remind the human that the edits sit uncommitted in the working tree: `git diff` to look,
`git checkout -- <files>` to revert. The owner commits.
