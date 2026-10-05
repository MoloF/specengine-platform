---
class: canon
tier: 1
scope: [crates/specengine-mcp]
owner: owner
reviewed: 2026-10-05
---

# specengine-mcp — the MCP server over stdio

One process serves one client over stdin/stdout on `rmcp =3.5.0` (features `server`, `macros`, `transport-io`, `request-state`; rmcp's `elicitation` feature is skipped because it pulls `url` / ICU — the wire message goes out through `send_request`). No HTTP stack in the graph. The default build serves the read tools `get_tree`, `get_node`, `search`, `get_context_bundle` and the resources over the CLI library; their contract (parity, schemas, resources, "no project", size, latency, the owner's check) is `docs/canon/mcp-read.md`. The queue tools `propose_change`, `ask_question`, `report_discrepancy`, `get_proposal` call the CLI's `propose … --brief`, `propose question|discrepancy`, `review --brief` (`docs/canon/agent-intake.md`). Reads refresh the project's index in SpecEngine's data directory, as the CLI's reads do, the queue tools write only its proposal queue there; no tool writes under the project root (ADR-0004, ADR-0005). Design rules for the full tool set: 07 §1.1–1.2.

Dependencies: `specengine-cli` (never the store or `rusqlite` directly; the reverse edge is forbidden), `rmcp`, `tokio`, `serde`, `serde_json`, `clap`, `getrandom` (optional, `probes`); pinned by eval `build_graph.rs`.

## Binary

`specengine-mcp [--lifecycle auto|legacy] [--root DIR] [--config FILE]`; `--root`, `--config` are the CLI globals of every read. stdout carries JSON-RPC and nothing else (a quiet panic hook); the only human text is one stderr line when the session fails. Exit 0 = the client closed the session (also empty stdin); 1 = the session did not start or the service failed. Launcher used by the owner checklist: `fixtures/mcp/mcp.json` (`cargo run -q -p specengine-mcp --features probes`, a scratch `HOME`).

## Protocol eras

rmcp has no server-side lifecycle mode (`ClientLifecycleMode::Auto` is client-only): the server detects the era from the client's first message.

- **legacy** — `initialize` opens a session; the consent form is a server-initiated `elicitation/create` request and the call waits for it.
- **stateless** (2026-07-28) — every request carries `_meta` with `io.modelcontextprotocol/protocolVersion` and `…/clientCapabilities` (either missing → -32602; `…/clientInfo` optional); `server/discover` is answered; `tools/list` and the resource results carry `ttlMs: 0`, `cacheScope: "public"`; no `ping`.
- `--lifecycle` sets `supported_protocol_versions`: `auto` (default) = known versions up to 2026-07-28; `legacy` = up to 2025-11-25, a stateless request then gets -32022.
- A first message other than `initialize`, `ping` or a complete stateless request ends the session (exit 1). rmcp skips non-JSON lines silently and, after stdin EOF, lets in-flight handlers run up to 5 s, then drops them unanswered.
- `instructions` (`INSTRUCTIONS`, plus a probe paragraph under `probes`; `TEXT_LIMIT`) and every tool description (`DESCRIPTION_LIMIT`) are compile-time asserted ≤ 2 048 bytes.

## Claude Code client

Verified on v2.1.283 by script and by hand (Phase 0); the read-tool items checked by hand on 2.1.288 (2026-10-03, `docs/canon/mcp-read.md` "Owner's check"); re-verify on an upgrade with the `probes` build.

- A stdio server gets the legacy handshake (2025-11-25) by default; `MCP_PROTOCOL_NEGOTIATION=auto` negotiates 2026-07-28: hence both eras. Project scope: `.mcp.json` in the repository, read at session start; transports `stdio | http | ws` (`sse` deprecated).
- The output cap counts characters: 48 000 pass inline with no warning reaching the model, 104 000 are rejected (2.1.288: 60 000 too, the result stored in a file), `MAX_MCP_OUTPUT_TOKENS` does not raise it. Tool descriptions and `instructions` are truncated at 2 048 characters; tool search is on by default, so `instructions` is the server's most important text.
- Elicitation (since 2.1.76, form and URL) works in both eras; with permission prompts bypassed the form still appears. The form is a flat object of primitives and enums; answers `accept | decline | cancel`; a URL ≤ ~8 000 characters. Sampling: no data; the Tasks extension: unsupported.
- `_meta["anthropic/requiresUserInteraction"]: true` (≥ 2.1.199): a permission prompt on every call, even in `bypassPermissions`; no "don't ask again", allow rules ignored, a `PreToolUse` hook cannot approve it — the strongest consent primitive.
- A call longer than 120 s goes to the background, runs to its end and returns its result as a notification; one held by an open elicitation form is exempt (≥ 11 min observed).
- `@server:uri` (`@specengine:spec://<slug>/node/<id>`) inserts a resource without a tool call: `PreToolUse` hooks do not fire. MCP prompts are slash commands (`/specengine:<prompt>`).

## Feature `probes`

