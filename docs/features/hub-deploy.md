---
class: spec
status: shipped
shipped: 2026-11-03
scope: [scripts/deploy, .github/workflows, crates/specengine-store, crates/specengine-http, crates/specengine-cli]
ref: RM-0064; Phase 7 step 11b-1 (08 s2); the hub-deploy analysis and the orchestrator's decisions Q1-Q10 of 2026-11-03, the owner's review open; iteration 2 on the review's five hardenings
adrs: [ADR-0066, ADR-0067]
---

# hub-deploy: compose, Traefik and Authelia, from a clean VPS to a hub whose trust a test pins (RM-0064)

## Why

A hub serves named people, confirms, pushes and serves AI calls under a login, all resting on six things the daemon cannot defend: trust items 1-5 of `docs/canon/hub.md#trust` and item 6 of `docs/canon/hub-mcp.md#trust`. Until a deployment guaranteed them, a forged `Remote-User` passed as anyone. This slice writes that deployment as files a test pins and a host an operator can rebuild, and closes `Author.login` unchecked on decode. The daemon keeps its loopback bind (ADR-0034). Dumps, restore, upgrade, health and `--push`: RM-0074; the checks only a real host can make: the gate, step 11b-3 (H1-H17).

Decided by the orchestrator on 2026-11-03, each awaiting the owner's review:
- **Q1** the files in `scripts/deploy/` (ADR-0029's), `scripts` a walked root;
- **Q2** the MCP credential a per-person secret, Traefik `basicAuth` on item 6's routers, `headerField: Remote-User`, the user the door login; room for an OAuth router later;
- **Q3** AI on a hub means Claude Code until the claude.ai check;
- **Q4** Authelia's filesystem notifier;
- **Q5** off-host dumps the owner's call (RM-0074);
- **Q6** a domain for the hub alone, the owner's to buy, staging CA first;
- **Q7** third-party pins exact, pending the owner;
- **Q8** the first tag `v0.29.0`, any other refused (ADR-0015);
- **Q9** the first real project the owner's, never named here;
- **Q10** item 5's step-up kept; if unusable, two factors for every session.
- **The spec's choices:** S1 Traefik's static config is its `command:` list; S2 port 7777, a constant; S3 `removeHeader: true` on the credential; S4 a root is its checkout's top; S5 `readTimeout` 300 s; S6 per-architecture tags merged into the list; S7 `.env.example`'s version is the plugin's; S8 Authelia's password reset off.

## Description and interactions

Canon: `docs/canon/hub-deploy.md` (ADR-0066, ADR-0067); the guide: `scripts/deploy/README.md`.

### The host

One compose file, three services: `traefik` (80 and 443, TLS, the routers, forward-auth, the MCP users file), `authelia` (the door: accounts, sessions in memory, passkeys and TOTP), `specengine` (the image in Traefik's network namespace, the daemon on 127.0.0.1:7777, `/projects` and the `/data` volume, the deploy key). How each trust item is met: `hub-deploy.md` "Door".

### The image

Node 24 builds `ui/dist`, Rust 1.90 builds `spec` and `specengine-http`, Debian 13 slim runs them with git, ssh and CA roots, user 10001; `image.yml` builds it on a `v*` tag for amd64 and arm64 and publishes `ghcr.io/molof/specengine:<v>`: `hub-deploy.md` "Image".

## Data

The files are the truth; `hub-deploy.md` describes them.

### Files

`scripts/deploy/`: `README.md` (the guide), `Dockerfile`, `Dockerfile.dockerignore`, `docker-compose.yml`, `.env.example`, `.gitignore` (`.env`, `secrets/`, `projects/`, `authelia/users/`, `traefik/mcp-users/`), `entrypoint.sh`, `clone.sh`, `tag-check.sh`, `traefik/dynamic/hub.yml`, `authelia/config/configuration.yml`; `.github/workflows/image.yml`. The host's `.env`, `secrets/` (five files), `projects/`, Authelia's users file and the MCP users file are the guide's, never committed.

### `entrypoint.sh` (`specengine-entrypoint`)

Each folder of `/projects` (`*` with `[ -d ]`) holding a `specengine.toml` becomes a `--root`, in byte order; then `exec specengine-http ... --public-origin https://$HUB_HOST --port 7777 [--push]`. Exit 2: no `HUB_HOST`, a bad `SPECENGINE_PUSH`, no project.

### `clone.sh` (`specengine-clone`)

`<url> <name> <committer-name> <committer-email>`: `git clone --no-checkout --reject-shallow`, the config alone checked out, then `spec sparse-set`'s paths as escaped anchored `--no-cone --stdin` patterns, the identity, the upstream checked. Exit 2 with nothing cloned on a bad argument; exit 1 after the clone, the folder named.

### `tag-check.sh`

Exit 0 printing the version when the tag is exactly `v` + `plugin.json`'s version and `.env.example`'s `SPECENGINE_VERSION`; else exit 1.

### `image.yml`

On `v*` tags, `contents: read`: `check` (the tag, the deploy tests, `compose config`, `docker build --check`, Authelia's `validate-config`), `build` per architecture (load, smoke, push `<v>-<arch>`), `merge` (the list `<v>`). The pushing jobs run only for `MoloF`, take `packages: write`, log in, then fail if `docker buildx imagetools inspect "$IMAGE:$version"` succeeds.

### Store: `Author.login` on decode

Store `queue.rs`'s decoder checks a present `login` with core `hub_login_problem`, never `author_field_problem`: a failure is a corrupt row naming `author`; a row with no `login` key reads as before; `restore` still inserts as given.

## Rules and edge cases

As `hub-deploy.md`: one door per router to the daemon; item 5 rests on the daemon's exact raw segments, since Authelia matches the cleaned path; nothing in the deployment blocks anything (ADR-0012); no `src` change but store `queue.rs` (ADR-0008); no test needs Docker.

## Acceptance criteria

Tests: http `tests/deploy.rs` (D1-D15), CLI `tests/deploy_clone.rs` (D16), store `tests/queue_state.rs` (D17). M marks the mutations that turned each red.

- [x] D1 — One `two_factor` rule for the hub host, first, its one resource byte for byte item 5's fence. M: drop `|trace/clear`; move it below `one_factor`.
- [x] D2 — `default_policy: deny`, no `bypass`, two hub-host rules. M: a `bypass` for `^/assets/`.
- [x] D3 — Forward-auth takes `CookieSession` alone. M: add `HeaderAuthorization`.
- [x] D4 — Every router to the daemon carries `door` or `mcp-credential`, never both; no forwarded-headers trust; `aliasHeadersStrategy=reject`. M: drop `door`; drop `Remote-User` from `authResponseHeaders`; `trustForwardHeader: true`; drop the alias flag.
- [x] D5 — `mcp-credential` only with item 6's `PathRegexp`, outranking `door`, `removeHeader: true`. M: `^/mcp/`; the credential on `hub`; priorities swapped.
- [x] D6 — One server `http://127.0.0.1:<entrypoint port>`, `passHostHeader` never false. M: `passHostHeader: false`; `specengine:7777`; port 7778.
- [x] D7 — `specengine` in `service:traefik`, no ports; only `traefik` publishes 80, 443. M: publish 7777; `authelia` in the namespace.
- [x] D8 — No socket, the file provider alone, no API, no call home, nothing privileged or host-mode. M: the socket; `--providers.docker`; `--api.dashboard`.
- [x] D9 — Three services, exact pinned images, the `:?` guard, no anchors, `.env.example` at the plugin's version. M: `:latest`; drop `:?`; a service; `plugin.json` alone bumped.
- [x] D10 — Compose `${}` names are `.env.example`'s keys; every `{{ env }}` set by its service; no key material. M: `${HUB_EXTRA}`; `DOOR_HOST` dropped from `traefik`; a `-----BEGIN` value.
- [x] D11 — Each secret granted to one service; `specengine` mounts `projects` and its volume alone; `*_FILE` secrets; `.gitignore`. M: the key to `authelia`; Authelia's users into `specengine`; a literal secret; `.env` unignored.
- [x] D12 — The entrypoint's roots in byte order, one argument each, `--push` only on `1`, exit 2 cases; `init`, `restart`, the named volume. M: drop `init`; `--push` always; a bind-mounted `/data`; unquoted roots; no `LC_ALL=C` (killed on the laptop, not under dash).
- [x] D13 — `hardening` on every daemon router; no buffering or compression; routers on `websecure` with TLS; the redirect; `readtimeout` at least 300 s. M: drop nosniff; add buffering; drop the redirect; `60s`.
- [x] D14 — Pinned stages; Rust at `rust-version`; Node 24; the UI before cargo; Debian 13 slim with git, ssh, CA roots, no git-lfs; exactly two binaries; a non-root `USER`; the source label; the dockerignore. M: `git-lfs`; no `USER`; the UI after cargo; `rust:1.91`; no label.
- [x] D15 — Tags alone; `contents: read`, `packages: write` only where pushing; both architectures natively; checks before builds; SHA-pinned actions; the compose file's image name, no `latest`; the published-version guard; the owner condition; `tag-check.sh`'s cases. M: `branches: [main]`; no arm64; `@v4`; a prefix match; no guard; no condition.
- [x] D16 — A clone by `file://` holds every object and both commits, sparse, tracking `origin/main`, with its identity and nothing missing; bad arguments clone nothing; a shallow source is refused, no folder. M: `--filter=blob:none`; `--depth 1`; no sparse step; no `--reject-shallow`.
- [x] D17 — A restored `"login":"mira\nProposed-by: x"` is inserted, then `get` fails naming `author`; `\u{43c}\u{438}\u{440}\u{430}`, `Mira K` and no `login` read. M: no check; `author_field_problem`.
- [x] D18 — With `scripts` walked, `export index` and `check` are green and the guide fits Tier 1 (7 071 of 10 240 B).

CI only, on the first tag (not yet run):
- [ ] C1 — Both images build; git 2.38 or later, no `git lfs`, `GET /` 200 with the page.
- [ ] C2 — `docker build --check` is clean.
- [ ] C3 — Authelia's `validate-config` accepts the configuration.

The gate, step 11b-3, by hand (the owner's; findings in `hub-deploy.md` "Gate"):
- [ ] H1 — TLS for both hosts, staging then production.
- [ ] H2 — 7777 unreachable from outside and from `authelia`.
- [ ] H3 — A forged `Remote-User` dropped on `hub`, with and without a session; on `/mcp` the Basic user wins.
- [ ] H4 — Login lands on the board; the door's answer to an expired JSON `fetch`.
- [ ] H5 — One factor reaches none of the four step-up routes; a second opens them.
- [ ] H6 — Authelia's rule sees the query; the step-up is a session level.
- [ ] H7 — `./`, `//`, `%2F`, `%61`, `%2E%2E`, a `%2F` inside a segment, a trailing `/`, a raw `#` on the four routes reach no handler with one factor.
- [ ] H8 — Claude Code with the Basic header: sessions, 17 tools, `login` on a proposal and a card, the credential stopped elsewhere, SSE unbuffered, an HTTP/1.1 GET stream and a waiting POST held past 300 s (else a known limit or a higher S5), a non-ASCII login, a users-file edit.
- [ ] H9 — claude.ai and Claude Desktop: PAR, `Origin`, the bearer's `Remote-User`.
- [ ] H10 — Restarts: `traefik` through compose, a crashed `traefik`, a reboot.
- [ ] H11 — Passkey login, TOTP, deleting a lost passkey by command line, a relayed code.
- [ ] H12 — A project brought over end to end.
- [ ] H13 — Dumps and a restore (RM-0074).
- [ ] H14 — `--push`, the branch protection lifted for the deploy key (RM-0074).
- [ ] H15 — `spec serve` in the container attaches and fails closed.
- [ ] H16 — Health (RM-0074).
- [ ] H17 — The VPS's architecture pulls and runs the list tag.

## Out of scope

RM-0074's operations; RM-0065's `admin` and `spec admin`; an OAuth door and claude.ai as a client; Redis, a second host, building on the VPS, LFS; hardening beyond a non-root user; a root below its repository's top; a top-level `deploy/`; any daemon change but the decode check.

## Implementation

Two iterations: ACCEPT on the first; the second took the review's five hardenings (alias headers rejected, no overwritten release, no shallow source, forks never publish, the source label). Mutations: 52 and 19, all killed (D12's `LC_ALL=C` survives under dash on CI: `hub-deploy.md` "Known limits"). Tests: `deploy` 20 of 20, `deploy_clone` 3 of 3, `queue_state` 9 of 9, eval 58 of 58; the workspace 4851 of 4853, the dependency guard (clears on commit) and the load race `a_fifo_swapped_in_after_the_size_check_never_holds_the_open` (3 of 3 alone, RM-0063). Clippy and `spec check` clean.

Deviations from the draft, standing:
1. the entrypoint loops over `*` with `[ -d ]`, not `*/`, so `a` sorts before `a-1`;
2. the clone's sparse patterns are escaped and anchored, fed through `--no-cone --stdin`; a failure after the clone exits 1, the folder named;
3. `Dockerfile.dockerignore` is an allowlist;
4. `image.yml` is `check`, then a `build` matrix that loads, smoke-tests and pushes, then `merge`; `login-action` runs before the "The version is not published yet" guard, so a private package never reads as unpublished;
5. the scripts are copied `--chmod=0755`, with no `# syntax=` line;
6. the image user's home is `/var/lib/specengine/home`.

Pins, recorded and pending the owner: `hub-deploy.md` "Pins".

### Files in scope

| Module | What it does |
|---|---|
| `scripts/deploy/Dockerfile`, `Dockerfile.dockerignore` | three pinned stages, the allowlisted context, user 10001, the source label |
| `scripts/deploy/docker-compose.yml`, `.env.example`, `.gitignore` | the three services, the seven keys, the secrets and volumes; what stays out of git |
| `scripts/deploy/traefik/dynamic/hub.yml`, `authelia/config/configuration.yml` | routers `hub`, `hub-mcp`, `login`; `door`, `mcp-credential`, `hardening`; the two access rules, the cookie session |
| `scripts/deploy/entrypoint.sh`, `clone.sh`, `tag-check.sh` | the roots, the clone, the tag |
| `.github/workflows/image.yml` | check, build, merge |
| store `src/queue.rs` | `Author.login` checked with `hub_login_problem` |
| http `Cargo.toml`, `Cargo.lock` | dev `serde-saphyr`, `toml`: two edges, no new crate |
| Tests | http `tests/deploy.rs`; CLI `tests/deploy_clone.rs`; store `tests/queue_state.rs` `a_restored_login_off_the_hubs_grammar_is_refused_when_read`; eval `build_graph.rs` `no_retired_package_alias_or_script_is_left`, admitting `scripts/deploy/` (superseding `spec-cli-switch.md` AC-12 for `scripts/`) |
