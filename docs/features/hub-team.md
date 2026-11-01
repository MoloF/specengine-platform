---
class: spec
status: shipped
shipped: 2026-11-01
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: RM-0071; Phase 7 step 9a (08 s2); owner's decisions 2, 3, 5 and instruction of 2026-11-01
adrs: [ADR-0060]
---

# hub-team: a `[team]` login accepts, clears and stages on a hub, and no spec file changes (RM-0071)

## Why

A hub gave nobody power over the brief; the owner's decisions 2, 3 and 5 need zone leads and reviewers to act with no terminal. This slice opens accept, clear, stages and round answers to the logins `[team]` names and changes no spec file, a terminal on the host confirming; RM-0050 then confirms one staged choice. A mechanism and its proofs: no route proposes before RM-0053, no hub stands before RM-0064.

## Description and interactions

Canon: `docs/canon/hub-team.md`.

### Classes

`docs/canon/hub-team.md` "Classes".

## Data

`[team]`, `GET .../team`, refusals: `docs/canon/hub-team.md` "Team", "Answers". No queue, index, `spec.lock` or `daemon.json` bump.

## Acceptance criteria

- [x] A1 — `from_toml` errs at the line on an unknown key, a bad role, a `lead` with no zones, `owner` zones, an undeclared zone, a bad email or login. `spec check`: one `team-config`, exit 1; reads, `spec card`, gate as before.
- [x] A2 — Another name on accept, clear, group clear: 403; null `as` records the login. Stages log `as`; a local event stays byte for byte.
- [x] A3 — Outside `[team]`: 403 on six acts; five writes, reads 200. A `member`'s group clear, a `lead` outside its zones: 403; `owner` 200.
- [x] A4 — Two roots, two teams. Bad `[team]`: 503 there, plain writes 200.
- [x] A5 — `.../accept?x=/move`, no `Origin`, every raw form: 403.
- [x] A6 — Null `revision` or `count`: 400; a count that differs: 409.
- [x] A7 — Local: `[team]` changes nothing, `GET .../team` is a 404; `tests/fence.rs`, `methods.rs`, `door.rs`, `fn fenced` undiffed.
- [x] A8 — A suspect card moves and is accepted; an open one's retarget or cancel reads `no card`; an accepted one refuses both.
- [x] A9 — `hubIdentity.test.tsx`: read-only names, no `NAME_IS_TYPED`, idle controls per `allowed`; local as before.

## Implementation

Past the draft, each in canon: a group clear takes the read's word, in both modes; `member` is judged again past the body; a cover binds a group clear alone; `--staged`, `--changed` judge `team-config` on the checked side.

### Files in scope

| Module | What it does |
|---|---|
| core `team_toml.rs` (new), `gate.rs`, `card.rs` | `TeamConfig`, `zones_from_toml`, `Zone::covers`, `hub_login_problem` |
| store `check.rs`, `queue/stage.rs` | `CheckSetup.team`, `stage_as`, `unstage_as` |
| CLI `team.rs` (new), `card.rs`, `trace.rs` | `team_read`, `TeamRow`, `group_marks`, `CardCause::Outside` |
| http `hub.rs`, `app.rs`, `answer.rs` | `class`, `member`, `needs`, `GET .../team` |
| UI `board/`, `trace/`, `inbox/`, `round/`, `styles/` | `useAllowed`, `ActOff`, `.field-fixed` |
| Tests | `hub_team.rs`, `team_toml.rs`, `team_config.rs` |
