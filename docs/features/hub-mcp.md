---
class: spec
status: draft
scope: [crates/specengine-mcp, crates/specengine-http, crates/specengine-cli, crates/specengine-eval, ui]
ref: RM-0053; Phase 7 step 11 (08 s2); the hub-mcp analysis and the orchestrator's decisions Q1-Q8 of 2026-11-03, each awaiting the owner's review
---

# hub-mcp: `/mcp/<slug>` on a hub, sessions bound to a login, proposals and cards under it, nothing decided (RM-0053)

## Why

Since `hub-confirm` a lead confirms on a hub with no terminal, yet nothing on a hub raises a proposal: its queue holds departing accepts' discrepancies, the operator's raises on the host and imported orphans (`docs/canon/hub-team.md`, intro). A writer's AI is that person's hands, and today it reaches SpecEngine only over stdio, on a machine holding the project (`crates/specengine-mcp/README.md`). This slice mounts the existing server in a hub's daemon: one Streamable HTTP endpoint per project, behind the hub's fence, inside the project's turn, with a smaller tool set that reads, raises proposals and writes cards under the request's `Remote-User` and decides nothing. The daemon holds no credential: the proxy's door does (ADR-0034, ADR-0059). This step says which routers the MCP credential may open, trust item 6, and RM-0064 builds it.

Decided by the orchestrator on 2026-11-03 while the owner is away. **Each awaits the owner's review.**
- **Q1** The transport is rmcp 3.5.0's Tower service, feature `transport-streamable-http-server`, in `specengine-http`; no version change. Its new transitive crates are recorded under the owner's approval of new dependencies ("Dependency").
- **Q2** Card writes over `/mcp` act as the login; over stdio they stay `agent` (`docs/canon/cards.md` "Who acts").
- **Q3** The hub serves the 17 tools of "Data".
- **Q4** The credential's strength is RM-0064's and RM-0065's; the daemon depends on none.
- **Q5** A hub refuses `Authorization` on every route but `/mcp/<slug>`, by presence.
- **Q6** Spec-ID collisions stay a known limit, now RM-0073's.
- **Q7** ADR `hub-mcp`, drafted under "Canon at ship".
- **Q8** The daemon side first; the client check by hand after it.

The spec's own choices, **accepted by the orchestrator on 2026-11-03**: S1 the caps, S2 eviction at a login's cap and refusal at the hub's, S3 the mount, S4 where rmcp's feature is declared, S5 no opt-in flag ("Rules"). Also accepted: the `Authorization` refusal at fence step 5, holding `rand` at 0.10.2, H10's turn holder, H12 once the canon page exists, the room plans of "Canon at ship".

## Description and interactions

### The route

On a hub alone (`--public-origin`, `docs/canon/hub.md` "Entry"), `/mcp/<slug>` serves the MCP Streamable HTTP transport for the served project `<slug>`: POST, GET (a legacy session's SSE stream), DELETE (a legacy session's end), no query. A local daemon serves no such route on any method; its router and fence stay byte for byte. The stdio binary is unchanged: 29 tools, `INSTRUCTIONS`, its BLAKE3 pins.

Both protocol eras are served (`crates/specengine-mcp/README.md` "Protocol eras"). Claude Code opens a legacy session and GETs an SSE stream, which a stateless server answers 405 (07 s1.1). A 2026-07-28 client sends every request alone with `_meta` and holds no session (04 s3). rmcp routes each request: `initialize` up to 2025-11-25 opens a session (`Mcp-Session-Id`); a request with per-request protocol metadata, and `server/discover`, are served statelessly.

### A call

Past the fence the handler, http `mcp::serve`, finds the project by its slug, judged as `App::project_of` judges one; an unknown slug is its 404. It then judges the session binding ("Rules", **Sessions**) and hands the request to that project's `StreamableHttpService::handle` (rmcp `tower.rs:1805`, public and inherent). rmcp injects the request's `http::request::Parts` into each MCP message's extensions (`tower.rs:1523`, `:2069-2075`, `:2156-2157`, `:2276`), so the fence's `Login` reaches a tool with each request, never through a session.

The hub server runs every call that reads the corpus or writes the queue inside the project's turn, as `answer::run` does (`crates/specengine-http/src/answer.rs:206-231`): the project's mutex, `spawn_blocking`, the process's `Env`, `project.globals()`, the slug found again (`found`), one CLI library call. `initialize`, `notifications/*`, `ping`, `tools/list`, `resources/templates/list` and `server/discover` take no turn. A request dropped while it waits never runs; one running runs to its end and its answer is dropped, as over stdio (`docs/canon/mcp-read.md` "No project, discovery, state").

`specengine-mcp` names no HTTP crate (`docs/canon/hub-confirm.md` "Rules", **No agent path**; `tests/hub_confirm.rs` `b9_the_mcp_crate_names_no_http_crate`). It holds the hub server and a seam the daemon implements, `HubSeam`: `login(&rmcp::model::Extensions) -> Option<String>`, where the daemon reads `Parts.extensions`' `Login`, and `turn(command) -> Answer`, the daemon's executor. The transport, `Parts` and the turn stay in `specengine-http`.

### The tools

Seventeen tools and both resources, each the same CLI library call as its stdio twin: eight reads, four queue tools without `task_id`, five card tools whose writes act as the login ("Data"). They read, raise and write the queue, and decide nothing. A person decides with Confirm on the hub (`hub-confirm.md`) or on a terminal. The twelve tools left out need code, a terminal or a task. Raising is open to any login, as the five plain writes are: `/mcp` never reads `[team]`, so a bad one stops no proposal (ADR-0012). The hub's queue mints `PR-` from its floor once `apply-outputs` (RM-0072) has shipped.

