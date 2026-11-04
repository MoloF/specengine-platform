---
class: canon
tier: 2
scope: [crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-04
---

# Hub mode: a public origin and one trusted login

ADR-0058, ADR-0059, built by `hub-identity` (RM-0049). `specengine-http --public-origin` serves named people on a public URL, behind a proxy on the same host, with no account inside SpecEngine. It is a second fence, not a second daemon: the routes, the library calls, the queue and SQLite are the local daemon's (`serve.md`, `crates/specengine-http/README.md`). A spec file changes on a hub only by a confirm (`hub-confirm.md`) or a sync's merge (`hub-git.md`); who acts past the plain writes is `[team]`'s (`hub-team.md`). Code: http `hub.rs`, `app.rs` `fence`, `main.rs`; core `card.rs` `hub_login_problem`; UI `src/board/acting.ts`.

## Entry

`--public-origin https://host[:port]` is the whole switch. Present: a hub. Absent: a local daemon, `App.hub` `None`, the local fence byte for byte, `Remote-User` never read. No `[hub]` table: `specengine.toml` reaches every developer through git, and the mode belongs to the process, which serves several `--root`. `spec serve` has no such flag and passes only `--root` and `--port` (`serve.md` "Launcher"), so a launched daemon is never a hub.

The bind does not change: `127.0.0.1`, `--host` and `--bind` exit 2, `--port` as ever. The proxy reaches loopback through a shared network namespace (`hub-deploy.md`). `daemon.json` keeps its five keys and loopback `url`.

Parsed by hand before anything is created; a refusal exits 2, `specengine-http: --public-origin <given>: <reason>`. The scheme is exactly `https://`; the host lower-cased, labels of `[a-z0-9-]` joined by `.`; an optional canonical port, 1 to 65535, no leading zero or `+`, `:443` dropped. Refused: a path (even `/`), query, fragment or userinfo; a loopback host (`localhost`, `*.localhost`, a loopback or unspecified IPv4). A numeric host (a last label of digits or `0x...`) must be a dotted quad. Both checks are a guard against an operator's mistake, not a guarantee: they read the text given, and `127.0.0.1.nip.io` cannot be caught.

A start prints `public <origin>` after the `serving` lines, before `listening`.

## Fence

In `fence()`, before routing, in this order. A failure is the local fence's answer: 403, the error body, `refused: ...`, nothing read, no `Allow`, `no-store`, no `Access-Control-*`. Any of these headers sent twice is refused. `X-Forwarded-*` is never read.

1. `Host` byte for byte the origin's authority. A loopback `Host` is refused.
2. `Origin` absent or byte for byte the origin.
3. `Sec-Fetch-Site` absent, `same-origin` or `none`, but for the landing.
4. `Remote-User`: "Identity".
5. The method and path, then a confirm route's query and `Origin`: "Routes"; `/mcp` and `Authorization`: `hub-mcp.md`.

```text
refused: the Host header must be <authority>: this hub serves its public origin only
refused: the Origin header, when sent, must be <origin>
refused: Sec-Fetch-Site, when sent, must be `same-origin` or `none`
refused: the <header> header is sent more than once
refused: this hub needs one Remote-User header from its proxy
refused: Remote-User: <why>
refused: this hub does not serve this route
```

**The landing.** One exception, for the hub alone: `GET /` with `Sec-Fetch-Mode: navigate` and `Sec-Fetch-Dest: document` passes step 3 whatever `Sec-Fetch-Site` says. The login door's page, on a sibling subdomain, navigates back as `same-site`, a chat link arrives as `cross-site`, and refusing them lands a person on a JSON 403. The page is static bytes and `frame-ancestors 'none'` stands (`serve-ui.md`). Every other route, `/assets/<file>` included, refuses both, and `/` takes the rest of the fence.

## Identity

`Remote-User`, one constant in `hub.rs`, no flag: `scripts/deploy/` ships at the same version, so a changed login door changes both in one commit, and a flag could name a header the proxy does not scrub. `Remote-Name`, `Remote-Email`, `Remote-Groups` are never read, so the deployment guards one header.

Judged on every request, `/` and assets too. None or a bad one: 403, never a 401, whose challenge the daemon does not own. An empty value counts as none; two values give `refused: the Remote-User header is sent more than once`, from `single()`. `hub.rs` keeps those steps and UTF-8; `hub_login_problem` (core `card.rs`, re-exported) refuses the rest: a login `.trim()` would change; `card_person_problem` (`cards.md` "Who acts"); a comma (a proxy folding a client's copy gives `eve, mira`); a Unicode format character (Cf) anywhere; `local` and `agent` in any ASCII case, the record's words for no person, and `-`, its word for clearing a name. hyper answers 400 to a control byte but a tab, before the fence.

The login door's duty, not judged here: Latin look-alikes, NFC against NFD, and invisible marks outside Cf (U+034F, variation selectors, U+3164). `hub-admin` refuses them at account creation by an ASCII grammar (`hub-deploy.md#accounts`).

The login is the person: recorded verbatim, compared exactly, never a display name, so a renamed login is another person. The handlers of the five writes and the confirm class set the `actor` of their `Card*Body` to it before the library call; the body's `as` must be null or the login, byte for byte, else 403, ``refused: `as` names <name>; this request's login is <login>``, nothing stored; a name off the person grammar is `from_json`'s 400 first. `spec card --as` keeps its meaning on a terminal: a shell on the host is the operator's. `by` compares with the login, so a flow's `by` stops a person for the first time, if it lists logins: `by = ["lead"]` refuses everyone with a 409 (`cards.md` "Rules"). A confirm's `Decided-by` and a departing accept's `Author` take the login: `hub-confirm.md`.

## Routes

A read needs the origin and one identity. Any identity reads every served project. With no `ui/dist`, `/` past the fence is the not-built 503, as locally. `GET /api/identity`, both modes, no turn: `{"login":"mira"}`, or `{"login":null}` on a local daemon, the UI's one way to learn the mode. `GET .../team`, a hub's alone: `hub-team.md`; `.../sync`: `hub-git.md`. A hub's streams, the events tail and `/mcp`'s GET, end 300 s after they open (`serve.md` "Live tail", `hub-mcp.md` "Sessions"), so one outlives a disable or an expired door session by at most that.

Writes are default deny, in the fence, so a non-GET route added later is refused until listed ("Known limits" for a GET). On a hub any method but GET is 403, a read route's 405 too, unless `hub.rs` `class` lists it, by exact raw segments: the five plain writes, POST on `.../cards` or `.../cards/:id/{move,edit,assign,cancel}`, and the team and confirm classes of `hub-team.md` "Classes". A listed write then keeps the door of `board-ui.md`: `Sec-Fetch-Site: same-origin`, JSON, the flow's `by`.

`.../proposals/:id/confirm`, a hub's alone, is the one approve route: `hub-confirm.md`. A move cannot reach `accept.to` (`cards.md` "Config"), so accept has no back door. ADR-0057 stands locally.

Identity is judged in the fence, so a refused request never queues for a turn. MCP: `/mcp/<slug>`, a hub's alone, under the login (`hub-mcp.md`).

<a id="trust"></a>
## Trust

No authentication inside SpecEngine (ADR-0034): the login door owns accounts, sessions, passkeys and recovery, acted on from the host's shell (`hub-deploy.md#accounts`); the daemon holds no account, token, key or login session (git's: `hub-git.md`; an MCP session: `hub-mcp.md`) and trusts one header (ADR-0059). A refused login is a door, not a discrepancy: no status, check or gate reads identity, open items stay open, the hook never asks the hub (ADR-0012, ADR-0039). A hub adds only a floor file (`queue-backup.md`): the login lands in queue fields and a confirm's commit.

**The daemon can defend**, at its fence:

- Other sites' pages and sibling subdomains: `Host`, `Origin`, `Sec-Fetch-Site`, `same-origin` on a write, JSON only, no CORS. The login door's cookie is the first ambient credential here, so none of this leans on its `SameSite`.
- A missing, doubled, folded or malformed `Remote-User`, and the words `local` and `agent`.
- A body's `as` other than the login.
- Every route off the list, whoever asks; a login outside `[team]` reaches no stage, accept, clear or confirm.

**The daemon cannot defend:**

- **A forged header.** It cannot tell the proxy's header from any process reaching its socket: a raw client sending `Remote-User: anyone` gets a 200 and `anyone` recorded, a `[team]` login's name accepts and confirms, and `hub_trusts_the_header_on_its_socket` pins that. A shared secret would be the token ADR-0034 forbids.
- **What a login may do.** Any login reads everything served and makes the five writes. `by` alone stops a person. `GET /api/projects` shows the absolute `root` to every login: accepted; in a container it is the mount path.
- **A forgotten proxy.** A forgotten flag fails closed: a local daemon refuses the public `Host`. A forgotten proxy fails open to whatever reaches the socket.

**The deployment guarantees** (`hub-deploy.md#door`), a test pinning each file:

1. The daemon's port is never published, and nothing but the proxy shares its network namespace.
2. Only the proxy sets `Remote-User`: forward-auth lists it in `authResponseHeaders` on every router reaching the daemon but item 6's, where the MCP credential's check sets it; no router or `bypass` rule reaches the daemon without one of the two.
3. `passHostHeader` stays on, so the daemon sees the public `Host`.
4. No agent, and nothing else untrusted, runs on the host.
5. Every router reaching the daemon steps up on the confirm class's four routes, by this rule alone:

   ```text
   ^/api/projects/[^/]+/(proposals/[^/]+/confirm|cards/[^/]+/(accept|clear)|trace/clear)$
   ```

6. The MCP credential opens only `/mcp` routers: `hub-mcp.md#trust`, its rule read by a test.

## Screen

With a resolved login, "Acting as" is read-only and shows it, writes send it as `as`, and `specengine.board.as` is not written. `null`, loading or a failed read: the box as `board-ui.md` "The acting name" says; the stored name stands until a resolved login replaces it, so a local write never records `local` by accident. `useHubLogin` beside `useActing` (`acting.ts`) gives the box its `readOnly`. The dialogs' names read alike, and controls an act's `allowed` omits stay on the page, idle: `hub-team.md` "Screen".

## Known limits

- **Session expiry.** A `fetch` following the login door's redirect fails as status 0, "no response from the daemon", and `EventSource` backs off. The gate (step 11b-3) checks what the door answers to a JSON request.
- **A stalled reader.** A client that stops reading keeps its socket past a stream's life, not its events: the end waits for it to read again.
- **Health.** A hub answers a probe of `127.0.0.1` with a 403 (RM-0074).
- **Scale.** Reads trust the index (`spec-read-freshness.md`); a team still waits in a project's turn.
- **A raw `#`.** `http::Uri` drops a fragment before the fence, so `.../accept#x` on a raw socket reaches a handler: the proxy encodes it, `%23` (`hub-deploy.md`).
- **GET has no list.** Default deny covers every method but GET; `tests/hub.rs` pins the GET route set (`the_get_routes_are_pinned`), so a new one fails until decided.
- **Before the login resolves.** While the identity loads or after a failed read the box is as today, so a typed name can reach the key, and a stored name sent then gets the daemon's 403.
- **One login per page.** The login is kept for the page, so changing login in another tab gives the `as` 403 until a reload.
- **The mock.** A `?scenario=` query on the landing opens the mock on the hub's origin, with its banner; past the page nothing reaches the daemon.
- **Headers.** `nosniff`, HSTS and COOP are set at the proxy (`hub-deploy.md`).

## Tests

http `tests/hub.rs`, whose test proxy sends what Traefik sends: public `Host`, `Origin`, `Sec-Fetch-*`, one `Remote-User`. It proves the daemon's half; the real proxy's half is `hub-deploy.md`'s. Units in `src/hub.rs`; `door.rs` and `no_domain.rs` scan `hub.rs` as they do every `src` file; `daemon_keys.rs` (`Identity`). UI: `src/board/hubIdentity.test.tsx`, beside `src/api/`'s, jsdom only.
