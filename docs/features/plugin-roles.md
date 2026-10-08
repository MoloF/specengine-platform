---
class: spec
status: shipped
scope: [plugin]
ref: plugin-roles analysis 2026-10-08 (HEAD b7009de); Claude Code docs read 2026-10-08, build 2.1.294; the orchestrator's Q1-Q3 per the owner's pre-approval; 08 s2 Phase 2, Plugin
shipped: 2026-10-08
---

# Plugin roles

## Why

Role pipelines lived per project, copied by hand, and touched no queue (this repository's `/feature` drafts no task, its spec writer writes `docs/`; pilot A copied its role names), so the Phase 2 live check (08 s2) had nothing to run on and P2-12 (07 s1.2) stood only over MCP. Now in any SpecEngine project the owner's `/specengine:feature` takes a requirement in words through analysis, approval on a terminal, implementation, review and tests to `done`, spec changes as proposals, routed project -> profile -> generic by the project's `CLAUDE.md`. ADR-0015, ADR-0027 ("Cost"); no new ADR.

Resolved per Q1: 06 s8 let the spec writer write the feature spec, criterion markers and "Implementation", against `docs/canon/architecture.md#apply`, 05 s1 principle 3 and 06 s0; 06 s8 is restated and 06 s0 gains the spec author: SpecEngine's roles write no spec file. Decided here: `feature` is a skill with `disable-model-invocation: true`, not `commands/feature.md` (skills supersede commands; a plugin skill's bare alias yields to a project command holding the name); roles `analyst`, `spec-author`, `implementer`, `tester`, `reviewer`.

How it works now: `docs/canon/plugin.md`; install: the root `README.md` "Claude Code plugin".

## Acceptance criteria

M: the mutation that turned it red; 39 run on the real files, all red. "Roles": `crates/specengine-mcp/tests/plugin_roles.rs`.

- [x] R-1 -- sixteen files under `plugin/` (`plugin_files.rs` `ac03_the_plugin_tree_is_data_s_ten_files`; M: an unlisted `agents/x.md`; `skills/feature/SKILL.md` removed).
- [x] R-2 -- 0.3.0, three equal descriptions naming the roles and `/specengine:feature`, `PINS` (Roles `r2_the_three_descriptions_name_the_roles_and_the_skill`, `plugin_files.rs` `ac10_the_content_and_version_are_the_last_pin`; M: an agent edited, `PINS` unchanged).
- [x] R-3 -- agent front-matter, `disallowedTools`, budgets (Roles `r3_each_agent_has_the_front_matter_of_its_role`; core `plugin_front_matter.rs` `each_role_and_the_skill_parse_as_their_lines`, `a_broken_block_is_named`; M: `tools: Read, Grep, Glob`; `permissionMode: acceptEdits`; `effort: high`; `isolation: worktree`; `model: claude-opus-5-5`; `disallowedTools: [Write, Edit`; `Edit`, `Write`, `Bash` each out of its role; `name` not the stem; `name: spec-writer`; descriptions 1 001 B; a 4 097 B body; `: ` in a description).
- [x] R-4 -- content (Roles `r4_the_agents_and_the_skill_keep_the_content_rules`, `plugin_skills.rs` `ac08_no_stack_word_digit_or_enum_value`, `mcp_tasks.rs` `p2_3_the_task_sources_and_the_plugin_name_no_stack_word`; M: a precedence sentence dropped; `get_bundle`; `mcp__plugin_specengine_specengine__get_task`; `cargo`; `mechanic`; "at most 3 rounds"; `` `partial` ``). Project facts and `[zones]` names: the reviewer's.
- [x] R-5 -- names per role (Roles `r5_each_role_names_its_tools_and_its_project`; M: `submit_plan` out of analyst; `claim_task` into it; `CLAUDE.md` out of implementer).
- [x] R-6 -- the skill (Roles `r6_the_skill_runs_a_task_and_never_waits`, `plugin_files.rs` `ac05_each_skill_file_has_data_s_front_matter_and_budgets`, `plugin_skills.rs` `ac11_each_skill_names_its_names_and_yields_to_the_project`; M: `disable-model-invocation` dropped; `argument-hint` first; a 4 097 B body; `specengine:tester`, `claim_task`, `spec task approve`, the leg-end sentence each dropped; "poll `get_task` until it is ready"; "wait until approved").
- [x] S-1 -- this spec. S-2 -- `specengine.toml` `exclude` + `plugin/specengine/agents/*.md`, `docs/README.md` "Walk". S-3 -- 06 s0, s8 per Q1.
- [x] S-4 -- `docs/canon/plugin.md` (12 201 B) from the README's rules + roles, routing, legs, install, verification; README shortened to install and a pointer; 07 s1.4, 08 s2, `CLAUDE.md` State; `export index && check` clean, worst W <= 108 468 B.
- [x] S-5 -- no `.claude/**` edit; no owner text: this repository keeps its own `/feature`.
- [x] T-1 to T-6 -- as R-1 to R-6; T-5 `check_parity.rs` (`SKILL_FILES` six, `AGENT_FILES` five; M: either exclude dropped); T-6 `anonymity.rs` `all_text_is_english_no_cyrillic_letters` (M: a Cyrillic letter in each of the six new paths).
- [x] T-7 -- workspace nextest 2 084 passed (20 skipped), clippy, fmt, `spec check` clean; `crates/*/src` unchanged.

