---
class: spec
status: shipped
shipped: 2026-11-03
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, ui]
ref: RM-0050; Phase 7 step 9b (08 s2); owner's decisions 2, 3, 5 and instruction of 2026-11-01; the orchestrator's iteration-2 decisions of 2026-11-02 on the review's REWORK
adrs: [ADR-0061]
---

# hub-confirm: one click confirms one staged choice, a commit in the hub's checkout, nothing pushed (RM-0050)

## Why

`hub-team` (RM-0071) opened accept, clear and stages on a hub; the owner's decisions 2 and 3 go further: a zone lead confirms a brief change with no terminal. Git is not the danger, since apply's steps run unchanged; the danger is a consent nobody gave, or a door an agent reaches. So a login confirms one staged choice, as a commit in the hub's own checkout, with no always-yes consent and no `approve` call in the daemon. ADR-0061 narrows ADR-0035 and ADR-0038 on a hub alone. Working answers, the owner's review open.

## Description and interactions

Canon: `docs/canon/hub-confirm.md`; who: `docs/canon/hub-team.md` "Acts". `GET`, `POST .../proposals/:id/confirm`, a hub's alone: the read returns the question a terminal would ask, the POST sends it back, and that text, byte for byte, is the library's only consent. A stage, never a choice; one proposal per click. An approve ends at apply step 10, nothing pushed. The dialog: `hub-confirm.md` "Screen".

## Data

`ConfirmRead {id, question, updated_at}`, `ConfirmBody {question, updated_at}`: `hub-confirm.md` "Routes". `Decided-by`, author and committer `<login> <email>`; `Author.login` from a hub's departing accept alone. **Bumps:** the plugin MINOR, 0.29.0, and a `SCHEMA_PINS` entry: `get_proposal`'s schema gains `login`. No queue, index, `spec.lock` or event bump.

## Rules and edge cases

`hub-confirm.md` "Order", "Rules": the cover, terminal only, escaping, `door.rs`. The `[team]` entry read again inside the project's turn is the one judged.

## Acceptance criteria

- [x] B1 — `mira` (lead) and `ann` (owner) confirm two proposals in a checkout whose `user.name` is `Owner`: two commits, `Decided-by: mira <...>`, `ann <...>`; queue, author and committer equal.
- [x] B2 — A confirm adds one commit, one path, `Proposal:`, status clean, a bare `origin`'s refs unchanged; a rejected one adds none, and its cards still move.
- [x] B3 — A lead outside its zone, an unzoned path, a discrepancy over two zones, an amendment of a node outside the zone, a staged `canon` outside it: 403, `owner` 200. A gone amended node: `owner` passes the cover and apply step 4 refuses, 409. A staged `canon` naming nothing: `owner` 200, the record written with it, the question showing `ref-dangling`. A `member`, a login outside `[team]`: 403, nothing stored.
- [x] B4 — A `question` other than the library's, a target moved after the read: 409, nothing changed. An `update` staged with a null `span_hash`: 409 naming the terminal; a file-form `create` and a question still confirm.
- [x] B5 — A staged reject confirmed rejects and never applies; nothing staged, or a stage replaced after the read: 409.
- [x] B6 — `editorial: true`, drift, task-bound, an `approved` row: 409 naming the terminal, `spec.lock` byte for byte.
- [x] B7 — `door.rs` loses no line, its tests green; `approve`, `approve_with`, `reject` caught in every `src` file, `confirm_question`, `confirm_staged` outside http `src/confirm.rs`; a `|_|` closure in either `confirm.rs` and a `use ... as` in http's caught, with positive controls.
- [x] B8 — Local `GET`, `POST .../proposals/PR-0001/confirm`: the unknown-route 404, with `Remote-User` too.
- [x] B9 — `tools/list` names no approve, reject, accept, clear or confirm tool; `crates/specengine-mcp/Cargo.toml` no HTTP crate; `Remote-User: agent` 403 on all four.
- [x] B10 — Three round answers stage three, apply none; each takes its own POST; the read adds no event on a failed place. `ConfirmDialog.test.tsx`: `question` verbatim, the login, focus on Cancel, Cancel keeps the stage, status 0 reads `No answer: read again to see whether it was confirmed.`, a null `diff` keeps Confirm off, no control over a set.
- [x] B11 — A departing accept on a hub reads `login=tomas` in `Proposed-by`; local trailers unchanged. `the_step_up_regex_is_the_confirm_class` reads the regex from `docs/canon/hub.md` and the writes from `app.rs`: the four, no other.
- [x] B12 — A staged note and a record holding U+200B, U+2060, U+FEFF, U+00AD: the question and the dialog show them escaped, the POST confirms, the terminal's question keeps them raw. A new-file path holding LF or U+0007: refused at `spec propose create` and MCP, nothing stored.
- [x] B13 — A lead demoted, or moved off the target, while its POST waits for the turn: 403, nothing changed.

## Implementation

Past the draft, in canon: a new `hub-confirm.md`, `hub-team.md` having no room; the cover adds amended nodes and a staged `canon`, a gone amended node meeting apply's own 409; the null-span refusal binds kinds recording a span; the terminal's escaping unchanged, `record::is_escaped` being a refusal too; a plugin bump. Deferred: `Author.login` unchecked on decode (RM-0064). Mutations 53 of 54, then 20 of 20; the survivor, an unlocked run, is held by step 7's compare-and-set.

### Files in scope

| Module | What it does |
|---|---|
| core `proposal.rs`, `team_toml.rs`, `card.rs` | `Author.login`; `TeamRole::confirms`; `is_format_character` |
| CLI `confirm.rs` (new) | `confirm_question`, `confirm_staged`, `Confirm{Body,Read,Outcome,Cause}` |
| CLI `apply.rs`, `stage.rs`, `create.rs`, `card.rs` | `HubRun`; `records_span`; `checked_file`; `CardAcceptRequest.login` |
| http `confirm.rs` (new), `hub.rs`, `app.rs`, `answer.rs` | the routes; `confirms`, `confirming`; the hub-only mount; statuses |
| MCP `mirror.rs`, plugin | `ProposalAuthor`; 0.29.0 |
| UI `inbox/ConfirmDialog` (new), `StagedChoice`, `round/`, `api/`, `ui/text.ts` | the dialog, Confirm per item, `shownConfirmText` |
| Tests | `hub_confirm.rs`, `door.rs`, `create_path_controls.rs`, `ConfirmDialog.test.tsx` |
