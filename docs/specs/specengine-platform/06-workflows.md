---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 06. Workflows

> How the owner and agents work through SpecEngine. The main scenario is §3 (discrepancy → agreement → development).

## 0. Participants and surfaces

| Who | Where | What they can do |
|---|---|---|
| **Owner** | Web UI, CLI in a terminal, elicitation forms in Claude Code | everything: edit specs, decide proposals, prepare and approve tasks |
| **Analyst agent** (`requirement-analyst`) | MCP | read; create proposals, questions and a draft plan |
| **Developer agent** (the project's implementer, e.g. `rust-developer`) | MCP + code | read; claim a `ready` task; report discrepancies; place `@implements` markers |
| **Tester / reviewer agent** | MCP + tests | read; place `@verifies`; report discrepancies |
| **CI / pre-commit** | CLI | `spec check`, `spec verify` |

Only `apply_proposal` writes to spec files, on an owner's action, or the owner directly in the UI/editor (an owner's edit is also a proposal, just applied instantly).

## 1. Describe business logic top-down

```
Project (project.md: vision, goals, constraints)
 └─ Domain (README): "Movement", "Network", "Persistence", "Economy"…
     └─ Mechanic: "Stamina", "Inventory", "Trading"…
         ├─ Rule / Invariant / Edge case  {#ID}
         ├─ Data contract (which config fields tune it)
         └─ links: derived_from R-…, depends_on MEC-…, uses_term TERM-…
```

- **UI → Tree**: create a node, drag it (change `parent`), write a rule. Rules get the EARS template: "WHEN \<condition\> the system SHALL \<behaviour\>". A recommendation, not a requirement (ADR-0007).
- **UI → Graph**: dependencies and impact. Select a node → "what an edit affects": nodes via `depends_on`/`constrains`, bound symbols, tests, open tasks.
- **CLI**: `spec new mechanic --parent DOM-MOVEMENT "Stamina"` → a file with the issued ID opens in `$EDITOR`; `spec tree DOM-MOVEMENT`; `spec graph MEC-STAMINA --impact`.
- Editing the file directly in an IDE is legitimate too: git is the truth. The daemon sees the change, recomputes hashes and shows drift.

**The first tree** is built by the project's importer (`spec init --import`, see `08-roadmap.md` §4) from documents, glossary and records; domain specifics live in `specengine.toml` and the importer (ADR-0008).

## 2. Prepare a task

```
 owner               SpecEngine                         analyst (agent)
    │ spec task new       │                                    │
    │  --nodes MEC-X ───► │ T-0107: draft                      │
    │                     │ ───── /specengine:analyze T-0107 ─►│
    │                     │ ◄─ get_task, get_context_bundle ───│  bundle ≤ 10k tok.
    │                     │ ◄─ find_symbols / code reading ────│  discovery (bindings known → fast)
    │                     │ ◄─ report_discrepancy / ask_question / propose_* ─┤
    │                     │ ◄─ submit_plan(T-0107, plan, criteria) ──────────┤
    │                     │ T-0107: review (waits for owner; open PRs visible)│
    │ ◄── notification ───│                                    │
    │ spec task show T-0107   — brief, plan, criteria, proposals
    │ approve | changes_requested "say what to do with bots" | cancel
    │                     │ T-0107: ready (spec_snapshot is fixed)
```

- **"Send back for more detail"**: `changes_requested` with a comment. The task returns to the analyst, who sees the comment in `get_task`, extends the plan or asks questions, and calls `submit_plan` again. As many cycles as the owner needs.
- SpecEngine returns the task as a stack-neutral package with a neutral brief built from nodes and the plan: goal, in/out of scope, nodes, criteria, risks, affected modules (via bindings; 07 §1.2, ADR-0027). The project's skill renders it for its stack or tracker, one way: approval stays here (§3.5).
- **Task contour** (`--contour`): `content | feature | feature → content`. For `content` no code changes, only data, and the gate is softer.
- **Open proposals do not hold a task** (ADR-0012). They are visible on its card (`spec task show`), and the owner decides: approve now, first handle some proposals, or send back for more detail. Any number of tasks and documents can run in parallel.

## 3. Spec/code discrepancy → agreement → development  ⟵ main scenario

### 3.1. Who finds a discrepancy and when

| Where | Example |
|---|---|
| Analyst during preparation | "spec: regeneration after a 1.5 s delay; code: no delay" |
| Developer during implementation | "spec requires `Exhausted` at 0, but `MessageWriter` has no such message, and R-12 says otherwise" |
| Reviewer / tester | "criterion AC-07 is unsatisfiable: contradicts AC-04" |
| `spec verify` (machine) | `code_ahead`: the symbol changed after the last verification |

