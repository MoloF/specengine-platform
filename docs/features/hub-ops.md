---
class: spec
status: shipped
shipped: 2026-11-04
scope: [scripts/deploy, .github/workflows, crates/specengine-http, docs]
ref: RM-0074; Phase 7 step 11b-2, release v3's last code item; Q5, S1-S12 and the later decisions are the orchestrator's working answers, awaiting the owner's review
adrs: [ADR-0071]
---

# hub-ops: off-host dumps by restic, restore, upgrade, health and turning on `--push` (RM-0074)

## Why

After 11b-1 and 11c-1 a hub runs, but its queue and its accounts live on one VPS. Losing the VPS loses every open proposal, card and task, and makes everyone enrol again. A hung daemon shows nothing. An upgrade has no order, and `--push` no procedure. This step adds what leaves the host, when, and how it comes back; how a release moves; how health shows; how the hub starts pushing to the game's main. After it comes only the owner's gate 11b-3: HO1-HO17 below join H1-H17 (`hub-deploy.md`), G1-G10 (`hub-accounts.md`) and SG1-SG2 (`stream-life.md`). No crate changes: `spec export state`, `spec import-state`, the door's CLI and restic, each in a container of its own image; the probe is a shell's.

### Decisions

Now: `docs/canon/hub-ops.md` (ADR-0071).
- **S1** restic, to any backend it reaches by environment credentials, `secrets/restic_env` and `secrets/restic_password`, never `.env` (it holds no key: D10); SFTP and rclone out. **S2** Three sidecars on their tools' images, two with no network; rejected: a loop in the entrypoint, one networked sidecar mounting live data. **S3** The door by Authelia's own exports under `hub-admin`'s lock; `sqlite3 .backup` the fallback if VF4 finds none, `VACUUM INTO` in a crate rejected (ADR-0068). **S4** The deploy key is never dumped. **S5** A sh loop per sidecar on a 15-minute UTC grid, staging on tmpfs. **S6** Two tagged series per host, 48 hours plus 30 daily, `init` and `check` by hand. **S7** The probe: bash `/dev/tcp`, the fence's loopback 403 plus port 443; a `spec` verb, curl, wget and git's `remote-http` rejected. **S8** Restore into an empty door and empty queues, the dumpers last. **S9** Upgrade after a dump; back by that dump. **S10** `--push` after the gate, merge commits to the game's main. **S11** The release stays `v0.29.1`; no version moves. **S12** The canon in `docs/canon/hub-ops.md`, which no role could write as `scripts/deploy/OPERATIONS.md`.
- **At implementation**: `healthcheck: {disable: true}` on `dump-door` and `admin`; 13 tries and 12 sleeps; no served project, no generation, exit 1; an empty door dump is sent; a stale or `CARRIED` generation is sent, the round not whole; `upload` waits 60 s first; short values redacted strictly; a duplicate `restic_env` key refused; `HUB_HOST` needed by `upload`, `snapshots` and a `latest` restore.
- **Review iteration 2**: a refused key is named only as an upper-case shell name (stricter than asked: a mixed-case base64 secret cut at `=` is a shell name); redaction longest first; `fresh` also fails on a part's newest generation past 35 minutes; `DUMP_BOUND` 300 s under the door image's `timeout`, a timed-out dump not retried; the claim by `mkdir`; `cp -Rp`; a CR refusal; a door with no users file, exit 1; a stopped restore names what it left. Not taken: a connect timeout in `health.sh` (the result is already unhealthy).

## Description and interactions

Now: `docs/canon/hub-ops.md`; the setup: `scripts/deploy/README.md`.

## Data

The files are the truth; the canon describes them.

### Files

The table in "Implementation".

### Compose

Canon "Dumps", "Health", "Pins".

### Secrets

Canon "Restic".

### Staging, snapshots, restore

Canon "Dumps".

### `hub-ops`

Canon "Dumps" (**The grid**, **A `queue` round**, **A `door` round**, **An `upload` round**, **Lines**), "Restic" (**`restore`**).

