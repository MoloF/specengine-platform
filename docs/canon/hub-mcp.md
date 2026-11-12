---
class: canon
tier: 2
scope: [crates/specengine-http, crates/specengine-mcp, crates/specengine-cli, ui]
owner: owner
reviewed: 2026-11-12
---

# Hub MCP: `/mcp/<slug>` under each request's login

ADR-0065, built by `hub-mcp` (RM-0053, step 11). Working answers of the orchestrator, 2026-11-03, the owner's review open. A hub (`hub.md`) serves `specengine-mcp`'s server over rmcp's Streamable HTTP, inside each project's turn. Its tool set reads, raises proposals and writes cards under the request's `Remote-User`, and decides nothing. A local daemon serves no `/mcp`. Code: http `mcp.rs` (`Mounts`, `serve`, the session table), `hub.rs` `class`, `app.rs`; mcp `hub.rs`; CLI the propose requests' `login`; UI `src/inbox/ProposalCard.tsx`.

## Route

`/mcp/<slug>`, on a hub alone, one per served project:
- POST;
- GET, a legacy session's SSE stream;
- DELETE, that session's end.

No method takes a query: `refused: this route takes no query`.

In the fence, POST and DELETE are class `Mcp`: any login, `[team]` never read. GET passes as every GET does. Other methods, and `/mcp/<slug>/...`, meet default deny. `Host`, `Origin`, `Sec-Fetch-Site` and `Remote-User` are judged as on every route. The board's `same-origin` door is not asked: no MCP client sends `Sec-Fetch-Site`. An unknown slug gets the unknown-slug 404. A local daemon answers every method with its unknown-route 404 and never reads `Remote-User`.

**`Authorization`.** On every hub route but `/mcp/<slug>`, an `Authorization` header, any value, once or more, is refused at fence step 5, before the class: `refused: the Authorization header is for /mcp alone: this route takes the login door's session`. On `/mcp/<slug>` it passes, never read, logged or stored. A local daemon never reads it.

**Both eras.**
- An `initialize` up to 2025-11-25 opens a legacy session (`Mcp-Session-Id`), which Claude Code needs for its GET stream.
- A 2026-07-28 request carries `_meta` and is served alone.

rmcp's configuration: `legacy_session_mode` on; `allowed_hosts` the hub's authority; `allowed_origins` its origin with the port always explicit (`https://host:443`), since rmcp matches a portless entry to any port; a 4 MiB body cap; a 15 s keep-alive; no session store. The hub server's `Lifecycle` is `Auto`. A 2026-07-28 request must carry SEP-2243's `Mcp-Method`, and `Mcp-Name` (a call's tool, a read's `uri`), else rmcp answers 400 with JSON-RPC `-32020`.

**Answers the daemon gives itself,** never reaching rmcp: the session 404s ("Sessions"); an unknown tool ("Tools"), framed as JSON (`application/json`) inside a session too, where rmcp would answer on its SSE stream; a legacy POST with no session id that is not an `initialize`, a 400 where rmcp gives 422.

## Sessions

Held in memory only: session id -> slug, login, last use. The ids are rmcp's, a UUID v4.

- **Bound.** A session belongs to the slug and login of the POST whose 2xx issued it. A request naming an id not bound to its own slug and login gets 404 `Not Found: Session not found`, rmcp's bytes, on every method, with rmcp never called. The id may be unknown, ended, another login's or another project's; all four answer alike, so no login learns of another's session.
- **One id per request.** A request carrying more than one `Mcp-Session-Id` value, equal or not, gets the same 404, with rmcp never called and nothing unbound. rmcp would close only the first value on DELETE, leaving live sessions no cap counts.
- **Drop-safe.** A request dropped after rmcp issued an id, before the daemon bound it, has that session closed: no unbound session outlives its request.
- **Caps.** 4 per login per project and 64 per hub, counting only the sessions rmcp still holds.
  - A login's fifth session in a project closes its least recently used one there.
  - Past 64, the new session is closed and the answer is 503 `refused: this hub holds 64 MCP sessions, its cap: try again once one ends`.
  - No login's session is ever closed for another's.
