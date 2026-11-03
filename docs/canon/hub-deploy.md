---
class: canon
tier: 2
scope: [scripts/deploy, .github/workflows]
owner: owner
reviewed: 2026-11-03
---

# Hub deploy: compose, Traefik, Authelia and the image

ADR-0066, ADR-0067, ADR-0068, built by `hub-deploy` (RM-0064, step 11b-1) and `hub-accounts` (RM-0065, step 11c-1). Working answers of the orchestrator, 2026-11-03, the owner's review open. One compose file is a hub's whole host. It guarantees what the daemon cannot defend (`hub.md#trust` items 1-5, `hub-mcp.md#trust` item 6), each item a file a test pins. The operator's guide: `scripts/deploy/README.md`. Files: `scripts/deploy/` and `.github/workflows/image.yml`. Dumps, restore, upgrade, health and turning on `--push`: RM-0074.

## Host

Three services, each `restart: unless-stopped`, and the one-shot `admin` (`#accounts`).

- **`traefik`** publishes 80 and 443 alone. Its static configuration is its `command:` list, since Traefik's static sources exclude each other; the dynamic one is `traefik/dynamic/hub.yml` on the file provider, watched. `web` only redirects to `websecure`. ACME by TLS-ALPN, the CA from `.env`, staging first. No API, dashboard, version check or usage report.
- **`authelia`**, the login door at `DOOR_HOST`: the file users backend (`authelia/users/users.yml`, mounted read-only, a person re-read on every request: `refresh_interval: always`); SQLite storage and the filesystem notifier on the `authelia-data` volume; sessions in memory; passkeys and TOTP; password reset off. Its config is rendered by its template filter from the service's environment.
- **`specengine`**, the image: `network_mode: "service:traefik"`, no port or network of its own; `init: true`, since the daemon installs no signal handler and git children a sync's deadline kills must be reaped; `depends_on` `traefik` with `restart: true`; `/projects` bind-mounted; `/data` the named volume `specengine-data` (`XDG_DATA_HOME`), so the data directory lies outside every root.

`.env` keys, each read, none secret: `HUB_DOMAIN` (the door's cookie domain: a domain for the hub alone), `HUB_HOST`, `DOOR_HOST`, `ACME_EMAIL`, `ACME_CA_SERVER`, `SPECENGINE_VERSION` (the plugin's version, the image tag; `:?` guarded, never `latest`), `SPECENGINE_PUSH` (`0` or `1`). Secrets are files under `secrets/`, as Docker secrets: `deploy_key` and `known_hosts` to `specengine` alone, named by `GIT_SSH_COMMAND` (git's: `hub-git.md` "Credentials"); `authelia_session_secret`, `authelia_jwt_secret` to `authelia` alone, `authelia_storage_key` to it and `admin`, as `*_FILE`. Directories are mounted, never single files. `.gitignore` keeps `.env`, `secrets/`, `projects/`, `authelia/users/`, `traefik/mcp-users/`, `admin-log/` and the stamp out of git.

<a id="door"></a>
## Door

| Item | Met by |
|---|---|
| 1 | `specengine` in Traefik's namespace on 127.0.0.1:7777, no `ports`, `expose` or `networks`; `authelia`, on the compose network, cannot reach it |
| 2 | Router `hub` (`Host(HUB_HOST)`, priority 10) carries `door`: forward-auth to Authelia, `authResponseHeaders: [Remote-User]`, `trustForwardHeader: false`; Authelia takes the cookie session alone (`authn_strategies`), `default_policy: deny`, no `bypass`. Router `hub-mcp` (item 6's matcher, priority 20) carries `mcp-credential`. `websecure` rejects alias headers (`aliasHeadersStrategy=reject`: a `Remote_User` would otherwise reach the daemon's header parser); no entry point trusts forwarded headers |
| 3 | One service, `http://127.0.0.1:7777`, `passHostHeader: true` |
| 4 | The compose file is the host: no Docker socket, the file provider alone, nothing privileged or host-mode |
| 5 | The hub host's two Authelia rules: first `two_factor`, whose one resource is item 5's regex; then `one_factor` |
| 6 | `mcp-credential`: `basicAuth` on `traefik/mcp-users/users` (`<login>:<bcrypt hash>`, one line per person, written by `hub-admin`), `headerField: Remote-User`, `removeHeader: true`, only on a router whose `PathRegexp` is item 6's regex |

Every router to the daemon also carries `hardening` (`nosniff`, HSTS, COOP `same-origin`); none buffers or compresses. `websecure`'s `readTimeout` is 300 s, with no write timeout. `Authorization` stays in place on `hub`, so the daemon's refusal outside `/mcp` is a second line.

**The invariant item 5 rests on.** Authelia matches `path.Clean` of the decoded path, so `%2E%2E`, or a `%2F` inside an ID or slug segment, takes one factor at the door while the daemon sees another path. Item 5 holds only because the daemon matches exact raw segments (`hub.md` "Routes") and no slug or ID can be `.` or `..` or hold `/`: a change to either reopens the hole. A raw `#` arrives `%23` (from memory); a query on the four routes takes one factor, then the daemon's refusal.

**The MCP credential** is a per-person random secret whose Basic user is the door login, byte for byte, so one person stays one login. Claude Code sends it as a static header. Room is kept for an OAuth router on the same matcher. A login holding `:` cannot be a Basic user and gets none.

## Image

`ghcr.io/molof/specengine:<v>`, from `scripts/deploy/Dockerfile` (ADR-0066), its context the repository root through an allowlist, `Dockerfile.dockerignore`: the manifests, `Cargo.lock`, `crates/`, `ui/` without `node_modules` or `dist`, and the two scripts. Stages: Node 24 with pnpm through corepack builds `ui/dist`; Rust 1.90, the workspace `rust-version`, runs `cargo build --locked --release -p specengine-cli -p specengine-http`; Debian 13 slim holds `spec`, `specengine-http`, git (2.38 or later), `openssh-client`, `ca-certificates` and the base's `sh` (a sync's SIGTERM goes through `sh -c kill`). No git-lfs, Rust toolchain or `rust-analyzer`. User 10001 owns `/data`, `/projects` and its home `/var/lib/specengine/home`; the scripts are copied `--chmod=0755`; the label `org.opencontainers.image.source` links the GHCR package to the repository. No `# syntax=` line.