The measurement build, never default: the consent demo below (its `requestState` key needs `getrandom`), `probe_output {tokens}` → `tokens` × 4 bytes of filler (max 200 000) and `probe_sleep {seconds}` (max 900, ends early on cancellation), both `readOnlyHint: true`, and their `instructions` paragraph.

## `review_proposal` — the Phase 0 consent demo (`probes`)

- Input `{proposal_id}`, matching `[A-Za-z][A-Za-z0-9._-]{0,63}`; a broken, non-Latin or mixed-script ID is a tool error (`isError: true`) naming the code point, before any form (ADR-0009; no autofix yet).
- Tool `_meta {"anthropic/requiresUserInteraction": true}`; annotations `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`; `outputSchema` = the `structuredContent` below.
- Form (`requestedSchema`): `decision` enum `approve|reject`, required; `comment` string, optional. A client without form elicitation gets a tool error saying the owner cannot be asked.
- Stateless: round 1 returns `resultType: "input_required"`, the form under `inputRequests.owner_review` and a `requestState` (`rs1.` prefix, HMAC-sealed with a per-process random key, associated data = tool name + proposal ID); round 2 repeats the call with it and `inputResponses.owner_review`. A tampered state, one sealed for another ID, or a missing answer → -32602.
- Result: prose text + `structuredContent {proposal_id, action: accept|decline|cancel, decision, comment, era: legacy|stateless, protocol_version}`; `decision` and `comment` are `null` unless accepted (a blank comment → `null`). A cancelled call gets no response.
- The state is not a consent record: no TTL, not single-use (a replay returns only the answer the client supplies again), a restart invalidates every state, the legacy form has no server-side timeout. The real tool's requirements: 07 §1.2.

## Modules

| Module | What it does |
|---|---|
| `lib.rs` | crate doc; re-exports `Lifecycle`, `SpecEngineServer`, `serve_stdio`, `ServeError`, `INSTRUCTIONS`, `MAX_RESULT_CHARS`; under `probes` `ReviewOutcome`, `FormAction`, `Decision`, `Era` |
| `server.rs` | `SpecEngineServer` (`ServerHandler`, no project state): `INSTRUCTIONS` (the eight tools) and their asserts, the tool routers (`read_tools`, `intake_tools`, `review_tools`), `get_info` (tools, resources), `Lifecycle` → `supported_protocol_versions`, `list_resources`, `list_resource_templates`, `read_resource` (blocking pool, cache hints from 2026-07-28); `serve_stdio` (clean exit on closed stdin, `ServeError`) |
| `read.rs` | the read tools (`read_tools` router): argument types, descriptions and their asserts, `MAX_RESULT_CHARS`, `call` (`spawn_blocking`), `Answer` → the tool result; text helpers shared with `intake.rs` |
| `intake.rs` | the queue tools (`intake_tools` router): argument types (one level of closed inline objects; `ChangeKind` only `update`), descriptions and their asserts (each cap, `INTAKE_MATCHES_MAX`, `DISTINCT_MAX`), one CLI library call each with `author_role` always passed, `utc_now`, `process_git` |
| `resources.rs` | `list` (200 per page), the node template, `parse`, `read`, `percent_encode`, `percent_decode`, the error codes |
| `mirror.rs` | `input_schema`, `output_schema` (`rmcp::schemars`, inlined, every key required); the mirror types of the CLI's `--json` documents (the review and intake documents too; `kind` free), schema only |
| `review.rs` | `probes` only: `review_tools` router, `check_proposal_id`, form capability check, `ask_legacy` (raced with cancellation), `ask_stateless` / `resume` (`RequestStateCodec`), `finish` → `ReviewOutcome` + prose |
| `probes.rs` | `probes` only: `probe_output`, `probe_sleep` |
| `main.rs` | clap CLI, the quiet panic hook, current-thread tokio runtime, exit codes |

## Open minors

- Cancelling a legacy call does not withdraw its outstanding form (fix: `send_cancellable_request` + `handle.cancel`).
- `check_proposal_id` echoes an unbounded ID into its error; the owner's `comment` is echoed unbounded twice (prose and `structuredContent`).
- rmcp's stdio codec has no maximum line length — revisit with the daemon bridge.
- The read path's open items: `docs/canon/mcp-read.md` "Open".

## Tests

`tests/common/mod.rs`: a spawned-binary JSON-RPC client, each spawn with a cleared environment, its own working directory and a fresh scratch `HOME`; `common/read.rs` the CLI-side expectations, `common/blake3.rs` a BLAKE3 written from the specification. The read files: `docs/canon/mcp-read.md` "Tests"; `mcp_intake.rs`: the queue tools (`mcp_door`, `mcp_genre` cover them too). `tests/mcp_stdio.rs` (`probes`: both eras, elicitation round trips, `requiresUserInteraction`, a 104 000-byte output, ID refusals, cancellation, empty stdin, bad first message, working directory untouched, `mcp.json` shape). Run: `cargo nextest run -p specengine-mcp --test <file>`, `--features probes` for `mcp_stdio`.
