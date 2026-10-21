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
- **Primary transport: stdio** (`specengine-mcp` in `.mcp.json`; a `spec mcp` bridge to the daemon on a trigger, 05 §1 (6)): Claude Code (≥ 2.1.84) GETs the MCP HTTP endpoint for an SSE stream, a stateless 2026-07-28 server answers 405 and the connection drops (claude-code#39790, "not planned"). HTTP `/mcp` comes later, with GET/SSE and `legacy_session_mode`.
- State passes only via explicit handles (`task_id`, `proposal_id`).
- **Long operations** (reindex, verify) may run past 120 s: Claude Code backgrounds the call, its result a notification (MCP README): no polling tool, no "call again" protocol, no long-blocking "wait for approval".

### 1.2. Tools (`core` set)

`get_tree`, `get_node`, `search`, `get_context_bundle` ship as reads over the CLI (`docs/canon/mcp-read.md`); `find_symbols`, `refs`, `unmapped` as reads of the code index, `check_binding` as `spec verify --node` (`docs/canon/mcp-code.md`). Still to come for them: `get_tree`'s `sync` and open-proposal counters, `get_node` `with: history | proposals` (`bindings` built: `docs/canon/code-reads.md`), `get_context_bundle {task_id}` and its log (05 §6). Intake (`propose_change`, `report_discrepancy`, `report_decision`, `ask_question`, `get_proposal`) shipped: `docs/canon/agent-intake.md`; `report_drift`: `docs/canon/proposal-drift.md`; `task_id` on them: `tasks.md`; a deciding item approved as a decision record shipped: `docs/canon/decision-record.md`; kinds `create`, `interpretation`, `decision`, `amendment`: `docs/canon/proposal-*.md`.

| Tool | Input | Output | Notes |
|---|---|---|---|
| `find_symbols` | one of `query` (+ `limit`), `node`, `file`, `at` (`PATH:LINE`) | `spec symbols --json`: qpath, item kind, signature, lines, layer, tier, bindings with their nodes, roles with `[code] frameworks` | shipped; repo-map on demand; roles: `docs/canon/code-roles.md`; general code navigation the agent does with the `LSP` tool of the code-intelligence plugin (04 §1.7), SpecEngine does not duplicate it |
| `get_impact` | one of `id`, `symbol`, `since` (a git revision) (+ `depth`) | `spec impact --json`: the nodes the impact walk reaches, their bindings with each `sync`, tests, this repository's open tasks on them | shipped (7b-1, 7b-2 `since`: the code changed since a revision against the disk): `docs/canon/spec-impact.md` |
| `refs` | `id` | `spec refs --json`: the node's markers and mentions, each with its binding | shipped; needed **during** refactoring; spec-side referrers: `get_node` with links |
| `unmapped` | `path?` | `spec unmapped --json`: rows of units, mapped, percent, mentioned; a file's unmapped items | shipped; where there is no spec (tracey `query unmapped`) |
| `get_roadmap` | `release?` \| `pool?` | `spec roadmap --json`: the plan's groups and items with `waits_on` and their tasks | shipped; capped as the CLI (`docs/canon/spec-roadmap.md` "Surfaces") |
| `get_task` | `task_id` \| `next: true` | the task package (versioned, ADR-0027): `structuredContent` = status, goal, plan, targets, criteria, assumptions, open proposals, owner comments, bindings, `spec_snapshot` + diff, `bundle_hash`, `profile`, `roadmap`; `content` = the neutral brief | shipped; `next`: the first `ready` by the plan (`docs/canon/spec-roadmap.md` "Next") |
| `claim_task` | `task_id`, `role`, `worktree` | ok \| refusal with reason | shipped; only `ready`; opens a run |
| `submit_plan` | `task_id`, `plan_md`, `criteria[]`, `affected_nodes[]` | task status | shipped; -> `review` |
| `check_binding` | `node`, `symbol?` | `spec verify --node --json`: each binding's state, class, levels and anchor, the counts, `delta`, the verdict | shipped; formerly `bind_code_symbol`, check only; the session's worktree, no base (`docs/canon/mcp-code.md`) |
| `report_run` | `task_id`, `outcome`, `summary`, `changed_files[]` | ok | shipped; `verified` comes from the owner's `spec verify --tests` evidence (`check_binding`'s `tested`), never this call |
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

