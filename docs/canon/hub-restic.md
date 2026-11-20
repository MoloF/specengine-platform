---
class: canon
tier: 2
scope: [scripts/deploy]
owner: owner
reviewed: 2026-11-20
---

# Hub restic: the off-host backend, off until the owner names one

ADR-0071, built by `hub-ops` (RM-0074); moved here from `hub-ops.md` by `hub-backups` (RM-0130). The dumps, their grid and the `upload` round: `hub-ops.md` "Dumps". **Off by the owner's word, 2026-11-20**: with no `RESTIC_REPOSITORY` in `restic_env`, `backup` stays up, unhealthy, its line(s) each round, calling no restic (`hub-ops.md` "Exits"). His steps to turn it on: `scripts/deploy/README.md` "Going live".

## The backend

- **`secrets/restic_password`** (`openssl rand -hex 32`, `RESTIC_PASSWORD_FILE`) and **`secrets/restic_env`**, not `.env` (a URL may hold a password): Docker secrets of `backup` alone, 600. Encryption is restic's, client-side.
- **`restic_env`**: a `KEY=value` per LF-ended line; empty and `#` lines skipped. Keys, each once: `RESTIC_REPOSITORY` (required), `RESTIC_REST_USERNAME`, `RESTIC_REST_PASSWORD`, or `AWS_`, `B2_`, `AZURE_`, `GOOGLE_`, `OS_`, `ST_` then `[A-Z0-9_]`. Read as data, never sourced: the value is the rest of the line. Another line, or a CR end, is refused naming its number, and its key only as an upper-case shell name (`[A-Z_][A-Z0-9_]*`, at most 64 B), else `not KEY=value`: text before `=` may be a pasted secret. No value is printed.
- **Redaction**: restic's last line is logged with every occurrence of each value covered as `<KEY>`, longest first, overlaps too.
- **Retention**: per `--host <HUB_HOST>`, tags `queue` and `door` apart; every round for 48 hours, one a day for 30 days.
- **A file secret** is a bind mount of the file's inode: write `restic_env` in place, or `docker compose restart backup` after editing it, since an editor that saves by rename leaves the running `backup` on the old inode.
- **Acts**, `docker compose run --rm backup <act>`: `init` (never automatic), `snapshots`, `check`, `restore DIR [QUEUE DOOR]` (`restic restore <id>:/snapshot/<part> --target DIR/<part>`, `DIR` empty; `latest`, the default, by host and tag).
- **`fresh`**, `backup`'s healthcheck (5 min, one retry, 20 min start): 0 when `/state/ok` and both parts' newest generations are under 35 minutes old.

## Known limits

- An all-upper-case pasted secret with `=` padding is named as a key; no backend key has that shape.
- An intruder on the host can delete the backups unless the backend locks objects.
