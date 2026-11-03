---
class: spec
status: draft
scope: [scripts/deploy, .github/workflows, crates/specengine-store, crates/specengine-http, crates/specengine-cli]
ref: RM-0064; Phase 7 step 11b-1 (08 s2); the hub-deploy analysis and the orchestrator's decisions Q1-Q10 of 2026-11-03, each awaiting the owner's review
---

# hub-deploy: compose, Traefik and Authelia, from a clean VPS to a hub whose trust a test pins (RM-0064)

## Why

Since `hub-identity` a hub serves named people, and since `hub-confirm`, `hub-git` and `hub-mcp` it confirms, pushes and serves AI calls under a login. All of it rests on six things the daemon cannot defend itself: trust items 1-5 of `docs/canon/hub.md#trust` and item 6 of `docs/canon/hub-mcp.md#trust`. Until a deployment guarantees them, a forged `Remote-User` passes as anyone, so no real hub exists. This slice writes the deployment as files a test can pin and a host an operator can rebuild: one compose file, Traefik's and Authelia's config, an image built on a tag, an entrypoint, a clone script; and it closes the last known hole a restored dump could open, `Author.login` unchecked on decode. The daemon keeps its loopback bind (ADR-0034, `hub.md` "Entry"), so no bind ADR is needed. Dumps, restore, upgrade, health and the `--push` steps are 11b-2 `hub-ops` (RM-0074); the checks only a real host can make are the gate, step 11b-3, the owner's session (H1-H17 below).

Decided by the orchestrator on 2026-11-03 while the owner is away. **Each awaits the owner's review.**
- **Q1** The files live in `scripts/deploy/`: ADR-0029 already gives `scripts/` to the rust-developer, and the tester's revert check covers it; `scripts` joins `[paths] roots` at ship. A top-level `deploy/` would need an ADR amending ADR-0029 and the owner's own edits of `.claude/agents`; the owner can `git mv` the folder later.
- **Q2** The MCP credential, for now: a per-person random secret checked by Traefik's `basicAuth` on the routers whose rule is `Host(<hub>) && PathRegexp(<item 6>)`, `headerField: Remote-User`, its user name the person's Authelia login byte for byte. Claude Code sends it as a static header. Room stays for a later OAuth router on the same item-6 matcher plus `HeaderRegexp(Authorization, ^Bearer )`, forward-authing to a door that sets the login. Rejected: Basic checked by Authelia itself, which puts the door password in a client file and fails for passkey-only people. Step 11 left the credential's strength to the owner.
- **Q3** Until the claude.ai half of the client check, AI on a hub means Claude Code; writers without it use the web UI. A stated limit.
- **Q4** Authelia's notifier is the filesystem: one-time codes land in a file the operator relays, so no outside account is needed (RM-0064: recovery never depends on one).
- **Q5** Off-host dumps are the owner's call, since they need a storage account: RM-0074 waits for it. This slice states only that the volumes exist.
- **Q6** The owner buys the domain. Recommended: a domain for the hub alone, the hub and the door on two of its subdomains, so the door's cookie on that domain reaches nothing else; dry runs use Let's Encrypt's staging CA.
- **Q7** Third-party pins are exact versions and digests for Traefik, Authelia and the base images, and commit SHAs for actions, listed "pending the owner" as `docs/canon/stack.md` does for crates ("Pins" below).
- **Q8** The first release tag is `v` + the plugin's version, `v0.29.0`; `image.yml` refuses any other tag: one version, one build (ADR-0015).
- **Q9** The first real project is the owner's choice, never named in the repository.
- **Q10** Item 5's step-up stays. If the gate finds it unusable, every hub session requires two factors and item 5 holds trivially.

The spec's own choices, **awaiting the orchestrator's acceptance and the owner's review**:
- **S1** Traefik's static configuration is the `traefik` service's `command:` list, because its static sources (file, flags, environment) are mutually exclusive and a static file cannot read `.env` (from memory; Traefik refuses a bad flag at start, H1); the dynamic configuration is one file in a mounted directory, reading `{{ env "..." }}`.
- **S2** The daemon's port is a constant, 7777, in the entrypoint and in Traefik's service URL, pinned equal by D6; it is no `.env` key.
- **S3** The credential middleware has `removeHeader: true`: the secret stops at the proxy. Every other router leaves `Authorization` in place, so the daemon's refusal outside `/mcp` stays a second line (`hub-mcp.md` "Route").
- **S4** A served project's root is the top of its checkout, `/projects/<name>`; a `specengine.toml` deeper in its repository is unsupported, a known limit.
- **S5** `websecure`'s `readTimeout` is 300 s.
- **S6** Native per-architecture jobs push `<v>-amd64` and `<v>-arm64`, then a merge job publishes the list `<v>`; no digest artifacts.
- **S7** `.env.example`'s `SPECENGINE_VERSION` equals the plugin's version, so the deploy files ship at the version they name (`hub.md` "Identity").
- **S8** Authelia's password reset is off: a lost password is the operator's (RM-0065), as a lost passkey is.

## Description and interactions

### The host

One compose file is the whole host (ADR-0059: no agent, nothing else untrusted). Three services and nothing else; RM-0065 adds its one-shot `admin`.

| Service | Image | Network | Holds |
|---|---|---|---|
| `traefik` | Traefik v3, pinned | the compose network; publishes 80 and 443, nothing else | TLS (ACME, TLS-ALPN challenge), routers, the door's forward-auth, the MCP credential file |
| `authelia` | Authelia 4.39 or later, pinned | the compose network | accounts, sessions in memory, passkeys and TOTP, SQLite storage on its volume, the filesystem notifier |
| `specengine` | `ghcr.io/molof/specengine:${SPECENGINE_VERSION}` | `network_mode: "service:traefik"`, no port, no network of its own | the daemon on 127.0.0.1:7777, the checkouts in `/projects`, the data directory in `/data`, the deploy key |

The daemon's loopback is Traefik's: Traefik reaches it at `http://127.0.0.1:7777`, and `authelia`, on the compose network, cannot reach it at all. `specengine` depends on `traefik` with `restart: true`, so `docker compose restart traefik` restarts both; a Traefik crash restarted by the engine, not by compose, leaves `specengine` in a dead namespace until `docker compose up -d` (a known limit, H10; the fallback, a namespace-holder service, is a note, never built here).

### The door: how each trust item is met

