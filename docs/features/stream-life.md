---
class: spec
status: draft
scope: [crates/specengine-http]
ref: RM-0075; Phase 7 step 11c-2 (08 s2); ADR-0068's last point and the orchestrator's Q7 of 2026-11-03 (`docs/features/hub-accounts.md`, its AC17), the owner's review open
adrs: []
---

# stream-life: a hub's streams end after 300 s and reconnect through the door; a local daemon's never (RM-0075)

## Why

The door re-reads a person on every request (ADR-0068), but a stream is one request: it passed the door when it opened and is never judged again. A disabled person, or one whose door session expired, keeps the board's live tail (`docs/canon/hub.md:104`) and `/mcp`'s GET stream (`docs/canon/hub-mcp.md:128`) until they close it. On a hub alone, both now end 300 s after they open. The client reconnects through the door, which refuses a disabled person, and resumes without losing an event. A local daemon's streams never end for their age.

Two ends exist today, neither the daemon's own rule. rmcp's idle end ends a session 300 s after its last request, its GET stream with it (rmcp 3.5.0 `src/transport/streamable_http_server/session/local.rs:1107-1127`). Since the door refuses a disabled person's requests, that already bounds `/mcp`, but only on rmcp's default (`local.rs:1253`). Traefik's `readTimeout` 300 s may cut a stream (`docs/canon/hub-deploy.md:83`), which is unverified and may be raised. The life makes the bound the daemon's own and the same for both streams.

Decided for this spec, each awaiting the owner's review:
- **S1 A constant.** `STREAM_LIFE`, 300 s (Q7), with no flag, config key or environment variable. A knob could lift the bound a disable relies on, as hub-mcp's S5 kept `/mcp` without a flag. Units inject a shorter life through constructors, as H17's idle unit injects its idle (`crates/specengine-http/src/mcp.rs:203`, `:742`). The integration daemon cannot take one without a production seam, so the choice of life (a hub's or none) is proved in a unit as well.
- **S2 One adapter, between whole items.** It applies to the tail's `Event`s before `Sse` and to rmcp's body frames. Each frame is one whole event or keep-alive comment (sse-stream 0.2.6 `src/body.rs:61-84`), and either body ends when its inner stream does (axum 0.8.9 `src/response/sse.rs:611-623`, sse-stream `:82`). The end sends nothing.
- **S3 `/mcp`'s GET resumes.**
  - rmcp 3.5.0 answers a legacy GET carrying `Last-Event-ID` by resuming the session's standalone channel and replaying its cache of the last 16 messages (`tower.rs:1928-1960`, `local.rs:709-751`, `channel_capacity` `:1252`).
  - A GET without the header opens a new standalone stream, which starts with the priming event `id: 0`, `retry: 3000` (`tower.rs:1961-1983`).
  - Nothing rides that channel on a hub. No hub tool notifies or asks, and `review_proposal`'s elicitation is stdio's alone (`hub-mcp.md` "Tools"). So an end loses nothing either way.
  - No event store is added. `sse_retry` stays 3 s. rmcp's 60 s `completed_cache_ttl` (`local.rs:490`, `:1255`) keeps a POST answer's stream for a resume, and is not this slice's.
- **S4 No UI change.** A stream that ends cleanly leaves `EventSource` `CONNECTING`. The browser reconnects with `Last-Event-ID`, and `subscribe` returns at once, saying no gap (`ui/src/api/http.ts:644-648`, tests `ui/src/api/http.test.ts:892`, `:912`). No `retry:` is added. The browser's own reconnection delay, a few seconds, delays events in that window, and they all arrive.
- **S5 POST answers are not capped.** Each ends with its answer, and a started call runs to its end (`hub-mcp.md` "Turn").
- **S6 The life ends a stream, never a session.** Nothing is closed or unbound. A session's idle end stays rmcp's.
- **S7 No ADR.** ADR-0068 decided the life. The canon takes it at ship ("Canon at ship").

## Description and interactions

**The events stream** (`GET /api/projects/<slug>/events`, `crates/specengine-http/src/tail.rs`, `docs/canon/serve.md` "Live tail"). On a hub, the stream answered at `tail.rs:84-86` ends `STREAM_LIFE` after its 200, at the first point between two events or comments past that time. The browser reconnects as it does after any drop: it sends the last dispatched `id`, the fence and the door judge the request anew, and `tail.rs:68-71` resumes after it (`seq > n`). Polled events not yet sent are re-read, since the cursor of the new stream is the client's id, not the old stream's. A refused reconnect meets `hub.md` "Known limits" **Session expiry**, unchanged.

**`/mcp`'s GET stream** (`mcp.rs` `Mounts::answer`, `:259-318`). A GET is judged against the binding as before. Then a 2xx answer from rmcp (`:303-306`) has its body ended `STREAM_LIFE` after the answer, between two frames. The session lives on: its next POST, GET or DELETE is answered as before. A client reconnects with `Last-Event-ID: 0` (the priming's id), or without the header. Either way it gets a new stream on the same session (S3), through the door and the binding judge. The stateless era has no GET stream: rmcp answers 405 (`tower.rs:1866-1877`), which is not 2xx and so not wrapped.

