---
class: spec
status: shipped
shipped: 2026-11-03
scope: [crates/specengine-mcp, crates/specengine-http, crates/specengine-cli, crates/specengine-eval, ui]
ref: RM-0053; Phase 7 step 11 (08 s2); the hub-mcp analysis and the orchestrator's decisions Q1-Q8 of 2026-11-03, the owner's review open; iterations 2 and 3 on the adversarial review's findings
adrs: [ADR-0065]
---

# hub-mcp: `/mcp/<slug>` on a hub, sessions bound to a login, proposals and cards under it, nothing decided (RM-0053)

## Why

Since `hub-confirm` a lead confirmed on a hub with no terminal, yet nothing on a hub raised a proposal. A writer's AI, that person's hands, reached SpecEngine only over stdio, on a machine holding the project. This slice mounts the MCP server in a hub's daemon: one Streamable HTTP endpoint per project, behind the fence, in the project's turn. Its 17 tools read, raise proposals and write cards under the request's `Remote-User`, and decide nothing. The daemon holds no credential; trust item 6 says which routers the proxy's MCP credential opens.

Decided by the orchestrator on 2026-11-03, each awaiting the owner's review:
- **Q1:** rmcp's Tower service, in `specengine-http`.
- **Q2:** card writes act as the login.
- **Q3:** the 17 tools.
- **Q4:** the credential is the proxy's.
- **Q5:** `Authorization` is refused outside `/mcp`.
- **Q6:** spec-ID collisions go to RM-0073.
- **Q7:** ADR-0065.
- **Q8:** the daemon side first, the client check by hand after.
- **The spec's choices, accepted:** S1 the caps (4 per login per project, 64 per hub); S2 evict a login's least recently used session, refuse at the hub's cap; S3 `.route(.., any(..))` with rmcp's `handle`; S4 rmcp's feature in the daemon's manifest; S5 no flag.

## Description and interactions

Canon: `docs/canon/hub-mcp.md` (ADR-0065).

### The route

`/mcp/<slug>`, its fence class, `Authorization` and both eras: `hub-mcp.md` "Route".

### A call

The binding, the turn raced with cancellation, `HubSeam`, and the router built once per process: `hub-mcp.md` "Sessions", "Turn".

### The tools

The 17, the 12 left out, the unknown-name refusal, the texts: `hub-mcp.md` "Tools".

### Identity

`Author.login`, the card's actor, no identity argument: `hub-mcp.md` "Identity".

## Data

rmcp's configuration, the session table and caps, the answers, the hub texts: `hub-mcp.md`. The seven new crates, their versions and licences: `docs/canon/stack.md`, the rmcp row.

**Bumps:**
- `GET_ROUTES` to 27;
- the fence's class `Mcp` and three answers;
- `HTTP_DEPENDENCIES` 8 to 10;
- seven transitive crates;
- seven CLI requests take `login`;
- the proposal card's login row.

No queue schema, event, `Author` field, dump, `daemon.json` key, daemon-keys set, flag, CLI command, stdio tool or text, or plugin change.

## Rules and edge cases

`hub-mcp.md` "Route", "Sessions", "Turn", "Tools", "Identity", "Trust", "Known limits".

## Acceptance criteria

