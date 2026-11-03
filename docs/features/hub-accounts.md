---
class: spec
status: draft
scope: [scripts/deploy, .github/workflows, crates/specengine-cli, crates/specengine-http]
ref: RM-0065; Phase 7 step 11c-1 (08 s2); the hub-accounts analysis and the orchestrator's decisions Q1-Q8 of 2026-11-03, each awaiting the owner's review; ADR hub-accounts a draft awaiting the owner's confirmation
---

# hub-accounts: a hub's accounts from the host's shell, by the deployment's script in a container with no network (RM-0065)

## Why

The owner asked, in an earlier session, for a root admin who manages accounts through a CLI on the server itself: one that can always recover a lost account or register new people from scratch, and that only the inside of a container reaches, never the public. Until now `scripts/deploy/README.md` "Accounts, by hand until RM-0065" has the operator edit Authelia's users file and Traefik's MCP users file by hand, and ADR-0067's cost names that: "a second secret per person, kept by hand until RM-0065".

This slice builds that CLI as the deployment's own script, `hub-admin`, run as a one-shot `admin` compose service with no network on the pinned Authelia image. That image carries the door's CLI, which hashes passwords and deletes passkeys and TOTP. The root is the host's shell: whoever runs `docker compose` there. SpecEngine gains one read, `spec team`, so the script can compare `[team]` with the door's accounts, and no account code. The door re-reads a person on every request, so a disable ends a live web session at its next request. A hub's streams, which passed the door once, get their maximum life in 11c-2 `stream-life` (RM-0075).

It also fixes a possible hole in 11b-1's install (from memory, VF1). Authelia writes a default user when its users file is missing. The guide makes an empty `authelia/users` (`scripts/deploy/README.md:27`), and compose mounts it writable (`docker-compose.yml:50`), so a first `up -d` before any account would create that user. This step mounts the folder read-only into `authelia` and puts a first `admin add` before `up -d`.

Decided by the orchestrator on 2026-11-03 while the owner is away. **Each awaits the owner's review.**
- **Q1, who carries out the acts: the deployment's script (design B).** A POSIX `sh` `hub-admin` in `scripts/deploy/admin/`, run as the one-shot `admin` service; not a `spec admin` in the CLI (design A). A `spec admin` that mints secrets, hashes passwords and owns account lifecycle would put account code inside SpecEngine:
  - against ADR-0034 ("no token, key, account, session ... on any surface", `docs/canon/architecture.md:112`) and ADR-0059 ("No account, token, key or session exists inside SpecEngine"), as RM-0065's old body already conceded;
  - in the daemon's binary too, since `specengine-http` links the CLI library (http `Cargo.toml:25`);
  - with argon2 and bcrypt crates (owner decisions) and a YAML writer;
  - still needing Authelia's CLI or its SQLite for passkeys and TOTP, where the CLI never links `rusqlite` (CLI `Cargo.toml:16`) and SpecEngine's image holds no `authelia`.

  B meets the request exactly (a CLI on the server, inside a container, never public) and keeps every rule. It costs 3.5-5 hours against A's 5.5-8.
- **Q2** One-shot `admin` with `network_mode: none`, rather than a script on the host.
- **Q3** The tool never writes `[team]`. It prints the row for a reviewed commit (ADR-0060), and `check` confirms the row once it arrives.
- **Q4** Enrolment relays a password and the second-factor code; password reset stays off (11b-1's S8). The users folder is read-only in `authelia`, so the tool is the users file's only writer.
- **Q5** New logins are ASCII: lower case, digits, `.`, `_`, `-`, a letter or digit at both ends, at most 64 bytes, never `:`. Display names may use any script. The pilots' configs hold no `[team]` (checked 2026-11-03), so nothing breaks.
- **Q6** "Last login" is dropped from `list`, unless the pinned door's CLI prints it (VF1).
- **Q7** A hub's streams get a maximum life of 300 s, on a hub only, so a disabled person's open tab stops receiving events: 11c-2 `stream-life`, RM-0075 (AC17 there).
- **Q8** The act log is a deployment file in a folder only `admin` mounts, gitignored, dumped by RM-0074.

The spec's own choices, **awaiting the orchestrator's acceptance and the owner's review**:
- **S1 The proxy's reload is a stamp, not a bare touch.** Traefik's basicAuth reads its users file when the middleware is built, and Traefik skips a configuration equal to the last one (both from memory). So 11b-1's `touch traefik/dynamic/hub.yml` (the guide's `:75`, `hub-deploy.md:83`) may rebuild nothing, and a revoked secret would still open `/mcp`. After every edit of the MCP users file, `hub-admin` writes `traefik/dynamic/mcp-stamp.yml`: one unused middleware whose value goes up by one. The configuration then differs, so Traefik builds anew and `mcp-credential` reads the file again. `hub.yml` stays byte for byte. VF2 checks this. If the stamp rebuilds nothing either, the analysis's fallback (the users inline in a dynamic file) replaces the one function `reload_proxy`, and D4 and D5 change with it.
- **S2** The acts, arguments, outputs and exits of "Data".
- **S3** `code` shows a secret, so it needs a terminal as the writing acts do; only `list`, `check` and `log` answer on a pipe.
- **S4** `add` refuses a login for which the MCP users file already holds a line, since the new account would take over another person's secret.
- **S5** "Disabling the last enabled owner" (the analysis) reads as "the last enabled account". The tool reads no `[team]` but `check`'s stdin, so it cannot know an owner. It warns and asks `[y/N]`; it never refuses on its own (ADR-0012).
- **S6** Passwords (16 random bytes, 32 hex) and MCP secrets (32 bytes, 64 hex) come from `/dev/urandom` through `od`. They reach the door's CLI on stdin, are printed once, and are never an argument or a log field.
- **S7** Every path the script uses sits under `HUB_ADMIN_ROOT`, empty in the container, so the tests run it on temp folders.

