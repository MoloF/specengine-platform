---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-http, crates/specengine-mcp, ui]
ref: RM-0050; Phase 7 step 9b (08 s2); owner's decisions 2, 3, 5 and instruction of 2026-11-01
---

# hub-confirm: one click confirms one staged choice, a commit in the hub's checkout, nothing pushed (RM-0050)

## Why

`docs/features/hub-team.md` (RM-0071) opens accept, clear and stages on a hub. The owner's decisions 2 and 3 go further: a zone lead confirms a brief change with no terminal. Git is not the danger, since apply's steps run unchanged; the danger is a consent nobody gave, or a door an agent reaches. So a login confirms one staged choice, as a commit in the hub's own checkout, with no always-yes consent and no `approve` call in the daemon.

Contradicts ADR-0035 ("the only confirmation") and ADR-0038 ("the UI only stages") on a hub, by decision 2. Nobody leans on it until steps 10 and 11.

Working answers, the owner's review open: approve commits on the hub's checkout before `hub-git`; a confirm takes only a stage, two clicks per item.

## Description and interactions

`...` is `/api/projects/:p`.

| Act | Route | Who | Writes |
|---|---|---|---|
| Approve, reject | POST `.../proposals/:id/confirm`, new, hub only | a `lead` whose zones cover every target; `owner` | approve: one commit; reject: queue |

- **Cover.** Every path named lies in a held zone: `target_path` for an applying kind; every `intake.target_ids` holder, resolved now, for a deciding kind (`target_path` is the first target alone, `queue.rs:648`). A gone target or a path in no zone: `owner` only.
- **Terminal only, on a hub too:** `--editorial` and drift (`#apply`: only a terminal `[y/N]` writes `spec.lock`), a task-bound proposal, completing an `approved` row, `spec approve --open`, `spec round answer`, `spec task approve`, `spec import-state`.
- **Approve applies in step 9.** An apply is a write and a commit in the recorded worktree, with no remote in it (ADR-0004, 0005, 0032); `Decided-by` exists only in a commit; step 10 needs a caller.
- **A confirm takes a stage, never a choice.** The choice stays `StageBody`'s, the one decoder (`decision-staging.md` "Daemon"), and is taken as `spec approve PR` with no flags takes it. `confirm_staged` dispatches on the stage, because a plain approve on a row with a reject staged applies (`proposal-apply.md` "Known limits").
- **9 ends** at apply step 10: `applied`, `applied_commit` set, `git status` clean, on the recorded branch of the hub's checkout, no fetch, merge or push. **10 begins** at the first network verb.
- **Between them,** `applied` means committed on this hub; the operator may push by hand. A new file leaves the index stale as on a terminal (N12); `Proposal:` numbers collide only once pushed.
- **Consent.** New library calls `confirm_question` and `confirm_staged`. The first returns the text `run_approve` or `run_reject` would ask. The second runs them with a closure answering `asked == question`, byte for byte, taking no `Consent` from the daemon. A rebase, a new raise, a replaced stage or another record ID is a 409. The text is public, so it is no token (ADR-0034). The rehearsal bypasses `failed()` (`apply.rs:1208`), which logs `proposal.apply_failed`, or a read writes an event.
- **One click per item.** The route names one proposal; no route takes a set; a round stages, then each item takes its own click. The dialog shows the diff or record, `question` verbatim (staged line, option line, stale note, raise) and the login; focus starts on Cancel, which keeps the stage. A one-factor `fetch` meets a redirect, status 0: the dialog says nothing was confirmed.
- **A person or a script: the route cannot tell.** A script holding a person's session passes, as a faked terminal does.

## Data

**`GET .../proposals/:id/confirm`,** a hub only, judged as the POST is, no event (`GET_ROUTES` 25; `ConfirmRead`, set 109 of `daemon-keys.json`):

```json
{"id":"PR-0007","question":"staged 2026-11-02T09:00:00Z: spec approve PR-0007\napply PR-0007 to docs/spec/maps/hold.md on main in <worktree> (applies)? [y/N]","updated_at":"2026-11-02T09:00:00Z"}
```

