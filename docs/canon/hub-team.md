---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# Hub team: who acts past the plain writes

ADR-0060, ADR-0061, built by `hub-team` (RM-0071) and `hub-confirm` (RM-0050). On a hub (`hub.md`) a project's `[team]` opens accept, clear, group clear, stages, round answers and a confirm to the logins it names; a terminal on the host still confirms. `/mcp` proposes under a login (`hub-mcp.md`); no real hub exists before step 11b-3. Working answers, the owner's review open. Code: core `team_toml.rs`, `gate.rs`; store `check.rs`, `queue/stage.rs`; CLI `team.rs`, `card.rs` `run_clear_group`, `trace.rs` `group_marks`; http `hub.rs`, `app.rs`; UI `src/board/acting.ts`, `ActOff.tsx`.

<a id="team"></a>
## Team

```toml
[zones]                     # the gate's table, read here whether or not [gate] is on
brief = ["docs/spec/**"]
maps  = ["docs/spec/maps/**"]
[team.mira]                 # the key is the login, byte for byte; quoted when not bare
name  = "Mira K"            # optional, shown only
role  = "lead"              # owner | lead | member
zones = ["maps"]            # a lead's, required
email = "mira@example.org"  # required
```

Read apart: core `TeamConfig::from_toml`, `None` with no `[team]`; `ProjectConfig` only name-checks the table, and older binaries refuse the file. Its `[zones]` comes through `zones_from_toml` (`gate.md` "Config"), only when a `[team]` exists, so a bad `[zones]` under an existing `[team]` is a bad `[team]`. Every error names its line. A project with no `[team]` opts in by a reviewed commit. `spec team` prints `login role email` lines read-only for `hub-admin check`.

