---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-http]
owner: owner
reviewed: 2026-11-09
---

# Daemon: spec serve

`spec serve` launches `specengine-http` (`crates/specengine-http/README.md`), one daemon per data directory, which publishes where it listens; as built by `spec-serve`; it serves the web UI compiled into it (`docs/canon/serve-ui.md`). The gate never consults it (ADR-0039, `docs/canon/architecture.md#control`); agents keep the stdio MCP server, a `spec mcp` bridge comes on a trigger (05 s1 (6)). Code: CLI `serve.rs` (`serve`, `ServeRequest`, `Serve::{Running, Start}`, `DaemonFile`, `DaemonProject`, `read_daemon_file`, `unpublished_daemon`, `daemon_text`; `DAEMON_BIN`, `DAEMON_LOCK`, `DAEMON_FILE`, `DAEMON_PORT` 7777, `DAEMON_POLL` 50 ms, `DAEMON_PUBLISH_WAIT` 2 s), `main.rs` `run_serve`, `exec`; http `daemon.rs` (`lock`, `publish`), `start.rs` `served`.

## Launcher

`spec serve [DIR...] [--port N]`, in order:

1. Not Unix: exit 2, checked first, before `--json`. Then `--json`, `--config`, the global `--root` beside a DIR: exit 2 ("Lines").
2. **Roots.** No DIR: the project any command finds (global `--root`, else the walk up from the cwd), its errors the CLI's lines, the missing-config one without the `--config` hint. DIRs: each as `--root DIR` takes it (no walk), in order, a repeated canonical root once; a DIR's error is `spec serve: DIR <given>: <the CLI's line>`, its `spec:` prefix kept (as `specengine-http: --root <given>: spec: ...`). A slug each.
3. **Guard**: the data directory (`crates/specengine-cli/README.md` "Database") inside any root: exit 2, nothing created (`checked_data_dir`).
4. **Lookup**: `specengine-http` beside `std::env::current_exe()`, else the first `PATH` entry holding it as an executable regular file.
5. **Probe**, creating nothing: no data directory or `daemon.lock`: none. Else the lock file opened read-only, `try_lock_shared`, released at once: acquired, none (a `daemon.json` there stale); held, `daemon.json` read, re-read every 50 ms within 2 s while missing or unreadable, the shared lock tried again before each read (a holder that ended lets the start go on).
6. **Exec** (`CommandExt::exec`, `execvp`): `<found path> --root <canonical root>... --port <N>`, `N` given or 7777; environment and cwd unchanged; from then on the output and exits are `specengine-http`'s.

## Start

`specengine-http --root DIR... [--port N]` (`start::served`, `main.rs`): each root canonicalised, its config read, a slug required; the data directory guarded against every root (nothing created), then created (`prepared_data_dir`); `daemon.lock` opened (created if absent, never written or deleted), `File::try_lock`ed (exclusive `flock`), `WouldBlock` retried every 50 ms up to 500 ms; a hub (`--public-origin`) then stores its `proposal-floor` once (CLI `store_hub_floor`; an invalid one: exit 2, `queue-backup.md` "ID floor"); then the runtime, the bind, `daemon.json` written (`daemon.json.tmp`, renamed), the start lines. A refusal after the lock (the port taken, `daemon.json` unwritable) leaves the data directory and an empty `daemon.lock`. Any end, `SIGKILL` included, frees the lock; `daemon.json` is never removed.

## Files

Beside `<slug>.db`, never under a root (ADR-0003). `daemon.lock`: empty, its `flock` the only liveness signal. `daemon.json`, trusted only while the lock is held: pretty JSON (two-space indent) plus LF, exactly these keys in this order:

```json
{
  "pid": 41873,
  "port": 7777,
  "url": "http://127.0.0.1:7777",
  "projects": [
    {
      "slug": "lantern-keep",
      "root": "/srv/spec-a"
    }
  ],
  "started": "2026-10-08T21:04:05Z"
}
```

`pid` `std::process::id()`; `port` the bound one (`--port 0`: the chosen); `url` the `listening` line's; `projects` in `--root` order, `root` canonical (as `/api/projects`); `started` `utc_now()`. No token, key, secret, version or build (ADR-0034). Readers take the five keys and ignore others; one missing or mistyped: unreadable.