`.github/workflows/image.yml` runs on `v*` tags alone, `contents: read`. `check`: `tag-check.sh` (the tag is exactly `v` + the plugin's version, which `.env.example` holds too), the deploy tests, `compose config`, `docker build --check`, Authelia's `validate-config`. `build`, per architecture on native runners (`ubuntu-24.04`, `ubuntu-24.04-arm`): builds with `load`, smoke-tests (git 2.38 or later, no `git lfs`, `GET /` a 200 page), pushes `<v>-<arch>`. `merge` publishes the list `<v>`. Both pushing jobs take `packages: write`, run only for the owner's account, `MoloF`, log in, then fail when `<v>` is already published: one version, one build. Actions are pinned by commit SHA. The host only pulls.

## Entrypoint

`specengine-entrypoint`, POSIX `sh`, `LC_ALL=C`. Each folder of `/projects` (`*` with `[ -d ]`, so `a` sorts before `a-1`; files are passed over) holding a `specengine.toml` becomes one `--root` argument, in byte order; others are named `skipped`. Then `exec specengine-http --root ... --public-origin https://$HUB_HOST --port 7777`, `--push` last when `SPECENGINE_PUSH=1`. Exit 2, running nothing: no `HUB_HOST`; a `SPECENGINE_PUSH` but empty, `0` or `1`; no project, since `specengine-http` requires a `--root`. It reads its folders at start, so a new project needs a restart.

## Clone

`specengine-clone <url> <name> <committer-name> <committer-email>` runs in the image (`docker compose run --rm --entrypoint specengine-clone specengine ...`), so the container's git, user and deploy key make the checkout. `git clone --no-checkout --reject-shallow`, no depth or filter: every blob, since an apply renders the index from the committed tree and never fetches. Then `/specengine.toml` alone is checked out (`spec sparse-set --root` needs it on disk), and `spec sparse-set`'s paths become escaped, anchored `--no-cone --stdin` patterns. `user.name` and `user.email` are set; the upstream must be `origin/<branch>`, and the set complete. A bad argument or a taken folder: exit 2, nothing cloned. A later failure: exit 1, the folder left and named. A served root is its checkout's top.

## Pins

Exact, recorded 2026-11-03, each pending the owner; full digests and SHAs are in the files.

| Pin | Value |
|---|---|
| Traefik | `v3.7.17@sha256:68ae5fe5...3748` |
| Authelia | `4.39.28@sha256:bd97cff4...6f54` |
| Debian | `trixie-20261027-slim@sha256:f6b08b09...3ec4` |
| Node | `24.22.2-trixie-slim@sha256:54f84b4c...b446` |
| Rust | `1.90.0-slim-trixie@sha256:7fa728f3...b39e` |
| pnpm | 10.28.2, `ui/package.json` |
| Actions | `actions/checkout` v7.0.1 `3d3c42e5`, `docker/login-action` v4.7.0 `81c6f93d`, `docker/setup-buildx-action` v4.4.1 `f87e5991`, `docker/build-push-action` v7.4.0 `c3c9e263` |

<a id="accounts"></a>
## Accounts

ADR-0068. Root is the host's shell: `docker compose run --rm admin <act>` runs `admin/hub-admin`, POSIX `sh` on the door's own image and CLI. `admin` has `network_mode: none`, `logging: {driver: none}`, no port, `depends_on` or restart, and `profiles: [admin]`, so `up -d` never starts it; `specengine` mounts nothing of it; an act that writes, or `code`, needs a terminal. No crate holds account code.

It alone writes `users.yml`, the MCP users file and the stamp `traefik/dynamic/mcp-stamp.yml` (an unused value that goes up, so Traefik builds anew), each staged, read back and renamed under the lock `admin-log/lock/`, logged to `admin-log/acts.log` without secrets. Logins match `^[a-z]([a-z0-9._-]{0,62}[a-z0-9])?$`, never `local`, `agent`, `null`, `true`, `false`; names and the users file refuse what the door's YAML parser rejects. `disable` writes the door first; a `reset` that left a passkey exits 1, and the account stays disabled until one leaves none. `add` prints the `[team]` row; `spec team | hub-admin check` compares. The read-only users folder and a first `add` before `up -d` close 11b-1's possible default user. Acts and procedures: the guide.

## Gate

Step 11b-3, by hand: the owner's session on a clean VM, then the VPS, through H1-H17 of `docs/features/hub-deploy.md`, G1-G10 of `hub-accounts.md` and the claude.ai half of `hub-mcp.md` "Client check". Findings land here with versions and dates. Not yet run; `--push` and real people wait for it.

## Known limits

- A Traefik crash restarted by the engine, not by compose, leaves `specengine` in a dead namespace until `docker compose up -d`.
- `readTimeout` 300 s may cut an HTTP/1.1 GET stream or a POST waiting in the turn (Go's `ReadTimeout`). The gate measures; a drop becomes a limit here or a higher value.
- A re-run of `build` after a failed `merge` overwrites `<v>-<arch>` before the list `<v>` exists; hosts pin only the list.
- D12's `LC_ALL=C` mutation survives under dash on CI.
- Authelia's own entrypoint runs `chown -R 0:0 /config` on the read-only mount: errors at start, not fatal.
- AI on a hub means Claude Code until the claude.ai check; writers without it use the web UI.
- Sessions live in memory: an Authelia restart logs everyone out.
- A `specengine.toml` below its repository's top is not served.
- The MCP users file is read when its middleware is built (from memory), hence the stamp; VF2 open.
- `hub-admin` until VF1-VF3 (`docs/features/hub-accounts.md`): the door's CLI flags and "nothing to delete" answers are from memory, busybox `od` and `awk` untested, the NUL case proven on macOS awk alone. Its smaller limits: the guide.
- A private GHCR package needs a pull credential on the host.
- Unverified until the gate (from memory): that Traefik keeps the client's `Host` and drops a client's copy of a listed header also when the door returns none; that the door on a sibling subdomain makes the landing a `same-site` navigation (from the Fetch Metadata rules, tried in no browser).

## Tests

http `tests/deploy_admin.rs` (`hub-accounts`: `hub-admin` under `sh` with a stub door, no Docker); `tests/deploy.rs` (D1-D15: the files read with `serde-saphyr` and `toml`, the canon's two fences read as `src/hub.rs`'s units do, the entrypoint run with a stub daemon; no network, no Docker); CLI `tests/deploy_clone.rs` (D16, `file://` bare repositories); store `tests/queue_state.rs` `a_restored_login_off_the_hubs_grammar_is_refused_when_read` (D17, `Author.login` checked on decode); eval `build_graph.rs` (`scripts/` holds the hook installer and `deploy/`). CI: `image.yml`'s checks, C1-C3.