**A local daemon.** `App.hub` is `None`, so the life is `None` and the tail is byte for byte today's. It has no `/mcp`.

**The client.** The board needs no change (S4). Claude Code's reconnect after a graceful end is the client's: SG2 checks it at the gate, and the outcome lands in `hub-mcp.md` "Client check". A client that never reopens the GET stream loses nothing on a hub, since every call is a POST.

## Data

No file, table, event, JSON key, route, header, flag, `daemon.json` key, `specengine.toml` key or environment variable is added or changed. What changes is in code alone:

```rust
// src/hub.rs
/// A hub's streams' maximum life: the events tail and `/mcp`'s GET (ADR-0068, RM-0075).
pub(crate) const STREAM_LIFE: Duration = Duration::from_secs(300);
/// `stream`, ending once `life` has passed since this call, between two items,
/// sending nothing more; `None`: `stream` as given.
pub(crate) fn lived<S: Stream>(stream: S, life: Option<Duration>) -> impl Stream<Item = S::Item>;

// src/app.rs, App: set by App::new from `hub` alone
/// A stream's maximum life: `Some(STREAM_LIFE)` on a hub, `None` locally.
pub life: Option<Duration>,

// src/tail.rs: the one function `events` returns through, reading `app.life`
fn answered<S>(app: &App, events: S) -> Response; // Sse::new(lived(events, app.life)).keep_alive(..)

// src/mcp.rs: Inner holds `life: Duration`; Mounts::new(projects, hub) takes STREAM_LIFE;
// the units' constructor (today `with_idle`) takes the life as well, e.g.
// `Mounts::with_ends(projects, hub, idle: Option<Duration>, life: Duration)`;
// `#[cfg(test)] fn life(&self) -> Duration` beside `handled()`.
```

The names are suggestions; the shapes are not. `lived` might be `stream.take_until(sleep(life))`. `take_until` never drops an item it has taken from the inner stream.

**On the wire, the events stream on a hub.** A keep-alive comment may stand between any two events. After the last whole event the body simply ends.

```text
id: 41

id: 42
event: card.moved
data: {"id":"C-0001","from":"drafted","to":"checked","position":0,"as":"tomas"}

```

The browser's reconnect, about 3 s later (the proxy adds `Remote-User`):

```http
GET /api/projects/lantern-keep/events HTTP/1.1
Host: hub.example.org
Accept: text/event-stream
Last-Event-ID: 42
```

The answer opens `id: 42` and a blank line, then gives every event with `seq > 42`.

**`/mcp`'s GET stream on a hub.** It carries rmcp's priming event (`id: 0`, `retry: 3000`, empty `data`), then a keep-alive comment every 15 s, and ends 300 s after the answer. The reconnect is `GET /mcp/<slug>` with `Mcp-Session-Id`, `Mcp-Protocol-Version: 2025-11-25`, `Accept: text/event-stream` and, if the client sends it, `Last-Event-ID: 0`. It gets 200 `text/event-stream`. rmcp's configuration is unchanged (`hub-mcp.md` "Route").

## Rules and edge cases

- WHEN a hub answers `GET .../events` with 200, the stream SHALL end `STREAM_LIFE` after the answer, between two events or comments, and SHALL send nothing at its end.
- WHEN a hub's `/mcp` answers a GET with a 2xx, the body SHALL end likewise. The session SHALL stay held and bound.
- WHEN the daemon is local (`App.hub` `None`), no stream SHALL end for its age.
- WHEN a stream ends by its life, the last `id` the client has SHALL be the last whole event's. So a reconnect with `Last-Event-ID` gets every later event, once.
- The life SHALL NOT be below rmcp's idle end. rmcp's worker counts a GET, a resume included, as session activity (`local.rs:1107-1127`). A life below the idle end would let a GET-only client's reconnects keep an otherwise idle session, and its place under the caps. At 300 s each, such a session ends at its idle end, about when its stream does. The reconnect, `retry` later, then gets the 404 and opens a session anew.
- The life is measured on tokio's clock from the answer. It has no jitter, so one state gives one result. Streams opened together reconnect together, a trivial load for a team.
- The opening poll's 503 or 400, a refused fence and an unknown slug answer as today and start no life.
- A daemon restart, a poll that cannot run, a changed slug and a client gone still end a stream as today (`serve.md` "Live tail"). The life adds an end and removes none.
- Traefik's `readTimeout` (`hub-deploy.md:83`) may end a stream first. To the client the two ends are the same.
- ADR-0012: an end is no refusal and blocks nothing. ADR-0034, ADR-0059: the life is blind to who holds the stream, and the daemon learns nothing about accounts. ADR-0008: no new name touches `tests/no_domain.rs` or `tests/door.rs`.

## Acceptance criteria

Units in `src/` (`#[cfg(test)]`), the life injected at 2 s at most, on a real or paused tokio clock (paused moves rmcp's timers too). Integration in `tests/`. M marks the mutations that must turn each red.

