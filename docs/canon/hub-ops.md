---
class: canon
tier: 2
scope: [scripts/deploy, .github/workflows]
owner: owner
reviewed: 2026-11-20
---

# Hub ops: off-host dumps, restore, upgrade, health and `--push`

ADR-0071, built by `hub-ops` (RM-0074, step 11b-2), working answers of 2026-11-04, the owner's review open. A hub's state leaves the host every 15 minutes, encrypted, once the owner names a backend (`hub-restic.md`); no crate change, Docker socket or `docker exec`. Files: `scripts/deploy/`; setup: its guide.

<a id="dumps"></a>
## Dumps

Three sidecars, `restart: unless-stopped`, `init: true`, run `/bin/sh /opt/hub-ops/hub-ops <mode>` (`./ops`, read-only) on their tool's image:
- **`dump-queue`**, the hub's, `network_mode: none`: `/projects` read-only, `specengine-data` (a WAL reader maps its `-shm`).
- **`dump-door`**, the door's, `network_mode: none`: its storage key, `./admin`, `./admin-log`, the users folders read-only, `authelia-data`.
- **`backup`**, restic's, alone with a network, its own; no live volume or host folder but `./ops`; `hub-dumps` read-only, tmpfs `/snapshot` and `/state`, `restic-cache`, `hostname: hub-backup` (a stale lock is its own).

**Staging**: the shared tmpfs volume `hub-dumps` (`size=256m,uid=10001,mode=0700`). Plaintext lives on tmpfs alone, but a restore's `restore/`, which the runbook deletes.

**The grid**, UTC: a round at start, then at each slot `now - (now - off) % 900 + 900` (`date -u +%s`), `off` 0 for `queue` and `door`, 300 for `upload`, whose first round waits 60 s. Slot 03:05 (`slot % 86400 = 11100`) prunes; `once` never. No state but the clock and the names: a restart or clock step costs a round at most.

**Generations**: `/dumps/<part>/<t>/`, `t` the round's `date -u +%Y%m%dT%H%M%SZ`, byte order time order; folders 700, files 600. A round stages in `.<t>.<pid>.tmp`, claims `<t>` by `mkdir .<t>.claim.tmp`, then renames; a second round in one second is dropped, exit 1. A part keeps its newest 3, removes `.*.tmp` past 60 minutes (`find -prune -mmin`).
- **`queue`**: per `/projects` folder with a `specengine.toml`, byte order, `spec export state --root /projects/<n> --out <tmp>/<n>.jsonl`. A failed export hard-links the last generation's `<n>.jsonl`, if any, lists `<n>` in `CARRIED`, exit 1. No served project: no generation, exit 1.
- **`door`**: `hub-admin dump <tmp>` ("Door dump"); a failure empties the staging; 13 tries, 12 sleeps of 10 s, then `skipped: <its last line>`, exit 1; a timeout is not retried.
- **`upload`**: `queue`, then `door`, its newest complete generation copied (`cp -Rp`) to `/snapshot/<part>`, then `restic backup --host "$HUB_HOST" --tag <part> /snapshot/<part>`. Not whole: none, one past 35 minutes or with `CARRIED` (still sent, its projects logged), a restic exit but 0. Exit 10 logs `run docker compose run --rm backup init`, never an `init`; 12, `the repository password is wrong`. At 03:05, per part, `forget --host <h> --tag <part> --keep-within 48h --keep-daily 30`, then `prune`. Whole: `/state/ok` touched.

**Exits**: 0 whole; 1 failed or not whole, named; 2 a bad call or setting, before anything. A loop ends on a signal alone (TERM: child ended, staging removed, 143): a setting refused at its start or in a round (`HUB_HOST`, `restic_env`) fails that round with its line, so the service stays up, unhealthy, never restarting; `once` and the acts exit 2 on it. **Lines** open `hub-ops: <mode> <t>: `. No dump blocks anything (ADR-0012).

## Restic

The backend, its two secrets, `restic_env`'s lines, redaction, retention, `backup`'s acts and `fresh`: `hub-restic.md`.

## Door dump

