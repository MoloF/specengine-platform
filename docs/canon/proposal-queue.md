---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store]
owner: owner
reviewed: 2026-10-15
---

# Proposal queue: commands, states, store

An agent or the owner proposes a change, touching no file; the owner decides on a terminal. Kind `update` replaces one node's span (a section with its subsections, or a file), applied as one commit where it was raised (ADR-0032, `proposal-apply.md`); `create`: `proposal-kinds.md`; `interpretation`, `decision`, `amendment`, `drift`: `proposal-<kind>.md`; `question`, `discrepancy`, `--brief`, MCP's queue tools: `agent-intake.md`; records: `decision-record.md`; revisions: `proposal-revisions.md`; inbox, review document, preview: `proposal-review.md`; completion, reject: `proposal-completion.md`. Library: `&Env, &Globals, &<Command>Request` (`git: GitEnv`, writers `now`, approve and reject a `Consent`); `main.rs` adds the terminal check, prompt and clock. `--task`: `tasks.md`. Code: core `patch` (`locate`, `span_bytes`, `splice`, `check_structure`, `update{,_refusal}`, `PatchCheck::introduced`), `proposal` (`PR-NNNN`, look-alikes, `prefix_clash`, `Author`, `commit_message{,_interpreting,_raising}`, trailers, `TEXT_MAX_BYTES`); store `introduced_findings`. Not yet: re-targeting, `has_open_proposal`.

## Commands

`--root`, `--json` as everywhere (CLI README); `--config` but the root's own `specengine.toml` -> exit 2. Outputs uncut by `OUTPUT_CAP_CHARS` but `--brief`'s.

- `spec propose update ID --base HASH --text-file F|- --rationale T [--author-role R] [--author-model M] [--run ID]` -> `PR-0001`, `introduced: <n>`, one `<severity>  <path>:<line>: <code>: <message>` per introduced finding. `HASH`: `spec show`'s `span_hash`. Role, model, run: `author_problem`'s grammar, else exit 2; any given -> author `agent`, none -> `human`. Text: UTF-8, <= 1 MiB (`TEXT_MAX_BYTES`), `-` = stdin; `--rationale` <= 4 096 bytes.
- `spec propose drift [REF…]`: `proposal-drift.md` "Raise".
- `spec inbox [--all]`, `spec review PR`: `proposal-review.md`.
- `spec approve PR [--note T]` -> `applied PR-0001 as <sha> on <branch>`; `spec reject PR --reason T` (empty -> exit 2) -> `rejected PR-0001`. Refused: stdout holds only a conflict's text; the reason is the `spec:` line and the last of `notes`.

JSON of propose, review, approve, reject: the **review document** (MCP `get_proposal`), its key order, `diff` and `preview`: `proposal-review.md`.

## Creation

`spec propose` stores nothing and takes no ID unless:

1. The target resolves as in `spec show` (CLI README "Rules"): an ID or `slug/ID` to one holder; a `.md` path to its file's document, the whole file (unlike `show`, no node -> exit 1). Not `class: generated`, its prefix (a section's: also its document's) without `immutable_text`. An alias, a legacy ID, `#SECTION`, `@rev`, `[[…]]` -> exit 1 naming the canonical ID when known; a look-alike, mixed script, `project:` -> exit 2. Stored **canonical**: a bare feature-scoped ID as `slug/ID`; a path as its document's `id:` as if written (refused, or held elsewhere: exit 1 `` `<p>` declares `<ID>`… ``), else as itself (an `id:` since -> exit 1).
2. `--base` is the span's hash now; stale -> exit 1 printing the current one.
3. The text verbatim (no line-ending normalisation), a section's trailing whitespace (space, tab, CR, LF) dropped, as no section span ends in it; spliced into exactly the span and parsed afresh, the file keeps its ordered (ID, heading level) list, the target spans exactly the text, and the file changes (else exit 1 `no change: …`). Refused: an `{#ID}` or `id:` dropped or added (`create`: `proposal-kinds.md`), a level changed, a heading of the same or a higher level added, a revision changed.
4. Validation: the tree read and parsed once, checked as is and with the one file swapped, judged as against `HEAD` (`spec-check-git.md`): introduced findings in `diagnostics`, never refusing (ADR-0012); a parser panic on the patched bytes gives none.
5. Binding (ADR-0032): the canonical worktree top, `root_rel` (the root in it, `''` at its top), the git common dir, the branch (`symbolic-ref`), `HEAD` as `base_commit`. A root in no worktree, a detached or unborn `HEAD` -> exit 2.

