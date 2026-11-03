---
class: spec
status: shipped
shipped: 2026-11-03
scope: [crates/specengine-http]
ref: RM-0075; Phase 7 step 11c-2 (08 s2); ADR-0068's last point and the orchestrator's Q7 of 2026-11-03 (`docs/features/hub-accounts.md`, its AC17), the owner's review open; one iteration
adrs: []
---

# stream-life: a hub's streams end after 300 s and reconnect through the door; a local daemon's never (RM-0075)

## Why

The door re-reads a person on every request (ADR-0068), but a stream is one request: it passed the door when it opened and was never judged again. A disabled person, or one whose door session expired, kept the board's events tail and `/mcp`'s GET stream until they closed it. Two ends existed, neither the daemon's own rule. rmcp's idle end bounded `/mcp`, but only on rmcp's default. Traefik's `readTimeout` may cut a stream, which is unverified and may be raised. On a hub alone, both streams now end 300 s after they open. The client reconnects through the door and loses no event. A local daemon's streams never end for their age.

Decided by this spec, each awaiting the owner's review:
- S1, a constant with no knob;
- S2, one adapter ending between whole items;
- S3, `/mcp`'s GET stream resumes, and nothing rides it on a hub;
- S4, no UI change;
- S5, POST answers are not capped;
- S6, the life ends a stream, never a session;
- S7, no ADR.

## Description and interactions

Canon: `docs/canon/serve.md` "Live tail" (the tail), `docs/canon/hub-mcp.md` "Sessions" (**Life**, `/mcp`'s GET), `docs/canon/hub.md` "Routes" and "Known limits".

## Data

No file, table, event, JSON key, route, header, flag, `daemon.json` key, `specengine.toml` key or environment variable is added. The code alone changes:
- `STREAM_LIFE`, 300 s;
- `lived(stream, Option<Duration>)`, which ends `stream` between two items once the life has passed and sends nothing more, `None` leaving it as given;
- `App.life`, `Some(STREAM_LIFE)` on a hub and `None` locally.

On the wire, the tail's body simply ends after a whole event. The browser's reconnect sends `Last-Event-ID: <last seq>` and gets `seq >` it. `/mcp`'s GET carries rmcp's priming event (`id: 0`, `retry: 3000`) and keep-alives, then ends. A GET with `Last-Event-ID: 0`, or without it, gets 200 on the same session. rmcp's configuration is unchanged.

## Rules and edge cases

`serve.md` "Live tail", "Known limits"; `hub-mcp.md` "Sessions" (the life equals rmcp's idle end, and a GET resuming a POST's stream is capped like any GET); `hub.md` "Known limits" (**A stalled reader**).

## Acceptance criteria

M marks the mutations that turn each red. Units are `#[cfg(test)]` in `src/`, with the life injected at 2 s at most on a real clock.

- [x] SL1 — `lived`, a unit of `src/hub.rs`. With life L, three items come whole and in order, then the stream ends no sooner than L and before L + 1 s. With `None` it is open at 3L. M: pass-through; end at once; `None` read as a life.
- [x] SL2 — the choice of life, a unit of `src/app.rs`. `STREAM_LIFE` is 300 s. `App::new` on a hub gives `Some(STREAM_LIFE)`, and its mounts' `life()` is the same. Locally it gives `None` and no mounts. M: `None` on a hub; `Some` locally; 60 s.
- [x] SL3 — the tail, a unit of `src/tail.rs`. `answered` with life L gives exactly three whole events, then ends no sooner than L and before L + 1 s, with no byte after the last event. With `None` it is open at 3L. M: no `lived`; a frame appended at the end. The mutation capping `None` at `STREAM_LIFE` survives (see "Implementation").
- [x] SL4 — `/mcp`'s GET stream, a unit of `src/mcp.rs` beside H17's idle unit.
  - The GET answers 200 `text/event-stream` with `id: 0` and `retry: 3000`, and ends no sooner than L and before L + 2 s.
  - Afterwards the session still answers `tools/list` and stays held and bound.
  - A GET with `Last-Event-ID: 0` gets 200 and ends again.
  - Another login's GET gets the 404 bytes.
  - M: no wrap on a GET; the end closing or unbinding the session; `Mounts::new`'s life used in place of the injected one.
- [x] SL5 — a resume loses nothing on a hub, `tests/hub.rs` through the test proxy. A tail dropped after `C-0001`'s creation (seq c) and reopened with `Last-Event-ID: c` gives exactly the move and the edit made meanwhile, in order, none twice. M: the cursor ignoring `Last-Event-ID`; `seq >= n`.
- [x] SL6 — unchanged elsewhere: `tests/live.rs`, `tests/tail.rs`, `tests/serve.rs`, `tests/hub_mcp.rs` H2, `tests/door.rs` and `tests/no_domain.rs` pass.
- [x] SL7 — the client: no file under `ui/` changes. A clean end leaves `EventSource` reconnecting by itself, and `ui/src/api/http.test.ts` `:892` and `:912` stand.
- [x] SL8 — form: clippy is clean, no flag, key or environment read is added, and `export index` and `check` give 0 errors and 0 warnings.

At the gate, by hand (step 11b-3, findings to `hub-deploy.md` "Gate"):
- [ ] SG1 — A board open 10 minutes shows at least two reconnects with `Last-Event-ID`. A card moved meanwhile by another login appears without a reload.
- [ ] SG2 — Claude Code's GET stream ends at 300 s and is reopened. Record whether the client sends `Last-Event-ID` and whether it reopens at all. Its next tool call answers in the same session.

## Out of scope

- A life for POST answers.
- An event store.
- A `retry:` field on the tail.
- A knob for the life.
- Jitter.
- Ending streams at the moment of a disable (ADR-0034).
- `/mcp` on a local daemon.
- UI changes.

## Implementation

One iteration, ACCEPT with nits only. The nits are now in the canon: the life equal to rmcp's idle end, the stalled reader, a GET resuming a POST's stream, and S3's premise. Tests: 4895 of 4895 in the workspace; clippy and `spec check` clean; `ui/` unchanged. Mutations: 17 of 18 killed.

The survivor is SL3's `answered` capping `None` with `unwrap_or(STREAM_LIFE)`. No test waits 300 s, and tokio's paused clock needs its `test-util` feature, which nothing in the graph enables. Adding a feature is a stack decision, so none was added. SL2 pins a local `None`, and SL1 pins `lived(.., None)` as timer-free.

Deviations, accepted:
- `Mounts::with_idle` is kept and delegates to `with_ends(projects, hub, idle, life)`;
- `answered<S, E>` is generic over the stream's error;
- the GET wrap is the helper `lived_body`.

| Module | What it does |
|---|---|
| http `src/hub.rs` | `STREAM_LIFE` (`:55`); `lived` (`:62`), `take_until` on tokio's `sleep`; SL1 |
| http `src/app.rs` | `App.life` (`:99`), set from `hub` alone in `App::new` (`:118`); SL2 |
| http `src/tail.rs` | `answered` (`:99`), the one return of `events`, `Sse::new(lived(events, app.life))`; SL3 |
| http `src/mcp.rs` | `Inner.life` (`:182`); `with_ends` (`:215`); a 2xx GET under `lived_body` (`:338`, `:348`), nothing closed or unbound; the test-only `Mounts::life()`; SL4 |
| http `tests/hub.rs` | SL5 (`:2030`) |
