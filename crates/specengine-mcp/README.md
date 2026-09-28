---
class: canon
tier: 1
scope: [crates/specengine-mcp]
owner: owner
reviewed: 2026-09-29
---

# specengine-mcp — the MCP server over stdio

One process serves one client over stdin/stdout on `rmcp =3.5.0` (features `server`, `macros`, `transport-io`, `request-state`; rmcp's `elicitation` feature is skipped because it pulls `url` / ICU — the wire message goes out through `send_request`). No HTTP stack in the graph. Today it carries only the consent tool as a demo: no queue behind it, nothing recorded, **no tool writes to disk** (ADR-0004, ADR-0005). Design rules for the full tool set: 07 §1.1–1.2; Claude Code behaviour it relies on: 04 §4.

## Binary

`specengine-mcp [--lifecycle auto|legacy]`. stdout carries JSON-RPC and nothing else; the only human text is one stderr line when the session fails. Exit 0 = the client closed the session (also empty stdin); 1 = the session did not start or the service failed. Launcher used by the owner checklist: `fixtures/mcp/mcp.json` (`cargo run -q -p specengine-mcp --features probes`).

## Protocol eras

rmcp has no server-side lifecycle mode (`ClientLifecycleMode::Auto` is client-only): the server detects the era from the client's first message.

- **legacy** — `initialize` opens a session; the consent form is a server-initiated `elicitation/create` request and the call waits for it.
- **stateless** (2026-07-28) — every request carries `_meta` with `io.modelcontextprotocol/protocolVersion` and `…/clientCapabilities` (either missing → -32602; `…/clientInfo` optional); `server/discover` is answered; `tools/list` carries `ttlMs: 0`, `cacheScope: "public"`.
- `--lifecycle` sets `supported_protocol_versions`: `auto` (default) = known versions up to 2026-07-28; `legacy` = up to 2025-11-25, a stateless request then gets -32022.
- A first message other than `initialize`, `ping` or a complete stateless request ends the session (exit 1). rmcp skips non-JSON lines silently and, after stdin EOF, lets in-flight handlers run up to 5 s, then drops them unanswered.
- `instructions` (`INSTRUCTIONS`, plus a probe paragraph under `probes`) are compile-time asserted ≤ 2 048 bytes (`TEXT_LIMIT`): Claude Code truncates at 2 048 characters.

## `review_proposal` — the owner's consent tool

- Input `{proposal_id}`, matching `[A-Za-z][A-Za-z0-9._-]{0,63}`; a broken, non-Latin or mixed-script ID is a tool error (`isError: true`) naming the code point, before any form (ADR-0009; no autofix yet).
- Tool `_meta {"anthropic/requiresUserInteraction": true}`; annotations `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`; `outputSchema` = the `structuredContent` below.
- Form (`requestedSchema`): `decision` enum `approve|reject`, required; `comment` string, optional. A client without form elicitation gets a tool error saying the owner cannot be asked.
- Stateless: round 1 returns `resultType: "input_required"`, the form under `inputRequests.owner_review` and a `requestState` (`rs1.` prefix, HMAC-sealed with a per-process random key, associated data = tool name + proposal ID); round 2 repeats the call with it and `inputResponses.owner_review`. A tampered state, one sealed for another ID, or a missing answer → -32602.
- Result: prose text + `structuredContent {proposal_id, action: accept|decline|cancel, decision, comment, era: legacy|stateless, protocol_version}`; `decision` and `comment` are `null` unless accepted (a blank comment → `null`).
- A cancelled call (`notifications/cancelled`) gets no response.
- The state is not a consent record: no TTL, not single-use (a replay returns only the answer the client supplies again), a restart invalidates every state, the legacy form has no server-side timeout. The requirements for the real tool are in 07 §1.2.

## Feature `probes`

Never default; both `readOnlyHint: true`. `probe_output {tokens}` → `tokens` × 4 bytes of filler (max 200 000); `probe_sleep {seconds}` (max 900), ends early on cancellation. Used by the interactive checks of 04 §4 — re-run them on a Claude Code upgrade.

## Modules

| Module | What it does |
|---|---|
| `lib.rs` | crate doc; re-exports `Lifecycle`, `SpecEngineServer`, `serve_stdio`, `INSTRUCTIONS`, `ReviewOutcome`, `FormAction`, `Decision`, `Era` |
| `server.rs` | `SpecEngineServer` (`ServerHandler`): tool router, `get_info`, `Lifecycle` → `supported_protocol_versions`; `serve_stdio` (clean exit on closed stdin, `ServeError`) |
| `review.rs` | `check_proposal_id`, form capability check, `ask_legacy` (raced with cancellation), `ask_stateless` / `resume` (`RequestStateCodec`), `finish` → `ReviewOutcome` + prose |
| `probes.rs` | `probe_output`, `probe_sleep` |
| `main.rs` | clap CLI, current-thread tokio runtime, exit codes |

## Open minors

- Cancelling a legacy call does not withdraw its outstanding form (fix: `send_cancellable_request` + `handle.cancel`).
- `check_proposal_id` echoes an unbounded ID into its error; the owner's `comment` is echoed unbounded twice (prose and `structuredContent`).
- Tool descriptions are not asserted ≤ 2 048 bytes (only `instructions` is).
- rmcp's stdio codec has no maximum line length — revisit with the daemon bridge.

## Tests

`tests/mcp_stdio.rs` (a spawned-binary JSON-RPC client in `tests/common/mod.rs`: both eras, elicitation round trips, `requiresUserInteraction`, a 104 000-byte output, ID refusals, cancellation, empty stdin, bad first message, working directory untouched, `mcp.json` shape); `tests/mcp_default.rs` (a default build lists only `review_proposal`). Run: `cargo nextest run -p specengine-mcp --features probes --test mcp_stdio`.
