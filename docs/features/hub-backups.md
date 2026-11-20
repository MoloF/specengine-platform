---
class: spec
status: shipped
shipped: 2026-11-20
scope: [scripts, crates/specengine-http]
ref: RM-0130; the orchestrator's findings at the hub's upgrade to 0.47.2, 2026-11-20 (08 s2 Phase 7, "Ops findings, 2026-11-20"); the owner's standing rule that a stability finding is a task with its evidence
adrs: []
---

# hub-backups: the door's dump stops failing, and `backup` waits instead of restart-looping (RM-0130)

## Why

At the hub's upgrade to 0.47.2, 2026-11-20, its logs showed that ADR-0071's off-host backups had never run:
- `dump-door` failed every round, the error twice in one line: `hub-ops: door <t>: skipped: hub-admin: the door's passkey export failed: Error: failed to load configuration from file path(/config/configuration.yml) source: stat /config/configuration.yml: no such file or directory`. `hub-admin`'s storage calls passed no `--config`, and neither `dump-door` nor `admin` mounts that file.
- `backup` restarted in a loop: `hub-ops upload` exited 2 before its loop on a `restic_env` with no `RESTIC_REPOSITORY`, and `restart: unless-stopped` ran it again.

The owner's word, 2026-11-20: off-host backups stay off for now ("not required, don't worry about it, let it stay off"). So the slice fixed the sidecars, made an unconfigured `backup` wait visibly, and wrote his steps for turning backups on.

## Data

Canon now: `docs/canon/hub-ops.md` "Dumps" (Exits), "Door dump", "Known limits"; `hub-restic.md`, the backend, moved out of `hub-ops.md` for its cap; `hub-deploy.md` "Gate"; `scripts/deploy/README.md` "Install", "Going live".

### The door's configuration: options A and B

A was taken (why: "Implementation"); B, mounting the door's `configuration.yml` and its secrets, was not needed. Every storage call passes `--config <tmp>/hub-admin.<pid>.storage.yml`: per process, comment-only, mode 600, written before each call and removed. The calls are the dump's exports, `restore`'s migration and imports, and `reset`'s deletes; `disable` makes no door call. `dump-door` and `admin` keep their mounts, environment and secrets.

### The loop: `hub-ops upload`

A loop says a refused `HUB_HOST` or `restic_env` at start and each round, runs no restic and touches no `/state/ok`: `backup` stays up, unhealthy, and takes a fixed `restic_env` at the next round. `once` and the acts keep exit 2; TERM gives 143.

### The test double for the door's CLI

The `authelia` stub in `tests/deploy_admin.rs` loads a configuration as 4.39.28 does: a `storage …` call whose `--config <p>` (else `/config/configuration.yml`) is no file exits 1, printing `Error: failed to load configuration from file path(<p>) source: stat <p>: no such file or directory` twice in one line.

## Rules and edge cases

- WHEN a setting is refused in loop mode, `backup` SHALL stay up, log its line(s) per round, call no `restic` and fail `fresh`, and SHALL take a fixed `restic_env` at the next round with no restart; `upload once` and the acts SHALL exit 2.
- WHEN an export exits 1, writes no file and says exactly `Error: no data to export`, the dump SHALL leave that file out; any other failure SHALL refuse the dump, exit 1. `restore` already skips a missing file.
- WHEN the door holds no `db.sqlite3`, the dump SHALL copy the users files and the act log alone.
- No line SHALL print a secret; nothing SHALL block a hub's request (ADR-0012).

## Acceptance criteria

- [x] HB1 — **The stub reproduces the defect.** The base commit's `hub-admin` export, under the stub's loading rule, fails with exactly the hub's line.
- [x] HB2 — **The door's dump.** `hub-admin dump DIR` over one user, a passkey and a TOTP exits 0, writes `users.yml`, `webauthn.yml`, `totp.yml`, `mcp-users`, `acts.log` and prints `dumped …`; every storage call passes the loading rule; an empty table's export is left out. Mutation: `--config` removed turns it red.
- [x] HB3 — **A `door once` round.** `hub-ops door once` with the real `hub-admin` and the stub exits 0, `dumps/door/<t>/` holding the five files. Mutation as HB2.
- [x] HB4 — **The compose pin.** `dump-door` and `admin` carry exactly their earlier mounts, environment and secrets: no `/config`, no session or JWT secret; `network_mode: none` on `dump-door`.
- [x] HB5 — **[docker, `#[ignore]`] The pinned image agrees.** The developer's run in "Implementation" is the evidence.
- [x] HB6 — **`admin`'s storage calls.** `reset`'s deletes and `restore`'s migration and imports pass the loading rule; a latent defect closed.
- [x] HB7 — **No restart loop.** `hub-ops upload`, loop mode, with no `RESTIC_REPOSITORY`, an unreadable `restic_env` or `HUB_HOST` unset: no exit over three slots, its line(s) at start and each round, no `restic`, no `state/ok`, `fresh` exits 1, TERM 143; `upload once` exits 2. Mutation: `read_env || exit 2` restored before the loop turns it red.
- [x] HB8 — **Fixed in place.** `restic_env` written whole between two rounds: the next round runs `restic backup --tag queue` and `--tag door` and touches `state/ok`; `fresh` exits 0.
- [x] HB9 — **Nothing else moves.** `deploy.rs`, `deploy_ops.rs`, `deploy_admin.rs` pass, their pins changed only where this slice changed the files; `image.yml`'s checks pass; `hub-ops.md` "Exits" says the new sentence.
- [x] HB10 — **The runbook.** `scripts/deploy/README.md` "Install" and "Going live", within 10 240 B: `backup` waits unconfigured, its line(s) each round, until the owner turns backups on; then his steps: a restic backend, `restic_env` written in place (or `backup` restarted), the secrets kept off the host, `backup init` once, `backup snapshots` listing `queue` and `door` under 35 minutes old with `backup` healthy, a restore drill on a scratch host.
- [x] HB11 — **[live hub, after the upgrade to 0.48.1]** `dump-door` logs `dumped …` each round; `backup` is up and unhealthy, its line(s) each round, no restart. Seen 2026-11-20: `dump-door` logged `dumped users.yml mcp-users acts.log webauthn.yml totp.yml` at 01:12, 01:15 and 01:30 UTC, 0 restarts; `backup` 0 restarts, one line a round on the grid at 01:20 and 01:35. When the owner turns backups on (his word keeps them off): `backup` healthy, `snapshots` lists both tags, a restore drill reads both back.

