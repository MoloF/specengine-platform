---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-06
---

# Decision staging: UI prepares, terminal confirms

The decided behaviour of ADR-0035 under ADR-0034 (`docs/canon/architecture.md#apply`, `#distribution`). **Not built yet: the `decision-staging` slice** (`docs/features/decision-staging.md`, after `task-package`). Until it ships the decision POST answers 403 (`crates/specengine-http/README.md` "One door"), the UI offers the `spec` command to copy, and a decision is `spec approve|reject` with its flags alone (`proposal-apply.md` "Consent", `decision-record.md` "Flags").

## Rule

- A decision on a proposal, approve or reject, is confirmed only by `spec approve|reject PR` on a terminal: its `[y/N]` is **the only confirmation of a decision**. The only control point stays the owner's task approval (ADR-0012, `#control`).
- Anything else only **stages**: the UI's decision dialog now, later an MCP form or URL mode onto a UI card (08 Phase 5). A stage is one replaceable choice per `open` proposal, the same flags as the CLI's, kept only in the queue: no file, no git, no apply step. It is an attribute, not a status: a staged proposal is `open`, listed, and blocks nothing.
- Agents never stage: no MCP tool writes a stage (ADR-0004); `get_proposal` shows one.
- Who staged is unknowable (ADR-0034): the commit's provenance stays the terminal's (ADR-0005); the event log keeps every stage.

## The stage

`staged`, JSON, one of:

```json
{"decision":"approve","option":1,"answer":null,"canon":null,"note":"keep the cap","span_hash":null}
{"decision":"reject","reason":"duplicate of PR-0003"}
```

An approve carries `option`, `answer`, `canon`, `note` (`null` when absent) and, for an `update` or `create`, `span_hash`: the target's span hash when staged (the staleness note). `staged_at` is the UTC time, `YYYY-MM-DDTHH:MM:SSZ`. A stage is checked as the terminal checks its flags, before anything is stored: the ID and repository (`proposal-queue.md` "Place, IDs, repositories"; an orphan takes only a reject), state `open`, the decision flags (`decision-record.md` "Flags"; a decision flag on an `update` refused), the caps (`note`, `reason` <= 4 096 bytes, as the CLI from then on; `answer` 2 048, `canon` 512), a character the queue escapes refused, named. No apply step runs; nothing is written in a worktree.

## Queue

Queue schema 5 (after `task-package`'s 4): `proposals` + `staged`, `staged_at`, both NULL when nothing is staged; `PROPOSAL_COLUMNS` 43. Store ops `stage_from`, `unstage_from`, on `open` only, one `Immediate` transaction with its event and a compare-and-set on `updated_at`. Leaving `open` (approve, reject, apply, a completion) clears both in the same transaction; a stage on any other state is a corrupt row, named. Events: `proposal.staged` (payload `{id, staged, staged_at}`), `proposal.unstaged` (`{id}`): the proposal events go from five to seven. `.approved` and `.rejected` gain `staged_at` when they confirm a stage.

## Daemon

`specengine-http` becomes a queue writer for this one pair of columns, through the CLI library (never the store):

- `POST /api/projects/:p/proposals/:id/decision`, body JSON: the stage's fields without `span_hash`, plus `updated_at` as the UI read it. 200 the review document; 400 a bad body or flag usage (the CLI's message); 404 unknown; 409 refused (not `open`, an approve on an orphan, `updated_at` changed: with the current document); 503 cannot run. A second POST replaces the stage.
- `DELETE` on the same path unstages: 200; nothing staged -> 200, no event.
- Past the fence: `Content-Type: application/json` only, `Sec-Fetch-Site: same-origin` required (labelled "not authentication"), a body cap; no `Access-Control-*`.

## Terminal

- `spec approve PR` without decision flags, an approve staged: before the question, on stderr, `staged <staged_at>: spec approve PR-0004 --option 1 --note "..."` (escaped) and the chosen option's line; the question is marked `staged`. `y` applies with the staged flags, checked again; anything else -> exit 1, the stage kept.
- Typed flags win whole, never merged with the stage; a note names the stage left unused. `spec reject PR`: `--reason` may be omitted only when a reject is staged.
- Step 7 and a reject compare the shown stage with the row byte for byte (with `updated_at`, 1 s resolution): changed -> exit 1, nothing changed. No `--yes`; without a terminal exit 2 before reading (`proposal-apply.md` "Consent").

## Staleness

A staged `span_hash` other than the target's now (apply step 5) gives a note at the prompt and in `spec review`; it never refuses or unstages. No expiry by age.

## Documents

The review document gains `staged`, `staged_at`; inbox entries `staged_at`; pinned by `fixtures/daemon-keys.json`. Backups keep stages (`queue-backup.md` "Format"; dumps of schemas 1-5 restore, NULL where absent).

## UI

The decision dialog stages (`stageDecision`, `unstageDecision`), fields from the review document. The card then reads ``Staged <at>. Confirm on a terminal: `spec approve PR-0004` `` with Copy (fixed words and an ID matching `^PR-[0-9]{4,}$`, no free text) and Unstage. A staged proposal stays in the inbox; a stage changed outside this tab raises an alert. Clarification and defer are not sent (not served).

## Threat model

- Any local process of any account can read everything served and stage (loopback is not per-user); the fence stops other sites' pages.
- None confirms from an ordinary shell; a process faking a terminal (`script`) can, as with flags today.
- Residual: a reflex `y` on a stage an agent or a page injected. Mitigations: the whole choice and its time before the key, the outside-change alert, the event log, no MCP stage tool, the header rule.
- Hosting or a shared machine: infrastructure first (ADR-0034).