- **Life.** A session ends at a restart, after 300 s idle, at its own login's DELETE, or at an eviction. The stateless era opens none. A GET stream (one resuming a POST's too) ends 300 s after it opens, not its session, which rmcp's idle, re-armed by that GET first, ends. Nothing rides it (no hub tool notifies or asks): a reconnect loses nothing.
- **Stored nowhere, naming no one.** No id is in the queue, an event, a dump, a trailer, a commit, an answer body or stderr. A session only refuses: it is never a login, an author, an actor, a `run` or an argument.

## Turn

These run in the project's turn, as `answer::run` runs its calls:
- each call that reads the corpus or the queue, or writes the queue;
- each resource read.

That is the mutex, `spawn_blocking`, `project.globals()`, the slug found again, in a `walking` scope that read tools leave (`spec-read-freshness.md` "Routes and tools"). `initialize`, notifications, `ping`, `tools/list`, `resources/templates/list` and `server/discover` take no turn.

The wait for the turn races the request's cancellation, as the board's `answer::run` does: an HTTP disconnect on a stateless call, or `notifications/cancelled` on a session. A call cancelled while it waits never runs and stores nothing. A call that has started runs to its end. Reads keep MCP's `View::Capped`, byte for byte the stdio answer (`mcp-read.md` "Tools", **Parity**), never the daemon's `View::Browser`.

`specengine-mcp` names no HTTP crate. The daemon implements its `HubSeam`: `login`, from `Parts.extensions`' `Login`; `turn(Command) -> TurnFuture`, a future of `()` done once the command ran or never will, the command carrying its own answer; the slug judged by `answer.rs` `found`.

The transport stays in `specengine-http`: `App::new` builds one service per project (`Mounts::new`), each with its own `LocalSessionManager`.

## Tools