Plugin skills carry them: `read-spec`, `ask-owner`, `propose-spec-change`; `/specengine:analyze <task>` (the plan, 06 §2), `implement <task>` (claim, run, completion); owner-only `/specengine:feature` (06 §8). Reserved: `prepare-task <node…>` (a task draft), `round` (the owner's question sheet, 06 §5).

### 1.5. Notifications

`subscriptions/listen` (2026-07-28) / `list_changed`: the server reports "proposal decided" and "task approved". ⚠ Claude Code's per-URI subscriptions are unconfirmed: poll `get_proposal`/`get_task` at a step's start.

## 2. CLI (`spec`)

```
# projects and index
spec init [--import IMPORTER]            # specengine.toml, layout, migration (08 §4)
spec index [--full]                      # the index (incremental by BLAKE3), + the code index: docs/canon/symbol-index.md
spec serve [DIR...] [--port 7777]        # built: execs specengine-http; canon/serve.md
specengine-http --root DIR... [--port N]  # built: reads, staging; crates/specengine-http/README.md
spec mcp                                 # on a trigger: MCP stdio bridge

# tree and nodes
spec tree [ROOT] [--depth N] [--kind K] [--archive]  # shipped: docs/canon/spec-cli-graph.md
spec show ID [--links [--archive]] [--bindings] [--history]  # --bindings shipped: docs/canon/code-reads.md
spec new KIND [--parent ID] [TITLE]      # unbuilt: the registry issues the ID, opens $EDITOR
spec edit ID                             # unbuilt: owner edit = instantly applied proposal
spec search "query"
spec graph ID [--impact] [--type T] [--depth N] [--archive] [--format dot|mermaid]
spec impact REF | --symbol QPATH | --since REV [--depth N] [--json]  # shipped: docs/canon/spec-impact.md
spec refs REF [--json]                   # reverse lookup from the code index, shipped: docs/canon/code-bindings.md
spec unmapped [PATH]                     # spec coverage of sources
spec bundle REF… [--budget N]            # shipped, default 2000: docs/canon/spec-cli-bundle.md; --task T: Phase 2

# owner queue: shipped, docs/canon/proposal-queue.md; a decided question or discrepancy: docs/canon/decision-record.md
spec propose update ID --base HASH --text-file F|- --rationale T
spec propose create TARGET [--base HASH] ...  # docs/canon/proposal-kinds.md
spec propose interpretation ID --decision REF ...  # proposal-interpretation.md
spec propose question|discrepancy …     # shipped: docs/canon/agent-intake.md
spec propose drift [REF…] [--task T]     # shipped: docs/canon/proposal-drift.md
spec inbox [--all]                       # to come: --severity, --task
spec review PR-ID                        # to come: interactive edit/changes/defer
spec approve PR-ID [--note ...] [--option N | --answer T] [--canon REF] [--editorial]  # a terminal's [y/N]; no flag: the staged choice
spec reject PR-ID --reason ... [--follow-up code|spec]  # --reason optional when a reject is staged (decision-staging); --follow-up: a drift item's draft task (proposal-completion.md)

# tasks: shipped (+ an agent's plan, claim, report, complete), docs/canon/tasks.md
spec task new --nodes REF… [--title ...]   # --contour: later
spec task show T | --next | list [--status S]
spec task approve T | changes T --note "..." | cancel T

# code and drift
spec symbols QUERY | --node REF | --file F | --at F:L   # shipped: docs/canon/symbol-reads.md; unbound nodes: spec health
spec verify [--changed | --staged] [--json]         # shipped: docs/canon/spec-verify.md
spec verify --task T [--changed] [--json]           # shipped: docs/canon/spec-verify-scope.md; the task's nodes' rows
spec verify --node REF [--symbol QPATH] [--json]    # shipped: docs/canon/spec-verify-scope.md; one node's files
spec verify --tests [--task T] [--json]             # shipped: docs/canon/spec-verify-tests.md; the owner's terminal, [y/N]
spec bump [--staged] [--json]                       # read-only: nodes to raise (shipped: docs/canon/node-revision.md)
spec lock accept REF [--editorial]                  # shipped: docs/canon/spec-lock-accept.md
spec lock regen                                     # a merge conflicting in spec.lock: docs/canon/spec-lock.md
spec lock rebase [--json]                           # an older format or code_recipe (shipped: docs/canon/spec-lock-rebase.md)
spec gate --worktree DIR --path FILE [--json] | --stdin  # shipped: docs/canon/gate.md
spec resolve [--timeout S] [--json]                 # layer C on request (shipped: docs/canon/layer-c.md, ADR-0049)

# checks, rounds, lifecycle
spec check [--staged | --changed] [--baseline F] [--debt]
spec health                                          # shipped: docs/canon/health.md, rows 4-5 health-code.md
spec roadmap [--release R | --pool] [--json]         # shipped: docs/canon/spec-roadmap.md; the plan items' path and pool
spec round new [--limit N] | round answer FILE | --staged  # shipped: docs/canon/round.md
spec ship SLUG [--accepted]
spec compact --dry-run
spec export                                          # to come: generated/queue.md
spec export state [--out PATH]                       # shipped: the queue's JSONL dump, docs/canon/queue-backup.md
spec import-state FILE                               # shipped: restore it into an empty queue, a terminal's [y/N]
spec export index [--stdout]                         # [paths] index, its shards and roadmap view by its registered generator (Q3)
```

`--json` is available everywhere for scripts and the consumer project's tests.

## 3. HTTP (daemon)

- `127.0.0.1:7777`, a Host, Origin and Sec-Fetch-Site fence; no authentication, ever (ADR-0034). Built, reads and staging: `specengine-http` by `spec serve` (`docs/canon/serve.md`).
- `GET /api/projects`, `/api/projects/:p/tree|nodes/{*ref}|search|bundle|graph|check|health|roadmap|inbox|proposals/:id|tasks|tasks/:id|round` (built); `symbols` to come
- `POST`, `DELETE /api/projects/:p/proposals/:id/decision`: stage, unstage, `POST .../round/answers` (built), `/tasks/:id/transition`, `/nodes/:id` (owner edit): only staging, a terminal confirms (ADR-0035)
- `GET /api/projects/:p/events` -- **SSE**, the project's `events` (built; the UI updates live)
- `/mcp` — MCP Streamable HTTP (rmcp Tower service in axum), on a trigger and only with GET/SSE (see §1.1)
- `/`, `/assets/<file>` -- the Web UI compiled in (built: `docs/canon/serve-ui.md`)

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
| **Roadmap** | the plan, read-only: a lane per release (current, the MVP cut, range bars), the pool with its tasks, unplaced items; each card's waits and executing tasks; a lazy chain canvas, Lanes its text twin (`docs/canon/spec-roadmap-screen.md`) |
| **Health** | "what is left" (rows 4-5 from `spec health`: `docs/canon/health-code.md`; drawn by `health-code-ui`), drift, budgets, W metrics |
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

Built (`plugin-gate`): only `PreToolUse`, its wrapper running `spec gate --stdin` (`docs/canon/gate.md`); the other three later. `Bash` is not gated: hooks cannot fully close shell writes, a stated limit.

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
crates = "cargo-metadata"    # refused, not built: only if 6a-2's TE-8 roles differ (docs/features/symbol-index.md Q2)
frameworks = ["bevy"]       # built: docs/canon/code-roles.md (opt-in role facts; `bevy = true` is an unknown key)
scip = false                # retired (ADR-0049): an unknown key; layer C is `spec resolve` (docs/canon/layer-c.md)

[verify]                    # built: docs/canon/spec-verify.md (mode), docs/canon/spec-verify-tests.md (tests, timeout)
mode = "observe"            # observe | enforce-introduced | enforce
tests = ["scripts/run-test", "{package}", "{target_kind}", "{target}", "{test}"]  # argv, no shell; run only by the owner
timeout = 600               # seconds per invocation, 1..86400
```