## Description and interactions

Canon to come: `docs/canon/hub-deploy.md` "Accounts" (ADR `hub-accounts`); the guide: `scripts/deploy/README.md`. The trust items stay `hub.md#trust` 1-5 and `hub-mcp.md#trust` 6, met as `hub-deploy.md#door` says. This slice adds no route, header or tool to the daemon.

### How "inside the container, never public" holds

1. `admin` has `network_mode: none` and no `ports`, `expose`, `networks` or `depends_on`. It sits under `profiles: [admin]`, so `up -d` never starts it, and it has no `restart`. The guide runs it with `--rm`.
2. Nothing in it listens.
3. The daemon gets no route or tool for these acts.
4. No folder `admin` writes is mounted into `specengine`.
5. Running `admin` needs the host's Docker, so the host's shell. No container mounts the Docker socket (D8).
6. Every act that writes, and `code`, refuses unless stdin is a terminal (`[ -t 0 ]`).

### Who writes what

| File or state | Written by | Mounted |
|---|---|---|
| `authelia/users/users.yml` | `hub-admin` | writable in `admin`; read-only in `authelia` (writable today, `docker-compose.yml:50`) |
| password digests | the door's `authelia crypto hash generate`, the secret on stdin | none |
| `traefik/mcp-users/users` | `hub-admin` | writable in `admin`; read-only in `traefik`, as today |
| `traefik/dynamic/mcp-stamp.yml` (S1) | `hub-admin` | `./traefik/dynamic` writable in `admin`; read-only in `traefik`, as today |
| passkey and TOTP rows | the door's `authelia storage user ...` | `authelia-data` in `admin`, with the secret `authelia_storage_key` |
| the act log and the lock | `hub-admin` | `./admin-log`, in `admin` alone |
| `[team]` rows | a reviewed commit, nobody on the host | none |
| `[team]` read for `check` | `spec team`, read-only, through `docker compose exec specengine` | `./projects`, as today |

**The check is one pipe.** `spec team` prints the logins, and `hub-admin check` compares them with the door's accounts, so SpecEngine never learns the door's formats:

```sh
docker compose exec -T specengine spec team --root /projects/<name> \
  | docker compose run --rm -T admin check
```

### The operator's day

- **Install:** the folders and secrets as today; then `docker compose run --rm admin add <owner> --name ... --email ...`, **before** `docker compose up -d`.
- **A new person:** `add`, then relay the password. At their first step-up, `code` shows the one-time code, which the operator relays. The printed `[team]` row goes into the project's config as a reviewed commit (leads and the owner). Then, for AI, `mcp rotate`, and relay the secret and the printed `claude mcp add` line.
- **The row reaches the hub's checkout** only through a `--push` sync, off until the gate, or a by-hand merge inside the container as its user (`docker compose exec specengine git -C /projects/<name> pull --no-rebase`, merging as `hub-git.md` does). The guide gives the step; `check` then confirms the row.
- **Lost passkey or password:** `reset`, then relay as for a new person.
- **Leaving or compromised:** `disable`. That one act removes the MCP line and reloads the proxy, and the door refuses the person at their next request (VF3). For a compromised account: `disable`, `docker compose restart authelia` (sessions live in memory), `reset`, `enable`.
- **Everything lost** (every account disabled, `authelia` stopped): `enable` and `reset` the owner, `up -d`, log in (G4).

## Data

### Files

```text
scripts/deploy/
  admin/hub-admin                     new, POSIX sh, committed 0755
  docker-compose.yml                  + service admin; authelia's ./authelia/users becomes :ro
  authelia/config/configuration.yml   + authentication_backend.refresh_interval: always
  .gitignore                          + admin-log/ and traefik/dynamic/mcp-stamp.yml
  README.md                           the guide (spec-writer, at ship)
  admin-log/                          made by the guide, gitignored: acts.log, lock/
  traefik/dynamic/mcp-stamp.yml       written by hub-admin, gitignored
.github/workflows/image.yml           + cargo test --locked -p specengine-http --test deploy_admin; C3 takes one image line
crates/specengine-cli/src/            spec team: main.rs's Command::Team, team.rs's rows
```

### `docker-compose.yml`

