---
class: canon
tier: 2
scope: [crates/specengine-http, scripts/deploy]
owner: owner
reviewed: 2026-11-15
---

# Daemon: slow requests, long turns and the proxy's access log

Built by `slow-requests` (RM-0118) and `hub-turn-fairness` (RM-0119), no ADR. The evidence a hang leaves: which request waited, which holder of a project's turn (`serve.md` "Concurrency") made it wait, how long, and whether a read walked. Lines on stderr alone: nothing stored, served or counted, no route. Code: http `slow.rs` (the request line: `watch`, the outermost layer; `routed`, each route's template), `turn.rs` (the turn taken and measured, the line per login, the watch), `app.rs` (`.noted` templates), `mcp.rs` (the tool), `sync.rs`. Criteria: `docs/features/slow-requests.md`, `docs/features/hub-turn-fairness.md`.

## Threshold

`specengine-http --slow-ms N`: 0 to 3 600 000 (`SLOW_MS_MAX`), 2 000 by default (`SLOW_MS`); 0 writes a line for every request and every hold. A threshold is passed when the time measured is at least `N` ms. Anything else is clap's refusal: exit 2, one stderr line, before listening (`serve.md` "Lines"). `spec serve` and the hub's entrypoint pass none, so both run at 2 000.

## Lines

One `writeln!` each on a locked stderr, keys in this order, an absent value `-`:

```text
specengine-http: slow request at=<T> method=<M> route=<R> project=<P> id=<I> tool=<N> status=<S> ms=<n> wait_ms=<n|-> run_ms=<n|-> index=<walked|trusted|-> class=<page|mcp> line_ms=<n|-> ahead=<n|->
specengine-http: slow turn at=<T> project=<P> by=<R|sync> id=<I> tool=<N> state=held|released held_ms=<n> class=<page|mcp|sync>
```

```text
specengine-http: slow turn at=2026-11-14T14:01:07.412Z project=dorm by=/mcp/{p} id=- tool=propose_change state=held held_ms=2000 class=mcp
specengine-http: slow turn at=2026-11-14T14:01:07.412Z project=dorm by=/mcp/{p} id=- tool=propose_change state=released held_ms=4385 class=mcp
specengine-http: slow request at=2026-11-14T14:01:07.610Z method=GET route=/api/projects/{p}/inbox project=dorm id=- tool=- status=200 ms=4300 wait_ms=4190 run_ms=110 index=trusted class=page line_ms=- ahead=1
```

- **A request line**: one per request whose whole time passes the threshold, written when the handler's answer returns. An answer whose body streams (an MCP call's SSE answer, which carries the call's result) writes it at the body's end; a HEAD's, a 1xx, 204 or 304 at the return. A client gone first: `status=gone`, `run_ms=-` when it never ran. None for the live tail (`GET …/events`) or `/mcp`'s GET stream, open up to 300 s by design; the tail's polls take no turn.
- **A turn's lines**: a hold reaching the threshold writes `held` at once, while still held (at 0, at acquisition, before the call runs). A watcher on its own std thread, outside the async runtime, keeps the timers, so a stalled runtime still leaves the line. That hold's end writes `released`, its whole length, before the turn passes on. Every holder: a request, a confirm's too (`answer.rs` `turned`), a hub's `/mcp` call or resource read (`hub-mcp.md` "Turn"), a `--push` sync's merge (`by=sync`, one pair per project of its repository, in `--root` order), a hub's delegated apply (`by=<login> id=<PR> tool=auto_apply class=mcp`, `hub-auto.md` "Lines"). A call once started is never cancelled, so a `gone` request's hold can outlast its line; its `released` gives the real length.

