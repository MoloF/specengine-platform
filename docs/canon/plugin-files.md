---
class: canon
tier: 2
scope: [plugin]
owner: owner
reviewed: 2026-10-10
---

# Plugin: files, contract, version, install

What a change to `plugin/` must keep; what the plugin does (skills, roles, routing): `docs/canon/plugin.md`. Tests enforcing it: `crates/specengine-mcp/tests/plugin_files.rs`, `plugin_skills.rs`, `plugin_roles.rs`, `crates/specengine-core/tests/plugin_front_matter.rs`.

## Changing it

- **Files**, a closed set of sixteen (`.DS_Store` ignored): `plugin/.claude-plugin/marketplace.json` (`name`, `owner`, `metadata.description`, one `plugins` entry `{name, source: "./specengine", description}`, both descriptions `plugin.json`'s, no `version`); `plugin/specengine/.claude-plugin/plugin.json` (keys within `name, version, description, author, license, homepage, repository, keywords`); `plugin/specengine/.mcp.json` = `{"specengine": {"command": "specengine-mcp"}}` (`"type": "stdio"` the one admitted addition); `skills/{read-spec,ask-owner,propose-spec-change,analyze,implement,feature}/SKILL.md`; `agents/{analyst,spec-author,implementer,tester,reviewer}.md`; `hooks/hooks.json` (one `PreToolUse` entry, `gate.md` "Hook") and `hooks/gate.sh`. No README. The doc walk excludes `skills/*/SKILL.md` and `agents/*.md` (no `class:`).
- **A skill**: front-matter `name` (its directory, `^[a-z0-9]+(-[a-z0-9]+)*$`, <= 64, never the reserved `prepare-task`, `round`) and `description` (one plain YAML line, 1-1 024 characters); the five model-invoked ones exactly these two keys, descriptions <= 1 200 B together (every session loads them; 1 170 at 0.6.0); `feature` exactly `name`, `description`, `argument-hint`, `disable-model-invocation: true`, in order, its description outside that sum (not in context); a body <= 4 096 B (`ask-owner` 4 059, `propose-spec-change` 4 092 at 0.6.0).
- **A role**: front-matter lines `name` (the file stem, the same grammar), `description`, `model: inherit`, and on analyst, spec-author, reviewer `disallowedTools` as `docs/canon/plugin.md` "Roles", in order, nothing else, parsed strictly: no `tools` (an allowlist drops every MCP tool), `effort`, `isolation` (another worktree, refused by the gate), `skills`, and none of the keys Claude Code ignores for plugin agents (`permissionMode`, `hooks`, `mcpServers`, `initialPrompt`); unparseable front-matter loads with its fields dropped, an unrestricted role. Descriptions one plain line, scoped to SpecEngine tasks, <= 1 000 B together; a body <= 4 096 B.
- **Content**, skills and roles: English and generic: no cap, limit, default or enum value (outside `json` blocks no digit but a list's `N. `; the round bound in words), no project fact, path, command, real ID, `[zones]` name or pilot material, no stack word (07 s1.2), `mechanic`, `edge-case`; tools by bare name, never `mcp__`; every backticked snake_case name a tool or a schema property of `tools/list`, all thirteen named; the `json` examples deserialize as their tool's arguments. Verbatim in each skill: "The project's CLAUDE.md and its roles take precedence over this skill."; in each role: "...over this role."; in `feature`: "This leg ends here: never wait for the approval."
- **Version**: `plugin.json` semver (current 0.7.0, `PINS` BLAKE3 `d93dbf9854171932f155dae6939c9b97e23a09b702b41627d13f13d1bdd11311`), PATCH a skill's or role's text alone, MINOR anything else (a skill or role added, renamed or removed, a key, `.mcp.json`, a tool or kind the texts newly name: 0.4.0 `interpretation`, 0.5.0 `report_decision`, 0.6.0 `amendment`; a key a tool returns: 0.7.0 `get_node`'s `norm_hash`); `PINS` in `plugin_files.rs`, append-only, ties each version to a BLAKE3 of `plugin/specengine/`. A change to what a named tool takes or returns updates the text and the version in the same change.

## Install and verification

One plugin: installing it, per SpecEngine project with `--scope local`, brings the roles together with the gate hook (passing without `[gate]`); SpecEngine never installs it. Claude Code 2.1.294: `claude plugin validate --strict` passes on `plugin` and `plugin/specengine`, agents and skills included. Its docs: plugin agents are `<plugin>:<name>`, below project agents, beside a same-named one; a plugin skill's bare alias yields to a command holding the name; plugin subagents see the MCP tools and load skills. Not documented, the owner's live check (`docs/features/plugin-roles.md` "Open"): the Agent tool's name form, bare `/feature` beside a project command, `--agent` and deny rules with both names, `skills:` naming.
