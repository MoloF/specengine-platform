---
class: spec
status: shipped
shipped: 2026-11-03
scope: [scripts/deploy, .github/workflows, crates/specengine-cli, crates/specengine-http]
ref: RM-0065; Phase 7 step 11c-1 (08 s2); the hub-accounts analysis, the orchestrator's decisions Q1-Q8, R1-R13 of 2026-11-03, the owner's review open; three iterations
adrs: [ADR-0068]
---

# hub-accounts: a hub's accounts from the host's shell, by the deployment's script in a container with no network (RM-0065)

## Why

The owner asked for a root admin who manages accounts through a CLI on the server itself: one that can always recover a lost account or register people from scratch, and that only the inside of a container reaches. Until this slice the operator edited Authelia's users file and Traefik's MCP users file by hand (ADR-0067's cost). The acts are now the deployment's script, `hub-admin`, in a one-shot `admin` service with no network on the door's own image. Root is the host's shell. SpecEngine gains one read, `spec team`, and no account code. The door re-reads a person on every request. The read-only users folder and a first `add` before `up -d` close a possible hole in 11b-1's install: a default user written when the users file was missing (from memory, VF1). A stream's maximum life is 11c-2, RM-0075 (AC17).

Decided by the orchestrator on 2026-11-03 while the owner was away, **each awaiting the owner's review**:
- **Q1** Design B: the script, not a `spec admin` in the CLI. That would put account code in SpecEngine and the daemon's binary against ADR-0034 and ADR-0059, with hash crates, a YAML writer and the door's SQLite.
- **Q2-Q4** `network_mode: none`. `[team]` is never written, only printed for a reviewed commit and checked. Enrolment relays a password and the second-factor code, password reset staying off.
- **Q5** An ASCII login grammar.
- **Q6** No "last login".
- **Q7** Streams' life is RM-0075.
- **Q8** The act log is a gitignored deployment file.
- **S1-S7**, the spec's choices:
  - S1, a stamp instead of a bare touch;
  - S3, `code` needs a terminal;
  - S4, `add` refuses a login holding an MCP line;
  - S5, the last enabled account asks `[y/N]`;
  - S6, secrets from `/dev/urandom` on stdin, replaced on 2026-11-10 (VF1): the door's CLI draws each one (`--random`), on no stdin and in no argument;
  - S7, `HUB_ADMIN_ROOT`.
- **R1-R13**, the reviews' decisions:
  - names and the users file refuse the bytes yaml.v3 rejects (R1, R8, R12);
  - a login starts with a letter, and `null`, `true`, `false` are reserved (R2);
  - a reset completes when a delete fails, but exits 1 on `passkeys-left` (R3, R10);
  - temp files are removed only under the lock, scratch kept off the host (R4);
  - the MCP file's reader and `disable`'s door-first order (R5, R13);
  - no log driver (R6);
  - the stage recorded, `--` before a login, `spec team`'s error line (R7);
  - the scan fails closed (R9);
  - a rotate with a failed stamp still shows its secret (R11).

## Description and interactions

Canon: `docs/canon/hub-deploy.md#accounts` (ADR-0068); `[team]` and `spec team`: `docs/canon/hub-team.md` "Team"; the procedures: `scripts/deploy/README.md` "Accounts", "AI over MCP: Claude Code", "Before the gate". No route, header or tool reaches the acts.

## Data

The files are the truth; the canon and the guide describe them.

### Files

`scripts/deploy/admin/hub-admin` (new), `docker-compose.yml` (service `admin`; `authelia`'s users mount `:ro`), `authelia/config/configuration.yml` (`refresh_interval: always`), `.gitignore` (`admin-log/`, `traefik/dynamic/mcp-stamp.yml`); `.github/workflows/image.yml`; CLI `team.rs`.

### `hub-admin`

`docker compose run --rm admin <act>`. The acts are `add <login> --name <n> --email <e>`, `disable`, `enable`, `reset`, `mcp rotate|revoke`, `code`, `list`, `check` and `log`. Exits: 0 done or `unchanged`; 1 refused by state or stopped part-way, each untouched file named `<file> is as it was`; 2 a bad call, no terminal included. New logins match `^[a-z]([a-z0-9._-]{0,62}[a-z0-9])?$`, never `local`, `agent`, `null`, `true`, `false`. The act log is `admin-log/acts.log`, tab-separated: time, act, login, detail. Its details:
- `-`;
- `mcp line removed`, `mcp-line-left`;
- `password passkeys totp`, `passkeys-left`, `totp-left`, `password-not-written`;
- `stamp-not-written`.

### The door's CLI

A password: `authelia crypto hash generate argon2 --random --random.characters 0123456789abcdef --random.length 32`. An MCP secret: `authelia crypto hash generate bcrypt --cost <lowest> --random --random.characters 0123456789abcdef --random.length 64`. The door draws the value: nothing goes on stdin and no value is an argument (4.39.28 refuses a piped password, and `--password` would show it in the process list: VF1). The door's stdout, a fresh mode-600 file under `$HUB_ADMIN_ROOT/tmp`, must hold one `Random Password: ` line of lower-case hex of that length: that value is the password or secret. Then `Digest: ` is stripped and its shape checked. A missing, doubled or off-shape `Random Password: ` line exits 1 with nothing written or called after the hash: ``hub-admin: the door's CLI answered off its shape: no one `Random Password: ` line of 32 lower-case hex; nothing changed`` (64 for the secret). `authelia storage user webauthn delete --all --sqlite.path <db> -- <login>` and `... totp delete --sqlite.path <db> -- <login>`: their flags are from memory until VF1.

### `mcp rotate`'s output

The 64-hex secret once, then `claude mcp add --transport http --scope user hub-<slug> https://<HUB_HOST>/mcp/<slug> --header "Authorization: Basic <b64>"`. The `<b64>` is base64 of `<login>:<secret>` with every newline removed, since busybox wraps at 76.

### `spec team`

Read-only over the root's config. It prints one `<login>\t<role>\t<email>` line per entry in byte order of the login; `--json` prints `{"team":[{login,name,role,zones,email}]}`, or `{"team":null}` with no `[team]` (nothing in plain, exit 0). A bad `[team]` gives one stderr line, `spec: <config>:<line>: <message>`, exit 2; `team_read`'s other callers print no prefix.

## Rules and edge cases

ADR-0008: no `crates/*/src` names the door or its formats. ADR-0012: `check` reports and exits 0; the last account's disable only asks. ADR-0034, ADR-0059: the daemon keeps no account. ADR-0060: `[team]` changes by a reviewed commit alone. A reset ends no live session. A disable ends one at its next request (VF3), and a stream lives on until RM-0075.

## Acceptance criteria

Tests: http `tests/deploy_admin.rs` (AC1-AC14, AC16, AC18-AC20; `sh` and dash, a stub door first on `PATH`, a terminal by `script(1)`); `tests/deploy.rs` (D7, D9, D11 rewritten); CLI `tests/team_config.rs` (AC15). M marks the mutations that turned each red.

- [x] AC1 — `add`. Writes `users.mira` with the digest unprefixed, leaves `olek` byte for byte, a new inode, mode 600. The password is the stub's `Random Password:` (`argon2 --random`, 32 hex), printed once; the hash gets no `--password`, no stdin, and the password in no argument; one log line; the `[team]` row is printed. M: keep the prefix; write in place; drop the log line; pipe the password to the hash.
- [x] AC2 — Grammar. Exit 2, nothing written or called, for these logins: `Mira`, `mi:ra`, `mi,ra`, ` mira`, `mira.`, `local`, `agent`, `null`, `true`, `false`, `-`, `1234`, `1mira`, `\u{43c}ira`, `mi\u{200b}ra`, 65 bytes, an existing one. The same for these names: `\xff`, `\xc0\xaf`, `\xed\xa0\x80`, `\u{85}`, `\u{9b}`, `\u{2028}`, `\u{2029}`, `\u{feff}`, `\u{fffe}`, `\u{ffff}`, a tab. A 64-byte login and a name in another script pass; the row reads `[team."mi.ra"]`. M: drop `:`; accept upper case; drop the existing-login check; a leading digit; drop `null`; accept `\u{2028}` or `\u{ffff}`; skip the UTF-8 check.
- [x] AC3 — Terminal. On a piped or `/dev/null` stdin, the acts that write and `code` exit 2, take no lock and call nothing; `list`, `check` and `log` answer. M: drop `[ -t 0 ]`.
- [x] AC4 — `disable`. Sets `disabled: true`, removes the MCP line and writes the stamp one higher; `hub.yml` stays byte for byte. A second one answers `unchanged`. The last account asks `[y/N]`. With the MCP folder unwritable, or an MCP line refused, the door is still disabled: exit 1, `run disable again`, `mcp-line-left`, and a rerun removes the line. A `mv` stub's SIGTERM after the rename still logs `disable`. M: keep the line; skip the stamp; drop the question; MCP before the door; "nothing changed"; `unchanged` with a line left; read the MCP file first.
- [x] AC5 — `enable`. `disabled: false`, no MCP line or stamp. M: re-issue a line.
- [x] AC6 — `reset`. A new digest. The stub sees the argon2 `--random` hash with nothing on stdin, its `Random Password:` the printed password, then `webauthn delete --all ... -- mira`, `totp delete ... -- mira`, nothing else. M: skip the WebAuthn delete; drop `--`.
- [x] AC7 — `mcp rotate`. One 64-hex secret and its line, Basic on one line. One `mira:` line, the others byte for byte; the secret is the stub's `Random Password:` (`bcrypt --cost 10 --random`, 64 hex), on no stdin and in no argument. A disabled login exits 2. A failed stamp keeps the line, shows the secret, logs `stamp-not-written`, says to restart `traefik`, and exits 1. M: append; the secret as an argument; base64 with newlines; exit without the secret.
- [x] AC8 — Lock and temp files. A held lock refuses every writing act and leaves the holder's `.mcp-stamp.tmp`. The lock is gone after an act and after SIGTERM. The door's output is kept under `$HUB_ADMIN_ROOT/tmp`, and `admin-log/` holds `acts.log` alone. M: no lock check; no trap; remove temp files without the lock; capture in `admin-log/`.
- [x] AC9 — A failing or off-shape hash in `add`, `reset` or `mcp rotate` leaves both files and the stamp byte for byte, with no delete and no `.tmp`. M: write before the hash returns; accept any digest line.
- [x] AC10 — Layout and bytes. Exit 1, naming the line, for: a tab, a flow mapping, `groups:`, six-space indentation, a duplicate login, a name holding `\u{85}`, `\u{2028}`, `\u{fffe}`, `\u{ffff}`, a NUL, `\x01` or `\xff`, a leading `\u{feff}`. 11b-1's example reads. Blank and spaces-only MCP lines read; padded or `\r` lines are refused. An `od` that fails for `-tu1`, or prints nothing, refuses a clean file and a name. M: accept any file; accept `\u{2028}`, `\u{fffe}` or a NUL; refuse a blank MCP line; accept a trailing space; ignore `od`'s exit.
- [x] AC11 — Orphans. `list` names MCP lines with no account or a disabled one; `mcp revoke zed` removes an orphan's line. M: drop that check; refuse a revoke with no account.
- [x] AC12 — `check`. The built `spec team` piped into `check` prints `no account: mi:ra`, `holds ':': mi:ra`, `email differs: mira: ...`, `disabled: olek`, `no account: zed`, `check: 4 logins, 5 findings`, and exits 0. M: exit 1 on a finding; another separator.
- [x] AC13 — Compose. `admin` has `network_mode: none`, `profiles: [admin]`, `logging: {driver: none}`, no `ports`, `expose`, `networks`, `depends_on`, `restart` or `user`, `authelia`'s image, one secret, six mounts. `authelia`'s users mount is `:ro`; `specengine` mounts none of them; the `.gitignore` entries. M: `service:traefik`; `ports`; no `profiles`; writable users in `authelia`; no `logging`.
- [x] AC14 — The door: `refresh_interval: always`, password reset off. M: drop the key; `5m`.
- [x] AC15 — `spec team`. Rows in byte order (`Mira K`, `mira`, `owner`) and `--json`. No `[team]`: nothing, exit 0. A bad one: the `spec: ` line, exit 2. Nothing written. M: drop a row; sort by `name`.
- [x] AC16 — No account name in `crates/*/src`; no `argon2`, `bcrypt` or `password-hash` in `Cargo.lock`. M: a `"users.yml"` constant in the CLI.
- [x] AC18 — `export index` and `check` green; the guide 9 531 of 10 240 B; `image.yml` runs `deploy_admin`, and C3 takes one image line. M: drop the file from `image.yml`.
- [x] AC19 — A failing delete. TOTP alone: exit 0, the warning, `totp-left`. Passkeys: the TOTP delete still runs, the password is written and shown, `passkeys-left`, exit 1, "do not enable". Both: exit 1. M: abort on a delete; skip TOTP after passkeys; exit 0 on `passkeys-left`.
- [x] AC20 — The door's value (hotfix, 2026-11-10). The stub answers `crypto hash generate` as 4.39.28 did on the VPS: with neither `--password` nor `--random` and stdin no terminal, exit 1, its `failed to read the password from the terminal` line, nothing on stdout; with `--random`, exactly `Random Password: <hex>` and `Digest: `. A `Random Password:` line missing, doubled or off its shape (empty, one hex short or long, upper case, a non-hex byte, `Random password:`) makes `add`, `reset` and `mcp rotate` exit 1 with the `nothing changed` line: no value shown, no call after the hash, every file and the stamp byte for byte, nothing logged, no temp file or lock. M: take the line unchecked; accept any length; accept upper case.

AC17, the streams' life: RM-0075. The decided changes to 11b-1's tests: D7, `admin`'s `network_mode: none`; D9, four services, `admin` on `authelia`'s image; D11, `authelia_storage_key` to `authelia` and `admin`, and the new committed and ignored paths.

By hand, before the gate (the owner's; findings in `hub-deploy.md`):
- [ ] VF1 — The door's image and CLI. Its `sh`, `awk`, `od`, `base64`, and where `authelia` lives. `crypto hash generate`, its output, the lowest bcrypt cost. Both deletes' flags with `--`, the storage key alone, and their answers for a person with no TOTP and no passkey. An invite or password change; a last login. A default user for a missing users file; a start on a read-only one. Found 2026-11-10 on the owner's VPS, Authelia 4.39.28: the first real `admin add` failed, the door refusing the password piped on stdin (`Error: failed to read the password from the terminal: you must either use an interactive terminal or use the --password flag`). After the hotfix (`--random`, "The door's CLI"), an `add` in a throwaway folder printed the password once and wrote an `$argon2id$v=19$m=65536` digest; the folder was then removed. The rest of VF1 stays open.
- [ ] VF2 — The proxy. A new stamp, and a bare touch, rebuild `mcp-credential`; `$2b$` passes; what a missing or empty users file does to `/mcp`.
- [ ] VF3 — On Linux. A disable ends a live session at its next request; two successive renames are both read.

The gate, step 11b-3, beside H1-H17:
- [ ] G1 — `admin add`, then `up -d`: the owner logs in over TLS, enrols with the relayed code, and confirms on a step-up route.
- [ ] G2 — A disabled person's next request lands on the door; their streams end (RM-0075); their Claude Code's next POST is refused.
- [ ] G3 — After a reset, a fresh login, and the old passkey refused.
- [ ] G4 — Everything disabled and `authelia` stopped: `enable` and `reset` restore the owner.
- [ ] G5 — Only 80 and 443 answer; no `admin` container is left after a run.
- [ ] G6 — A piped `run -T admin add` is refused.
- [ ] G7 — Two runs at once: one refused, both files whole.
- [ ] G8 — A `[team]` commit brought in, `check` names it.
- [ ] G9 — MCP latency at the chosen bcrypt cost.
- [ ] G10 — A dump during an act restores whole files (RM-0074).

## Out of scope

OAuth and claude.ai credentials; writing `[team]`; dumps and restore (RM-0074); any route or tool for these acts or `spec team`; a root account; SMTP; removing an entry; revoking one person's sessions; a stream's life (RM-0075); changes to `hub_login_problem`, the daemon or the fence.

## Implementation

Three iterations. Iteration 1: REWORK on two majors (bytes in `--name` that the door rejects; a reset that could never finish). Iteration 2: REWORK on one (U+FFFE and U+FFFF). Iteration 3: ACCEPT. Mutations: 46, 54 and 51, all red. One mutation of iteration 3 is equivalent by design: moving `disable`'s stage alone changes nothing, since `stopped_log` re-reads the users file. Tests: workspace 4887 of 4887; `deploy_admin` 33 of 33 under `sh` and dash; `deploy` 20 of 20; `team_config` 8 of 8; clippy and `spec check` clean.

| Module | What it does |
|---|---|
| `scripts/deploy/admin/hub-admin` | `act_add`, `act_disable`, `act_enable`, `act_reset`, `act_mcp_rotate`, `act_mcp_revoke`, `act_code`, `act_list`, `act_check`, `act_log`. Checks: `need_terminal`, `check_new_login`, `check_name`, `check_email`, and `utf8_scan` (`od` to numbers, fail-closed). Readers: `users_scan`, `users_query`, `mcp_query`, `stamp_read`. Writers: `edit_lines`, `install_file`, `write_stamp`, `remove_mcp_line`. The door: `find_door_cli`, `door_run` (output under `$root/tmp`, made fresh), `hash_password`, `hash_secret` (`--random`), `random_of`, `digest_of`, `delete_passkeys`, `delete_totp`. The lock and log: `take_lock`, `log_act`, `stopped_log`, `cleanup` |
| `scripts/deploy/docker-compose.yml`, `.gitignore` | service `admin`; `authelia`'s `./authelia/users:/users:ro`; four services in the header; the ignored log folder and stamp |
| `scripts/deploy/authelia/config/configuration.yml` | `authentication_backend.refresh_interval: always` |
| `.github/workflows/image.yml` | `--test deploy_admin` after the CLI's test, which builds `spec`; C3's `sort -u` |
| CLI `src/team.rs`, `main.rs`, `lib.rs` | `team`, `TeamOutcome`, `TeamEntryRow`, `render_text`, `spec_prefixed`; `Command::Team` |
| Tests | http `tests/deploy_admin.rs`, `tests/deploy.rs` (D7, D9, D11); CLI `tests/team_config.rs` |

The developer's choices, accepted:
- A missing mounted folder is refused (`need_dir`), never created.
- A second MCP line for one login is refused.
- `disabled:` is written unquoted.
- `code` takes the lock.
- `--sqlite.path` sits under the root.
- A scan that fails exits 3 inside `utf8_scan`, which its callers turn into 1.
- `stopped_log`'s re-read covers R13 with no new stage.
- A clean reset logs `password passkeys totp`.
- A signalled reset logs as left the deletes it could not confirm.
- A spaces-only MCP line is blank.
- A refusal says `<file> is as it was`.

Hotfix, 2026-11-10 (VF1): 4.39.28 reads no piped password, so the door draws each value. `hash_password` and `hash_secret` ask `--random` and set `password` or `secret` from `random_of`, the one `Random Password: ` line with its length and hex checked; `random_hex` and its `/dev/urandom` read are gone; `door_run` removes its output files first, so umask 077 makes them mode 600. Tests: the stub answers as 4.39.28 (`NO_TERMINAL`); AC1, AC6 and AC7 rewritten; AC20 new, `door_stub_answers_crypto_hash_as_the_pinned_cli` and `a_missing_or_off_shape_random_password_changes_nothing`.

Spec's reading of R4, standing: staging files are renamed beside their target, because a rename is atomic only within one file system. A killed act can leave one, and the guide names it. Smaller limits: `hub-deploy.md` "Known limits" and the guide's "Limits".
