---
class: canon
tier: 1
scope: [crates/specengine-mcp]
owner: owner
reviewed: 2026-11-11
---

# specengine-mcp — the MCP server over stdio, and a hub's

One process serves one client over stdin/stdout on `rmcp =3.5.0` (features `server`, `macros`, `transport-io`; not `request-state`, unused since the unsealed `requestState`; not `elicitation`, which pulls `url` / ICU: wire messages go through `send_cancellable_request`). The crate names no HTTP crate, in its manifest or its own graph (`cargo tree -p`); a workspace build unifies rmcp's features, so the binary built beside the daemon compiles rmcp's HTTP transport too, unused. A hub's daemon serves its hub mode at `/mcp/<slug>` (`hub-mcp.md`). The default build serves 30 tools (read, code, queue, task, card, the owner's form `review_proposal`) and the resources, each one call into the CLI library (the sets: `mcp-read.md` "Binary"). Contracts: reads (parity, schemas, size, latency) `mcp-read.md`; code `mcp-code.md`; queue `agent-intake.md`; tasks `task-package.md` "MCP"; the form `review-form.md`; a `target` or `node_ids` item may be a root-relative `.md` path (`proposal-queue.md` "Creation" 1). Reads refresh the project's index in SpecEngine's data directory as the CLI's do (a hub's may trust it), the queue and task tools and the form write only its queue there; no tool writes under the project root (ADR-0004, ADR-0005). Design rules for the full tool set: 07 §1.1-1.2.

Dependencies: `specengine-cli` (never the store or `rusqlite` directly; no reverse edge), `rmcp`, `tokio`, `serde`, `serde_json`, `clap`; pinned by eval `build_graph.rs`.

## Binary

`specengine-mcp [--lifecycle auto|legacy] [--root DIR] [--config FILE]`; `--root`, `--config` are the CLI globals of every read. stdout carries JSON-RPC and nothing else (a quiet panic hook); the only human text is one stderr line when the session fails. Exit 0 = the client closed the session (also empty stdin); 1 = the session did not start or the service failed. The owner checklist's launcher: `fixtures/mcp/mcp.json` (a `probes` build, a scratch `HOME`).

## Protocol eras

rmcp has no server-side lifecycle mode (`ClientLifecycleMode::Auto` is client-only): the server detects the era from the client's first message.

- **legacy**: `initialize` opens a session; the owner's form is a server-initiated `elicitation/create` (`send_cancellable_request`) the call waits for; a cancelled call withdraws it.
- **stateless** (2026-07-28): every request carries `_meta` with `io.modelcontextprotocol/protocolVersion` and `…/clientCapabilities` (either missing -> -32602; `…/clientInfo` optional); `server/discover` is answered; `tools/list` and the resource results carry `ttlMs: 0`, `cacheScope: "public"`; no `ping`.
- `--lifecycle` sets `supported_protocol_versions`: `auto` (default) = known versions up to 2026-07-28; `legacy` = up to 2025-11-25, a stateless request then gets -32022.
- A first message other than `initialize`, `ping` or a complete stateless request ends the session (exit 1). rmcp skips non-JSON lines silently and, after stdin EOF, lets in-flight handlers run up to 5 s, then drops them unanswered.
- `instructions` (`INSTRUCTIONS`, every build; `TEXT_LIMIT`) and every tool description (`DESCRIPTION_LIMIT`) are compile-time asserted <= 2 048 bytes.

## Claude Code client

Verified on v2.1.283 by script and by hand (Phase 0); the read-tool items checked by hand on 2.1.288 (2026-10-03, `mcp-read.md` "Owner's check"); re-verify on an upgrade with the `probes` build.

