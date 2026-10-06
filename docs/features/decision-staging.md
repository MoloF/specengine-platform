---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: decision-staging analysis 2026-10-06; the owner's decisions of 2026-10-06, every recommendation accepted; 08 s2 Phase 2
adrs: [ADR-0034, ADR-0035]
---

# Decision staging

## Why

The owner reads a proposal best in the browser, but the UI cannot decide: the decision POST answers 403 and the card offers a command to copy (`crates/specengine-http/README.md` "One door"). The owner decided (2026-10-06): no authentication inside SpecEngine, ever (ADR-0034); a decision is **staged anywhere, confirmed only on a terminal** (ADR-0035). The behaviour is canon already, marked not built: `docs/canon/decision-staging.md` (read it first; this spec does not repeat it). This slice builds it: one optional staged choice per open proposal, written by `specengine-http`, shown and confirmed by the existing `spec approve|reject` `[y/N]`.

**Order**: `proposal-kinds`, `task-package` (queue schema 4, 41 proposal columns, review document 43 keys), this, `ui-live`. Roles: `rust-developer` store, CLI library, `specengine-http`, MCP `mirror.rs`; `ui-developer` `ui/src`; `test-engineer` tests, `fixtures/daemon-keys.json` (regenerated). No hook, CI or plugin change; no new crate or package.

**Owner's questions, decided 2026-10-06 as recommended**: Q1 after `task-package`, schema 5; Q2 typed flags win whole; Q3 agents never stage (no MCP tool); Q4 staging requires `Sec-Fetch-Site: same-origin`, labelled "not authentication"; Q5 staleness is a note; Q6 stages travel in backups.

