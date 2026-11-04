---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, ui]
owner: owner
reviewed: 2026-11-04
---

# Hub confirm: one click, one staged choice, one commit

ADR-0061, narrowing ADR-0035 and ADR-0038 on a hub alone, built by `hub-confirm` (RM-0050). On a hub (`hub.md`) a login `[team]` entitles (`hub-team.md`) confirms one staged choice with no terminal: an approve is one commit in the hub's own checkout, a reject the queue alone; a `--push` sync carries the branch (`hub-git.md`). Working answers, the owner's review open. Code: CLI `confirm.rs`, `apply.rs` `HubRun`, `stage.rs` `records_span`, `create.rs` `checked_file`; http `confirm.rs`, `hub.rs` `confirms`, `confirming`, `answer.rs`; core `team_toml.rs` `TeamRole::confirms`, `card.rs` `is_format_character`, `proposal.rs` `Author`; UI `src/inbox/ConfirmDialog.tsx`, `src/ui/text.ts`.

## Routes

`...` is `/api/projects/:p`. `GET` and `POST .../proposals/:id/confirm` are mounted on a hub alone: a local daemon answers its unknown-route 404, with `Remote-User` too, router and fence as they were. The read takes no query and logs nothing (`GET_ROUTES` 25; `ConfirmRead`, set 109 of `fixtures/daemon-keys.json`):

```json
{"id":"PR-0007","question":"staged 2026-11-02T09:00:00Z: spec approve PR-0007\napply PR-0007 to docs/spec/maps/hold.md on main in <worktree> (applies, staged)? [y/N]","updated_at":"2026-11-02T09:00:00Z"}
```

`question` is what `spec approve PR` or `spec reject PR` would ask, its stage line first (`decision-staging.md` "Terminal"): it ends `(applies, staged)`, `(new file, staged)` or `(applies; raises X 1 -> 2, staged)`; a record's shows the whole record and names the decider, so it differs per login.

The POST, the confirm class's fourth route (`hub-team.md` "Classes"), sends back what the read gave: `ConfirmBody {question, updated_at}`, closed keys, so a `choice` is a 400; at most 4 MiB, since a record's question is the record.

| Status | Body |
|---|---|
| 200 | the read; the POST's review document |
| 404 | `ConfirmCause::Unknown`: no such proposal, a bad ID |
| 409 | `Refused`: the review document, the reason its last note |
| 403 | `Outside`, and the fence's, `member`'s, `confirms`' lines: the error body (`hub-team.md` "Answers") |
| 400, 413, 415, 503 | the error body; 503 a bad `[team]` or a run that cannot run |

## Order

Past the fence, nothing read or stored on a refusal: the slug; for the POST a same-origin page; the ID; `member` and `confirms`; for the POST JSON, the cap, the closed body, then `member` and `confirms` again. Inside the project's turn the `[team]` entry is read once more (`hub::confirming`; a refusal `ConfirmOutcome::outside`, 403), and that read is the one judged, so a login demoted or moved while its POST waits is judged by its new role and zones. Then one library call: the repository, the status (applied, rejected; an `approved` row a terminal's), a drift item, a task-bound proposal, nothing staged, an editorial stage, an approve staged with no `span_hash` where the kind carries one, the cover (403), the `updated_at` read, the question. A POST refused at apply's steps 2-6 logs `proposal.apply_failed` as a terminal's run does, a failed place being exit 2, a 503; a read logs nothing.

## Rules