```yaml
  authelia:
    volumes:
      - ./authelia/config:/config:ro
      - ./authelia/users:/users:ro      # hub-admin is the only writer (Q4)
      - authelia-data:/var/lib/authelia
  admin:
    # The account acts (docs/canon/hub-deploy.md "Accounts"): one-shot, run as
    # `docker compose run --rm admin <act>` on the host's terminal; no network.
    image: <the authelia service's image, byte for byte>
    profiles: [admin]
    network_mode: none
    entrypoint: ["/bin/sh", "/opt/hub-admin/hub-admin"]
    environment:
      HUB_HOST: "${HUB_HOST}"
      AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE: /run/secrets/authelia_storage_key
    secrets: [authelia_storage_key]
    volumes:
      - ./admin:/opt/hub-admin:ro
      - ./admin-log:/var/lib/hub-admin
      - ./authelia/users:/users
      - ./traefik/mcp-users:/mcp-users
      - ./traefik/dynamic:/traefik-dynamic
      - authelia-data:/var/lib/authelia
```

`admin` has no `restart`, `ports`, `expose`, `networks`, `depends_on`, `user` or `{{ env }}` read. The comment block above `services:` says four services. `image.yml`'s C3 step pipes `config --images | grep '^authelia/'` through `sort -u`. With two services on one image the grep may print two equal lines, and `docker run "$image"` would break.

### `authelia/config/configuration.yml`

```yaml
authentication_backend:
  # A lost password is the operator's (S8).
  password_reset: {disable: true}
  # A person is read again on every request, so a disable ends a session at its
  # next request (hub-accounts).
  refresh_interval: always
  file: {path: /users/users.yml, watch: true}
```

The key's place is from memory; C3's `validate-config` refuses an unknown key in CI.

### `hub-admin`

Run as `docker compose run --rm admin <act> [<argument>...]`.

| Act | Terminal | Writes | Prints | Log |
|---|---|---|---|---|
| `add <login> --name <display name> --email <address>` | yes | the users file | `added <login>`, the password once, the `[team]` row | `add` |
| `disable <login>` | yes | the users file; the MCP file and the stamp if a line was there | `disabled <login>`, `mcp line removed` when one was | `disable` |
| `enable <login>` | yes | the users file | `enabled <login>` | `enable` |
| `reset <login>` | yes | the users file; passkey and TOTP rows | `reset <login>`, the password once | `reset` |
| `mcp rotate <login>` | yes | the MCP file, the stamp | the secret once, the `claude mcp add` line | `mcp-rotate` |
| `mcp revoke <login>` | yes | the MCP file, the stamp | `revoked <login>` | `mcp-revoke` |
| `code` | yes | nothing | the notifier's file, as is | `code` |
| `list` | no | nothing | "list" below | none |
| `check` | no | nothing | "check" below | none |
| `log` | no | nothing | the act log | none |

**Exits.** 0 done, or `unchanged <login>` when the act would change nothing, which writes and logs nothing. 1 refused by state: the lock is held; a file in an unknown layout; the door's CLI failed or answered off its shape; the operator answered no. 2 a bad call: an unknown act or option, a missing argument, the login grammar, an unknown login (any act but `add`), an existing one (`add`), a disabled one (`mcp rotate`), an MCP line already there (`add`, S4), or no terminal. Messages go to stderr as `hub-admin: <message>`; nothing is written and the door's CLI is not called before a refusal.

