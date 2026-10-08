---
class: spec
status: in-progress
scope: [specengine]
ref: research-2026-09-28
---

# 07. Interfaces: MCP, CLI, HTTP, hooks, config

> All four adapters call the same core functions. Names are preliminary.

## 1. MCP server

### 1.1. Design rules (from 04 §2–4)

Shipped with the read tools (canon `docs/canon/mcp-read.md`, `crates/specengine-mcp/README.md`): few tools, `instructions` and descriptions ≤ 2,048 characters; `readOnlyHint`; flat JSON Schema 2020-12 inputs, `outputSchema` + `structuredContent`, `content` the CLI's text; the CLI's 40,000-character cut and narrowing tails in place of pagination; both protocol eras over stdio; `rmcp` pinned exactly (04 §6); determinism stated in every description; the "one door" test (no agent tool writes to spec files), which every new tool keeps. Still to come:

- Tool-set levels as in Task Master: `core` (agents by default) and `admin`.
- A status header, a "Delta — what changed since the last request" section and "Hints — what to ask next" (as in tracey); `compact: bool` on reading tools (detail levels as in code-graph-mcp), on measured need.
- **Primary transport: stdio via `spec mcp`** in `.mcp.json`, a bridge to the daemon (Phase 2; today the `specengine-mcp` binary): Claude Code (≥ 2.1.84) GETs the MCP HTTP endpoint for an SSE stream, a stateless 2026-07-28 server answers 405 and the connection drops (claude-code#39790, "not planned"). HTTP `/mcp` comes later, with GET/SSE and `legacy_session_mode`.
- State passes only via explicit handles (`task_id`, `proposal_id`).
- **Long operations** (reindex, verify) may run past 120 s: Claude Code backgrounds the call, its result a notification (MCP README): no polling tool, no "call again" protocol, no long-blocking "wait for approval".

### 1.2. Tools (`core` set)

`get_tree`, `get_node`, `search`, `get_context_bundle` ship as reads over the CLI (`docs/canon/mcp-read.md`). Still to come for them: `get_tree`'s `sync` and open-proposal counters, `get_node` `with: bindings | history | proposals`, `get_context_bundle {task_id}` and its log (05 §6). Intake (`propose_change`, `report_discrepancy`, `ask_question`, `get_proposal`) shipped: `docs/canon/agent-intake.md`; `task_id` on them: `tasks.md`; a question or discrepancy approved as a decision record shipped: `docs/canon/decision-record.md`; kind `create`: `docs/canon/proposal-kinds.md`; `decision`, `interpretation`, `amendment` later.

| Tool | Input | Output | Notes |
|---|---|---|---|
| `find_symbols` | `query` \| `node_id` \| `file` \| `path:line` | qpath, kind, Bevy layer, signature, file:lines, bound nodes | repo-map on demand; general code navigation the agent does with the `LSP` tool of the code-intelligence plugin (04 §1.7), SpecEngine does not duplicate it |
| `get_impact` | `node_id` \| `qpath` \| `since: <commit>` | nodes, symbols, tests, tasks within the radius | graph + bindings; `since` = git diff -> graph walk (change impact) |
| `refs` | `node_id` | reverse lookup: all markers, tests, records referring to the node | needed **during** refactoring |
| `unmapped` | `path?` | source tree with spec coverage percentage | where there is no spec (tracey `query unmapped`) |
| `get_task` | `task_id` \| `next: true` | the task package (versioned, ADR-0027): `structuredContent` = status, goal, plan, targets, criteria, assumptions, open proposals, owner comments, bindings, `spec_snapshot` + diff, `bundle_hash`, `profile`; `content` = the neutral brief | shipped; `next`: the lowest-numbered `ready` (priority later) |
| `claim_task` | `task_id`, `role`, `worktree` | ok \| refusal with reason | shipped; only `ready`; opens a run |
| `submit_plan` | `task_id`, `plan_md`, `criteria[]`, `affected_nodes[]` | task status | shipped; -> `review` |
| `check_binding` | `node_id`, `qpath` | whether the marker resolves, `ast_hash`, `sync` | formerly `bind_code_symbol`, check only |
| `report_run` | `task_id`, `outcome`, `summary`, `changed_files[]` | ok | shipped; `verified` is set by `spec verify`, not by this call |
| `complete_task` | `task_id` | task status | shipped; run reported -> `done` |

**Task package** (ADR-0027; shipped: `docs/canon/task-package.md`). One type in `specengine-model`, its schema pinned by a test; a removed, renamed or re-meant key raises `schema_version`. Every key always present, absent = `null`; `compact` shortens only `content`. The bundle comes by reference (`bundle_hash` -> `get_context_bundle`): a 10k-token bundle collides with the 48,000-character cap. `claim_task.role` is the project's own role name, verbatim. Stack wording, tracker tickets and routing belong to the project's skills (06 §8); a task stores no ticket key.

Phase 2 contract checks (tested; `task-package.md` "Genre"): P2-1 `get_task` `structuredContent` = `spec task show --json`; P2-2 the pinned schema snapshot fails on a key removed or renamed without a bump; P2-3 a source scan (as core `check_genre.rs`) finds no `cargo`, `nextest`, `clippy`, `bevy`, `pnpm`, `npm`, `nest`, `react`, `jira` or this repository's role names in the package and brief sources or `plugin/**`; P2-4 fixtures `spec-a`, `-b`, `-c` give the same key set and `schema_version`; P2-5 a non-Rust fixture (`fixtures/spec-c`, Python) whose records hold no stack words yields none in JSON or content, whole cycle, CLI and MCP; P2-6 changing `profile` changes only that value; P2-7 `claim_task` with `role = "nest-developer"` succeeds, stored verbatim; P2-8 no key starts with `block` (ADR-0012); P2-9 two projects in one daemon never see each other's tasks, deleting one database leaves the other intact, nothing is written to SpecEngine's repository; P2-10 `get_task` at the 10k budget stays under 48,000 characters; P2-11 the package alone carries the verbatim title, goal, criteria text, target titles and open questions; P2-12 a project without a profile runs `get_task` -> `claim_task` -> `report_run` -> `complete_task` on generic prompts at MCP level.

**Human tools** (`owner` set, `_meta["anthropic/requiresUserInteraction"]: true`; 08 Phase 5) only **stage** (ADR-0035, `docs/canon/decision-staging.md`): `spec approve|reject` on a terminal confirms, so no form carries consent (no nonce, TTL or sealed `requestState` binding).

| Tool | What it does |
|---|---|
| `review_proposal` | URL-mode elicitation to the UI card, or a form (option, note) staging the human's choice |
| `approve_task` | the same for a task; `spec task approve` on a terminal confirms |

The MCP server remembers nothing between calls; a `cancel` stages nothing (the agent never re-opens the form itself), sending `notifications/cancelled` for the outstanding `elicitation/create`. No agent tool stages; `get_proposal` shows a stage.

### 1.3. Resources (for `@`-mentions)

`spec://{project}/tree` and the template `spec://{project}/node/{id}` ship (canon `docs/canon/mcp-read.md` "Resources"). To come: `spec://{project}/task/{id}` — task package (§1.2); `spec://{project}/inbox` — open proposals.

### 1.4. Prompts (= slash commands in Claude Code)

Plugin skills carry them: `read-spec`, `ask-owner`, `propose-spec-change` (`plugin-skills`); `/specengine:analyze <task>` (the plan, 06 §2), `implement <task>` (claim, run, completion; `plugin-gate`). Reserved: `prepare-task <node…>` (a task draft), `round` (the owner's question sheet, 06 §5).

### 1.5. Notifications

`subscriptions/listen` (2026-07-28) / `list_changed`: the server reports "proposal decided" and "task approved". ⚠ Claude Code's per-URI subscriptions are unconfirmed: poll `get_proposal`/`get_task` at a step's start.

## 2. CLI (`spec`)

```
# projects and index
spec init [--import IMPORTER]            # specengine.toml, layout, migration (08 §4)
spec index [--full]                      # rebuild the index (incremental by BLAKE3)
spec serve [--port 7777]                 # daemon: HTTP + SSE + MCP(HTTP) + UI + watcher
specengine-http --root DIR... [--port N]  # built: reads, staging; crates/specengine-http/README.md
spec mcp                                 # MCP over stdio (for .mcp.json)

# tree and nodes
spec tree [ROOT] [--depth N] [--kind K] [--archive]  # shipped: docs/canon/spec-cli-graph.md
spec show ID [--links [--archive]] [--bindings --history]
spec new KIND [--parent ID] [TITLE]      # the registry issues the ID, opens $EDITOR
spec edit ID                             # owner edit = instantly applied proposal
spec search "query"
spec graph ID [--impact] [--type T] [--depth N] [--archive] [--format dot|mermaid]
spec refs ID                             # reverse lookup
spec unmapped [PATH]                     # spec coverage of sources
spec bundle REF… [--budget N]            # shipped, default 2000: docs/canon/spec-cli-bundle.md; --task T: Phase 2

# owner queue: shipped, docs/canon/proposal-queue.md; a decided question or discrepancy: docs/canon/decision-record.md
spec propose update ID --base HASH --text-file F|- --rationale T
spec propose create TARGET [--base HASH] ...  # docs/canon/proposal-kinds.md
spec propose question|discrepancy …     # shipped: docs/canon/agent-intake.md
spec inbox [--all]                       # to come: --severity, --task
spec review PR-ID                        # to come: interactive edit/changes/defer
spec approve PR-ID [--note ...] [--option N | --answer T] [--canon REF]  # a terminal's [y/N]; no flag: the staged choice
spec reject PR-ID --reason ...          # --reason optional when a reject is staged (decision-staging)

# tasks: shipped (+ an agent's plan, claim, report, complete), docs/canon/tasks.md
spec task new --nodes REF… [--title ...]   # --contour: later
spec task show T | --next | list [--status S]
spec task approve T | changes T --note "..." | cancel T

# code and drift
spec symbols [--file F] [--unbound]
spec verify [--task T] [--tests] [--changed PATH]   # exit 0 ok · 1 Fix-level findings · 2 run cannot be trusted
spec bump [--editorial ID]                           # bump rev of changed nodes (pre-commit fails without it)
spec lock accept ID [--editorial]                   # accept code changes / an editorial edit
spec gate --worktree DIR --path FILE [--json] | --stdin  # shipped: docs/canon/gate.md

# checks, rounds, lifecycle
spec check [--staged | --changed] [--baseline F] [--debt]
spec round new [--limit N] | round answer FILE | --staged  # shipped: docs/canon/round.md
spec ship SLUG [--accepted]
spec compact --dry-run
spec export                                          # to come: generated/queue.md
spec export state [--out PATH]                       # shipped: the queue's JSONL dump, docs/canon/queue-backup.md
spec import-state FILE                               # shipped: restore it into an empty queue, a terminal's [y/N]
spec export index [--stdout]                         # [paths] index by its registered generator (Q3)
```

`--json` is available everywhere for scripts and the consumer project's tests.

## 3. HTTP (daemon)

- `127.0.0.1:7777`, a Host, Origin and Sec-Fetch-Site fence; no authentication, ever (ADR-0034). Built, reads and staging: `specengine-http` (`crates/specengine-http/README.md`).
- `GET /api/projects`, `/api/projects/:p/tree|nodes/{*ref}|search|bundle|graph|check|inbox|proposals/:id|tasks|tasks/:id|round` (built); `symbols|health` to come
- `POST`, `DELETE /api/projects/:p/proposals/:id/decision`: stage, unstage, `POST .../round/answers` (built), `/tasks/:id/transition`, `/nodes/:id` (owner edit): only staging, a terminal confirms (ADR-0035)
- `GET /api/projects/:p/events` -- **SSE**, the project's `events` (built; the UI updates live)
- `/mcp` — MCP Streamable HTTP (rmcp Tower service in axum), **after MVP** and only with GET/SSE (see §1.1)
- `/` — Web UI (embedded via `rust-embed`)

### Web UI — screens

UI in **English** (ADR-0014); spec content as is (the project's language), fonts and search handle non-Latin scripts. Data: `specengine-http` by default, the mock with `?scenario=` (`ui/README.md` "Contract seam").

| Screen | Content |
|---|---|
| **Home** | per project: tasks waiting for approval or changed since, the queue's counts and first items; a Cmd-K palette to a section, task, proposal, node or project (`docs/features/ui-home.md`) |
| **Tree** | hierarchy Project → Domain → Mechanic → Rule; statuses, `sync`, open-proposal counter; search |
| **Node** | rendered markdown, Source a toggle away (ADR-0036; editing: CodeMirror later); tabs: links (direct and reverse), bindings (symbol, signature, `sync`), history (git), proposals |
| **Graph** | `@xyflow/react` with a hand-written layered layout (no ELK or dagre; `docs/features/ui-graph.md`); filter by link type; "impact" mode |
| **Queue** | proposal and question cards: evidence, options with price, `@codemirror/merge` diff editable in place; a decision staged, its command to copy; hotkeys a/e/r/c/d |
| **Tasks** | a list by state, "Waiting for you" first, beside the task's plan, spec changes, proposals and runs; owner actions as commands to copy (`docs/features/ui-tasks.md`) |
| **Health** | "what is left", drift, budgets, W metrics |
| **Questions** | the round's sheet and its answers, staged at once, each confirmed on a terminal (`docs/canon/round-ui.md`); print, paste later |

## 4. Claude Code hooks (shipped by the plugin)

```jsonc
// hooks/hooks.json (fragment)
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command",
      "command": "specengine hook session-start" }] }],          // short summary: active task, open proposals (≤ 300 tok.)
    "PreToolUse": [{ "matcher": "Edit|Write|MultiEdit|NotebookEdit", "hooks": [{ "type": "command",
      "command": "sh \"${CLAUDE_PLUGIN_ROOT}/hooks/gate.sh\"", "timeout": 10 }] }],  // built
    "PostToolUse": [{ "matcher": "Edit|Write|MultiEdit", "hooks": [{ "type": "command",
      "command": "specengine hook touched --stdin" }] }],         // fast reindex of the file, code_ahead warning
    "SubagentStop": [{ "hooks": [{ "type": "command",
      "command": "specengine hook subagent-stop --stdin" }] }]    // reminder: markers, report_discrepancy, report_run
  }
}
```

Built (`plugin-gate`): only `PreToolUse`, its wrapper running `spec gate --stdin` (`docs/canon/gate.md`); the other three later. `Bash` is not gated: hooks cannot fully close shell writes, a limit stated openly.

## 5. `specengine.toml` (at the project repository root)

```toml
[project]
slug = "example"
name = "Example"
language = "en"
profile = "example-stack"   # optional; opaque, passed through in the task package (ADR-0027)

[paths]                     # role keys, roots, exclude: crates/specengine-core/README.md
roots = ["docs", "CLAUDE.md"]
tier0 = "CLAUDE.md"          # spec check slots: the only canon tier 0,
tier1_name = "README.md"    # the only canon tier 1 name,
index = "docs/index.md"     # the index, capped by index_bytes
link_base = "docs"          # fallback base of relative file links: docs/canon/spec-check-links.md

[ids]                       # prefix → kind, shape, width, aliases_from: crates/specengine-model/README.md
R    = { kind = "requirement", width = 2, immutable_text = true }

[budgets]                   # document caps in bytes (tier0_bytes …), [classes], [check]: docs/canon/spec-check.md;
                            # [[generators]] (the generated-document registry): docs/canon/spec-check-graph.md
bundle_node = 2000          # bundle budgets in estimated tokens (default 2000): docs/canon/spec-cli-bundle.md
bundle_task = 10000         # Phase 2, --task

[zones]                     # for the gate and role write rules
code  = ["src/**"]
data  = ["data/**"]
tests = ["tests/**"]
docs  = ["docs/**", "CLAUDE.md"]

[gate]                      # built: docs/canon/gate.md "Config"
mode = "selective"          # off | observe | selective (strict later): ONLY that an approved task covers the write (ADR-0012)
zones = ["code"]            # min_files, contours: later

[code]
languages = ["rust", "ron"]
crates = "cargo-metadata"
bevy = true
scip = false                # enable enrichment via rust-analyzer scip
```
