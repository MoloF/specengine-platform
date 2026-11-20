---
class: spec
status: draft
scope: [scripts, crates/specengine-http]
ref: RM-0130; the orchestrator's findings at the hub's upgrade to 0.47.2, 2026-11-20 (08 s2 Phase 7, "Ops findings, 2026-11-20"); the owner's standing rule that a stability finding is a task with its evidence
adrs: []
---

# hub-backups: the door's dump stops failing, and `backup` waits instead of restart-looping (RM-0130)

## Why

On 2026-11-20 the hosted hub was upgraded to 0.47.2. Its logs showed that the off-host backups designed by ADR-0071 had never run:
- `dump-door` fails every 15-minute round with this line (the error appears twice in it): `hub-ops: door <t>: skipped: hub-admin: the door's passkey export failed: Error: failed to load configuration from file path(/config/configuration.yml) source: stat /config/configuration.yml: no such file or directory`. So the door's users, passkeys and TOTP are not dumped at all.
- `backup` restarts in a loop, logging `hub-ops: upload <t>: restic_env: no RESTIC_REPOSITORY line: name the repository`. So no snapshot has left the host.
- Only `dump-queue` works. Beside it, the host keeps the copies taken by hand before each upgrade (`~/hub-snapshots/`).

**The causes, read from the committed files:**
1. **The door's CLI finds no configuration.** `hub-admin dump` calls `authelia storage user webauthn export --file … --sqlite.path …` and `… totp export …` with no `--config`. The pinned image then reads `/config/configuration.yml`, and `dump-door` mounts nothing there (`scripts/deploy/docker-compose.yml`): only `authelia` mounts `./authelia/config:/config:ro`. The `admin` service mounts no `/config` either, so its own storage calls carry the same latent defect: the `webauthn` and `totp` deletes of `reset` and `disable`.
2. **The loop exits on a setting.** `hub-ops upload` checks `restic_env` before its loop (`read_env || exit 2`, `scripts/deploy/ops/hub-ops`). A bad file ends the process, `restart: unless-stopped` starts it again, and the loop never runs. That is a restart loop: a crash-class defect under the owner's rule. The rounds themselves already fail softly ("a failed round is logged and the next slot runs").

The repository and its credentials are the owner's choice (`hub-ops.md` "Restic"). **His word, 2026-11-20:** off-host backups stay off for now ("not required, don't worry about it, let it stay off"). So this slice:
- makes the sidecars correct;
- makes an unconfigured `backup` wait visibly, one line per round, never restart-looping;
- writes into the runbook the steps for when he turns backups on.

## Description and interactions

- **`dump-door`** dumps the door every round: `users.yml`, `webauthn.yml`, `totp.yml`, the MCP users file and the act log, as `hub-ops.md` "Door dump" says.
- **`admin`**'s storage acts load the same configuration, so `reset` and `disable` delete a person's passkeys and TOTP for real.
- **`backup`** stays up with a missing or bad setting. It logs one line at start and one per round, never calls restic, and reports unhealthy (`fresh` fails) until the operator fixes the setting. The next round reads the fixed file; no restart is needed.
- **`once` and the acts** (`init`, `snapshots`, `check`, `restore`) keep exit 2 on a bad setting: they are the operator's commands, and a loud failure is what he wants there.
- **The operator's guide** (`scripts/deploy/README.md`) says what the owner does: choose a backend, write the two secrets, keep them off the host, `init` once, check the snapshots, and run a restore drill.

**No crate changes.** The fix lives in `scripts/deploy/`: the compose file, `ops/hub-ops`, and, if option A holds, `admin/hub-admin`. Tests are in `crates/specengine-http/tests/deploy*.rs`, as today.

**The release.** 0.48.1, after 0.48.0. The fix waits as a patch until then. The hub needs a new image. A hub image's tag must equal the plugin's version (`tag-check.sh`), so the plugin takes a version-only PATCH bump on top of the version current when this ships (`plugin-files.md` "Changing it").

**No ADR.** ADR-0071 decided the sidecars, what they dump and that only `backup` has a network; none of that changes. How a loop treats a refused setting is canon detail (`hub-ops.md` "Exits"), changed in the same change. Option B, if needed, gives `dump-door` and `admin` two more door secrets, a hardening change recorded in `hub-deploy.md`. It does not reverse ADR-0071: no secret leaves the host.

## Data

### The door's configuration: options A and B

