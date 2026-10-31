---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: RM-0071; Phase 7 step 9a (08 s2); owner's decisions 2, 3, 5 and instruction of 2026-11-01
---

# hub-team: a `[team]` login accepts, clears and stages on a hub, and no spec file changes (RM-0071)

## Why

Today a hub gives nobody power over the brief (`docs/canon/hub.md` "Routes"); the owner's decisions 2, 3 and 5 need zone leads and reviewers to act with no terminal. This slice gives accept, clear, stages and round answers to the logins `[team]` names, changes no spec file and ships alone, a terminal on the host confirming; `docs/features/hub-confirm.md` (RM-0050) then lets a login confirm one staged choice.

No route proposes until RM-0053 and no real hub exists before RM-0064: step 9 delivers a mechanism and its proofs.

Working answers, the owner's review open: three roles, group clear above `member`; a required `email`; a session-level second factor.

## Description and interactions

`...` is `/api/projects/:p`.

| Act | Route on a hub | Who | Class |
|---|---|---|---|
| Five plain writes | as built | any login | plain |
| Stage, unstage | POST, DELETE `.../proposals/:id/decision` | any `[team]` login | team |
| Round answers | POST `.../round/answers` | any `[team]` login | team |
| Accept | POST `.../cards/:id/accept` | a `[team]` login, the card's reviewer, never its assignee | confirm |
| Clear | POST `.../cards/:id/clear` | any `[team]` login | confirm |
| Group clear | POST `.../trace/clear` | a `lead` whose zones cover every node it re-freezes; `owner` | confirm |

**Roles.** `member`: all above but group clear. `lead`: also group clear inside its zones. `owner`: all, unzoned paths too. A gone target or a path in no zone is `owner` only.

**Classes.** `Hub::fenced` (`hub.rs:123`) keeps default deny and exact raw segments. A plain route adds `hub::acting`; a team route, membership; a confirm route, in the fence no query and a required `Origin`, in the handler membership, `hub::acting`, and `revision` or `count`.

The confirm class is the acts taking a `Consent`; a stage takes none. RM-0050 adds its fourth route and RM-0064 the proxy's step-up on the four, which the daemon cannot see: a marker header is a second one to scrub (ADR-0059). It gives the proxy one byte form: the fence passes the path alone (`app.rs:215`), so `.../accept?x=1` would reach the handler while a `$`-anchored rule misses it (`tests/hub.rs:1645`). `Origin` is byte for byte (today absent passes; a browser sends it on a POST), with `Sec-Fetch-Site: same-origin`, JSON, no CORS.

**Screen.** The login replaces the typed name: "Accepting as", "Clearing as" read-only, login and `name`; `NAME_IS_TYPED` gives way to `This is recorded under your login.`; ADR-0057 stands locally. Controls follow `GET .../team`; the route's 403 stays the truth. `CardAcceptBody` defaults to `local` (`card.rs:544`), so `hub::acting` runs before `.request()` in the three handlers (`app.rs:1016-1050`). The page sends `revision` (`CardDialogs.tsx:655,769`); the group count, shown at `:877`, is not sent today.

**A suspect mark** stays as built, no consent added: an accepted card refuses `edit` and `cancel`, so `confirmed` is never reached (`crates/specengine-cli/tests/card_trace.rs:497`), and a retarget keeps its entry, so a returning target marks again (`two_edits_cannot_clear_a_mark`). Refusing them would be a discrepancy blocking work (ADR-0012).

## Data

```toml
[zones]                     # the gate's table, read here whether or not [gate] is on
brief = ["docs/spec/**"]
maps  = ["docs/spec/maps/**"]
[team.mira]                 # the key is the login, byte for byte; quoted when not bare
name  = "Mira K"            # shown only, a card person's grammar
role  = "lead"              # owner | lead | member
zones = ["maps"]            # lead: required, each a [zones] key; owner, member: refused
email = "mira@example.org"  # required: one @, printable ASCII, no < >
```

**Reader.** Core `TeamConfig::from_toml`, read only by the acts that need it; `RawFile` (`project_toml.rs:284`) only name-checks `team`, and older binaries refuse the file. Closed keys, at most 256 entries. `zones_from_toml` and a new `Zone::covers` leave `gate.rs` with their messages; gate `off` keeps `[zones]` unread.