- [ ] SL1 — `lived`, a unit of `src/hub.rs`. A stream yields three items at once, then nothing. With life L, the three come whole and in order, and the stream ends at no less than L and before L + 1 s, nothing yielded after. With `None` it is still open at 3L. M: pass the stream through; end at once; read `None` as a life.
- [ ] SL2 — the choice of life, a unit of `src/app.rs`.
  - `STREAM_LIFE` is 300 s.
  - `App::new(vec![], 0, Some(hub), None)` has `life == Some(STREAM_LIFE)`, and its mounts' `life()` is `STREAM_LIFE`.
  - `App::new(vec![], 0, None, None)` has `life == None` and no mounts.
  - M: `None` on a hub (no cap); `Some` locally (a cap on a local daemon too); 60 s.
- [ ] SL3 — the events stream, a unit of `src/tail.rs`. It calls `answered` with an `App` whose `life` is set to L, over three `Event`s with `id`, `event` and `data`, then silence.
  - The body is exactly the three whole events, with keep-alive comments allowed between them.
  - It ends at no less than L and before L + 1 s, with no byte after the last event's blank line.
  - With `life: None` it is still open at 3L.
  - M: `answered` without `lived`; `answered` capping `None` (`unwrap_or(STREAM_LIFE)`); a frame appended at the end.
- [ ] SL4 — `/mcp`'s GET stream, a unit of `src/mcp.rs` beside `an_idle_session_answers_404_and_frees_its_place`. `Mounts` has life L and rmcp's default idle; a session is opened as `open` opens one.
  - A GET naming the session (`Accept: text/event-stream`, `Mcp-Protocol-Version: 2025-11-25`) answers 200 `text/event-stream`. Its first event holds `id: 0` and `retry: 3000`.
  - Its body ends at no less than L and before L + 2 s, on a blank line.
  - Then the session's `tools/list` POST answers 200, and `sessions(SLUG)` still lists the id on both sides.
  - A GET with `Last-Event-ID: 0` answers 200 `text/event-stream` and ends again at no less than L.
  - A GET naming the id as another login still gets the 404 bytes.
  - M: no wrap on a GET; the end closing or unbinding the session; `Mounts::new`'s life used in place of the injected one.
- [ ] SL5 — a resume loses nothing on a hub, in `tests/hub.rs` through the test proxy (`Stream::open_raw`, `Proxied::fetch`, as at `:831-835`).
  - `mira` opens the tail with `Last-Event-ID: 0` and reads `C-0001`'s creation, seq c, then drops the stream as a life's end does.
  - `tomas` moves and then edits the card.
  - A tail reopened with `Last-Event-ID: c` opens `id: c`, then gives exactly the move and the edit, in order, none twice, and nothing else within 1 s.
  - M: the opening cursor ignoring `Last-Event-ID` (the highest seq); resuming at `seq >= n`.
- [ ] SL6 — unchanged elsewhere. These pass unchanged:
  - `tests/live.rs` (`ac07_last_event_id_resumes_exactly_after_it_and_never_another_projects_row` included), `tests/tail.rs` and `tests/serve.rs`;
  - `tests/hub_mcp.rs` H2 (`h2_a_legacy_session_opens_streams_lists_and_ends`, a GET stream open past 20 s);
  - `tests/door.rs` and `tests/no_domain.rs`.

  A local daemon's never-ending half is SL1-SL3's, since a 300 s life is not waited for in integration.
- [ ] SL7 — the client. No file under `ui/` changes, and `ui/src/api/http.test.ts` "leaves a dropped stream to the browser..." (`:892`) and "says a gap only when it opened the stream again itself..." (`:912`) pass.
- [ ] SL8 — form. `cargo clippy -p specengine-http --all-targets` is clean. No flag, key or environment read is added (`git diff` of `main.rs` and `specengine.toml`). `export index` and `check` give 0 errors and 0 warnings.