- **Entries**: closed keys, at most 256 (`TEAM_ENTRIES_MAX`).
- **A login key** passes `hub_login_problem` (`hub.md` "Identity"), holds no `<` or `>`, and neither opens nor ends in a character git trims (`.,:;"'\`): any of these would change a `Decided-by`.
- **`name`**: optional, a card person's grammar (`cards.md` "Who acts"), trimmed, shown and never compared.
- **`zones`**: a `lead`'s, required and not empty, each a `[zones]` key, none named twice. On `owner` or `member` the key is refused, `zones = []` too.
- **`email`**: required, since `<login>@<public host>` is unjudgeable by `spec check`. Printable ASCII with no space, no `<` or `>`, one `@` with both sides non-empty, both ends under the login's rule. A forge's no-reply address is fine.

Read per request, no cache in `App`, through `answer::read`, before the turn, from the root the route's slug names, so two roots hold two teams and a commit acts on the next request.

## Acts

`...` is `/api/projects/:p`.

| Act | Route on a hub | Who | Class |
|---|---|---|---|
| Seven plain writes | POST `.../cards`, `.../cards/split`, `.../cards/:id/{move,edit,assign,cancel,items}` | any login | plain |
| Stage, unstage | POST, DELETE `.../proposals/:id/decision` | any `[team]` login | team |
| Round answers | POST `.../round/answers` | any `[team]` login | team |
| Accept | POST `.../cards/:id/accept` | a `[team]` login, the card's reviewer, never its assignee | confirm |
| Clear | POST `.../cards/:id/clear` | any `[team]` login | confirm |
| Group clear | POST `.../trace/clear` | a `lead` whose zones cover every node it re-freezes; `owner` | confirm |
| Confirm a stage | GET, POST `.../proposals/:id/confirm` | a `lead` whose zones cover every target; `owner` | confirm |

`member`: all above but group clear and confirm. `lead`: those two inside its zones. `owner`: all, a path in no zone too.

**Only a group clear and a confirm judge a cover.** A path in no zone is `owner` only there, a confirm's gone target too (`hub-confirm.md` "Rules"). A group clear cannot reach a gone target, which stands on no node of the group, while a single clear by any `[team]` login can re-freeze one (`spec-trace.md` "Clearing"), whatever its zones: a zone bounds no single card.

## Classes

Default deny, in `Hub::fenced`, by exact raw segments and exact methods (`class`). The handlers do the rest, in order, nothing read or stored on a refusal.

- **Plain**: `hub::acting` binds the body's `as` to the login.
- **Team**: `hub::member`, the login's entry.
- **Confirm**: in the fence no query, a bare `?` too, and a required `Origin`; in the handler `member`, `acting`, then `needs`, a required `revision` or `count`. A group clear adds `clears_groups` after each `member`; a confirm, with no `as`, `confirms`.

`member` runs before a body is read, so an outsider costs no body, and again once it is in, in `stage`, `round_answers`, `accept_card`, `clear_card`, `clear_group`, `confirm::write`, so a login taken out of `[team]` during a slow body keeps no power; `unstage` reads no body. `acting` runs before `.request()`, since `CardAcceptBody` defaults to `local`. Accept's and clear's always-yes consents are reached past these judges alone: the dialog is the consent, as locally (ADR-0057). A confirm has none.

A local daemon has no `Login`: `member` and `needs` read nothing, `[team]` changes nothing for a typed `as` or `spec card accept --as`, and `GET .../team` is the unknown-route 404.

The confirm class is the acts a person confirms; a stage is none. RM-0064 adds the proxy's step-up on its four routes (`hub.md` "Trust"), which the daemon cannot see: a marker header is a second one to scrub (ADR-0059). The fence gives the proxy one byte form: it passes the path alone, so without the refusal `.../accept?x=1` would reach the handler while a `$`-anchored rule missed it. `Origin` is byte for byte, with `Sec-Fetch-Site: same-origin`, JSON, no CORS.

## Answers

`GET .../team`, a hub's alone, no turn, no query, the asker's own row (`GET_ROUTES` 24; `Team`, set 108 of `fixtures/daemon-keys.json`):

```json
{"login":"mira","name":"Mira K","allowed":["stage","accept","clear","group_clear","confirm"]}
```

`member` the first three, `lead` and `owner` all five. Outside `[team]` or no `[team]`: 200, `name` null, `allowed` empty.

| When | Answer, nothing stored |
|---|---|
| Bad `[team]`, `GET .../team` too | 503 `specengine.toml:<line>: <message>` |
| No `[team]` | 403 ``refused: this project has no `[team]` `` |
| Outside `[team]` | 403 ``refused: <login> is not in this project's `[team]`: reads and plain writes`` |
| A `member`'s group clear or confirm | 403 ``refused: <login> is a `member`: this act needs a `lead` `` |
| Outside a zone | 403 ``refused: <login> leads <zones>; <path> lies outside them`` |
| A confirm's gone target | 403 ``refused: <login> leads <zones>; <why>: this act needs an `owner` `` |
| Confirm class: a query, no `Origin` | 403 ``refused: this route takes no query``, ``refused: this route needs the Origin header`` |
| Null `revision` or `count` | 400 ``a hub needs `revision` ``, `count` alike |
| A count that differs | 409 `` `<group>` holds <n> suspect cards, not <m>; nothing changed `` |
| Off the list | 403 ``refused: this hub does not serve this route`` |

## Group clear

Which cards a group takes is `spec-trace.md` "Clearing", so `count`, the cover and the dialog use one set. `run_clear_group` judges with it known, before its consent: none taken, the cover, the count. `count` joins `CardClearGroupBody`: the page sends it on a hub alone, a hub requires it, a local daemon compares it when sent, and `spec card clear --group` has none.

The cover: `CardClearGroupRequest.entry`, the daemon's last `member`, must cover every path holding a target the clear re-freezes, the first outside named by text. `Zone::covers` is the gate's glob test; `owner` covers all. `CardCause::Outside` is the 403 in `write_status`. No new `card_` name.

## Stages

`StageRequest`, `UnstageRequest`, `RoundAnswersRequest` take `actor`: on a hub the login, a stage body's `as` naming another a 403; locally a stage body's `as` (`card-questions.md` "Stage"). Store `stage_as`, `unstage_as` add it to `proposal.staged`, `.unstaged` as a trailing `as`, never a column, so an event with none stays byte for byte; `stage_from`, `unstage_from` call them with `None`.

The stage's compare runs on a one-second clock (`decision-staging.md` "Queue"): many logins can race a stage, last wins, each event names its `as`. A confirm takes the staged value into its compare.

## Confirm

`.../proposals/:id/confirm`, a hub's alone: one staged choice per click, never a choice sent; an approve is a commit in the hub's checkout, pushed by a `--push` sync (`hub-git.md`). Who: "Acts"; the rest: `hub-confirm.md`.

## Check

`team-config`, an error at the config's line, subject `team`, the 37th of 38 `CHECK_CODES` (`card-config`, `[card]`'s twin, the last), carried by `CheckSetup.team` and added by every store run under the baseline. Never `cannot-check`, which would stop every run on a typo. `--staged` and `--changed` judge it on the checked side alone, so a bad `[team]` at `HEAD` counts as introduced until fixed. The gate never reads it (ADR-0039).

## Rules

- **ADR-0012 holds.** `[team]` is read only by the acts above past the plain writes; no read, move, `by`, status, flow, gate or check of a discrepancy reads it, so a bad one leaves reads, plain writes and `spec card` as they are. With no lead free, items stay open and a terminal may still confirm: a role is no second control point. A terminal is never judged, a shell on the host being the operator's.
- **ADR-0008, `by`.** Role words name authority over the queue, no trade; the UI branches on `allowed`. `by` stays literal, else a bad `[team]` stops a plain write, so a project lists logins.
- **A suspect mark** stays as built, no consent added: a suspect card moves and is accepted, an accepted card refuses `edit` and `cancel`, so `confirmed` is never reached that way, and an open card's retarget or cancel reads `no card`, recorded under the login. A retarget keeps its entry, so a returning target marks again (`spec-trace-record.md` "Snapshot"). Refusing them would be a discrepancy blocking work (ADR-0012).

## Screen

The login replaces the typed name in "Accepting as" and "Clearing as": read-only, `.field-fixed` as the header's "Acting as", with `[team]`'s `name` beside it when read, shown only, never sent. `NAME_IS_TYPED` gives way to `This is recorded under your login.` ADR-0057 stands locally.

Controls follow `GET .../team`, the route's 403 staying the truth: `useTeam` in `queries.ts`, `useTeamRow`, `useAllowed` in `acting.ts`. It is asked only once a login is known, so a local daemon never is; window focus reads it again, since `[team]` changes by a commit, which raises no event. Unknown reads as on and the route answers: no login, the read still out, a failed read.

"Off" means the button stays, focusable, `aria-disabled`, doing nothing, with one line beside it (`ActOff`, `labels.ts` `ACT_OFF`): Accept in `board/CardPanel`, Clear there and in `trace/CardsBlock`, Clear all there, Stage, Unstage and the decisions that stage in `inbox/ProposalCard` and `round/RoundScreen`. The inbox's keys announce the same line (`inbox/InboxView`). A round's item stays to read, its choices off, no answer written (`round/RoundItemCard`). The two decisions that send nothing stay on. The Move menu drops its Accept line: a menu has no line to keep.

## Known limits

- **`assign` is a plain write.** An assignee can drop themself or reassign and name themself reviewer, each step recorded under a login.
- **Free text.** `by` and assignee are compared exactly with a login, so a display name in `assignee` does not trip "never its assignee".
- **A removed login's open browser** keeps reading what it has: any login reads (`hub.md` "Routes"), and `allowed` is read again on focus alone.
- **A forged `Remote-User` now accepts and confirms**: `hub.md` "Trust".
- **The step-up is session-level** (RM-0064): a second factor on the session, not a person behind each click: `hub-confirm.md`.
- **A refused group document** shows only its last note on the page.

## Tests

http `tests/hub_team.rs`, `tests/hub_confirm.rs`, `tests/common/hub.rs`, `tests/hub.rs` (the raw table), `src/hub.rs` `a_write_is_refused_until_listed`, `daemon_keys.rs`; core `tests/team_toml.rs`; CLI `tests/team_config.rs`, `card_trace.rs`; UI `src/board/hubIdentity.test.tsx`, `src/api/`'s.
