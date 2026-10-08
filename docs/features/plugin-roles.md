---
class: spec
status: draft
scope: [plugin]
ref: plugin-roles analysis 2026-10-08 (HEAD b7009de); Claude Code docs read 2026-10-08, build 2.1.294; the orchestrator's Q1-Q3 per the owner's pre-approval; 08 s2 Phase 2, Plugin
---

# Plugin roles

## Why

**Task.** Role pipelines live per project, copied by hand, and touch no queue (this repository's `/feature` drafts no task, its spec writer writes `docs/`; pilot A copies its role names), so the Phase 2 live check (08 s2) has nothing to run on and P2-12 (07 s1.2) stands only over MCP. Outcome: in any SpecEngine project one owner-invoked skill takes a requirement in words through analysis, approval on a terminal, implementation, review and tests to `done`, spec changes as proposals. Prompts and tests, no crate source. ADR-0015, ADR-0027 ("Cost": generic roles take commands from the project's `CLAUDE.md`), `docs/canon/architecture.md#distribution`, `#tasks`, `#apply`; no new ADR.

**Contradiction, resolved per Q1.** 06 s8 let the spec writer write "the feature spec and criterion markers" and "Implementation", against `#apply`, 05 s1 principle 3 and 06 s0. This draft restates 06 s8 and adds the spec author to 06 s0: SpecEngine's roles write no spec file; a project's own spec role is its layer's choice.

## Description and interactions

**Roles** `plugin/specengine/agents/<name>.md`, always called `specengine:<name>` (WA-3). `reviewer`, not `spec-reviewer`: it reviews code against the spec, and `specengine:reviewer` coexists with a project `reviewer` (D2).

| Role | Stage | Does | `disallowedTools` |
|---|---|---|---|
| `analyst` | analysis | a requirement's targets; on a task: package, bundle, gaps as records with working answers, `submit_plan`; never claims, edits, approves | Write, Edit, MultiEdit, NotebookEdit, Bash |
| `spec-author` | spec, update | the project's documentation convention in brief (`spec check` decides); each spec change a proposal with `task_id`; may run `spec check`, the registered generators | the first four |
| `implementer` | implementation | the package in the claimed worktree; commands, path areas from the project's `CLAUDE.md`; markers where its languages take them | none |
| `tester` | tests | tests as the project's `CLAUDE.md` says, `@verifies`; results its own claim | none |
| `reviewer` | review | the change (Bash: diffs, checks) against the package and the project's rules; findings or accept | the first four |

Each role stands alone, names the skills it loads with the Skill tool (`specengine:read-spec`, `ask-owner`, `propose-spec-change`, `analyze`, `implement`; WA-10, D6), raises gaps with a working answer and `task_id`, writes in the project's language (its `CLAUDE.md`, else `[project] language`; WA-9), never commits.

**`/specengine:feature`** is `skills/feature/SKILL.md`, `disable-model-invocation: true`, not `commands/feature.md` (WA-7): skills supersede commands for new work, and a plugin skill's bare alias yields to a command holding the name, so a project's own `/feature` keeps it (D3). Cost: one owner-invoked skill, two more keys, its description outside the in-context sum (D5).

**Routing** (WA-4): per stage (`analysis`, `spec`, `implementation`, `review`, `tests`) the role a `Stage | Role` table in the project's `CLAUDE.md` names (its own agent or a profile's, as called), else `specengine:<role>`. Precedence project -> profile -> generic lives there, not in name shadowing; no `specengine.toml` key (`#tasks`). No table (pilot A today): all generic. This repository keeps its own `/feature`.

**Leg 1**, a requirement in words (WA-5):