### Identity

Each call reads the login from its own request. A proposal's `Author` is `{type: agent, role, model, run, login}`: `role`, `model` and `run` the agent declares per call, as over stdio, never taken from `clientInfo` or a session id; `login` the request's. `Proposed-by:` then ends ` login=<login>` (core `Author::provenance`). A card write's `actor` is the login, so a flow's `by` holds against the person (`hub.md` "Identity"). No hub schema holds `login`, `as`, `actor` or `task_id`.

### The client check, by hand (Q8)

Run once H1-H17 pass; record each finding in `docs/canon/hub-mcp.md` "Client check" with the client's version and the date, as `mcp-read.md` "Owner's check" records AC-16.

1. **Claude Code, on the laptop, in this slice.** `specengine-http --root <scratch copy of fixtures/spec-a> --public-origin https://<name>:<port>`, where `<name>` resolves to loopback and is not `localhost`, which the flag refuses. In front of it, a local TLS proxy: `basicAuth` on `/mcp` alone with `headerField: Remote-User`, its certificate trusted by Claude Code through `NODE_EXTRA_CA_CERTS`. Then `claude mcp add --transport http lantern https://<name>:<port>/mcp/lantern-keep --header "Authorization: Basic <base64 of mira:...>"`. Record:
   - the session opens and the GET stream stays open;
   - `/mcp` lists the 17 tools, and the instructions reach the model;
   - `get_tree` answers;
   - `propose_change` stores `login` `mira`, and `create_card` records `mira`;
   - a second login gets sessions of its own;
   - with no header, the proxy answers and the daemon is never reached.
2. **claude.ai and Claude Desktop, in RM-0064, on its public origin.** Does the connector's OAuth client send PAR, which Authelia requires of bearer-authz clients? Does it send an `Origin` header, which the fence refuses unless it is the hub's? Which `Remote-User` does a bearer request get? Until this is run, "writers install nothing" is a working answer.

## Data

**Route and fence** (http `hub.rs` `class`, `app.rs`), `<seg>` one non-empty raw segment:

