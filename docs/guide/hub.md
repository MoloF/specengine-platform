---
class: canon
tier: 2
scope: [guide]
owner: owner
reviewed: 2026-11-24
summarises: [docs/canon/hub.md#identity, docs/canon/hub.md#trust, docs/canon/hub-team.md#team, docs/canon/hub-team.md#acts, docs/canon/hub-confirm.md#rules, docs/canon/hub-mcp.md#tools, docs/canon/hub-ops.md#dumps, docs/canon/hub-ops.md#upgrade, docs/canon/hub-restic.md#the-backend, docs/canon/hub-deploy.md#accounts]
---

# The hub

A hub is one shared SpecEngine server behind a login door, where the whole team works on the spec in a browser and each person's agent works over MCP under that person's login. People read, move cards, raise questions and confirm decisions there without a terminal. This page covers logins and `[team]`, confirming, MCP on a hub, and running one: accounts, upgrades and dumps.

## What it solves

Writers, artists and producers have no terminal and no checkout, yet they decide much of a game's spec. Without a shared server, every decision passes through the one person who has `spec` installed. A hub serves the same UI and tools to everyone, records each act under the person who made it, and keeps the one-door rule: a spec file changes only when an entitled person confirms.

## When to use it

- **More than one person** works on the spec, or anyone works without a terminal.
- **A remote team**: people and their agents reach one project from anywhere.
- **Not for a solo laptop**: there `spec serve` runs the same UI locally with no login.

## How

### CLI

The operator runs the hub with Docker Compose from `scripts/deploy/` (its `README.md` is the full guide). Accounts are made on the host:

```sh
docker compose run --rm admin add mira --name "Mira K" --email mira@example.org
docker compose run --rm admin mcp rotate mira
docker compose exec -T specengine spec team --root /projects/lamp-game | docker compose run --rm -T admin check
```

`add` prints the password once and the person's `[team]` row; `mcp rotate` prints their MCP secret once. Bringing a project over: decide its open proposals on the laptop, `spec export state > lamp-game.jsonl`, clone the repository inside the image, then `spec import-state` into the hub's empty queue.

### MCP

Each person connects Claude Code with their own secret, at user scope:

```sh
claude mcp add --transport http --scope user hub-lamp-game https://hub.example.org/mcp/lamp-game \
  --header "Authorization: Basic <printed>"
```

The hub serves the reads (`get_tree`, `get_node`, `search`, `get_context_bundle`, `get_roadmap`, `get_flow`, `get_trace`, `get_proposal`), the queue tools (`propose_change`, `ask_question`, `report_discrepancy`, `report_decision`), the card tools, the comment tools (`list_comments`, `add_comment`), the reference tools (`list_references`, `get_reference`, `add_reference`, links only) and `get_guide`. Each item is stored under the caller's login. No tool decides. Code tools and task tools stay on terminals: a hub holds no code index and runs no `T-` tasks.

### Screen

The same web UI as locally. Each login lands on its department (`[[workspace]] logins`). A staged choice shows Confirm to the people entitled to it. Confirm, a card's Accept and the clears ask for the second factor at the door when the session has only a password.

## Where it lives

On the host: the proxy (Traefik), the login door (Authelia), the SpecEngine image, and sidecars that dump the queue and the door every 15 minutes. A `backup` sidecar sends the dumps off the host with restic once a backend is named in `secrets/restic_env`; until then it waits, unhealthy, saying why each round. Projects are checkouts under `/projects/<name>`; the queue and index are in a data volume, references' files in a volume of their own, `specengine-blobs`, which no dump or backup carries: they are lost with the host, so keep your originals. `.env` pins `SPECENGINE_VERSION` and turns on `SPECENGINE_PUSH`.

In the project, the people who act beyond plain writes are listed in the committed `specengine.toml`:

```toml
[zones]
narrative = ["docs/spec/narrative/**"]
art       = ["docs/spec/art/**", "docs/spec/practices/art/**"]

[team.ann]
role  = "owner"
email = "ann@example.org"

[team.mira]
name  = "Mira K"
role  = "lead"
zones = ["narrative"]
email = "mira@example.org"

[team.leo]
role  = "member"
email = "leo@example.org"
```

### The login is the person {#GD-HUB-LOGIN-IS-THE-PERSON}

SpecEngine holds no accounts. The login door authenticates, and the hub trusts the login the proxy passes; that login is recorded on every act, verbatim. A renamed login is another person. `agent` and `local` are never logins.

### `[team]` decides who acts {#GD-HUB-TEAM-ROLES}

Any login reads, proposes, asks, and creates, moves, edits and splits cards. A `member` of `[team]` also stages choices, answers rounds, accepts the cards it reviews and clears marks. A `lead` also confirms and group-clears inside its zones. The `owner` does all of it, anywhere, including paths in no zone. A login absent from `[team]` has only the plain writes.

### Confirm is one click, one commit {#GD-HUB-CONFIRM-ONE-CLICK}

A lead or the owner confirms one staged choice at a time; the hub applies it as one commit in its checkout, under the login's name and `[team]` email. Some acts stay a terminal's even with a hub: an editorial approve, a drift item, a task-bound proposal, `spec task approve`, `spec round answer` and `spec import-state`.

### One MCP secret per person {#GD-HUB-MCP-PER-PERSON}

Each person's Claude Code gets its own secret, at `--scope user`. `mcp revoke` or `disable` cuts it off at the next call.

### Dump before every upgrade {#GD-HUB-UPGRADE-WITH-DUMP}

Upgrade by the runbook: a dump of the queue and the door on the old version (with off-host backups off, copy the dumps off the host by hand), then the new tag, `SPECENGINE_VERSION`, `docker compose pull` and `up -d`. Upgrade the hub before any terminal starts using a config key the old build refuses. A newer queue refuses an older binary, so the dump is your way back. Never remove `specengine-blobs`.

### Turn off-host backups on deliberately {#GD-HUB-OFFSITE-BACKUPS}

Off-host backups stay off until you name a backend. To turn them on: choose a restic backend; write `RESTIC_REPOSITORY` and its keys into `secrets/restic_env` in place, or run `docker compose restart backup` after editing it; keep that file, `restic_password` and `.env` off the host too; run `docker compose run --rm backup init` once. After the next round, `docker compose run --rm backup snapshots` lists `queue` and `door`, and `backup` turns healthy. Rehearse a restore on a scratch host before you rely on it.

## What not to do

### Never share an account {#GD-HUB-NO-SHARED-ACCOUNTS}

One login, one person. A shared login makes every `Decided-by` a lie, and the hub cannot tell a person from a script holding their session. Disable a departing person's account the day they leave.

### Never commit an MCP secret {#GD-HUB-NO-SECRET-IN-REPO}

A project's `.mcp.json` is committed, so a hub secret there is everyone's. Keep it in the person's own settings.

### Never move a project with open proposals {#GD-HUB-DECIDE-BEFORE-MOVING}

A proposal belongs to the worktree it was raised in. Brought to a hub open, it arrives an orphan that can only be rejected. Decide or reject everything open, then export.