The developer first tries **A** on the pinned image `authelia/authelia:4.39.28@sha256:bd97cff4…` (HB5), and takes **B** only if A fails. The run, and why, go in "Implementation".

| | A (preferred) | B (fallback) |
|---|---|---|
| What | `hub-admin` writes a minimal configuration and passes it to every storage call as `--config "$tmp_dir/storage.yml"` | `dump-door` and `admin` mount `./authelia/config:/config:ro` and carry the door's configuration environment |
| The file | written fresh (umask 077) under `hub-admin`'s `tmp/`, holding only what a storage command needs; removed on exit | the committed `configuration.yml` |
| Secrets | none new: the storage key is already `AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE` | adds `authelia_session_secret` and `authelia_jwt_secret`, with `X_AUTHELIA_CONFIG_FILTERS: template`, `HUB_DOMAIN`, `HUB_HOST`, and the three `*_FILE` variables as `authelia` has them |
| Files changed | `admin/hub-admin` (its `door_run` storage calls) | `docker-compose.yml` (two services) |

**Under either option:**
- `network_mode: none` stays on `dump-door`.
- `admin`'s network does not change.
- No other mount, secret or environment is added.

### The loop: `hub-ops upload`

```sh
upload)
    need_host_or_note       # loop: log and go on; once: exit 2 as today
    read_env_or_note        # loop: log and go on; once: exit 2 as today
    ...
```

**In loop mode** (no `once`):
- A refused `HUB_HOST` or `restic_env` at start is logged, as the line `need_host` or `read_env` writes today, and the loop starts.
- Each round reads the settings anew (`upload_round` already calls `read_env`, and checks `HUB_HOST` the same way). A refused setting fails the round softly, with one line, and runs no restic. `/state/ok` is not touched, so `fresh` fails and the service is unhealthy.
- A TERM still ends it with 143.

**In `once` mode and the acts:** exit 2, unchanged.

**The `hub-ops.md` "Exits" sentence** changes from "A loop ends on 2 or a signal" to: "A loop ends on a signal alone: a setting refused at its start or in a round (`HUB_HOST`, `restic_env`) fails that round with its line, so the service stays up, unhealthy, never restarting; `once` and the acts exit 2 on it."

### The test double for the door's CLI

The `authelia` stub in `tests/deploy_admin.rs` gains the pinned image's loading rule, as recorded from the hub's log. A `storage …` call fails, exit 1, printing exactly `Error: failed to load configuration from file path(<p>) source: stat <p>: no such file or directory`, unless one of these holds:
- under A, its `--config <p>` names an existing file;
- under B, `<HUB_ADMIN_ROOT>/config/configuration.yml` exists.

Here `<p>` is `--config`'s value, else `/config/configuration.yml`. The rule is recorded as observed, and HB5 checks it against the real image.

## Rules and edge cases

- WHEN `restic_env` is missing, unreadable, refused at a line, or names no `RESTIC_REPOSITORY`, `backup` SHALL stay up, log one line per round, call no `restic`, and fail `fresh`.
- WHEN the operator fixes `restic_env`, the next round SHALL upload, with no restart.
- WHEN `upload once`, `init`, `snapshots`, `check` or `restore` meet a refused setting, they SHALL exit 2 as today.
- WHEN the door holds no `db.sqlite3`, the dump SHALL copy the users files and the act log alone, as today.
- No line SHALL print a value of `restic_env` or a secret (R5 unchanged).
- Nothing in this slice SHALL block a hub's request (ADR-0012): the dumps stay sidecars.

## Acceptance criteria