## Out of scope

- choosing or paying for a backend, the owner's credentials;
- off-host copies of the deploy key, Authelia's secrets or `traefik-acme` (ADR-0071);
- the dumps' grid, retention or format; any crate change; monitoring beyond the healthcheck and the lines;
- `reset` of a person with no TOTP, which warns `TOTP not deleted` and logs `totp-left`: RM-0131, the owner's question.

## Implementation

| Module | Lane | What it does |
|---|---|---|
| `scripts/deploy/admin/hub-admin` | `rust-developer` | `door_conf`; `door_run` writes it before each storage call; `--config` on every storage call; `dump_export` leaves an empty table's file out (`EMPTY_EXPORT`) |
| `scripts/deploy/ops/hub-ops` | `rust-developer` | `host_set`, `upload_settings`: a loop never exits on a setting; `once` keeps exit 2 |
| `fixtures/hub-backups/webauthn.yml` | `test-engineer` | one invented credential, for HB5 |
| http `tests/deploy_admin.rs`, `deploy_ops.rs`, `deploy.rs` | `test-engineer` | the stub's loading rule and `empty-at`; `hb01`-`hb06` (HB3 here, beside the door's stub), `hb07`, `hb08`, `hb04` |
| `plugin/`, `.env.example` | `rust-developer` | 0.48.1, a version-only PATCH (`plugin-files.md` "Changing it") |

**HB5's run** (the developer's, 2026-11-20; `cargo test -p specengine-http --test deploy_admin hb05 -- --ignored`). The image is `authelia/authelia@sha256:bd97cff4…`, 4.39.28, the one `dump-door` runs. A scratch SQLite, the storage key a file as `AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE` gives it:
- **Without `--config`**, every storage call prints `Error: failed to load configuration from file path(/config/configuration.yml) source: stat /config/configuration.yml: no such file or directory`, twice in one line, and exits 1. The image's `X_AUTHELIA_CONFIG` names that path, which no sidecar mounts. That is the hub's line, so option A, a `--config` of hub-admin's own, was tried first.
- **With `--config`** naming a comment-only file, `storage migrate up` and `storage user webauthn import` of `fixtures/hub-backups/webauthn.yml` exit 0. The exact export call `hub-admin` makes, under `timeout 300`, exits 0, and its file holds the credential's `kid` and `public_key`. Both exports and the deletes exit 0. A file holding no key was enough, so B was not needed.
- **An empty table:** the TOTP export of a door with no TOTP answers `Error: no data to export`, exit 1, and writes no file.
- **The deletes on nothing:** `webauthn delete --all` exits 0; `totp delete` exits 1 with `no TOTP configuration for user`.
- **Last,** the real `hub-admin dump`, mounted as `dump-door` mounts it, exits 0 with `dumped users.yml mcp-users acts.log webauthn.yml`.

**Readings** (the orchestrator's): the file is `hub-admin.$$.storage.yml`, per process, not `storage.yml`; the stub prints the error doubled, as the image does; HB3 lives in `deploy_admin.rs`; `disable` makes no storage call.

**Canon at ship:** `hub-ops.md` "Exits", "Door dump", "Upgrade" steps 2 and 4, "Known limits"; its "Restic" moved whole to `hub-restic.md` (the 12 288 B cap), with the file-secret inode rule; `hub-deploy.md` "Gate"; `plugin-files.md` 0.48.1; the guide's "Install", "Going live", "Before the gate".

**Release.** 0.48.1, tag `v0.48.1`, a version-only PATCH of the plugin for the image's tag; no crate change, no ADR (ADR-0071 stands). The review: ACCEPT.