### `hub-admin`: `dump` and `restore`

Canon "Door dump".

### `specengine-health`

Canon "Health".

### Image and CI

Canon "Health", "Tests".

## Rules and edge cases

**R1** nothing blocks; **R2** no live write; **R3** whole, claimed generations; **R4** plaintext on tmpfs; **R5** no secret logged; **R6** no identity header; **R7** `backup` mounts nothing live; **R8** the lock never broken; **R9** no merge; **R10** UTC; **R11** removed projects drop out; **R12** carry; **R13** `--push` after the gate; **R14** a `--host` per hub.

## Acceptance criteria

Tests: http `tests/deploy_ops.rs` (new; compose and Dockerfile read as `deploy.rs` reads them; `hub-ops` under `sh` with stubs `spec`, `restic`, `date` and `sleep` first on `PATH`, plus the real `hub-admin` with `deploy_admin.rs`'s stub `authelia`; `health.sh` under `bash` against `std::net::TcpListener`s on free ports, through its two variables); `tests/deploy_admin.rs` (OP11-OP15, a terminal by `script(1)`); `tests/deploy.rs` (OP6, OP23). M marks the mutations that must turn each red.

- [x] OP1 -- Each new service has exactly the keys, mounts, secrets and environment in "Compose". `dump-queue`'s image is byte for byte `specengine`'s, and `dump-door`'s is `authelia`'s. `backup` has `restic/restic:<x.y.z>@sha256:<64 hex>`. No new service has `ports`, `expose`, `privileged`, `cap_add`, `pid`, `ipc`, `devices` or a `service:` or `host` network. Amended at ship: `dump-door` and `admin` carry exactly `healthcheck: {disable: true}`, the door image's own `HEALTHCHECK` off. M: `network_mode: service:traefik` on `dump-queue`; `backup` on `default`; `/projects` writable in `dump-queue`; `restic_env` granted to `dump-door`; `hub-dumps` writable in `backup`; `init` dropped from one; `restic/restic:latest`.
- [x] OP2 -- `hub-dumps` is tmpfs (`driver_opts` `type` and `device` `tmpfs`, `o` holding `size=` and `mode=0700`). Network `backup` holds `backup` alone. The two secrets are as in "Compose". `.env.example` gains no key and still passes D10. `.gitignore` holds `restore/`. M: `hub-dumps` a plain volume; `authelia` on `backup`; `RESTIC_REPOSITORY` in `.env.example`; `restore/` dropped.
- [x] OP3 -- The trust items stay:
  - `specengine` is alone in Traefik's namespace.
  - No `docker.sock` anywhere.
  - `deploy_key` and `known_hosts` go to `specengine` alone; `authelia_storage_key` to `authelia`, `admin` and `dump-door` alone; the restic secrets to `backup` alone.
  - `backup` mounts none of `specengine-data`, `authelia-data`, `./projects`, `./authelia/users`, `./traefik/mcp-users`, `./admin-log`.

  M: `specengine-data` into `backup`; the socket into `backup`; `deploy_key` to `dump-queue`.
- [x] OP4 -- `specengine`'s healthcheck is as in "Compose". The Dockerfile copies `health.sh` as `specengine-health` with `--chmod=0755`, sets no `HEALTHCHECK` and installs no `curl` or `wget`; the dockerignore keeps the file. The probe's two defaults equal the entrypoint's port and `websecure`'s address. M: no `COPY`; `curl` installed; the default 7778; the proxy default 80.
- [x] OP5 -- `image.yml`'s check job runs `--test deploy_ops`, and C4 and C5 are steps of their jobs. M: the test dropped.
- [x] OP6 -- No version moves: `.env.example`'s `SPECENGINE_VERSION` equals `plugin.json`'s version, 0.29.1; `tag-check.sh` accepts `v0.29.1` and refuses `v0.29.2`; `image.yml`'s check job runs it as its first step. M: `.env.example` at 0.29.2; `plugin.json` alone bumped; the `tag-check.sh` step moved after the tests.
- [x] OP7 -- The probe:
  - exit 0 for a daemon listener answering `HTTP/1.1 403 Forbidden` with the proxy listener open;
  - exit 1, with its line, for: a `200`; a `502`; a close with no answer; the proxy port closed while the daemon answers 403; no listener;
  - exit 1 after about 5 s for a listener that accepts and never writes;
  - the request it sends has `Host: 127.0.0.1:<port>` and none of `Remote-User`, `Origin`, `Sec-Fetch-Site`.

  M: any status accepted; the proxy test dropped; `Host: hub.example.org`; a `Remote-User` sent; no `-t`.
- [x] OP8 -- `hub-ops queue once` over projects `a`, `a-1`, `c` (with `b` holding no `specengine.toml`, and a file `z`) calls the stub `spec` exactly as "A `queue` round" does, in byte order `a`, `a-1`, `c`, never for `b` or `z`. It leaves one generation, matching `^[0-9]{8}T[0-9]{6}Z$`, with mode 700 and files of mode 600, and no `.tmp`. M: the `*/` glob; `--out` inside a project; no rename; no `umask`.
- [x] OP9 -- When `spec` fails for `a-1` (exit 2, one stderr line), the generation holds the new `a.jsonl`, the previous generation's `a-1.jsonl` byte for byte, and `CARRIED` reading `a-1`; the log names `a-1` and the line; the exit is 1. With no previous generation, `a-1.jsonl` is absent and `CARRIED` lists it. M: no carry; exit 0; no `CARRIED`.
- [x] OP10 -- After five `once` rounds (stub `date` advancing), the newest three generations remain. A `.tmp` dated two hours back is removed; one dated ten minutes back is kept. M: keep all; a young `.tmp` removed.
- [x] OP11 -- `hub-ops door once` with the real `hub-admin`:
  - the generation's `users.yml`, `mcp-users` and `acts.log` equal their sources byte for byte;
  - `webauthn.yml` and `totp.yml` are the stub's;
  - the stub saw exactly the two export calls of "`hub-admin`: `dump` and `restore`";
  - the act log is unchanged, and the lock is gone.

  Without `db.sqlite3`, there is no CLI call and the three files alone. M: copy before the lock; log the dump; skip an export; call the CLI with no database.
- [x] OP12 -- The lock:
  - With `admin-log/lock/` held, `door once` makes 13 tries and 12 sleeps (stub `sleep` called 12 times with 10) and leaves no generation; the lock and its `taken` stay byte for byte; exit 1.
  - While a `dump` holds the lock (stub export held), `hub-admin disable mira` is refused with the new message.
  - TERM to `door` mid-export leaves no lock and no staging.

  M: the lock broken; no retry; no trap; the old message.
- [x] OP13 -- `hub-admin dump`:
  - exit 2, nothing locked or called, for: no argument; two arguments; a missing, non-empty or `-`-led `DIR`;
  - a failing export: exit 1, the lock released, no act log line;
  - with stdin `/dev/null`, it still runs.

  M: a non-empty `DIR` accepted; a terminal required; `stopped_log` on `dump`.
- [x] OP14 -- `hub-admin restore` into an empty door, on a terminal, answered `y`:
  - the stub saw `migrate up`, `webauthn import`, `totp import` in that order, with DIR's files after `--`;
  - then `users.yml` and the MCP users file are installed byte for byte, each with a new inode and mode 600;
  - the stamp went up;
  - `acts.log` is DIR's bytes plus one `restore` line.

  Answered `n`: exit 1, nothing called. M: the users file before the imports; no stamp; the log without the `restore` line, or without DIR's lines.
- [x] OP15 -- `restore` refusals:
  - exit 2 for: stdin a pipe; no `DIR/users.yml`;
  - exit 1, nothing called or written, for each of a non-empty users file, MCP users file or act log, and an existing `db.sqlite3`, each path named;
  - a `DIR/users.yml` in an unknown layout: exit 1;
  - a failing import: exit 1, both users files untouched, the database named.

  M: the database check dropped; a non-empty users file accepted.
- [x] OP16 -- `hub-ops upload once`, given fresh complete `queue/<t1>` and `door/<t2>`:
  - `/snapshot/queue` and `/snapshot/door` hold their bytes;
  - the stub `restic` saw `backup --host hub.example.org --tag queue /snapshot/queue`, then the same for `door`;
  - its environment holds each `restic_env` key with its value, and `RESTIC_PASSWORD_FILE`;
  - `/state/ok` is touched; there is no `init`, `forget` or `prune`.

  M: back up `/dumps` itself; `init` on exit 10; skip the copy.
- [x] OP17 -- An upload round that is not whole leaves `/state/ok` untouched and exits 1, naming why:
  - a door generation 40 minutes old: the queue is still sent;
  - a `CARRIED` file;
  - `restic` exiting 10 (the `init` line, and no `init`) or 12;
  - no generations at all (no `backup` call).

  `hub-ops fresh` exits 0 for an `ok` 10 minutes old and 1 for 40, or for none. M: `ok` on a stale part; `fresh` always 0; 350 minutes for 35.
- [x] OP18 -- A loop round at slot 03:05 UTC (stub `date`) runs, after both backups, `forget --host <h> --tag queue --keep-within 48h --keep-daily 30`, then the same for `door`, then `prune`. The slot 03:20 runs none of them, and neither does `once`. M: `--prune` in `forget`; `--keep-daily 300`; a prune every round.
- [x] OP19 -- `restic_env`:
  - exit 2 naming the line, and the key only when an upper-case shell name (`[A-Z_][A-Z0-9_]*`, at most 64 B), no value printed, `restic` never called, for: `PATH=/x`; `LD_PRELOAD=x`; `RESTIC_PASSWORD=x`; `export AWS_X=y`; `aws_x=y`; a line with no `=`; no `RESTIC_REPOSITORY`; two of them;
  - comments and empty lines are skipped;
  - a value holding `$HOME`, spaces, `'` and `"` reaches `restic` verbatim.

  M: the file sourced with `.`; the value printed; `RESTIC_PASSWORD` accepted.
- [x] OP20 -- The acts:
  - `init` runs `restic init` once; `snapshots` and `check` run as in "`hub-ops`";
  - `restore /r` runs `restore latest:/snapshot/queue --host <h> --tag queue --target /r/queue`, then the same for door; `restore /r Q D` runs `restore Q:/snapshot/queue --target /r/queue`, with no filter;
  - exit 2 for: a missing or non-empty `DIR`; an unknown mode; an extra argument.

  M: `restore latest` with no subfolder; a non-empty `DIR` accepted.
- [x] OP21 -- The loops, with stub `date` fixed at epoch E and a stub `sleep` that records each argument and sends TERM to its parent at the call the test names:
  - `queue` ran one round, then asked for `900 - E % 900` (TERM at the first call);
  - `upload` asked for 60 (its first round's start wait, `docs/canon/hub-ops.md` "Dumps"), ran one round, then asked for `900 - (E - 300) % 900` (TERM at the second call);
  - TERM during a round's export (a stub `spec` that waits) left no `.tmp`, and the exit was 143.

  M: no round at start; a fixed `sleep 900`; no trap.
- [x] OP22 -- OP8-OP21 pass under the laptop's `/bin/sh`, and on CI under `dash`; `health.sh` under bash 3.2 (macOS) and 5. M: a `[[` or `${v:0:1}` in `hub-ops` (dash rejects both).
- [x] OP23 -- 11b-1's D7, D9, D11 and D14 and 11c-1's AC8 and AC18 (three services on the door's image, one C3 line) are rewritten as these decided changes, and the rest of `deploy.rs` and `deploy_admin.rs` pass unchanged:
  - seven services, the new images pinned;
  - each secret granted per OP3;
  - three scripts copied into the image;
  - the lock's new message.
- [x] OP24 -- `export index` and `check` are green; the guide stays within Tier 1, `docs/canon/hub-ops.md` within 12 288 B, `hub-deploy.md` within its cap.

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
  - nothing of it on the host's disk, on a host without swap (tmpfs can be swapped).
- [ ] VF7 -- The restic image:
  - busybox `sh`, `cp -Rp`, `find -prune -mmin`, `date -u +%s`;
  - `restore <id>:/snapshot/queue`;
  - exits 10 and 12;
  - the cache volume;
  - a stale lock after a killed `prune`, with the fixed `hostname`.
- [ ] VF8 -- Debian's bash in the image opens `/dev/tcp`; `docker compose ps` shows `healthy`, then `unhealthy` after a `kill -STOP` of the daemon.
- [ ] VF9 -- `docker compose run -v "$PWD/restore:/restore"` mounts for `backup` and `admin`, and `run --rm` of a looping service runs `once` and leaves nothing.

The gate, step 11b-3, by hand: the owner's session. Findings go in `hub-deploy.md` "Gate".
- [ ] HO1 -- Install with the backend, `backup init`, `up -d`: six services up, and `specengine` and `backup` healthy within 20 minutes.
- [ ] HO2 -- Two rounds land off the host on the grid: `backup snapshots` shows both tags at 5 minutes past a quarter. The backend's own listing holds only restic's encrypted objects.
- [ ] HO3 -- `docker compose logs dump-queue dump-door backup` hold no secret, password, digest or TOTP. `hub-dumps` is tmpfs (`findmnt` in a dumper), and the host's disk holds no dump (on a host without swap).
- [ ] HO4 -- (G10) A dump while an act holds the lock retries or skips; an act during a dump is refused with the new message; the next snapshot restores whole files.
- [ ] HO5 -- A wrong key in `restic_env`, with `backup` restarted, turns `backup` unhealthy within about 40 minutes; it is healthy again a round after the fix. A stopped `dump-door` does the same.
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

SFTP, rclone, append-only setup, a `git bundle`, alerts, log rotation, any crate change, the gate 11b-3.

## Implementation

Review: 1 REWORK (`refused_key` logged a bad line's text before `=`); 2 ACCEPT, nits accepted. Mutations: 34 named, all killed: OP1-OP23 (OP7 in two forms) and I1-I10 for iteration 2; I2 (redaction in file order) first survived, killed by a new test that a covered run is named by its longest value. OP24 is the clean check. `deploy`, `deploy_admin`, `deploy_ops`: 90 of 90.

| Module | What it does |
|---|---|
| `ops/hub-ops` (new) | `read_env`, `is_name`, `refused_key`, `last_line`; `finish_round`; `queue_round`, `door_round`, `upload_round`, `loop`; `act_fresh` |
| `admin/hub-admin` | `act_dump`, `dump_failed`, `DUMP_BOUND`; `act_restore`, `door_empty`, `restore_refuse` |
| `health.sh` (new) | `specengine-health` |
| compose, `Dockerfile`, dockerignore, `.gitignore`, `image.yml` | the services; the probe; `restore/`; C4, C5 |
| http `tests/deploy_ops.rs`, `deploy_admin.rs`, `deploy.rs` | OP1-OP23 |

Deviation: C5 also checks each image's tools. Spec defects, fixed: OP23 lacked AC18; OP12 counted sleeps as tries; the 60 s wait was in OP21 alone; VF7 lacked `-prune`. Not covered: C4, C5 (CI); VF4-VF9, HO1-HO17 (the owner's); busybox `ash` (VF7); a real `import-state` of a dump (HO6). Limits: the canon's.

### Canon at ship

New `docs/canon/hub-ops.md` and shard `docs/index-canon-hub.md` (`docs/canon/hub*.md`), so the tests pinning the shard set changed: store `parity_config/mod.rs`, `check_parity.rs`, CLI `shards_repo.rs`, `parity.rs`. Pins moved in from `hub-deploy.md`.