| Trust item | How the deployment meets it | Pinned by |
|---|---|---|
| 1. Port unpublished; only the proxy in the namespace | `specengine` has `network_mode: "service:traefik"`, no `ports`, `expose` or `networks`; no other service joins that namespace | D7 |
| 2. `Remote-User` only from the proxy | Browser routers: forward-auth to Authelia, `Remote-User` in `authResponseHeaders`, `trustForwardHeader` off; Authelia denies by default and has no `bypass`. `/mcp` routers: the credential middleware sets it (item 2 reworded at ship by ADR `hub-door`) | D2, D4 |
| 3. `passHostHeader` on | never false; one service URL, `http://127.0.0.1:7777` | D6 |
| 4. Nothing untrusted on the host | the compose file is the host; no Docker socket; the file provider alone; no privileged or host-mode service | D8, D9 |
| 5. Step-up on the confirm class, by its regex alone | one Authelia `two_factor` rule whose only resource is item 5's regex, first among the hub host's two rules | D1 |
| 6. The MCP credential opens only `^/mcp/[^/]+$` | Traefik `basicAuth` with `headerField: Remote-User`, only on routers whose `PathRegexp` is item 6's regex | D5 |

A raw `#` reaches Traefik as part of the path; Go's request parsing keeps it, and the forwarded path and Authelia's `X-Forwarded-Uri` both carry it encoded, `%23` (from memory, H7), which the daemon's exact raw segments refuse, so a handler never answers a form the `$`-anchored rule missed (`hub.md` "Known limits"). Path variants (`./`, `//`, `%2F`, `%61`) reach Authelia and the daemon in one form, Traefik's (from memory, H7). Authelia's `resources` sees the query (from memory, H6), so `.../confirm?x` takes one factor at the door and the daemon's query refusal after it.

### The image

`scripts/deploy/Dockerfile`, built with the repository root as its context and `scripts/deploy/Dockerfile.dockerignore` beside it, in three stages: Node 24 with pnpm 10.28.2 (`ui/package.json` `packageManager`, through corepack) builds `ui/dist`; a Rust 1.90 builder (`rust-version` of the root `Cargo.toml`) runs `cargo build --locked --release -p specengine-cli -p specengine-http` with that `ui/dist` in place, so the page is compiled in (`docs/canon/serve-ui.md`); a pinned Debian 13 slim runtime takes `spec` and `specengine-http` alone, with `git` (2.38 or later: `merge-tree --write-tree`), `openssh-client`, `ca-certificates` and the base's `sh`, which the sync's SIGTERM needs (store `worktree.rs` sends it through `sh -c kill`), so no distroless image. No `git-lfs` (`hub-git.md` "Known limits"), no Rust toolchain, no `rust-analyzer`. A non-root user, uid and gid 10001, owns `/data` and `/projects` in the image, so a fresh named volume takes that ownership.

`.github/workflows/image.yml` runs on `v*` tags alone. A check job runs `tag-check.sh`, the deploy tests and `compose config`; two native jobs (`ubuntu-24.04`, `ubuntu-24.04-arm`) build, smoke-test (C1) and push `<v>-amd64`, `<v>-arm64`; a merge job publishes the list `<v>`. The server only pulls; the VPS never builds; with no registry, `docker save` piped over SSH to `docker load`.

### Bringing a project over

The guide, `scripts/deploy/README.md`, written at ship from "Guide" below, takes the operator through it; the steps that touch git are `specengine-clone`, run inside the image so the container's git, user and deploy key make the checkout:

1. On the laptop, decide every open proposal, since no path is rewritten (ADR-0032) and an open one arrives an orphan, only rejectable; then `spec export state`.
2. `docker compose run --rm --entrypoint specengine-clone specengine <ssh-url> <name> <committer-name> <committer-email>`: a clone with every blob, sparse to the set `spec sparse-set` prints, with its upstream and a committer identity.
3. `docker compose up -d`, or `docker compose restart specengine` for a later project: the entrypoint reads the folder at start, and now serves `/projects/<name>`.
4. The dump copied to `projects/<name>.jsonl` (the entrypoint serves folders, never files), then `docker compose exec -it specengine spec import-state --root /projects/<name> /projects/<name>.jsonl`, whose consent is a terminal, into the empty queue; the dump removed.

### Until RM-0065

Accounts are made by hand: an Authelia users file whose hashes come from Authelia's own CLI in its container, and, for a person who needs AI, one line in the MCP credential file, `<login>:<bcrypt hash>` of a fresh random secret. Claude Code adds the hub with `claude mcp add --transport http --scope user <name> https://<hub>/mcp/<slug> --header "Authorization: Basic <base64 of login:secret>"`: `--scope user`, since a committed `.mcp.json` leaks the secret. A login holding `:`, which `hub-team.md` "Team" allows inside a key, cannot be a Basic user name, so that person gets no credential.

## Data

### Files

```text
scripts/deploy/
  README.md                     the guide (Tier 1, written at ship)
  Dockerfile
  Dockerfile.dockerignore
  docker-compose.yml
  .env.example
  .gitignore                    .env, secrets/, projects/, authelia/users/, traefik/mcp-users/
  entrypoint.sh                 in the image as /usr/local/bin/specengine-entrypoint
  clone.sh                      in the image as /usr/local/bin/specengine-clone
  tag-check.sh                  image.yml's first step
  traefik/dynamic/hub.yml       routers, middlewares, services
  authelia/config/configuration.yml
.github/workflows/image.yml
```

On the host, made by the guide and ignored by git: `.env`; `secrets/` (`deploy_key`, `known_hosts`, `authelia_session_secret`, `authelia_storage_key`, `authelia_jwt_secret`, each mode 600); `projects/` (owned by 10001); `authelia/users/users.yml`; `traefik/mcp-users/users`. Directories are mounted, never single files: an editor replaces a file, which a watch on it misses.

### `.env.example`

```sh
# The door's cookie domain: a domain for the hub alone (Q6).
HUB_DOMAIN=example.org
# The daemon's --public-origin is https://$HUB_HOST.
HUB_HOST=hub.example.org
# Authelia's portal, a sibling subdomain.
DOOR_HOST=login.example.org
ACME_EMAIL=ops@example.org
# Staging first; production after the dry run.
ACME_CA_SERVER=https://acme-staging-v02.api.letsencrypt.org/directory
# The plugin's version (S7): the image tag.
SPECENGINE_VERSION=0.29.0
# 1 turns on --push, after the gate (RM-0074).
SPECENGINE_PUSH=0
```

Comments stand on their own lines, so a value never carries one.

