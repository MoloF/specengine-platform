---
class: spec
status: draft
scope: [crates/specengine-cli, crates/specengine-http]
ref: spec-serve analysis 2026-10-08 (A1-A13; Q1-Q3 decided under the owner's pre-approval, review open); 08 s2 Phase 2, the daemon
adrs: [ADR-0039]
---

# spec serve

## Why

The owner runs the UI as two processes (`specengine-http --root <project>`, `pnpm --dir ui dev`: `ui/README.md` "Owner's manual steps"), and nothing makes "the daemon" one thing: two can run under one `HOME`, no client can find one. `specengine-http` already is the daemon's HTTP API and SSE (`crates/specengine-http/README.md`); this slice gives it an entry point and an identity: `spec serve` launches it, one daemon per data directory, a discovery file says where it listens.

The gate stays daemon-free for good (ADR-0039, superseding the two gate decisions whose rule held "until `spec serve`"). The `spec mcp` bridge shows no gain today: warm direct MCP reads take 2-72 ms (`docs/canon/mcp-read.md`), MCP events reach the SSE within 1 s without it, direct writers are safe by design (WAL, `busy_timeout`, CAS). Cut: 1. `spec-serve`, this slice; 2. `ui-embed`, next in Phase 4: `/` serves the built UI under the same fence, `rust-embed` 8.12.0 approved (Q3); 3. `mcp-bridge` on a trigger ("Out of scope").

## Description and interactions

**`spec serve [DIR...] [--port N]`**, a launcher in the CLI (A1): find the roots, guard, look up, probe, exec. No async runtime, thread or socket in `spec`; no new dependency: eval `build_graph.rs` `CLI_FORBIDDEN` keeps `tokio`, `axum`, `hyper`, `rmcp`, `specengine-http` out of `spec`, whose start every gated write pays (`docs/canon/gate.md`).

1. **Roots** (A8), after the usage checks (Data, "Lines"). No DIR: the project any command finds (global `--root`, else the walk up from the cwd; none -> the CLI's exit 2 naming `spec init`). DIRs: each as `--root DIR` takes it (no walk), in order, a repeated canonical root once. Each root's config read and its `[project] slug` required as `specengine-http` reads them (the CLI's lines), the root canonicalised.
2. **Guard** (A13): the data directory (`crates/specengine-cli/README.md` "Database") inside any root -> exit 2, the CLI's line, nothing created (`checked_data_dir` per root).
3. **Lookup**: `specengine-http` in the directory of `std::env::current_exe()`, else the first `PATH` entry holding an executable regular file of that name; none -> exit 2 (Data, "Lines").
4. **Probe**, creating nothing: no data directory or no `daemon.lock` -> no daemon. Else the lock file opened read-only, `try_lock_shared`: acquired (dropped at once) -> no daemon, a `daemon.json` there stale; `WouldBlock` -> a daemon runs: `daemon.json` read (retried every 50 ms up to 2 s while missing or unreadable: a winner writes it after its bind) and answered without exec (Rules, A7).
5. **Exec** (`std::os::unix::process::CommandExt::exec`): argv `<found path> --root <root>... --port <N>`, `N` the given port, else 7777 (A6); environment and cwd unchanged. Exec failing -> exit 2 naming the path and the OS error. From then on the output and exits are `specengine-http`'s.

**`specengine-http` start**, new steps between the configs and the bind (A3, A4): the guard against every root (exit 2, nothing created); the data directory created if absent (the CLI's `prepared_data_dir`, made public); `daemon.lock` opened, created if absent, never written or deleted, and locked with `File::try_lock` (exclusive `flock`; std, stable since Rust 1.89, the workspace pins 1.90: the developer confirms), `WouldBlock` retried every 50 ms up to 500 ms (a probe holds its shared lock for microseconds), then exit 2 before binding. The daemon locks, not `spec`: Rust opens files close-on-exec, a lock taken before exec would die; a direct launch is covered too. Then the bind (taken -> exit 2 as today), `daemon.json` written atomically (`daemon.json.tmp`, rename) with the bound port, then the start lines as today. Any end, `SIGKILL` included, frees the lock; the discovery file is never removed and is trusted only while the lock is held.

Untouched: `spec gate`, `hooks/gate.sh`, the plugin (0.3.0, `.mcp.json`), `specengine-mcp` (agents' server, A10), the fence, the routes, `ui/` code, `.githooks`, CI.

## Data

**Files**, in the data directory beside `<slug>.db`, never under a root (ADR-0003): `~/Library/Application Support/specengine/`; else `$XDG_DATA_HOME/specengine/` when absolute, else `$HOME/.local/share/specengine/`.

- `daemon.lock`: empty; its `flock` is the one liveness signal.
- `daemon.json`: one JSON object, `serde_json::to_string_pretty` plus LF, keys exactly these five in this order, a project exactly `slug`, `root`:

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

`pid` the daemon's `std::process::id()`; `port` the bound one (`--port 0`: the chosen); `url` the `listening` line's; `projects` in `--root` order, `root` canonical as `/api/projects` gives it; `started` `utc_now()`, `YYYY-MM-DDTHH:MM:SSZ`. No token, key, secret, version or build (ADR-0034). Readers take the five keys and ignore others; a missing or mistyped one makes the file unreadable.

**Lines.** Start (`specengine-http`, unchanged): `serving <slug> <root>` per root, then `listening http://127.0.0.1:<port>`. Already serving (`spec`, stdout, exit 0):

```
already running: pid 41873
serving lantern-keep /srv/spec-a
listening http://127.0.0.1:7777
```

Exit 2, one stderr line, prefix `spec serve: `:

| Cause | Message |
|---|---|
| `--json` | `--json is not supported: the daemon prints lines` (A12) |
| `--config` | `--config is not supported: each root's specengine.toml is read` |
| `--root` and DIR | `give the roots as DIR... or as --root, not both` |
| no binary | `specengine-http not found beside <spec path> or on PATH; install it: cargo install --locked --path <clone>/crates/specengine-http` |
| served elsewhere | `a daemon (pid <pid>, <url>) serves <slug> <root>[, <slug> <root>]...; stop it, then run: spec serve <root>...[ --port <port>]` |
| unpublished | `a daemon holds <lock path>, but <discovery path> is unreadable: <reason>` |

`specengine-http`, exit 2 before binding, prefix `specengine-http: `: `a daemon already runs under <data dir>: pid <pid>, <url>, serving <slug> <root>[, ...]; stop it first`, or the unpublished line.

## Rules and edge cases

- WHEN the lock is held and every requested (slug, canonical root) pair is in `projects`, `spec serve` SHALL print the "already running" lines for the requested roots and exit 0 without exec; a `--port` other than the daemon's does not matter.
- WHEN the lock is held and a requested pair is not served (another root, or one now under another slug), it SHALL exit 2 with the "served elsewhere" line: every served root in order, then the new ones; `--port` only when the daemon's is not 7777; a root holding anything outside `A-Za-z0-9/._-` single-quoted (`'` as `'\''`). No live registration: a loopback route would let any local account widen what is served (A7, ADR-0034).
- `spec` never creates either file; the daemon is their only writer. A `daemon.json` with the lock free (crash, `kill -9`, a reused pid) is stale: the next start replaces it; no reader checks the pid.
- Two starts at once: one locks; the other retries 500 ms, then exit 2 naming the winner if its file is written, else the "unpublished" line.
- One daemon per data directory (A2): a scratch `HOME` or `XDG_DATA_HOME` gets its own; two under different ones race for 7777, the second exits 2 "port taken", no silent fallback (`--port 0` is the explicit way; the UI dev proxy targets 7777, `ui/vite.config.ts`).
- Foreground only (A5): SIGINT, SIGTERM end it, every write one SQLite transaction; no detach, idle exit, log file or request log.
- Unix only: on other targets `spec serve` exits 2 `not supported on this platform`.
- `spec gate` never reads `daemon.lock` or `daemon.json` nor connects anywhere: its verdicts are identical with the daemon up or down (A9, ADR-0039).
- Deleting the data directory under a live daemon lets a second start lock a new file: a limit, not guarded.

**Working answers** A1-A13, the analyst's, are marked `(An)` where they apply and held for the owner's review. Choices made here: positional `DIR...` for several roots (the global `--root` is single-valued); `daemon.lock` plus `daemon.json`; the probe by a shared lock; the 500 ms and 2 s retries; the restart command naming every root.

**Risks.** R1 canon contradicting ADR-0039: fixed in this change. R2 05 s1 (6) once made hooks bridges: reworded. R3 version skew: all crates 0.1.0, no build id; an old daemon after `cargo install` answers `SchemaTooNew` 503s ("restart the daemon"); a build id comes with the bridge. R4 port 7777 taken -> exit 2. R5 stale file: the lock decides, the retry covers simultaneous starts. R6 macOS scans a fresh binary's first exec for seconds: harmless in the foreground. R7 fence unchanged, no token. R8 (bridge): no line cap in `rmcp`'s stdio codec; cwd, `--root`, `--config`, git variables per connection; a macOS socket path caps at 104 bytes. R9 genre: `tests/no_domain.rs` keeps scanning http `src`. R10 a pilot with a per-root `--config` is not servable yet.

## Acceptance criteria

- [x] AC-S1 -- `docs/decisions/ADR-0039.md` <= 1536 B, `scope: [process]`, `canon: docs/canon/architecture.md#control`, superseding both earlier gate decisions, each now `status: superseded-by ADR-0039`; `#control`, `CLAUDE.md`, `gate.md`, the http README cite ADR-0039; `rg -n 'until .spec serve.' docs/canon CLAUDE.md` finds nothing; `export index && check` exit 0. M: ADR-0039's `canon:` at a missing anchor -> check red.
- [x] AC-S2 -- this spec; 05 s1 (6), 07 s1.1, s2, s3, 08 s1, s2 reworded, the bridge's triggers named (05 s1 (6)).

Scratch git byte copies of `fixtures/spec-a`, `-b`, a scratch `HOME`, `--port 0`, every child under an alarm:

- [ ] AC-D1 -- `spec serve --port 0` from `<copy>/docs` prints `serving lantern-keep <canonical copy>`, then `listening http://127.0.0.1:<p>`; `GET /api/projects` lists exactly that root. M: pass the cwd for the discovered root.
- [ ] AC-D2 -- `cargo nextest run -p specengine-eval --test build_graph` green, `CLI_FORBIDDEN`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`, `http_direct_dependencies_are_the_cli_library_axum_and_tokio` unchanged. M: `specengine-http` or `tokio` in `specengine-cli`.
- [ ] AC-D3 -- `spec` copied alone into an empty directory, `PATH` without `specengine-http`: exit 2, one stderr line naming both places and `cargo install`; the data directory absent after. M: exit 0, or create the data directory before the lookup.
- [ ] AC-D4 -- a daemon up under `HOME` H: a second `specengine-http`, and `spec serve` in the `-b` copy, exit 2 before any `listening`, naming the first's pid and URL; under H2 each starts; two `specengine-http --port 0` started at once under one `HOME`: exactly one listens. M: skip the lock.
- [ ] AC-D5 -- `spec serve` again in the served copy prints the three "already running" lines with the first's pid and URL, exits 0; `daemon.json` byte-identical, no new listener. M: always exec.
- [ ] AC-D6 -- `daemon.json` has exactly the five keys and a project exactly `slug`, `root`; `pid` the child's, `port` and `url` the `listening` line's, `projects` the `serving` lines in order, `started` shaped; no key holds `token`, `key` or `secret`. M: add a `token` key, or write the configured port 0.
- [ ] AC-D7 -- after `kill -9` the file still names the dead daemon; `spec serve` execs (no "already running"); the new start rewrites the file with the new pid and port. M: refuse whenever the file exists.
- [ ] AC-D8 -- `git status --porcelain` of every served root byte-identical before start, after reads, a stage and SIGTERM; both files only in the data directory; `HOME` set to a copy's root: `spec serve` and `specengine-http` exit 2, nothing created. M: write the discovery file into the root.
- [ ] AC-D9 -- `crates/specengine-cli/tests/gate.rs`, `gate_wrapper.rs`: a pass and a refusal give identical exit codes and stderr with a daemon up under the same `HOME` and with none. M: refuse when there is no `daemon.json`.
- [ ] AC-D10 -- `spec serve --json`, `spec serve --config F`, `spec --root A serve B`: exit 2, the usage line, nothing created; no `specengine.toml` up the tree: exit 2 naming `spec init`. M: accept `--json`.
- [ ] AC-D11 -- `cargo nextest run -p specengine-http` green (`fence.rs`, `door.rs`, `no_domain.rs`, `live.rs` asserting what they did), at least one Host-fence and one live-tail case also against a daemon started by `spec serve`; `-p specengine-mcp --test plugin_files` green, `.mcp.json` and 0.3.0 byte-identical. M: drop the Host check, or point `.mcp.json` at `spec mcp` without a bump.
- [ ] AC-C1 (review) -- `serve` in `spec` is roots, guard, lookup, probe, exec: no async runtime, thread, socket or decision path; `specengine-http` gains no route.
- [ ] AC-C2 (review) -- no new literal names a project, slug, kind or stack.

## Owner's manual check

O1: in this repository `spec serve`, then `pnpm --dir ui dev`: the tree shows; a second `spec serve` prints "already running" and the URL; Ctrl-C ends the first; `spec serve` again starts clean.

## Out of scope

`spec mcp` and HTTP `/mcp`; detach, idle exit, auto-start, a log file; registering a project into a live daemon; a persistent registry; per-root `--config` (`docs/features/daemon-read.md`); a watcher; the iCloud/Dropbox warning (05 s8); the embedded UI (`ui-embed`); the `specengine` alias (A11); SSE or notifications to agents; any gate or plugin change.

**`mcp-bridge`, on a trigger** (Q2; ADR-0019 unchanged): a watcher or in-memory index (Phase 3 drift), Phase 5's MCP staging forms, measured `Busy` exits, or a cross-process reject-versus-apply lock (`docs/canon/proposal-apply.md`). Then a byte relay of MCP JSON-RPC to the same `SpecEngineServer` hosted in the daemon, never tools mapped onto REST; the plugin's `.mcp.json` -> `spec mcp`, 0.4.0. Even then `spec approve`, `spec gate` and the CLI write directly: no sole writer.

## Open

1. Q1-Q3 decided by the orchestrator under the owner's pre-approval: ADR-0039 accepted, as the gate decision it supersedes was, the bridge deferred, `rust-embed` 8.12.0 for `ui-embed`; the owner's explicit review is open (the superseded gate decision's folds into it).
2. ADR-0038's review stays open, unrelated.
3. If `File::try_lock` is not usable on the pinned toolchain, the developer stops and reports: no new dependency without the owner.

## Implementation

Not yet. On shipping the canon moves: the http README "Binary" (start steps; the data directory written), the CLI README "Commands" (`serve`), `docs/canon/architecture.md#storage` (both files), the root `README.md` install line for `specengine-http`, `ui/README.md` step 2 -> `spec serve`, the `CLAUDE.md` state line.
