---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: decision-staging analysis 2026-10-06, the owner's decisions as recommended; readiness check 2026-10-07; 08 s2 Phase 2
adrs: [ADR-0034, ADR-0035]
---

# Decision staging

## Why

The UI cannot decide: the decision POST answers 403, the card offers a command to copy (`crates/specengine-http/README.md` "One door"). The owner decided (2026-10-06): no authentication inside SpecEngine, ever (ADR-0034); a decision is **staged anywhere, confirmed only on a terminal** (ADR-0035). The behaviour is canon, marked not built: `docs/canon/decision-staging.md` (read it first). This slice builds it: one optional staged choice per open proposal, written by `specengine-http`, shown and confirmed by the existing `spec approve|reject` `[y/N]`.

**Order** (08 s2): after `task-package`, `ui-live-tasks` (shipped). Roles: `rust-developer` store, CLI library, `specengine-http`, `mirror.rs`, `plugin/`; `ui-developer` `ui/src`; `test-engineer` tests, `fixtures/daemon-keys.json`, `PINS`. No hook, CI, crate or package added.

**Assumptions**: A1 "one key" is `y` + Enter, the existing prompt; A2-A4 are canon now; A5 `daemon-read` stays: this slice supersedes its AC-03 (the POST's 403). **Working answers** Q1-Q5 (readiness check 2026-10-07, as recommended, open to the owner): marked in place.

## Description and interactions

1. The owner stages in the UI's dialog; `spec approve|reject PR` on a terminal confirms (the canon's "UI", "Terminal").
2. Agents read `staged`, `staged_at` (`get_proposal`, `spec review --json`); no tool stages. Plugin `ask-owner` "Whose words count": a staged choice is not an answer (any local process writes one, ADR-0034); 0.1.5 -> 0.1.6 (root README "Claude Code plugin"; Q3).
3. Task-bound proposals: a stage never touches `task_id`, a snapshot or a package; the refresh stays the terminal apply's.

**Docs at shipping**: new normative text only in `docs/canon/decision-staging.md` ("Not built yet" goes). Elsewhere byte-neutral only (4 -> 5, 41 -> 43, 43 -> 45, 11 -> 12, five -> seven, "not yet" -> the fact; no op rows): `tasks.md` "Store", "Backup", "Task-bound proposals"; `proposal-kinds.md` "Queue row"; `proposal-queue.md` "States and events", "Store" (schema, "Connection", `Seen`); `queue-backup.md` "Format", "Store"; `architecture.md#apply`, `#ui`; the http README title, intro, "Fence", "Endpoints", "One door" (drop "Until staging ships" to pay), "Live tail", "Concurrency", "Tests"; `ui/README.md` "Contract seam"; root README version; 07 s3.

## Data

**Queue schema 5**: `QUEUE_SCHEMA_VERSION` 5; `migrate` (`queue.rs`) adds `STEP_5` under `if version == 4`, same transaction: `ALTER TABLE proposals ADD COLUMN staged TEXT`, then `staged_at TEXT`. `PROPOSAL_COLUMNS` 43 = `PRAGMA table_info`; `proposal_columns(4)` the first 41. Schema 4 opens as 5, both NULL; 6 -> exit 2. A schema-5 DB refuses older builds (`SchemaTooNew`): restart the daemon, reinstall `specengine-mcp` from one commit.

`staged`: compact JSON, the canon's two shapes ("The stage"), keys in its order, `null` when absent. `span_hash`: an `update`'s or section-form `create`'s target span hash (step 5's reading); `null` for a file-form `create`, a deciding kind. `staged_at` by `utc_now`. **Corrupt row**, named (`get`, `list` fail; `inbox` skips, noted): another shape (a key missing, extra, reordered; a wrong type); one column NULL alone; staged, not `open`; `option` or `canon` on a kind taking none.

**Store ops** (`ProposalQueue`; `Seen` + `staged`); any op leaving `open` NULLs both:

| Op | Does |
|---|---|
| `stage_from(id, seen, stage, now)` | `open`, compare-and-set on `Seen`: sets both, `updated_at` = `now`, `proposal.staged`; else `Status`/`Changed`, no write |
| `unstage_from(id, seen, now)` | alike, NULLs both, `proposal.unstaged`; nothing staged -> `Ok(false)`, no write, no event |
| `approve_from`, `approve_record_from`, `reject_from`, `reject_orphan` | compare `staged` too |
| `approve`, `reject`, `applied_with` | no compare (`applied_with` by design: `queue.rs` "Runs") |

**Events**: `proposal.staged` `{"id":"PR-0004","staged":{...},"staged_at":"..."}`, `proposal.unstaged` `{"id":"PR-0004"}`; `.approved` (step 7 or a completion) and `.rejected` add `"staged_at"` when confirming a stage. `QUEUE_EVENT_TYPES` 16, the two after `proposal.apply_failed`.