1. the analysis role finds the targets (none: the nearest existing parent document), a title, a goal;
2. the session runs `spec task new --nodes ... --title ... --goal ... --author-role <that role>` (no MCP tool; `spec` off `PATH`: the hint and the owner's command, end);
3. the analysis role: `get_task`, gaps with `task_id`, `submit_plan`;
4. the session prints the owner's step, `spec task approve <T>` (or `changes`) on a terminal, then `/specengine:feature <T>`, and ends.

**Leg 2**, a task ID, by `get_task`'s status:

- `draft`, `changes_requested`: leg 1 steps 3-4; `review`: the owner's step, end;
- `ready`: one `claim_task` by the session (`role` the implementation role as called, `worktree` its top); `in_progress` claimed here, run open: no claim; then the spec role (feature spec, node edits); implementation, review, tests until accepted and green, at most the project's `CLAUDE.md` bound, else three rounds (WA-8); the spec role ("Implementation", canon diff); `report_run`; `complete_task` last;
- rounds out: `report_run`, outcome not completed, open findings in the summary, no `complete_task`;
- claimed elsewhere, run closed, `done`, `cancelled`: say which and the owner's options; end.

Never: wait or poll for approval (04 s4, 07 s1.5), approve, commit, write a spec file, use another worktree. A `stale` package is reported, never blocks; a refused write goes to the owner (`specengine:implement`).

**Canon at shipping**: the root `README.md` (11 835 of 12 288 B) cannot take the role rules; a Tier 2 `docs/canon/plugin.md` (scope `[plugin]`) takes "Changing it", both tables and routing; the README keeps install and a pointer.

## Data

```yaml
---
name: analyst
description: Analyse a requirement or a SpecEngine task when /specengine:feature or the project routes to it. Find the targets, raise gaps with working answers, submit the plan; never claim, edit or approve.
model: inherit
disallowedTools: Write, Edit, MultiEdit, NotebookEdit, Bash
---
```

```yaml
---
name: feature
description: Run a SpecEngine task through the project's stages, from a requirement in words to a plan awaiting approval, or from an approved task to done.
argument-hint: <requirement in words> or <task ID>
disable-model-invocation: true
---
```

A project's `CLAUDE.md` routing, unnamed stages generic:

```markdown
| Stage | Role |
|---|---|
| spec | docs-keeper |
| implementation | stackprofile:implementer |
```

**Files**, sixteen: the README's ten, `specengine/agents/{analyst,spec-author,implementer,tester,reviewer}.md`, `specengine/skills/feature/SKILL.md`. `plugin.json` `0.3.0`; its, `metadata.description` and the entry's descriptions equal, naming the roles and `/specengine:feature`; `PINS` appended. `specengine.toml` `exclude` + `plugin/specengine/agents/*.md` (now).

## Rules and edge cases

**Agent front-matter** (WA-2): `name`, `description`, `model`, and on analyst, spec-author, reviewer `disallowedTools`, in order, one `key: value` line each, nothing else; `name` the stem, `^[a-z0-9]+(-[a-z0-9]+)*$`, no P2-3 role name; `description` one plain YAML line (no indicator first, no `: `, ` #`), 1-1 024 characters, the five <= 1 000 B together (ADR-0015 "Cost"), scoped to SpecEngine tasks; `model: inherit`, no `effort` (WA-6, Q3); body <= 4 096 B. Nothing else: `tools` drops MCP tools, four keys are ignored (D4), `isolation` moves the role to a worktree the gate refuses, `skills` naming is undocumented; unparseable front-matter loads with its fields dropped (D4), a broken `disallowedTools` an unrestricted role: parse strictly.

**Skill front-matter**: as "Data", in order, nothing else; the description outside the 1 200 B sum (D5), one plain line <= 1 024 characters; body <= 4 096 B. The five model-invoked skills keep two keys.

**Content** (the skills' rules, extended to agents): English; no P2-3 word, `mechanic`, `edge-case`, `mcp__`; outside `json` blocks no digit but a list's `N. `; no backticked enum value; every backticked snake_case name a tool or a `tools/list` schema property; no project fact, path, ID, command, cap or `[zones]` name; the round bound in words. Each agent holds "The project's CLAUDE.md and its roles take precedence over this role." verbatim; the skill the skills' sentence and "This leg ends here: never wait for the approval."

**Writes** (WA-1): front-matter and prompt only. Bash writes pass every role and the gate (07 s4 limit); prompts forbid shell writes to spec files; the gate stays role-blind; an agent's `spec task approve` exits 2. **Version** 0.3.0 MINOR; then a text PATCH, a role, skill or key MINOR.

**Claude Code docs, 2.1.294.** Documented: D1 plugin agents are `<plugin>:<name>` (typeahead, hooks; not `/agents`); D2 they rank last, a project `reviewer` and `specengine:reviewer` are two; D3 `commands/<f>.md` is `/<plugin>:<f>`, skills supersede commands, a plugin skill gets a bare alias unless a command holds it; D4 the agent keys, `permissionMode`, `hooks`, `mcpServers`, `initialPrompt` ignored, unknown keys silently, `tools` drops MCP tools; D5 `disable-model-invocation` keeps the description out of context; D6 plugin subagents see the MCP tools and load skills; D7 `validate --strict` checks skill, agent, command front-matter. Undocumented: what O-1 to O-3 record (bare `/feature` beside a project command, against "a skill beats a `.claude/commands/` file").

**Working answers**, marked inline (D1, D2, D4, D6 firm WA-2, WA-3, WA-10); also WA-11 the live check by `--plugin-dir`, no install; WA-12 markers where the project's languages take them, no comment syntax in plugin text; texts English (ADR-0024). Q2: one plugin (ADR-0015), per SpecEngine project, `--scope local`; the roles come with the gate hook, open without `[gate]`.

**Risks**: collisions (pilot A's `/feature`, same-named roles; O-3); context and auto-delegation (O-5); genre leaks past the word scan (the reviewer reads; spec roots from `[paths]`); no convention text in consumer projects (`spec check` decides); verification a role's claim until `spec verify`; undocumented behaviour ("Open").

## Acceptance criteria

M: the mutation that must turn it red. Developer (`plugin/`):

- [ ] R-1 -- under `plugin/` exactly "Files" sixteen, `.DS_Store` ignored.
- [ ] R-2 -- `0.3.0`; three equal descriptions naming the roles and `/specengine:feature`; `PINS` ends `("0.3.0", <BLAKE3 of plugin/specengine/>)`.
- [ ] R-3 -- agents as "Agent front-matter", `disallowedTools` exactly as "Roles", order free.
- [ ] R-4 -- agents and skill as "Content".
- [ ] R-5 -- backticked: analyst `get_task`, `get_context_bundle`, `ask_question`, `report_discrepancy`, `task_id`, `working_answer`, `submit_plan`, never `claim_task`; spec-author `get_node`, `propose_change`, `span_hash`, `task_id`; the others `get_task`, `report_discrepancy`, `task_id`; implementer, tester hold `CLAUDE.md`; every role "the project's language".
- [ ] R-6 -- the skill as "Skill front-matter"; its body backticks `get_task`, `submit_plan`, `claim_task`, `report_run`, `complete_task`, `ask_question`, `report_discrepancy`, `task_id`, holds `spec task new`, `spec task approve`, each role as `specengine:<name>`, `Stage | Role`, the leg-end sentence; no word `poll`.

Spec writer:

- [x] S-1 -- this draft: criteria, WA-1-WA-12, Q1-Q3 (owner review open), D1-D7.
- [x] S-2 -- the `exclude`; `docs/README.md` "Walk" names it; the README "Files" bullet points here.
- [x] S-3 -- 06 s0, s8 per Q1, WA-3, WA-4: no line of 06 lets a spec writer write a spec file.
- [ ] S-4 -- shipping: `docs/canon/plugin.md` <= 12 288 B; 07 s1.4 lists `/specengine:feature`; 08 s2 "Plugin"; `CLAUDE.md` State; README "Status"; `export index && check` clean with the agents, no plugin finding, worst W <= 108 468 B.
- [ ] S-5 -- no `.claude/**` edit; no owner text.

Test engineer:

- [ ] T-1 -- `plugin_files.rs` `FILES` sixteen, `PINS` last `0.3.0` (M: an unlisted `specengine/agents/x.md`; `skills/feature/SKILL.md` removed; an agent edited, `PINS` unchanged).
- [ ] T-2 -- new `plugin_roles.rs`: R-3, R-5 (M: `tools: Read, Grep, Glob` on analyst; `permissionMode: acceptEdits`; `effort: high`; `isolation: worktree`; `model: claude-opus-5-5`; `disallowedTools: [Write, Edit`; `Edit` out of reviewer's, `Write` out of spec-author's, `Bash` out of analyst's; `name` not the stem; `name: spec-writer`; descriptions over 1 000 B; `submit_plan` out of analyst; `claim_task` into analyst; a precedence sentence dropped; `CLAUDE.md` out of implementer).
- [ ] T-3 -- R-6 (`plugin_files.rs` AC-05, `plugin_skills.rs` AC-11, `plugin_roles.rs`; M: `disable-model-invocation` dropped; `argument-hint` first; a 4 097 B body; `specengine:tester`, `claim_task`, `spec task approve` or the leg-end sentence dropped; "poll `get_task` until it is ready" added); a 1 024-character `feature` description keeps the 1 200 B sum green.
- [ ] T-4 -- AC-08's scan and rules over the agents (M: `cargo` in implementer; `mechanic` in spec-author; "at most 3 rounds" in the skill; `` `partial` `` in reviewer; `get_bundle` in tester; `mcp__plugin_specengine_specengine__get_task` in analyst); `mcp_tasks.rs` green.
- [ ] T-5 -- `parity_config/mod.rs` `SKIP_FILES` + the agents glob; `check_parity.rs`: it dropped -> exactly the five agents join; the skill glob dropped -> exactly six `SKILL.md` (M: either out of `SKIP_FILES`).
- [ ] T-6 -- `anonymity.rs`: a Cyrillic letter in each of the six new paths flagged.
- [ ] T-7 -- workspace nextest, clippy `-D warnings`, fmt, `spec check` green; `git diff crates/*/src` empty.

## Owner's live check

Record the version; a scratch copy of `fixtures/spec-c`, `[gate]` `selective`, `spec` on `PATH`, `claude --plugin-dir <clone>/plugin/specengine`:

- O-1 `claude plugin validate --strict` passes on `plugin`, `plugin/specengine`; with `permissionMode: acceptEdits` in a role (ignored, D4): recorded.
- O-2 typeahead lists the five as `specengine:<name>`; the skill's calls run each; the Agent tool's `subagent_type` form and a `skills:` list's naming recorded.
- O-3 with scratch `.claude/agents/reviewer.md`, `.claude/commands/feature.md`: bare `/feature` runs the project's, `/specengine:feature` the plugin's; without the command bare `/feature` the plugin's; `--agent reviewer`, deny `Agent(reviewer)`, `Agent(specengine:reviewer)` recorded.
- O-4 the analyst cannot Edit or run Bash, reads `get_task`, calls `submit_plan`; `inherit` runs on the session's model.
- O-5 `/context`: five role descriptions, none for `feature`; bytes recorded.
- O-6 leg 1 ends at `review` with the owner's step; `spec task approve` on a terminal; leg 2 claims; a role's write in the claimed worktree passes, an unclaimed one is refused, stderr to the model; proposals in `spec inbox`; `report_run`, `complete_task` -> `done`; `git log`: spec files changed only by `spec approve` commits.
- O-7 plugin-skills AC-13, plugin-gate AC-16 closed or recorded.

## Out of scope

Other 07 s4 hooks; a role-aware gate (hooks now name a subagent's scoped type; supplementing ADR-0037 takes a new ADR); `[zones]` role write rules; profiles, stack roles (ADR-0027); new MCP tools (reserved `prepare-task`, `round`); `spec verify`, marker binding (Phase 3); `.claude/**` here or in a pilot; installing the plugin; a separate gate plugin.

## Open

1. Q1-Q3 decided by the orchestrator per the owner's pre-approval; the owner's explicit review is open. Another Q1 (roles writing spec files) or Q2 (a separate gate plugin) takes a new ADR; another Q3 is a PATCH.
2. The live check. Fallbacks: the skill taking a bare `/feature` from a project command -> `commands/feature.md` (MINOR); the Agent tool refusing scoped names -> back to this spec before shipping.
3. The owner's reviews of ADR-0037 and ADR-0038 stay open, unrelated.

## Implementation

Filled in after implementation.