**`POST .../proposals/:id/confirm`,** the confirm class's fourth route. Body `{"question":"<the read's>","updated_at":"<the read's>"}`, closed keys, so a `choice` is a 400. Nothing staged, or a stage, target or question moved after the read: 409. 200 the review document; 403 the lines of `hub-team.md`; 404 unknown; 503 cannot run. An exit 1 is a 404 by default (`answer.rs`), so a `ConfirmCause` map: `Unknown` 404, `Refused` 409, `Outside` 403.

**Identity.** `confirm_staged` sets `GIT_COMMITTER_*` and `GIT_AUTHOR_*` by `GitEnv::with_var` (`git.rs:184`) to the login and its `email`; `WorktreeGit` drops only `LOCAL_ENV_VARS` (`worktree.rs:56`), so `apply.rs:675,1933`, `decide.rs:369`, `create.rs:1386` give `mira <mira@example.org>` with no step changed. Queue `decided_by`, trailer, author and committer hold that string. The name part is the login; `name` is display. The terminal keeps its identity, on the host too.

**`Author`** (`proposal.rs:139`) gains an optional `login`, skipped when none, set only by a departing accept on a hub (`card.rs:2306`). `provenance()` appends ` login=<login>` last, only when set, so stored rows, dumps and local trailers stay byte for byte, and `daemon-keys.json` keeps `Author` at four keys. `ProposalAuthor` (`mirror.rs:676`) and `ui/src/api/types.ts` gain the field.

**Bumps.** None, as in `hub-team.md`.

## Rules and edge cases

- **`door.rs`.** Existing tests stay as written: `forbidden()` keeps `approve`, `approve_with`, `reject` for every `src` file, hub code included. Added: `HUB_CONFIRM_CALLS`, caught in any file but `src/confirm.rs`, a file-aware `caught` with positive controls, no `|_|` closure in `confirm.rs`.
- **Local.** WHEN `app.hub` is `None`, the route SHALL not be mounted: the unknown-route 404, router and fence as they were.
- **No agent path.** No MCP tool is added; `specengine-mcp` names no HTTP crate; `agent` and `local` are refused logins (`hub_login_problem`). RM-0053 inherits a duty: a `/mcp` credential yields the same `Remote-User`, so only the proxy can keep it off every other path.
- **Step-up.** The four routes, in one regex: `^/api/projects/[^/]+/(proposals/[^/]+/confirm|cards/[^/]+/(accept|clear)|trace/clear)$`. The GET read shares the path, so a one-factor session learns at dialog open. A proxy step-up is a session level (from memory, unverified), not this click a person's; a per-click passkey is WebAuthn inside, forbidden by ADR-0034.
- **Risks.** A forged `Remote-User` now means a commit as the owner: `hub.md#trust` items 1-4 guard spec files; no real use before RM-0064's "Done". Agent text renders where Confirm lives, so CSP `default-src 'self'` (`ui.rs:57`) and no raw HTML in `ui/src` stay pinned. A driven browser clicks: only the log and the commit name who. A stage can carry `editorial: true` (`Staging::for_approve`), so a route forgetting it writes the lock.
- **Assumption.** No `T-` flow runs on a hub before phase two.

## Acceptance criteria

The `M:` names stay in this draft until ship. Compression (§6) may drop them and nothing is lost: each test keeps its `M:` in its doc comment.