Every key is read and every read is a key (D10). RM-0064's "repository URL" is no key: the clone takes it as an argument, one per project.

### `docker-compose.yml`, the load-bearing keys

```yaml
services:
  traefik:
    image: traefik:v3.X.Y@sha256:<pending the owner>
    restart: unless-stopped
    ports: ["80:80", "443:443"]
    environment: {HUB_HOST: "${HUB_HOST}", DOOR_HOST: "${DOOR_HOST}"}
    command:
      - --global.checknewversion=false
      - --global.sendanonymoususage=false
      - --entrypoints.web.address=:80
      - --entrypoints.web.http.redirections.entrypoint.to=websecure
      - --entrypoints.web.http.redirections.entrypoint.scheme=https
      - --entrypoints.websecure.address=:443
      - --entrypoints.websecure.transport.respondingtimeouts.readtimeout=300s
      - --providers.file.directory=/etc/traefik/dynamic
      - --providers.file.watch=true
      - --certificatesresolvers.le.acme.email=${ACME_EMAIL}
      - --certificatesresolvers.le.acme.caserver=${ACME_CA_SERVER}
      - --certificatesresolvers.le.acme.storage=/acme/acme.json
      - --certificatesresolvers.le.acme.tlschallenge=true
    volumes:
      - ./traefik/dynamic:/etc/traefik/dynamic:ro
      - ./traefik/mcp-users:/etc/traefik/mcp-users:ro
      - traefik-acme:/acme
  authelia:
    image: authelia/authelia:4.X.Y@sha256:<pending the owner>
    restart: unless-stopped
    environment:
      X_AUTHELIA_CONFIG_FILTERS: template
      HUB_DOMAIN: "${HUB_DOMAIN}"
      HUB_HOST: "${HUB_HOST}"
      DOOR_HOST: "${DOOR_HOST}"
      AUTHELIA_SESSION_SECRET_FILE: /run/secrets/authelia_session_secret
      AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE: /run/secrets/authelia_storage_key
      AUTHELIA_IDENTITY_VALIDATION_RESET_PASSWORD_JWT_SECRET_FILE: /run/secrets/authelia_jwt_secret
    secrets: [authelia_session_secret, authelia_storage_key, authelia_jwt_secret]
    volumes:
      - ./authelia/config:/config:ro
      - ./authelia/users:/users
      - authelia-data:/var/lib/authelia
  specengine:
    image: ghcr.io/molof/specengine:${SPECENGINE_VERSION:?set SPECENGINE_VERSION in .env}
    restart: unless-stopped
    init: true
    network_mode: "service:traefik"
    depends_on:
      traefik: {condition: service_started, restart: true}
    environment:
      HUB_HOST: "${HUB_HOST}"
      SPECENGINE_PUSH: "${SPECENGINE_PUSH}"
      GIT_SSH_COMMAND: ssh -i /run/secrets/deploy_key -o IdentitiesOnly=yes -o UserKnownHostsFile=/run/secrets/known_hosts -o StrictHostKeyChecking=yes
    secrets: [deploy_key, known_hosts]
    volumes:
      - ./projects:/projects
      - specengine-data:/data
secrets:            # each {file: ./secrets/<name>}
  deploy_key: {file: ./secrets/deploy_key}
  # known_hosts, authelia_session_secret, authelia_storage_key, authelia_jwt_secret alike
volumes: {specengine-data: {}, authelia-data: {}, traefik-acme: {}}
```

Authelia's key names, the template filter and the JWT secret's need are checked against the pinned version by C3 (from memory). The deploy key is git's (`hub-git.md` "Credentials"): no `src` string names it, and `GIT_SSH_COMMAND` reaches the sync's git because the store passes the process's variables but git's local ones (`GitEnv`, `worktree.rs` `LOCAL_ENV_VARS`). `init: true` because the daemon installs no signal handler and must reap the git children a sync's deadline kills. `XDG_DATA_HOME=/data` and `HOME=/home/specengine` are set in the image, so the data directory (`crates/specengine-cli/README.md` "Database") lies outside every root.

### `traefik/dynamic/hub.yml`

```yaml
http:
  routers:
    hub:
      rule: 'Host(`{{ env "HUB_HOST" }}`)'
      entryPoints: [websecure]
      priority: 10
      middlewares: [door, hardening]
      service: specengine
      tls: {certResolver: le}
    hub-mcp:
      rule: 'Host(`{{ env "HUB_HOST" }}`) && PathRegexp(`^/mcp/[^/]+$`)'
      entryPoints: [websecure]
      priority: 20
      middlewares: [mcp-credential, hardening]
      service: specengine
      tls: {certResolver: le}
    login:
      rule: 'Host(`{{ env "DOOR_HOST" }}`)'
      entryPoints: [websecure]
      service: authelia
      tls: {certResolver: le}
  middlewares:
    door:
      forwardAuth:
        address: http://authelia:9091/api/authz/forward-auth
        trustForwardHeader: false
        authResponseHeaders: [Remote-User]
    mcp-credential:
      basicAuth:
        usersFile: /etc/traefik/mcp-users/users
        headerField: Remote-User
        removeHeader: true
    hardening:
      headers:
        contentTypeNosniff: true
        stsSeconds: 31536000
        customResponseHeaders: {Cross-Origin-Opener-Policy: same-origin}
  services:
    specengine:
      loadBalancer: {servers: [{url: "http://127.0.0.1:7777"}], passHostHeader: true}
    authelia:
      loadBalancer: {servers: [{url: "http://authelia:9091"}]}
```

A "router to the daemon" is one whose service is `specengine`. Only `Remote-User` is listed: `Remote-Name`, `Remote-Email` and `Remote-Groups` are never read (`hub.md` "Identity"). No `buffering` or `compress` middleware: SSE and MCP streams pass unbuffered. The users file is read when the middleware is built, so the guide touches `hub.yml` after editing it (from memory, H8).

### `authelia/config/configuration.yml`, the keys the tests read