### At the gate, by hand (step 11b-3, findings to `hub-deploy.md` "Gate")

- [ ] SG1 — A board open 10 minutes on the VPS shows at least two reconnects with `Last-Event-ID` in the network panel. A card moved by another login during a reconnect appears without a reload.
- [ ] SG2 — Claude Code over `/mcp`: its GET stream ends at 300 s, and the client reopens it. Record whether it sends `Last-Event-ID` and whether it reopens at all. Its next tool call answers in the same session. `hub-accounts.md` G2 remains the proof for a disable.

## Out of scope

- A life for POST answers (S5).
- An event store or a resume across restarts.
- A `retry:` field on the events stream.
- A flag, key or variable for the life (S1).
- Jitter.
- Telling a person their door session ended (`hub.md` "Known limits" **Session expiry**, the gate).
- Ending streams at the moment of a disable: the daemon knows no account (ADR-0034).
- `/mcp` on a local daemon.
- Any UI change.

## Implementation

To fill in after implementation. Code comments cite the canon (`docs/canon/serve.md` "Live tail", `docs/canon/hub-mcp.md` "Sessions") or this spec's "Acceptance criteria", the headings that survive compaction.

### Files in scope

| Module | What it changes |
|---|---|
| http `src/hub.rs` | `STREAM_LIFE`, `lived` |
| http `src/app.rs` | `App.life`, set in `App::new` from `hub` |
| http `src/tail.rs` | `answered`, the handler returning through it; the module doc's life sentence |
| http `src/mcp.rs` | `Inner.life`; a 2xx GET answer's body under `lived`; the units' constructor takes the life; the module doc's **Life** bullet |
| Units (`#[cfg(test)]` in the four files) | SL1-SL4; the test-only `Mounts::life()` |
| http `tests/hub.rs` | SL5 |

The rust-developer writes `src/`. The test-engineer writes the units, the test-only accessor and SL5. No `Cargo.toml` changes, since `futures-util` and tokio's `time` are already the daemon's, and no UI file changes.

### Canon at ship

Nothing below is written before the ship commit. Free bytes are as of this spec. A page without room rewords in place, and no cap is raised.

- **`docs/canon/hub.md`** (282 B free): `:104` (**Streams.**, 93 B) leaves "Known limits". "Routes" `:57` gains "A hub's streams, the events tail and `/mcp`'s GET, end 300 s after they open (`serve.md` "Live tail", `hub-mcp.md` "Sessions"), so one outlives a disable or an expired door session by at most that." (198 B).
- **`docs/canon/serve.md`** (713 B free), "Live tail" `:95` gains "On a hub a stream ends 300 s after it opens (`STREAM_LIFE`, `hub.rs`), between two events, sending nothing more; the browser reconnects with `Last-Event-ID` through the door, which judges the person again, and loses no event. A local daemon's never ends." (255 B).
- **`docs/canon/hub-mcp.md`** (16 B free, and `:128` frees 82 B, 98 B in all, used fully):
  - `:128` leaves "Known limits".
  - "Sessions" **Life.** `:45` gains "A GET stream ends at 300 s, the session not; `Last-Event-ID` resumes it." (+73 B).
  - "Client check" `:141`: "the session and GET stream hold" becomes "the session holds, the GET stream reopens at 300 s" (+19 B).
  - "Tests" `:146`: the `src/mcp.rs` units gain "life, " (+6 B).
- **`docs/canon/hub-deploy.md`** (11 B free), "Gate" `:78` becomes "Step 11b-3, by hand: the owner's, on a clean VM, then the VPS, through H1-H17 of `docs/features/hub-deploy.md`, G1-G10 of `hub-accounts.md`, SG1-SG2 of `stream-life.md` and the claude.ai half of `hub-mcp.md` "Client check". Findings land here, with versions and dates; `--push` and real people wait for it." (+10 B).
- **Tier 1:** in the http README (192 B free), "Live tail" `:68` gains "On a hub it ends after 300 s (`serve.md` "Live tail")." (55 B).
- **`CLAUDE.md`:** the state line names step 11c-2 `stream-life`. "The hub goes on with step 11c-2 `stream-life` (RM-0075); ..." becomes "The hub goes on with 11b-2 `hub-ops` (RM-0074), waiting for the owner's dump destination; 11b-3 is the owner's gate on the VPS."
- **Plan:** RM-0075 shipped and compressed; 08's row 11c-2 shipped; this file compressed.
- **Unchanged:** `architecture.md` (no rule changes), ADR-0068, `ui/README.md`, `hub-deploy.md:83`.
