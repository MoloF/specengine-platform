---
class: canon
tier: 1
scope: [scripts/deploy]
owner: owner
reviewed: 2026-11-22
---

# scripts/deploy -- a hub on one host

The operator's guide: from a clean Linux VPS with Docker to a running hub. Why the files are trusted: `docs/canon/hub-deploy.md`; the hub: `docs/canon/hub.md`; dumps, restore, upgrades, health, `--push`: `docs/canon/hub-ops.md`.

## Before you start

- **A domain for the hub alone**, final before anyone enrols: passkeys bind to it. Two names on it point at the VPS (A/AAAA), e.g. `hub.example.org` and `login.example.org`; nothing else lives on that domain, since the door's cookie covers all of it.
- **A VPS**: at least 2 vCPU, 4 GB of memory, 40-80 GB of local SSD; Docker Engine with the compose plugin; nothing else running. Keep a second SSH key and the provider's console.
- **The image** is `ghcr.io/molof/specengine:<version>`, pulled, never built here. If private: `docker login ghcr.io` with a read-only token, or make it public.

## Install

```sh
git clone --branch v0.52.1 https://github.com/MoloF/specengine-platform
cd specengine-platform/scripts/deploy
cp .env.example .env            # set the three names and ACME_EMAIL; keep the staging CA
install -d -m 700 secrets authelia/users traefik/mcp-users admin-log
sudo install -d -o 10001 -g 10001 projects
```

`.env` names the version the host runs (`SPECENGINE_VERSION`, the tag without `v`). Leave `SPECENGINE_PUSH=0`.

**Secrets**, each a file in `secrets/`, mode 600:

```sh
for s in authelia_session_secret authelia_storage_key authelia_jwt_secret restic_password; do
  openssl rand -hex 32 > secrets/$s
done
touch secrets/restic_env
ssh-keygen -t ed25519 -N '' -C specengine-hub -f secrets/deploy_key
chmod 600 secrets/*
sudo chown 10001:10001 secrets/deploy_key
```

- Add `secrets/deploy_key.pub` to the project's repository as a deploy key, read-only for now (write comes with `--push`: `docs/canon/hub-ops.md` "Push"). Then delete or move the `.pub` file out of `secrets/`.
- `secrets/known_hosts` holds the forge's published SSH host keys, checked against the fingerprints it documents: `ssh-keyscan` alone trusts the network. `chown 10001:10001` it too.
- `authelia_storage_key` encrypts the door's database: lose it and every account is lost with it.
- **Off-host backups** stay off until the owner turns them on ("Going live"): with `restic_env` empty, `backup` waits, unhealthy, its line(s) each round.
- **References' files** live on `specengine-blobs`, never dumped or backed up: lost with the host (ADR-0088). Never remove it; from 0.50.x: `docs/canon/hub-ops.md` "Upgrade".

The container user is 10001: if a mounted file or folder is not theirs, git refuses a "dubious ownership" and ssh a key file's mode.

**Your account, before anything starts.** `up -d` never starts `admin`, and the door's users folder is read-only to it, so no account exists that you did not add:

```sh
docker compose run --rm admin add <you> --name "<Your Name>" --email <you@example.org>
```

Keep the printed password, then bring a project over: its step 3 is the first `up -d`.

## Accounts

`docker compose run --rm admin <act>`: a one-shot container, no network or log, on the door's image; root is your shell. Acts that write, and `code`, need a terminal: a piped `run -T` is refused. One act at a time; each is logged in `admin-log/acts.log`, never a secret.

| Act | Does |
|---|---|
| `add <login> --name "<name>" --email <address>` | a new account; its password once, and its `[team]` row |
| `disable <login>` | refused from its next request; its MCP line removed |
| `enable <login>` | allowed again, with no MCP secret |
| `reset <login>` | a new password, once; passkeys and TOTP deleted |
| `mcp rotate <login>`, `mcp revoke <login>` | a new MCP secret; none |
| `code`, `list`, `log` | the door's last one-time code; accounts and orphan MCP lines; the act log |
| `check` | `[team]` against the accounts, below |

A login is the person: recorded verbatim, compared exactly (`docs/canon/hub.md` "Identity"). A new one is lower case, digits, `.`, `_` or `-`, a letter first, at most 64 bytes; a name may be any script.

- **A new person.** `add`; relay the password privately. The door sends no mail: registering a TOTP or passkey (a first step-up, or its settings page) takes a one-time code `code` shows; relay it, one person at a time. Leads and the owner also need the printed `[team]` row in the project's `specengine.toml` (`docs/canon/hub-team.md` "Team"), a reviewed commit. Until `--push`, bring it in by hand: `docker compose exec specengine git -C /projects/<name> pull --no-rebase`, then `docker compose exec specengine spec index --root /projects/<name>`. Then `docker compose exec -T specengine spec team --root /projects/<name> | docker compose run --rm -T admin check` names a `[team]` login with no account, a disabled one, or a `:`.
- **A lost passkey or password.** `reset`, relay the password; the person enrols again.
- **Someone leaves.** `disable`: the door refuses them at their next request, and their AI's next call fails.
- **A compromised account.** `disable`; `docker compose restart authelia` (sessions live in memory: everyone logs in again); `reset`; `enable` only when `reset` ended without `passkeys-left`. A passkey left signs in alone: reset again.
- **Everything lost.** With every account disabled, `authelia` stopped even: `enable` and `reset` your own, then `docker compose up -d`.