## Implementation

Three iterations; `claude plugin validate --strict` passes on 2.1.294 for `plugin` and `plugin/specengine`.

| File | What it does |
|---|---|
| `plugin/specengine/agents/{analyst,spec-author,implementer,tester,reviewer}.md` | the five roles, `docs/canon/plugin.md` "Roles"; 3 298, 3 243, 3 764, 3 144, 2 928 B; descriptions 980 of 1 000 B |
| `plugin/specengine/skills/feature/SKILL.md` | `/specengine:feature`, "Routing and legs"; 4 298 B, body 4 038 |
| `plugin.json`, `marketplace.json` | 0.3.0, descriptions naming the roles; `PINS` `("0.3.0", "acb1bbdb64d85643a9b11271d8010ed10dc884fcabf8bf63a49c9fb119b9d023")` |
| mcp `tests/plugin_roles.rs` (new), `plugin_files.rs`, `plugin_skills.rs`; core `tests/plugin_front_matter.rs` (new, strict YAML) | R-1 to R-6 |
| store `tests/check_parity.rs`, `parity_config/mod.rs`; eval `tests/anonymity.rs` | T-5, T-6 |

Accepted deviations: 1 `$ARGUMENTS`; 2 "its run already reported" for "run closed"; 3 implementation on the proposed text while proposals wait; 4 no `poll`: "never ... re-read the task until it changes"; 5 `author_role`, claim `role` as called; 6 markers backticked; 7 plugin descriptions name roles bare. Iteration 2 (review major): the claim rule, as an implementer following `implement` could report and complete before review; repository leaks dropped; `get_proposal`, `@assumes`. Iteration 3: review always runs; a refused claim ends the leg. Tests: `parity_config` `skipped_file` now matches a `*suffix` segment (`glob_matches`), not only a whole `*`.

## Open

1. Q1-Q3 decided by the orchestrator per the owner's pre-approval; the owner's review is open. Another Q1 (roles writing spec files) or Q2 (a separate gate plugin) takes a new ADR; another Q3 is a PATCH.
2. Owner's live check (version recorded; a `fixtures/spec-c` copy, `[gate]` `selective`, `--plugin-dir`), only what the docs leave open: O-1 validate with `permissionMode` in a role; O-2 the Agent tool's name form, `skills:` naming; O-3 beside scratch `.claude/agents/reviewer.md`, `.claude/commands/feature.md`: bare `/feature` runs the project's, `--agent reviewer`, deny `Agent(reviewer)` vs `Agent(specengine:reviewer)`; O-4 the analyst lacks Edit and Bash, reaches `submit_plan`; O-5 `/context`: no `feature` description; O-6 the cycle to `done`, spec files changed only by `spec approve`; O-7 plugin-skills AC-13, plugin-gate AC-16. Fallbacks: bare `/feature` taken from a project command -> `commands/feature.md` (MINOR); scoped names refused -> reopen.
3. Later PATCH: `propose-spec-change` "send again once `get_proposal` shows it rejected" reads as a wait under `/specengine:feature`.
4. Comments citing the README "Claude Code plugin" for moved rules (mcp, store, eval tests; the MCP README) still resolve: retarget to `docs/canon/plugin.md` when touched. Untested: Claude Code's YAML parser vs `serde-saphyr`; Bash writes.
5. The owner's reviews of ADR-0037 and ADR-0038 stay open, unrelated.