`hub-admin dump DIR`, no terminal; it writes nothing of the door's, nor the act log.
- Exit 2 before anything: not one existing empty `DIR`, or `-`-led.
- The lock, never waited for or broken; held: exit 1, `another act or a dump holds admin-log/lock (taken <time>); if none runs, remove it` (every act's). A stale lock stops door dumps; `fresh` shows it.
- No or empty users file: exit 1.
- It copies by `cat`, each present: `users.yml`, the MCP users file as `mcp-users`, `acts.log`. With `db.sqlite3`: `authelia storage user webauthn export --config C --file DIR/webauthn.yml`, then `totp`'s, each under `timeout` `DUMP_BOUND` (300 s; none: exit 1); exit 1 `the dump timed out` or `the door's passkey|TOTP export failed: <door said>`. stdout: `dumped <files>`.
- **An empty export**, its file left out, is one that exits 1, writes no file and says exactly `Error: no data to export` (no one holds a passkey, or a TOTP); any other failure still refuses the dump, exit 1. `restore` skips a missing file.
- **`C`**, on every storage call (the dump's exports, `restore`'s migration and imports, `reset`'s deletes; `disable` makes no door call): `<tmp>/hub-admin.<pid>.storage.yml`, comment-only, mode 600, written before each call and removed. Without it the CLI loads `/config/configuration.yml`, the image's `X_AUTHELIA_CONFIG`, which neither `admin` nor `dump-door` mounts. Verified on 4.39.28, 2026-11-20 (`hub-backups.md` HB5).

Never dumped: the deploy key, Authelia's secrets (a restore imports under new ones), `traefik-acme`, the index, `proposal-floor`, `.env`.

`hub-admin restore DIR`, on a terminal, into an empty door alone (no non-empty users, MCP or act file, no `db.sqlite3`; exit 1, rechecked under the lock). After `y`: `storage migrate up`, the imports (`-- DIR/<file>`), the users files as `add` installs them, the stamp, the act log as DIR's plus a `restore` line. A refusal or signal names what to remove.

## Restore

1. Have the off-host `.env`, `restic_password`, `restic_env`, the forge and DNS accounts.
2. Install by the guide at the lost host's tag or later: `.env` back, the restic files in `secrets/`, new `authelia_*` secrets, a new deploy key (the lost one deleted at the forge), `known_hosts`; no `admin add`.
3. The files, the door before its first start:
   ```sh
   docker compose run --rm backup snapshots
   install -d -m 700 restore
   docker compose run --rm -v "$PWD/restore:/restore" backup restore /restore   # [<queue> <door>]
   docker compose run --rm -v "$PWD/restore/door:/restore:ro" admin restore /restore
   ```
4. Clone each project (the guide's "Bring a project over" step 2).
5. After `--push`, before the first start: `docker compose run --rm -T --entrypoint git dump-queue -C /projects/<name> log '--format=%(trailers:key=Proposal,valueonly)' | sort -t- -k2n | tail -n 1` per project; if one plus the highest exceeds 100001, write it: `docker compose run --rm --entrypoint sh dump-queue -c 'mkdir -p /data/specengine && printf "%s\n" <N> > /data/specengine/proposal-floor'` (`queue-backup.md#id-floor`).
6. `docker compose up -d traefik authelia specengine`. Per project: `sudo install -o 10001 -g 10001 -m 600 restore/queue/<name>.jsonl projects/`, `docker compose exec -it specengine spec import-state --root /projects/<name> /projects/<name>.jsonl`, `y`, delete it. `CARRIED` names projects from an older dump.
7. Redo every act after `admin log`'s last line, `disable`s first, `mcp rotate`s too; check `admin list`.
8. `docker compose exec specengine spec index --root /projects/<name>`.
9. `docker compose up -d`; DNS to the new host; `sudo rm -rf restore`.

Without `--push` the hub's commits die with the host: a restored `applied` proposal names a lost `applied_commit` and is raised again; with it, only an unpushed sync's. About 20 minutes are lost, IDs issued again (`queue-backup.md` "Known limits"). Same host: once `snapshots` shows it, `down`, remove `authelia-data` and `specengine-data`, then step 3.

## Upgrade

1. `git diff v<old> v<new> -- scripts/deploy`.
2. A dump on the old version, each exit 0: `docker compose run --rm dump-queue queue once`, `... dump-door door once`, `... backup upload once`; note both snapshot IDs (backups off, or no `backup` service: copy the dumps off tmpfs first).
3. `git fetch --tags origin && git checkout v<new>`, `diff .env.example .env` (new optional keys may stay unset), `SPECENGINE_VERSION=<new>`.
4. `docker compose pull`, `up -d`, `ps`: all healthy (`backup` only once backups are on).

Migrations run forward: an older binary refuses the DB. Back: `down`, the old tag and `.env`, remove `specengine-data`, the floor if raised, `up -d traefik authelia specengine`, the noted queue snapshot by Restore step 6, `up -d`; a door pin alike (`authelia-data`, the door snapshot). Every `spec` writing to its repositories runs 0.29.1+ before a hub's first record or create (`spec-ids.md` "Older binaries").

## Push

After the gate (`hub-git.md` "Rules"):
1. `docker compose exec specengine git -C /projects/<name> log --oneline @{u}..`, the first sync's push; a trial's commits: bring the project over again.
2. `ssh-keygen -t ed25519 -N '' -C specengine-hub -f secrets/deploy_key.new`, its `.pub` at the forge with write access, then deleted; `mv` to `secrets/deploy_key`, `chmod 600`, `sudo chown 10001:10001`; the read-only key deleted; `known_hosts` checked.
3. The hub merges into the game's main when diverged, never rebasing or forcing (`hub-git.md` "Merge commit"): for its key lift linear history, required pull requests, reviews, checks, signatures; keep force pushes and deletion forbidden. GitHub: a ruleset bypass for deploy keys (from memory; HO13).
4. `SPECENGINE_PUSH=1` in `.env`, `docker compose up -d`.
5. The log says `push every 5 min`; after a confirm, `git ls-remote origin <branch>` is the hub's `HEAD`.

## Health

- **`specengine`**: `specengine-health`, bash for `/dev/tcp`, every 30 s. Its `GET /` has `Host: 127.0.0.1:<port>` alone. Alive: `HTTP/1.1 403 ` within 5 s (the fence refuses a loopback `Host`: `hub.md` "Fence" step 1), and `127.0.0.1:443` accepting, so a Traefik crash's dead namespace is unhealthy. Ports `SPECENGINE_HEALTH_PORT` (7777), `SPECENGINE_HEALTH_PROXY_PORT` (443); exit 0 or 1, a line.
- **`backup`**: `fresh`. **`admin`, `dump-door`**: `healthcheck: {disable: true}`: the door image's own probes a server they never run.
- **Outside**: `https://<DOOR_HOST>/` answers 200. Docker restarts nothing unhealthy; `docker compose logs dump-queue dump-door backup` say why.

## Auto

ADR-0082, from 0.47.0 (`hub-auto.md`). Older binaries refuse `[auto]`: upgrade the hub first ("Upgrade"), then the laptop's `spec` and plugin, and only then commit and push `[auto]`. In `docker compose exec specengine`: `spec auto show --root /projects/<name> --surface hub` shows the verdicts.
- **Stop**: `spec auto stop` writes `/data/specengine/auto-stop`, holding all projects, no restart; `spec auto resume` (`exec -it`) removes it after `y`. For good: commit the table's removal.
- **Read**: `git -C /projects/<name> log --grep '^Delegated-by: auto'`.
- **Revert one**: `git revert <sha>` on a clone, pushed, which `--push` merges; without it, `git -C /projects/<name> revert --no-edit <sha>`. The proposal stays `applied`.

## Known limits

- The door's storage calls verified on 4.39.28, 2026-11-20 ("Door dump"). Until VF4-VF9 (`docs/features/hub-ops.md`): the images' `find -prune -mmin`, `date -u +%s`, `awk` `ENVIRON`, `tr`, `ls -A`, restic's exits 10, 12; redaction tried in BSD awk alone.
- tmpfs can be swapped: plaintext stays off the host's disk only without swap.
- `fresh` turns unhealthy about 40 minutes after a part's last generation (busybox `-mmin +35` is 36, plus the check's 5).
- A timeout is read by its exit: an OOM kill reads `timed out`; a door exiting 1 on TERM is retried (VF5).
- A claim `mkdir` failing for space reads `claimed by another round`; a CRLF empty line is `not KEY=value`; the probe's connect waits Docker's 10 s.

## Tests

http `tests/deploy_ops.rs` (under `sh`, dash and bash, with stubs), `deploy_admin.rs`, `deploy.rs`: OP1-OP23 (`docs/features/hub-ops.md`). CI `image.yml`: C5, the images' CLIs and tools; C4, the built image's probe.