## Place, IDs, repositories

`PR` taken in `[ids]` (a prefix or `aliases_from`) -> exit 2. The queue is the project's `<slug>.db` (CLI README "Database"), shared by every worktree and repository of the slug; the current repository is the root's git common dir. An ID is `PR-` and 4 or more digits as written by the queue; a look-alike (U+0420 for `P`) -> exit 2 naming the Latin form; other text (`PR-1`, `PR-00001`) -> exit 1 `no proposal …`. Another repository's proposal (same slug) -> exit 2 naming it, `run the command there`. One whose recorded common dir no longer exists is an **orphan**: review and approve exit 2 `` `PR` belongs to the repository <dir> (worktree <w>), which no longer exists: `` + the inbox note's hint; only reject takes it (`proposal-completion.md` "Reject").

## Exit codes

0 done. 1 refused by the proposal or its target: unknown ID, creation, apply steps 3-10, applied or rejected, a declined prompt, a lost compare-and-set. 2 cannot run here: the CLI README's, `--config`, `PR` in `[ids]`, a look-alike ID, no terminal, another repository, an unbound place, no git identity, a corrupt row, a newer build's queue.

## States and events

`open -> approved -> applied`, `open -> rejected`; also `approved -> rejected` (no commit of it in history), `approved -> open` (a refusal reopening the run's own hold), `open -> applied` (a completion). `approved` only inside an apply or after its crash. Each change: one `Immediate` transaction with its event, `seq` rising, payload JSON with `id`: `proposal.created`, `.approved`, `.applied` (`commit`; a decision's `record` on both), `.rejected` (`reason`), `.apply_failed` (`step`, `reason`; one per attempt refused at steps 2-10), `.staged`, `.unstaged`. Closed, with `task.*` (`tasks.md`), `gate.observed`; `specengine-http` streams it.

## Store

Tables in `<slug>.db` beside the index, `STRICT`, every column `TEXT` but `seq`:

```
proposals(id PRIMARY KEY, project, kind, status, target_id, target_path, git_common_dir, worktree,
  root_rel, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author,
  diagnostics, decided_by, decided_at, decision_note, applied_commit, created_at, updated_at)
events(seq INTEGER PRIMARY KEY, project, type, payload, at)
```

`id`: highest number + 1, in the inserting transaction; rows never deleted, no ID reused. `target_id` canonical (Creation 1; a path iff `.md`, then `target_path`; unchecked when read), `target_path` its holder, root-relative; `base_hash` `b3:` + BLAKE3 of the span bytes (store `span_hash`); `patch_hash` `b3_hash(target_id LF base_hash LF new_text)`; `author` JSON `{type: human|agent, role, model, run}`; `diagnostics` JSON; times UTC `YYYY-MM-DDTHH:MM:SSZ` from the caller.

- **Schema**: the queue's own steps on `PRAGMA user_version` (`QUEUE_SCHEMA_VERSION` 8; higher -> exit 2, nothing changed; a migrated queue refuses older builds, `SchemaTooNew`), all due ones in one `Immediate` transaction; `proposal_columns(schema)` a schema's columns (`PROPOSAL_COLUMNS` 46). The index leaves them and `user_version` alone.

