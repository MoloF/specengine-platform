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

- **Few tools, good `instructions`** (≤ 2,048 characters: Claude Code truncates, and tool search defers definitions). Tool-set levels as in Task Master: `core` (agents by default) and `admin`.
- Responses are **prose for the model** with a status header, a "Delta — what changed since the last request" section and "Hints — what to ask next" (as in tracey); the machine part lives in `structuredContent`.
- All reading tools carry `readOnlyHint`; each has `compact: bool` (detail levels in the spirit of code-graph-mcp); a response stays well under ~48,000 characters and paginates beyond that — Claude Code's cap counts characters and `MAX_MCP_OUTPUT_TOKENS` does not raise it (04 §4).
- Input schemas are flat JSON Schema 2020-12, no root `anyOf/oneOf`; there is `outputSchema` + `structuredContent`, and `content` holds Markdown for the model.
- **Primary transport is stdio** (`spec mcp` in `.mcp.json`, a bridge to the daemon). Reason: Claude Code (≥ 2.1.84) issues a GET to the MCP HTTP endpoint to open an SSE stream, and a purely stateless 2026-07-28 server answers 405, so the connection drops (claude-code#39790 closed as "not planned"). Besides, Claude Code still sends the legacy `initialize`. HTTP `/mcp` is added later, with GET/SSE and `legacy_session_mode`. A stdio-only build does not pull the HTTP stack into rmcp.
- Both protocol eras are supported (legacy `initialize` and 2026-07-28 stateless); the server detects the era from the client's first message (04 §4). State is passed only via explicit handles (`task_id`, `proposal_id`).
- **Long operations** (reindex, verify) may run past 120 s: Claude Code backgrounds the call and delivers its result as a notification (04 §4), so there is no polling tool and no "call again" protocol. A long-blocking "wait for approval" stays excluded.
- `rmcp` is pinned exactly (3 major versions in 7 months). Output schemas are generated from `Json<T>`/`Parameters<T>` + schemars. If Tasks are needed, the TTL is set explicitly: the default is 5 minutes, and a full reindex will not fit.
- **All tools are deterministic**: one state gives one response, no LLM inside (as in cgr, stated in every tool description).
- **No agent tool writes to spec files** (the "one door" test on a temporary repository).

### 1.2. Tools (`core` set)

| Tool | Input | Output | Notes |
|---|---|---|---|
| `get_tree` | `root?`, `depth?`, `kinds?` | id, kind, title, statuses, `sync`, open-proposal counters | no node bodies |
| `get_node` | `id`, `with: [links, bindings, history, proposals]` | full section text + the selected extras | read-more by id |
| `search` | `query`, `kinds?`, `limit?` | id, title, snippet | FTS5; "search instead of list-all" |
| `get_context_bundle` | `task_id` \| `node_ids[]`, `budget?` | Markdown bundle + `bundle_hash` + "left out" tail | 05 §6; deterministic, logged |
| `find_symbols` | `query` \| `node_id` \| `file` \| `path:line` | qpath, kind, Bevy layer, signature, file:lines, bound nodes | repo-map on demand; general code navigation the agent does with the `LSP` tool of the code-intelligence plugin (04 §1.7), SpecEngine does not duplicate it |
| `get_impact` | `node_id` \| `qpath` \| `since: <commit>` | nodes, symbols, tests, tasks within the radius | graph + bindings; `since` = git diff → graph walk (change impact) |
| `refs` | `node_id` | reverse lookup: all markers, tests, records referring to the node | needed **during** refactoring |
| `unmapped` | `path?` | source tree with spec coverage percentage | where there is no spec (tracey `query unmapped`) |
| `get_task` | `task_id` \| `next: true` | brief, status, blockers, owner comments, spec diff against `spec_snapshot` | `next` — first `ready` by priority (as `bd ready` / `next_task`) |
| `claim_task` | `task_id`, `role`, `worktree` | ok \| refusal with reason | only `ready`; records the run in `runs` |
| `submit_plan` | `task_id`, `plan_md`, `criteria[]`, `affected_nodes[]` | task status | analyst; → `review` (waits for the owner) |
| `report_discrepancy` | `task_id?`, `node_ids[]`, `gap_type`, `severity`, `working_answer`, `summary`, `evidence[]`, `options[]`, `recommendation`, `proposed_patch?` | `proposal_id` \| "already decided: DEC-…" | 06 §3.2 |
| `ask_question` | `node_ids[]`, `text`, `working_answer`, `price_of_other?`, `severity` | `Q-id` \| "already answered" | deduplication |
| `propose_change` | `kind: update\|create\|decision\|interpretation`, `target`, `patch`, `rationale` | `proposal_id` + diagnostics | validated before display |
| `get_proposal` | `proposal_id` | status, decision, owner comment | to continue after the decision |
| `check_binding` | `node_id`, `qpath` | whether the marker resolves, `ast_hash`, `sync` | formerly `bind_code_symbol`, check only |
| `report_run` | `task_id`, `outcome`, `summary`, `changed_files[]` | ok | `verified` is set by `spec verify`, not by this call |

**Human tools** (`owner` set, `_meta["anthropic/requiresUserInteraction"]: true`):

| Tool | What it does |
|---|---|
| `review_proposal` | MRTR/elicitation: an "option / comment / decision" form or URL mode to the UI card. The human writes the decision |
| `approve_task` | same, for moving a task to `ready` |

The MCP server remembers nothing between calls: the owner's decision is stored by the proposal queue when given, never assumed remembered by the server; a `cancel` records nothing and leaves the proposal pending (the agent does not re-open the form on its own). **A consent tool must** (none of this is in the Phase 0 `review_proposal` skeleton yet): bind the sealed `requestState`'s associated data to the proposal revision / patch hash; carry a single-use nonce persisted in the queue; expire (TTL); share one `requestState` key across processes once a multi-process HTTP server exists; on cancellation send `notifications/cancelled` for the outstanding `elicitation/create`.

### 1.3. Resources (for `@`-mentions)

- `spec://{project}/tree` — tree without bodies.
- `spec://{project}/node/{id}` — node (RFC 6570 template).
- `spec://{project}/task/{id}` — task brief.
- `spec://{project}/inbox` — open proposals.

⚠ An `@`-mention inserts content without a tool call, so hooks do not fire. Irrelevant for the gate: resources are read-only.

### 1.4. Prompts (= slash commands in Claude Code)

- `/specengine:analyze <task>` — analyst scenario (06 §2).
- `/specengine:implement <task>` — developer scenario with marker rules and `report_discrepancy`.
- `/specengine:prepare-task <node…>` — create a task draft from nodes.
- `/specengine:round` — build the question sheet for the owner (06 §5).

### 1.5. Notifications

`subscriptions/listen` (2026-07-28) / `list_changed`: the server reports "proposal decided" and "task approved". ⚠ Per-URI subscription support in Claude Code is unconfirmed, so rely on polling `get_proposal`/`get_task` at the start of a step.

## 2. CLI (`spec`)

```
# projects and index
spec init [--import IMPORTER]            # specengine.toml, layout, migration (08 §4)
spec index [--full]                      # rebuild the index (incremental by BLAKE3)
spec serve [--port 7777]                 # daemon: HTTP + SSE + MCP(HTTP) + UI + watcher
spec mcp                                 # MCP over stdio (for .mcp.json)

# tree and nodes
spec tree [ROOT] [--depth N] [--kind mechanic]
spec show ID [--links --bindings --history]
spec new KIND [--parent ID] [TITLE]      # the registry issues the ID, opens $EDITOR
spec edit ID                             # owner edit = instantly applied proposal
spec search "query"
spec graph ID [--impact] [--format dot|mermaid]
spec refs ID                             # reverse lookup
spec unmapped [PATH]                     # spec coverage of sources
spec bundle (--task T | ID…) [--budget 10000]

# owner queue
spec inbox [--severity high] [--task T]
spec review PR-ID                        # interactive: approve/edit/reject/changes/defer
spec approve PR-ID [--option N] [--note ...]
spec reject PR-ID --reason ...

# tasks
spec task new --nodes ID… [--title ...] [--contour feature]
spec task show T | list [--status ready]
spec task approve T | changes T --note "..." | cancel T

# code and drift
spec symbols [--file F] [--unbound]
spec verify [--task T] [--tests] [--changed PATH]   # exit 0 ok · 1 Fix-level findings · 2 run cannot be trusted
spec bump [--editorial ID]                           # bump rev of changed nodes (pre-commit fails without it)
spec lock accept ID [--editorial]                   # accept code changes / an editorial edit
spec gate --worktree DIR --path FILE                 # for the PreToolUse hook; exit 2 = block

# checks, rounds, lifecycle
spec check [--baseline .spec-debt.toml] [--debt] [--changed] [--staged]
spec round new | round answer FILE
spec ship SLUG [--accepted]
spec compact --dry-run
spec export [--state]                                # generated/queue.md and state.jsonl
spec export index [--stdout]                         # [paths] index by its registered generator (Q3)
```

`--json` is available everywhere for scripts and the consumer project's tests.

## 3. HTTP (daemon)

- `127.0.0.1:7777`, local token (`~/.config/specengine/token`), `Origin` check.
- `GET /api/projects/:p/tree|nodes/:id|search|graph|inbox|tasks|symbols|health`
- `POST /api/projects/:p/proposals/:id/decision`, `/tasks/:id/transition`, `/nodes/:id` (owner edit)
- `GET /api/events` — **SSE** stream of events from the `events` table (the UI updates live)
- `/mcp` — MCP Streamable HTTP (rmcp Tower service in axum), **after MVP** and only with GET/SSE (see §1.1)
- `/` — Web UI (embedded via `rust-embed`)

### Web UI — screens

UI in **English** (ADR-0014); spec content is shown as is (the project's language), fonts and search handle non-Latin scripts.

| Screen | Content |
|---|---|
| **Tree** | hierarchy Project → Domain → Mechanic → Rule; statuses, `sync`, open-proposal counter; search |
| **Node** | CodeMirror (markdown) + preview; tabs: links (direct and reverse), bindings (symbol, signature, `sync`), history (git), proposals |
| **Graph** | `@xyflow/react` + ELK/dagre; filter by link type; "impact" mode |
| **Queue** | proposal and question cards: evidence, options with price, `@codemirror/merge` diff editable in place; hotkeys a/e/r/c/d |
| **Tasks** | board: draft → analysis → review ⇄ changes_requested → ready → in_progress → in_review → done → accepted; brief, plan, open proposals, runs |
| **Health** | "what is left", drift, budgets, W metrics |
| **Round** | question sheet in domain language for printing and sending + answer paste |

## 4. Claude Code hooks (shipped by the plugin)

```jsonc
// hooks/hooks.json (fragment)
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command",
      "command": "specengine hook session-start" }] }],          // short summary: active task, blockers (≤ 300 tok.)
    "PreToolUse": [{ "matcher": "Edit|Write|MultiEdit", "hooks": [{ "type": "command",
      "command": "specengine gate --stdin" }] }],                 // exit 2 = block; daemon down → exit 2
    "PostToolUse": [{ "matcher": "Edit|Write|MultiEdit", "hooks": [{ "type": "command",
      "command": "specengine hook touched --stdin" }] }],         // fast reindex of the file, code_ahead warning
    "SubagentStop": [{ "hooks": [{ "type": "command",
      "command": "specengine hook subagent-stop --stdin" }] }]    // reminder: markers, report_discrepancy, report_run
  }
}
```

`gate` reads the hook JSON from stdin (file path, cwd), determines the worktree and the active task, and checks the path against `zones` from the config. For `Bash` there is a separate matcher with `if:` on write commands; writes via the shell cannot be fully closed by hooks, and this limitation must be stated openly.

## 5. `specengine.toml` (at the project repository root)

```toml
[project]
slug = "example"
name = "Example"
language = "en"

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
bundle_node = 2000          # bundle budgets in tokens, multilingual estimator
bundle_task = 10000

[zones]                     # for the gate and role write rules
code  = ["src/**"]
data  = ["data/**"]
tests = ["tests/**"]
docs  = ["docs/**", "CLAUDE.md"]

[gate]
mode = "selective"          # off | observe | selective | strict — checks ONLY that an approved task exists (ADR-0012); observe: stops nothing
min_files = 3
contours = ["feature", "feature→content"]

[code]
languages = ["rust", "ron"]
crates = "cargo-metadata"
bevy = true
scip = false                # enable enrichment via rust-analyzer scip
```