**CLI library** (no `spec stage`): `stage(&Env, &Globals, &StageRequest {id, body: StageBody, now, git})`, `unstage(&Env, &Globals, &UnstageRequest {id, now, git})` -> `Result<StageOutcome, CliError>` (`Outcome::Stage`). `StageBody`: serde, `deny_unknown_fields`, approve keys absent = `null`; the daemon decodes with it, no literal of its own (`no_domain.rs`). Checks: the canon's ("The stage"), `updated_at` equal to the row's after `open`, flags by `first_checks` (`decide.rs`). `StageOutcome {proposal: ProposalOutcome, cause: Option<StageCause>}`, `StageCause` `Usage` (an exit-2 usage line) | `Unknown` | `Refused` (the last note). No name in `specengine-http` is `Staged`, `Changed`, `baseline` (`door.rs` `not_plain`).

**Daemon**: `answer.rs` `answered` keys on `Outcome::Stage` as on the check exception: no cause 200, `Usage` 400, `Unknown` 404, `Refused` 409 (the refused review document, Q2), `CliError` 503; error bodies the README's.

| Request | Answer |
|---|---|
| `POST .../proposals/:id/decision`, body `StageBody`: the stage less `span_hash`, plus `updated_at` | 200 the review document; 400 bad JSON, a key unknown or missing, flag usage (a question with only a path target: no `--canon`); 404; 409 not `open`, an orphan's approve, another repository, `updated_at` changed; 413 over 16 384 bytes; 415 not `application/json`; 503 |
| `DELETE`, same path | 200 the document, nothing staged too (no event); 404; 409 not `open`; 503 |

Past the fence, these two only: `Sec-Fetch-Site: same-origin` (absent, `none` -> 403 ``staging needs a same-origin page (not authentication: ADR-0034)``); else 405 `Allow: POST, DELETE` (`app.rs` `only_post`, `methods.rs`). `pnpm dev` as is (its proxy keeps `Sec-Fetch-Site`: `vite.config.ts`). One door: `door.rs` `forbidden` kept, handlers add `stage`, `unstage`; only its first test's decision-POST block is rewritten.

