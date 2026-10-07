---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-08
---

# Decision staging: UI prepares, terminal confirms

ADR-0035 under ADR-0034 (`docs/canon/architecture.md#apply`, `#distribution`), built by `decision-staging` (2026-10-08). Code: store `queue/stage.rs`, CLI `stage.rs`, http `app.rs`, UI `ownStages.ts`.

## Rule

- A decision on a proposal, approve or reject, is confirmed only by `spec approve|reject PR` on a terminal: its `[y/N]` is **the only confirmation of a decision**. The only control point stays the owner's task approval (ADR-0012, `#control`).
- Anything else only **stages**: the UI's decision dialog now, later an MCP form or URL mode onto a UI card (08 Phase 5). A stage is one replaceable choice per `open` proposal, the same flags as the CLI's, kept only in the queue: no file, no git, no apply step. It is an attribute, not a status: a staged proposal is `open`, listed, and blocks nothing; a task's `task_id`, snapshot and package are untouched.
- Agents never stage: no MCP tool writes a stage (ADR-0004); `get_proposal` shows one, and the plugin's `ask-owner` counts it as no answer.
- Who staged is unknowable (ADR-0034): the commit's provenance stays the terminal's (ADR-0005); the event log keeps every stage.

## The stage

`staged`, compact JSON, keys in this order, one of:

```json
{"decision":"approve","option":1,"answer":null,"canon":null,"note":"keep the cap","span_hash":null}
{"decision":"reject","reason":"duplicate of PR-0003"}
```

An approve's absent flags are `null`. `span_hash`: an `update`'s or section-form `create`'s target span hash as apply step 5 reads it (steps 2-6 run read-only, as `spec review`); `null` for a file-form `create`, a deciding kind, and when steps 2-4 cannot read the target or the proposal's own commit is on its branch. `staged_at`: UTC `YYYY-MM-DDTHH:MM:SSZ`.