- A stdio server gets the legacy handshake (2025-11-25) by default; `MCP_PROTOCOL_NEGOTIATION=auto` negotiates 2026-07-28: hence both eras. Project scope: `.mcp.json` in the repository, read at session start; transports `stdio | http | ws` (`sse` deprecated).
- The output cap counts characters: 48 000 pass inline with no warning reaching the model, 104 000 are rejected (2.1.288: 60 000 too, the result stored in a file), `MAX_MCP_OUTPUT_TOKENS` does not raise it. Tool descriptions and `instructions` are truncated at 2 048 characters; tool search is on by default, so `instructions` is the server's most important text.
- Elicitation (since 2.1.76, form and URL) works in both eras; with permission prompts bypassed the form still appears. The form is a flat object of primitives and enums (titled enums' labels unverified: `docs/canon/review-form.md` "Owner's check"); answers `accept | decline | cancel`; a URL <= ~8 000 characters. Sampling: no data; the Tasks extension: unsupported.
- `_meta["anthropic/requiresUserInteraction"]: true` (>= 2.1.199): a permission prompt on every call, even in `bypassPermissions`; no "don't ask again", allow rules ignored, a `PreToolUse` hook cannot approve it: the strongest consent primitive.
- A call longer than 120 s goes to the background, runs to its end and returns its result as a notification; one held by an open elicitation form is exempt (>= 11 min observed).
- `@server:uri` (`@specengine:spec://<slug>/node/<id>`) inserts a resource without a tool call: `PreToolUse` hooks do not fire. MCP prompts are slash commands (`/specengine:<prompt>`).
- The plugin (root `README.md` "Claude Code plugin") runs this binary from `PATH` as `specengine`, no arguments; its skills name tools and arguments: changing either updates them and the plugin's `version`.

## Feature `probes`

The measurement build, never default: `probe_output {tokens}` -> `tokens` × 4 bytes of filler (max 200 000) and `probe_sleep {seconds}` (max 900, ends early on cancellation), both `readOnlyHint: true`. A hub's server also routes `probe_panic`, a panic in the turn (`HUB_PROBES`).

## Modules

| Module | What it does |
|---|---|
| `lib.rs` | crate doc; re-exports `Lifecycle`, `SpecEngineServer`, `serve_stdio`, `ServeError`, `INSTRUCTIONS`, `MAX_RESULT_CHARS`; the hub's `HubSeam`, `Command`, `TurnFuture`, `HUB_TOOLS`, `HUB_READ_TOOLS`, `HUB_PROBES`, `HUB_INSTRUCTIONS` |
| `hub.rs` | the hub mode: `SpecEngineServer::hub(Arc<dyn HubSeam>)`; `HubSeam` (`login`, `turn(Command) -> TurnFuture`, the daemon's); `turned`, the turn raced with the request's cancellation; `HUB_TOOLS` (18), `HUB_READ_TOOLS`, `reads_trust` (`spec-read-freshness.md`), `HUB_ROUTER` (a `static`), `HUB_INSTRUCTIONS` and nine descriptions, their asserts; hub argument types without `task_id`; the login into requests and card actors |
| `server.rs` | `SpecEngineServer` (`ServerHandler`, no project state): `INSTRUCTIONS` (bytes: `mcp-read.md` **Texts**) and their asserts, the tool routers (`read_tools`, `code_tools`, `intake_tools`, `task_tools`, `card_tools`, `review_tools`), `get_info` (tools, resources), `Lifecycle` -> `supported_protocol_versions`, `list_resource{s,_templates}`, `read_resource` (blocking pool, 2026-07-28 cache hints); `serve_stdio` (clean exit on closed stdin, `ServeError`) |
| `read.rs` | the read tools (`read_tools` router; `get_impact`: `spec-impact.md`; `get_trace`: `spec-trace.md`): argument types (`with`: `code-reads.md`, `spec-history.md`), descriptions (a hub's four too) and their asserts, `MAX_RESULT_CHARS`, `call` (`spawn_blocking`), `answer_of`, `Answer` -> the tool result; text helpers shared with `intake.rs` |
| `code.rs` | the code tools (`code_tools`): arguments, the form checks before any call, descriptions, asserts; `read.rs`' `call` |
| `intake.rs` | the queue tools (`intake_tools` router): argument types (one level of closed inline objects; `ChangeKind`: `proposal-kinds.md`), descriptions (the path form, `spec approve` too) and their asserts (each cap, `INTAKE_MATCHES_MAX`, `DISTINCT_MAX`), one CLI library call each with `author_role` always passed, `utc_now`, `process_git` |
| `tasks.rs` | the task tools (`task_tools`): arguments, descriptions, asserts |
| `cards.rs` | the card tools (`card_tools`, `cards.md` "MCP"): arguments, descriptions, asserts; `{id, reason}` without `structuredContent` |
| `resources.rs` | `list` (200 per page), the node template, `parse`, `read`, `percent_encode`, `percent_decode`, the error codes |
| `mirror.rs` | `input_schema`, `output_schema` (`rmcp::schemars`, inlined, every key required); the mirror types of the CLI's `--json` documents (review, intake, drift, code documents, a record's keys, a one-key `Choice`, `BundleRowShape` (`BindingRowShape` + `tokens_est`), `ImpactDocument`, `TraceDocument`, layer C's `SymbolsLayerC`, `SymbolsTier`, `SnapshotState`, `SnapshotFreshness`; `kind` free), schema only |
| `review.rs` | the owner's form (`review_tools`, its description and asserts): `review` (`stage_form` first, the form-capability check, the era; legacy `send_cancellable_request`, stateless `input_required` with the unsealed `requestState`), `resume` (round 2), `answered` (the answer -> `StageBody`), `after` (`stage`, the read after it, the outcome lines); `form_of` (by family), `value` (one line, escaped, cut) |
| `probes.rs` | `probes` only: `probe_output`, `probe_sleep`, a hub's `probe_panic` |
| `main.rs` | clap CLI, the quiet panic hook, current-thread tokio runtime, exit codes |

## Open minors

- rmcp's stdio codec has no maximum line length: revisit with the daemon bridge.
- The read path's open items: `mcp-read.md` "Open".

## Tests

`tests/common/mod.rs`: a spawned-binary JSON-RPC client, each with a cleared environment, its own cwd and a fresh scratch `HOME`; `common/read.rs` the CLI-side expectations, `common/blake3.rs` a BLAKE3 written from the specification. The read files: `docs/canon/mcp-read.md` "Tests"; `mcp_code.rs`, `mcp_code_pilot.rs`: the code tools; `mcp_layer_c.rs`, `common/layer_c.rs`: a stored layer C snapshot (`docs/canon/layer-c-reads.md`); `mcp_intake.rs`: the queue tools (`mcp_door`, `mcp_genre` too); `mcp_decision.rs`, `mcp_create.rs`, `mcp_tasks.rs`, `mcp_drift.rs`: a decided item, a create, the task tools, a drift; `mcp_hub.rs`, `mcp_hub_trust.rs`: the hub mode's 18 tools and texts; `plugin_files.rs`, `plugin_skills.rs`: the plugin; `mcp_review_form.rs` (both eras), `mcp_stage.rs` (no tool input carries a choice): the form. `tests/mcp_stdio.rs` (`probes`: both eras, a 104 000-byte output, cancellation, startup, `mcp.json`). Run: `cargo nextest run -p specengine-mcp --test <file>`, `--features probes` for `mcp_stdio`.
