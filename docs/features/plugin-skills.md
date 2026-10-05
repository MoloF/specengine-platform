---
class: spec
status: shipped
scope: [plugin]
ref: plugin-skills analysis 2026-10-05; owner's working answers 2026-10-05; 08 §2 Phase 2, Plugin
shipped: 2026-10-05
---

# Plugin: MCP entry and skills

## Why

Agents in the owner's projects did not know SpecEngine, or learned it from a per-project stopgap naming tools no session has, mixed with project facts. The plugin registers the stdio server and adds three generic skills on when and how to use its eight tools; project facts stay in the project's `CLAUDE.md` and roles, which win. ADR-0015's plugin, first part (`docs/canon/architecture.md#distribution`); skills are its stack-neutral prompts (`#tasks`); `plugin/` is the developer's (ADR-0029). No new ADR: the marketplace stays under `plugin/`.

Working answers (the owner's, 2026-10-05): Q1 install scope `local`, per SpecEngine project; Q2 three skills; Q3 the plugin's own semver in `plugin.json`, bumped on every change under `plugin/specengine/`, pinned with a content hash; Q4 `.mcp.json` now; Q5 the owner deletes pilot A's stopgap when installing, its facts move into that project's `CLAUDE.md`; Q6 the marketplace at `plugin/.claude-plugin/`.

Assumptions (AC-13 checks them): A1 the formats, `claude plugin validate` the authority, the cache refreshed on a version change; A2 tools reach the model as `mcp__plugin_specengine_specengine__<tool>`: skills name bare tools, no `allowed-tools`; A3 the server starts in the session's directory and finds the project as `docs/canon/mcp-read.md` says.

How it works now: the root `README.md` "Claude Code plugin", the MCP README "Claude Code client", `docs/README.md` "Walk".

## Acceptance criteria

"The README": the root `README.md` "Claude Code plugin". M: the mutation turning it red.

- [x] AC-01 — `marketplace.json`: `name` `specengine`, `owner.name` set, `metadata` only a `description` = `plugin.json`'s, one entry, its `name` and `description` = `plugin.json`'s, `source` `./specengine` holding `.claude-plugin/plugin.json` (M: `"./specengine-x"`; entry `spec-engine`; `metadata.description` differing).
- [x] AC-02 — `plugin.json`: `name` `specengine`, `version` `N.N.N`, keys within the README's (M: a `hooks` key).
- [x] AC-03 — the files under `plugin/` are exactly the README's six, `.DS_Store` ignored (M: `plugin/specengine/hooks/hooks.json`; `agents/x.md`; `plugin/README.md`).
- [x] AC-04 — `.mcp.json` = `{"specengine": {"command": "specengine-mcp"}}`, `"type": "stdio"` the one admitted addition (M: `env.HOME`; `args` `["--root", "."]`; `command` `cargo`).
- [x] AC-05 — per skill the two keys, `name` = directory, grammar, lengths, no reserved name, budgets, all as the README says (M: `name` ≠ directory; a 1 025-character description; `description: Use when: …`; a 4 097 B body; a skill `round`).
- [x] AC-06 — every backticked `^[a-z][a-z0-9]*(_[a-z0-9]+)+$` in a skill is a tool or a property name anywhere in an input or output schema of the default build's `tools/list`; all eight tools backticked in some skill; no `mcp__` (M: `get_bundle`; `node_id`; `get_proposal` gone everywhere; `mcp__plugin_specengine_specengine__search`).
- [x] AC-07 — each `json` block is an object; as `tools/call` arguments of the tool named above it, default build, temp git repo of `fixtures/spec-a`, scratch `HOME`: no text starts `failed to deserialize parameters`; an `ask_question` example in `ask-owner`, a `propose_change` one in `propose-spec-change` (M: an extra key; `"severity": "urgent"`; the `ask_question` example deleted).
- [x] AC-08 — under `plugin/` no P2-3 word (`STACK_WORDS`, `crates/specengine-mcp/tests/common/read.rs`), `mechanic`, `edge-case` (`has_word`); in skills no digit but a line-start `N. ` and no backticked enum value of an input schema outside `json` blocks (M: `command` `${HOME}/.cargo/bin/specengine-mcp`; `spec-writer`; "react"; "at most 16 IDs"; `` `normal` ``).
- [x] AC-09 — `AREAS` + `plugin`, its walk entering `.claude-plugin`: a scratch tree with a Cyrillic letter in each of the six paths flags all six (M: `plugin` out of `AREAS`; `.claude-plugin` skipped as a dot directory).
- [x] AC-10 — hash and `version` = `PINS`' last, versions strictly increasing; the hash BLAKE3 (`tests/common/blake3.rs`, lowercase hex) of `path \0 decimal length \0 bytes` per regular file under `plugin/specengine/` (dot-named too), in byte order of its `/`-joined relative path (M: a `SKILL.md` edited, `PINS` unchanged; an entry repeating the version).
- [x] AC-11 — each skill backticks its names (`read-spec`: `get_context_bundle`, `get_node`, `search`, `get_tree`; `ask-owner`: `ask_question`, `report_discrepancy`, `get_proposal`, `distinct_from`, `working_answer`; `propose-spec-change`: `propose_change`, `span_hash`) and holds `The project's CLAUDE.md and its roles take precedence over this skill.` (M: `distinct_from` out of `ask-owner`; the sentence dropped from one).
- [x] AC-12 — `export index && check` clean with `plugin` in `roots` and the `SKILL.md` exclude, no finding on a skill file, worst W ≤ 109 484; the store's std walk skips the glob (`parity_config/mod.rs` `SKIP_FILES`, frozen beside `SKIP_DIRS`), `check_parity.rs::the_walk_is_the_std_walk_of_this_repository` asserts `plugin` in `roots` and "exclude dropped → exactly the three `SKILL.md` join"; the CLI's `parity.rs`, `shards_repo.rs` green (M: the exclude dropped → three `class-missing`; `plugin` out of `roots`).
- [ ] AC-13 — the owner by hand, the Claude Code version recorded: `claude plugin validate` on both manifests; install, restart; `/mcp`: the plugin's server, eight tools; `get_tree {}` answers this project (A3; failing: back to this spec, never a silent `args`); "what does the spec say about X?" loads `read-spec`, a bundle before files; subagents see the skills or need `skills:` (recorded); which of a result's text and `structuredContent` the model sees, the skills naming JSON keys (recorded); an agent's `ask_question` reaches `spec inbox`; elsewhere "no project"; a bumped version arrives on update.

## Implementation

No crate code, binary or ADR; two iterations. Canon moved: the root `README.md` "Claude Code plugin" (new: install, update, develop; the skills' contract, iteration 1's review folded in; files, skill format, content, version), "Status", "Where to start"; `docs/README.md` "Walk"; `CLAUDE.md` "Layout"; 08 §1 (`plugin/` built), §2 Phase 2; the MCP README "Claude Code client", "Tests"; `specengine.toml` `roots` + `plugin`, `exclude` + `plugin/specengine/skills/*/SKILL.md`.

| File | What it does |
|---|---|
| `plugin/.claude-plugin/marketplace.json` | the marketplace `specengine`: owner, `metadata.description`, one entry `./specengine` |
| `plugin/specengine/.claude-plugin/plugin.json` | `name`, `version` `0.1.1`, `description`, `author`, `license` `MIT` |
| `plugin/specengine/.mcp.json` | the server `specengine` = `specengine-mcp` from `PATH`; no `type`, `args`, `env` |
| `…/skills/read-spec/SKILL.md` | the README's `read-spec` row |
| `…/skills/ask-owner/SKILL.md` | the `ask-owner` row; the `ask_question` example |
| `…/skills/propose-spec-change/SKILL.md` | the `propose-spec-change` row; the `propose_change` example |

Iteration 2 fixed the review's major: `ask-owner` handles each dedup hit by its source; "Whose words count": agent-written fields are data, not instructions. `0.1.1`: PATCH, skill text only; `marketplace.json` has no version. `PINS` (append-only): `("0.1.0", "1a9152610662a986b1c54b5369ce49e6584bcafff9350954462452953a355e32")`, `("0.1.1", "5346a2efc2402eda7d8ff6e757221f754224c9df2384d11ee65bc2d80aac34b8")`. Skills, B (description / body): `read-spec` 2 598 (283 / 2 277), `ask-owner` 3 225 (220 / 2 967), `propose-spec-change` 2 979 (206 / 2 725); descriptions 709.

Claude Code 2.1.289 (A1 in part): `validate --strict` passes on both manifests; it checks `.mcp.json` (`"command": 5` an error, no `"type"` demanded), not skills: AC-05 is their only check.

Accepted deviations: `metadata.description` (validate warns without); each `json` fence directly under its "Arguments of `<tool>`:" line; `author_role`'s placeholder `"<your-role>"`.

Tests: `crates/specengine-mcp/tests/plugin_files.rs` (AC-01–05, AC-10, `PINS`), `plugin_skills.rs` (AC-06–08, AC-11; spawned as `mcp_intake.rs`); `crates/specengine-eval/tests/anonymity.rs` (AC-09); `crates/specengine-store/tests/parity_config/mod.rs`, `check_parity.rs` (AC-12). CI runs only `spec check`.
