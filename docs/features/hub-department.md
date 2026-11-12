---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, fixtures, ui]
ref: RM-0089, v5 core slice 12 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the analyst's analysis of 2026-11-12 (section RM-0089, HD1 to HD5); the orchestrator's working answers, the owner's review open
adrs: []
---

# hub-department: a hub login lands on its own department (RM-0089)

## Why

On a hub every person lands on the first department and picks their own from the "Department" select; the choice is per browser (`docs/canon/workspaces.md` "Screen"), so two logins in one browser share it. After this slice a `[[workspace]]` may list `logins`; the read serves only the asker's `default`, never the map, which would publish who works where. A person with no stored choice lands on it, and a hub keeps the choice per login.

A department stays a lens, never an entitlement, a control point or a filter (ADR-0073, ADR-0076); `[team]` stays the acts' entitlements, unread here (ADR-0060); the login is the one the hub trusts (ADR-0059). No ADR: ADR-0073 reserves "a hub login's default" for RM-0089, and nothing reads `[team]` or stores per-login state on the hub.

## Description and interactions

### Decisions

- **D1. `[[workspace]] logins`**, a reviewed commit. Each entry is judged by `hub_login_problem` (core `card.rs`, `docs/canon/hub.md` "Identity"), so `agent`, `local` and `-` are left out with a note; a login in two workspaces lands in the first, in config order, with a note.
- **D2. Only the asker's default.** `spec workspace --login L` adds `default`, the workspace whose `logins` holds `L` byte for byte, else `null`. `GET …/workspaces` passes the hub's login (`hub.rs` `Login`), none locally. No login list is served; a note names an entry's position, never its login, since every login reads the notes.
- **D3. A stored choice wins**: the stored workspace while served, else `default` while served, else the first. A default that overrode a choice would read as an assignment.
- **D4. On a hub the keys are per login**: `specengine.workspace.<p>.<login>`, `specengine.module.<p>.<login>`; locally unchanged. The module key follows, since choosing a workspace removes it.
- **D5. No new word.** The select stays "Department": no "View as", "role", "yours" or "assigned"; no catalog string.

## Data

### Config (core `workspace_toml.rs`)

`[[workspace]]` gains `logins`, absent meaning none (`WORKSPACE_KEYS` 7). The example is `fixtures/card-press`'s two tables, their other keys left out:

```toml
[[workspace]]
name = "newsroom"
title = "Newsroom"
sections = ["board", "tree", "inbox"]
logins = ["mira", "agent", "mira", "ann "]

[[workspace]]
name = "pressroom"
title = "Pressroom"
sections = ["board", "flow", "tree", "health"]
logins = ["mira", "ann"]
```

Judging as `workspaces.md` "Judging": a `logins` that is no array of strings, or holds more than 256 (`MAX_LOGINS`, as `TEAM_ENTRIES_MAX`; `` `logins` holds at most 256 entries ``), is a malformed table: the implicit workspace, one note. Then each entry, in config order, is left out with a note when `hub_login_problem` refuses it, when its workspace lists it twice, or when an earlier workspace lists it:

```text
<config>:<line>: workspace `<w>`: logins[<i>]: <hub_login_problem's message>: left out
<config>:<line>: workspace `<w>`: logins[<i>] is listed twice: the second left out
<config>:<line>: workspace `<w>`: logins[<i>] already lands in `<v>`: left out
```

`<i>` is 0-based. `hub_login_problem`'s messages echo no login except its own words (`agent`, `local` and `-` in any case). The example gives `mira` -> `newsroom`, `ann` -> `pressroom`, and four notes: `logins[1]` (`agent`), `logins[2]` (twice), `logins[3]` (`white space around the login`), and pressroom's `logins[0]` (already in `newsroom`).

Core: `WorkspaceConfig` gains `logins: BTreeMap<String, String>` (login -> workspace), never serialized, and `default_for(&self, login) -> Option<&str>`. `Workspace` is unchanged, so no served workspace holds a login; the implicit one has none.

### Read

- **CLI** `workspace.rs`: `spec workspace [--json] [--login L]`, `WorkspaceRequest {login: Option<String>}`. `L` is any string, never refused, never printed.
- **JSON** `{source, notes, workspaces, modules, default}`, `default` last: `null` without `--login` or with no match.
- **Text**: with `--login`, a second line `default <name>` or `default (none)`; without it, byte-equal to today's.
- **Route** `GET /api/projects/:p/workspaces` (http `app.rs` `workspaces`): on a hub `spec workspace --json --login <Remote-User>` byte for byte, locally `spec workspace --json`. Still no query (400), no turn, no `[team]` read. `WorkspaceDocument` 4 -> 5 keys (`fixtures/daemon-keys.json`).

