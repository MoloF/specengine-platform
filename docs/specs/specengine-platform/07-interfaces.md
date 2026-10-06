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
- A status header, a "Delta — what changed since the last request" section and "Hints — what to ask next" (as in tracey); `compact: bool` on reading tools (detail levels in the spirit of code-graph-mcp), on measured need.
- **Primary transport: stdio via `spec mcp`** in `.mcp.json`, a bridge to the daemon (Phase 2; today the `specengine-mcp` binary). Reason: Claude Code (≥ 2.1.84) issues a GET to the MCP HTTP endpoint to open an SSE stream, and a purely stateless 2026-07-28 server answers 405, so the connection drops (claude-code#39790 closed as "not planned"). HTTP `/mcp` is added later, with GET/SSE and `legacy_session_mode`.
- State is passed only via explicit handles (`task_id`, `proposal_id`).
- **Long operations** (reindex, verify) may run past 120 s: Claude Code backgrounds the call and delivers its result as a notification (MCP README), so there is no polling tool and no "call again" protocol. A long-blocking "wait for approval" stays excluded.

### 1.2. Tools (`core` set)

`get_tree`, `get_node`, `search`, `get_context_bundle` ship as reads over the CLI (canon `docs/canon/mcp-read.md`). Still to come for them: `get_tree`'s `sync` and open-proposal counters, `get_node` `with: bindings | history | proposals`, `get_context_bundle {task_id}` and its log (05 §6). Intake (`propose_change`, `report_discrepancy`, `ask_question`, `get_proposal`) shipped: `docs/canon/agent-intake.md`; `task_id` on them: `task-package`; a question or discrepancy approved as a decision record shipped: `docs/canon/decision-record.md`; kinds `create`, `decision`, `interpretation`, `amendment`: `proposal-kinds`.

| Tool | Input | Output | Notes |
|---|---|---|---|
| `find_symbols` | `query` \| `node_id` \| `file` \| `path:line` | qpath, kind, Bevy layer, signature, file:lines, bound nodes | repo-map on demand; general code navigation the agent does with the `LSP` tool of the code-intelligence plugin (04 §1.7), SpecEngine does not duplicate it |
| `get_impact` | `node_id` \| `qpath` \| `since: <commit>` | nodes, symbols, tests, tasks within the radius | graph + bindings; `since` = git diff → graph walk (change impact) |
| `refs` | `node_id` | reverse lookup: all markers, tests, records referring to the node | needed **during** refactoring |
| `unmapped` | `path?` | source tree with spec coverage percentage | where there is no spec (tracey `query unmapped`) |
| `get_task` | `task_id` \| `next: true` | the task package (versioned, ADR-0027): `structuredContent` = status, goal, plan, targets, criteria, assumptions, open proposals, owner comments, bindings, `spec_snapshot` + diff, `bundle_hash`, `profile`; `content` = the neutral brief | `next` — first `ready` by priority (as `bd ready` / `next_task`) |
| `claim_task` | `task_id`, `role`, `worktree` | ok \| refusal with reason | only `ready`; records the run in `runs` |
| `submit_plan` | `task_id`, `plan_md`, `criteria[]`, `affected_nodes[]` | task status | analyst; → `review` (waits for the owner) |
| `check_binding` | `node_id`, `qpath` | whether the marker resolves, `ast_hash`, `sync` | formerly `bind_code_symbol`, check only |
| `report_run` | `task_id`, `outcome`, `summary`, `changed_files[]` | ok | `verified` is set by `spec verify`, not by this call |

**Task package** (ADR-0027). One type in `specengine-model`; its JSON Schema is generated and pinned by a test. `schema_version`: a new key keeps it; a removed, renamed or re-meant key raises it. Every key is always present, absent = `null`; `compact` shortens only the Markdown `content`, never the key set. `spec task show T --json` emits the same document. The bundle body comes by reference (`bundle_hash` → `get_context_bundle`): a 10k-token bundle collides with the 48,000-character cap. `claim_task.role` is the project's own role name, stored verbatim (no enum). Stack wording, tracker tickets and routing belong to the project's skills (06 §8); a task stores no ticket key.

Phase 2 contract checks (the `task-package` feature spec expands them): P2-1 `get_task` `structuredContent` = `spec task show --json`; P2-2 the pinned schema snapshot fails on a key removed or renamed without a bump; P2-3 a source scan (as `crates/specengine-core/tests/check_genre.rs`) finds no `cargo`, `nextest`, `clippy`, `bevy`, `pnpm`, `npm`, `nest`, `react`, `jira` or this repository's role names in the package and brief sources or `plugin/**`; P2-4 fixtures `spec-a` and `spec-b` give the same key set and `schema_version`; P2-5 a synthetic non-Rust fixture whose records hold no stack words yields none in the JSON or `content`; P2-6 changing `profile` changes only that value; P2-7 `claim_task` with `role = "nest-developer"` succeeds, stored verbatim; P2-8 no key starts with `block` (ADR-0012); P2-9 two projects in one daemon never see each other's tasks, deleting one database leaves the other intact, nothing is written to SpecEngine's repository; P2-10 `get_task` at the 10k budget stays under 48,000 characters; P2-11 the package alone carries the verbatim title, goal, criteria text, target titles and open questions; P2-12 a project without a profile runs `get_task` → `claim_task` → `report_run` → `complete_task` on generic prompts at MCP level.

**Human tools** (`owner` set, `_meta["anthropic/requiresUserInteraction"]: true`):

| Tool | What it does |
|---|---|
| `review_proposal` | MRTR/elicitation: an "option / comment / decision" form or URL mode to the UI card. The human writes the decision |
| `approve_task` | same, for moving a task to `ready` |

The MCP server remembers nothing between calls: the owner's decision is stored by the proposal queue when given, never assumed remembered by the server; a `cancel` records nothing and leaves the proposal pending (the agent does not re-open the form on its own). **A consent tool must** (none of this is in the Phase 0 `review_proposal` skeleton yet): bind the sealed `requestState`'s associated data to the proposal revision / patch hash; carry a single-use nonce persisted in the queue; expire (TTL); share one `requestState` key across processes once a multi-process HTTP server exists; on cancellation send `notifications/cancelled` for the outstanding `elicitation/create`.

### 1.3. Resources (for `@`-mentions)

`spec://{project}/tree` and the template `spec://{project}/node/{id}` ship (canon `docs/canon/mcp-read.md` "Resources"). To come: `spec://{project}/task/{id}` — task package (§1.2); `spec://{project}/inbox` — open proposals.

### 1.4. Prompts (= slash commands in Claude Code)

Plugin skills carry them (`plugin-skills`: `read-spec`, `ask-owner`, `propose-spec-change`). Reserved for tasks: `/specengine:analyze <task>` (analyst, 06 §2), `implement <task>` (markers, `report_discrepancy`), `prepare-task <node…>` (a task draft), `round` (the owner's question sheet, 06 §5).

### 1.5. Notifications

`subscriptions/listen` (2026-07-28) / `list_changed`: the server reports "proposal decided" and "task approved". ⚠ Per-URI subscription support in Claude Code is unconfirmed, so rely on polling `get_proposal`/`get_task` at the start of a step.

## 2. CLI (`spec`)

```
# projects and index
spec init [--import IMPORTER]            # specengine.toml, layout, migration (08 §4)
spec index [--full]                      # rebuild the index (incremental by BLAKE3)
spec serve [--port 7777]                 # daemon: HTTP + SSE + MCP(HTTP) + UI + watcher
specengine-http --root DIR... [--port N]  # built: its read surface, crates/specengine-http/README.md
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
spec propose question|discrepancy …     # shipped: docs/canon/agent-intake.md
spec inbox [--all]                       # to come: --severity, --task
spec review PR-ID                        # to come: interactive edit/changes/defer
spec approve PR-ID [--note ...] [--option N | --answer T] [--canon REF]  # a terminal's [y/N]
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
spec check [--staged | --changed] [--baseline F] [--debt]
spec round new | round answer FILE
spec ship SLUG [--accepted]
spec compact --dry-run
spec export                                          # to come (task-package): generated/queue.md
spec export state [--out PATH]                       # shipped: the queue's JSONL dump, docs/canon/queue-backup.md
spec import-state FILE                               # shipped: restore it into an empty queue, a terminal's [y/N]
spec export index [--stdout]                         # [paths] index by its registered generator (Q3)
```

`--json` is available everywhere for scripts and the consumer project's tests.

## 3. HTTP (daemon)

- `127.0.0.1:7777`, `Origin` check; a local token (`~/.config/specengine/token`) with the first write endpoint. Built, reads only: `specengine-http` (`crates/specengine-http/README.md`), a Host, Origin and Sec-Fetch-Site fence.
- `GET /api/projects`, `/api/projects/:p/tree|nodes/{*ref}|search|bundle|inbox|proposals/:id` (built); `graph|tasks|symbols|health` to come
- `POST /api/projects/:p/proposals/:id/decision` (today always 403: decided on a terminal), `/tasks/:id/transition`, `/nodes/:id` (owner edit)
- `GET /api/projects/:p/events` -- **SSE**, the project's `events` (built; the UI updates live)
- `/mcp` — MCP Streamable HTTP (rmcp Tower service in axum), **after MVP** and only with GET/SSE (see §1.1)
- `/` — Web UI (embedded via `rust-embed`)

### Web UI — screens

UI in **English** (ADR-0014); spec content is shown as is (the project's language), fonts and search handle non-Latin scripts. Data: `specengine-http` by default, the mock with `?scenario=` (`ui/README.md` "Contract seam").

| Screen | Content |
|---|---|
| **Home** | per project: tasks waiting for approval or changed since, the queue's counts and first items; a Cmd-K palette to a section, task, proposal, node or project (`docs/features/ui-home.md`) |
| **Tree** | hierarchy Project → Domain → Mechanic → Rule; statuses, `sync`, open-proposal counter; search |
| **Node** | CodeMirror (markdown) + preview; tabs: links (direct and reverse), bindings (symbol, signature, `sync`), history (git), proposals |
| **Graph** | `@xyflow/react` with a hand-written layered layout (no ELK or dagre; `docs/features/ui-graph.md`); filter by link type; "impact" mode |
| **Queue** | proposal and question cards: evidence, options with price, `@codemirror/merge` diff editable in place; hotkeys a/e/r/c/d |
| **Tasks** | a list by state, "Waiting for you" first, beside the task's plan, spec changes, proposals and runs; owner actions as commands to copy (`docs/features/ui-tasks.md`) |
| **Health** | "what is left", drift, budgets, W metrics |
| **Round** | question sheet in domain language for printing and sending + answer paste |

## 4. Claude Code hooks (shipped by the plugin)

```jsonc
// hooks/hooks.json (fragment)
{
  "hooks": {
    "SessionStart": [{ "hooks": [{ "type": "command",
      "command": "specengine hook session-start" }] }],          // short summary: active task, open proposals (≤ 300 tok.)
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