### 3.2. What the agent sends

MCP `report_discrepancy` (or `ask_question`): the nodes, a summary, gap type, severity, evidence `[{file, qpath, lines, observed, documented}]`, priced options `[{label, effect, price}]` with a recommendation, optionally a `proposed_patch` for "spec to code". Arguments, validation and deduplication (an accepted decision on the nodes: "already decided", nothing created): `docs/features/agent-intake.md`. To come: the informational flag `has_open_proposal` on the nodes (**nothing is blocked**, ADR-0012: the node stays open for reading, editing and other proposals, the task keeps its status); an SSE event, optionally an OS notification.

The agent receives `PR-0042` and **keeps working** on the current spec or on the working answer it named in the proposal. Next to the code it places `// @assumes PR-0042`, so after the decision it is visible what to revisit. The agent must not wait for approval. It may stop only by its own judgement, if the task loses meaning without an answer, and then says so in `report_run`.

### 3.3. How the owner decides

One queue in three places:

**CLI** (shipped for `update`: `docs/canon/proposal-queue.md`): `spec inbox` lists `<id> | <kind> | <status> | <target_id> | <branch> | <created_at> | <rationale>` by ID, no labels or relative ages (ADR-0012, determinism); `spec review PR` prints the diff, introduced findings and `applies|rebases|conflicts`; `spec approve`, `spec reject --reason` decide on a terminal, `[y/N]`. To come: evidence, priced options, edit and accept, clarification, defer.

**Web UI → Queue**: a card with evidence (code highlighted), the node's current text, options with price, the diff (`@codemirror/merge`), editable in the card before accepting.

**Inside a Claude Code session** (fast path): the `review_proposal` tool is marked `_meta["anthropic/requiresUserInteraction"]: true` and via MRTR/elicitation shows a form (option, comment) or URL mode with a link to the card in the UI. The model cannot answer for the human, and allow rules and hooks cannot bypass this.

### 3.4. What happens after the decision