**Assumptions**: A1 "one key" is `y` + Enter, the existing prompt; A2 `note` and `reason` are capped at 4 096 bytes for staging and the CLI alike (a new exit 1 on the CLI); A3 staged text holding a character the queue escapes is refused, named; A4 who staged is unknowable: the commit format is unchanged, the event log keeps every stage; A5 `daemon-read` is not rewritten: this slice supersedes its AC-03 (the POST's 403) and `door.rs` changes with it.

## Description and interactions

1. The owner opens a card (`GET .../proposals/:id`), chooses in the decision dialog: the POST stages, the card reads ``Staged <at>. Confirm on a terminal: `spec approve PR-0004` `` with Copy and Unstage; the proposal stays in the inbox.
2. On a terminal `spec approve PR-0004` (no flags) prints the stage and its time, then the question; `y` applies exactly as `spec approve PR-0004 --option 1 --note "..."` would today.
3. An agent sees `staged`, `staged_at` in `get_proposal` and `spec review --json`; no tool stages (Q3). Neighbours: `proposal-apply.md` "Consent", `decision-record.md` "Flags", `queue-backup.md` "Format", `agent-intake.md` "Review document", `ui/README.md` "Contract seam".

## Data

**Queue schema 5** (`QUEUE_SCHEMA_VERSION` 4 -> 5, one `Immediate` transaction): `ALTER TABLE proposals ADD COLUMN staged TEXT`, then `staged_at TEXT`; `PROPOSAL_COLUMNS` 41 -> 43, pinned to `PRAGMA table_info`. A schema-4 DB opens as 5 with both NULL; a schema-6 DB -> exit 2, nothing changed. Columns, not a table: one stage per proposal.

`staged`, compact JSON, keys in this order, `null` for an absent value:

```json
{"decision":"approve","option":1,"answer":null,"canon":null,"note":"keep the cap","span_hash":null}
{"decision":"approve","option":null,"answer":null,"canon":null,"note":null,"span_hash":"b3:9f2c..."}
{"decision":"reject","reason":"duplicate of PR-0003"}
```

`span_hash`: an `update`'s or `create`'s target span hash when staged (apply step 5's reading), `null` for a deciding kind. `staged_at` `YYYY-MM-DDTHH:MM:SSZ` (CLI `utc_now`). **Corrupt row**, named (`get`, `list` fail; `inbox` skips with its note): `staged` not one of the two shapes (a key missing, extra or reordered; a wrong type); one of the pair NULL without the other; a stage on a row not `open`; `option` or `canon` on a kind that takes none (`decision-record.md` "Flags").

**Store ops** (`ProposalQueue`; `Seen` gains `staged`, the text as read):

| Op | Does |
|---|---|
| `stage_from(id, seen, stage, now)` | `open` only; compare-and-set on `Seen`; sets both, `updated_at` = `now`; `proposal.staged`; else `Status`/`Changed`, nothing written |
| `unstage_from(id, seen, now)` | `open` only; both NULL, `updated_at` = `now`, `proposal.unstaged`; nothing staged -> `Ok(false)`, no write, no event |
| `approve_from`, `reject_from`, `applied_with`, `reject_orphan` | compare `staged` too; leaving `open` NULLs both in the same transaction |

**Events**: `proposal.staged` `{"id":"PR-0004","staged":{...},"staged_at":"..."}` (`staged` an object); `proposal.unstaged` `{"id":"PR-0004"}`; `.approved`, `.rejected` add `"staged_at"` when they confirm a stage. Proposal events 5 -> 7, listed alike in `proposal-queue.md` "States and events", the http README "Live tail" and `ui/src/api/http.ts` `QUEUE_EVENT_TYPES`.

**CLI library** (no `spec stage` command): `stage(&Env, &Globals, &StageRequest {id, stage, updated_at, now, git}) -> ProposalOutcome`, `unstage(&Env, &Globals, &UnstageRequest {id, now, git})`. In order: the ID (`proposal-queue.md` "Place, IDs, repositories"), the current repository, the orphan rule (only a reject stages), state `open`, `updated_at` equal to the row's, `first_checks` (`decide.rs`) on the staged flags, the caps (`note`, `reason` 4 096; `answer` 2 048; `canon` 512), escaped characters refused, named. No apply step, no git write, nothing under any root. Each refusal names its cause for the daemon's status.

**Daemon** (`specengine-http`; error body as the README's, two keys):

| Request | Answer |
|---|---|
| `POST .../proposals/:id/decision`, body `{"decision":"approve","option":1,"answer":null,"canon":null,"note":"...","updated_at":"..."}` or `{"decision":"reject","reason":"...","updated_at":"..."}` | 200 the review document; 400 bad JSON, an unknown or missing key, flag usage (the CLI's message); 404 unknown ID; 409 the refused document (its reason the last note: not `open`, an orphan's approve, another repository, `updated_at` changed); 413 body over 16 384 bytes; 415 not `application/json`; 503 cannot run |
| `DELETE .../proposals/:id/decision` | 200 the review document, also when nothing was staged (no event); 404; 409 not `open`; 503 |

Past the fence, for these two only: `Sec-Fetch-Site: same-origin` required (absent or `none` -> 403 ``staging needs a same-origin page (not authentication: ADR-0034)``); other methods -> 405 `Allow: POST, DELETE`. Approve keys may be absent (= `null`). The one door changes: handlers call `stage`, `unstage`, never `approve`, `reject`, `propose`, `import_state`, `export_*`, `init`, `index`, `check`.

**Terminal** (`main.rs`, `apply.rs`): `spec approve PR` with no decision flag and an approve staged takes its flags: stderr ``staged 2026-10-06T09:14:02Z: spec approve PR-0004 --option 1 --note "keep the cap"`` (values double-quoted, `\` and `"` backslashed, then escaped), the chosen option's review line (`  [1] label | effect | price`), then the question, its parenthesis ending `, staged`. Any typed flag (`--note` too) wins whole: note ``the choice staged <at> is not used: the typed flags decide``. `spec approve` with a reject staged runs as unstaged, the same note. `spec reject PR` without `--reason`: a reject staged -> its reason; nothing staged -> exit 2 naming `--reason`; an approve staged -> exit 2 naming `spec approve PR`. Declined -> exit 1, the stage kept.

**Staleness**: staged `span_hash` other than step 5's: note ``staged against <h1>; the target is now <h2>: the change applies as it rebases`` at the prompt and in `spec review`'s `notes`. Never refuses or unstages; no expiry.

**Documents and keys**: review document + `staged` (object or `null`), `staged_at` after `task_id`: 45 keys; text `staged:` compact JSON, `staged_at:`. Inbox entries + `staged_at` (12 keys); text status `open (staged)`. `fixtures/daemon-keys.json` regenerated; `mirror.rs` mirrors both.

**Backup**: the two columns travel in dumps (`queue_schema` 5); import takes schemas 1-5 (later columns NULL); a schema-4 DB exports as 5, unmigrated.

**UI**: `SpecEngineClient.stageDecision(project, id, stage, updatedAt)`, `unstageDecision(project, id)` replace `decideProposal` (`HttpClient`, the mock, `stubClient.ts`); `provisional.ts` `Stage`, `Proposal.staged`, `staged_at`, `InboxEntry.staged_at`; the dialog's fields come from the review document; accept -> approve stage, reject -> reject stage; clarification and defer send nothing ("Not built yet"). The card's Copy copies only fixed words and an ID matching `^PR-[0-9]{4,}$`. A `proposal.staged` or `.unstaged` refetches that project's inbox and that proposal; a stage changed outside this tab raises a `role="alert"`.

## Rules and edge cases

- WHEN a stage arrives for a proposal not `open` THEN the system SHALL refuse it (409) and store nothing.
- WHEN the stage shown at the prompt differs byte for byte from the row at step 7 or at reject (`updated_at` has 1 s resolution) THEN exit 1 ``PR-0004 changed since the question: its staged choice was replaced or removed; nothing changed``.
- WHEN a proposal leaves `open` THEN its stage SHALL be cleared in that transaction, without `proposal.unstaged`.
- A stage never blocks (ADR-0012): no status, no hold; a staged proposal is listed, decided by anyone's terminal.
- No `--yes`, no TTY -> exit 2 before anything is read (`proposal-apply.md` "Consent").
- Risks: a reflex `y` on an agent-made or injected stage (mitigated: the whole choice and time first, the alert, the event log, no MCP tool, the header rule; residual: forged headers plus an unread prompt); a faked terminal and other local accounts (ADR-0034, stated); `specengine-http` becomes a queue writer (README "One door", `proposal-queue.md` "Store"); the UI's four choices against the CLI's two.

## Acceptance criteria

- [ ] AC-01 -- docs: ADR-0034, ADR-0035 each <= 1 536 B with `canon:` and "Cost"; `rg 'specengine/[t]oken|first write [e]ndpoint'` finds nothing in live docs, READMEs, `crates/*/src`, `ui/src`; `docs/canon/decision-staging.md` <= 12 288 B; the five queue canons within cap (M: 07 s3's token line restored).
- [ ] AC-02 -- schema: a schema-4 DB opens as 5, both NULL; `PROPOSAL_COLUMNS` 43 = `PRAGMA table_info`; a schema-6 DB exit 2 (M: the columns added without the version bump).
- [ ] AC-03 -- staging an open `update`, `question`, `discrepancy` -> 200 with `staged`; `git status --porcelain` empty, `HEAD`, branches, status unchanged; one `proposal.staged` each, a subscriber sees it < 1 s; `door.rs` green (M: the handler approves with an always-yes consent).
- [ ] AC-04 -- every refusal of `decision-record.md` "Flags" and a decision flag on an update, via POST: the CLI's message, 400 or 409, nothing stored (M: the daemon's own option-range check).
- [ ] AC-05 -- `approved`, `applied`, `rejected` -> 409; an orphan's approve 409, its reject stored; unknown ID 404; a stale `updated_at` 409 with the current document (M: an applied row staged).
- [ ] AC-06 -- a second POST replaces the stage, its event carries the new one; DELETE NULLs both, one `proposal.unstaged`; DELETE with nothing staged 200, no event (M: no unstage event).
- [ ] AC-07 -- on a pty, `{option: 1, note: "n"}` staged: `spec approve PR` prints `staged <at>:`, the command, the option line, then the question; `y` -> record option 1, note `n`, stage NULL, `.approved` has `staged_at`; `n` -> exit 1, stage kept; no TTY -> exit 2 before reading (M: the stage not read).
- [ ] AC-08 -- option 1 staged, typed `--option 2` -> the unused-stage note, record option 2, empty note slot (M: the staged note merged).
- [ ] AC-09 -- reject staged: `spec reject PR` stores its reason in `decision_note`; nothing staged exit 2 naming `--reason`; approve staged exit 2 naming `spec approve` (M: an empty reason stored).
- [ ] AC-10 -- the stage replaced between the prompt and `y` within the same second -> exit 1, no commit (M: compare-and-set on status and `updated_at` only).
- [ ] AC-11 -- staged at hash H, the target changed after -> the note at the prompt and in review, applied as it rebases; changed only before staging -> no note (M: the note from `preview != applies`).
- [ ] AC-12 -- leaving `open` clears the stage without `.unstaged`; a stage on another state is a named corrupt row; export, import, export byte-identical with stages; format-1 and format-2 schema-4 dumps restore with NULL stages (M: export drops `staged`).
- [ ] AC-13 -- `http.ts`, the http README, `proposal-queue.md` list the same 7 proposal event types; the UI refetches that project's inbox and that proposal on each (M: `http.ts` without `proposal.staged`).
- [ ] AC-14 -- UI: accept with option 2 and a note -> one POST whose body is exactly the stage plus `updated_at`; the card shows Staged, the command, Copy (no free text), Unstage (DELETE); still listed; clarification and defer send nothing; the mock alike; daemon keys regenerated (M: the proposal dropped from the inbox once staged).
- [ ] AC-15 -- `tools/list` holds no stage tool; `get_proposal` = `review --brief --json` with both keys; a POST with a foreign `Origin`, `text/plain`, an oversize body or no `Sec-Fetch-Site` -> refused, nothing stored, no `Access-Control-*` (M: the content-type check removed).

## Out of scope

Task staging (`approve_task`, `spec task approve`); `changes_requested`, `deferred`; MCP elicitation, URL mode (08 Phase 5); a single raw keypress; a `spec stage` command; a Unix socket; age-based cleanup; authentication of any kind (ADR-0034).

**Owner's text for `.claude/agents/ui-developer.md`** (lines 32-33): "the owner's choice, staged, is the UI's only writing action; before it a diff, after it the staged choice and its `spec` command, confirmed on a terminal (ADR-0035);".

## Implementation

Filled in after implementation.