**The login grammar (Q5),** for `add` only: `^[a-z0-9]([a-z0-9._-]{0,62}[a-z0-9])?$`, so at most 64 bytes, and not `local` or `agent` (the record's words for no person, `hub.md` "Identity"). Every other act takes any login the users file holds, so hand-made 11b-1 logins still work. `--name`: 1-128 bytes, no ASCII control byte, written double-quoted with `\` and `"` escaped. `--email`: at most 254 bytes of printable ASCII with no space, `"`, `'`, `\`, `<`, `>`, `#` or `,`, one `@` with both sides non-empty.

**The users file, read.** Accepted lines, else exit 1 naming the line (`hub-admin: users.yml:<line>: <reason>`), nothing written:
- blank lines and whole-line comments;
- `users:` once, as the first line that is neither blank nor a comment;
- an entry `  <key>:`: two spaces, a key holding no `:`, `#` or quote and neither starting nor ending with a space, an optional trailing `# comment`;
- a field `    <name>: <value>`: four spaces, `<name>` one of `displayname`, `email`, `password`, `disabled`, the value double-quoted, single-quoted or plain, an optional trailing `# comment`.

Refused: tabs, flow style, anchors, any other key (`groups`), other indentation, a duplicate login or field, an entry with no `password`. 11b-1's guide example reads. A missing file is an empty `users:`.

**The users file, written.** A new entry is appended in this form, after a final newline if one was missing:

```yaml
users:
  mira:
    displayname: "Mira K"
    email: "mira@example.org"
    password: "$argon2id$v=19$m=65536,t=3,p=4$c2FsdA$aGFzaA"
    disabled: false
```

`disable` and `enable` replace the entry's `disabled:` line, or insert one after its last field. `reset` replaces its `password:` line. Every other byte stays. A write goes through a temp file `.users.yml.<pid>.tmp` in the same folder, mode 600, which is read back with the reader above before it is renamed over `users.yml`. The previous file is kept as `users.yml.prev`, mode 600.

**The MCP users file** (`traefik/mcp-users/users`, mode 600): only lines `<login>:<digest>`, each digest matching `^\$2[aby]\$[0-9]{2}\$[./A-Za-z0-9]{53}$`; any other line is refused, exit 1, naming it. `mcp rotate` replaces the login's line or appends one. `mcp revoke` and `disable` remove it. Each write goes through a temp file, read back, renamed, with `users.prev` kept.

**The stamp (S1),** written last, after any change to the MCP file, through `.mcp-stamp.tmp` (not `.yml`, so the file provider never loads it), mode 644. `<n>` is one more than the current file's value, 1 when it is absent:

```yaml
# Written by hub-admin (docs/canon/hub-deploy.md "Accounts"): a new value makes
# Traefik build its configuration anew, so mcp-credential reads its users file again.
http:
  middlewares:
    mcp-users-stamp:
      headers:
        customRequestHeaders:
          X-Mcp-Users-Stamp: "<n>"
```

**The door's CLI**, one function each, so VF1 changes a function and the stub's recordings and nothing else. The `authelia` binary is found on `PATH`, else at `/app/authelia` (VF1). Each digest line must match its shape, else exit 1, nothing written:

| Function | Call (flags from memory, VF1) | Read |
|---|---|---|
| `hash_password` | `authelia crypto hash generate argon2`, the password on stdin | one line `Digest: <d>`, `<d>` matching `^\$argon2id\$v=19\$m=[0-9]+,t=[0-9]+,p=[0-9]+\$[A-Za-z0-9+/]+\$[A-Za-z0-9+/]+$` |
| `hash_secret` | `authelia crypto hash generate bcrypt --cost <BCRYPT_COST>`, the secret on stdin | `Digest: <d>`, `<d>` as the MCP file's digest; `BCRYPT_COST` the lowest the pinned CLI accepts (VF1), since the 256-bit secret is what protects it; bcrypt reads 72 bytes, and the 64-hex secret fits |
| `delete_passkeys` | `authelia storage user webauthn delete <login> --all --sqlite.path /var/lib/authelia/db.sqlite3` | exit 0 |
| `delete_totp` | `authelia storage user totp delete <login> --sqlite.path /var/lib/authelia/db.sqlite3` | exit 0 |

The `Digest: ` prefix is stripped. `reset` calls `hash_password`, then `delete_passkeys`, then `delete_totp`, then writes. A failure after `delete_passkeys` logs `failed after passkeys`, exits 1, and leaves both files as they were. A busy database fails cleanly and changes nothing.

**`add`'s output:**

```text
added mira
password, shown once; relay it privately: 3f9c0a6e5b17d2c48e01f7a9b3c6d5e2
[team] row for a reviewed commit to the project's specengine.toml (ADR-0060):
[team.mira]
name  = "Mira K"
role  = "member"            # owner | lead | member; a lead adds zones
email = "mira@example.org"
```

A login holding `.` is quoted in the row (`[team."mi.ra"]`), since a bare `.` makes a dotted TOML key; the name is TOML-escaped.

**`mcp rotate`'s output**, `HUB_HOST` from the service, `<hub host>` when unset. The header's value is `base64` of `<login>:<secret>` with every newline removed: busybox wraps at 76 columns, and macOS has no `-w`.

```text
mcp secret for mira, shown once; relay it privately: <64 hex>
on mira's machine, once per project slug:
claude mcp add --transport http --scope user hub-<slug> https://hub.example.org/mcp/<slug> --header "Authorization: Basic <base64>"
```

**`list`,** in the users file's order, tab-separated, then one line per orphan MCP line:

```text
mira	enabled	mcp	mira@example.org	Mira K
olek	disabled	-	olek@example.org	Olek
orphan mcp line: zed (no account)
```

`(disabled)` names an MCP line whose account is disabled. Exit 0 with orphans.

**`check`** reads `spec team`'s plain lines on stdin. It prints, in stdin order and per login in this order, `no account: <login>` or `disabled: <login>`, then `holds ':': <login>`, then `email differs: <login>: team <a>, door <b>`; last, `check: <n> logins, <k> findings`. It exits 0 whatever it finds (ADR-0012), 2 on a stdin line that is not three tab-separated fields (naming it), and 1 on a users file in an unknown layout. An email difference is information, never a refusal.

**`code`** prints `/var/lib/authelia/notification.txt` as it is, or `no notice` (exit 0). The file notifier keeps one notice (from memory), so the guide enrols one person at a time.

**The act log,** `admin-log/acts.log` (`/var/lib/hub-admin/acts.log`), mode 600 in a 700 folder: one line per act that changed something, or failed after an irreversible step, appended under the lock. Fields are tab-separated: UTC time (`date -u +%Y-%m-%dT%H:%M:%SZ`), act, login (`-` for `code`), detail (`-`, `mcp line removed`, `password passkeys totp`, `failed after passkeys`). No password, secret, digest or code ever enters it.

```text
2026-11-04T09:12:03Z	add	mira	-
2026-11-04T09:20:41Z	mcp-rotate	mira	-
2026-11-05T17:02:10Z	disable	mira	mcp line removed
```

**The lock.** Every act that writes takes `admin-log/lock/` with `mkdir`, writing `taken` (the time) in it; macOS has no `flock(1)`. A trap on `EXIT`, `INT`, `TERM` and `HUP` removes it and any temp file. If the lock is held: exit 1, `hub-admin: another act holds admin-log/lock (taken <time>); if none runs, remove it`. A `SIGKILL` leaves it for the operator. RM-0074's dumps take the same lock.

### `spec team`

A read-only command over the root's config. It opens no index and writes nothing, and it reuses CLI `team_read` (`team.rs`, core `TeamConfig`).
- **Plain:** one line per entry in byte order of the login (`TeamConfig`'s keys), `<login>\t<role>\t<email>`. A `[team]` login holds no control character (`hub_login_problem`), so a tab cannot occur in one.
- **No `[team]`:** nothing printed, exit 0; `--json` gives `{"team":null}`.
- **A bad `[team]`, or no project:** `spec: <config>:<line>: <message>` on stderr, exit 2, as every `team_read` caller does.

```json
{"team":[{"login":"mira","name":"Mira K","role":"lead","zones":["maps"],"email":"mira@example.org"},
         {"login":"owner","name":null,"role":"owner","zones":[],"email":"owner@example.org"}]}
```

(one line on stdout; wrapped here). Clap's doc line: "Print the project's `[team]`: one `login, role, email` line per entry, in login order; read-only. Exit 0 with no `[team]`, 2 with a bad one."

## Rules and edge cases

- WHEN stdin is not a terminal, every act that writes, and `code`, SHALL exit 2 before taking the lock, reading a file or calling the door's CLI.
- WHEN the door's CLI fails or answers off its shape, at any step, both files SHALL stay byte for byte and no temp file SHALL remain.
- WHEN the lock is held, every act that writes SHALL exit 1 and write nothing.
- WHEN `disable` would leave no enabled account, it SHALL warn (`the last enabled account; the host can still restore one`) and ask `[y/N]` on the terminal. Anything but `y` exits 1 and writes nothing (S5).
- WHEN a login is disabled, the same act SHALL remove its MCP line and write the stamp.
- WHEN `add` names a login with an MCP line, it SHALL exit 2 (S4).
- A reset ends no live session. A disable does, at the next request, through `refresh_interval: always` (VF3). A stream lives on until RM-0075.
- **ADR-0008.** No `crates/*/src` file names the door or its formats. `spec team` prints SpecEngine's own config; the script alone knows Authelia's and Traefik's files.
- **ADR-0012.** Nothing reads `hub-admin`'s output. `check` reports and exits 0, a disable of the last account only asks, and no status, check or gate reads the door.
- **ADR-0034, ADR-0059.** The daemon keeps no account, token, key or session. `admin` is the deployment's, outside every crate, and no route or tool reaches it.
- **ADR-0060.** `[team]` changes only by a reviewed commit; the script prints a row and never writes one.
- **A login with `:`.** `add` refuses it, and `check` names any `[team]` login holding one.
- **Shell portability.** busybox `sh` and `awk` in the image, macOS's `sh` on the laptop, dash on CI. POSIX utilities only, plus `od`, `base64` (newlines removed) and `mkdir` for the lock. No `stat`, `flock`, `mktemp`, `sed -i` or `local`; temp names use `$$`.
- **Existing hosts.** No real hub exists before the gate. On a host installed by 11b-1's guide, a `users.yml` entry you did not add (`authelia`, VF1) is removed by hand before the first act, since `hub-admin` has no remove act.
- **RM-0074.** Its dumps include the MCP users file and the act log and take this lock; a restore ends with `admin list` and, by hand, the acts the log records after the dump.

## Acceptance criteria

These run on the laptop and on CI with no image build, no network and no Docker. `sh` runs `hub-admin` against temp folders under `HUB_ADMIN_ROOT`, with a stub `authelia` first on `PATH` (D12's pattern). The stub logs its arguments and stdin and replays a recorded output for each call, from memory until VF1 re-records it from the pinned image (and again at every pin bump). A terminal comes from `script(1)`: the BSD form on macOS, util-linux's on Linux. Tests: http new `tests/deploy_admin.rs` (AC1-AC14, AC16, AC18), http `tests/deploy.rs` (D7, D9, D11 rewritten), CLI `tests/team_config.rs` (AC15). M marks the mutations that must turn each red.

- [ ] AC1 — add. On a terminal, `add mira --name "Mira K" --email mira@example.org` over a users file holding `olek` writes `users.mira` with `displayname`, `email`, `disabled: false` and the digest without `Digest: `; `olek`'s lines stay byte for byte. `users.yml` gets a new inode and mode 600; the password is printed once and is the stub's stdin; the act log gains one `add mira` line; the `[team]` row is printed. M: keep the prefix; write in place; drop the log line.
- [ ] AC2 — grammar. `add` exits 2, writes nothing and never calls the stub for `Mira`, `mi:ra`, `mi,ra`, ` mira`, `mira.`, `local`, `agent`, `-`, `\u{43c}ira` (written as an escape in the test), `mi\u{200b}ra`, a 65-byte login and an existing login. A 64-byte login is accepted; `mi.ra`'s row is `[team."mi.ra"]`. M: drop `:` from the refused set; accept upper case; drop the existing-login check.
- [ ] AC3 — terminal. With stdin a pipe or `/dev/null`, `add`, `disable`, `enable`, `reset`, `mcp rotate`, `mcp revoke` and `code` exit 2, write nothing, take no lock and call nothing; `list`, `check` and `log` answer. M: drop `[ -t 0 ]`.
- [ ] AC4 — disable. One act sets `disabled: true`, removes the login's MCP line and writes the stamp, its value one higher; `hub.yml` stays byte for byte and the stamp holds one middleware, no router. A second `disable` prints `unchanged mira` and writes nothing. Disabling the last enabled account asks `[y/N]`: `n` exits 1 with both files unchanged, `y` disables. M: keep the line; skip the stamp; drop the question.
- [ ] AC5 — enable. Sets `disabled: false` and writes no MCP line or stamp. M: re-issue a line.
- [ ] AC6 — reset. Writes a new digest; the stub's log shows, in order, the argon2 hash, the WebAuthn delete with `--all` and the TOTP delete for that login, and nothing else. M: skip the WebAuthn delete.
- [ ] AC7 — rotate. One 64-hex secret is printed with the `claude mcp add` line, whose Basic value decodes to `mira:<secret>` on one line. It replaces `mira`'s line, so the file has one `mira:` line, and leaves the others byte for byte. The secret is the stub's stdin and in none of its arguments. `rotate` of a disabled login exits 2. M: append a line; pass the secret as an argument; base64 without removing newlines.
- [ ] AC8 — lock. While `admin-log/lock/` exists, every act that writes exits 1 and writes nothing. The lock is gone after a normal act and after SIGTERM to an act held in the stub. M: no lock check; no trap.
- [ ] AC9 — failure. With the stub exiting 1 at each of its steps in `add`, `reset` and `mcp rotate`, both files and the stamp stay byte for byte and no `.tmp` file remains. The same holds for a `Digest:` line off its shape. M: write before the hash returns; accept any digest line.
- [ ] AC10 — layout. A users file with a tab, a flow mapping, a `groups:` key, six-space indentation or a duplicate login is refused with exit 1, naming its line; 11b-1's guide example (comments, a plain email) is read. M: accept any file.
- [ ] AC11 — orphan. `list` names every MCP line whose login is missing or disabled. M: drop that check.
- [ ] AC12 — check. The real `spec team` (the built binary) over a project whose `[team]` holds `mira`, `olek` (disabled at the door), `zed` (no account) and `"mi:ra"`, piped into `check`, prints exactly `no account: mi:ra`, `holds ':': mi:ra`, `email differs: mira: team ..., door ...` (`mira`'s address changed), `disabled: olek`, `no account: zed` and `check: 4 logins, 5 findings`, in that order, and exits 0. M: exit 1 on a finding; change `spec team`'s separator.
- [ ] AC13 — compose. `admin` is as "Data" says: `network_mode: none`; `profiles: [admin]`; no `ports`, `expose`, `networks`, `depends_on`, `restart` or `user`; `image` byte for byte `authelia`'s; exactly the `authelia_storage_key` secret; exactly the six mounts. `authelia` mounts `./authelia/users` read-only. `specengine` mounts none of `./admin`, `./admin-log`, `./authelia`, `./traefik`. `.gitignore` ignores `admin-log/acts.log` and `traefik/dynamic/mcp-stamp.yml` and not `admin/hub-admin`. M: `network_mode: service:traefik`; add `ports`; drop `profiles`; make the users folder writable in `authelia`.
- [ ] AC14 — door. `authentication_backend.refresh_interval` is `always` and `password_reset.disable` is true. M: drop the key; set `5m`.
- [ ] AC15 — `spec team`. Over a copy of `fixtures/spec-a` with a `[team]` of `owner`, `mira` and `"Mira K"`, it prints three lines in byte order of the login (`Mira K` before `mira` before `owner`); `--json` gives the rows of "Data". With no `[team]` it prints nothing and exits 0 (`{"team":null}`); a bad `[team]` prints `spec: specengine.toml:<line>: ...` and exits 2. The root and `HOME` hold no new file afterwards. M: drop a row; sort by `name`.
- [ ] AC16 — no account code. No file under `crates/*/src` names, in any case, `users.yml`, `mcp-users`, `authelia`, `htpasswd`, `argon2` or `bcrypt`; `Cargo.lock` has no package `argon2`, `bcrypt` or `password-hash`. M: add a `"users.yml"` constant to the CLI.
- [ ] AC18 — docs and CI. `export index` and `check` are green, 0 errors and 0 warnings, and the guide fits Tier 1 (10 240 B). `image.yml`'s check job runs `cargo test --locked -p specengine-http --test deploy_admin`, and its C3 step takes one image line. M: drop the file from `image.yml`.

AC17, the streams' life, is RM-0075's.

**Decided changes to 11b-1's tests**, rewritten in `tests/deploy.rs` with their doc lines:
- **D7:** `admin`'s `network_mode` is `none`, and no service but `specengine` and `admin` has one.
- **D9:** the services are exactly `admin`, `authelia`, `specengine`, `traefik`, with `admin`'s image equal to `authelia`'s.
- **D11:** `authelia_storage_key` is granted to `authelia` and `admin`; `admin/hub-admin` joins the committed files, and `admin-log/acts.log` and `traefik/dynamic/mcp-stamp.yml` the private ones.

Each keeps its old mutations. D4's "the dynamic folder holds `hub.yml` alone" stays, since the stamp is never committed.

### Verify first, by hand, with Docker and the pinned images (the owner's, before the gate)

- [ ] VF1 — The pinned Authelia image and its CLI:
  - its `sh`, `awk`, `od`, `base64`, and where `authelia` lives;
  - `crypto hash generate` reading stdin (else its random mode, whose two lines the functions would read), its exact output, and the lowest bcrypt cost;
  - `storage user webauthn delete --all` and `storage user totp delete`: their flags, and whether `AUTHELIA_STORAGE_ENCRYPTION_KEY_FILE` alone suffices;
  - whether an invite or a password change exists, and whether its CLI shows a last login (Q6);
  - whether a missing users file gets a default user, and that a read-only `/users` with the file present starts.

  The stub's recordings are taken here.
- [ ] VF2 — The proxy:
  - whether a new stamp makes `mcp-credential` read its users file again, and whether a bare `touch` does, which settles 11b-1's guide;
  - whether a `$2b$` digest passes basicAuth (else `hash_secret` writes `$2y$`, the same algorithm);
  - whether a missing or empty users file breaks `hub-mcp` and lets `/mcp` fall through to `hub`.
- [ ] VF3 — On Linux: whether `disabled: true` with `refresh_interval: always` ends a live session at its next request, and whether two successive renames of `users.yml` are both picked up by `watch`.

Until these run, the spec carries the "from memory" items with their fallbacks.

### The gate's additions (beside H1-H17 of `hub-deploy.md`)

- [ ] G1 — `admin add` the owner, then `up -d`; the owner logs in over TLS, enrols a passkey with the relayed code and confirms on a step-up route.
- [ ] G2 — Disable a second person while their board is open. Their next request lands on the door, their streams end within the life (RM-0075), and their Claude Code's next POST is refused.
- [ ] G3 — After a reset, the person logs in again and the old passkey is refused.
- [ ] G4 — With every account disabled and `authelia` stopped, `enable` plus `reset` brings the owner back.
- [ ] G5 — Only 80 and 443 answer, and no `admin` container is left after a run.
- [ ] G6 — A piped `docker compose run -T admin add` is refused.
- [ ] G7 — Two runs at once: one is refused, and both files stay whole.
- [ ] G8 — After a `[team]` commit is brought in, `check` names it.
- [ ] G9 — MCP latency at the chosen bcrypt cost.
- [ ] G10 — A dump taken during an act restores whole files (RM-0074).

## Out of scope

- OAuth and claude.ai credentials (after H9).
- Writing `[team]`.
- Dumps and restore (RM-0074).
- Any HTTP route or MCP tool for these acts, or for `spec team`.
- A root account (root is the host's shell).
- SMTP.
- Removing an entry (disable keeps it; deleting is by hand).
- Revoking one person's sessions (they live in memory; restart `authelia`).
- A stream's life (RM-0075).
- Changes to `hub_login_problem`, the daemon, or the fence.

## Implementation

To fill in after implementation.

### Files in scope

| Module | What it changes |
|---|---|
| `scripts/deploy/admin/hub-admin` | new: the acts, readers, writers, lock and log of "Data"; POSIX `sh`, 0755 |
| `scripts/deploy/docker-compose.yml` | service `admin`; `authelia`'s users mount `:ro`; the header comment's count |
| `scripts/deploy/authelia/config/configuration.yml` | `refresh_interval: always` |
| `scripts/deploy/.gitignore` | `admin-log/`, `traefik/dynamic/mcp-stamp.yml` |
| `.github/workflows/image.yml` | the third deploy test; C3's `sort -u` |
| CLI `src/main.rs`, `src/team.rs`, `src/lib.rs` | `Command::Team` and its plain and JSON rows over `team_read` |
| http `tests/deploy.rs` | D7, D9, D11 as decided above |
| http new `tests/deploy_admin.rs` | AC1-AC14, AC16, AC18 |
| CLI `tests/team_config.rs` | AC15 |

The rust-developer writes `scripts/deploy/` (but its `README.md`), `image.yml` and the CLI command. The test-engineer writes the three test files. The spec-writer writes the guide and the canon at ship.

### The guide at ship

`scripts/deploy/README.md`, within 10 240 B (7 071 today):
- **"Install":** `install -d -m 700 secrets authelia/users traefik/mcp-users admin-log`, the secrets, then the first `admin add` before `up -d`.
- **"Accounts"** replaces "Accounts, by hand until RM-0065" (`:49-65`): the acts' table, enrolment by password and `code`, one person at a time; reset; the compromised order; recovery with everything lost; the `check` pipe; the `[team]` row and how it reaches the checkout (`--push`, or the by-hand `pull --no-rebase`).
- **"AI over MCP"** (`:67-89`): `mcp rotate` and its printed line, `--scope user`, `mcp revoke`.
- **"Limits"** gains a host from 11b-1's guide (remove an `authelia` entry you did not add).

### Canon at ship

Nothing below is written before this slice's ship commit. Free bytes are as of this spec; a page without room rewords in place, and no cap is raised.

- **`docs/canon/hub-deploy.md`** (1 545 B free):
  - `:15` "Three services and nothing else; RM-0065 adds a one-shot `admin`." becomes "Three services and a one-shot `admin` (`#accounts`).";
  - `:18`: the users file read-only, re-read on every request;
  - `:21`: `authelia_storage_key` to `authelia` and `admin`, and the two new ignored paths;
  - `:33` and `:39`: the users file written by `hub-admin`;
  - `:83`'s touch becomes the stamp, with VF2's finding;
  - a new `<a id="accounts"></a>` "## Accounts": the six guarantees, who writes what, the acts and exits, the lock and the log, the check pipe, ADR `hub-accounts`, at most about 1.2 KB, details left to the guide;
  - "Tests" gains `deploy_admin.rs`;
  - "Known limits": a reset ends no session; the notifier keeps one notice; streams until RM-0075.
- **`architecture.md`** (4 B free), `#distribution`: the authentication paragraph's list gains the new ADR (10 B). Room in the same sentence: "needs infrastructure in front, per service" becomes "needs infrastructure per service" (ADR-0034's own words, 10 B).
- **`hub.md`** (371 B free):
  - `:51` "RM-0065's `spec admin` should refuse them at account creation." becomes "`hub-admin` refuses them at account creation by an ASCII grammar (`hub-deploy.md#accounts`).";
  - `:68`: "the login door owns accounts, sessions, passkeys and recovery" gains ", its acts run from the host's shell (`hub-deploy.md#accounts`)";
  - `:104` stays on RM-0075 until 11c-2.
- **`hub-team.md`** (13 B free), "Team": one sentence on `spec team` ("`spec team` prints the logins, read-only, for `hub-admin check`"). Room: the intro's "no real hub exists before step 11b-3, so this is a mechanism and its proofs" becomes "no real hub exists before step 11b-3".
- **`hub-mcp.md`** (16 B free): `:128` stays on RM-0075; "Known limits"' Basic user line is unchanged.
- **Tier 1:** the CLI README (230 B free) lists `team` among the commands. The http README (201 B free) "Tests" gains `deploy_admin.rs`.
- **`CLAUDE.md`:** the state line names `hub-accounts` and the new ADR, and "the hub goes on with 11c-2 `stream-life` (RM-0075)".
- **Plan:** RM-0065 shipped and compressed; 08's row 11c-1 shipped; this file compressed.
- **Unchanged:** ADR-0034, ADR-0059, ADR-0060 and ADR-0067, never edited. The new ADR supplements them and closes ADR-0067's "kept by hand until RM-0065".

### Draft ADR

**A draft, awaiting the owner's confirmation** (08's row: owner-confirmed). It takes the next free number, the one after ADR-0067 as of this spec, and its date at ship. It is issued in the ship commit once the owner confirms. If the owner has not confirmed by then, the orchestrator decides whether to issue it as a working answer, with `ref:` saying the confirmation is open. 1 318 B with its front-matter:

```markdown
---
class: decision
id: ADR-0068
title: A hub's accounts are the deployment's script, run from the host's shell in a one-shot container with no network
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/hub-deploy.md#accounts
ref: hub-accounts (RM-0065, step 11c-1); the orchestrator's decisions of 2026-11-03 (Q1-Q8); confirmed by the owner on 2026-11-DD
---

# ADR-0068

**Decision.** Supplements ADR-0034, ADR-0059 and ADR-0067. A hub's account acts are `scripts/deploy/admin/hub-admin`, POSIX `sh` driving the door's own CLI, run by `docker compose run --rm admin` in a one-shot service with no network, port or restart, and refused without a terminal: root is the host's shell. No crate holds account code; SpecEngine adds only the read `spec team`. The door re-reads a person on every request, and a hub's streams end after a maximum life (RM-0075).

**Why.** The owner asked for a root CLI on the server that only a container reaches. A `spec admin` would put accounts in SpecEngine and in the daemon's binary, against ADR-0034 and ADR-0059, with hash crates and the door's SQLite.

**Cost.** Shell over the door's files and CLI flags, re-recorded at every pin bump; a `[team]` row is still a reviewed commit; a reset ends no live session; a stream outlives a disable for up to its life.
```
