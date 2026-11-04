---
class: spec
status: draft
scope: [scripts/deploy, .github/workflows, crates/specengine-http, docs]
ref: RM-0074; Phase 7 step 11b-2 (08 s2), the last code item of release v3; the 11b and 11c analysis notes of 2026-11-03; the orchestrator's working answer to Q5 (restic) of 2026-11-04 and S1-S12 below, the owner's review open
adrs: []
---

# hub-ops: off-host dumps by restic, restore, upgrade, health and turning on `--push` (RM-0074)

## Why

After 11b-1 and 11c-1 a hub runs, but its queue and its accounts live on one VPS. Losing the VPS loses every open proposal, card and task, and makes everyone enrol again. A hung daemon shows nothing. An upgrade has no order, and `--push` no procedure. This step adds the hub's operations: what leaves the host, when, and how it comes back; how a release moves; how health shows; how the hub starts pushing to the game's main. After it comes only the owner's gate 11b-3: HO1-HO17 below join H1-H17 (`hub-deploy.md`), G1-G10 (`hub-accounts.md`) and SG1-SG2 (`stream-life.md`).

No crate changes. The dumps use `spec export state` and `spec import-state` (`docs/canon/queue-backup.md`), the door's own CLI and restic, each in a container of its own image; the probe is a shell's. The daemon gains no client, route or flag. No change to `docs/canon/architecture.md`, no network verb in a crate, no Docker socket.

### Decisions

The orchestrator's working answer (Q5: restic) and this spec's settlements, each awaiting the owner's review. The runbooks and formats they lead to are in "Data".

- **S1. The destination: restic,** to any backend it reaches through environment credentials: S3-compatible stores, B2, Azure, GCS, Swift, a REST server. `secrets/restic_env` holds `RESTIC_REPOSITORY` and the backend's keys, `secrets/restic_password` the repository password, both Docker secrets of `backup` alone. This departs from Q5's `.env` on purpose: `.env` holds no key material (`docs/canon/hub-deploy.md` "Host", D10), and a REST or S3 URL may carry a password, so `.env` gains no key. Encryption is restic's own, client-side, so every dump, the door's included, is encrypted at rest off the host. SFTP and rclone are out: each needs a key file or a config file beyond the environment.
- **S2. Three sidecars, each on the image whose tool it runs.** `dump-queue` runs on the hub's image (`spec export state`) and `dump-door` on the door's (`hub-admin dump`), both with `network_mode: none`. `backup`, on restic's image, is the only one with a network, its own, and it mounts no live data. Rejected:
  - (a) a loop in `specengine-entrypoint`: a second lifecycle in the daemon's container that no crash restarts; D12 would change; restic would still need a container.
  - (b) one sidecar with restic and `sqlite3` added to the hub's image: the networked container would mount the live queue and the door.

  No `docker exec`, no socket.
- **S3. The door: Authelia's own exports, under `hub-admin`'s lock.** No pinned image has `sqlite3` (from memory, VF4), and a file copy under a running door is never taken. `hub-admin dump DIR` (new) copies `users.yml`, the MCP users file and `acts.log`, and runs the CLI's passkey and TOTP exports. Options:
  - (a) the exports: recommended. They do not depend on the storage key and survive door upgrades.
  - (b) `sqlite3 .backup` on an image that gains it, with the storage key dumped too: the fallback if VF4 finds no export.
  - (c) `VACUUM INTO` through a `spec` subcommand: rejected, the door's data in a crate (ADR-0068).

  **The storage key needs no restore:** `hub-admin restore` imports the exports under the new host's key. Lost on purpose: preferences, the authentication log and bans. The session and JWT secrets are made anew.
- **S4. The deploy key is never dumped.** It lives in `secrets/deploy_key` alone. A restore makes a new one and deletes the lost host's key at the forge. `known_hosts` is made again from the forge's published fingerprints.
- **S5. The schedule:** a POSIX sh loop per sidecar on a 15-minute UTC grid, with a round at start, `restart: unless-stopped` and `init: true`. A loop keeps no state but the clock and the generation names, so a restart costs at most one round. Staging is the tmpfs volume `hub-dumps`, so no dump touches the host's disk.
- **S6. Retention:** two series per hub, `--host <HUB_HOST>` with tags `queue` and `door`, so a failing part never blocks the other. The 03:05 UTC round runs `forget --keep-within 48h --keep-daily 30` per tag and one `prune`, so a reset person's old digests and TOTP stay that long. `init` and `check` are the operator's acts; nothing initialises a repository by itself.
- **S7. Health.**
  - **The probe:** `specengine-health`, bash copied into the image. A loopback `GET /` must be answered `HTTP/1.1 403` (the fence refuses a loopback `Host`: `docs/canon/hub.md` "Fence", step 1), and `127.0.0.1:443` must accept a connection. The second test turns the dead namespace a Traefik crash leaves into `unhealthy`. No identity, no new route.
  - **Tool options:**
    - (a) bash's `/dev/tcp`, Debian's bash being Essential: recommended. It is the one non-POSIX script, since POSIX sh has no socket.
    - (b) a `spec` subcommand: rejected, a network verb in a crate.
    - (c) curl or wget: rejected, an HTTP client in the image.
    - (d) git's `remote-http`: a misuse.
  - **The chain:** `backup`'s healthcheck, `hub-ops fresh`.
  - **From outside:** `https://<DOOR_HOST>/` answering 200, which sees neither the daemon nor the dumps.
  - Docker restarts no unhealthy container: `unhealthy` is the operator's signal.