Eighteen tools and both resources, each its stdio twin's CLI call:
- **reads:** `get_tree`, `get_node`, `search`, `get_context_bundle`, `get_roadmap`, `get_flow`, `get_trace`, `get_proposal`;
- **queue**, without `task_id`: `propose_change` (four kinds, `dry_run`, a create's `distinct_from`, `keywords`: `proposal-similar.md`), `ask_question`, `report_discrepancy`, `report_decision`;
- **cards:** `list_cards`, `get_card`, `create_card`, `split_card`, `edit_card`, `move_card`.

None decides: a person confirms on the hub (`hub-confirm.md`) or on a terminal. Any login raises proposals, since `/mcp` never reads `[team]` (ADR-0012). Any message whose `method` is `tools/call` and whose `params.name` is not one of the 18 (`probes` builds: or `HUB_PROBES`) never reaches rmcp, judged on the raw JSON as rmcp reads it, never only on a typed `CallToolRequest`. A request, one whose malformed `arguments` parse as a custom request included, gets rmcp's unknown-tool answer, JSON-RPC `-32602` `tool not found` with its id. A notification gets a 202. Nothing is cached: rmcp's per-name schema cache would grow on invented names.

Left out: `get_impact`, `find_symbols`, `refs`, `unmapped`, `check_binding` (a hub holds no code); `report_drift` (a terminal's); `review_proposal` (local forms); the five task tools (no `T-` flow on a hub).

**Texts.** `HUB_INSTRUCTIONS`, and thirteen hub descriptions: `get_tree`, `get_node`, `search`, `get_context_bundle` (stdio's, a hub tail), `get_roadmap` (with no `get_task` and no "the owner's decision on a terminal"), the four queue tools and the four card writes. Reads' tails: `spec-read-freshness.md` "Routes and tools". `propose_change` names `from 100001 on this hub`, asserted against `HUB_ID_START` (`spec-ids.md`). The other five are stdio's. Each hub text is ASCII and at most 2 048 bytes, compile-time asserted, holds the determinism sentence, and names no tool left out.

## Identity

Each call reads the login from its own request.

**Proposals.** A proposal's `Author` is `{type: agent, role, model, run, login}`.
- `role`, `model` and `run` are agent-declared per call, as over stdio.
- `login` is the request's, never taken from `clientInfo` or a session id.
- `Proposed-by:` ends ` login=<login>`.
- The seven CLI propose requests carry `login`, which only a hub sets, so stdio rows keep no `login` key.

**Cards.** A card write's actor is the login, so a flow's `by` holds against the person (`hub.md` "Identity"). Over stdio the actor stays `agent` (`cards.md` "Who acts").

No hub schema holds `login`, `as`, `actor` or `task_id`. A hub write that finds no login stores nothing.

<a id="trust"></a>
## Trust

As in `hub.md#trust`: no authentication inside SpecEngine (ADR-0034); the daemon trusts one header (ADR-0059), holding no account, token, key or login session. A protocol session binds a login in memory and identifies no one. Raising proposals is the daemon's for `/mcp` and a card's `items`, on ADR-0057's precedent of a proposing daemon.

**The daemon can defend:**
- another login's session id, or a second id in one request (404);
- an invented tool name, refused before rmcp;
- `Authorization` on a route but `/mcp`, where the proxy forwards it;
- a query;
- an identity argument, which no schema holds;
- every decision, which no tool makes.

**The daemon cannot defend:** an MCP client's `Remote-User` looks like a browser's, so only the proxy keeps the credential off every other path.

**The deployment guarantees** (`hub-deploy.md#door`), beside `hub.md#trust`'s items 1-5:

6. The credential an MCP client holds opens only the routers matching this rule:

   ```text
   ^/mcp/[^/]+$
   ```

   Every other router reaching the daemon, the confirm class's four routes first, takes the login door's browser session alone. The credential's `Remote-User` is the person's browser login, byte for byte.

A unit test of http `src/hub.rs` reads this rule from this page, as `the_step_up_regex_is_the_confirm_class` reads item 5 from `hub.md`. The rule matches every `/mcp` route served and no other route. It is this page's only fenced line.

## Known limits

- AI reads wait in the turn beside the page's; no limit bounds the calls waiting.
- A login's AI reads others' proposals and cards: they are data, never a decision.
- A bearer `Remote-User` that is an email or a UUID would split a person in two; the deployment's Basic user is the door login.
- Answers carry the corpus to the client's model provider.
- A forged `Remote-User` raises proposals as anyone: no real use before the gate, step 11b-3.
- A body's buffer is reserved from its `Content-Length`, at most the 4 MiB cap.
- A body that is no JSON-RPC message but names an unknown tool is refused as one (rmcp: a parse error).
- A duplicated JSON key can cost up to three parses of one body.
- `Mounts::shutdown` effectively never runs: sessions end with the process.
- The drop-safe bind has no end-to-end test; the window is one task's step.

## Client check

By hand at the gate, not yet run; findings go to `hub-deploy.md` "Gate" with the client's version and date.
1. **Claude Code on the laptop:** a hub on a name resolving to loopback (not `localhost`), behind a local TLS proxy with `basicAuth` on `/mcp` and `headerField: Remote-User`; `claude mcp add --transport http ... --header "Authorization: Basic ..."`. Expected: the session holds, the GET stream reopens at 300 s, 18 tools, `login` on a proposal and a card, a second login's own sessions.
2. **claude.ai and Claude Desktop, at the gate, step 11b-3,** on the public origin: whether the connector's OAuth client sends PAR, which Authelia requires of bearer-authz clients; whether it sends an `Origin`; which `Remote-User` a bearer request gets. Until then "writers install nothing" is a working answer.

## Tests

http `tests/hub_mcp.rs` (H1-H11, H13-H20 of `docs/features/hub-mcp.md`), its client `tests/common/mcp_http.rs`; `tests/hub.rs` (`GET_ROUTES` with `/mcp/{p}`, the fence order with `Authorization`); `tests/door.rs` (`specengine_mcp` named in `src/mcp.rs` alone); units in `src/hub.rs` (item 6's rule) and `src/mcp.rs` (idle, life, the bound count, the `handle` counter). mcp `tests/mcp_hub.rs` (the 18, the texts' budget), `mcp_hub_trust.rs`, `mcp_genre.rs`; http `tests/hub_trust.rs`; eval `build_graph.rs` (the daemon's ten dependencies, rmcp's feature, the seven crates' versions). UI `src/inbox/ProposalCard.test.tsx`.