```yaml
server:
  address: 'tcp://:9091/'
  endpoints:
    authz:
      forward-auth:
        implementation: ForwardAuth
        authn_strategies: [{name: CookieSession}]      # no HeaderAuthorization: no Basic at the door
authentication_backend:
  password_reset: {disable: true}                     # S8
  file: {path: /users/users.yml, watch: true}
access_control:
  default_policy: deny
  rules:
    - domain: '{{ env "HUB_HOST" }}'
      resources: ['^/api/projects/[^/]+/(proposals/[^/]+/confirm|cards/[^/]+/(accept|clear)|trace/clear)$']
      policy: two_factor
    - domain: '{{ env "HUB_HOST" }}'
      policy: one_factor
session:
  cookies:
    - domain: '{{ env "HUB_DOMAIN" }}'
      authelia_url: 'https://{{ env "DOOR_HOST" }}'
      default_redirection_url: 'https://{{ env "HUB_HOST" }}/'
storage: {local: {path: /var/lib/authelia/db.sqlite3}}
notifier: {filesystem: {filename: /var/lib/authelia/notification.txt}}
totp: {issuer: '{{ env "HUB_HOST" }}'}
webauthn: {enable_passkey_login: true}
```

Sessions live in memory: an Authelia restart logs everyone out. Passkey login needs Authelia 4.39 or later (from memory); if the gate finds it unusable, passkeys stay a second factor beside TOTP and the guide says so.

### `entrypoint.sh` (`specengine-entrypoint`)

POSIX `sh`, `set -eu`, `LC_ALL=C`. Reads `HUB_HOST`, `SPECENGINE_PUSH` and `SPECENGINE_PROJECTS` (default `/projects`, set only by tests). For each folder `"$SPECENGINE_PROJECTS"/*/` in byte order: a regular file `specengine.toml` in it makes `--root <folder>`, its trailing `/` dropped, one argument whatever the name holds; else stderr `specengine-entrypoint: skipped <folder>: no specengine.toml`. Then:

```sh
exec specengine-http --root <folder>... --public-origin "https://$HUB_HOST" --port 7777 [--push]
```

| Input | Answer |
|---|---|
| `HUB_HOST` unset or empty | exit 2, `specengine-entrypoint: HUB_HOST is not set: give it in .env` |
| `SPECENGINE_PUSH` unset, empty or `0` | no `--push` |
| `SPECENGINE_PUSH` `1` | `--push` last |
| any other `SPECENGINE_PUSH` | exit 2, `specengine-entrypoint: SPECENGINE_PUSH is <value>: give 0 or 1` |
| no folder holds a `specengine.toml` | exit 2, `specengine-entrypoint: no project under <dir> holds a specengine.toml: bring one over first (scripts/deploy/README.md)`, since `specengine-http` requires a `--root` |