- **Cover.** Every path named lies in a held zone (`TeamEntry::covers`): `target_path` for a kind that applies, an `amendment` adding the holder of every node its `amends` links name; for one that decides every target's holder (its `target_path` is the first target's alone) and a staged `canon` REF's holder; each holder resolved now. A gone target, amended node or `canon`, or a path in no zone: `owner` only. The owner passes the cover and the run decides: a gone amended node is apply step 4's 409; a dangling `canon` is written into the record as named, the question showing `ref-dangling`. A `member` never confirms.
- **A stage, never a choice.** The choice stays what `StageBody` stored, taken as `spec approve PR` with no flags takes it; the stage picks the call, since a plain approve on a row with a reject staged applies (`proposal-apply.md` "Known limits").
- **Consent.** `confirm_question` rehearses the run, answering no, with `failed()` quiet (`HubRun.rehearsal`). `confirm_staged` runs it again; its one consent answers `asked == question`, byte for byte, and it takes none from its caller. A rebase, a new raise, a replaced stage or another record ID makes another text: a 409, nothing changed. The run must read the row judged (`HubRun.judged`), so a stage raced on the one-second clock is compared by value. The text is public, so it is no token (ADR-0034).
- **Escaping.** The question and the dialog's shown text escape every format character (Cf: U+00AD, U+200B, U+2060, U+FEFF, the rest) as `\u{XX}`, as controls are; the compare takes the escaped form. The terminal's `escape_controls` is unchanged: it runs on core `record::is_escaped`, which records, stages, cards and rounds refuse, and U+200C, U+200D carry Persian, Indic and emoji text. A create's new-file path holding a control character is refused at propose (`proposal-kinds.md` "Target").
- **Terminal only, on a hub too:** an editorial stage and a drift item (only a terminal `[y/N]` writes `spec.lock`), a task-bound proposal, completing an `approved` row, an approve staged with no `span_hash` on a kind whose stage carries one (stored while its preview failed: with none, a span change after the read would leave the question unchanged); no route runs `spec approve --open`, `spec round answer`, `spec task approve` or `spec import-state`.
- **One click per item.** The route names one proposal and no route takes a set: a round stages, then each item takes its own click.
- **Identity.** `confirm_staged` sets `GIT_COMMITTER_*` and `GIT_AUTHOR_*` to the login and its `[team]` `email`, the read too, so every step of apply reads `mira <mira@example.org>` from git with no step changed: queue `decided_by`, `Decided-by`, author and committer hold it. The name part is the login; `name` is display. A terminal keeps its identity, on the host too.
- **`Author.login`.** Optional, its key skipped when none, set by a hub: a departing accept, or a proposal over `/mcp` (`hub-mcp.md`); `Proposed-by` then ends ` login=<login>`. Checked on decode by core `hub_login_problem`, so a dumped login off that grammar is a corrupt row naming `author`. Stored rows, dumps and local trailers stay byte for byte; MCP's `ProposalAuthor` is an untagged enum of two closed shapes.
- **Where it ends.** At apply step 10: `applied`, `applied_commit` set, `git status` clean, on the recorded branch of the hub's checkout. `applied` means committed on this hub: a `--push` sync pushes it, never awaited by a confirm nor refusing one (`hub-git.md`); without `--push` the operator pushes by hand. The commit carries the index's changed outputs as a terminal's does, a gap's warning appended to the POST's `notes` (`proposal-outputs.md`); a hub's `PR-` numbers start at its floor (`queue-backup.md` "ID floor").
- **No agent path.** No MCP tool, over stdio or a hub's `/mcp` (`hub-mcp.md`); `specengine-mcp` names no HTTP crate; `agent` and `local` are refused logins. `tests/door.rs` keeps its tests: `approve`, `approve_with`, `reject` are caught in every `src` file, `HUB_CONFIRM_CALLS` (`confirm_question`, `confirm_staged`) in any but `src/confirm.rs`, which holds no `|_|` closure and no `use ... as` re-export; the always-yes scan covers the CLI's `confirm.rs` too, where the consent closures live.

## Known limits

- **A person or a script:** the route cannot tell; a script holding a person's session passes, as a faked terminal does. A driven browser clicks: only the log and the commit name who.
- **The step-up** (`hub.md` "Trust", item 5) is a session's level, from memory, unverified: a second factor on the session, not this click a person's. A per-click passkey is WebAuthn inside, forbidden by ADR-0034. The read shares the path, so a one-factor session learns at dialog open.
- **A deciding kind's record** lands under `[decision_records] dir`, which the cover does not judge.
- **A forged `Remote-User`** is a commit as anyone, and raises proposals as anyone (`hub-mcp.md`): `hub.md` "Trust" items 1-4 guard spec files; no real use before the gate, step 11b-3. Agent text renders beside Confirm, so CSP `default-src 'self'` and no raw HTML in `ui/src` stay pinned.
- **Step 10's blob compare** (`proposal-outputs.md` "Steps") warns on the daemon's stderr only, never in the POST's answer; 10b's UI line may surface it.
- **No `T-` flow** runs on a hub before phase two: task-bound proposals stay a terminal's.
- **A failed post-commit update**, or a confirm refused after it may have written, marks the project's index dirty: the next read walks (`spec-read-freshness.md` "A hub's trusted index").

## Screen

Confirm stands beside Unstage on a staged proposal (`inbox/StagedChoice`) and on a round's staged item (`round/RoundItemCard`) only where `allowed` lists `confirm`; the terminal's command and Copy stay beside it, and a local daemon's page has no Confirm. It opens `inbox/ConfirmDialog` on that one item, which reads the question when it opens and shows the diff or record, the question and the login as text, never markup (`ui/text.ts` `shownConfirmText`: the CLI's escaping plus every Cf, the ranges of `is_format_character`), with what the click does in the page's words: `Confirming applies this staged choice as one commit in this hub's checkout, under your login; this click pushes nothing.` Focus starts on Cancel, which keeps the stage. The POST carries the read's `question` and `updated_at` alone. Confirm is on only while the document shown is the version read; an applying kind with no diff shows the review's notes with Confirm off. One call at a time; any refusal keeps the dialog in the daemon's words and Confirm off until Read again; status 0 reads `No answer: read again to see whether it was confirmed.`, since it may follow a commit. No control reaches a second item. A 200 reads `Confirmed <id>: <state> as commit <sha>, in this hub's checkout; this click pushes nothing.` or, with no commit, `Confirmed <id>: <state>; no commit.`; when its document holds notes (a gap's warning, a reject's follow-up), the dialog stays open: `Confirmed.`, the notes as text, Confirm gone, focus on Close, which hands the document on.

## Tests

http `tests/hub_confirm.rs`, `tests/door.rs`, `tests/daemon_keys.rs`; `src/hub.rs` `the_step_up_regex_is_the_confirm_class` reads the rule from `hub.md` and the writes from `app.rs`: it matches the four and no other. CLI `tests/create_path_controls.rs`. UI `src/inbox/ConfirmDialog.test.tsx`, `confirm.live.test.tsx`, `src/api/confirm.http.test.ts`, `src/mocks/confirm.test.ts`, `src/ui/text.test.ts`.