- **S8. Restore** into an empty door (`hub-admin restore`, new, run before the door's first start) and empty queues (`spec import-state`). The dumpers start last, so no empty state becomes the newest snapshot.
- **S9. Upgrade:** a dump first. Queue migrations run forward only, so the way back is that dump, restored on the old version.
- **S10. `--push`, after the gate:** a new write key, the forge's rules lifted for it, `SPECENGINE_PUSH=1`. Merge commits go to the game's main (the owner's decision of 2026-11-03).
- **S11. The release stays `v0.29.1`.** No tag is published yet: the plugin went to 0.29.1 in RM-0073 only to fix the first release's version. The hub's first tag, `v0.29.1`, is cut after this slice ships and carries its files, so the gate's host gets them at a tag (08 s2, "The hub's gate"). No version changes: `plugin.json`, `.env.example`, `tag-check.sh` and the guide's clone tag stay at 0.29.1.
- **S12. The canon: `docs/canon/hub-ops.md`, not `scripts/deploy/OPERATIONS.md`.** The check would accept the latter: `tier-invalid` binds tiers 0 and 1 alone, and the root `README.md` is a Tier 2 page outside `docs/canon/`. But no role may write it: outside `docs/`, the spec-writer writes only `*/README.md` files, and the developer writes no documentation (`CLAUDE.md` "Process"). The page needs an index shard of its own ("Canon at ship").

## Description and interactions

**Install, two steps beyond the guide's.** Put `secrets/restic_password` beside the other secrets and `secrets/restic_env` naming the backend. Run `docker compose run --rm backup init` once. Keep three copies off the host: `.env`, `restic_password` and `restic_env`. Without the last two the dumps cannot be read, and nothing on the host can replace them.

**Running.** `up -d` starts six services: `traefik`, `authelia`, `specengine`, `dump-queue`, `dump-door` and `backup`; `admin` stays one-shot. Every 15 minutes each dumper writes a complete generation into `hub-dumps`. Five minutes later `backup` sends the newest of each to restic, then marks the round whole or not. `docker compose ps` shows `specengine` and `backup` healthy or unhealthy; `docker compose logs dump-queue dump-door backup` say why.

**Acts.** `docker compose run --rm backup <act>` (`init`, `snapshots`, `restore`, `check`) and `docker compose run --rm admin restore DIR` are run from the host's shell; restore, upgrade and push follow the runbooks in "Data". The procedures go into `docs/canon/hub-ops.md` at ship; the guide `scripts/deploy/README.md` points there.

**Interactions:**
- `queue-backup.md`:
  - "Format": format 3 dumps.
  - "Commands": export needs no terminal, import does.
  - "Import": import goes into an empty queue alone.
  - "ID floor" and "Known limits" R1: a restored queue issues again the IDs minted after its dump, and a `--push` restore raises the floor above the remote's.
- `spec-ids.md` "Older binaries": every writer runs 0.29.1 or later before the hub's first spec-ID mint (ADR-0070).
- `spec-read-freshness.md` "A hub's trusted index": a restored or upgraded hub's first read per project walks (R8); a `spec index` before the DNS switch makes it a stat walk.
- `hub-git.md`: "Credentials" and "Rules" (`--push` stays off until the gate).
- `hub-deploy.md#accounts`: the lock and the act log.
- `hub.md` "Fence": the probe's 403.

## Data

The files are the truth; the canon and the guide describe them.

### Files

| Path | New or changed |
|---|---|
| `scripts/deploy/docker-compose.yml` | three services, two healthchecks, a network, two volumes, two secrets |
| `scripts/deploy/ops/hub-ops` | new, POSIX sh: the loops and `backup`'s acts |
| `scripts/deploy/health.sh` | new, bash: `specengine-health` |
| `scripts/deploy/admin/hub-admin` | acts `dump` and `restore`; the lock's message; the usage |
| `scripts/deploy/Dockerfile`, `Dockerfile.dockerignore` | the probe copied in, allowlisted |
| `scripts/deploy/.gitignore` | `restore/` |
| `.github/workflows/image.yml` | `--test deploy_ops`; C4, C5 |

### Compose

Additions, with every other line unchanged. The new image is pinned as 11b-1's are: the exact version and the multi-arch index digest, recorded from Docker Hub on the day, pending the owner. It is restic's current release, at least 0.17.

```yaml
services:
  specengine:
    healthcheck:
      test: ["CMD", "specengine-health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 120s
  dump-queue:
    image: ghcr.io/molof/specengine:${SPECENGINE_VERSION:?set SPECENGINE_VERSION in .env}
    restart: unless-stopped
    init: true
    network_mode: none
    entrypoint: ["/bin/sh", "/opt/hub-ops/hub-ops"]
    command: ["queue"]
    volumes:
      - ./ops:/opt/hub-ops:ro
      - ./projects:/projects:ro
      - specengine-data:/data
      - hub-dumps:/dumps
  dump-door:
    image: authelia/authelia:4.39.28@sha256:<authelia's, byte for byte>
    restart: unless-stopped
    init: true
    network_mode: none
    entrypoint: ["/bin/sh", "/opt/hub-ops/hub-ops"]
    command: ["door"]
    environment:
      AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE: /run/secrets/authelia_storage_key
    secrets: [authelia_storage_key]
    volumes:
      - ./ops:/opt/hub-ops:ro
      - ./admin:/opt/hub-admin:ro
      - ./admin-log:/var/lib/hub-admin
      - ./authelia/users:/users:ro
      - ./traefik/mcp-users:/mcp-users:ro
      - authelia-data:/var/lib/authelia
      - hub-dumps:/dumps
  backup:
    image: restic/restic:<version>@sha256:<index digest>
    restart: unless-stopped
    init: true
    hostname: hub-backup          # restic's lock host: a stale lock is its own after a restart
    networks: [backup]
    entrypoint: ["/bin/sh", "/opt/hub-ops/hub-ops"]
    command: ["upload"]
    environment:
      HUB_HOST: "${HUB_HOST}"
      RESTIC_PASSWORD_FILE: /run/secrets/restic_password
      RESTIC_CACHE_DIR: /cache
    secrets: [restic_password, restic_env]
    tmpfs: [/snapshot, /state]
    volumes:
      - ./ops:/opt/hub-ops:ro
      - hub-dumps:/dumps:ro
      - restic-cache:/cache
    healthcheck:
      test: ["CMD", "/bin/sh", "/opt/hub-ops/hub-ops", "fresh"]
      interval: 5m
      timeout: 10s
      retries: 1
      start_period: 20m
networks:
  backup: {}
secrets:
  restic_password: {file: ./secrets/restic_password}
  restic_env: {file: ./secrets/restic_env}
volumes:
  hub-dumps: {driver: local, driver_opts: {type: tmpfs, device: tmpfs, o: "size=256m,uid=10001,mode=0700"}}
  restic-cache: {}
```

`dump-queue` mounts `/data` read-write because a reader of a WAL database maps its `-shm` file (`crates/specengine-store/README.md`); it writes nothing else there. The healthcheck runs as the container's user.

### Secrets

- `secrets/restic_password`: `openssl rand -hex 32`, mode 600, as the guide makes the others.
- `secrets/restic_env`, mode 600: one `KEY=value` per line, LF-ended. Lines that are empty or start with `#` are skipped. `KEY` is `RESTIC_REPOSITORY` (required, once), `RESTIC_REST_USERNAME`, `RESTIC_REST_PASSWORD`, or a name starting `AWS_`, `B2_`, `AZURE_`, `GOOGLE_`, `OS_` or `ST_`, of `[A-Z0-9_]`. The value is the rest of the line, verbatim: no quotes removed, no expansion, never sourced as shell. Any other line refuses the run with exit 2, naming its line number and key and never its value: an `export`, a lower-case key, `PATH`, `LD_*`, any `RESTIC_PASSWORD*`, a line without `=`.

```
# the hub's dumps, an S3-compatible store
RESTIC_REPOSITORY=s3:https://s3.example.org/hub-dumps
AWS_ACCESS_KEY_ID=<key id>
AWS_SECRET_ACCESS_KEY=<key>
```

### Staging, snapshots, restore

```
/dumps/queue/20261104T121500Z/        dir 700 (10001); a complete generation
    a.jsonl  a-1.jsonl                600; spec export state, format 3, one per served project
    CARRIED                           only when an export failed: one project name per line
/dumps/queue/.20261104T123000Z.4711.tmp/   a round in progress, never read by backup
/dumps/door/20261104T121500Z/         dir 700 (root)
    users.yml  mcp-users  acts.log  webauthn.yml  totp.yml   each when present, 600
backup: /snapshot/queue/, /snapshot/door/ -> restic --host <HUB_HOST> --tag queue | door
restore: <DIR>/queue/<name>.jsonl [CARRIED], <DIR>/door/<files>
```

A generation's name is `date -u +%Y%m%dT%H%M%SZ` at its round's start, so byte order is time order. Each dumper keeps its newest 3 generations and removes any `.*.tmp` older than 60 minutes.

### `hub-ops`

`scripts/deploy/ops/hub-ops <mode> [<argument>...]`, POSIX sh, with `LC_ALL=C` and `umask 077`. Every path sits under `HUB_OPS_ROOT`, which is empty in a container: the tests' scratch root, as `HUB_ADMIN_ROOT` is `hub-admin`'s.

| Mode | Service | Does |
|---|---|---|
| `queue [once]` | `dump-queue` | each round: one generation of every served project's queue |
| `door [once]` | `dump-door` | each round: `hub-admin dump` into one generation |
| `upload [once]` | `backup` | each round: the newest generation of each part to restic; the 03:05 UTC round prunes |
| `fresh` | `backup` (healthcheck) | exit 0 when `/state/ok` is younger than 35 minutes |
| `init`, `snapshots`, `check` | `backup` (by hand) | `restic init`; `restic snapshots --host <HUB_HOST>`; `restic check` |
| `restore DIR [QUEUE DOOR]` | `backup` (by hand) | two snapshots, `latest` by default, into `DIR/queue` and `DIR/door` |

- **Exits.** 0: the round or act was whole. 1: a round or act that failed or was not whole, named on stderr. 2: a bad call or refused setting, before anything runs: an unknown mode, an extra argument, a `DIR` that is not an existing empty directory, a bad `restic_env`, an absent `HUB_HOST`. A loop exits only on 2 or a signal; a failed round is logged and the next slot runs. TERM ends the running child, removes the round's staging and exits 143.
- **The grid.** `now=$(date -u +%s)`; the next slot is `now - (now - off) % 900 + 900`, with `off` 0 for `queue` and `door` and 300 for `upload`. A round is the 03:05 one when `slot % 86400 = 11100`; `once` never prunes.
- **A `queue` round.**
  1. Make `.<t>.<pid>.tmp`.
  2. For each folder of `/projects` holding a `specengine.toml`, in byte order (`*` with `[ -d ]`, as `specengine-entrypoint`), run `spec export state --root /projects/<n> --out <tmp>/<n>.jsonl` and log its line.
  3. On a failed export, hard-link the previous generation's `<n>.jsonl` if there is one, and add `<n>` to `CARRIED`.
  4. Rename the staging to `<t>`, then prune.
  5. Exit 1 if any export failed.
- **A `door` round.** Make the staging, then run `/bin/sh /opt/hub-admin/hub-admin dump <tmp>`. A failure empties the staging and tries again, 12 times 10 s apart, then logs `skipped: <hub-admin's last line>`, removes the staging and exits 1. A success renames the staging and prunes.
- **An `upload` round.**
  1. Read `restic_env`.
  2. For each part, `queue` then `door`, take the newest complete generation G. The round is not whole when there is none, when G is older than 35 minutes (`find -mmin`), or when G holds `CARRIED`.
  3. Copy G to `/snapshot/<part>`, then run `restic backup --host "$HUB_HOST" --tag <part> /snapshot/<part>`. Any exit but 0 means not whole. Exit 10 logs `the repository is not initialised: run docker compose run --rm backup init` (never an `init`); exit 12 logs `the repository password is wrong`.
  4. In the 03:05 round, run `restic forget --host "$HUB_HOST" --tag queue --keep-within 48h --keep-daily 30`, then the same for `door`, then `restic prune`.
  5. When the round is whole, `touch /state/ok`.
- **`restore`.** `restic restore <QUEUE>:/snapshot/queue --target DIR/queue`, then the same for `door`. Only `latest` adds `--host "$HUB_HOST" --tag <part>`.
- **Lines** start `hub-ops: <mode> <t>: `, and no line holds a secret or a `restic_env` value.

### `hub-admin`: `dump` and `restore`

- **`dump DIR`**, which, like `list`, needs no terminal, and which writes nothing of the door's.
  - Exit 2, before anything: no `DIR`, an extra argument, or a `DIR` that is not an existing empty directory, or that starts with `-`.
  - It takes the lock: held, exit 1, with the lock's message.
  - It copies, by `cat` under `umask 077`, each one present: `/users/users.yml` to `DIR/users.yml`, `/mcp-users/users` to `DIR/mcp-users`, `/var/lib/hub-admin/acts.log` to `DIR/acts.log`.
  - When `/var/lib/authelia/db.sqlite3` exists, it runs `authelia storage user webauthn export --file DIR/webauthn.yml --sqlite.path <db>`, then `authelia storage user totp export --file DIR/totp.yml --sqlite.path <db>`, both through `door_run`. A failure is exit 1: `the door's passkey|TOTP export failed: <door said>`.
  - stdout: `dumped <files>`.
  - It never writes the act log, also when it fails: `stopped_log` skips `dump`. Every exit releases the lock.
- **`restore DIR`**, which writes, so it needs a terminal.
  - Exit 2: no `DIR`, an extra argument, a `DIR` that is not a directory, or a `DIR/users.yml` that is absent or empty.
  - Exit 1, nothing called or written, before the question: the door is not empty. That is a non-empty `users.yml`, MCP users file or `acts.log`, or an existing `db.sqlite3`. The message is `the door is not empty: <path> exists; a restore goes into a new host's empty door; nothing changed`.
  - `DIR/users.yml` is read by `list`'s reader; an unknown layout is exit 1.
  - The question: `restore <n> account(s) from DIR, with passkeys and TOTP when DIR holds them? [y/N]`. Only `y` or `yes` goes on; else exit 1, `not restored: the answer was not y; nothing changed`.
  - Under the lock:
    1. When DIR holds `webauthn.yml` or `totp.yml`: `authelia storage migrate up --sqlite.path <db>`, then `authelia storage user webauthn import --sqlite.path <db> -- DIR/webauthn.yml`, then `... totp import --sqlite.path <db> -- DIR/totp.yml`. A failure is exit 1: `the door's <step> failed: <door said>; the users files are as they were; remove the door volume's db.sqlite3 before restore runs again`.
    2. The users file, then the MCP users file, installed as `add` installs them: staged, read back, renamed, mode 600.
    3. The stamp is raised.
    4. The act log becomes DIR's `acts.log` bytes plus one line: `<time>\trestore\t-\t<users mcp passkeys totp, those restored>`.
  - stdout: `restored <n> account(s) from DIR`, then `next: docker compose up -d traefik authelia specengine`.
- **The door's CLI calls** above (flags, argument order, the files) are from memory until VF4.
- **The lock's message**, for every act: `another act or a dump holds admin-log/lock (taken <time>); if none runs, remove it`.
- **Usage** gains `dump <empty folder>` and `restore <folder>`.

### `specengine-health`

`scripts/deploy/health.sh`, copied as `/usr/local/bin/specengine-health`, `#!/bin/bash`. It reads `SPECENGINE_HEALTH_PORT` (default 7777, the entrypoint's port) and `SPECENGINE_HEALTH_PROXY_PORT` (default 443, `websecure`'s).

1. It opens `/dev/tcp/127.0.0.1/<port>` and writes `GET / HTTP/1.1\r\nHost: 127.0.0.1:<port>\r\nConnection: close\r\n\r\n`, with no other header.
2. It reads one line within 5 s (`read -t 5`) and strips its CR. Alive needs that line to start `HTTP/1.1 403 `.
3. It opens `/dev/tcp/127.0.0.1/<proxy port>`.

Exit 0 means alive, 1 means not; one stdout line either way:
- `alive: <status line>`
- `not alive: no answer from 127.0.0.1:<port>`
- `not alive: 127.0.0.1:<port> answered <line>: a hub answers a loopback probe 403`
- `not alive: nothing listens on 127.0.0.1:<proxy port>: the proxy left this namespace; run docker compose up -d`

### Runbooks (the text of `docs/canon/hub-ops.md` at ship)

**Restore**, the old host lost:
1. Have at hand the off-host copies (`.env`, `restic_password`, `restic_env`) and the forge and DNS accounts.
2. Install as the guide does, at the lost host's tag or later. Copy `.env` back, put `restic_password` and `restic_env` in `secrets/`, and make new `authelia_*` secrets. Make a new deploy key, add it at the forge as the old one was, and delete the lost host's key there. Make `known_hosts` as at install. Skip "Your account": no `admin add`.
3. Restore the files:
   ```sh
   docker compose run --rm backup snapshots                  # the newest queue and door
   install -d -m 700 restore
   docker compose run --rm -v "$PWD/restore:/restore" backup restore /restore    # [<queue id> <door id>]
   docker compose run --rm -v "$PWD/restore/door:/restore:ro" admin restore /restore
   ```
   The door is restored before it first starts.
4. Clone each project as the guide's "Bring a project over" step 2 does.
5. If the old host ran `--push`, raise the floor before the hub's first start. `docker compose run --rm -T --entrypoint git dump-queue -C /projects/<name> log '--format=%(trailers:key=Proposal,valueonly)' | sort -t- -k2n | tail -n 1` prints each project's highest. When one plus the highest number over all projects exceeds 100001, write it: `docker compose run --rm --entrypoint sh dump-queue -c 'mkdir -p /data/specengine && printf "%s\n" <N> > /data/specengine/proposal-floor'` (`queue-backup.md#id-floor`).
6. Bring the queues back:
   - Start the door and the hub but not the dumpers: `docker compose up -d traefik authelia specengine`.
   - Per project, `sudo install -o 10001 -g 10001 -m 600 restore/queue/<name>.jsonl projects/<name>.jsonl`, then `docker compose exec -it specengine spec import-state --root /projects/<name> /projects/<name>.jsonl`, answer `y`, and delete the file.
   - `restore/queue/CARRIED`, when present, names the projects restored from an older dump.
7. Redo the lost acts.
   - Check the accounts with `docker compose run --rm admin list` and the log with `docker compose run --rm admin log`.
   - The log's last line is the last act the dump holds. Redo every act made on the old host after it, each `disable` first.
   - A `mcp rotate` made after it is redone too, since the restored line is the old secret's.
8. `docker compose exec specengine spec index --root /projects/<name>` per project, so the daemon's first walk is a stat walk.
9. `docker compose up -d`, so the dumpers and `backup` join. Point DNS at the new host; TLS follows. Then `sudo rm -rf restore`.

- **Without `--push`,** every commit the hub made died with the host. The restored `applied` proposals keep an `applied_commit` that no checkout holds, and their texts are absent from the tree; whoever confirmed them raises them again. With `--push`, only the commits after its last push are lost: those of a `held` or `retrying` sync, which the sync line counts.
- **The loss window** is everything after the restored round, about 20 minutes at most: proposals, cards and tasks with their IDs (issued again: `queue-backup.md` "Known limits", R1), and acts.
- **On the same host** (a broken volume): `docker compose down`, then remove `authelia-data` and `specengine-data` (`docker volume ls`), but only once `backup snapshots` shows the snapshot to restore. Then go on from step 3.

**Upgrade:**
1. Read the release's changes: `git diff v<old> v<new> -- scripts/deploy`.
2. Take a dump with the old version: `docker compose run --rm dump-queue queue once`, `... dump-door door once`, `... backup upload once`. Each must exit 0. Note both snapshot IDs (`backup snapshots`): they are the way back.
3. In the checkout, `git fetch --tags origin && git checkout v<new>`. `.env`, `secrets/` and the folders are untracked. Run `diff .env.example .env` for new keys, then set `SPECENGINE_VERSION=<new>` in `.env`.
4. `docker compose pull`, then `docker compose up -d`, then `docker compose ps`: all healthy.

- **Back.** Queue migrations run forward on the first open, and an older binary then refuses the DB. To go back: `docker compose down`; the old tag and `.env`; remove `specengine-data`; the floor again if a restore raised it; `docker compose up -d traefik authelia specengine`. Then restore the noted queue snapshot and run Restore step 6, then `up -d`. A door pin moved forward goes back the same way: remove `authelia-data`, then `admin restore` the noted door snapshot.
- **The version floor.** Before the hub's first record or create, every `spec` that writes to its repositories runs 0.29.1 or later (`spec-ids.md` "Older binaries", ADR-0070). An upgrade never lowers that floor.

**Push**, after the gate:
1. See what the first sync pushes: `docker compose exec specengine git -C /projects/<name> log --oneline @{u}..`. If those are a trial's commits, bring the project over again first.
2. Make a write key for the one repository: `ssh-keygen -t ed25519 -N '' -C specengine-hub -f secrets/deploy_key.new`. Add its `.pub` at the forge with write access, then delete that `.pub`. `mv secrets/deploy_key.new secrets/deploy_key`, `chmod 600`, `sudo chown 10001:10001`. Delete the read-only key at the forge. Check `known_hosts` again against the forge's published fingerprints.
3. Lift the branch's rules for the deploy key. The hub pushes the branch its checkout tracks, the game's main, with a merge commit whenever it and a developer have diverged, and never rebases or forces (`hub-git.md` "Merge commit"). So linear history is lifted for that key, and so are required pull requests, reviews, status checks and signatures; force pushes and deletion stay forbidden. On GitHub this is a ruleset whose bypass list holds deploy keys, since classic branch protection cannot exempt one (from memory; HO13).
4. Set `SPECENGINE_PUSH=1` in `.env`. `docker compose up -d` recreates `specengine` with the new key.
5. Check: the daemon's log has `push every 5 min`; after a confirm, `git ls-remote origin <branch>` equals the hub's `HEAD`; the board's sync line shows nothing.

### Image and CI

- **Dockerfile:** `COPY --chmod=0755 scripts/deploy/health.sh /usr/local/bin/specengine-health`. There is no `HEALTHCHECK` (compose holds the timing), and no package is added: Debian's bash is Essential.
- **`Dockerfile.dockerignore`:** allows the probe.
- **`image.yml`, `check`:** `cargo test --locked -p specengine-http --test deploy_ops`, plus C5.
- **`image.yml`, `build`'s smoke:** C4.

## Rules and edge cases

- **R1.** ADR-0012: a failed dump, upload or probe blocks nothing. No confirm, read, check or sync waits on one, and the daemon never learns of them.
- **R2.** No dumper writes into `/projects`, the door's files or the queue. In `/data`, `dump-queue` writes only SQLite's shared-memory file.
- **R3.** A generation is renamed into place only after its round, and `backup` reads only complete ones. When two rounds fall in one second, the second's rename fails: its staging is removed, and the round exits 1.
- **R4.** Plaintext dumps live on tmpfs alone (`hub-dumps`, `backup`'s `/snapshot`). The one exception is a restore's `restore/` folder, which the runbook deletes.
- **R5.** No secret appears in `.env`, `environment:`, an argument, a log line or the act log. `restic_env`'s values are never printed, and restic reads the password from its file.
- **R6.** The probe sends no `Remote-User`, `Origin` or `Sec-Fetch-*` header. Its `Host` is loopback, so the fence refuses it at step 1, and nothing is read or recorded.
- **R7.** `backup` mounts no live volume and no host folder but `./ops`; nothing else joins its network.
- **R8.** `hub-admin dump` never waits for the lock and never breaks it; `hub-ops door` retries 12 times, then skips the round. A stale lock, left by a killed act, stops door dumps until someone removes it; `backup`'s health shows that.
- **R9.** A restore goes into an empty door (no users file, MCP users file, act log or database) and an empty queue (`queue-backup.md` "Import" step 3). Neither ever merges.
- **R10.** Time is UTC throughout. Each round recomputes the grid from `date -u +%s`, so a clock step moves at most one round.
- **R11.** A folder removed from `/projects` is absent from the next generation; its older snapshots age out by S6.
- **R12.** A failed export keeps that project's previous dump, hard-linked, and names it in `CARRIED`. So a snapshot always holds every project's newest good dump, and the round is not whole.
- **R13.** `--push` waits for the gate (`hub-git.md` "Rules"). This step adds the procedure; `.env.example` keeps `SPECENGINE_PUSH=0`.
- **R14.** One hub per restic `--host`: two hubs sharing a repository need two `HUB_HOST`s.

## Acceptance criteria

Tests: http `tests/deploy_ops.rs` (new; compose and Dockerfile read as `deploy.rs` reads them; `hub-ops` under `sh` with stubs `spec`, `restic`, `date` and `sleep` first on `PATH`, plus the real `hub-admin` with `deploy_admin.rs`'s stub `authelia`; `health.sh` under `bash` against `std::net::TcpListener`s on free ports, through its two variables); `tests/deploy_admin.rs` (OP11-OP15, a terminal by `script(1)`); `tests/deploy.rs` (OP6, OP23). M marks the mutations that must turn each red.

- [ ] OP1 -- Each new service has exactly the keys, mounts, secrets and environment in "Compose". `dump-queue`'s image is byte for byte `specengine`'s, and `dump-door`'s is `authelia`'s. `backup` has `restic/restic:<x.y.z>@sha256:<64 hex>`. No new service has `ports`, `expose`, `privileged`, `cap_add`, `pid`, `ipc`, `devices` or a `service:` or `host` network. M: `network_mode: service:traefik` on `dump-queue`; `backup` on `default`; `/projects` writable in `dump-queue`; `restic_env` granted to `dump-door`; `hub-dumps` writable in `backup`; `init` dropped from one; `restic/restic:latest`.
- [ ] OP2 -- `hub-dumps` is tmpfs (`driver_opts` `type` and `device` `tmpfs`, `o` holding `size=` and `mode=0700`). Network `backup` holds `backup` alone. The two secrets are as in "Compose". `.env.example` gains no key and still passes D10. `.gitignore` holds `restore/`. M: `hub-dumps` a plain volume; `authelia` on `backup`; `RESTIC_REPOSITORY` in `.env.example`; `restore/` dropped.
- [ ] OP3 -- The trust items stay:
  - `specengine` is alone in Traefik's namespace.
  - No `docker.sock` anywhere.
  - `deploy_key` and `known_hosts` go to `specengine` alone; `authelia_storage_key` to `authelia`, `admin` and `dump-door` alone; the restic secrets to `backup` alone.
  - `backup` mounts none of `specengine-data`, `authelia-data`, `./projects`, `./authelia/users`, `./traefik/mcp-users`, `./admin-log`.

  M: `specengine-data` into `backup`; the socket into `backup`; `deploy_key` to `dump-queue`.
- [ ] OP4 -- `specengine`'s healthcheck is as in "Compose". The Dockerfile copies `health.sh` as `specengine-health` with `--chmod=0755`, sets no `HEALTHCHECK` and installs no `curl` or `wget`; the dockerignore keeps the file. The probe's two defaults equal the entrypoint's port and `websecure`'s address. M: no `COPY`; `curl` installed; the default 7778; the proxy default 80.
- [ ] OP5 -- `image.yml`'s check job runs `--test deploy_ops`, and C4 and C5 are steps of their jobs. M: the test dropped.
- [ ] OP6 -- No version moves: `.env.example`'s `SPECENGINE_VERSION` equals `plugin.json`'s version, 0.29.1; `tag-check.sh` accepts `v0.29.1` and refuses `v0.29.2`; `image.yml`'s check job runs it as its first step. M: `.env.example` at 0.29.2; `plugin.json` alone bumped; the `tag-check.sh` step moved after the tests.
- [ ] OP7 -- The probe:
  - exit 0 for a daemon listener answering `HTTP/1.1 403 Forbidden` with the proxy listener open;
  - exit 1, with its line, for: a `200`; a `502`; a close with no answer; the proxy port closed while the daemon answers 403; no listener;
  - exit 1 after about 5 s for a listener that accepts and never writes;
  - the request it sends has `Host: 127.0.0.1:<port>` and none of `Remote-User`, `Origin`, `Sec-Fetch-Site`.

  M: any status accepted; the proxy test dropped; `Host: hub.example.org`; a `Remote-User` sent; no `-t`.
- [ ] OP8 -- `hub-ops queue once` over projects `a`, `a-1`, `c` (with `b` holding no `specengine.toml`, and a file `z`) calls the stub `spec` exactly as "A `queue` round" does, in byte order `a`, `a-1`, `c`, never for `b` or `z`. It leaves one generation, matching `^[0-9]{8}T[0-9]{6}Z$`, with mode 700 and files of mode 600, and no `.tmp`. M: the `*/` glob; `--out` inside a project; no rename; no `umask`.
- [ ] OP9 -- When `spec` fails for `a-1` (exit 2, one stderr line), the generation holds the new `a.jsonl`, the previous generation's `a-1.jsonl` byte for byte, and `CARRIED` reading `a-1`; the log names `a-1` and the line; the exit is 1. With no previous generation, `a-1.jsonl` is absent and `CARRIED` lists it. M: no carry; exit 0; no `CARRIED`.
- [ ] OP10 -- After five `once` rounds (stub `date` advancing), the newest three generations remain. A `.tmp` dated two hours back is removed; one dated ten minutes back is kept. M: keep all; a young `.tmp` removed.
- [ ] OP11 -- `hub-ops door once` with the real `hub-admin`:
  - the generation's `users.yml`, `mcp-users` and `acts.log` equal their sources byte for byte;
  - `webauthn.yml` and `totp.yml` are the stub's;
  - the stub saw exactly the two export calls of "`hub-admin`: `dump` and `restore`";
  - the act log is unchanged, and the lock is gone.

  Without `db.sqlite3`, there is no CLI call and the three files alone. M: copy before the lock; log the dump; skip an export; call the CLI with no database.
- [ ] OP12 -- The lock:
  - With `admin-log/lock/` held, `door once` makes 12 tries (stub `sleep` called 12 times with 10) and leaves no generation; the lock and its `taken` stay byte for byte; exit 1.
  - While a `dump` holds the lock (stub export held), `hub-admin disable mira` is refused with the new message.
  - TERM to `door` mid-export leaves no lock and no staging.

  M: the lock broken; no retry; no trap; the old message.
- [ ] OP13 -- `hub-admin dump`:
  - exit 2, nothing locked or called, for: no argument; two arguments; a missing, non-empty or `-`-led `DIR`;
  - a failing export: exit 1, the lock released, no act log line;
  - with stdin `/dev/null`, it still runs.

  M: a non-empty `DIR` accepted; a terminal required; `stopped_log` on `dump`.
- [ ] OP14 -- `hub-admin restore` into an empty door, on a terminal, answered `y`:
  - the stub saw `migrate up`, `webauthn import`, `totp import` in that order, with DIR's files after `--`;
  - then `users.yml` and the MCP users file are installed byte for byte, each with a new inode and mode 600;
  - the stamp went up;
  - `acts.log` is DIR's bytes plus one `restore` line.

  Answered `n`: exit 1, nothing called. M: the users file before the imports; no stamp; the log without the `restore` line, or without DIR's lines.
- [ ] OP15 -- `restore` refusals:
  - exit 2 for: stdin a pipe; no `DIR/users.yml`;
  - exit 1, nothing called or written, for each of a non-empty users file, MCP users file or act log, and an existing `db.sqlite3`, each path named;
  - a `DIR/users.yml` in an unknown layout: exit 1;
  - a failing import: exit 1, both users files untouched, the database named.

  M: the database check dropped; a non-empty users file accepted.
- [ ] OP16 -- `hub-ops upload once`, given fresh complete `queue/<t1>` and `door/<t2>`:
  - `/snapshot/queue` and `/snapshot/door` hold their bytes;
  - the stub `restic` saw `backup --host hub.example.org --tag queue /snapshot/queue`, then the same for `door`;
  - its environment holds each `restic_env` key with its value, and `RESTIC_PASSWORD_FILE`;
  - `/state/ok` is touched; there is no `init`, `forget` or `prune`.

  M: back up `/dumps` itself; `init` on exit 10; skip the copy.
- [ ] OP17 -- An upload round that is not whole leaves `/state/ok` untouched and exits 1, naming why:
  - a door generation 40 minutes old: the queue is still sent;
  - a `CARRIED` file;
  - `restic` exiting 10 (the `init` line, and no `init`) or 12;
  - no generations at all (no `backup` call).

  `hub-ops fresh` exits 0 for an `ok` 10 minutes old and 1 for 40, or for none. M: `ok` on a stale part; `fresh` always 0; 350 minutes for 35.
- [ ] OP18 -- A loop round at slot 03:05 UTC (stub `date`) runs, after both backups, `forget --host <h> --tag queue --keep-within 48h --keep-daily 30`, then the same for `door`, then `prune`. The slot 03:20 runs none of them, and neither does `once`. M: `--prune` in `forget`; `--keep-daily 300`; a prune every round.
- [ ] OP19 -- `restic_env`:
  - exit 2 naming the line and key, no value printed, `restic` never called, for: `PATH=/x`; `LD_PRELOAD=x`; `RESTIC_PASSWORD=x`; `export AWS_X=y`; `aws_x=y`; a line with no `=`; no `RESTIC_REPOSITORY`; two of them;
  - comments and empty lines are skipped;
  - a value holding `$HOME`, spaces, `'` and `"` reaches `restic` verbatim.

  M: the file sourced with `.`; the value printed; `RESTIC_PASSWORD` accepted.
- [ ] OP20 -- The acts:
  - `init` runs `restic init` once; `snapshots` and `check` run as in "`hub-ops`";
  - `restore /r` runs `restore latest:/snapshot/queue --host <h> --tag queue --target /r/queue`, then the same for door; `restore /r Q D` runs `restore Q:/snapshot/queue --target /r/queue`, with no filter;
  - exit 2 for: a missing or non-empty `DIR`; an unknown mode; an extra argument.

  M: `restore latest` with no subfolder; a non-empty `DIR` accepted.
- [ ] OP21 -- The loops, with stub `date` fixed at epoch E and a stub `sleep` that records each argument and sends TERM to its parent at the call the test names:
  - `queue` ran one round, then asked for `900 - E % 900` (TERM at the first call);
  - `upload` asked for 60, ran one round, then asked for `900 - (E - 300) % 900` (TERM at the second call);
  - TERM during a round's export (a stub `spec` that waits) left no `.tmp`, and the exit was 143.

  M: no round at start; a fixed `sleep 900`; no trap.
- [ ] OP22 -- OP8-OP21 pass under the laptop's `/bin/sh`, and on CI under `dash`; `health.sh` under bash 3.2 (macOS) and 5. M: a `[[` or `${v:0:1}` in `hub-ops` (dash rejects both).
- [ ] OP23 -- 11b-1's D7, D9, D11 and D14 and 11c-1's AC8 are rewritten as these decided changes, and the rest of `deploy.rs` and `deploy_admin.rs` pass unchanged:
  - seven services, the new images pinned;
  - each secret granted per OP3;
  - three scripts copied into the image;
  - the lock's new message.
- [ ] OP24 -- `export index` and `check` are green; the guide stays within Tier 1, `docs/canon/hub-ops.md` within 12 288 B, `hub-deploy.md` within its cap.

CI only, on the next tag:
- [ ] C4 -- In the built image, `specengine-health` exits 1 against the smoke's local daemon (a loopback `GET /` answers 200) and 0 against a second daemon started with `--public-origin https://hub.example.org`; ports are given through its two variables.
- [ ] C5 -- The pinned door image exits 0 for `--help` of `storage user webauthn export`, `storage user webauthn import`, `storage user totp export`, `storage user totp import` and `storage migrate up`. The pinned restic image prints `restic 0.17` or later and holds `sh`, `cp`, `find`, `ln`, `touch` and `date`.

Before the gate, the owner's, with Docker and the pinned images (VF1-VF3: `hub-accounts.md`):
- [ ] VF4 -- The door's CLI:
  - `webauthn export --file` and `totp export --file`, and the files they write;
  - `migrate up` on an absent database;
  - both imports into it, with their flags and argument order;
  - a door started on that database under the same key, and passkey and TOTP sign-ins after;
  - an import's answer for a credential already present;
  - no `sqlite3` in any pinned image.
- [ ] VF5 -- Exports under a running door while people sign in: no lost write, and no `database is locked` beyond the CLI's own wait; the door's journal mode.
- [ ] VF6 -- `hub-dumps`:
  - one volume that three containers share;
  - its content kept while any of them runs, and lost at reboot;
  - `uid` and `mode` honoured;
  - root in the door's and restic's images reads and writes it;
  - nothing of it on the host's disk.
- [ ] VF7 -- The restic image:
  - busybox `sh`, `cp -R`, `find -mmin`, `date -u +%s`;
  - `restore <id>:/snapshot/queue`;
  - exits 10 and 12;
  - the cache volume;
  - a stale lock after a killed `prune`, with the fixed `hostname`.
- [ ] VF8 -- Debian's bash in the image opens `/dev/tcp`; `docker compose ps` shows `healthy`, then `unhealthy` after a `kill -STOP` of the daemon.
- [ ] VF9 -- `docker compose run -v "$PWD/restore:/restore"` mounts for `backup` and `admin`, and `run --rm` of a looping service runs `once` and leaves nothing.

The gate, step 11b-3, by hand: the owner's session. Findings go in `hub-deploy.md` "Gate".
- [ ] HO1 -- Install with the backend, `backup init`, `up -d`: six services up, and `specengine` and `backup` healthy within 20 minutes.
- [ ] HO2 -- Two rounds land off the host on the grid: `backup snapshots` shows both tags at 5 minutes past a quarter. The backend's own listing holds only restic's encrypted objects.
- [ ] HO3 -- `docker compose logs dump-queue dump-door backup` hold no secret, password, digest or TOTP. `hub-dumps` is tmpfs (`findmnt` in a dumper), and the host's disk holds no dump.
- [ ] HO4 -- (G10) A dump while an act holds the lock retries or skips; an act during a dump is refused with the new message; the next snapshot restores whole files.
- [ ] HO5 -- A wrong key in `restic_env`, with `backup` restarted, turns `backup` unhealthy within 40 minutes; it is healthy again a round after the fix. A stopped `dump-door` does the same.
- [ ] HO6 -- A restore onto a fresh VM by the runbook:
  - `admin list` equals the old host's;
  - an old passkey and an old TOTP sign in, and an old MCP secret works;
  - the board shows the old proposals, cards and tasks;
  - `import-state`'s counts equal the dump's header;
  - no snapshot newer than the restored one is empty.
- [ ] HO7 -- On a host in use, `admin restore` is refused naming the database, and `import-state` naming the filled queue.
- [ ] HO8 -- A `disable` made after the last snapshot is absent from the restored `admin log`; once redone, the person is refused.
- [ ] HO9 -- Without `--push`, a confirm of the old host reads `applied` on the new one, and `git cat-file -e <applied_commit>` fails; after HO13, a new one's commit is present.
- [ ] HO10 -- An upgrade from `v0.29.1`, the first tag, to the next published one, run at that release, by the runbook keeps the queue and the accounts. The way back on the old tag, by the noted snapshots, works.
- [ ] HO11 -- Health:
  - `kill -STOP` of `specengine-http` turns `specengine` unhealthy within 2 minutes, and `kill -CONT` healthy again;
  - a `docker kill` of `traefik`, restarted by the engine, turns it unhealthy until `docker compose up -d`;
  - `https://<DOOR_HOST>/` answers 200, and `https://<HUB_HOST>/` redirects to the door.
- [ ] HO12 -- A probe leaves nothing in the daemon: no event, no queue row, no log line.
- [ ] HO13 -- `--push` by the runbook:
  - `push every 5 min` is logged;
  - a confirm's commit is on the remote within a minute;
  - after a developer's diverging commit, a merge commit lands on main with the hub's commit as first parent.
- [ ] HO14 -- With linear history enforced again for the key, the push reads `held` and the sync line shows it; confirms go on.
- [ ] HO15 -- A `--push` restore raises the floor above the remote's highest `Proposal:` trailer, and the new hub's next proposal is above it.
- [ ] HO16 -- After a restore, the forge refuses the lost host's deploy key.
- [ ] HO17 -- After three days, `backup snapshots` holds every round of the last 48 hours and one a day before; `backup check` is clean; the 03:05 prune is in the log.

## Out of scope

SFTP and rclone backends. Setting up an append-only or object-locked backend: recommended, the operator's. Off-host copies of unpushed commits (a `git bundle`): `--push` carries commits off the host. Dumps of `traefik-acme` (reissued), the index (rebuilt), `proposal-floor` (constant unless raised) and `.env` (the operator's copy). A drill automation, an alert channel, log rotation, Traefik's own health. Any crate change. The gate itself, 11b-3.

## Implementation

Not started. The estimate is 2.5-3.5 pipeline hours, above RM-0074's 1-1.5: three services and two `hub-admin` acts. If iteration 1 fails, the slice splits into 11b-2a (the dumps and the restore) and 11b-2b (health, the upgrade, `--push`).

### Files in scope

| Module | What it does |
|---|---|
| `scripts/deploy/ops/hub-ops` | the three loops, the grid, the generations, `restic_env`, `backup`'s acts |
| `scripts/deploy/health.sh` | `specengine-health` |
| `scripts/deploy/admin/hub-admin` | `dump`, `restore`, the lock's message, the usage |
| `scripts/deploy/docker-compose.yml` | `dump-queue`, `dump-door`, `backup`, two healthchecks, `backup`'s network, `hub-dumps`, `restic-cache`, two secrets |
| `scripts/deploy/Dockerfile`, `Dockerfile.dockerignore` | the probe in the image |
| `scripts/deploy/.gitignore` | `restore/` |
| `.github/workflows/image.yml` | `deploy_ops`, C4, C5 |
| http `tests/deploy_ops.rs` (new), `tests/deploy.rs`, `tests/deploy_admin.rs` | OP1-OP23 |

### Work order

1. `hub-admin dump` and `restore`, with the stub door's new shapes.
2. `hub-ops`.
3. `health.sh`, the Dockerfile and the dockerignore.
4. Compose and `.gitignore`.
5. `image.yml`.

The tester writes `deploy_ops.rs` beside step 2.

### Canon at ship

Free bytes as of 2026-11-04.

| Page | Bytes / cap (free) | Change |
|---|---|---|
| `docs/canon/hub-ops.md` | new / 12 288 | Tier 2, `scope: [scripts/deploy, .github/workflows]`. Sections: Dumps (S1-S6, the services, the secrets, retention, the chain's health), Restore, Upgrade, Health, Push, Pins (moved here, plus restic's), Known limits, Tests. Target under 11 500 B. The ADR's `canon:` is `#dumps` |
| `docs/canon/hub-deploy.md` | 12 276 / 12 288 (12) | The header's RM-0074 sentence becomes a pointer (-42). "Host": six services, the dumpers named, a pointer (+95); `specengine`'s healthcheck (+55); the restic secrets, `dump-door`'s key, `restore/` (+82). "Image": the third script (+30). "Pins": a one-line pointer to `hub-ops.md` (-640). "Gate": HO1-HO17 (+30). "Known limits": the Traefik crash shows `unhealthy` (+18). "Tests": `deploy_ops.rs` (+40). Net about -330 |
| `scripts/deploy/README.md` | 10 152 / 10 240 (88) | The header's "not here yet" sentence becomes a pointer (-75). Install: `restic_password` in the loop (+16), a `restic_env` and `backup init` line (+110). The deploy key bullet points to "Push" (0). "Before the gate": VF4-VF9 in `docs/features/hub-ops.md` (+45). Net about +96, over by about 8: trim a "Limits" line |
| `docs/canon/hub.md` | 12 282 / 12 288 (6) | "Known limits" "Health": `(RM-0074)` becomes `: hub-ops.md` (+5) |
| `docs/canon/queue-backup.md` | 11 803 / 12 288 (485) | "Known limits": the pointer, and the `--push` restore's floor raise (+100) |
| `docs/canon/queue-export.md` | 4 191 / 12 288 | "Known limits": the pointer (0) |
| `docs/canon/spec-ids.md` | 10 190 / 12 288 (2 098) | "Older binaries": `hub-ops.md` "Upgrade" (+5) |
| `crates/specengine-http/README.md` | 10 229 / 10 240 (11) | `deploy{,_admin,_ops}.rs` (+5) |
| `docs/canon/hub-git.md`, `hub-mcp.md`, `spec-read-freshness.md` | 7, 2, 75 free | unchanged |
| `docs/index-canon.md` | 10 159 / 10 240 (81) | the new page's line would overflow it, so the `hub*` lines move to a new shard |
| `docs/index-canon-hub.md` | new, generated | claims `docs/canon/hub*.md` (seven pages) |
| `specengine.toml` | | `writes` gains the shard; `shards` gains `{ path = "docs/index-canon-hub.md", claims = ["docs/canon/hub*.md"] }` before `docs/canon/*.md` |
| `docs/README.md` | 10 230 / 10 240 (10) | "Generated": "Here eight", the shard named (+55). "Enforcement" "Roles": "run `export index`, then `check`, before handing in" (-60) |
| `docs/index.md` | 2 916 / 10 240 | one shard line more (generated) |
| `CLAUDE.md` | 9 140 / 16 384 | "State": `hub-ops` shipped; the next-step sentence |
| `08-roadmap.md`, `docs/roadmap/RM-0074.md` | | the 11b-2 row and the item: shipped |
| `docs/canon/architecture.md` | | unchanged: no rule moves |

### Draft ADR

Issued at ship under the next free number, `canon: docs/canon/hub-ops.md#dumps`, `scope: [scripts/deploy]`, `ref: hub-ops (RM-0074); the orchestrator's working answer to Q5 of 2026-11-04, awaiting the owner's review`. Title: "A hub's state leaves the host every 15 minutes, restic-encrypted, by the deployment's sidecars; no key and no SpecEngine code".

**Decision.** Accepted by the orchestrator in the owner's absence, awaiting the owner's review. Supplements ADR-0067 and ADR-0068. Every 15 minutes, two sidecars with no network stage each served project's queue (`spec export state`) and the door's state on tmpfs. The door's state is its users file, Authelia's own passkey and TOTP exports, the MCP users file and the act log, taken under `hub-admin`'s lock. Restic then sends them, encrypted on the host, to the operator's backend; only that sidecar has a network. The deploy key and Authelia's storage key never leave the host: a restore imports under new ones. No crate changes.

**Why.** Losing the VPS must not lose the queue or force anyone to enrol again. A copy of a running door's SQLite is unsafe, and no pinned image has `sqlite3`; the door's exports do not depend on its key. Restic encrypts client-side and reaches most backends with no code.

**Cost.** An outside storage account. The restic password and `restic_env` must be kept off the host, or the dumps cannot be read. Up to 20 minutes of work dies with a host. An intruder on the host can delete the backups unless the backend locks objects. Three more services.