Nothing is created; the daemon judges the rest (`--public-origin`'s grammar: `hub.md` "Entry").

### `clone.sh` (`specengine-clone`)

`specengine-clone <url> <name> <committer-name> <committer-email>`, POSIX `sh`, under `SPECENGINE_PROJECTS` like the entrypoint.

1. `<name>` matches `[a-z0-9][a-z0-9-]*`, at most 64 bytes, and `<dir>/<name>` does not exist; else exit 2, nothing cloned. Four arguments exactly.
2. `git clone --no-checkout --origin origin -- <url> <dir>/<name>`: no `--depth`, `--filter`, `--shallow-*`, `--reference` or `--shared`.
3. A sparse checkout (`--no-cone`, anchored patterns) of `/specengine.toml` alone, the worktree populated on the default branch: `spec sparse-set --root` needs the config on disk though it reads `HEAD`'s.
4. `spec sparse-set --root <dir>/<name>`'s paths become the sparse set, `/<path>` each, a directory keeping its `/`.
5. `user.name` and `user.email` set in the checkout's config.
6. `@{upstream}` must be `origin/<branch>`, and `spec sparse-set` must print no `warning:`; else exit 1 naming what is wrong and the folder, which is left for the operator to remove.
7. stdout `cloned <name>: <branch> tracking origin/<branch>, <n> paths`.

The URL is SSH in the guide; the script judges none (git's own URL rules and `GIT_ALLOW_PROTOCOL` stand).

### `tag-check.sh`

`tag-check.sh <tag>`: exit 0 and stdout `<version>` when `<tag>` is exactly `v` + `plugin/specengine/.claude-plugin/plugin.json`'s `version` and `.env.example`'s `SPECENGINE_VERSION` is that version; else exit 1, `tag-check: <tag> is not v<version> (plugin.json, .env.example)`. Paths from the script's own location.

### `image.yml`

```yaml
name: image
"on": {push: {tags: ["v*"]}}
permissions: {contents: read}
jobs:
  check:   # ubuntu-24.04: tag-check.sh "$GITHUB_REF_NAME"; cargo test --locked -p specengine-http --test deploy;
           # cargo test --locked -p specengine-cli --test deploy_clone; docker compose ... config --quiet (C2, C3 here too)
  build:   # needs check; matrix {linux/amd64: ubuntu-24.04, linux/arm64: ubuntu-24.04-arm};
           # permissions {contents: read, packages: write}; build, load, smoke (C1), push ghcr.io/molof/specengine:<v>-<arch>
  merge:   # needs build; permissions {contents: read, packages: write};
           # docker buildx imagetools create -t ghcr.io/molof/specengine:<v> <v>-amd64 <v>-arm64
```

Every `uses:` is pinned to a 40-hex commit SHA with its version in a comment. No `latest` tag, no secret but `GITHUB_TOKEN`, no Rust toolchain action (the runner's `cargo`). The GHCR name is lowercase: `molof`, the repository owner's.

### Store: `Author.login` on decode

In store `queue.rs`'s proposal decoder, beside the loop over `role`, `model` and `run` (`queue.rs:2303-2307`, `author_field_problem`): a present `login` failing core `hub_login_problem` (`card.rs`) makes the row corrupt, column `author`, as a bad `role` does (`docs/canon/proposal-queue.md` "Store", **Checked when read**). Never `author_field_problem` (`proposal.rs:240`): printable ASCII without spaces would refuse non-ASCII and inner-space logins, which are valid hub logins (`hub.md` "Identity"). `restore` still inserts rows as given (`queue/state.rs`). A row with no `login` key reads as before; stored bytes, dumps and trailers are unchanged.

## Rules and edge cases

- WHEN a router's service is `specengine`, it SHALL carry `door`, or `mcp-credential` and then only with item 6's matcher; no router to the daemon carries neither or both.
- WHEN a request matches both hub routers, `hub-mcp` SHALL win by its explicit priority; a `/mcp/<slug>/x` falls to `hub`, which takes the door's session alone, so the credential opens nothing more.
- WHEN a client sends `Remote-User` on a browser router, forward-auth SHALL replace or drop it, with or without a session (from memory, H3); on `/mcp` the Basic user replaces it (from memory, H3).
- WHEN a client sends `X-Forwarded-*`, no entry point SHALL trust it, and forward-auth computes its own (`trustForwardHeader: false`).
- WHEN `.env` lacks `SPECENGINE_VERSION`, `docker compose` SHALL refuse to start (`:?`), never pulling `latest`.
- WHEN the projects folder holds no project, `specengine` SHALL exit 2 with the entrypoint's line; under `restart: unless-stopped` it retries, so the guide clones before `up`.
- WHEN a pushed tag is not `v` + the plugin's version, `image.yml` SHALL fail before building anything.
- WHEN a dumped `Author.login` fails `hub_login_problem`, the row SHALL be corrupt naming `author`; nothing reaches `Proposed-by:`.
- WHEN the clone's name or arguments are bad, nothing SHALL be cloned; a failure after the clone leaves the folder and names it.
- The deployment blocks nothing (ADR-0012, ADR-0039): no status, check or gate reads it. The core knows no deployment (ADR-0008): no `src` change but store `queue.rs`.
- Ownership: the bind-mounted `projects/` and the deploy key must be the container user's (10001), or git refuses a "dubious ownership" and ssh a key file's mode; the clone runs inside the container for that reason.
- `docker compose` is not among the session's allowed commands (the owner's settings, never a role's), so no test needs Docker; `compose config` runs in CI and by hand.

## Acceptance criteria

The tests read the committed files with no network, no image build and no Docker: http `tests/deploy.rs` (D1-D15) parses YAML with `serde-saphyr` and the root `Cargo.toml` with `toml` into its own types or `serde_json::Value`, and reads the canon's two fences as `src/hub.rs`'s units do; CLI `tests/deploy_clone.rs` (D16) runs `clone.sh` with the built `spec` first on `PATH`; store `tests/queue_state.rs` (D17). M marks the mutation that must turn each red.

- [ ] D1 — Authelia has exactly one `two_factor` rule naming the hub host. Its `resources` is one entry, byte for byte the fenced `^/api` line of `docs/canon/hub.md` "Trust", and it is the first rule naming the hub host. M: delete `|trace/clear`; move the rule below `one_factor`.
- [ ] D2 — `default_policy` is `deny`; no rule anywhere is `bypass`; the hub host has exactly two rules, `two_factor` then `one_factor`. M: add a `bypass` rule for `^/assets/`.
- [ ] D3 — The forward-auth endpoint's `authn_strategies` is exactly `CookieSession`. M: add `HeaderAuthorization`.
- [ ] D4 — Every router to the daemon carries either `door` (forward-auth with `Remote-User` in `authResponseHeaders` and `trustForwardHeader` not true) or `mcp-credential` (`basicAuth` with `headerField: Remote-User`), never both; no middleware removes `Authorization` on a router carrying `door`; no `--entrypoints.*.forwardedheaders.*` or `proxyprotocol` flag. M: remove `door` from `hub`; remove `Remote-User` from `authResponseHeaders`; set `trustForwardHeader: true`.
- [ ] D5 — Every router carrying `mcp-credential` has exactly one `PathRegexp`, byte for byte the fenced `^/` line of `docs/canon/hub-mcp.md` "Trust", and a priority above every `door` router of the same host; no other router carries `basicAuth`; `removeHeader` is true. M: the regex `^/mcp/`; `mcp-credential` on `hub`; the two priorities swapped.
- [ ] D6 — The `specengine` service has one server, `http://127.0.0.1:<N>`, where `N` is the `--port` the entrypoint passes (D12's stub); `passHostHeader` is never false. M: `passHostHeader: false`; `http://specengine:7777`; the entrypoint's port 7778.
- [ ] D7 — `specengine` has `network_mode: service:traefik` and no `ports`, `expose` or `networks`; no other service has a `network_mode`; only `traefik` has `ports`, exactly `80:80` and `443:443`. M: publish 7777; give `authelia` the namespace.
- [ ] D8 — No volume or mount names `docker.sock`; Traefik's only provider flags are `--providers.file.directory` and `--providers.file.watch`; no `--api*` flag; `checknewversion` and `sendanonymoususage` are false; no service is `privileged` or has `pid`, `ipc` or `network_mode` `host`. M: mount the socket; add `--providers.docker=true`; add `--api.dashboard=true`.
- [ ] D9 — The services are exactly `traefik`, `authelia`, `specengine`. `specengine`'s image is `ghcr.io/molof/specengine:` + `${SPECENGINE_VERSION:?` ... `}`; every other `image` holds an exact `x.y.z` version and `@sha256:` + 64 hex, never `latest`. No YAML file under `scripts/deploy/` or `image.yml` holds an anchor, alias or `<<:` merge key. `.env.example`'s `SPECENGINE_VERSION` is `plugin.json`'s version. M: `traefik:latest`; drop the `:?`; add a service; bump `plugin.json` alone.
- [ ] D10 — The `${NAME...}` names in `docker-compose.yml` are exactly `.env.example`'s keys; every `{{ env "NAME" }}` in `hub.yml` or `configuration.yml` is a name that service's `environment` sets from `${NAME}`; no value in `.env.example` or the compose file holds `-----BEGIN`. M: add `${HUB_EXTRA}` to the compose file; drop `DOOR_HOST` from `traefik`'s `environment`; put a `-----BEGIN` value in `.env.example`.
- [ ] D11 — `deploy_key` and `known_hosts` are granted to `specengine` alone, the three `authelia_*` secrets to `authelia` alone, each a `file:` under `./secrets/`; `specengine` mounts only `./projects` and `specengine-data`; no `./authelia`, `./traefik` or `./secrets` path is mounted into it; every `environment` name holding `SECRET`, `PASSWORD`, `TOKEN` or `KEY` ends `_FILE` with a value under `/run/secrets/`. `scripts/deploy/.gitignore` ignores `.env`, `secrets/`, `projects/`, `authelia/users/` and `traefik/mcp-users/`. M: grant `deploy_key` to `authelia`; mount `./authelia/users` into `specengine`; set `AUTHELIA_SESSION_SECRET` to a literal; drop `.env` from `.gitignore`.
- [ ] D12 — `entrypoint.sh`, run by `sh` with `HUB_HOST=hub.example.org`, `SPECENGINE_PROJECTS` a scratch folder and a stub `specengine-http` first on `PATH` that prints its arguments one per line: folders `b`, `a`, `B`, `a-1`, `with space` holding a `specengine.toml` and `empty` holding none give exactly `--root <d>/B --root <d>/a --root <d>/a-1 --root <d>/b --root <d>/with space --public-origin https://hub.example.org --port 7777`, `skipped .../empty` on stderr; `SPECENGINE_PUSH=1` adds `--push` last; `0`, empty and unset add nothing; `yes` and an unset `HUB_HOST` exit 2 with "Data"'s lines and never run the stub; no project exits 2 with its line. Compose gives `specengine` `init: true`, `restart: unless-stopped` and the named volume `specengine-data` at `/data`. M: drop `init`; pass `--push` always; bind-mount `./data` at `/data`; expand the roots unquoted; glob without `LC_ALL=C`.
- [ ] D13 — Every router to the daemon carries `hardening`, which sets `contentTypeNosniff: true`, a non-zero `stsSeconds` and `Cross-Origin-Opener-Policy: same-origin`; no middleware anywhere is `buffering` or `compress`; every router's `entryPoints` is exactly `[websecure]` with TLS; `web` has the redirect flags to `websecure` and `https`; `websecure`'s `readtimeout` is at least `300s`, and no `writetimeout` is set. M: drop `contentTypeNosniff`; add a `buffering` middleware; drop the redirect; set `readtimeout=60s`.
- [ ] D14 — The Dockerfile's every `FROM` holds a version and `@sha256:` + 64 hex; the build stage's Rust tag starts with the root `Cargo.toml`'s `rust-version` (read with `toml`); the Node stage's tag starts `24.`; the UI is built, and its `dist` copied into the build stage, before `cargo build --locked`; the last stage is Debian 13 slim, installs `git`, `openssh-client` and `ca-certificates` and never `git-lfs`, copies from the build stage exactly `spec` and `specengine-http`, and has a `USER` that is neither `root` nor `0`; `Dockerfile.dockerignore` exists and excludes `target`, `target.noindex`, `.git` and `node_modules`. M: add `git-lfs`; remove `USER`; build the UI after cargo; `rust:1.91`.
- [ ] D15 — `image.yml` runs on `push` of `tags: [v*]` alone (no `branches`, `pull_request` or `workflow_dispatch`); the workflow's permissions are `contents: read`, and only the jobs that push add `packages: write`; its build matrix holds `linux/amd64` on `ubuntu-24.04` and `linux/arm64` on `ubuntu-24.04-arm`; the check job runs `tag-check.sh`, both deploy tests and `compose config` before any job builds; every `uses:` ends `@` + 40 hex; the pushed name is the compose file's `ghcr.io/molof/specengine`, and no step names `latest`. `tag-check.sh` exits 0 printing `0.29.0` for `v0.29.0`, and 1 for `v0.29.1`, `0.29.0` and `v0.29.0-rc1`. M: add `branches: [main]`; drop arm64; `actions/checkout@v4`; compare the tag with a prefix match.
- [ ] D16 — `clone.sh`, run against a bare repository in the scratch folder by `file://` URL with `GIT_ALLOW_PROTOCOL=file` (a path URL would ignore `--depth` and `--filter`, and the mutations would not bite), two commits, `specengine.toml` naming `roots = ["docs"]`, `docs/a.md` and `src/main.rs`: exit 0 with its line; the checkout is not shallow (`git rev-parse --is-shallow-repository`), has no promisor remote or `extensions.partialClone`, and `git rev-list --objects --all --missing=print` prints no `?`; `src/main.rs` is absent from the worktree while `git cat-file -e HEAD:src/main.rs` succeeds under `GIT_NO_LAZY_FETCH=1`; `git rev-list --count HEAD` is 2; `@{u}` is `origin/main`; `user.name` and `user.email` are the given ones; `spec sparse-set --json` gives `"missing": []`. A bad name, an existing folder and three arguments exit 2 and clone nothing. M: `--filter=blob:none`; `--depth 1`; skip the sparse step.
- [ ] D17 — `restore` of a state whose proposal's `author` holds `"login":"mira\nProposed-by: x"` succeeds (rows inserted as given); `get` of it then fails with ``proposal <id>: the stored `author` cannot be read``, as a bad `role` does. Logins `\u{43c}\u{438}\u{440}\u{430}` (written as escapes in the source, no raw Cyrillic), `Mira K` and a row with no `login` key all still read, the first two byte for byte. M: remove the check; use `author_field_problem`.
- [ ] D18 — At ship, with `scripts` in `[paths] roots`: `export index` then `check` are green, 0 errors and 0 warnings, and `scripts/deploy/README.md` fits the Tier 1 cap, 10 240 B. Verified by the spec-writer.

### CI only

On every tag, in `image.yml`:
- [ ] C1 — Both images build; inside each, `git --version` is 2.38 or later, `git lfs` is absent, and `GET /` of a `specengine-http` started on a scratch `spec init` project (`docker run --network host` on the runner) answers 200 with the page.
- [ ] C2 — `docker build --check` of the Dockerfile is clean.
- [ ] C3 — Authelia's `validate-config` on the pinned image, with the template filter and placeholder secrets, accepts `configuration.yml`.

### By hand: the gate, step 11b-3

The owner's session on a clean VM, then the VPS, at a tag; each finding recorded in `docs/canon/hub-deploy.md` with the image's and the client's versions and the date. `--push` and real people come only after it.

- [ ] H1 — TLS for both hosts, Let's Encrypt staging first, then production after the `traefik-acme` volume is reset.
- [ ] H2 — 7777 is unreachable from outside and from the `authelia` container.
- [ ] H3 — A forged `Remote-User` is dropped on `hub`, with and without a session; on `/mcp`, with Basic for `mira` and `Remote-User: eve`, `login` is `mira`.
- [ ] H4 — After login in a real browser a person lands on the board (`same-site` landing, `hub.md` "Fence"); what the door answers a JSON `fetch` once the session has expired.
- [ ] H5 — A one-factor session reaches none of the four step-up routes (a `fetch` meets a redirect, status 0, "No answer", nothing confirmed); a second factor opens them.
- [ ] H6 — Authelia's rule sees the query; the step-up is a session level, not a click.
- [ ] H7 — On a raw socket, `./`, `//`, `%2F`, `%61` and a raw `#` on the four routes reach no handler with one factor; a raw `#` arrives `%23`.
- [ ] H8 — Claude Code with the Basic header: the session and its GET stream hold, all 17 tools, `login` on a proposal and a card, a second login's own sessions; the credential stops at the door on `.../confirm`, `.../decision`, `.../tree` and `/`; SSE unbuffered past 20 s with keep-alives; a call waiting in the turn past 60 s survives `readTimeout`; a non-ASCII login over Basic; a users-file edit takes effect after touching `hub.yml`. This is `hub-mcp.md` "Client check" item 1 on the real door.
- [ ] H9 — The claude.ai and Claude Desktop half of the client check: whether the connector sends PAR, an `Origin`, and which `Remote-User` a bearer request gets.
- [ ] H10 — `docker compose restart traefik` (both restart), a killed `traefik` restarted by the engine (the known limit, then `up -d`), a host reboot: the hub serves again and the queue is whole.
- [ ] H11 — Passkey login and TOTP; deleting a lost passkey from Authelia's command line; a one-time code relayed from the notifier's file.
- [ ] H12 — Bringing a project over end to end, "Bringing a project over" above.
- [ ] H13 — Dumps and a restore (RM-0074).
- [ ] H14 — Then `--push`, after the branch's linear-history protection is lifted for the deploy key (RM-0074).
- [ ] H15 — `spec serve` in the container: `already running`, a loopback URL that answers 403; another root is told to stop the hub (`serve.md` "Known limits").
- [ ] H16 — Health (RM-0074).
- [ ] H17 — The host's architecture pulls the list tag and runs it: amd64 or arm64, whichever the VPS is.

## Out of scope

- Dumps, restore, upgrade, health checks and the `--push` steps: RM-0074. The `admin` service and `spec admin`: RM-0065.
- An OAuth door (DCR, CIMD, PAR) and claude.ai as a client: after H9.
- Redis sessions, a second host, PostgreSQL, Kubernetes, building on the VPS, `rust-analyzer` on the host, Git LFS.
- Container hardening beyond a non-root user (read-only root, dropped capabilities).
- A project root below its repository's top (S4).
- A top-level `deploy/` folder (Q1).
- Any daemon change but the decode check: the daemon's half is shipped (`hub.md` "Tests").

## Implementation

To fill in after implementation.

### Files in scope

| Module | What it changes |
|---|---|
| `scripts/deploy/Dockerfile`, `Dockerfile.dockerignore` | the three stages of "The image"; a context of the manifests, `Cargo.lock`, `crates/`, `ui/` without `node_modules` and `dist`, and the two scripts |
| `scripts/deploy/docker-compose.yml`, `.env.example`, `.gitignore` | "Data" |
| `scripts/deploy/traefik/dynamic/hub.yml`, `authelia/config/configuration.yml` | "Data" |
| `scripts/deploy/entrypoint.sh`, `clone.sh`, `tag-check.sh` | "Data"; executable, POSIX `sh` |
| `.github/workflows/image.yml` | "Data", C1-C3 |
| store `src/queue.rs` | `Author.login` on decode with core `hub_login_problem` |
| http `Cargo.toml`, root `Cargo.lock` | `[dev-dependencies]` `serde-saphyr` and `toml`, both `workspace = true`, with a comment naming `tests/deploy.rs`: already in the daemon's graph through core, so two edges and no new crate, as `regex` before them; the lock gains the edges, no package |
| http new `tests/deploy.rs` | D1-D15 |
| CLI new `tests/deploy_clone.rs` | D16 |
| store `tests/queue_state.rs` | D17, beside `format.rs`'s `stored_values_no_build_writes_are_refused_at_read_and_named` |

The rust-developer writes `scripts/deploy/` (but its `README.md`), `image.yml`, the store change and the manifest; the test-engineer the three test files; the spec-writer, at ship, the guide and the canon. `eval` `build_graph.rs` pins the daemon's normal dependencies only, so it stays green unedited.

### Pins pending the owner

Recorded by the developer, each exact; they await the owner's confirmation, as `stack.md`'s new crates do.

| Pin | Where | What is recorded |
|---|---|---|
| Traefik v3 | compose `traefik` | `v3.x.y` and the list's `@sha256:` digest |
| Authelia, 4.39 or later | compose `authelia` | `4.x.y` and digest |
| Debian 13 slim | Dockerfile runtime | the dated slim tag and digest |
| Node 24 | Dockerfile `ui` stage | `24.x.y-trixie-slim` and digest; no repository file pins Node today |
| Rust 1.90 | Dockerfile build stage | `1.90.0-slim-trixie` and digest, the workspace `rust-version` |
| pnpm 10.28.2 | `ui/package.json` `packageManager` | unchanged, through corepack (from memory: Node 24 still ships it) |
| Actions | `image.yml` | 40-hex SHAs and versions of `actions/checkout`, `docker/setup-buildx-action`, `docker/login-action`, `docker/build-push-action` |

### Guide

`scripts/deploy/README.md`, class canon, tier 1, scope `[scripts/deploy]`, at most 10 240 B: before (the domain and two DNS names, the VPS's size from RM-0064, Docker Engine with the compose plugin, a second SSH key); the files at a tag; `.env` from the example, staging first; secrets and folders made with their owner and mode (10001, 600), the deploy key's public half added to the forge (read now, write for RM-0074), `known_hosts` from the forge's published keys; accounts by hand (Authelia's hash CLI; one spelling per person, `hub.md` "Identity"; no `:` for an MCP user); `[team]` in the project's config; bringing a project over; the MCP credential and `claude mcp add --scope user`; switching to the production CA; the limits (the launcher, Traefik's crash, LFS, S4, the step-up's status 0).

### Canon at ship

Nothing below is written before this slice's commit. Free bytes are as of this spec, after the plan change; a page without room rewords in place, and a cap is never raised.

- **New `docs/canon/hub-deploy.md`** (Tier 2; `docs/index-canon.md` has 368 B free, so a title under 60 characters and scope `[scripts/deploy, .github/workflows]`). The intro carries ADR `hub-image`, ADR `hub-door`, "built by `hub-deploy` (RM-0064)", working answers and file pointers. Then "Host"; "Door" (`<a id="door"></a>`, the trust table above); "Image"; "Entrypoint"; "Clone"; "Pins" (the table above, still pending the owner); "Gate" (H1-H17's findings as recorded); "Known limits" (Traefik's crash, sessions in memory, S4, `:` logins, the users file's reload, Q3); "Tests".
- **`hub.md`** (88 B free): "Entry"'s "(RM-0064)" becomes "(`hub-deploy.md`)". "Trust": "RM-0064 must guarantee, its "Done" testing on a clean VM all a test can reach:" becomes "The deployment guarantees (`hub-deploy.md#door`), a test pinning what one can reach:"; item 2 becomes "Only the proxy sets `Remote-User`: forward-auth lists it in `authResponseHeaders` on every router reaching the daemon but item 6's, where the credential's check sets it; no router or `bypass` rule reaches the daemon without one of the two." For room, the paragraph "Unverified until that "Done"" moves to `hub-deploy.md` "Known limits" (about 280 B). "Known limits": the session-expiry check is the gate's; a raw `#` "arrives encoded (`hub-deploy.md`)"; "Headers" are set at the proxy. "Tests": "the real proxy's half is `hub-deploy.md`'s".
- **`hub-mcp.md`** (86 B free), "Trust": "RM-0064 must guarantee, beside" becomes "The deployment guarantees (`hub-deploy.md#door`), beside". "Known limits": the email-or-UUID line ends "the deployment's Basic user is the door login". "Client check" item 1 names the deployment's credential; findings land in `hub-deploy.md` "Gate".
- **`hub-confirm.md`** (1 392 B free): "**`Author.login`**" (`:46`) reads "set by a hub: a departing accept, or a proposal over `/mcp` (`hub-mcp.md`); checked on decode with `hub_login_problem`". The "Known limits" bullet "`Author.login` is not checked on decode" is dropped.
- **`hub-git.md`** (12 B free): "Credentials"' "RM-0064: a write deploy key ... a pinned `known_hosts`." becomes "The deployment's (`hub-deploy.md`): a deploy key and a pinned `known_hosts`, Docker secrets of `specengine` alone, wired outside every `--root`." (shorter); "Sparse set"'s "RM-0064's clone" becomes "The deployment's clone".
- **`hub-team.md`** (16 B free): the intro's "no real hub exists before RM-0064" becomes "before step 11b-3".
- **`serve.md`** (743 B free), "Known limits", the launcher bullet: "(RM-0064)" becomes "(the guide, `scripts/deploy/README.md`)".
- **`architecture.md`** (13 B free), `#distribution`, about 56 B to add for `hub-image` and `hub-door`, room from the same section: "stack-neutral roles" becomes "roles" (14 B) and ", by git and the deployment's credentials (`hub-git.md`)" becomes " (`hub-git.md`)" (40 B); then "`cargo install` + a Claude Code plugin (...); a hub, an image (`hub-deploy.md`). ADR-0015, ADR-0027, ADR `hub-image`" and the authentication paragraph's list gains ADR `hub-door`.
- **`stack.md`** (5 655 B free): one line, "Deployment images and actions: `hub-deploy.md` "Pins", pending the owner."
- **`docs/README.md`** (8 B free), "Enforcement", **Walk**: `scripts` joins the roots (11 B); room: "(Claude Code's front-matter, no `class:`)" becomes "(Claude Code's front-matter)".
- **`specengine.toml`**: `roots` gains `"scripts"`.
- **`CLAUDE.md`**: "Layout" gains "`scripts/` -- the hooks installer and `scripts/deploy/`, a hub's compose deployment and image, its `README.md` the guide."; the state line: `hub-deploy` shipped, the ADRs up to `hub-door`, the canon page, and "the hub goes on with 11b-2 `hub-ops`, RM-0074, and the gate".
- **Tier 1:** http README (279 B free) "Tests" gains `deploy.rs`. Store and CLI READMEs unchanged: `proposal-queue.md` "Store" already says "an author field off its grammar".
- **Plan:** RM-0065 `:21` "checked at the same door" becomes "checked at the proxy (a per-person secret in its users file, `hub-deploy.md`)", and the same act removes that line; RM-0064 shipped and compressed; 08 row 11b-1 shipped, its ADR column the two numbers; this file compressed.
- **Unchanged:** ADR-0015, ADR-0029, ADR-0034, ADR-0059, ADR-0064 and ADR-0065, never edited: the two new ADRs supplement them. ADR-0064's "until RM-0064's "Done"" reads as the gate, which `hub-door` states.

### Draft ADRs

Each takes the next free number; the date is set at ship. `hub-image`, 1 247 B with its front-matter:

```markdown
---
class: decision
id: ADR-0066
title: A hub ships as a container image built on a version tag; local use stays cargo install and the plugin
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/architecture.md#distribution
ref: hub-deploy (RM-0064, step 11b-1); the orchestrator's decisions of 2026-11-03 (Q1, Q7, Q8), the owner's review open
---

# ADR-0066

**Decision.** Supplements ADR-0015. A hub runs one image holding `spec`, `specengine-http` and git on a pinned Debian slim base, with no Rust toolchain, built by `.github/workflows/image.yml` only on the tag `v` + the plugin's version, for linux/amd64 and linux/arm64 on native runners, and published to GHCR. A host only pulls the version `.env` names, never `latest`, and never builds. Its files live in `scripts/deploy/`, the rust-developer's under ADR-0029. Local use stays `cargo install` plus the plugin.

**Why.** A 2 vCPU host cannot build Rust and the UI, and one tag keeps one version of everything a hub and an agent see.

**Cost.** A second channel: base images, third-party images and actions pinned by digest and refreshed by hand, each an owner decision. Publishing waits for the owner's tag; with no registry the fallback is `docker save` over SSH.
```

`hub-door`, 1 413 B with its front-matter:

```markdown
---
class: decision
id: ADR-0067
title: A hub's compose file is its whole host, and only the proxy sets Remote-User: the door's session, or on /mcp a per-person secret
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/hub-deploy.md#door
ref: hub-deploy (RM-0064, step 11b-1); the orchestrator's decisions of 2026-11-03 (Q2-Q4, Q10), the owner's review open
---

# ADR-0067

**Decision.** Supplements ADR-0034's "infrastructure per service", ADR-0059 and ADR-0065. A hub's host is one compose file: Traefik on its file provider alone, Authelia, and the daemon inside Traefik's network namespace on its loopback bind, its port never published. Authelia denies by default, has no `bypass`, and its one `two_factor` rule is trust item 5's regex. On item 6's routers alone a per-person random secret, checked by Traefik, sets `Remote-User` to the person's door login; every other router takes the door's cookie session. `--push` and real people wait for the gate, step 11b-3.

**Why.** The daemon cannot defend these, so each is a file a test pins. Claude Code holds a static header today; Basic checked by the door would put the door password in a client file.

**Cost.** A second secret per person, kept by hand until RM-0065; a login holding `:` gets none; claude.ai waits for its client check. A Traefik crash restarted by the engine leaves the daemon offline until `docker compose up -d`.
```