- [ ] HB1 — **The stub reproduces the defect.** With the stub's loading rule, the new door-dump test on the base commit's `hub-admin` and compose layout fails with exactly the hub's line. This proves the tests see what the hub saw.
- [ ] HB2 — **The door's dump.**
  - `hub-admin dump DIR` (deploy_admin), with a users file of one user and a door database holding one passkey and one TOTP (the stub's exports write the files they are asked for), exits 0.
  - It writes `users.yml`, `webauthn.yml`, `totp.yml`, `mcp-users` and `acts.log`, and prints `dumped …`.
  - Every storage call passes the loading rule.
  - Mutation: under A, `--config` removed; under B, the mount removed. Either turns it red.
- [ ] HB3 — **A `door once` round.** `hub-ops door once` (deploy_ops), with the real `hub-admin` and the stub, exits 0, and `dumps/door/<t>/` holds the five files. Mutation as HB2.
- [ ] HB4 — **The compose pin.**
  - `dump-door` and `admin` carry exactly the chosen option's mounts, environment and secrets (OP-style YAML reading); under A, no session or JWT secret; `network_mode: none` on `dump-door`.
  - Mutation: the option's addition removed turns it red.
- [ ] HB5 — **[docker, `#[ignore]`] The pinned image agrees.**
  - On a machine with Docker, the pinned `authelia` image runs `storage migrate up` on a scratch SQLite, then `storage user webauthn import` of `fixtures/hub-backups/webauthn.yml` (one invented credential), then the exact export call `hub-admin` makes. It exits 0 and the exported file holds that credential.
  - Under A, without `--config`, the same export fails with the recorded line.
  - The run is in "Implementation".
- [ ] HB6 — **`admin`'s deletes.** `reset` and `disable` under the stub's loading rule exit 0 and their storage deletes pass it. They would have failed before, so this is a latent defect closed.
- [ ] HB7 — **No restart loop.**
  - `hub-ops upload`, loop mode, under the stub clock (deploy_ops), with `restic_env` naming no `RESTIC_REPOSITORY`: no exit over three slots; one `restic_env: no RESTIC_REPOSITORY line: name the repository` line at start and one per round; no `restic` call; `state/ok` absent; `hub-ops fresh` exits 1. A TERM gives 143.
  - The same holds with `restic_env` unreadable, and with `HUB_HOST` unset.
  - `upload once` still exits 2 with the line.
  - Mutation: `read_env || exit 2` restored before the loop turns it red.
- [ ] HB8 — **Fixed in place.** With the loop running as in HB7, `restic_env` is written whole between two rounds. The next round calls `restic backup --host <h> --tag queue` and `--tag door`, and touches `state/ok`; `fresh` exits 0.
- [ ] HB9 — **Nothing else moves.** `tests/deploy.rs`, `deploy_ops.rs` and `deploy_admin.rs` pass, their pins changed only where this slice changes the files. The image workflow's checks (`image.yml`, `tag-check.sh`) pass. `hub-ops.md` "Exits" says the new sentence.
- [ ] HB10 — **The runbook.** `scripts/deploy/README.md` "Install" and "Going live" say, within 10 240 B, that `backup` waits unconfigured, one line a round, until the owner turns backups on. Then they say what he does:
  - choose a restic backend;
  - write `secrets/restic_env` (`RESTIC_REPOSITORY` and its keys) and `secrets/restic_password`, kept off the host;
  - run `docker compose run --rm backup init` once;
  - after the next round, `docker compose run --rm backup snapshots` lists `queue` and `door` under 35 minutes old, and `docker compose ps` shows `backup` healthy;
  - a restore drill on a scratch host (`hub-ops.md` "Restore").
- [ ] HB11 — **[live hub, after the upgrade to 0.48.1]**
  - `docker compose logs dump-door` shows `dumped users.yml … webauthn.yml totp.yml` each round.
  - `backup` is up and unhealthy with one line a round, with no restart, in `docker compose ps` and its logs.
  - **When the owner turns backups on** (his word of 2026-11-20 keeps them off for now): `backup` becomes healthy, `snapshots` lists both tags, and a restore drill on a scratch host reads both back.

## Out of scope

- Choosing or paying for a backend; the owner's credentials.
- Off-host copies of the deploy key, Authelia's secrets or `traefik-acme` (ADR-0071: never dumped).
- A change to the dumps' grid, retention or format.
- Any crate change.
- Monitoring beyond the healthcheck and the lines.

## Implementation

To be filled in after implementation. The plan:

| Lane | Files |
|---|---|
| `rust-developer` | `scripts/deploy/admin/hub-admin` (A), or `scripts/deploy/docker-compose.yml` (B); `scripts/deploy/ops/hub-ops` (the loop); `fixtures/hub-backups/webauthn.yml` (an invented credential); `plugin/` version-only at release |
| `test-engineer` | `crates/specengine-http/tests/deploy_admin.rs` (the stub's loading rule, HB1, HB2, HB6), `deploy_ops.rs` (HB3, HB7, HB8), `deploy.rs` (HB4, HB9), the `#[ignore]` HB5 |

**Order:** HB5's run on the pinned image picks A or B; then the fix; then the tests.

**Canon at ship:**
- `hub-ops.md`: "Exits", "Door dump" (`--config` under A);
- `hub-deploy.md`: the services' mounts and secrets under B;
- `scripts/deploy/README.md` (HB10);
- 08's "Ops findings" paragraph closed, with what the owner still owes.