## AI over MCP: Claude Code

Each person who wants AI on the hub gets a secret of their own, checked by the proxy on `/mcp` alone: `docker compose run --rm admin mcp rotate mira` prints it once, with the line to run on their machine:

```sh
claude mcp add --transport http --scope user hub-<slug> https://hub.example.org/mcp/<slug> \
  --header "Authorization: Basic <printed>"
```

- Relay it privately. `--scope user` keeps it in the person's own settings. **Never** put it in a project's `.mcp.json`: that file is committed.
- `mcp revoke` or `disable` removes it, and the proxy reads its users file again.
- A login holding `:` gets no secret.
- claude.ai and Claude Desktop are not checked yet: until then writers without Claude Code use the web UI.

## Bring a project over

1. On the laptop, decide every open proposal first: paths are never rewritten, so an open one arrives an orphan, only rejectable. Then `spec export state > <name>.jsonl`.
2. Clone inside the image, so the container's git, user and deploy key make the checkout:

   ```sh
   docker compose run --rm --entrypoint specengine-clone specengine \
     git@<forge>:<owner>/<repo>.git <name> "SpecEngine hub" hub@example.org
   ```

   `<name>` is `[a-z0-9][a-z0-9-]*`; the identity signs the hub's merge commits. The project's `specengine.toml` must sit at its repository's top. A failure names the folder to remove.
3. `docker compose up -d`, or `docker compose restart specengine` for a later project: the entrypoint serves the folders it finds at start.
4. Copy the dump to `projects/<name>.jsonl`, readable by 10001, then restore it into the empty queue and delete it:

   ```sh
   docker compose exec -it specengine spec import-state --root /projects/<name> /projects/<name>.jsonl
   ```

   It asks on the terminal; answer `y`. Do it before anyone logs in: it restores only into an empty queue.

## Going live

- **TLS.** The staging CA gives untrusted certificates: check the whole flow with them. Then set `ACME_CA_SERVER=https://acme-v02.api.letsencrypt.org/directory`, `docker compose down`, remove the `traefik-acme` volume, `docker compose up -d`.
- **Running.** `docker compose ps`: `specengine` healthy, `backup` once backups are on; else `docker compose logs dump-queue dump-door backup` say why. Runbooks: `docs/canon/hub-ops.md` "Restore", "Upgrade", "Push".
- **Slow pages.** `docker compose logs -t --since 2h specengine | grep ' slow '`, then `traefik`'s: `docs/canon/serve-slow.md` "Reading them"; every line: `SPECENGINE_SLOW_MS=0` in `.env`, `up -d`, later removed.
- **Off-host backups**, the owner's step: choose a restic backend; write `RESTIC_REPOSITORY` and its keys into `secrets/restic_env` (`docs/canon/hub-restic.md`) in place, or `docker compose restart backup` after an editor that saves by rename (the secret is a bind mount of the old inode); keep it, `restic_password` and `.env` off the host too; `docker compose run --rm backup init` once. After a round, `docker compose run --rm backup snapshots` lists `queue` and `door` under 35 minutes old and `backup` is healthy; then a restore drill on a scratch host (`hub-ops.md` "Restore").
- **The gate.** Before real people, the owner tries the hub by hand (step 11b-3, `docs/features/hub-deploy.md` H1-H17). `--push` waits for it.

## Before the gate

`hub-admin` and `hub-ops` were tested on stand-ins; the door's storage calls on its pinned image, 2026-11-20 (`docs/features/hub-backups.md` HB5). Before real people, on the pinned images: VF1-VF3 of `docs/features/hub-accounts.md` and VF4-VF9 of `docs/features/hub-ops.md`.

## Limits

- **Hand changes.** After changing a project's files on the host by hand (a `git pull`, an edit, a restore after a stopped sync), run `docker compose exec specengine spec index --root /projects/<name>`; else the hub shows it within 5 minutes of reads (`docs/canon/spec-read-freshness.md`).
- **Developers' `spec`.** Before the hub records a decision or creates a node, everyone who commits to its repositories runs this release or later (`docs/canon/spec-ids.md`).
- **Never run `spec serve` here**: in the container it attaches to the hub, a loopback URL answering 403.
- **A Traefik crash** the engine restarts leaves the daemon offline, `unhealthy`: run `docker compose up -d`; `restart traefik` restarts both.
- **Git LFS** content in the served paths is unsupported.
- **A one-factor session** on a step-up act gets a notice linking the door: add the second factor there, then Retry.
- **A failed stamp:** restart `traefik` as told; a second `disable` of that login answers `unchanged`. A `mcp rotate` stopped before it showed the secret: run it again. A killed act can leave a `.tmp` beside a users file: delete it.
- **A hand-made users value** with a YAML escape but `\"` and `\\` is shown as written.