| Request | Hub | Local daemon |
|---|---|---|
| POST, DELETE `/mcp/<seg>` | class `Mcp`: any login, no query | unknown-route 404 |
| GET `/mcp/<seg>` | as every GET, but no query | unknown-route 404 |
| any query on `/mcp/<seg>`, `?` alone too | 403 `refused: this route takes no query` | unknown-route 404 |
| any other method on `/mcp/<seg>`; POST `/mcp/<seg>/x` | 403 `refused: this hub does not serve this route` | unknown-route 404 |
| GET `/mcp`, `/mcp/`, `/mcp/<seg>/x` | unknown-route 404 | unknown-route 404 |
| `/mcp/<unknown slug>`, any listed method | the unknown-slug 404 (`project_of`'s body) | unknown-route 404 |
| `Authorization` on any route but `/mcp/<seg>` | 403, step 5, before the class | never read |

**Answers this slice adds**, each in the daemon's error body but the 404, which is rmcp's own bytes for an unknown session (`tower.rs:1921`, `:2057`), sent the same on every method:

```text
refused: the Authorization header is for /mcp alone: this route takes the login door's session
refused: this hub holds 64 MCP sessions, its cap: try again once one ends
Not Found: Session not found
```

**rmcp's configuration**, one `StreamableHttpService` per served project, with a `LocalSessionManager` the daemon keeps a handle to:

| Key | Value | Why |
|---|---|---|
| `legacy_session_mode` | `true` (default) | Claude Code's legacy session and GET stream |
| `stateless_protocol_metadata_required` | `false` (default) | legacy clients send none |
| `json_response` | `false` (default) | SSE responses |
| `sse_keep_alive`, `sse_retry` | 15 s, 3 s (defaults) | |
| `allowed_hosts` | the hub's authority alone | the default is loopback only |
| `allowed_origins` | the hub's origin alone | the fence's rule, judged twice |
| `max_request_body_bytes` | 4 MiB (default) | the confirm's cap |
| `session_store` | none | stored nowhere |
| `cancellation_token` | the daemon's shutdown | |
| `SessionConfig.keep_alive` | 300 s (default, `session/local.rs` `DEFAULT_KEEP_ALIVE`) | the idle end |
| the hub server's `Lifecycle` | `Auto` | both eras |

**Sessions** (http `mcp.rs`, memory only): session id -> `{slug, login, last_used}`. `MCP_SESSIONS_PER_LOGIN: usize = 4`, per login per project; `MCP_SESSIONS_MAX: usize = 64`, per hub. Ids are rmcp's, a UUID v4 (`transport/common/server_side_http.rs:16-18`). rmcp keeps them in a `HashMap` with no cap (`session/local.rs:34`). A worker idle past `keep_alive` quits (`session/local.rs:1107-1126`).

**Tools**, the hub's `tools/list`:

| Tool | On a hub, against its stdio twin |
|---|---|
| `get_tree`, `get_node`, `search`, `get_context_bundle`, `get_roadmap`, `get_flow`, `get_trace`, `get_proposal` | the same schema, description and answer; in the turn |
| `propose_change` (four kinds, `dry_run`), `ask_question`, `report_discrepancy`, `report_decision` | no `task_id`; `Author.login` the login; hub descriptions |
| `list_cards`, `get_card` | the same |
| `create_card`, `edit_card`, `move_card` | `actor` the login; hub descriptions |
| resources `spec://<slug>/tree`, `spec://<slug>/node/{id}` | the same; in the turn |

Left out, twelve: `get_impact`, `find_symbols`, `refs`, `unmapped`, `check_binding` (a hub holds no code, its image no toolchain); `report_drift` (it judges code; drift stays a terminal's); `review_proposal` (the forms are stage-only and local); `get_task`, `claim_task`, `submit_plan`, `report_run`, `complete_task` (no `T-` flow runs on a hub).

**Texts** (mcp, new `hub.rs`): `HUB_INSTRUCTIONS`, ASCII, at most 2 048 bytes (`TEXT_LIMIT`). In this order:
- the opening;
- the reads with their twins;
- the queue: what it writes, and "a person decides with Confirm, on this hub or on a terminal";
- the cards: "you act as your person's login";
- REF, the cut, the resources;
- the rules.

No left-out tool is named, nor `task_id`. The four queue descriptions and the three card-write ones get hub variants:
- no `task_id:` line;
- the owner's `spec approve PR` and `spec reject PR` become a Confirm on the hub or a terminal;
- "You act as `agent`: ..." becomes "You act as your person's login: a move a flow gives to named people refuses you unless it lists that login".

The other ten descriptions are shared. The stdio `INSTRUCTIONS` hold 2 048 bytes with none left (`crates/specengine-mcp/src/server.rs:63-68`), so they do not change.

**A proposal raised as `mira`**, stored author, then the confirmed commit's trailer:

```json
{"type":"agent","role":"drafter","model":"m-1","run":"r-1","login":"mira"}
```

```text
Proposed-by: agent role=drafter model=m-1 run=r-1 login=mira
```

**An exchange**, legacy then stateless (headers the proxy sets as Traefik does; `Authorization` may reach the daemon on `/mcp`, unread):

```text
POST /mcp/lantern-keep HTTP/1.1
Host: hub.example.org
Remote-User: mira
Content-Type: application/json
Accept: application/json, text/event-stream

{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"c","version":"1"}}}

HTTP/1.1 200 OK
Mcp-Session-Id: 3f2c9d5e-0b7a-4c1e-9a43-5d8e2f61b0c4
Content-Type: text/event-stream
Cache-Control: no-store
```

```json
{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"get_tree","arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}
```

The stateless request also carries `MCP-Protocol-Version: 2026-07-28` and no `Mcp-Session-Id`.

<a id="dependency"></a>
**Dependency.** Feature `transport-streamable-http-server` (rmcp `Cargo.toml`: `-session`, `server-side-http`, `transport-worker`) adds these crates to the daemon's normal graph. Versions are read from the root `Cargo.lock`, from the local cargo registry's sources, and from rmcp 3.5.0's own `Cargo.lock` shipped in its source:

| Crate | Version | Licence | Through | Version read from |
|---|---|---|---|---|
| `rand` | 0.10.2 | MIT OR Apache-2.0 | `server-side-http` | registry source; rmcp's lock holds 0.10.3 |
| `rand_core` | 0.10.1 | MIT OR Apache-2.0 | `rand`; `getrandom` 0.4.3 gains `sys_rng` | registry source, rmcp's lock |
| `chacha20` | 0.10.2 | MIT OR Apache-2.0 | `rand` `std_rng` | registry source, rmcp's lock |
| `base64` | 0.23.1 | MIT OR Apache-2.0 | `server-side-http` | registry source, rmcp's lock |
| `tokio-stream` | 0.1.19 | MIT | `-session`, `transport-worker` | the registry cache's `.crate`, rmcp's lock |
| `sse-stream` | 0.2.6 | not yet read: the developer records it before ship (below) | `server-side-http` | rmcp's lock alone |
| `async-trait` | 0.1.92 (a proc macro on `syn` 3.0.6) | not yet read: the developer records it before ship (below) | `-session` | rmcp's lock, the index cache |

Reused from the lock: `bytes` 1.12.1, `http` 1.5.0, `http-body` 1.1.0, `http-body-util` 0.1.5, `tower-service` 0.3.3 (all through axum); `uuid` 1.26.1 and `getrandom` 0.4.3 (rmcp's `server`); `cpufeatures` 0.3.1, `cfg-if` 1.0.5, `futures-core`, `futures-util` 0.3.34, `pin-project-lite` 0.2.17, `syn` 3.0.6, `quote` 1.0.47, `proc-macro2` 1.0.107.

Recorded under the owner's approval of new dependencies; the seven crates and their versions await the owner's confirmation. The lock holds them at these versions. WHEN the resolver picks another THEN the developer reports it and this row changes before ship, since a version is the owner's (`docs/canon/stack.md`).

**The two licences not read here** (accepted by the orchestrator, 2026-11-03). Before ship, the developer reads the `license` field of `sse-stream` 0.2.6 and `async-trait` 0.1.92 from the fetched crates' manifests and records each in this table. WHEN either expression is not met by MIT or Apache-2.0 alone THEN the slice SHALL stop for the owner, unshipped. That holds even for a licence `build_graph.rs`'s `ALLOWED_LICENCES` would admit (BSD-3-Clause, ISC and the rest).

**Bumps:**
- `GET_ROUTES` 25 to 26, or 27 after 10b's `/sync`;
- the fence's class `Mcp` and three answers;
- `HTTP_DEPENDENCIES` 8 to 10;
- seven transitive crates;
- seven CLI requests gain `login`;
- the proposal card's login row.

No queue schema, event, `Author` field (`login` exists since `hub-confirm`), dump, `daemon.json`, daemon-keys set, flag or CLI command changes. Neither does any stdio tool, schema or text, nor the plugin.

## Rules and edge cases

- **The fence class.** WHEN a hub's fence meets `/mcp/<seg>` THEN:
  - POST and DELETE SHALL be class `Mcp`: any login, `[team]` never read;
  - GET SHALL pass as every GET does;
  - a query on any method SHALL be refused, judged before `Hub::fenced`'s GET return.

  Every other method, and `/mcp/<seg>/...`, keeps default deny. `Host`, `Origin`, `Sec-Fetch-Site` (absent, `same-origin` or `none`) and `Remote-User` are judged as on every route. The board's `same_origin` write door (`app.rs`) is not asked of `Mcp`, since no MCP client sends `Sec-Fetch-Site`; rmcp asks JSON and `Accept` itself (415, 406). A local daemon changes in nothing.
- **`Authorization` (Q5).** WHEN a hub request carries `Authorization`, once or more, any value, on a path other than `/mcp/<seg>` THEN the fence SHALL refuse it at step 5, before the class, nothing read. On `/mcp/<seg>` it passes: its value is never read, logged or stored, and rmcp, built without `auth`, ignores it. A browser holding Basic credentials for a path outside `/mcp` is refused there: a door, failing closed.
- **The turn.** WHEN a hub call reads the corpus, reads or writes the queue, or reads a resource THEN it SHALL run through `HubSeam::turn`, as "A call" says. The stdio `call` stays byte for byte (`read.rs` `call`, `spawn_blocking` with the server's globals). Reads keep MCP's `View::Capped` (`crates/specengine-cli/src/cap.rs:75-85`): the 40 000-character cut, its tails and `MAX_RESULT_CHARS`, never the daemon's `View::Browser`. A changed slug answers the CLI's exit-2 line as an `isError`, as the daemon's 503 does.
- **Sessions.**
  - **Bind.** WHEN rmcp answers a POST that carried no `Mcp-Session-Id` with a 2xx carrying one THEN the daemon SHALL bind that id to the request's slug and login.
  - **Judge.** WHEN a request names an `Mcp-Session-Id` not bound to its own slug and login THEN the daemon SHALL answer 404 with "Data"'s rmcp bytes, on every method, rmcp not called, nothing run. The id may be unknown, ended, another login's or another project's; all four answer alike, so no login learns of another's session.
  - **S1, the caps.** At most 4 sessions per login per project, at most 64 per hub. Only sessions rmcp still holds count: the daemon asks `has_session` before it counts.
  - **S2, eviction and refusal.**
    - WHEN an `initialize` gives a login its fifth session in a project THEN the daemon SHALL close that login's least recently used session there (`close_session`) and keep the new one. The closed client meets a 404 and opens another, since a 2025-11-25 client re-initializes on a 404.
    - WHEN the hub then holds 65 THEN the daemon SHALL close the new session and answer 503 with "Data"'s line and no `Mcp-Session-Id`. It never closes another login's session.
    - Rejected: refusing at a login's cap, which locks a person out for up to 300 s after clients exit without DELETE; evicting across logins, which lets one person end another's sessions.
  - **Life.** A session ends at a restart, after 300 s idle, at a DELETE by its own login, or at eviction. The stateless era opens none.
  - **Stored nowhere.** No session id appears in the queue, an event, a dump, a trailer, a commit, an answer body or the daemon's stderr. The daemon installs no `tracing` subscriber, so rmcp's events print nothing.
  - **Identifies no one.** A session only refuses. It is never a login, an author, an actor, a `run` or an argument.
- **Identity (Q2).**
  - WHEN a hub queue tool stores a proposal THEN its `Author.login` SHALL be the request's login, through the seven CLI requests' new `login` (`ProposeRequest`, `CreateRequest`, `InterpretRequest`, `AmendmentRequest`, `QuestionRequest`, `DiscrepancyRequest`, `DecisionRequest`). Only the hub sets it: no clap flag, no stdio tool. Stdio rows, dumps and trailers stay byte for byte, with no `login` key.
  - WHEN a hub card write runs THEN its `actor` SHALL be the login, never `TOOL_ACTOR`. The fence has already judged the login (`hub_login_problem`).
  - WHEN a hub tool that writes finds no `Login` THEN it SHALL store nothing and answer `isError` with the fence's `refused: this hub needs one Remote-User header from its proxy`. The fence makes this unreachable; the tool fails closed.
- **The door (Q3, Q7b).** No hub tool approves, rejects, stages, accepts, clears, confirms, claims, judges drift, reviews or rounds. The daemon raises proposals for `/mcp` alone, on ADR-0057's precedent of a proposing daemon: http `src` names `specengine_mcp` in `mcp.rs` alone, and still no `propose*` call (`tests/door.rs`). `ac03_no_endpoint_changes_the_repository_or_the_queue` runs on a local daemon, unchanged.
- **The texts' budget.** `HUB_INSTRUCTIONS` and each hub description SHALL be ASCII and at most 2 048 bytes, under compile-time asserts, as stdio's. Each description holds `Deterministic: one state, one result; no LLM inside.`, and each number a text states is asserted against its CLI constant. `mcp_genre.rs` already scans every mcp `src` literal, the hub texts with it, so they hold no trade or stack word. The stdio texts do not move.
- **Trust item 6 (Q7c).** It lives in `docs/canon/hub-mcp.md` "Trust", the rule in its own fenced block, the page's only fenced line starting `^/`:

  ```text
  ^/mcp/[^/]+$
  ```

  The credential an MCP client holds opens only the routers matching this rule. Every other router reaching the daemon, the confirm class's four first, takes the login door's browser session alone. The credential's `Remote-User` is the person's browser login, byte for byte. A unit test reads the rule from that page, as `the_step_up_regex_is_the_confirm_class` reads item 5 from `hub.md`. The page lands in this slice's commit, so the test-engineer runs H12 once it exists.
- **S3, the mount.** `.route("/mcp/{p}", any(mcp::serve))`, in `app.rs`'s hub-only block beside `/team` and `/confirm`. `serve` calls `handle`, not `tower_service::Service`, so no direct `tower-service` dependency. There is no `.route_service(`, so `the_get_routes_are_pinned` (`tests/hub.rs:1477-1483`) reads the route unchanged, and `GET_ROUTES` gains `/mcp/{p}` alone.
- **S4, the feature.** `specengine-http`'s manifest declares `rmcp = { workspace = true, features = ["transport-streamable-http-server"] }`, plus `specengine-mcp` by path. The root pin and `specengine-mcp`'s manifest are unchanged, since a member's extra features are its own (`stack.md`). A workspace build unifies features, so the stdio binary built beside the daemon compiles rmcp's transport, unused. `cargo build -p specengine-mcp` does not, nor does `cargo install` of it.
- **S5, no flag.** `/mcp` is on whenever `--public-origin` is: that flag is the whole switch (ADR-0058). The proxy's routers decide whether a client reaches it.
- **`Origin`.** The fence refuses any `Origin` but the hub's own. Whether claude.ai sends one is the client check's. Admitting another would be a new decision, never a widened fence.
- **Known limits.**
  - **Load.** AI reads wait in the turn beside the page's, until `scale-reads` (RM-0054).
  - **Other people's text.** A login's AI reads others' proposals and cards. They are data: no tool decides, and the UI renders them as text.
  - **Login shapes.** A bearer `Remote-User` that is an email or a UUID splits a person into two logins; RM-0064 guarantees the browser login.
  - **The corpus leaves the hub.** Answers go to the client's model provider, the project's choice to allow.
  - **Writers.** "Writers install nothing" breaks if no door gives claude.ai a per-person credential; Claude Code still works.
  - **Streams.** A GET stream outlives a login door session, as `tail.rs`'s does (`hub.md` "Known limits").
  - **A forged `Remote-User`** raises proposals as anyone: no real use before RM-0064's "Done".
  - **Code reads.** `with: ["bindings"]` reads no code on a hub's sparse checkout.
  - **One project's churn.** Five clients of one login on one project evict each other in turn.
  - **Spec IDs.** Spec-ID collisions stay as `apply-outputs` states them: RM-0073.

## Acceptance criteria

The tests serve `fixtures/spec-a` (`lantern-keep`) and `-b` (`zerkalo`) copies with `specengine-http --public-origin https://hub.example.org`. The test proxy sends `tests/hub.rs`'s headers, a scratch `HOME` holds the data, and an HTTP MCP client lives in `tests/common`. M marks the mutation that must turn each red.

- [ ] H1 — Fence and route.
  - POST `/mcp/lantern-keep` with no `Remote-User`, with two, or with `agent`: 403 with the fence's line; the queue's events unchanged.
  - PUT: 403 `refused: this hub does not serve this route`.
  - GET, POST or DELETE with `?x=1`, or a bare `?`: 403 `refused: this route takes no query`.
  - GET `/mcp`, `/mcp/` and `/mcp/lantern-keep/x`: the unknown-route 404. POST `/mcp/lantern-keep/x`: the 403 above. POST `/mcp/nope`: the unknown-slug 404. None of these runs anything.
  - A local daemon answers every method on `/mcp/lantern-keep`, with `Remote-User: mira` too, with the unknown-route 404. `tests/fence.rs` and `methods.rs` pass unchanged.

  M: mount it on a local daemon; judge the query after GET's return.
- [ ] H2 — Legacy session.
  - `initialize` (2025-11-25) as `mira`: 200 with one `Mcp-Session-Id`. `notifications/initialized` on it: 202.
  - GET on it: `text/event-stream`, still open after 20 s, with a keep-alive comment received.
  - `tools/list`: exactly "Data"'s 17 names.
  - DELETE: 202; then any request naming it: 404.

  M: `legacy_session_mode: false`; rmcp's default `allowed_hosts`.
- [ ] H3 — Stateless era. A 2026-07-28 `tools/call get_tree {}` with `_meta` and `MCP-Protocol-Version`, no session: the tree, no `Mcp-Session-Id`, the session table still empty. `server/discover` returns `HUB_INSTRUCTIONS` byte for byte. M: serve the hub as `Lifecycle::Legacy`.
- [ ] H4 — A session belongs to its login. On a session `mira` opened:
  - `create_card` as `tomas`: 404, status and body equal to a never-issued id's; no card stored; events unchanged.
  - GET and DELETE as `tomas`: 404, and `mira`'s next `tools/list` on it is 200.
  - The id at `/mcp/zerkalo` as `mira`: 404.

  M: no binding; binding by id alone.
- [ ] H5 — Identity to author.
  - `propose_change` (update) as `mira`, role `drafter`, model `m-1`, run `r-1`, stores "Data"'s author.
  - Confirmed by `POST .../proposals/<id>/confirm` as the `[team]` `owner` `tomas`: the commit holds "Data"'s `Proposed-by:`, and `Decided-by` names `tomas`.
  - `ask_question`, `report_discrepancy` and `report_decision` store `login` `mira` too.
  - The same four over stdio store no `login` key, their rows byte for byte as before.

  M: drop the login; take it from an argument.
- [ ] H6 — No identity or task argument. No input schema of the hub's `tools/list` holds `login`, `as`, `actor` or `task_id`. Each of the seven hub writes called with one added: `isError` beginning `failed to deserialize parameters`, nothing stored. M: keep `task_id` in the hub schema.
- [ ] H7 — Cards act as the login.
  - `create_card` and `move_card` as `mira` record actor `mira`.
  - A move whose flow says `by = ["tomas"]` is refused; with `by = ["mira"]` it moves.
  - Over stdio the same calls record `agent`.

  M: `TOOL_ACTOR` on the hub.
- [ ] H8 — One door on a hub (08 AC-3). Every one of the 17 tools is called with valid and with invalid arguments, in both eras, `dry_run` too, and both resources are read. Then:
  - `git status --porcelain=v1 -z --untracked-files=all --ignored` is empty;
  - `HEAD`, every ref and every worktree file are unchanged;
  - every path created is under the scratch `HOME`.

  A tool in `tools/list` the test does not call fails it. M: a hub tool writes under the root; a new tool with no call.
- [ ] H9 — Nothing decides.
  - The hub's `tools/list` is exactly the 17. No name holds `approve`, `reject`, `stage`, `accept`, `clear`, `confirm`, `claim`, `drift`, `review`, `round` or `task`.
  - The stdio `tools/list` (29) and `INSTRUCTIONS`' BLAKE3 pins (`mcp_decision.rs`, `mcp_path.rs`) are unchanged.
  - `b9_the_mcp_crate_names_no_http_crate` and `mcp_direct_dependencies_are_the_cli_library_and_the_protocol_crates` pass unchanged.
  - `door.rs` finds `specengine_mcp` in http `src/mcp.rs` alone.

  M: `review_proposal` or `report_drift` in the hub set; rmcp's HTTP feature in `specengine-mcp`'s manifest.
- [ ] H10 — The turn. A confirm on `lantern-keep` whose `pre-commit` hook sleeps 3 s holds that turn. Meanwhile:
  - `get_tree` there has not answered after 300 ms, and answers once the confirm has;
  - `get_tree` on `zerkalo` answers within 1 s;
  - `tools/list`, `ping` and `initialize` on `lantern-keep` answer within 1 s.

  M: the call outside the turn; `tools/list` inside it.
- [ ] H11 — Texts.
  - `HUB_INSTRUCTIONS` is ASCII and at most 2 048 bytes; it names each of the 17, and none of the 12 nor `task_id`.
  - Each hub description is ASCII and at most 2 048 bytes, and holds the determinism sentence.
  - The seven variants name no `spec approve`, `spec reject`, `task_id` or ``You act as `agent` ``.
  - Compile-time asserts carry each limit; `mcp_genre.rs` covers the hub texts.

  M: `claim_task` in `HUB_INSTRUCTIONS` turns a test red; padding it to 2 049 bytes fails the build.
- [ ] H12 — Item 6's rule. A unit test of http `src/hub.rs` reads the fenced rule of `docs/canon/hub-mcp.md` "Trust". It matches the router's `/mcp/{p}` with each served slug. It matches no other route, `GET_ROUTES`' and `class`'s writes with slugs and IDs filled in. Item 5's rule (`hub.md`) matches no `/mcp` path. M: serve MCP at `/api/projects/{p}/mcp`.
- [ ] H13 — `Authorization` outside `/mcp`. On a hub, `Authorization: Basic bWlyYTp4` with a valid `Remote-User`:
  - on GET `/`, GET `.../tree`, POST `.../proposals/<id>/decision` and POST `.../proposals/<id>/confirm`: 403 with "Data"'s line, nothing stored;
  - on POST `/mcp/lantern-keep`: answered as without it.

  A local daemon answers every route as without it. M: drop the check; refuse it on `/mcp` too.
- [ ] H14 — Parity. A scratch corpus whose `get_tree {}` passes 40 000 characters. `get_tree {}`, `get_node` of its largest document and `search` of a common term, over `/mcp/<slug>`: `content` is byte-equal to `specengine-mcp --root <root>`'s under the same `HOME`, and `structuredContent` is equal as JSON, the cut and its tail included. M: the hub reads with `View::Browser`.
- [ ] H15 — Live. With `GET .../events` open as `tomas`, a `report_discrepancy` as `mira` over `/mcp` arrives within 1 s, and `GET .../inbox` lists it with `author.login` `mira`. UI, jsdom, mocked client: the proposal card shows `Login` `mira` as text, and a login holding `<b>` and U+202E shows them escaped, with no `b` element. M: the hub server on another data directory; the card drops `login`.
- [ ] H16 — The session id is stored nowhere. After H2-H8, no issued session id appears in `spec export state`'s output, in `git log --all --format=%B`, in the bytes of any file under the root or `HOME`, the SQLite files included, or in the daemon's stderr. M: `run` defaults to the session id.
- [ ] H17 — Bounded sessions (S1, S2).
  - `mira`'s fifth `initialize` on `lantern-keep` answers 200 with a new id. Her least recently used session then answers 404, her other three and the new one 200; `tomas`'s are untouched. Her fifth on `zerkalo` evicts nothing on `lantern-keep`.
  - Sixteen logins at four sessions each fill the hub. A seventeenth login's `initialize` answers 503 with "Data"'s line and no `Mcp-Session-Id`, and the 64 still answer.
  - After a restart every old id answers 404.
  - A unit test of http `src/mcp.rs`, with an idle of 1 s: an idle session answers 404 and frees its place.

  M: no cap; evict another login's session; count sessions rmcp has ended.

## Out of scope

- The claude.ai half of the client check, and the door's OAuth (DCR, CIMD, PAR): RM-0064. The credential's issue and revocation: RM-0065.
- An inbox read tool, which waits for the writers' trial; any new tool.
- On a hub: code tools, `get_impact`, `report_drift`, `review_proposal`, task tools, `T-` flows; `assign`, `cancel`, `accept` and `clear` tools; elicitation; prompts.
- A session store, or sessions that survive a restart.
- `/mcp` on a local daemon, and the stdio bridge to the daemon (05 s1 (6)).
- Hardening headers and Traefik's SSE settings: RM-0064. Read scale: RM-0054.
- Spec-ID collisions: RM-0073.
- Rejected:
  - the Tower service inside `specengine-mcp`;
  - a login read from the session;
  - an `--mcp` flag;
  - one endpoint `/mcp` for every project.

## Implementation

To fill in after implementation.

### Files in scope

| Module | What it changes |
|---|---|
| mcp, new `hub.rs` | the hub mode: `SpecEngineServer::hub(Arc<dyn HubSeam>)`, the 17-tool router, `HUB_INSTRUCTIONS`, the seven description variants and their asserts, `HubSeam` |
| mcp `read.rs`, `server.rs`, `lib.rs` | `call` and the resource reads through the seam on a hub, stdio byte for byte; `get_info`'s hub instructions; re-exports |
| mcp `intake.rs`, `cards.rs` | hub argument types without `task_id`; the login into the CLI request; card writes' `actor` the login |
| CLI `propose.rs`, `create.rs`, `interpret.rs`, `amend.rs`, `intake.rs` | `login: Option<String>` on the seven requests, into `Author.login` after `Author::new` |
| http, new `mcp.rs` | per project a `StreamableHttpService` (the config of "Data"); `serve`; the session table, binding, caps, eviction, the 503; `HubSeam` (the login from `Parts`, the turn as `answer::run`) |
| http `app.rs`, `hub.rs`, `main.rs` | the hub-only route; class `Mcp`, the query rule on every method, the `Authorization` refusal; `main.rs` has `mcp.rs` build the services at start and cancel them at shutdown, naming no `specengine_mcp` item itself |
| http `Cargo.toml`, root `Cargo.lock` | `rmcp` with the feature, `specengine-mcp` by path; the seven crates at "Dependency"'s versions; the two licences read from the fetched crates and reported for "Dependency"'s table |
| ui `src/inbox/ProposalCard.tsx`, `src/mocks/` | the `Login` row, escaped as `shownConfirmText`; a mock proposal with a login |

**Pins, decided changes** (eval `tests/build_graph.rs`; never edited merely to pass):
- `HTTP_DEPENDENCIES` (`:1943-1952`): 8 to 10, adding `rmcp` and `specengine-mcp`.
- `http_direct_dependencies_are_the_cli_library_axum_and_tokio` (`:1999-2008`, and the path check above it): `specengine-mcp` by path, one key, as `specengine-cli`; `rmcp` takes `workspace` and exactly `features = ["transport-streamable-http-server"]`; `tokio` unchanged.
- The licence test (`:2316`): its `wanted` adds `rmcp` and `sse-stream`.
- `daemon_pins_are_the_approved_ones`: the seven crates resolve once, at "Dependency"'s versions (`locked_versions`), as `rust-embed-impl`'s hold.
- Unchanged and green: the root rmcp features (`:1663-1672`, S4) and axum's features (`:2197`), since rmcp's HTTP transport names no axum.

`tests/hub.rs` `GET_ROUTES` gains `/mcp/{p}`; the fence-order test adds step 5's `Authorization`.

**Tests:**
- http: new `tests/hub_mcp.rs` (H1-H11, H13-H17); `tests/common/mod.rs`, an HTTP MCP client for both eras, JSON-RPC over POST, `Mcp-Session-Id`, SSE frames; `tests/hub.rs`; `tests/door.rs`; units in `src/hub.rs` (H12) and `src/mcp.rs` (H17's idle and caps).
- mcp: `tests/mcp_genre.rs` (H11); the existing stdio pins.
- eval: `build_graph.rs`.
- UI: new `src/inbox/ProposalCard.test.tsx` (H15).

### Canon at ship

Nothing below is written before this slice's commit. Free bytes are as of 2026-11-03, before 10a and 10b; a page without room rewords in place, and a cap is never raised.

- **New `docs/canon/hub-mcp.md`** (Tier 2; `docs/index-canon.md` has 743 B free, before 10b's line). The intro carries ADR `hub-mcp`, "built by `hub-mcp` (RM-0053)" and code pointers. Then:
  - "Route": the route and fence class, and `Authorization`;
  - "Sessions": binding, caps, eviction, life, stored nowhere;
  - "Turn";
  - "Tools": the 17 and the 12 left out;
  - "Texts";
  - "Identity";
  - "Trust" (`<a id="trust"></a>`): what the daemon defends and cannot, and item 6 with its fenced rule, the page's one fenced `^/` line;
  - "Client check", "Known limits", "Tests".

  `hub.md`, `hub-confirm.md`, `hub-team.md`, `cards.md`, `serve.md` and both READMEs point at it.
- **`hub.md`** (24 B free; after 10a and 10b, about 27 B):
  - Fence step 5 adds "; `/mcp`'s class and `Authorization`: `hub-mcp.md`".
  - "Routes": the 82 B "MCP is untouched: no `/mcp` route, tools keep acting as `agent` locally (RM-0053)." becomes "MCP: `/mcp/<slug>`, a hub's alone (`hub-mcp.md`)."
  - "Trust": "no account, token, key or session" becomes "... or login session (an MCP session: `hub-mcp.md`)", joined with 10b's words. The guarantee list adds "6. The MCP credential's routers: `hub-mcp.md#trust`."
  - For room, "Known limits" **A launcher attaches to a hub** (245 B) moves to `serve.md` "Known limits".
- **`architecture.md`** (1 B), `#distribution`: "session" becomes "login session", and ADR `hub-mcp` joins its list. The ~16 B come from rewording that paragraph, as 10b finds its own room.
- **`cards.md`** (7 B):
  - "MCP": "acting as `agent`" becomes "acting as `agent`, over a hub's `/mcp` as the login (`hub-mcp.md`)".
  - Room, ~45 B: "Who acts" drops its repeat of that rule.
- **`hub-team.md`** (52 B; 10b takes ~35): the intro's "No route proposes until RM-0053" becomes "`/mcp` proposes under a login (`hub-mcp.md`)"; no growth.
- **`hub-confirm.md`** (2 287 B):
  - **No agent path**: "No MCP tool" becomes "No MCP tool, over stdio or a hub's `/mcp` (`hub-mcp.md`)".
  - "Known limits", a forged `Remote-User`: adds "and raises proposals as anyone".
- **`serve.md`** (2 540 B):
  - "Concurrency": a hub's `/mcp` calls take the turn, and its handshakes and lists take none.
  - "Known limits": the launcher bullet from `hub.md`.
  - A new "Live tail": the http README's second "Live tail" paragraph, ~720 B.
- **`stack.md`** (6 164 B), the rmcp row:
  - `specengine-http` adds `transport-streamable-http-server`;
  - "would mount in axum" becomes "mounts in axum at a hub's `/mcp/<slug>` (`hub-mcp.md`)";
  - the seven crates with versions and licences, "recorded under the owner's approval of new dependencies; they await the owner's confirmation", as the `rust-embed` row says.
- **Unchanged:** `mcp-read.md` (5 B), the stdio contract; `agent-intake.md`; ADR-0034, ADR-0057 and ADR-0059, never edited, since ADR `hub-mcp` supplements them.
- **Tier 1:**
  - **http README** (23 B): Dependencies add `rmcp` (`transport-streamable-http-server`) and `specengine-mcp`. "One door" adds "a hub's `/mcp` raises proposals and writes cards through `specengine-mcp`'s hub server (`hub-mcp.md`)". "Tests" adds `hub_mcp.rs`. Room: "Live tail"'s second paragraph moves to `serve.md`.
  - **mcp README** (954 B): the intro names the hub mode, and "No HTTP stack in the graph" becomes "no HTTP crate in its manifest or its own graph (S4)"; "Modules" adds `hub.rs`.
  - **CLI README** (7 B): unchanged; `hub-mcp.md` "Identity" names the requests' `login`.
  - **`ui/README.md`** (2 054 B) "Screen rules": a proposal's login is shown as text.
- **`CLAUDE.md`:** `hub-mcp` shipped; the ADRs up to `hub-mcp`; "The hub goes on from step 11b, RM-0064"; Interfaces: "MCP over stdio and on a hub".
- **Specs:**
  - 07: s1.1's "HTTP `/mcp` comes later" and the `/mcp` row (`:141`) become built, on a hub, as `/mcp/<slug>`.
  - 08: step 11 shipped, its ADR column `hub-mcp`.
  - RM-0053 shipped.
  - RM-0064 gains the bullet below and, in "Done", the credential stopping at the door.
  - This file compressed.

The bullet for RM-0064:

```markdown
- **What `hub-mcp` stands on.** The daemon cannot tell an MCP client's `Remote-User` from a browser's, so this step guarantees item 6 of `docs/canon/hub-mcp.md#trust`. Two door paths: the routers matching `^/mcp/[^/]+$` take the MCP credential, and every other router takes the browser session alone. The scrub of identity headers holds on `/mcp` too: only the door sets `Remote-User`. The credential's `Remote-User` is the person's browser login, byte for byte: never a `client_credentials` subject `oauth2:client:<id>`, an email or a UUID, which would split one person in two. `Authorization` stays in place on every router, so the daemon's refusal outside `/mcp` is a second line. A `deploy/` test pins the rule's bytes to the canon's, as item 5's. Checked by hand: the credential stops at the door on `.../confirm`, `.../decision`, `.../tree` and `/`; SSE passes Traefik unbuffered, with idle timeouts above the 15 s keep-alive; the claude.ai half of RM-0053's client check runs on this public origin. Which credential to use, a Basic file `spec admin` keeps or OAuth through a door with DCR or CIMD, is this step's and RM-0065's, the owner's choice.
```

The draft ADR takes the next free number after 10b's `hub-push`; `board-snapshot` (10c) waits for the owner's answer. It is 1 357 B with its front-matter; the date is set at ship.

```markdown
---
class: decision
id: ADR-0065
title: A hub serves MCP at /mcp/<slug> under each request's login; its protocol sessions identify no one
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/hub-mcp.md#trust
ref: hub-mcp (RM-0053); the orchestrator's decisions of 2026-11-03 (Q1-Q8), the owner's review open
---

# ADR-0065

**Decision.** Supplements ADR-0034 and ADR-0059, whose "no session" means a login's. On a hub alone, `/mcp/<slug>` serves rmcp's Streamable HTTP in both eras, each call in the project's turn. A legacy protocol session lives in memory, bound to the login that opened it, capped, stored nowhere; it identifies no one, since every call reads the login from its own request. Seventeen tools read, raise proposals and write cards under that login and decide nothing: the daemon's one door widens for `/mcp` alone, on ADR-0057's precedent. Which routers the MCP credential opens is the proxy's sixth trust item; the daemon refuses `Authorization` on every other route.

**Why.** A writer's AI is that person's hands on a hub; stdio needs a machine holding the project.

**Cost.** AI reads wait in the turn beside the page until RM-0054. Answers carry the corpus to the client's provider. One person's AI reads every other's proposals and cards, as data. Whether claude.ai holds a per-person credential is unmeasured.
```