### The UI

- **Types**: `provisional.ts` `WorkspaceDocument.default: string | null`, citing HD1.
- **Choice** (`src/workspace/choice.ts`): `workspaceKey(project, login)`, `moduleKey(project, login)`, `login: string | null`, null giving today's key; `workspaceOf(document, stored)` per D3; `storedWorkspace(project, login, document)` and its caller `SplitDialog` follow.
- **The login**, from `useIdentity()` (`board/acting.ts`): pending, the choice waits with the nav's skeleton; answered, its `login` (`null` locally); failed, no key is read or written, the choice held until a reload (as storage refusing), the landing on `default`.
- **Landing**: the `#/` replacement takes the chosen workspace's landing, so a first visit lands on the default's.
- **Mock**: `hub` (login `mira`) serves a `default` that is not the mock's first department; other scenarios `null`.

## Rules and edge cases

- WHEN a hub serves `…/workspaces`, the system SHALL serve `default` for that request's login alone, and no `logins` key or other login.
- WHEN `[team]` is missing, bad or silent on the login, the route SHALL answer as with a good one: the default and `[team]` never read each other.
- WHEN a login has a default, every section SHALL keep its route, palette entry and view; no redirect, right, gate or finding changes (ADR-0012).
- A login is compared exactly: `Mira` is not `mira` (`hub.md` "Identity"). On a hub origin the old browser-wide keys stay unread, so each login lands once on its default.

**Rollout.** An older binary judges `logins` an unknown key, so the whole `[[workspace]]` is malformed: the implicit workspace, every section but Tasks, with a note (`workspaces.md` "Judging"); one older than `workspaces` refuses the config, and a `v0.29.1` hub answers 503. ADR-0073's rule holds: **commit no `logins` before every hub, terminal and plugin serving the project runs a build that reads it**. The gate's hook is unaffected.

**Test churn.** `daemon_keys.rs`, `fixtures/daemon-keys.json`, `daemonKeys.test.ts`; `card-press` and `studio-lamp` expectations holding workspace JSON (`"default": null`); `workspace_cli.rs`, http `workspaces.rs`, `choice.test.ts`, `shell.test.tsx`, the `SplitDialog` tests.

## Acceptance criteria

A browser look is marked **[look]**. Every other criterion is a test that the named mutation turns red. Corpora are scratch copies of `fixtures/card-press` with Data's two `logins` lines added. Logins are made up.

- [ ] HD1 — **The asker's default.** `spec workspace --json --login mira` gives `"default": "newsroom"`, `--login ann` `"pressroom"`, `--login eve` and no `--login` `null`. On a test hub (`tests/common/hub.rs`) `GET …/workspaces` as `mira` is byte-equal to the `--login mira` run, as `eve` to `--login eve`'s; a local daemon's to the run without. Mutation, red: the route ignores the login.
- [ ] HD2 — **`[team]` unread.** That hub with a bad `[team]` (a `[team.mira]` whose `role` is `chief`) answers `GET …/workspaces` as `mira` 200 with `"default": "newsroom"`, while `GET …/team` is the 503. Mutation, red: the default kept only for logins `[team]` lists.
- [ ] HD3 — **A stored choice wins** (`choice.test.ts`, `shell.test.tsx`). No key: `#/` lands on the default's landing; a stored served name is kept; one no longer served falls to the default, an unserved default to the first. As `mira`, choosing writes `specengine.workspace.<p>.mira`, removes `specengine.module.<p>.mira`, leaves `specengine.workspace.<p>` alone; with `{login: null}` it writes `specengine.workspace.<p>`. Mutations, each red: the default overrides a stored choice; the key ignores the login.
- [ ] HD4 — **A lens, not a filter.** `crates/specengine-http/tests/hub_team.rs` and `hub.rs` pass with no line changed. In the `hub` scenario each of the nine section routes renders its view, no redirect, the palette listing all nine. Mutation, red: a section off the default's nav redirecting to its landing.
- [ ] HD5 — **Entries judged** (core `workspace_toml.rs` tests). Data's example gives `mira` -> `newsroom`, `ann` -> `pressroom` and exactly its four notes, by line; `logins = "mira"` and 257 entries each the implicit workspace and one note. Mutations, each red: `agent` accepted; a later workspace winning a repeated login.
- [ ] HD6 — **Never the map.** As `eve` the route's body holds no `mira`, `ann` or `logins`; as `mira`, no `ann`; no note holds a login `hub_login_problem` accepts. Mutations, each red: `logins` serialized on `Workspace`; a note naming the login.
- [ ] HD7 — **Text as pinned.** `--login mira` prints `default newsroom` second, `--login eve` `default (none)`; without `--login`, byte-equal to `card-press`'s pinned text. Mutation, red: the line printed without `--login`.
- [ ] HD8 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900: `?scenario=hub` lands on the mock's default; a switch survives a reload; cleared storage lands on the default again; `?scenario=normal` on the first; no new word by the select.
- [ ] HD9 — **Gates.** `cargo nextest run` green for `specengine-core`, `-cli`, `-http`, then the full run once; the eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass; UI lint, `tsc`, `pnpm test`, `pnpm build` pass, first-load JS within 845 kB.

