---
class: canon
tier: 2
scope: [root]
owner: owner
reviewed: 2026-10-15
---

# SpecEngine

A local specification engine for projects built together with AI agents.

As a project grows, its documentation grows with it. An agent spends more and more context gathering what it needs, makes more mistakes, and relies on documents that have already drifted from the code. SpecEngine keeps the context cost of a task constant and makes drift visible:

- **business-logic tree and atomic records** — requirements, assumptions, questions, decisions, criteria — live in git next to the code, one file per record;
- **code bindings** — `// @implements ID@rev` markers and normalized AST hashes of symbols show where spec and code have diverged;
- **task context bundle** — the minimal set of nodes, signatures and open questions within a token budget, instead of reading the whole corpus;
- **owner queue** — an agent reports a discrepancy with evidence and options and keeps working on a working answer; nothing is blocked, and only tasks approved by the owner go into development;
- **interfaces** — CLI, an MCP server for Claude Code agents, later a web UI.

Rust comes first (including Bevy ECS and RON data); other languages via tree-sitter grammars.

## Status

**Phase 0 done** (2026-09-29): architectural decisions made, key claims measured on two pilot corpora. **Phase 1 done** (2026-10-05), the reading core: the spec parser (`specengine-model`, `specengine-core`), the SQLite index (`specengine-store`), `spec check` and the `spec` CLI (`specengine-cli`: `init`, `index`, `search`, `show`, `tree`, `graph`, `bundle`, `check`, `export index`) and the stdio MCP server's read tools and resources (`specengine-mcp`) are shipped; `spec check` gates this repository's documentation. Also built: layer A parsing and hashing (`specengine-code`), a corpus census, importer and layout round trip (`specengine-import`), rust-analyzer loading (`specengine-ra`) and the measurement harness (`specengine-eval`). Task W is measured on both pilots (`pilot-w`); A migrates first, then B. **Phase 2**, queue and tasks, in progress: proposals (edits, new files and ID sections) are raised, reviewed and applied as commits (`spec propose`, `inbox`, `review`, `approve`, `reject`), a question, discrepancy or agent's decision approved as a decision record (several at once: `spec round`), agents raise them over MCP; tasks only the owner approves hand agents a versioned package (`spec task`, `get_task`); the Claude Code plugin brings the server, skills, stack-neutral roles run by `/specengine:feature` and the task gate on agent writes; the web UI (`ui/`) reads over HTTP (`specengine-http`, run by `spec serve`) and stages decisions, each confirmed on a terminal ([`08-roadmap`](docs/specs/specengine-platform/08-roadmap.md)).

## Where to start

| What | Where |
|---|---|
| Rules for humans and agents | [`CLAUDE.md`](CLAUDE.md) |
| SpecEngine in your Claude Code project | "Claude Code plugin" below |
| Documentation index | [`docs/index.md`](docs/index.md) |
| Architecture rules | [`docs/canon/architecture.md`](docs/canon/architecture.md) |
| Decisions | [`docs/decisions/`](docs/decisions/) |
| Design and plan | [`docs/specs/specengine-platform/`](docs/specs/specengine-platform/README.md) |
| Documentation convention | [`docs/canon/documentation-system.md`](docs/canon/documentation-system.md), applied in [`docs/README.md`](docs/README.md) |

## Claude Code plugin

`plugin/` is a local marketplace with one plugin, `specengine`: the stdio MCP server `specengine` (`specengine-mcp` from `PATH`, no arguments: the project is the session's directory, the data under `HOME`); five model-invoked skills, `specengine:read-spec`, `specengine:ask-owner`, `specengine:propose-spec-change`, `specengine:analyze`, `specengine:implement`, on when and how to use its fifteen tools (the task tools `get_task`, `claim_task`, `submit_plan`, `report_run`, `complete_task` included); five stack-neutral roles, `specengine:analyst`, `spec-author`, `implementer`, `tester`, `reviewer`, and the owner's `/specengine:feature`, running a task through them from a requirement in words to `done`, every spec change a proposal; the project's `CLAUDE.md` and roles take precedence (skills, roles, routing: `docs/canon/plugin.md`; files and rules: `docs/canon/plugin-files.md`); and the task gate (`docs/canon/gate.md`): a `PreToolUse` hook running the wrapper `hooks/gate.sh`, i.e. `spec gate --stdin`, before every Edit, Write, MultiEdit, NotebookEdit wherever the plugin is enabled (a user-scope install: every project; without `[gate]` it passes, one process start per write). Install per project, scope `local` (the roles come with the gate hook):

```bash
cargo install --locked --force --path crates/specengine-cli  # both from one commit
cargo install --locked --force --path crates/specengine-mcp
claude plugin marketplace add <clone>/plugin                 # inside the project
claude plugin install specengine@specengine --scope local    # restart Claude Code
# update: git -C <clone> pull, both installs, then
claude plugin marketplace update specengine
claude plugin update specengine@specengine                   # restart
claude --plugin-dir <clone>/plugin/specengine                # develop; installed copy disabled
```

One route only: a project `.mcp.json` running `specengine-mcp` too duplicates the tools. Launched outside a shell, Claude Code may lack `~/.cargo/bin` on `PATH` (`/mcp`: failed; the gate refuses writes under `[gate]`, a `PATH` hint). An older `spec` without `gate`: the same refusal; reinstall from one commit. Verified on Claude Code 2.1.294: `claude plugin validate --strict` passes on `plugin` and on `plugin/specengine`, skills and agents included; the commands above await the owner's check (`docs/features/plugin-skills.md` AC-13, `plugin-roles.md` "Open").

## Web UI

```bash
pnpm --dir ui install --frozen-lockfile && pnpm --dir ui build  # first: the daemon embeds ui/dist
cargo install --locked --force --path crates/specengine-http  # beside spec, same commit
spec serve                  # in the project: open the listening URL, http://127.0.0.1:7777/
```

UI work: `pnpm --dir ui dev` (`ui/README.md`). The gate never needs the daemon (ADR-0039); how it runs: `docs/canon/serve.md`, the UI it serves: `docs/canon/serve-ui.md`.

## Development

All development goes through the Claude Code role pipeline (`.claude/agents/`, command `/feature`): analyst → spec writer → developer ⇄ reviewer and test engineer → documentation update.

```bash
./scripts/hooks-install.sh                        # pre-commit: documentation check
cargo run -q -p specengine-cli -- export index    # regenerate docs/index.md
cargo run -q -p specengine-cli -- check           # the documentation check; its summary shows the worst W
```

Requires Rust ≥ 1.90 (≥ 1.98 for `specengine-eval --features ra`) and `cargo-nextest`.

## License

[MIT](LICENSE), copyright (c) 2026 MoloF. Free to use, modify and distribute; copies must keep the copyright notice and the license text (ADR-0025).
