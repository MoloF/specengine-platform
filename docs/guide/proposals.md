---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-20
summarises: [docs/canon/architecture.md#apply, docs/canon/architecture.md#control, docs/canon/proposal-apply.md#consent, docs/canon/proposal-queue.md#commands, docs/canon/agent-intake.md#tools, docs/canon/decision-staging.md#rule, docs/canon/hub-confirm.md#rules]
---

# Proposals and the one writing door

Every change an agent or a tool makes to a spec file goes through one door: someone proposes it, a person confirms it, and SpecEngine applies it as one commit that records who proposed and who decided. This page covers proposing, reviewing, approving and confirming, and why nothing else writes a spec file.

## What it solves

Agents write fast and sometimes wrong, and a spec that changes without anyone deciding stops being a spec. The queue lets anyone, agent or person, propose a change or ask a question at once, while a person decides each item on its own. Nothing waits on the decision: work goes on under a working answer, and the commit's trailers keep the provenance.

## When to use it

- **Changing a node**: an agent, or a person who wants a review, proposes the new text of a section or a file.
- **Adding a node**: a new record, a new feature spec, new `{#ID}` sections.
- **Asking or reporting**: an open question, a discrepancy between code and spec, a decision taken during work.
- **Deciding**: the owner, or a lead inside their zones on a hub, reviews and confirms.

## How

### CLI

Read the node's current `span_hash`, then propose against it:

```text
$ spec show R-012
...
$ spec propose update R-012 --base b3:9f2c... --text-file r012.md --rationale "Delay is 1.5 s, not 1 s"
PR-0007
introduced: 0
```

The kinds are `update`, `create`, `interpretation` and `amendment`, which apply text, and `question`, `discrepancy` and `decision`, which are answered by a decision record (`guide:features#GD-FEATURES-PROMOTION`). `introduced` counts the check findings the change would add; they are shown, never refused.

The owner decides on a terminal:

```text
$ spec inbox
PR-0007 | update | open | R-012 | main | 2026-11-20T09:12:03Z | Delay is 1.5 s, not 1 s
$ spec review PR-0007
$ spec approve PR-0007
apply PR-0007 to docs/records/R/R-012.md on main in /work/lamp-game (applies)? [y/N] y
applied PR-0007 as 3f2a1c9 on main
$ spec reject PR-0008 --reason "Duplicate of PR-0007"
rejected PR-0008
```

`spec approve --open` walks every open item, each with its own question. `spec round new` prints the open questions on one sheet; `spec round answer` stages the answers and asks each item's own `[y/N]`.

### MCP

`propose_change {kind, target, base, text, rationale}` for text; `ask_question`, `report_discrepancy` and `report_decision`, each with a `working_answer`; `report_drift` for code drift; `get_proposal` to read one. When a similar item exists, the tool lists it and stores the new one only if `distinct_from` names each hit. On a hub each item is stored under your person's login.

### Screen

Inbox lists open items. Accept and Reject open a dialog that stages your choice. On a local daemon the staged choice is confirmed on the terminal (`spec approve PR-0007`), shown there as staged. On a hub, a `[team]` lead or owner presses Confirm: one click, one commit in the hub's checkout under their login (`guide:hub#GD-HUB-CONFIRM-ONE-CLICK`).

## Where it lives

A proposal is a row in the project's queue, in SpecEngine's data directory, never a file in the repository. It records the worktree, branch and commit it was raised on. Approving writes the target file, plus the index's changed outputs, and makes one commit:

```text
spec: apply PR-0007

Delay is 1.5 s, not 1 s

Proposal: PR-0007
Decided-by: Ann Owner <ann@example.org>
Proposed-by: agent role=implementer model=unknown run=unknown
Base-commit: <the commit it was raised on>
```

### One item, one yes {#GD-PROPOSALS-ONE-YES}

Each proposal is confirmed on its own: a terminal's `[y/N]`, or one click on a hub. There is no `--yes` and no confirm over a set. `spec approve` refuses to run when its input is not a terminal, so no script or agent shell can answer for the owner.

### Applied where it was raised {#GD-PROPOSALS-WHERE-RAISED}

An approval applies in the worktree and on the branch the proposal was raised on, so the spec change travels with the code of the same work. Another branch, a detached `HEAD` or a missing worktree refuses it. Decide open proposals before moving a project to another place: their paths are never rewritten.

### Propose against the text you read {#GD-PROPOSALS-BASE-HASH}

`--base` (MCP `base`) is the `span_hash` of the text you changed. If the node changed since, the proposal is refused with the current hash: read it again and redo the change. At approval, a proposal whose base moved is rebased onto the new text when it can be, and the question says `rebases`.

### Nothing waits on a decision {#GD-PROPOSALS-NOTHING-BLOCKS}

A question, a discrepancy or an open proposal blocks no task, no card and no commit. The agent keeps working on its working answer and marks the code that relies on it, `// @assumes PR-0009`. If the decision differs from the working answer, a follow-up task brings the work in line.

## What not to do

### Never write a spec file by hand from an agent {#GD-PROPOSALS-ONE-DOOR}

An agent changes a spec file only by proposing: `propose_change` over MCP, or `spec propose` in a shell. It never edits the file with its own tools, even when the change looks trivial, because a person must see and confirm it. The plugin's skills and roles always propose; say the same in your project's `CLAUDE.md`. A person may still commit a hand edit; `spec check` judges it like any other commit.

### Never let an agent choose for the owner {#GD-PROPOSALS-NO-AGENT-CHOICE}

No tool approves, rejects or stages an owner's choice. `review_proposal` only records the answer the owner gave in its form, when the owner asks. Do not pipe input into `spec approve`, script a browser on a hub, or give an agent a person's session: the commit would claim a decision nobody made.

### Never raise a duplicate {#GD-PROPOSALS-NO-DUPLICATE}

Search before asking: `search`, or `get_node` with links. When the tool lists similar items, either name each in `distinct_from` with a reason it is different, or work from the existing one. Two open questions on one rule split the owner's answer.