`proposal-floor`, a hub's alone: its `PR-` floor and its spec-ID side, written once at its start (`queue-backup.md` "ID floor").

## Lines

stdout. A start (`specengine-http`): `serving <slug> <root>` per root, a hub's `public <origin>` and a `--push` hub's `push every <N> min` (`hub-git.md`), then `listening http://127.0.0.1:<port>`. Already running (`spec`, exit 0): `already running: pid <pid>`, `serving <slug> <root>` per requested root, `listening <url>`.

Exit 2, one stderr line; `spec serve: ` or `specengine-http: ` first:

| Line | When |
|---|---|
| `not supported on this platform` | `spec`, not Unix |
| `--json is not supported: the daemon prints lines` | `spec --json` |
| `--config is not supported: each root's specengine.toml is read` | `spec --config` |
| `give the roots as DIR... or as --root, not both` | both |
| `DIR <given>: <CLI line>` | a DIR missing, without config or slug, its config broken, the guard |
| `--root <given>: <CLI line>`; a slug twice, naming both roots | http, likewise |
| `specengine-http not found beside <spec path> or on PATH; install it: cargo install --locked --path <clone>/crates/specengine-http` | no binary |
| `cannot open <lock>: <error>`; `cannot lock <lock>: <error>` | either side |
| `a daemon (pid <pid>, <url>) serves <slug> <root>[, ...]; stop it, then run: spec serve <root>...[ --port <port>]` | `spec`, served elsewhere |
| `a daemon holds <lock>, but <daemon.json> is unreadable: <reason>` | either, unpublished |
| `cannot run <path>: <error>` | `spec`, exec failed |
| `a daemon already runs under <data dir>: pid <pid>, <url>, serving <slug> <root>[, ...]; stop it first` | http, lock held |
| `cannot write <daemon.json>: <error>` | http, after the bind |
| `cannot listen on 127.0.0.1:<port>: <error>` | http, the port taken |
| `--push needs --public-origin: only a hub pushes`; `--push-every needs --push`; `--push-every takes 1 to 1440 minutes, not <n>`, `--push-deadline` alike | http, `hub-git.md` "Flags" |

## Rules

- Every requested (slug, canonical root) pair among `projects`: the "already running" lines, exit 0, no exec, whatever `--port`. Else (another root, or one now under another slug): "served elsewhere", every served root in order, then the new ones, ` --port <port>` when the daemon's is not 7777 (a `--port 0` daemon's ephemeral one too). No registration into a live daemon: a loopback route would let any local account widen what is served (ADR-0034).
- Every text read from `daemon.json` (url, slug, root, an unpublished reason) prints through `daemon_text`: controls and bidirectional marks escaped, one line. The restart command's roots: escaped, then shell-quoted (outside `A-Za-z0-9/._-`: single quotes, `'` as `'\''`).
- `spec` never creates either file. Stale: `daemon.json` with the lock free (crash, `kill -9`, a reused pid); the next start replaces it; no reader checks the pid.
- Two starts at once: one locks; the other exits 2 after 500 ms, naming the winner if published, else the unpublished line.
- One daemon per data directory: a scratch `HOME` or `XDG_DATA_HOME` gets its own; two under different ones race for 7777, no fallback (`--port 0` is explicit; the UI dev proxy targets 7777).
- Foreground only: SIGINT, SIGTERM end it, every write one SQLite transaction; no detach, idle exit or log file.

<a id="trust"></a>
## Trust

No authentication inside SpecEngine (ADR-0034, `docs/canon/architecture.md#distribution`). The daemon's `127.0.0.1` bind, its Host, Origin and Sec-Fetch-Site fence (`crates/specengine-http/README.md`) and absent CORS stop other sites' pages; they identify no one. A link from another site still opens `/`: static bytes in the person's tab, never framed, whose requests take the whole fence (`serve-ui.md` "Rules"). Threat model: any local process of any account can read everything served, stage a decision, and move, cancel, accept or clear a card under any name (loopback is not per-user; ADR-0057, `board-ui.md`), a departing accept storing one discrepancy; none confirms a decision from an ordinary shell; one faking a terminal (`script`) can, as with `spec approve` today. Hosting or a shared machine needs infrastructure first, per service (a WebAuthn proxy in front), never in-app; a hub's answer to all of this is `hub.md#trust`.

