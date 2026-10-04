---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp]
ref: proposal-apply analysis 2026-10-05, owner answers Q1-Q8 as recommended (session delegation); 08 §2 Phase 2, slice 1
adrs: [ADR-0032]
---

# Proposal apply: the one write door

## Why

Every Phase 2 spec change (accepted discrepancies, answers, the homoglyph fix of 08 §3 AC-11, compaction) goes through `apply_proposal` (ADR-0004, ADR-0005), which needs no daemon, `norm_hash` or kind vocabulary. Slice 1: the door and the smallest owner loop for one kind, `update` (replace one node's span), applied where it was raised (ADR-0032) as one commit with provenance.

## Description and interactions

An agent or the owner runs `spec propose` (no file touched); the owner reads `spec inbox`, `spec review`, then, in a terminal, `spec approve` (apply, commit `spec: apply PR-0001`) or `spec reject`. `spec show` prints the span hash `--base` takes. Commands: the `specengine-cli` library (05 §1 principle 1), the terminal check in `main.rs`; splice and structure in core; queue and git writes in store. MCP gains only the `span_hash` mirror key. The queue lives in `<slug>.db` (CLI README "Database"), per git common dir: linked worktrees share it.

## Roles

- `rust-developer`: `crates/specengine-{store,core,cli,mcp}/src`; no manifest or lock change: std `IsTerminal`, system `git`, no `rusqlite_migration` or `similar` (Q8).
- `test-engineer`: `crates/*/tests`; temp git repos of `fixtures/spec-a`, `spec-b`, each with a linked worktree on branch `t1`; scratch `HOME`; injected clock.
- `spec-writer` at shipping: a Tier 2 canon for queue and apply (with Q2's consent rule); CLI, store README pointers; 05 §3.3, §7, §8, 06 §3.2–3.4, 07 §2, 08 Phase 2 cut to pointers and corrected (commit not optional; no "main tree"; DB not derived).

## Data

**Tables** (store, `STRICT`), made by the queue's own steps on `PRAGMA user_version` (0 → 1; higher → exit 2), outside the index's drop lists (store README "Cache key and format stamp"):

```sql
CREATE TABLE proposals (
  id TEXT PRIMARY KEY,  -- PR-0001: highest number + 1, in the inserting transaction; rows never deleted
  project TEXT, kind TEXT, status TEXT,  -- slug; 'update'; open|approved|applied|rejected
  target_id TEXT, target_path TEXT,  -- R-12 or slug/AC-01; root-relative
  git_common_dir TEXT, worktree TEXT, root_rel TEXT,  -- canonical; the root inside the worktree, '' at its top
  branch TEXT, base_commit TEXT,  -- HEAD at creation
  base_hash TEXT, base_text TEXT, new_text TEXT,  -- b3: of the span bytes; the span; its replacement
  patch_hash TEXT,  -- b3:(target_id LF base_hash LF new_text), 07 §1.2
  rationale TEXT, author TEXT,  -- JSON {type: human|agent, role, model, run}
  diagnostics TEXT,  -- JSON: introduced findings
  decided_by TEXT, decided_at TEXT, decision_note TEXT, applied_commit TEXT,
  created_at TEXT, updated_at TEXT);  -- UTC 2026-10-05T21:14:03Z
CREATE TABLE events (seq INTEGER PRIMARY KEY, project TEXT, type TEXT, payload TEXT, at TEXT);
```

States `open → approved → applied`, `open → rejected`; `approved` only inside an apply or after a crash in it. Events in their change's transaction, payload JSON with `id`: `proposal.created`, `.approved`, `.applied` (`commit`), `.rejected` (`reason`), `.apply_failed` (`step`, `reason`; one per attempt refused at steps 2–10). Store API, no `rusqlite` type public (`#distribution`): `trait ProposalQueue {create, get, list, approve, applied, reopen, reject, events}`, `SqliteQueue::open(db, project)`.

**CLI** (`--root`, `--json` as elsewhere; `--config` other than `<root>/specengine.toml` → exit 2):

- `spec propose update ID --base HASH --text-file F|- --rationale T [--author-role R] [--author-model M] [--run ID]` → `PR-0001`, `introduced: <n>`, the finding lines. Role, model, run: printable ASCII, no spaces, ≤ 128 B (any → `type: agent`). Text: UTF-8, ≤ 1 MiB.
- `spec inbox [--all]` → this repository's `open`, `approved` (`--all`: all) by ID: `<id> | update | <status> | <target_id> | <branch> | <created_at> | <rationale line 1, ≤ 80 chars>`; JSON `{proposals, notes}`.
- `spec review PR` → the keys below, `key: value` lines; rationale, diagnostics, diff, conflict as indented blocks.
- `spec approve PR [--note T]` → `applied PR-0001 as <sha> on <branch>`; `spec reject PR --reason T` (non-empty) → `rejected PR-0001`. JSON of propose, approve, reject: `review`'s.

**`review` JSON** (later MCP `get_proposal`; every key, absent = null, none starting `block`): `{id, project, kind, status, target_id, target_path, worktree, branch, base_commit, base_hash, base_text, new_text, patch_hash, rationale, author, diagnostics, diff, preview, conflict, decided_by, decided_at, decision_note, applied_commit, created_at, updated_at, notes}`. `preview` (open, approved): apply steps 2–6 read-only → `applies | rebases | conflicts | unavailable` (why: `notes`); `conflict`: `git merge-file`'s text. `diff`: base → new hunks of `git diff --no-index --no-color --no-ext-diff --diff-algorithm=myers -U3`, headers replaced by `--- base <path>`, `+++ proposed <path>`. Times as stored, never relative.

**Exit codes.** 0 done; 1 refused by the proposal or target (Rules); 2 cannot run here: the CLI README's, no git worktree, a look-alike or declared `PR`, non-terminal stdin, another repository, unbound place (creation 5, apply 2), no git identity.

**`spec show`**: node key `span_hash` = store `b3_hash` of the exact span bytes read (no added `\n`, no U+FFFD, all of it when output is cut); the text node line ends ` | span b3:<hex>`; MCP `get_node`'s mirror gains it.

**Commit** (ADR-0005; absent author fields → `unknown`):

```
spec: apply PR-0001

<rationale, verbatim>

Proposal: PR-0001
Decided-by: Ann Owner <ann@example.org>
Proposed-by: agent role=spec-writer model=claude-opus-5-5 run=unknown
Base-commit: <40 hex>
```

## Rules and edge cases

**Creation.** WHEN `spec propose` runs, the system SHALL store nothing and consume no ID unless:

1. ID (an ID or `slug/ID`; alias, path, `#SECTION` → exit 1 naming the canonical ID if known) resolves as in `spec show` to one holder, not `class: generated` (`#apply`), prefix not `immutable_text` (Q5);
2. `--base` = the current span hash (Q4; stale → exit 1 printing it);
3. the text, verbatim (A8) but a section's trailing whitespace dropped (spans never end in it), spliced and parsed afresh, keeps the file's ordered (ID, heading level) list, the target's span exactly the text (a section's span holds its subsections: model `Node::span`);
4. validation: `check_source` on the worktree and on an overlay `Source` with the target replaced, `judge` against the unpatched findings; introduced ones stored (05 §7 item 2), never refusing (ADR-0012);
5. binding (ADR-0032): canonical worktree top, `root_rel`, common dir, branch (`symbolic-ref`), `HEAD`.

**Apply**, `approve(&Env, &Globals, &ApproveRequest {id, note, now}, consent)`:

1. Consent: `main.rs` refuses a non-terminal stdin (no event); after step 6 the callback prompts `apply PR-0001 to <path> on <branch> in <worktree> (applies|rebases)? [y/N]` on stderr; only `y`/`yes` consents.
2. Place: the worktree exists, top and common dir as recorded, `symbolic-ref` = branch; no merge, rebase, cherry-pick, revert, bisect or sequencer state. Git runs `-C <worktree>`, stdin null, `GIT_TERMINAL_PROMPT=0`, without every variable `git rev-parse --local-env-vars` lists.
3. File: probed and read by the store's `WorkingTree` of `<worktree>/<root_rel>` (walk rules, no symlink component); regular, UTF-8, not generated; tracked, `git status --porcelain=v1 -z --untracked-files=all -- <path>` empty.
4. Resolve: one holder in the recorded root (index refreshed), at `target_path`, in a fresh parse of the bytes just read; never stored offsets.
5. Text: span hash = `base_hash` → `new_text`; else `git merge-file -p -L current -L base -L proposed` over data-directory temp files: clean → the merge; conflict → exit 1 printing it.
6. Structure: creation 3 on this file.
7. `approved` in one `Immediate` transaction, only from `open`/`approved`; `decided_by` = `git var GIT_COMMITTER_IDENT` minus the date.
8. Write: re-read; bytes ≠ step 4's → refuse; else temp sibling + rename, mode kept.
9. Commit: `git commit --only --cleanup=verbatim -F <temp> -- <path>`; hooks run, never `--no-verify`; failure → bytes restored, `open`.
10. Verify: parent = old `HEAD`, only `<path>` changed, `Proposal:` = id → `applied`, `applied_commit`, index `update_paths` (failure: `warning:`); else stays `approved`, exit 1 naming the commit.

**Idempotence.** `applied` → exit 1 naming its commit; `rejected` → exit 1. `approved`: a commit in `base_commit..<branch>` with that `Proposal:` trailer, changing only the path, completes it, no new commit; else steps 2–10 (a crash after the write leaves the file dirty: step 3 refuses until the owner restores or commits it).

**Reject.** Terminal and prompt as approve; `open` only; `decided_by` from the recorded worktree, else the current repository.

**Safety.** Only the target file and the recorded worktree's git state change; only the recorded branch moves; other open proposals on the node stay open and rebase at their own apply (ADR-0012). `inbox` lists only the current common dir's proposals; naming another's → exit 2. `PR-` + 4+ digits is the engine's; a look-alike (`script::ascii_look_alike`) → exit 2 naming the fix; `PR` in `[ids]` (prefix or `aliases_from`) → every queue command exit 2 (Q7).

## Assumptions and risks

A1 one kind, target, file, commit. A2 staleness = BLAKE3 of span bytes, stricter than `norm_hash` (05 §3.3). A3 CLI processes write the queue until the daemon. A4 approve = apply; no decision record, the commit is the record (Q3). A5 author self-reported; decider = the worktree's git identity (ADR-0017). A6 Claude Code's Bash tool has a non-terminal stdin (verify): decide in a separate terminal. A7 review writes nothing in a worktree. A8 no line-ending normalisation.
R1 a pseudo-terminal passes; real consent needs the consent tool (08 §3 AC-4). R2 no backup until slice 2. R3 racing terminals: `index.lock` reported. R4 Phase 3's rev rule (08 AC-13) will refuse unbumped apply commits. R5 hooks or signing needing stdin fail → restored.

## Acceptance criteria

Setup: the Roles' temp repos; "library approve": consent yes, cwd the main worktree. "Refused": nothing stored or written, no commit, status `open`.

- [ ] AC-01 — `show --json` `span_hash` = `b3:` + BLAKE3 of the span bytes; `propose` with it from the linked worktree prints `PR-0001`; all files byte-identical, `git status` empty in both worktrees (M: hash the printed text; validation writes the file).
- [ ] AC-02 — `--base` h1 after a committed span change (h2): exit 1 naming h2, refused, the next ID still `PR-0001` (M: base comparison dropped).
- [ ] AC-03 — exit 1, refused: unknown ID; an ID in two files; spec-b `GLS-task-branch` (generated); spec-a `R-12` (immutable); text dropping `{#ID}`, adding an ID section, changing the level, adding a same-level heading (M: drop any one).
- [ ] AC-04 — text citing an undeclared ID: `review` lists that finding as introduced; clean text, none; a pre-existing finding, never (M: validate the unpatched tree; attribute all).
- [ ] AC-05 — `inbox` in ID order; `review` has every key of Data; two runs 1 s apart print byte-identical text and JSON; no key starts `block` (M: relative age).
- [ ] AC-06 — piped stdin: `approve`, `reject` exit 2, refused, no event (M: terminal check removed).
- [ ] AC-07 — library approve: one commit on `t1` in the linked worktree, subject and 4 trailers, only the target path, the file = old bytes with exactly the span replaced; main worktree `HEAD`, index, files unchanged; `applied`, the SHA stored (M: work in the cwd's worktree).
- [ ] AC-08 — an unrelated staged file and another modified one: the commit holds only the target; both stay as they were (M: no `--only <path>`).
- [ ] AC-09 — target staged-modified, then unstaged-modified: exit 1, refused, one `apply_failed` (M: dirty check dropped).
- [ ] AC-10 — exit 2, refused: another branch; detached `HEAD`; worktree removed; merge in progress; a second repo with the same slug (M: drop any one).
- [ ] AC-11 — two proposals on one section, different lines: the second rebases, both edits, two commits; overlapping: the second refused printing the conflict (08 §3 AC-4b in part; M: overwrite; take the proposal's side).
- [ ] AC-12 — an edit above the target committed after creation survives; exactly the span replaced (M: splice at stored offsets).
- [ ] AC-13 — the target's directory a symlink to outside: exit 1, the outside file unchanged (M: no symlink check).
- [ ] AC-14 — `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` set to another repo: the commit lands only in the recorded worktree (M: pass them through).
- [ ] AC-15 — `pre-commit` exits 1: exit 1, bytes restored, refused; the hook's marker file exists (M: no restore; `--no-verify`).
- [ ] AC-16 — approving `applied`: exit 1 naming its commit; set back to `approved` by SQL, its commit on the branch: `applied`, no new commit (M: trailer lookup dropped).
- [ ] AC-17 — `reject --reason x`: `rejected`, reason in `review`, files and commits unchanged, one `proposal.rejected`; approve then exits 1 (M: approve after reject).
- [ ] AC-18 — the queue tables' dump equal across `spec index --full` and an `INDEX_FORMAT` bump (M: a queue table in the drop list).
- [ ] AC-19 — one event per state change, `seq` rising; `PR-0001` with U+0420 for `P`: exit 2 naming `PR-0001`; `PR` in `[ids]`: exit 2 (M: skip an event; accept the look-alike).
- [ ] AC-20 — AC-01, -07, -11 pass on spec-a and spec-b; a scan as core `tests/check_genre.rs` finds no fixture prefix in the new modules; manifests and lock unchanged (M: a hard-coded prefix; `similar` added).
- [ ] AC-21 — docs gate clean; worst W ≤ min(109 484, W at the start); ADR-0032 ≤ 1 536 B; the new canon ≤ 12 288 B; Tier 1 READMEs, index ≤ 10 240 B; `CLAUDE.md` not grown; no document calls the DB derived or the commit optional (M: one byte added to `CLAUDE.md`).

## Next slices

2 `queue-export` (may swap with 3): queue page, `--state` JSONL backup, `import-state` (ADR-0003, Q6). 3 `agent-intake`: MCP write tools (`propose_change`, `get_proposal`, `report_discrepancy`, `ask_question`), discrepancy and question kinds, dedup (08 §3 AC-5), clarification, defer, edit-and-accept, comments, decision records (their prefixes: an ADR on `#universal`). 4 `task-package`: the ADR-0027 package, `spec_snapshot`, `stale`, task tools; proposals gain `task_id`. Then daemon, gate hook, plugin, consent tools.

## Out of scope

The next slices; other kinds; multi-target, re-targeting; `has_open_proposal`; `norm_hash`, rev bumps; the real `review_proposal`; this repository's own process.

## Open

Working answers made here (owner may change): canonical-ID targets; sections lose trailing whitespace; the prompt after the read-only steps; a `proposal.approved` event; untracked targets and an outside `--config` refused; place refusals exit 2; rationale in the commit.

## Implementation

Filled in after implementation.