**A login key** passes `hub_login_problem`, moved from CLI `card.rs:1266` into core with `is_format_character` and re-exported, and holds no `<`, `>` or end character git trims (`.,:;"'\`), which would change `Decided-by`; `email` takes the same end rule. `email` is required: `<login>@<public host>` is unjudgeable by `spec check`. A forge's no-reply address is fine.

**`GET .../team`,** a hub only, the asker's own row (`GET_ROUTES` 24; `Team`, set 108 of `daemon-keys.json`):

```json
{"login":"mira","name":"Mira K","allowed":["stage","accept","clear","group_clear"]}
```

`member` the first three, `lead` and `owner` all four; RM-0050 adds `confirm` for those two. Outside `[team]` or no `[team]`: 200, `name` null, `allowed` empty.

**Bodies, events.** `CardClearGroupBody` gains `count`, sent on a hub alone; its request also the entry whose cover is judged. `StageRequest`, `UnstageRequest`, `RoundAnswersRequest` gain `actor`, `None` locally: `proposal.staged`, `.unstaged` add `as` on a hub only.

| When | Answer, nothing stored |
|---|---|
| Bad `[team]`, `GET .../team` too | 503 `specengine.toml:<line>: <message>` |
| No `[team]` | 403 ``refused: this project has no `[team]` `` |
| Outside `[team]` | 403 ``refused: <login> is not in this project's `[team]`: reads and plain writes`` |
| A `member`'s group clear | 403 ``refused: <login> is a `member`: this act needs a `lead` `` |
| Outside a zone | 403 ``refused: <login> leads <zones>; <path> lies outside them`` |
| Confirm class: a query, no `Origin` | 403 ``refused: this route takes no query``, ``refused: this route needs the Origin header`` |
| Null `revision` or `count` | 400 ``a hub needs `revision` ``, `count` alike |
| A count that differs | 409 `` `<group>` holds <n> suspect cards, not <m>; nothing changed `` |
| Off the list | 403 ``refused: this hub does not serve this route`` |

**`spec check`.** New `team-config`, an error at the line, `CHECK_CODES` 37 (`core/src/check/mod.rs:127`), carried by `CheckSetup`; never `cannot-check`: through `load_config` (`store/src/check.rs:179`) a typo would stop every run.

**Bumps.** No queue, index, `spec.lock`, `export state`, `daemon.json` or event type change.

## Rules and edge cases

- A project with no `[team]` opts in by a reviewed commit, the natural second switch.
- `[team]` SHALL be read per request from the root the route's slug names, no cache in `App`, through `answer::read` (`answer.rs:220`), before the turn. A bad one SHALL leave reads, plain writes and `spec card` as they are; a terminal is never judged, a shell on the host being the operator's.
- A group clear's cover and count SHALL be judged in `run_clear_group` (`card.rs:2732`), the set known, before its consent; a new `CardCause::Outside` is a 403 in `write_status` (`answer.rs:164`). No new `card_` name: `door.rs` stays unedited.
- **ADR-0012 holds.** `[team]` is read only by the acts above past the plain writes; no read, move, `by`, status, flow, gate or check of a discrepancy reads it. With no lead free, items stay open and a terminal may still confirm: a role is no second control point. The gate never reads `team-config` (ADR-0039).
- **ADR-0008, `by`.** Role words name authority over the queue, no trade; the UI branches on `allowed`. `by` stays literal: else a bad `[team]` stops a plain write.
- **Until 9b** a departing accept stores `Author::human()`; `card.accepted` names the login.

## Acceptance criteria

The `M:` names stay in this draft until ship. Compression (§6) may drop them and nothing is lost: each test keeps its `M:` in its doc comment.

- [ ] A1 — `TeamConfig::from_toml` errs at the line on an unknown key, a role outside three, `lead` with no zones, `owner` with zones, an undeclared zone, a bad email, a login git would alter. `spec check`: one `team-config`, exit 1, other findings listed; `spec show`, `spec card`, gate as before. M: routing it through `load_config`; dropping the zone lookup.
- [ ] A2 — On a hub, a name other than the login on accept, clear, group clear: 403, nothing changed; null `as` records the login. A stage and an unstage log `as`, the login; a local event stays byte for byte. M: dropping `acting` from any of the three; `as` on a local event.
- [ ] A3 — A login outside `[team]`: 403 on stage, unstage, round answers, accept, clear, group clear, nothing stored; five writes and every read 200, `GET .../team` with `allowed` empty. A `member`'s group clear, a `lead` outside its zones: 403; `owner` 200. M: an absent entry read as `member`.
- [ ] A4 — Two roots: `tomas` in `card-press`'s `[team]` alone accepts there, 403 on `lantern-keep` (`fixtures/spec-a`). Bad `[team]`: 503 on those routes, plain writes 200. M: a team cached in `App`; a plain write reading `[team]`.
- [ ] A5 — `POST .../cards/C-0001/accept?x=/move` with a good login: 403; every other form of `tests/hub.rs:1634-1645` stays 403; no `Origin` on the class: 403. M: `Hub::fenced` reading the path alone.
- [ ] A6 — Null `revision` on accept or clear, null `count` on a group clear: 400; a count that differs: 409, no card cleared. M: dropping the compare.
- [ ] A7 — Local: `[team]` in the file changes nothing for a typed `as` or `spec card accept --as`; `GET .../team` is the unknown-route 404. `git diff`: nothing in `tests/fence.rs`, `methods.rs`, `door.rs` or the local `fn fenced` (`app.rs:231`). M: judging `[team]` with no `Login`.
- [ ] A8 — A suspect card moves and is accepted; a retarget or cancel of an open one reads `no card`, never `confirmed`; an accepted one refuses both. M: opening `edit` on a closed card.
- [ ] A9 — `hubIdentity.test.tsx`: both fields read-only with the login, no `NAME_IS_TYPED`, Clear all and the stage controls off when `allowed` omits them; local as before. M: showing it on a hub.

## Out of scope

Confirm, `Decided-by`, `Author.login`: RM-0050. Push: RM-0051. `/mcp`: RM-0053. The real step-up: RM-0064.

## Implementation

To fill in after implementation.

### Files in scope

Core `project_toml.rs`, new `team_toml.rs`, `gate.rs`, `card.rs`, `check/mod.rs`. Store `check.rs`, `queue.rs`, `queue/stage.rs`. CLI new `team.rs`, `card.rs`, `stage.rs`, `round.rs`, `lib.rs`. HTTP `hub.rs`, `app.rs`, `answer.rs`. UI `board/` (`CardPanel` too), `trace/CardsBlock`, `inbox/ProposalCard`, `api/`, `mocks/`, `test/stubClient`.

Tests: new `hub_team.rs`, `Proxied` and `Setup` moved to `tests/common/hub.rs`. Rewritten: `tests/hub.rs` AC-08, the raw table, `src/hub.rs` `a_write_is_refused_until_listed`. `daemon_keys.rs`. Core `tests/verify_judge.rs:852-856` pins 36 and the last code.

### Canon at ship

Nothing here is written before this slice's commit.

```yaml
id: ADR-0060
title: A hub's [team] entitles a login to accept, clear and stage
canon: docs/canon/hub-team.md#team
ref: hub-confirm analysis 2026-11-01; working answers, the owner's review open
```

- **It** supplements ADR-0057, ADR-0059. Cost: a forged `Remote-User` now accepts; an assignee can name themself reviewer; older binaries refuse `[team]`.
- **`docs/canon/hub-team.md`**, new Tier 2, as `hub.md` has 81 B free. **`hub.md`:** the fence line, "Routes", "Screen"; "Known limits" loses "A mark leaves without `clear`" and "Dialogs".
- **`spec-trace-record.md`** "Dropped entries": "neither `edit_card` nor two edits can remove a mark" holds of the hash alone.
- **No growth:** `cards.md` (4 B free), `decision-staging.md` (32 B), `spec-check.md` (20 B), `spec-check-process.md`, `spec-check-graph.md` 36 -> 37. Also `07-interfaces.md:137`, `board-ui.md`, `gate.md`.
- **Tier 1:** http `README.md` 2 B free, core 1 B, store 3 B, CLI 25 B: reword in place.