| Key | Value |
|---|---|
| `at` | UTC `YYYY-MM-DDTHH:MM:SS.mmmZ`: a request's arrival; a hold's start |
| `method` | as sent, if 1 to 16 ASCII uppercase letters, else `?` |
| `route` | the route template (`/api/projects/{p}/cards/{id}/accept`, `/mcp/{p}`, `/assets/{file}`, `/`), never the raw path; `-` unrouted |
| `project`, `id` | the `{p}` and `{id}` segments; on a turn line, `project` is the served slug |
| `tool` | on `/mcp/{p}`, a `tools/call`'s name when it is one of the hub's tools (a `probes` build: or `HUB_PROBES`), else `?`; `resources/read`; never an argument |
| `status` | the answer's code; `gone`: the client left first |
| `ms` | from the outermost layer's first sight, before the fence, to the answer or the drop |
| `wait_ms` | from arrival to the turn acquired, the login's line included, or to the drop; `-` with no turn |
| `run_ms` | the turn held for it, to the answer or the drop; `-` when it never ran |
| `index` | `walked`: the project's `IndexTrust` walked while it held the turn; `trusted`: a hub's that did not; `-` on a local daemon or with no turn |
| `class` | `page` (every route but `/mcp/{p}`), `mcp`; a turn line also `sync` |
| `line_ms` | the wait in its login's line (`hub-mcp.md` "Turn"); `-` with none |
| `ahead` | the turn's holders and waiters when it began waiting for the turn; `-` with no turn |
| `by` | the holder: its route template, `sync`, or a delegated apply's login |
| `held_ms` | `held`: the threshold (0 at 0); `released`: the hold's whole length |

**Hygiene.** No login but a delegated apply's `by`, which no request carries (else the door's log names who; `at`, to the millisecond, joins the logs), body, header, query string, raw path, cookie or MCP argument. `project`, `id` and that `by` print as 1 to 64 bytes of `[A-Za-z0-9._-]`, `tool` of `[A-Za-z0-9._/-]`, else `?`; nothing is percent-decoded, quoted or holds a space or control. **Never a change**: no answer, status, header or order changes; a failed write is ignored; `ms` is measured before the write; lines of different requests interleave line by line, never within one.

## Traefik's access log

Fourteen flags in the `traefik` service's `command:` (`hub-deploy.md` "Host"; `tests/deploy.rs` SR11): `--accesslog=true`, `format=json` on stdout, `filters.minduration=2s`; `fields.defaultmode=drop` but `StartUTC`, `Duration`, `Overhead`, `OriginDuration`, `RouterName`, `RequestMethod`, `RequestHost`, `OriginStatus`, `DownstreamStatus` kept; `fields.headers.defaultmode=drop`. No `RequestPath` or `RequestLine` (a door's query can carry a one-time token; the daemon's line names the route), no client address or username. It shows what the daemon cannot: time at the proxy and in the door's forward-auth (`Overhead`), and requests that never reached the daemon.

<a id="reading"></a>
## Reading them

`docker compose logs -t --since 2h specengine | grep ' slow '`, then `docker compose logs -t --since 2h traefik`.
- A `held` line with a long `held_ms` names the holder; the request lines after it with a large `wait_ms` are what it stalled; `ahead` counts who stood before each, `line_ms` an agent's wait behind its own calls.
- `index=walked` on a hub: a read that ran the reconcile walk (`spec-read-freshness.md` "A hub's trusted index"). `status=gone`: a person who left. `by=sync`: the `--push` merge.
- **A stream, not a hang.** The page's tail (`GET …/events`, router `hub@file`) and an agent's `/mcp` GET stream (`hub-mcp@file`) stay open up to 300 s: Traefik logs each with a `Duration` near 300 s, and the daemon writes no `slow request` line for it. A long Traefik line with a daemon request line at the same moment is a slow request; one with neither a daemon line nor a 300 s stream's shape spent its time at the proxy or the door (`Overhead`).

## Known limits

- Route templates are written twice, in each route and its `.noted(…)`, since axum's `matched-path` feature is off in the pinned build; turning it on, an owner's option, would let one source serve both.
- A hub cannot lower `--slow-ms` from the compose: the entrypoint (`hub-deploy.md` "Entrypoint") rebuilds its arguments and reads no variable for it, so a diagnosis session at a lower threshold needs an entrypoint change.
- No rotation (Docker's log driver keeps the logs), metrics, sampling, request ID or per-person view. The CLI and the stdio MCP server write none.
- Traefik's fields are from its documentation: that a request of 2 s or more logs `Duration`, `RouterName` and `DownstreamStatus` is the owner's check on the host.

## Tests

http `tests/slow.rs` (SR1-SR10, the spawned binary's stderr), `tests/common/slow.rs` (the lines' grammar), `tests/hub_fairness.rs` (TF1-TF6, TF8, TF9; a `probes` build, `#[ignore]`d: `--features specengine-mcp/probes --run-ignored ignored-only`), `src/lines_tests.rs` (TF7), `tests/deploy.rs` SR11.