## Concurrency

Requests of one project run one at a time on the blocking pool (a read refreshes its index; `health` both, for rows 4-6 (ADR-0051), its first call building the whole code index inside the turn, and row 6 spawning verify's read-only git there (`rev-parse`, the unmerged check, `hash-object` without `-w`: `health-drift.md` "Route"), no timeout bounding it, a call once started never cancelled (`health-code.md` "Route"); `check`, refreshing none, too): each awaits its project's turn (a `tokio` mutex) in its own task, so one whose client left first is dropped unrun; the turn goes with the call to the pool. Projects run in parallel; `/api/projects` and the tail's polls take no turn. A `--push` hub's sync takes the turns of its repository's projects, in `--root` order, for its merge alone; its fetch and push take none (`hub-git.md`). A hub's `/mcp` calls and resource reads take their project's turn, the wait raced with the request's cancellation; its handshakes and lists take none (`hub-mcp.md`). With the CLI and the plugin's MCP server: one more direct writer of the index and the queue (WAL, `busy_timeout`; `proposal-queue.md` "Store"); one daemon and one queue per data directory.

## Live tail

`GET .../events`, the stream: `crates/specengine-http/README.md` "Live tail". Types: all stored (`proposal-queue.md` "States and events", `tasks.md` "Store", `gate.md` "Event", `spec-verify-tests.md`); a row with a NULL type or payload or CR, LF in its type skipped. Polls at most 250 ms apart, at once after a full page (`EVENTS_PAGE_MAX`, 512 rows read, skipped ones counted), each reads the config (a changed slug ends the stream), then one short read transaction on the stream's own connection (CLI `EventsTail`), kept between polls, reopened when the file is replaced, dropped when gone; no database created. A poll that cannot run ends the stream; a client gone ends it and the connection. On a hub a stream ends 300 s after it opens (`STREAM_LIFE`, `hub.rs`), between two events, sending nothing more; the browser reconnects with `Last-Event-ID` through the door, which judges the person again, and loses no event. A local daemon's never ends.

## Known limits

- Between a winner's lock and its publish, an earlier daemon's stale `daemon.json` reads as published: a probe answers `already running: pid <dead pid>`, exit 0. A fix conflicts with "never removed" and "no pid check".
- A root holding a control character is named escaped in the restart command: not runnable as printed.
- `execvp` runs a found file that is no binary through `/bin/sh` (ENOEXEC; 127 when it fails).
- Deleting the data directory under a live daemon lets a second start lock a new file.
- Version skew: crates 0.1.0, no build id; an old daemon after `cargo install` answers `SchemaTooNew` 503s: restart it.
- `daemon.json.tmp` is one fixed name, safe because the lock serialises starts.
- **The local half by units alone.** No test waits 300 s, and tokio's paused clock (`test-util`) is in no part of the graph, so `answered` capping `None` at `STREAM_LIFE` survives SL3; SL2 pins a local `None`, SL1 `lived(.., None)` (`docs/features/stream-life.md`).
- **A launcher attaches to a hub.** `spec serve --root <served>` exits 0 with `already running` and a loopback URL that answers 403, and another root is told to stop the hub. AC-01 keeps `daemon.json`'s five keys, and it fails closed (the guide, `scripts/deploy/README.md`).

## Tests

http `tests/serve.rs` (17: AC-D1, D3-D8, D10, D11, the rules, a DIR's line, escaping, the probe's retry); `common/mod.rs` `beyond_daemon` (`HOME` beyond `daemon.lock`, `daemon.json`; 15 sites adapted), `try_start_program`, `signal_and_reap`; CLI `tests/common/serve.rs` (`Daemon`), `gate.rs` and `gate_wrapper.rs` `serve_d9_*`. Not covered: the Unix-only line, an exec failure, clap's `--port` refusal, raw controls in an unpublished reason (unreachable), a DIR's broken-config line naming `specengine.toml:1:` without its root.
