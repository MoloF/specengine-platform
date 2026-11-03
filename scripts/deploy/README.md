---
class: canon
tier: 1
scope: [scripts/deploy]
owner: owner
reviewed: 2026-11-03
---

# scripts/deploy -- a hub on one host

The operator's guide: from a clean Linux VPS with Docker to a running hub. How the files guarantee the hub's trust, and why: `docs/canon/hub-deploy.md`; the hub itself: `docs/canon/hub.md`. Dumps, restore, upgrades, health and `--push` are not here yet (RM-0074): until then a hub has no off-host backup and pushes nothing.

## Before you start

- **A domain for the hub alone**, final before anyone enrols: passkeys bind to it. Two names on it point at the VPS (A/AAAA), e.g. `hub.example.org` and `login.example.org`; nothing else lives on that domain, since the door's cookie covers all of it.
- **A VPS**: at least 2 vCPU, 4 GB of memory, 40-80 GB of local SSD; Docker Engine with the compose plugin; nothing else running. Keep a second SSH key and the provider's console: losing SSH is not this guide's to fix.
- **The image** is `ghcr.io/molof/specengine:<version>`, pulled, never built here. If the GHCR package is private, log in first with a read-only token (`docker login ghcr.io`), or have the owner make it public.

## Install

```sh
git clone --branch v0.29.0 https://github.com/MoloF/specengine-platform
cd specengine-platform/scripts/deploy
cp .env.example .env            # set the three names and ACME_EMAIL; keep the staging CA
install -d -m 700 secrets
sudo install -d -o 10001 -g 10001 projects
install -d authelia/users traefik/mcp-users
```

`.env` names the version the host runs (`SPECENGINE_VERSION`, the tag without `v`). Leave `SPECENGINE_PUSH=0`.

**Secrets**, each a file in `secrets/`, mode 600:

```sh
for s in authelia_session_secret authelia_storage_key authelia_jwt_secret; do
  openssl rand -hex 32 > secrets/$s
done
ssh-keygen -t ed25519 -N '' -C specengine-hub -f secrets/deploy_key
chmod 600 secrets/*
sudo chown 10001:10001 secrets/deploy_key
```

- Add `secrets/deploy_key.pub` to the project's repository as a deploy key, read-only for now (write comes with `--push`, RM-0074). Then delete the `.pub` file or keep it outside `secrets/`.
- `secrets/known_hosts` holds the forge's published SSH host keys, checked against the fingerprints it documents: `ssh-keyscan` alone trusts the network. `chown 10001:10001` it too.
- `authelia_storage_key` encrypts the door's database: lose it and every account is lost with it.

The container user is 10001: if a mounted file or folder is not theirs, git refuses a "dubious ownership" and ssh a key file's mode.

## Accounts, by hand until RM-0065

A login is the person: recorded verbatim, compared exactly (`docs/canon/hub.md` "Identity"). Give each person one spelling, used everywhere: no surrounding space, no comma, no invisible character, never `local`, `agent` or `-`, and no Latin look-alike or second normal form, which only you can catch.

`authelia/users/users.yml`:

```yaml
users:
  mira:                         # the login
    displayname: "Mira K"
    email: mira@example.org
    password: "<hash>"
```

The hash comes from Authelia's own CLI: `docker compose run --rm --no-deps authelia authelia crypto hash generate argon2 --password '<password>'`. Authelia re-reads the file. Leads and the owner also need an entry in the project's `[team]` (`docs/canon/hub-team.md` "Team"), a reviewed commit.

A second factor is enrolled at the first step-up. Its one-time code lands in a file, which you relay to the person: `docker compose exec authelia cat /var/lib/authelia/notification.txt`. Passwords cannot be reset by mail: a lost password or passkey is yours to replace.

## AI over MCP: Claude Code

Each person who wants AI on the hub gets a random secret of their own, checked by the proxy on `/mcp` alone:

```sh
secret=$(openssl rand -hex 32)
docker compose run --rm --no-deps authelia authelia crypto hash generate bcrypt --password "$secret"
# append  mira:<that hash>  to traefik/mcp-users/users, then:
touch traefik/dynamic/hub.yml   # the proxy re-reads the users file
```

Give `mira` the secret over a private channel. On their machine:

```sh
claude mcp add --transport http --scope user hub-<slug> https://hub.example.org/mcp/<slug> \
  --header "Authorization: Basic $(printf 'mira:%s' '<secret>' | base64)"
```

- `--scope user` keeps the secret in the person's own settings. **Never** put it in a project's `.mcp.json`: that file is committed, and the secret goes with it.
- The user name must be the person's door login, byte for byte. A login holding `:` cannot be a Basic user name: that person gets no MCP credential.
- claude.ai and Claude Desktop are not checked yet: until then writers without Claude Code use the web UI.

To revoke, delete the person's line and touch `hub.yml` again.

## Bring a project over

1. On the laptop, decide every open proposal first: paths are never rewritten, so an open one arrives an orphan, only rejectable. Then `spec export state > <name>.jsonl`.
2. Clone inside the image, so the container's git, user and deploy key make the checkout:

   ```sh
   docker compose run --rm --entrypoint specengine-clone specengine \
     git@<forge>:<owner>/<repo>.git <name> "SpecEngine hub" hub@example.org
   ```

   `<name>` is `[a-z0-9][a-z0-9-]*`. The clone holds every blob and commit, sparse to the paths `spec sparse-set` prints, tracking `origin/<branch>`, with that committer identity for the hub's merge commits. The project's `specengine.toml` must sit at its repository's top. A failure names the folder to remove.
3. `docker compose up -d`, or `docker compose restart specengine` for a later project: the entrypoint serves the folders it finds at start.
4. Copy the dump to `projects/<name>.jsonl`, readable by 10001, then restore it into the empty queue and delete it:

   ```sh
   docker compose exec -it specengine spec import-state --root /projects/<name> /projects/<name>.jsonl
   ```

   It asks on the terminal; answer `y`. Do it before anyone logs in: it restores only into an empty queue.

## Going live

- **TLS.** The staging CA gives untrusted certificates: check the whole flow with them. Then set `ACME_CA_SERVER=https://acme-v02.api.letsencrypt.org/directory`, `docker compose down`, remove the `traefik-acme` volume, `docker compose up -d`.
- **The gate.** Before real people, the owner tries the hub by hand (step 11b-3, `docs/features/hub-deploy.md` H1-H17). `--push` waits for it.

## Limits

- **Never run `spec serve` here.** In the container it attaches to the hub: `already running` and a loopback URL that answers 403; another root is told to stop the hub.
- **A Traefik crash** that the engine restarts leaves the daemon offline: run `docker compose up -d`. `docker compose restart traefik` restarts both.
- **Sessions live in memory**: restarting `authelia` logs everyone out.
- **Git LFS** content in the served paths is unsupported.
- **A one-factor confirm** in the page reads `No answer`: the person completes the second factor on the door's page, then reads again.
- Authelia prints `chown` errors at start, because its config is mounted read-only: harmless.
