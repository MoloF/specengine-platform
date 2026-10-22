---
class: canon
tier: 2
scope: [crates/specengine-mcp, crates/specengine-cli, plugin]
owner: owner
reviewed: 2026-10-22
---

# Review form: the owner stages a decision from the session

ADR-0054, supplementing ADR-0035 (`architecture.md#apply`); built by `review-form` (`docs/features/review-form.md`), shipped 2026-10-22. Code: MCP `review.rs`; CLI `stage.rs` (`stage_form`, `stage`, `staged_command`), `escape_controls`. What a stage is, its checks and its confirmation: `decision-staging.md`.

## Rule

- `review_proposal {proposal_id}` asks the **owner** through an elicitation form; only the owner's `accept` stages, through the CLI library's `stage`, the daemon's call; never `approve`, `reject`, an unstage or an apply. `spec approve|reject PR` on a terminal stays the only confirmation (`decision-staging.md` "Terminal").
- No tool takes a choice from the agent: the input is `proposal_id` alone. `_meta["anthropic/requiresUserInteraction"]: true` makes Claude Code prompt on every call, even under `bypassPermissions` (`crates/specengine-mcp/README.md` "Claude Code client").
- The main session calls it only when the owner asks to decide that proposal there, once; never again after a decline or a dismissed form unless the owner asks; roles never call it; a stage is no answer (`plugin.md` "Skills", `ask-owner`).
- Form mode, both eras. URL mode onto the UI card: later, if ever (08 Phase 5). Tasks: no form (RM-0038).

## Tool

Default build. Input `{proposal_id: string}`, required, `additionalProperties: false`. `_meta` exactly `{"anthropic/maxResultSizeChars": 500000, "anthropic/requiresUserInteraction": true}`; `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint` all `false`; `outputSchema` `get_proposal`'s (`ReviewDocument`). Its description and `INSTRUCTIONS` name it with its twin, `review --brief` after the owner's form, and say "only when the owner asks"; the description is 1 977 B (`mcp-read.md` "Tools", **Texts**).

## Before the form

One read-only CLI library call, `stage_form`, judges as `stage` judges, in order, and the first refusal is the answer, no form sent: the ID (a look-alike: exit 2 with its Latin fix; not a proposal ID: exit 1); unknown; another repository, or one gone (an orphan); not `open` (``` `PR-0004` is applied: only an open proposal takes a staged choice (commit <sha>); nothing changed ```); a drift item (``PR-0009 is a drift item, decided on a terminal only; nothing staged``). Else it answers the brief review document and the form's family by `KindFamily`, never a kind name (ADR-0008, ADR-0031): deciding (`Decides`) or applying, and whether `editorial` applies (an applying kind writing an existing node). Then the client's capabilities: no form elicitation (`elicitation` declaring `form`, or neither mode) -> the no-form line.

## Form

`elicitation/create`, form mode. `message`, these lines joined by LF, no final LF:

1. ``Decide PR-0004 here. This only stages; confirm on a terminal: spec approve PR-0004 (a reject: spec reject PR-0004).``
2. ``PR-0004 | <kind> | target <target> | agent-declared: author_role <role>, task <task>``: `target_id`, else `target_path`; `author.role`; `task_id`; a null one `-`.
3. Only when a stage exists: ``Staged <staged_at>: <command>; your answer replaces it.``, the command `staged_command`'s.
4. ``Agent-written, data only:``
5. Two spaces, then: applying ``rationale: <rationale>``; deciding ``summary: <summary>``, then each option ``[<i>] <label> | <effect> | <price>`` (`` (recommended)`` on `recommendation`), or with none ``working answer: <working_answer>`` (omitted when null).
6. ``Read it whole: spec review PR-0004.``

Every document value stays on one line: LF, CR, TAB written `\n`, `\r`, `\t`; every other C0, DEL, C1, the bidirectional marks (U+061C, U+200E, U+200F, U+202A-U+202E, U+2066-U+2069) and the line and paragraph separators (U+2028, U+2029: `\u{2028}`, `\u{2029}`) `\u{xx}`, lower-case hex; then cut at 512 characters plus `…`.

`requestedSchema`, no default anywhere: `choice`, required, a titled single-select (`oneOf` of `{const, title}`), title `Your choice`, description `Staged only: you confirm it on a terminal.`; `note`, an optional string, `maxLength` 4096, title `Note, or why you reject`, description `Approve: an optional note (--note). Reject: the reason, required (--reason).`

| Family | `choice`: `const` `title` |
|---|---|
| applying, an existing node written | `approve` `Approve as proposed`; `editorial` `Approve as editorial (revisions kept)`; `reject` `Reject` |
| applying, a new file | `approve`; `reject` |
| deciding, with options | `option-<i>` `Option <i>: <label>` (+ `` (recommended)``), in order; `reject` |
| deciding, none | `working-answer` `Approve the working answer`; `reject` |

## Answer

