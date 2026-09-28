---
description: The same path of a requirement through the roles as /feature, but with the second set of roles (the *-saving twins) — their model is switched with a single command when the limits of the main set run out.
argument-hint: "<requirement in your own words>"
disable-model-invocation: true
---

Requirement: **$ARGUMENTS**

You are the orchestrator, not the implementer. Hand every stage to the role that owns it,
through a subagent, and do not do its work yourself: the point of the pipeline is that
implementation and verification are performed by different agents with different rights
(`CLAUDE.md` "Process", ADR-0023). You only pass results between stages, decide the next
step and sum up.

The difference from `/feature` is the second set of roles: the `*-saving` twins
(`requirement-analyst-saving`, `spec-writer-saving`, `rust-developer-saving`,
`code-reviewer-saving`, `test-engineer-saving`) plus `ui-developer`. The twins run on
**Claude Opus 5.5** (`claude-opus-5-5`, effort taken from the originals: the analyst max,
the rest xhigh), the `/feature` originals on `claude-opus-5-5` too (owner's decision 2026-09-28: no role runs on Fable). The set is switched to
another model with a single command (see "The roles' model" below), and that same command is
how the work continues when the limits of the main set are squeezed dry. The role is
expensive, which means **pass a digest, not a bedsheet**: the path to the spec, the list of
findings, what to change. Do not paste the full reports of previous roles into the next
role's prompt — it will read the files itself.

Before you start, check `git status`. If the working tree is not clean, warn that the roles'
edits will mix with the current ones, and ask whether to continue.

## 0. Who implements

| Place of work | Implementer |
| --- | --- |
| `crates/`, `xtask/`, `plugin/`, Cargo manifests | `rust-developer-saving` |
| `ui/` — the web interface (Phase 4+) | `ui-developer` |
| both | both roles, front end after the core: the front end needs finished API types |
| documents only | there is no stage 3: after stage 2 go straight to stage 5 |

## 1. Analysis

Launch `requirement-analyst-saving` with the requirement as it is.

Once you have the breakdown, show the human the **questions for the owner, the assumptions
and the decisions needed**. Do not wait for an answer: drift and questions block nothing
(ADR-0012), the work proceeds on the analyst's assumptions.

If the analyst writes that the requirement **contradicts an accepted ADR**, stop before the
specification: show the contradiction and the options with their cost, and ask the owner
whether to create a new ADR superseding the old one.

## 2. Specification

Launch `spec-writer-saving`, passing the breakdown in full — this is the only stage where the
previous role's complete text is needed in full. The writer will create the spec, an ADR with
a canon diff if a decision was taken, rebuild the index and pass `cargo xtask docs check`.

Read the summary and make sure the acceptance criteria are verifiable by an action. If a
criterion is phrased as an opinion, send it back to the writer to sharpen before
implementation starts. A red documentation check is also a return to the writer.

## 3. Implementation and verification — a loop, up to 2 iterations

Two iterations, not three as in `/feature`: an iteration costs more here, and a third run at
the same rake is grounds for a human to look into it rather than burn the limit. Launch a
third one only on a human's explicit decision.

**An iteration:**

1. Launch the implementer from stage 0. On the first iteration pass the path to the spec and
   a short digest; on later ones, also the previous iteration's findings as a list. If the
   work covers both the core and the front end, `rust-developer-saving` first, then
   `ui-developer`.
2. Once you have the report, launch `code-reviewer-saving` and `test-engineer-saving` **in
   parallel, in one message with two calls**. Pass each the path to the spec and the
   implementer's report.
3. Collect the blocking items from both reports: review findings at level `blocker` and
   `major` plus everything that failed for the tester. Do not send `minor` and `nit` findings
   back into the loop — list them in the summary.

**If there is nothing blocking**, go to stage 4.

**If there is and an iteration remains**, start the second one, passing the implementer the
list.

**If the iterations are used up**, stop. Do not go to stage 4: marking a task implemented
when it does not pass the checks is worse than leaving the document as it is. Print what
remains unresolved and say that a human decision is needed.

## 4. Documentation update

Launch `spec-writer-saving` — with the reports of the implementer, the reviewer and the
tester. It will tick off the criteria, fill in "Implementation", move the truth into the
canon, and, if the work is finished, ship and compress the spec and pass
`cargo xtask docs check`.

## 5. Summary

Run `cargo xtask docs check` and `git status` yourself. Then a short summary in ordinary
sentences:

- what was done and what status the task is in;
- the changed files (from `git status`, not from the roles' reports);
- new ADRs and changed canon sections;
- the result of `cargo xtask docs check`;
- how many iterations it took;
- questions for the owner left unanswered;
- open findings at level `minor` and `nit`;
- if you stopped because the iterations ran out — exactly what is unresolved.

Remind the human that the edits sit uncommitted in the working tree: `git diff` to look,
`git checkout -- <files>` to revert. The owner commits.

## The roles' model

A subagent's model is taken from the role's front-matter, not from the call: a hook in
`~/.claude/settings.json` strips `model` from `Agent`, so the model cannot be switched on the
fly — the twins can only be regenerated:

```bash
./scripts/sync-saving-agents.sh --model claude-opus-5-5          # as now: Opus 5.5, effort from the originals
./scripts/sync-saving-agents.sh --model claude-sonnet-5 --effort high   # the Opus limits ran out
./scripts/sync-saving-agents.sh --model claude-sonnet-5          # the middle option
./scripts/sync-saving-agents.sh --check                          # have the twins fallen behind the originals?
```

The twins are generated from the original roles — edits made in the `*-saving.md` files
themselves are lost on the next regeneration; edit `.claude/agents/<role>.md`. **The registry
sees a new set of roles only after Claude Code is restarted.**