| Schema | Step adds | `proposals` columns | Canon |
|---|---|---|---|
| 1 | `proposals`, `events` | 24 | above |
| 2 | the intake's eleven, `target_ids` to `linked` | 35 | `agent-intake.md` "Stored" |
| 3 | a record's five, `record_id` to `choice` | 40 | `decision-record.md` "Queue and documents" |
| 4 | `tasks`, `runs`; `task_id` | 41 | `tasks.md` "Store" |
| 5 | `staged`, `staged_at` | 43 | `decision-staging.md` "Queue" |
| 6 | `interpretation` | 44 | `proposal-interpretation.md` "Row and review" |
| 7 | no column: open approve stages gain `"editorial":null` | 44 | `proposal-revisions.md` "Stage and UI" |
| 8 | `drift`, `drift_hash` | 46 | `proposal-drift.md` "The item" |

- **Connection**: the index's PRAGMAs but `synchronous=FULL`; until the daemon is the sole writer, CLI, MCP and `specengine-http` (stages) write directly.
- **Not derived**: backup `queue-backup.md` (ADR-0003).

`trait ProposalQueue`, `SqliteQueue::open(db, project)`; `QueueError` (`Status` names an applied one's commit; `Store`, `SchemaTooNew` exit 2). `Seen {status, updated_at, staged}` (`Proposal::seen()`), a run's compare-and-set key; `Decision {decided_by, note, staged_at}`.

| Op | Does |
|---|---|
| `create(&NewProposal, now)` | `open` under the next ID, `proposal.created`; a kind whose `family()` is not `Applies` -> `Invalid`; a reserved new ID -> `Reserved` |
| `create_drift(&[NewDrift], now)` | a raise's drift items, deduplicated (`proposal-drift.md` "Dedup") |
| `get`, `list(&ProposalFilter {git_common_dir, statuses})` | by ID number; a corrupt row fails, named |
| `list_readable` | `list` skipping a corrupt row into `ProposalList.unreadable` (`UnreadableRow {id, column, reason}`) |
| `approve_from(id, seen, decision, now)` | step 7: `open -> approved` (event), or `approved` kept with the decision replaced, no event, when `now` is later than `seen.updated_at` (else `Invalid`); state not `seen` -> `Changed`, nothing written |
| `reopen_from(id, held, failure, now)` | one `apply_failed`; `approved -> open`, decision cleared, only while still `held`, never at step 10 (`APPLY_VERIFY_STEP`) |
| `log_failure(id, failure, now)` | one `apply_failed`, state untouched |
| `applied_with(id, commit, decision, now)` | `open -> applied` (`.approved`, `.applied`) or `approved -> applied`, decision kept |
| `reject_from(id, seen, decision, now)` | `open`/`approved` -> `rejected`, `.rejected`, compare-and-set as above |
| `reject_orphan` | `reject_from`, for an orphan |

Also `events()`, `dump()`, `reserved()`; unconditional ops for store tests. Step 10's recording and a completion (`applied_with`, `applied`) are no compare-and-set: the commit is in history whichever run holds the state. Any op on `applied` or `rejected` -> `Status`, nothing logged.

**Checked when read**: a `base_commit` no object ID, a `branch` git refuses as a branch name or starting with `-`, an author field off its grammar: a corrupt row, never handed to git (`get`, `list` fail naming row and column, review, approve, reject exit 2; `inbox` skips it, a note).

## Terminal and git safety

- **Escaping** (agent-written text in the queue commands' output, prompts, stderr): every C0 control but LF, TAB and a CRLF's CR, DEL, every C1 and the bidi marks, embeddings, overrides and isolates (U+061C, U+200E, U+200F, U+202A-U+202E, U+2066-U+2069) print as `\u{1b}` (lower-case hex); JSON escapes by itself, values raw. `inbox`'s rationale turns a lone CR into a space.
- **Git** (store `WorktreeGit`): `git -C <dir>`, stdin null, `GIT_TERMINAL_PROMPT=0`, without every variable `git rev-parse --local-env-vars` lists; every command but `commit` with `GIT_OPTIONAL_LOCKS=0`. Paths as `:(literal)` pathspecs (`commit_only_paths`: several); every revision after `--end-of-options`, checked by `rev-parse --verify`; `blob_at`, `has_path` refuse an empty, `./` or `../` path unread. `diff`, `merge-file` with `GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1` (the repository's config still read). Scratch files (merge and diff sides, the message): new, in the data directory, removed after.