## Out of scope

Preferences stored on the hub or any write route; reading `[team]`, a role or an entitlement; a "View as" select; a local viewer's default; `logins` in `scripts/presets/` (a project's people); an MCP tool; a `spec check` finding; moving the old browser-wide keys.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/workspace_toml.rs`; `crates/specengine-cli/src/{workspace,main,lib}.rs`; `crates/specengine-http/src/app.rs` |
| test-engineer, Rust | core `workspace_toml.rs`, CLI `workspace_cli.rs`, http `workspaces.rs` (a hub case through `tests/common/hub.rs`); `daemon_keys.rs` and `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`, once) with `daemonKeys.test.ts` in the same change; the `card-press`, `studio-lamp` expectations |
| ui-developer | `provisional.ts`, `src/workspace/choice.ts`, `src/app/Shell.tsx` (the login), `src/board/SplitDialog.tsx`, `src/mocks/MockClient.ts` |
| test-engineer, UI | `choice.test.ts`, `shell.test.tsx`, `SplitDialog` tests; HD3, HD4's mutations |

Comments cite `docs/features/hub-department.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. After core slice 11 `department-overview` ships: both edit `src/workspace/` and the Shell's landing.
2. Rust core, CLI, http; the UI behind `AHEAD_OF_FIXTURE`; tests overlap per crate; reviews; the look (HD8); ship ("Canon at ship"). At most three iterations a lane. Its value is on a hub, after the owner's gate 11b-3.

### Risks

- **Older binaries** collapse every department while `logins` is committed early ("Rollout"); the upgrade order is the owner's.
- **Wording**: a "(default)" marker would read as an assignment, so D5 adds none.

### Canon at ship

| File | Edit |
|---|---|
| `docs/canon/hub-department.md` (new, Tier 2, `hub*` shard) | `logins`, judged; `default` on the read and the route; never the map; the UI's order and per-login keys; "Known limits" (older binaries, a browser's old keys); "Tests" |
| `docs/canon/workspaces.md` [14] | "The lens": "a hub login's default is RM-0089's" becomes a pointer; "Config": `logins` in the table, by pointer. Net |
| `docs/canon/hub.md` [103] | "Screen": one pointer |
| `ui/README.md` "Shell" | the keys `workspace.<p>[.<login>]`, `module.<p>[.<login>]` |
| READMEs: core, cli, http | a pointer each, net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 12; RM-0089 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q1. Where does the default live? | `[[workspace]] logins` | Preferences on the hub: a write route, personal state, an ADR on ADR-0059 |
| Q2. Does the default beat a stored choice? | No; on a hub the stored key is per login | A default that overrides reads as an assignment, close to an entitlement |
| Q3. Serve the map? | Only the asker's `default` | The map publishes who works where to every login |

**The spec-writer's:**
- W1: a malformed `logins` collapses the table, as every other key does. Only its entries are left out one by one. A softer rule for one key would be a second judging model.
- W2: the module key goes per login too (D4). The brief named only the workspace key, but a shared module key would carry one login's module into another's department.
- W3: notes name an entry's position, never its login (D2, HD6), since `notes` reach every login.
- W4: `--login` is never refused. A value no entry can hold just gives `null`, and the value is never printed.
