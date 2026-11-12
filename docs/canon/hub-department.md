---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# Hub department: a hub login lands on its own department

Built by `hub-department` (RM-0089), no ADR: ADR-0073 reserved a hub login's default for it. A department stays a lens, never an entitlement, a control point or a filter (ADR-0073, ADR-0076, ADR-0012); `[team]` stays the acts' entitlements and is never read here (ADR-0060); the login is the one the hub trusts (ADR-0059, `hub.md` "Identity"). The rest of the lens: `workspaces.md`. Code: core `workspace_toml.rs`; CLI `workspace.rs`; http `app.rs` `workspaces`; UI `src/workspace/choice.ts`, `src/app/Shell.tsx`.

## Config

`[[workspace]] logins`, the hub logins landing there, absent meaning none: the seventh of `WORKSPACE_KEYS`, so an unknown key's message ends `` …, `module`, `logins` ``. At most 256 (`MAX_LOGINS`, as `TEAM_ENTRIES_MAX`). A malformed `logins` makes the table malformed (the implicit workspace, one note, `workspaces.md` "Judging"); its problem names a type and a position, never the value:

- `` `logins` is <type>, not an array of strings ``
- `` `logins[<i>]` is <type>, not a string ``
- `` `logins` holds at most 256 entries ``

Then, after sections, landing, flows and module, each entry in config order is left out with a note when `hub_login_problem` refuses it (`agent`, `local`, `-` in any case, white space around it, ...), when its workspace lists it twice, or when an earlier workspace lists it, the first in config order winning; `<i>` 0-based:

```text
<config>:<line>: workspace `<w>`: logins[<i>]: <hub_login_problem's message>: left out
<config>:<line>: workspace `<w>`: logins[<i>] is listed twice: the second left out
<config>:<line>: workspace `<w>`: logins[<i>] already lands in `<v>`: left out
```

`WorkspaceConfig.logins` (login -> workspace, never serialized), `default_for(login)`: the workspace whose kept entries hold the login byte for byte (`Mira` is not `mira`). `Workspace` is unchanged, so no served workspace holds a login; the implicit one has none. The tests' example, `fixtures/card-press` with `newsroom` `logins = ["mira", "agent", "mira", "ann "]` and `pressroom` `["mira", "ann"]`: `mira` -> `newsroom`, `ann` -> `pressroom`, four notes (newsroom's `logins[1]`, `[2]`, `[3]`; pressroom's `logins[0]`).

## Privacy

Every login reads the notes, so none names a login: a refused entry echoes only `hub_login_problem`'s own words, a malformed one a type and a position, and a config that is not TOML the reader's `message()`, no source snippet. No `logins` key or login list is served; a read serves the asker's `default` alone. The notes reach `spec workspace`, `GET …/workspaces`, `spec delivery` and its route (`spec-delivery.md`), positions only in each.

## Read

`spec workspace [--json] [--login L]` (CLI `workspace.rs`, `WorkspaceRequest {login}`): `L` any string, never refused, never printed. JSON `{source, notes, workspaces, modules, default}`, `default` last (`WorkspaceOutcome.default`): `default_for(L)`; `null` without `--login`, with no match, or when the implicit workspace is served. Text: with `--login`, line 2 `default <name>` or `default (none)`; without it, as before. `--login` takes hyphen values, so `--login --json` takes `--json` as the login: `--login=VALUE`.

`GET /api/projects/:p/workspaces`: on a hub `spec workspace --json --login <login>`, the login of the fence's `Login` extension (`login_of`, as every hub route); a local daemon has none, so `default` `null`. Still config only: no turn, no query, no `[team]` read, so a bad `[team]` leaves it 200 while `…/team` is 503. `WorkspaceDocument` 5 keys (`fixtures/daemon-keys.json`).

## Screen

- **The login**, `useIdentity()`: a hub's, `null` locally. While it is out, a lens the login could change (`choosesByLogin`: two or more workspaces, or a module) waits with the nav's skeleton. A failed read reads and writes no key: the choice holds in the Shell until a reload, landing on `default`.
- **Keys**: on a hub `specengine.workspace.<p>.<login>`, `specengine.module.<p>.<login>`; locally `specengine.workspace.<p>`, `specengine.module.<p>` (`workspaces.md` "Screen"). A hub reads no browser-wide key, so each login lands once on its default.
- **Order** (`workspaceOf`): the stored workspace while served, else `default` while served, else the first. A default never overrides a stored choice: it would read as an assignment. `#/` takes the chosen workspace's landing, so a first visit lands on the default's; `storedWorkspace` and `SplitDialog` follow.
- **No new word**: the select stays "Department"; no "View as", role or default marker, no catalog string. Mock: `hub` (login `mira`) serves a `default` that is not its first department; the other scenarios `null`.

## Known limits

- **Older binaries** judge `logins` an unknown key, so the whole `[[workspace]]` is malformed: the implicit workspace with a note; one older than `workspaces` refuses the config (a `v0.29.1` hub: 503). **Commit no `logins` before every hub, terminal and plugin serving the project runs a build that reads it** (`workspaces.md` "Known limits").
- It matters on a hub, after the owner's gate 11b-3; a local viewer has no default.

## Tests

core `workspace_toml.rs`; CLI `workspace_cli.rs`, `departments.rs`; http `workspaces.rs` (a hub through `tests/common/hub.rs`, a bad `[team]`), `daemon_keys.rs`; `card-press`, `studio-lamp` expectations (`"default": null`); UI `workspace/choice.test.ts`, `workspace/shell.test.tsx`, the `SplitDialog` tests, `mocks/hubDepartment.smoke.test.tsx`. Not asserted: a note's line for an entry of a multi-line `logins` array. Criteria: `docs/features/hub-department.md`.