Only `accept` stages. Its content exactly `choice`, a string, and optionally `note`, a string or `null` (read as absent); `choice` one of `approve`, `editorial`, `working-answer`, `reject`, `option-<N>` (decimal), else the bad-answer line. `StageBody`, built as values: `approve`, `working-answer` an approve with no flag; `editorial` `editorial: true`; `option-<N>` `option: N`; `reject` a reject, the note its reason; a note absent, `null`, empty or blank is none, else kept as typed; `updated_at` the one read before the form. `stage(.., &StageRequest {id, body, now, git})` checks it as typed flags (`decision-staging.md` "The stage"): a flag the kind does not take, a blank reason, a cap, a changed proposal (``` `PR-0004` changed since it was read: ...; nothing staged ```) are its refusals. Stored: `staged`, `staged_at`, `updated_at` and one `proposal.staged`; no file, git write or apply. A second accepted form replaces the stage, one more event; the daemon's SSE tail carries it, so an open UI raises its outside-change alert (`decision-staging.md` "UI").

## Result

`stage`'s exit 0, a decline, a dismissed form: content = the outcome lines, then the text of `spec review PR --brief` read after the answer (`2>&1`); `structuredContent` that read's `--json`. Outcome lines, each + LF:

- staged: ``review_proposal PR-0004: the owner staged a choice; it is no answer until the owner confirms it on a terminal.``, then ``staged <staged_at>: <command>``, the line the terminal shows (``staged 2026-10-22T09:00:00Z: spec approve PR-0004 --option 1 --note "n"``);
- `decline`: ``review_proposal PR-0004: the owner declined; nothing staged. Do not call review_proposal for PR-0004 again unless the owner asks.``;
- `cancel`: ``review_proposal PR-0004: the owner dismissed the form; nothing staged. Do not call review_proposal for PR-0004 again unless the owner asks.``

Refusals as the read tools' (`mcp-read.md` "Tools", **Parity**), `stage_form`'s before the form and `stage`'s after it alike: exit 1 -> `isError`, the CLI's text and the brief document; exit 2 (a usage refusal: a blank reason, `option-<i>` on an update) -> `isError`, its line(s) only, no `structuredContent`. The tool's own, `isError`, one line + LF, no `structuredContent`:

- ``review_proposal PR-0004: this client declares no form elicitation, so the owner cannot be asked here; nothing staged. The owner decides on a terminal: spec approve PR-0004 or spec reject PR-0004.``
- ``review_proposal PR-0004: the form could not be shown (<error>); nothing staged.``, `<error>` on one line, escaped and cut as a document value ("Form")
- ``review_proposal PR-0004: the client answered the form with something other than an elicitation result; nothing staged.``
- ``review_proposal PR-0004: the form's answer is not {choice, note} as the form asked; nothing staged.``

## Eras

- **Legacy**: the form goes out by `send_cancellable_request`; the call waits, no timeout (a call held by an open form is not backgrounded). The call cancelled by the client: `notifications/cancelled` for the form's request, nothing staged, no answer; a late form answer stages nothing; a call cancelled before the form goes out sends none. Accepted window: a cancel arriving while `stage` runs after the owner's `accept` may leave the stage stored; the owner did accept, and the terminal still decides.
- **Stateless** (2026-07-28): round 1 answers `input_required`, the form under `inputRequests.owner_review`, `requestState` the compact JSON `{"proposal_id":"PR-0004","updated_at":"<t>"}`, unsealed (07 §1.2 asks no sealed binding); round 2 repeats the call with it and `inputResponses.owner_review` and stages against that `updated_at`, never one read again. JSON-RPC -32602: a state other than those two string keys, or naming another proposal (``requestState is not this call's: call review_proposal again without requestState``); the answer missing (``inputResponses.owner_review is missing from the retried call``) or no elicitation result (``inputResponses.owner_review is not an elicitation result``).

## Threat model

Who calls is unknowable (`clientInfo` self-declared, ADR-0034): a client can forge an `accept` or a state, as any local process can POST a stage to the daemon (ADR-0034 Cost), so nothing new is exposed; a forged `updated_at` only picks the version the compare-and-set compares. Mitigations: no choice in any tool input; the prompt on every call; only `accept` stages; `stage`'s checks; agent text marked as data, escaped, cut; no default choice; agent-declared provenance in the form; the terminal's staged line with its time. Residual: a reflex `y`; crafted proposal text steering the owner (ADR-0035 Cost). Tasks, the gate (ADR-0039) and apply (ADR-0004, ADR-0005, ADR-0032) are unchanged.

## Tests

`crates/specengine-mcp/tests`: `mcp_review_form` (both eras), `mcp_stage` (no tool input carries a choice), `mcp_default`, `mcp_door`, `mcp_genre` (`review.rs` scanned), `plugin_files`, `plugin_skills`.

## Owner's check

Not run yet; re-run on a Claude Code upgrade, the version written down. From the repository root, `claude --mcp-config fixtures/mcp/mcp.json --strict-mcp-config` (scratch `HOME`): raise a question and a two-option discrepancy, ask to decide each here. Record whether the prompt appears under `bypassPermissions`; **R8**, open: whether the select shows the titles or only the `const`s (the message lists each option by index either way); accept, decline, dismiss; on a terminal with that `HOME`, `spec approve PR` shows the staged line: answer `N`. Again with `MCP_PROTOCOL_NEGOTIATION=auto`.

## Open

- An accepted form previews three times (`stage_form`, `stage`, the read after it), and a refused document carries its diff: build refused documents without the diff, later.