- [x] H1 — Fence and route: missing, doubled or `agent` `Remote-User` 403; PUT 403; any query 403; `/mcp/<slug>/x` 404 on GET, 403 on POST; unknown slug 404; a local daemon's unknown-route 404 on every method. M: mount it locally; the query after GET's return.
- [x] H2 — Legacy session: `initialize` 200 with one id, `initialized` 202, a GET stream open past 20 s with keep-alives, the 17 listed, DELETE then 404. M: `legacy_session_mode: false`; rmcp's default `allowed_hosts`.
- [x] H3 — Stateless 2026-07-28 `tools/call`: answered, no session; `server/discover` gives `HUB_INSTRUCTIONS`. M: `Lifecycle::Legacy`.
- [x] H4 — Another login's or project's use of a session: 404 as an unknown id's, nothing stored, the session lives. M: no binding; by id alone.
- [x] H5 — Four queue tools store `login` `mira`; a confirm's `Proposed-by:` ends `login=mira`, `Decided-by` names the confirmer; stdio rows keep no `login`. M: drop the login; take it from an argument.
- [x] H6 — No hub schema holds `login`, `as`, `actor`, `task_id`; adding one fails deserialization. M: keep `task_id`.
- [x] H7 — Hub card writes record `mira`, `by` holds against her; stdio records `agent`. M: `TOOL_ACTOR` on the hub.
- [x] H8 — Every tool and resource called, both eras: `git status --ignored` empty, refs and files unchanged, new paths under `HOME`. M: a write under the root; a tool with no call.
- [x] H9 — The 17 exactly, no deciding word; stdio's 29 and BLAKE3 pins unchanged; b9 and the MCP manifest pin pass; `specengine_mcp` in `src/mcp.rs` alone. M: `review_proposal` or `report_drift` added; the feature in `specengine-mcp`.
- [x] H10 — A held turn delays that project's call, not another's, nor `tools/list`, `ping`, `initialize`. M: the call outside the turn.
- [x] H11 — The texts: ASCII, at most 2 048 B, the 17 named and no other; eight variants without `spec approve`, `task_id`, `get_task`, "You act as `agent`"; nine shared. M: `claim_task` in the text; stdio's `get_roadmap` on the hub; 2 049 B.
- [x] H12 — Item 6's rule, read from the canon, matches `/mcp/{p}` and no other route; item 5's matches no `/mcp`. M: MCP at `/api/projects/{p}/mcp`.
- [x] H13 — `Authorization` refused on `/`, a read, a stage and a confirm; passing on `/mcp`; ignored locally. M: no check; refused on `/mcp`.
- [x] H14 — Parity past 40 000 characters with stdio, cut and tail included. M: `View::Browser`.
- [x] H15 — A raised proposal reaches `events` within 1 s, `GET .../proposals/<id>` carries `author.login`; the card shows it as text, escaped. M: another data directory; the card drops `login`.
- [x] H16 — No session id in an export, a commit message, any file or stderr. M: `run` defaults to the session id.
- [x] H17 — A login's fifth session evicts its least recently used; the 65th is a 503; a restart ends every id; idle ends one (a unit). M: no cap; cross-login eviction; ended sessions counted.
- [x] H18 — Two `Mcp-Session-Id` values: 404, rmcp never called, the bound count equal to rmcp's. M: unbind every value.
- [x] H19 — Unknown tool names (typed, a custom-parsed request, a notification): `-32602` `tool not found` or 202, never reaching `handle`. M: hand it to rmcp; filter only typed `CallToolRequest`.
- [x] H20 — A call cancelled while waiting for the turn (disconnect, `notifications/cancelled`) stores nothing; a later one does. M: no race.

## Out of scope

The claude.ai half of the client check, the door's OAuth, deploy (RM-0064); credentials' issue (RM-0065); an inbox read tool; code, drift, review and task tools on a hub; a session store; `/mcp` locally and a stdio bridge; read scale (RM-0054); spec-ID collisions (RM-0073).

## Implementation

Three iterations: REWORK (two session ids on a DELETE slipped past S1's caps), REWORK (the unknown-tool filter missed untyped shapes), ACCEPT with four nits. All four nits are in `hub-mcp.md` "Known limits". Tests: 4828 of 4829 in the workspace; the one red, `show_bindings no_workspace_dependency_is_added_against_main`, clears once the dependencies are committed. UI 3522; clippy and `spec check` clean. Mutations: 34 on iteration 2 and 6 on iteration 3, all killed.

Standing deviations from the draft:
1. `HubSeam::turn(Command) -> TurnFuture`: a future of `()`, the answer carried in the command;
2. the services are built in `App::new` (`Mounts::new`), not `main.rs`;
3. `answer.rs` `found` is `pub(crate)`;
4. an unknown tool inside a session is framed as JSON, where rmcp itself answers on SSE;
5. a session-less legacy client gets 400 where rmcp gives 422.

Not covered end to end: the drop-safe bind, which no client can hit on purpose; H17's idle part and H19, killed only by unit tests of `src/mcp.rs`. The client check by hand is still to do (`hub-mcp.md` "Client check").

### Files in scope

| Module | What it does |
|---|---|
| mcp `hub.rs` (new), `server.rs`, `read.rs`, `intake.rs`, `cards.rs`, `lib.rs` | `HubSeam`, `Command`, `TurnFuture`, `turned` (the turn raced with the request's token), `HUB_TOOLS`, `HUB_ROUTER` (a `static`), `HUB_INSTRUCTIONS`, eight descriptions and their asserts; `SpecEngineServer`'s hub mode; hub argument types without `task_id`; the login into requests and card actors |
| CLI `propose.rs`, `create.rs`, `interpret.rs`, `amend.rs`, `intake.rs` | `login: Option<String>` on the seven requests, into `Author.login` |
| http `mcp.rs` (new), `app.rs`, `hub.rs`, `answer.rs` | `Mounts` (a service and session manager per project), `serve`, the session table, one id per request, the drop-safe bind, caps and eviction, the raw-JSON unknown-tool filter, the seam; the hub-only route; class `Mcp`, the query and `Authorization` rules; `found` shared |
| http `Cargo.toml`, root `Cargo.toml`, `Cargo.lock` | `rmcp` with `transport-streamable-http-server`, `specengine-mcp` by path; the seven crates |
| ui `inbox/ProposalCard.tsx`, `api/provisional.ts`, mocks | the `Login` row, escaped |
| Tests | http `hub_mcp.rs`, `common/mcp_http.rs`, `hub.rs`, `door.rs`, `hub_confirm.rs`, `src/hub.rs` and `src/mcp.rs` units; mcp `mcp_hub.rs`, `mcp_genre.rs`; eval `build_graph.rs`; UI `ProposalCard.test.tsx`, `smoke.test.tsx` |
