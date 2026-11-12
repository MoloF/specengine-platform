---
class: spec
status: shipped
shipped: 2026-11-13
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, fixtures, ui]
ref: RM-0089, v5 core slice 12 (08 s2 Phase 7, "Owner's word, 2026-11-12")
adrs: []
---

# hub-department: a hub login lands on its own department (RM-0089)

## Why

On a hub everyone landed on the first department, one choice per browser. Now a `[[workspace]]` may list `logins`; a read serves only the asker's `default`, never the map; with no stored choice a login lands on it, and a hub keeps the choice per login. A lens, never an entitlement; `[team]` unread. No ADR: ADR-0073 reserved it. Canon: `docs/canon/hub-department.md`.

## Acceptance criteria

Corpora: scratch `fixtures/card-press` with the canon's two `logins` lines.

- [x] HD1 — `--login mira` `"default": "newsroom"`, `ann` `"pressroom"`, `eve` and none `null`; a test hub's route byte-equal per login, a local daemon's to the run without.
- [x] HD2 — A bad `[team]`: the route 200 with the default, `…/team` 503.
- [x] HD3 — A stored choice wins, then the default, then the first; per-login keys on a hub, today's with `login` `null`.
- [x] HD4 — `hub_team.rs`, `hub.rs` unchanged; the nine sections route, no redirect.
- [x] HD5 — The example's map and four notes; `logins = "mira"` and 257 entries: the implicit workspace, one note.
- [x] HD6 — As `eve` the body holds no `"logins":` key and no other login's value (a bare `logins` would trip on the notes' `logins[<i>]`); as `mira` no `ann`; no note holds an accepted login.
- [x] HD7 — `default newsroom`, `default (none)` second; without `--login` byte-equal.
- [x] HD8 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900: `?scenario=hub` lands on the mock's default; a switch survives a reload; cleared storage lands on the default again; `?scenario=normal` on the first; no new word by the select. Two passes: light, English; dark, Russian (the default's Overview, a switch kept).
- [x] HD9 — Gates: the full run green (5 970; two pins and a README line fixed, two timing failures green alone), eval (`workspace_names` too), `spec check`, clippy; UI 5 712/5 712, build within 845 kB.

## Implementation

| Module | What it does |
|---|---|
| core `workspace_toml.rs` | `logins`, the seventh key, `MAX_LOGINS` 256, its notes; `WorkspaceConfig.logins`, `default_for` |
| CLI `workspace.rs`, `lib.rs`, `main.rs` | `--login L`, `WorkspaceRequest {login}`, `WorkspaceOutcome.default` last |
| http `app.rs` | `workspaces` passes `login_of(&request)` on a hub |
| UI `workspace/choice.ts`, `app/Shell.tsx`, `board/SplitDialog.tsx`, `provisional.ts`, mocks | per-login keys, `workspaceOf`, `choosesByLogin`, the identity wait |

**Departures:** malformed `logins` notes name a type and a position, never the value, unlike the TOML reader's text; JSON without `--login` gains `"default": null` (`WorkspaceDocument` 5 keys); a pending identity holds only a lens the login could change (`choosesByLogin`).

**Untested:** a note's line for an entry of a multi-line `logins` array.

**Open:** `--login --json` takes `--json` as the login (`--login=VALUE`); value on a hub after gate 11b-3; commit no `logins` before every binary reads it; Q1-Q3 await the owner.
