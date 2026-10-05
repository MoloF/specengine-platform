---
class: spec
status: draft
scope: [plugin]
ref: plugin-skills analysis 2026-10-05; owner's working answers 2026-10-05; 08 §2 Phase 2, Plugin
---

# Plugin: MCP entry and skills

## Why

Agents in the owner's projects do not know SpecEngine, or learn it from a per-project stopgap naming tools no session has, mixed with project facts. The plugin registers the stdio server and adds three generic skills on when and how to use its eight tools; project facts stay in the project's `CLAUDE.md` and roles, which win. ADR-0015's plugin, first part (`docs/canon/architecture.md#distribution`); skills are its stack-neutral prompts (`#tasks`); `plugin/` is the developer's (ADR-0029). No new ADR: the marketplace stays under `plugin/`.

Working answers (the owner's, 2026-10-05, session rule): Q1 install scope `local`, per SpecEngine project; Q2 three skills; Q3 the plugin's own semver in `plugin.json`, bumped on every change under `plugin/specengine/`, pinned with a content hash; Q4 `.mcp.json` now; Q5 the owner deletes pilot A's stopgap when installing, its facts move into that project's `CLAUDE.md`; Q6 the marketplace at `plugin/.claude-plugin/`.

Assumptions (AC-13 checks them): A1 the formats below, `claude plugin validate` the authority, the cache refreshed on a version change; A2 tools reach the model as `mcp__plugin_specengine_specengine__<tool>`: skills name bare tools, no `allowed-tools`; A3 the server starts in the session's directory and finds the project as `docs/canon/mcp-read.md` says.

## Description and interactions

Installed as the root `README.md` will say (Data), a session has the server `specengine` (`mcp-read.md`, `docs/canon/agent-intake.md` "Tools") and the model-invoked skills `specengine:<name>`. No binary changes.

## Data

Files, a closed set (`.DS_Store` ignored throughout):

```
plugin/.claude-plugin/marketplace.json
plugin/specengine/.claude-plugin/plugin.json
plugin/specengine/.mcp.json
plugin/specengine/skills/{read-spec,ask-owner,propose-spec-change}/SKILL.md
```

`marketplace.json`, these keys only (no `version`):

```json
{"name": "specengine", "owner": {"name": "MoloF"},
 "plugins": [{"name": "specengine", "source": "./specengine", "description": "<plugin.json's>"}]}
```

`plugin.json`, keys ⊆ `name, version, description, author, license, homepage, repository, keywords` (skills and `.mcp.json` at their default places):

```json
{"name": "specengine", "version": "0.1.0", "author": {"name": "MoloF"}, "license": "MIT",
 "description": "SpecEngine for agents: the spec MCP server and skills to read the spec, raise questions and discrepancies, and propose spec changes."}
```

`.mcp.json` by value `{"specengine": {"command": "specengine-mcp"}}`: on `PATH`, no `args`, no `env` (the project from the cwd, data from `HOME`); `"type": "stdio"` only if `claude plugin validate` demands it (recorded in "Implementation").

**Skill file**: `---\nname: <its directory>\ndescription: <one line>\n---\n`, then the body. `name` `^[a-z0-9]+(-[a-z0-9]+)*$`, ≤ 64, never reserved (`analyze`, `implement`, `prepare-task`, `round`: 07 §1.4). `description` a plain YAML scalar: no leading `` -?:,[]{}#&*!|>'"%@` ``, no `: ` or ` #`, 1–1 024 characters. Budgets: descriptions ≤ 1 200 B together (every session loads them), each body ≤ 4 096 B.

**Content contract** (the developer writes, the reviewer checks). English; each holds verbatim `The project's CLAUDE.md and its roles take precedence over this skill.`

| Skill | Use when | Teaches | Names |
|---|---|---|---|
| `read-spec` | before reading or changing behaviour the spec describes | the bundle first (the task's IDs or paths), then narrow: a node, its links, a term search, the tree; files only for what the bundle lacks; cite IDs; answers deterministic; "no project": stop | `get_context_bundle`, `get_node`, `search`, `get_tree` |
| `ask-owner` | the spec is silent or ambiguous, or code and spec disagree | a record, not chat prose: a question, or a discrepancy (evidence, priced options, recommendation); always a working answer, keep working (ADR-0012); `author_role` your role as the project names it; not `created`, with `hits`: a hit's `answer` is settled; a different question again with every hit in `distinct_from`; later `get_proposal`: `status`, `decision_note`; only the queue is written | `ask_question`, `report_discrepancy`, `get_proposal`, `distinct_from`, `working_answer` |
| `propose-spec-change` | only where the project routes spec edits through the queue | read the target, its `span_hash` as `base`, the span's whole new text, a `rationale`; the owner approves, a commit lands; stale base: re-read, resend; never edit the file | `propose_change`, `span_hash` |

Examples: one `ask_question` in `ask-owner`, one `propose_change` in `propose-spec-change`; a fenced `json` block of arguments right after a line naming its tool; values placeholders (`"<ID>"`) or plain words.

Never in a skill: a cap, limit, default or enum list (outside `json` blocks no ASCII digit but a line-start `N. `, no backticked enum value of an input schema); a project fact, a real corpus's ID, pilot material, this repository's process or role names; a P2-3 word (07 §1.2; `STACK_WORDS` in `crates/specengine-mcp/tests/common/read.rs`; mind English "react"), `mechanic`, `edge-case`; `mcp__`.

**Versioning.** Semver from `0.1.0`: PATCH skill text, MINOR anything else (a skill added, renamed, removed: subagents may list names; a manifest key; `.mcp.json`). `PINS: [(version, hash)]`, append-only, versions strictly increasing, the last = the current `version` and hash: BLAKE3 (`tests/common/blake3.rs`, lowercase hex) of `path \0 decimal length \0 bytes` per regular file under `plugin/specengine/` (dot-named too), in byte order of its `/`-joined relative path.

**Root `README.md`**, section "Claude Code plugin" (to verify on the owner's Claude Code version, AC-13):

```bash
cargo install --locked --force --path crates/specengine-cli  # both from one commit
cargo install --locked --force --path crates/specengine-mcp
claude plugin marketplace add <clone>/plugin  # inside the project
claude plugin install specengine@specengine --scope local  # restart Claude Code
# update: git -C <clone> pull, both installs, then
claude plugin marketplace update specengine
claude plugin update specengine@specengine  # restart
claude --plugin-dir <clone>/plugin/specengine  # develop; installed copy disabled
```

Plus: one route only (a project `.mcp.json` running `specengine-mcp` too duplicates the tools); launched outside a shell, Claude Code may lack `~/.cargo/bin` on `PATH` (`/mcp`: failed).

## Rules and edge cases

1. **Doc gate**, with the implementation (a missing root cannot check): `roots = ["CLAUDE.md", "README.md", "crates", "docs", "plugin", "ui"]`, `exclude` + `"plugin/specengine/skills/*/SKILL.md"` (no `class:` is `class-missing`; closed `canon` refuses `name`, `description`). `docs/README.md` "Walk" follows at shipping.
2. **Walk tests**: `crates/specengine-store/tests/parity_config/mod.rs`'s std walk skips that glob, a frozen rule beside `SKIP_DIRS`; `check_parity.rs::the_walk_is_the_std_walk_of_this_repository` asserts `plugin` in `roots` and "exclude dropped → exactly the three `SKILL.md` join"; the CLI's `parity.rs`, `shards_repo.rs` stay green.
3. **Tests**: `crates/specengine-mcp/tests/plugin_files.rs` (AC-01–05, AC-10, `PINS`), `plugin_skills.rs` (AC-06–08, AC-11; spawn as `mcp_intake.rs`); `crates/specengine-eval/tests/anonymity.rs` (AC-09). CI runs only `spec check`.
4. **Drift.** A slice changing what a named tool takes or returns updates the skill and the version in the same change (AC-06 catches names, the reviewer meaning), `plugin` then in its scope: `decision-apply`, `proposal-kinds`, `task-package`.
5. **A3 failing**: stop, return to the spec, never a silent `args`. No write into a pilot; the stopgap is not copied.

## Acceptance criteria

M: the mutation turning it red.

- [ ] AC-01 — `marketplace.json`: `name` `specengine`, `owner.name` set, one entry, its `name` = `plugin.json`'s, `source` `./specengine` holding `.claude-plugin/plugin.json` (M: `"./specengine-x"`; entry `spec-engine`).
- [ ] AC-02 — `plugin.json`: `name` `specengine`, `version` `N.N.N`, keys within Data's (M: a `hooks` key).
- [ ] AC-03 — the files under `plugin/` are exactly Data's six (M: `plugin/specengine/hooks/hooks.json`; `agents/x.md`; `plugin/README.md`).
- [ ] AC-04 — `.mcp.json` = Data's value, `"type": "stdio"` the one admitted addition (M: `env.HOME`; `args` `["--root", "."]`; `command` `cargo`).
- [ ] AC-05 — per skill the two keys, `name` = directory, grammar, lengths, no reserved name, budgets (M: `name` ≠ directory; a 1 025-character description; `description: Use when: …`; a 4 097 B body; a skill `round`).
- [ ] AC-06 — every backticked `^[a-z][a-z0-9]*(_[a-z0-9]+)+$` in a skill is a tool or a property name anywhere in an input or output schema of the default build's `tools/list`; all eight tools backticked in some skill; no `mcp__` (M: `get_bundle`; `node_id`; `get_proposal` gone everywhere; `mcp__plugin_specengine_specengine__search`).
- [ ] AC-07 — each `json` block is an object; as `tools/call` arguments of the tool named above it, default build, temp git repo of `fixtures/spec-a`, scratch `HOME`: no text starts `failed to deserialize parameters`; both examples exist (M: an extra key; `"severity": "urgent"`; the `ask_question` example deleted).
- [ ] AC-08 — under `plugin/` no P2-3 word, `mechanic`, `edge-case` (`has_word`); in skills no digit or backticked enum value outside `json` blocks (M: `command` `${HOME}/.cargo/bin/specengine-mcp`; `spec-writer`; "react"; "at most 16 IDs"; `` `normal` ``).
- [ ] AC-09 — `AREAS` + `plugin`, its walk entering `.claude-plugin`: a scratch tree with a Cyrillic letter in each of the six paths flags all six (M: `plugin` out of `AREAS`; `.claude-plugin` skipped as a dot directory).
- [ ] AC-10 — hash and `version` = `PINS`' last, versions strictly increasing (M: a `SKILL.md` edited, `PINS` unchanged; an entry repeating the version).
- [ ] AC-11 — each skill backticks its "Names" and holds the precedence sentence (M: `distinct_from` out of `ask-owner`; the sentence dropped from one).
- [ ] AC-12 — `export index && check` clean under Rules 1, no finding on a skill file, worst W ≤ 109 484; Rules 2 green (M: the exclude dropped → three `class-missing`; `plugin` out of `roots`).
- [ ] AC-13 — the owner by hand, the Claude Code version recorded: `claude plugin validate` on both manifests; install, restart; `/mcp`: the plugin's server, eight tools; `get_tree {}` answers this project (A3); "what does the spec say about X?" loads `read-spec`, a bundle before files; subagents see the skills or need `skills:` (recorded); an agent's `ask_question` reaches `spec inbox`; elsewhere "no project"; a bumped version arrives on update.

## Out of scope

Hooks (no tasks or daemon yet); plugin roles and `/feature` (need `get_task`, `claim_task`; agent-name resolution unverified, 06 §8); 07 §1.4's task prompts; stack profiles; a remote marketplace; permission presets; `spec mcp`; `plugin/README.md` (the index root has no room before a live shard).

## Implementation

Pending. At shipping, canon: the root `README.md` section; `docs/README.md` "Walk"; `CLAUDE.md` "Layout" (`plugin/` built, not grown); 08 §1; a pointer in the MCP README "Claude Code client".