- [ ] B1 — `mira` (lead) and `ann` (owner) confirm two proposals, the checkout's `user.name` being `Owner`: two commits, `Decided-by: mira <...>`, `ann <...>`, queue, author and committer equal. M: a default committer.
- [ ] B2 — A confirm adds one commit, one path, `Proposal:`, status clean, a bare `origin`'s refs unchanged; a rejected one adds none, and its cards still move. M: a push; a reject touching a card.
- [ ] B3 — A lead outside its zone, an unzoned path, a discrepancy spanning two zones: 403 for the lead, 200 for `owner`. A `member`, a login outside `[team]`: 403, nothing stored. M: judging `target_path` alone.
- [ ] B4 — A `question` other than the library's, or a target moved after the read: 409, nothing changed. M: an always-yes closure.
- [ ] B5 — A staged reject confirmed rejects and never applies; nothing staged, or a stage replaced after the read: 409. M: always calling approve.
- [ ] B6 — `editorial: true`, drift, task-bound, an `approved` row: 409 naming the terminal, `spec.lock` byte for byte. M: dropping the refusal.
- [ ] B7 — `git diff` of `door.rs` removes no line; its tests green; `approve`, `approve_with`, `reject` caught in every `src` file; `confirm_staged` caught outside `confirm.rs`. M: `app.rs` calling it; `confirm.rs` calling `approve_with` with `|_| true`.
- [ ] B8 — Local `GET`, `POST .../proposals/PR-0001/confirm`: the unknown-route 404, with `Remote-User` too. M: mounting it unconditionally.
- [ ] B9 — `tools/list` names no approve, reject, accept, clear or confirm tool, `crates/specengine-mcp/Cargo.toml` no HTTP crate, `Remote-User: agent` is 403 on all four. M: a confirm tool.
- [ ] B10 — Three round answers stage three, apply none; each takes its own POST. The confirm read adds no event on a failed place. `ConfirmDialog.test.tsx`: `question` verbatim, the login, focus on Cancel, Cancel keeps the stage, status 0 reads nothing confirmed, no control over a set. M: `round_answers` confirming; rehearsing through `failed()`; focus on Confirm.
- [ ] B11 — A departing accept on a hub reads `login=tomas` in `Proposed-by`; local trailers unchanged. `the_step_up_regex_is_the_confirm_class`, a unit in `src/hub.rs`, reads the regex from `docs/canon/hub.md` and the non-GET routes from `app.rs`, as `get_routes_in` reads the GET ones: it matches the four and no other write. M: renaming a route alone.

## Out of scope

Push, merge, sole writer, `apply-outputs`, `PR-` ranges: RM-0051. `/mcp`: RM-0053. The real step-up: RM-0064. Baselines, any confirm over a set: RM-0056.

## Implementation

To fill in after implementation.

### Files in scope

Core `proposal.rs`. CLI new `confirm.rs`, `apply.rs`, `decide.rs`, `create.rs` (the quiet rehearsal), `card.rs`, `lib.rs`. HTTP `hub.rs`, `app.rs`, new `confirm.rs`, `answer.rs`; its `Cargo.toml` gains `[dev-dependencies] regex.workspace = true` for B11, already in the graph (root `Cargo.toml:41`), one `Cargo.lock` line. MCP `src/mirror.rs`. UI `inbox/StagedChoice`, new `inbox/ConfirmDialog`, `round/RoundItemCard`, `api/` (`client`, `http`, `queries`, `provisional`, `types`), `mocks/MockClient`, `test/stubClient`. Tests: `door.rs`, new `hub_confirm.rs`, `daemon_keys.rs`, `src/hub.rs`.

### Canon at ship

Nothing here is written before this slice's commit; 9a's ADR and `hub-team.md` stand by then.

```yaml
id: ADR-0061
title: On a hub a login [team] entitles confirms one staged choice
canon: docs/canon/architecture.md#apply
ref: hub-confirm analysis 2026-11-01; working answers, the owner's review open
```

- **The ADR:** no `supersedes:` key: ADR-0035 and ADR-0038 stay accepted for a local daemon, the CLI and MCP, and that key would retire them whole; its text narrows the two clauses in "Why" on a hub alone. ADR-0054 is untouched. Cost: a script holding a session passes; a forged header commits as anyone.
- **`CLAUDE.md`, exact,** replacing the rule on `apply_proposal`:

  ```text
  - Spec files are written only by `apply_proposal` once a person confirms, where the proposal was raised, as a separate commit with provenance: on a terminal, or on a hub alone under a login `[team]` entitles, one click per item (ADR-0004, ADR-0005, ADR-0032, ADR-0035, ADR-0061).
  ```

- **`architecture.md`** (6 B free): `#apply` takes the same clause, `#ui` its "confirmed on a terminal"; each cut by what it adds.
- **`hub.md`:** the fifth item of "RM-0064 must guarantee", with the regex in its own fenced block, which B11 reads, in the bytes 9a freed. **`hub-team.md`:** a "Confirm" section. Also `decision-staging.md` (32 B free), `proposal-apply.md` "Consent", `07-interfaces.md:139`.
- **Tier 1:** http `README.md` 2 B free, core 1 B, CLI 25 B: reword in place.