| Decision | Effect |
|---|---|
| **Accept "spec to code"** / edit and accept | `apply_proposal` (`docs/canon/proposal-apply.md`): the edit lands in the worktree it was raised in (ADR-0032; a task's: its worktree), rebased if the node changed meanwhile, always committed `spec: apply PR-0042` (ADR-0005); to come: a decision record (`answers: PR-0042`, `cost`, `canon:`), `spec_hash` recomputed, the task's `spec_snapshot` updated (its own edit) |
| **Accept "code to spec"** | spec unchanged; an item is added to the task plan; a decision record says the discrepancy is a code defect |
| **Reject** | closed with a reason (`spec reject`); to come: the reason indexed, an agent's repeated attempt gets it back |
| **Needs clarification** | `changes_requested` with the owner's question → the agent sees it in `get_proposal`/`get_task` and extends |
| **Defer** | the proposal stays in the queue with a mark; work continues on the working answer |

**Reconciling with the working answer.** If the decision matches the working answer, the `@assumes PR-0042` markers are removed with one command. If not, SpecEngine finds every `@assumes PR-0042` (code, tests, data) and creates a follow-up task, or reopens the original one if it is not yet accepted. Either way it is visible in the queue.

On the next `get_task`/`get_context_bundle` the agent gets a fresh bundle headed "changed since last time: RULE-STAM-REGEN (diff)".

### 3.5. The single control point — task approval (ADR-0012)

Discrepancies, questions and proposals **block nothing**. The guarantee "nothing goes into final development without me" rests on one thing: **the developer takes only a task the owner approved**.

1. **In SpecEngine**: `claim_task` returns only `ready`. `ready` is set by the owner (`spec task approve`), and open proposals do not prevent it. If a task node changed after approval, the task is marked `stale`: an informational flag, the agent gets the diff in the bundle.
2. **Claude Code `PreToolUse` hook** on `Edit|Write|MultiEdit` (and `Bash` with `if:` on writes) for paths in `zones.code`: an edit is allowed if this worktree has a claimed task in `ready`/`in_progress`. The hook is a `command` (`specengine gate`, 07 §4), not `http`, which lets the edit through when the service is down (04 §4). If the daemon is unavailable — **exit 2** (closed) (ADR-0006).
3. **Selectivity**: the hook is on for tasks of the `feature` contour. Bugfixes and small edits go without it, but `spec verify` shows the drift. `gate.mode = "observe"` turns off even this check: everything is computed and shown, nothing stops.

## 4. The machine found drift

```
spec verify (pre-push / CI / daemon on FileChanged)
  → RULE-STAM-REGEN: code_ahead (ast_hash changed, spec did not) — commit 8263ce2
  → to the owner's queue: "Code changed after verification"
       [accept code changes]   → lock updated (editorial for code)
       [this changes the rule] → task to an agent: propose a spec edit
       [reject]                → task: restore behaviour (spec_ahead)
```

`conflict` (both spec and code changed) opens in the diff inspector: spec `git diff <lock.commit>..HEAD` by section, code by the symbol's range.

## 5. Question round for the owner

SpecEngine collects questions into a sheet and parses the answers.

1. `spec round new` gathers open questions into **one sheet in domain language**. Order: by `severity`, then by the number of tasks and `@assumes` depending on the answer. Each question has text, what it unblocks, the working answer and the price of a different answer, plus a reading-time estimate. Export to markdown or a UI page.
2. The answer comes in free form by the protocol "Q-12: yes", "Q-13: option b". `spec round answer answers.txt` parses the lines into **proposals** per question: close Q, create DEC/R/A, cancel a working assumption. Answers like "did not understand the question" automatically send it back for rephrasing.
3. The owner confirms the parse in batch. Same as §3.3, grouped by round.
4. **Interpretations**: if a decision became a rule only after interpretation, the proposal gets kind `interpretation` and separately shows "how the decision was applied and why interpretation was needed".

## 6. Implementation, verification, acceptance

1. The developer places `// @implements <node>` at a new or changed symbol, the tester places `// @verifies <AC|node>` at tests.
2. `spec verify --task T-0107 --tests` runs the bound tests (nextest filter), writes the result to `runs`, on success updates `spec.lock` and `impl_status → verified`. **The machine sets the status, not the agent's self-report.**
3. `complete_task` → `done`. Manual criteria (`kind: manual`) stay open and land in the "awaiting acceptance" panel.
4. `spec ship <feature>`: the feature spec is compressed to intent and criteria, the rest goes to the archive; `--accepted` sets `acceptance: accepted`.

## 7. Corpus health and compaction

- **"What is left" panel**: `severity: high` proposals → old open ones → unapplied amendments → `@assumes` with a decision different from the working answer → `unbound` accepted nodes → drift → documents over budget.
- **Metrics**: open proposals by `severity`, age of open ones, share of decisions diverging from the working answer, decisions without a price, **task W — median and p90 from bundle logs**, number of "third steps" of further reading.
- **Compaction**: `spec compact --dry-run` proposes absorbing settled decisions into canon, closing superseded ones, compressing shipped specs. The result is a batch of proposals for agreement, not an auto-edit.

## 8. Integration with the role pipeline

SpecEngine plugs into the consumer project's role pipeline (analyst, spec writer, developer, tester, reviewer) without changing the order of stages.

| Stage | Through SpecEngine |
|---|---|
| **Analyst** | via MCP: bundle → discovery via bindings → **discrepancies and questions as records** (`report_discrepancy`, `ask_question`, `propose_*`), not prose → `submit_plan` |
| **Task approval by the owner** | `approve` \| `changes --note` \| `cancel`; open proposals are visible on the card but do not hold the task (§3.5) |
| **Spec** | approved node edits are applied by `apply_proposal`; the spec writer writes only the feature spec and criterion markers |
| **Implementation ⇄ review + tests** | `claim_task` takes only `ready`; `@implements`/`@verifies` markers; a discrepancy is `report_discrepancy` + `@assumes`, work continues; implementation roles do not touch `docs/`, so the discrepancy stays visible |
| **Update** | `spec verify --tests` sets `verified` (machine, not self-report); the spec writer fills in "Implementation"; `complete_task` → `done`, the run is recorded in `runs` |

**Three layers** (ADR-0027). SpecEngine's plugin: hooks, prompts, stack-neutral roles (analyst; spec writer, since the ADR-0022 convention is universal; a reviewer for conformance to the spec; a generic implementer and tester taking commands from the project's `CLAUDE.md`) and `/feature` in the stage order above. A stack profile, a plugin hosted outside SpecEngine's repository: implementer and test roles, build/test/lint commands, where tests go, a brief-rendering skill, suggested `[zones]` as text the owner copies. The project: routing and its own rules. Precedence project → profile → generic, chosen by distinct role names and explicit routing in the project layer, not by name shadowing: Claude Code's agent name resolution across project and plugins is unverified (check it on the pinned version in Phase 2). This repository is such a project: under ADR-0023 its `.claude/` is its layer.