**Terminal** (`main.rs`, `apply.rs`): `spec approve PR`, no decision flag, an approve staged: stderr ``staged 2026-10-06T09:14:02Z: spec approve PR-0004 --option 1 --note "keep the cap"`` (values double-quoted, `\` `"` backslashed, then escaped), the option's review line (`  [1] label | effect | price`), the question: apply's parenthesis ends `, staged`, the completion's (none now: `proposal-apply.md` "Consent") is ` (staged)`; a completion of an `open` proposal records the staged flags, the stage compared again before `applied_with` (Q4). A typed flag (`--note` too) wins whole: ``the choice staged <at> is not used: the typed flags decide``; a reject staged under `spec approve`: the same. `--reason` optional (`Reject`, `RejectRequest`): a reject staged -> its reason; nothing staged -> exit 2 naming `--reason`; an approve staged -> exit 2 naming `spec approve PR`. Declined -> exit 1, stage kept.

**Staleness** note: ``staged against <h1>; the target is now <h2>: the change applies as it rebases``, at the prompt and in `spec review`'s `notes`.

**Documents**: review + `staged` (object or `null`), `staged_at` between `task_id` and `notes` (45 keys); text `staged:` compact JSON, `staged_at:`. Inbox entries + `staged_at` after `record_id` (12); text `open (staged)`. `daemon-keys.json` 34 sets (+ the two shapes, 6 and 2 keys). `mirror.rs`: `staged` two closed objects, `staged_at` after `task_id`; `decision` a field identifier, never a literal (`mcp_genre.rs`).

**Backup**: `STATE_FORMAT` 2, `queue_schema` 5. Import (`state_file.rs`): format 1 holds schemas 1-3, format 2 4-5 (schema 4: stages NULL). `stored_rows` (`queue/state.rs`) reads tasks, runs at `version >= 4`, not `==`; schema 4 exports as 5, unmigrated.

**UI**: `stageDecision(project, id, stage, updatedAt)`, `unstageDecision(project, id)` replace `decideProposal` (`SpecEngineClient`, `HttpClient`, mock, `stubClient.ts`), no `AbortSignal` (a write's answer always taken). `http.ts` `request` takes `DELETE`; a POST/DELETE 409 carrying the review document rejects as `ClientError {409, its last note}`. `provisional.ts` `Stage`, `Proposal.staged`, `staged_at`, `InboxEntry.staged_at`; dialog fields from the review document; accept, reject stage; clarification, defer send nothing ("Not built yet"); `decisions.ts` `WORDING` accept, reject `effect` reworded (staged, confirmed on a terminal). Copy: fixed words and an ID matching `^PR-[0-9]{4,}$`. A stage (replacing `useDecideProposal`) keeps the entry, sets the returned document, re-reads only that inbox and proposal; `useLiveQueue` alike on `proposal.staged`, `.unstaged`; a stage changed outside this tab: `role="alert"`.

## Rules and edge cases

The canon's ("Rule", "Queue", "Terminal"), and:

- WHEN the stage shown at the prompt differs byte for byte from the row at step 7, a completion or reject (`updated_at` has 1 s resolution) THEN exit 1 ``PR-0004 changed since the question: its staged choice was replaced or removed; nothing changed``.
- Gap (Q1: the stage's compare-and-set key stays `updated_at`): a stage made in the same second as the UI's read replaces it without a 409; its event still raises the alert.
- Risks past the canon's "Threat model": `specengine-http` a queue writer; the UI's four choices against the CLI's two.

## Acceptance criteria

- [ ] AC-01 -- docs: ADR-0034, ADR-0035 each <= 1 536 B with `canon:` and "Cost"; `rg 'specengine/[t]oken|first write [e]ndpoint'` finds nothing in live docs, READMEs, `crates/*/src`, `ui/src`; every touched canon within cap (M: 07 s3's token line restored).
- [ ] AC-02 -- a schema-4 DB opens as 5, both NULL; `PROPOSAL_COLUMNS` 43 = `PRAGMA table_info`; schema 6 exit 2 (M: the columns without the version bump).
- [ ] AC-03 -- staging an open `update`, `question`, `discrepancy` -> 200 with `staged`; `git status --porcelain` empty, `HEAD`, branches, status unchanged; one `proposal.staged` each, seen by a subscriber < 1 s; `door.rs`, `no_domain.rs` green (M: the handler approves with an always-yes consent).
- [ ] AC-04 -- each refusal of `decision-record.md` "Flags", a decision flag on an update or a create, via POST: the CLI's message, 400 or 409, nothing stored; a path-only question's approve 400 (M: the daemon's own option-range check).
- [ ] AC-05 -- `approved`, `applied`, `rejected` -> 409; an orphan's approve 409, its reject stored; unknown ID 404; a stale `updated_at` 409 with the current document (M: an applied row staged).
- [ ] AC-06 -- a second POST replaces the stage, its event the new one; DELETE NULLs both, one `proposal.unstaged`; DELETE, nothing staged: 200, no event (M: no unstage event).
- [ ] AC-07 -- pty, `{option: 1, note: "n"}` staged: `spec approve PR` prints `staged <at>:`, the command, the option line, the question ending `, staged)? [y/N]`; `y` -> record option 1, note `n`, stage NULL, `.approved` with `staged_at`; `n` -> exit 1, stage kept; no TTY -> exit 2 unread (M: the stage not read).
- [ ] AC-08 -- option 1 staged, typed `--option 2` -> the unused-stage note, option 2, empty note (M: the staged note merged).
- [ ] AC-09 -- reject staged: `spec reject PR` stores its reason in `decision_note`; nothing staged exit 2 naming `--reason`; approve staged exit 2 naming `spec approve` (M: an empty reason stored).
- [ ] AC-10 -- the stage replaced between prompt and `y` in the same second, at step 7 or a completion -> exit 1, no commit (M: compare-and-set on status and `updated_at` only).
- [ ] AC-11 -- staged at hash H, the target changed after -> the note at the prompt and in review, applied as it rebases; changed only before -> no note; a file-form create's `span_hash` null (M: the note from `preview != applies`).
- [ ] AC-12 -- leaving `open` clears the stage, no `.unstaged`; staged on another state: a named corrupt row; export, import, export byte-identical with stages; format-1 (schemas 1-3), format-2 schema-4 dumps restore, stages NULL; format 1 at schema 4 refused; a schema-4 DB exports its tasks (M: export drops `staged`; `stored_rows` keeps `==`).
- [ ] AC-13 -- `http.ts` (16 types), the http README, `proposal-queue.md` list the same 7 proposal events; a stage event re-reads that inbox and proposal, no task read (M: `http.ts` without `proposal.staged`; tasks re-read).
- [ ] AC-14 -- UI: accept, option 2, a note -> one POST, its body exactly the stage plus `updated_at`; the card: Staged, the command, Copy (no free text), Unstage (DELETE); still listed; a 409 -> `ClientError` 409, the last note; clarification, defer send nothing; the mock alike; 34 key sets (M: the proposal dropped once staged).
- [ ] AC-15 -- `tools/list` has no stage tool; `get_proposal` = `review --brief --json` with both keys; `ask-owner` names a stage no answer, 0.1.6 in `PINS`; a POST with a foreign `Origin`, `text/plain`, an oversize body or no `Sec-Fetch-Site` -> refused, nothing stored, no `Access-Control-*` (M: the content-type check removed).
- [ ] AC-16 -- an `open` proposal, approve staged, completed by its own commit: the question ends ` (staged)? [y/N]`; `y` -> the staged flags recorded, `.approved` with `staged_at` (M: `applied_with` given no flags).

## Out of scope

Task staging; `changes_requested`, `deferred`; `--answer`, `--canon` from the UI (Q5); MCP elicitation, URL mode (08 Phase 5); a raw keypress; `spec stage`; a Unix socket; age-based cleanup; any authentication (ADR-0034).

**Owner's text for `.claude/agents/ui-developer.md`** (lines 32-33): "the owner's choice, staged, is the UI's only writing action; before it a diff, after it the staged choice and its `spec` command, confirmed on a terminal (ADR-0035);".

## Implementation

Filled in after implementation.