Checked as the terminal checks its flags, before anything is stored: the ID and repository (`proposal-queue.md` "Place, IDs, repositories"; another existing repository refused; an orphan takes only a reject, and may be unstaged); state `open` (an applied one's refusal naming its commit); then the `updated_at` the caller read (``` `PR-0004` changed since it was read: updated at <t1>, the choice was made on the one of <t2>; read it again; nothing staged ```); the decision flags (`decide.rs` first checks, `decision-record.md` "Flags": ``--option 5: `PR-0003` has options 0-1; nothing changed``; a decision flag on an `update` or `create`; a question whose only target is a path, no `--canon`: a usage refusal); a blank reason; the caps, `note`, `reason` 4 096 bytes (``--note: <n> bytes; at most 4096; nothing changed``; `spec approve --note` and `spec reject --reason` refuse alike, exit 1), `answer` 2 048, `canon` 512; a character the queue escapes (U+202E, U+0007), named.

## Queue

Queue schema 5 (step 5 under `version == 4`, one transaction: `ADD COLUMN staged TEXT`, then `staged_at TEXT`): both NULL when nothing is staged; `PROPOSAL_COLUMNS` 43, `proposal_columns(4)` the first 41. Schema 4 opens as 5; 6 -> exit 2 ``schema version 6, this build knows 5``. A migrated queue refuses older binaries (`SchemaTooNew`): restart the daemon, reinstall `specengine-mcp` from one commit.

`stage_from(id, seen, stage, now)` sets both (`staged_at`, `updated_at` = `now`) on `open` only, one `Immediate` transaction with `proposal.staged` and a compare-and-set on `Seen {status, updated_at, staged}`; else `Changed`, `Status`, `Invalid` (a stage the kind cannot hold), nothing written. `unstage_from(id, seen, now)` NULLs both with `proposal.unstaged`; nothing staged -> `Ok(false)`, no write, no event. Every op leaving `open` (approve, reject, apply, a completion) NULLs both in its transaction, no `.unstaged`. `approve_from`, `approve_record_from`, `reject_from`, `reject_orphan` compare `staged` too. `Decision {decided_by, note, staged_at}`: `staged_at` the stage it confirms, else `None`; a compare-and-set op refuses one the row does not hold (`Invalid`); `approve`, `reject`, `applied_with` compare nothing (the CLI compares before a completion: "Terminal").

Events `proposal.staged` `{"id":"PR-0004","staged":{...},"staged_at":"..."}`, `proposal.unstaged` `{"id":"PR-0004"}`: seven proposal events, `QUEUE_EVENT_TYPES` 16 (the two after `.apply_failed`). `.approved`, `.rejected` add `"staged_at"` when they confirm a stage.

**Corrupt row**, named (`get`, `list` fail; `inbox` skips it, a note): another shape (a key missing, extra or reordered, a wrong type; an answer on a discrepancy; `option` or `canon` on a kind taking none; `span_hash` on a deciding kind or a file-form create; a blank reason); one column NULL alone; a stage on a state but `open`.

**Gap** (the compare-and-set key `updated_at` has 1 s resolution): a stage made in the same second as the UI's read replaces another without a 409; its event still raises the alert ("UI"). The terminal compares the stage itself.

## Daemon

`specengine-http` writes this one pair of columns through the CLI library, never the store: `stage(&Env, &Globals, &StageRequest {id, body, now, git})`, `unstage(.., &UnstageRequest {id, now, git})` -> `StageOutcome {proposal, cause: Option<StageCause>}` (`Outcome::Stage`, a review document). `StageBody` (serde, `deny_unknown_fields`, an approve's absent keys `null`) is the daemon's only decoder; `StageCause` `Usage` (a usage line), `Unknown`, `Refused` (the last note).

| On `/api/projects/:p/proposals/:id/decision` | Answer |
|---|---|
| `POST`, the stage less `span_hash`, plus `updated_at` as read | 200 the review document; 400 a body that does not decode (``spec: the stage is not ... as JSON: <why>; nothing changed``) or flag usage; 404 unknown; 409 refused, the refused review document; 503 cannot run |
| `DELETE` | 200 the document, nothing staged too (no event); 404; 409 not `open`, another repository; 503 |

A second POST replaces the stage, one event. Bodies: 404, 409 the review document (exit 1's); 400, 403, 405, 413, 415, 503 the error body `{status, message}`.

Past the fence (`crates/specengine-http/README.md` "Fence"), in order, nothing read or stored on a refusal: the slug (unknown: 404); `Sec-Fetch-Site: same-origin`, else 403 ``staging needs a same-origin page (not authentication: ADR-0034)``; the path's ID decoded (400); for POST, one `Content-Type` of media type `application/json` (any case, any parameter), else 415 ``the body is JSON: send it with Content-Type: application/json``; a body over 16 384 bytes (a declared `Content-Length` over it: unread) -> 413 ``the body is over 16384 bytes``. DELETE reads no body. Any other method: 405 `Allow: POST, DELETE`, ``this path takes only POST (stage a choice) and DELETE (unstage it); a staged choice is confirmed on a terminal``. No `Access-Control-*`.

## Terminal

`spec approve PR` with no decision flag (nor `--note`) and an approve staged confirms it. On stderr, right before the question: ``staged 2026-10-07T17:49:50Z: spec approve PR-0003 --option 1 --note "n"`` and the chosen option's review line (``  [1] label | effect | price``). Values double-quoted, `\` and `"` backslashed; the staged line stays one line: LF, CR, TAB written `\n`, `\r`, `\t`, every other control escaped. The question's parenthesis ends `, staged`: `(applies, staged)`, `(rebases, staged)`, a new file's `(new file, staged)`, a record's mid-line ``apply PR-0003 as DEC-0024 (option 1, staged) on <branch> in <worktree>? [y/N]``; a completion's gains ` (staged)`. `y` applies with the staged flags, checked again as typed ones, `.approved` carrying `staged_at`; anything else -> exit 1, the stage kept.

`spec reject PR`: `--reason` may be omitted only when a reject is staged (`RejectRequest.reason` optional); shown alike (``staged <at>: spec reject PR-0005 --reason "..."``, the question ending `, staged)? [y/N]`), its reason stored in `decision_note`. Nothing staged -> exit 2 ``` `spec reject PR` needs `--reason T`: no reject is staged on it; say why the proposal is rejected; nothing changed ```; an approve staged -> exit 2 ``` `PR` has an approve staged at <at>: `spec approve PR` confirms it; `spec reject PR --reason T` rejects it; nothing changed ```.

Typed flags win whole, never merged: ``note: the choice staged <at> is not used: the typed flags decide`` above the question and among the outcome's notes; a reject staged under `spec approve`, any stage under a typed `--reason`, alike.

A run reads the stage once, before the question (`Seen`). Step 7, a record's and a reject's compare-and-set compare the row's stage byte for byte with it; replaced or removed on a still `open` proposal -> exit 1, nothing changed: ``` `PR-0001` not applied (step 7): PR-0001 changed since the question: its staged choice was replaced or removed; nothing changed ``` (step 7 logs its `proposal.apply_failed`). A completion compares the stage it read, shown or not, before `applied_with`: a stage appeared -> ``PR-0001 changed since the question: a choice was staged on it meanwhile; nothing changed``, changed or removed -> ``...: its staged choice was replaced or removed; ...``. No `--yes`; without a terminal exit 2 before reading (`proposal-apply.md` "Consent").

## Staleness

A staged `span_hash` other than the target's now (step 5) gives ``staged against <h1>; the target is now <h2>: the change applies as it rebases`` at the prompt and in `spec review`'s `notes`, only when the preview applies or rebases; it never refuses or unstages. No expiry by age.

## Documents

The review document: `staged` (the object or `null`), `staged_at` between `task_id` and `notes` (45 keys); text lines `staged:` (compact JSON), `staged_at:`. Inbox entries: `staged_at` after `record_id` (12 keys); text state `open (staged)`. `fixtures/daemon-keys.json` pins them, 34 sets (the two shapes, 6 and 2 keys). MCP `get_proposal` = `review --brief --json`, its output schema's `staged` two closed objects. Backups keep stages (`queue-backup.md` "Format": `queue_schema` 5; format-1 dumps of schemas 1-3 and format-2 of 4-5 restore, NULL where absent; a schema-4 DB exports as 5, unmigrated).

## UI

`stageDecision(project, id, stage, updatedAt)` (POST), `unstageDecision(project, id)` (DELETE), no `AbortSignal`: a write's answer is always taken. The decision dialog stages against the shown version's `updated_at`: Accept an option (`[0]`, `[1]`, as `--option N`) or the working answer, and a note; Reject a reason; no `--answer`, `--canon`; it says when it replaces a staged choice. Clarification and defer send nothing (``Not built yet: only Accept and Reject are staged``). The card's "Staged decision": ``Staged <at>. Confirm on a terminal: spec approve PR-0004`` (a reject: `spec reject PR-0004`) with Copy (fixed words and an ID matching `^PR-[0-9]{4,}$`, else no command), the choice, Unstage; the list marks it "Staged"; it stays in the inbox. A 409 or 404 carrying a review document rejects as `ClientError` with its last note; the dialog stays. A stage event re-reads that project's inbox and that proposal, no task. A stage changed outside this tab raises a `role="alert"` above every screen of the project (Show, Dismiss): this tab's writes are tracked per `QueryClient`, each accounting for one event, for 60 s; an unstage is tracked only when a stage was cached and no outside stage event arrived since. `StagedChoice`, `DecisionDialog` load lazily. The mock stages alike; `?scenario=conflict`: a first stage finds one staged elsewhere (409), the next is taken.

## Threat model

- Any local process of any account can read everything served and stage (loopback is not per-user); the fence stops other sites' pages. `specengine-http` is a queue writer.
- None confirms from an ordinary shell; a process faking a terminal (`script`) can, as with flags today.
- Residual: a reflex `y` on a stage an agent or a page injected. Mitigations: the whole choice and its time on one line before the key, the outside-change alert, the event log, no MCP stage tool, the header rule. The UI offers four choices; two are staged, the CLI confirms them.
- Hosting or a shared machine: infrastructure first (ADR-0034).
