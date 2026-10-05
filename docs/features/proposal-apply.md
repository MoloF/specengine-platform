---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-core, crates/specengine-cli, crates/specengine-mcp]
ref: proposal-apply analysis 2026-10-05, owner answers Q1-Q8 as recommended (session delegation); 08 §2 Phase 2, slice 1
shipped: 2026-10-05
adrs: [ADR-0032]
---

# Proposal apply: the one write door

## Why

Every Phase 2 spec change (accepted discrepancies, answers, the homoglyph fix of 08 §3 AC-11, compaction) goes through `apply_proposal` (ADR-0004, ADR-0005), which needs no daemon, `norm_hash` or kind vocabulary. Slice 1: the door and the smallest owner loop for one kind, `update` (replace one node's span), applied where it was raised (ADR-0032) as one commit with provenance.

How it works now: `docs/canon/proposal-queue.md` (commands, creation, states, store, escaping, git safety) and `docs/canon/proposal-apply.md` (consent, apply steps, completion, reject, known limits); the binding rule: `docs/canon/architecture.md#apply`. Next slices: 08 Phase 2.

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a`, `spec-b`, each with a linked worktree on branch `t1`, scratch `HOME`, injected clock; "library approve": consent yes, cwd the main worktree. "Refused": nothing stored or written, no commit, status `open`. M: the mutation that must turn it red.

- [x] AC-01 — `show --json` `span_hash` = `b3:` + BLAKE3 of the span bytes; `propose` with it from the linked worktree prints `PR-0001`; all files byte-identical, `git status` empty in both worktrees (M: hash the printed text; validation writes the file).
- [x] AC-02 — `--base` h1 after a committed span change (h2): exit 1 naming h2, refused, the next ID still `PR-0001` (M: base comparison dropped).
- [x] AC-03 — exit 1, refused: unknown ID; an ID in two files; spec-b `GLS-task-branch` (generated); spec-a `R-12` (immutable); text dropping `{#ID}`, adding an ID section, changing the level, adding a same-level heading (M: drop any one).
- [x] AC-04 — text citing an undeclared ID: `review` lists that finding as introduced; clean text, none; a pre-existing finding, never (M: validate the unpatched tree; attribute all).
- [x] AC-05 — `inbox` in ID order; `review` has every key of the document; two runs 1 s apart print byte-identical text and JSON; no key starts `block` (M: relative age).
- [x] AC-06 — piped stdin: `approve`, `reject` exit 2, refused, no event (M: terminal check removed).
- [x] AC-07 — library approve: one commit on `t1` in the linked worktree, subject and 4 trailers, only the target path, the file = old bytes with exactly the span replaced; main worktree `HEAD`, index, files unchanged; `applied`, the SHA stored (M: work in the cwd's worktree).
- [x] AC-08 — an unrelated staged file and another modified one: the commit holds only the target; both stay as they were (M: no `--only <path>`).
- [x] AC-09 — target staged-modified, then unstaged-modified: exit 1, refused, one `apply_failed` (M: dirty check dropped).
- [x] AC-10 — exit 2, refused: another branch; detached `HEAD`; worktree removed; merge in progress; a second repo with the same slug (M: drop any one).
- [x] AC-11 — two proposals on one section, different lines: the second rebases, both edits, two commits; overlapping: the second refused printing the conflict (08 §3 AC-4b in part; M: overwrite; take the proposal's side).
- [x] AC-12 — an edit above the target committed after creation survives; exactly the span replaced (M: splice at stored offsets).
- [x] AC-13 — the target's directory a symlink to outside: exit 1, the outside file unchanged (M: no symlink check).
- [x] AC-14 — `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` set to another repo: the commit lands only in the recorded worktree (M: pass them through).
- [x] AC-15 — `pre-commit` exits 1: exit 1, bytes restored, refused; the hook's marker file exists (M: no restore; `--no-verify`).
- [x] AC-16 — approving `applied`: exit 1 naming its commit; set back to `approved` by SQL, its commit on the branch: `applied`, no new commit (M: trailer lookup dropped).
- [x] AC-17 — `reject --reason x`: `rejected`, reason in `review`, files and commits unchanged, one `proposal.rejected`; approve then exits 1 (M: approve after reject).
- [x] AC-18 — the queue tables' dump equal across `spec index --full` and an `INDEX_FORMAT` bump (M: a queue table in the drop list).
- [x] AC-19 — one event per state change, `seq` rising; `PR-0001` with U+0420 for `P`: exit 2 naming `PR-0001`; `PR` in `[ids]`: exit 2 (M: skip an event; accept the look-alike).
- [x] AC-20 — AC-01, -07, -11 pass on spec-a and spec-b; a scan as core `tests/check_genre.rs` finds no fixture prefix in the new modules; manifests and lock unchanged (M: a hard-coded prefix; `similar` added).
- [x] AC-21 — docs gate clean; worst W 108 744 B ≤ min(109 484, W at the start: 109 294 at shipping, ~108 800 at the analysis); ADR-0032 1 254 B; the new canons 12 186 and 12 057 B ≤ 12 288; Tier 1 READMEs ≤ 10 240 B (store 10 194, CLI 10 020), index 9 587 B; `CLAUDE.md` 5 448 B, not grown; no document calls the DB derived or the commit optional (M: one byte added to `CLAUDE.md`).

## Implementation

Canon: `docs/canon/proposal-queue.md`, `proposal-apply.md` (new); `architecture.md#apply` (pointer, orphans); `spec-cli-bundle.md` (target header without ` | span …`), `mcp-read.md` (`span_hash`); CLI, store, core READMEs; 04 §6, 05 §3.3, §7, §8, 06 §3.2–3.4, 07 §2, 08 Phase 2 cut to pointers and corrected (the commit always made, no "main tree", the DB not derived). Six iterations (the last two owner-approved), the final review accepted; clippy, fmt, gate clean; every named mutation red.

| Module | What it does |
|---|---|
| core `patch.rs`, `proposal.rs` (new) | span, splice, structure check, creation refusals, introduced findings; `PR-NNNN` and look-alikes, `prefix_clash`, `Author`, the commit message, `patch_hash` input, UTC stamps |
| store `queue.rs` (new) | tables on the queue's own `user_version` step, `ProposalQueue`/`SqliteQueue`, compare-and-set ops, events, row checks at read, `synchronous=FULL` |
| store `worktree.rs` (new) | `WorktreeGit` (git without local `GIT_*`, place, dirty check, `merge-file`, diff, `commit --only`, trailer lookups, `blob_at`, `has_path`), `replace_file`, `same_repository` |
| store `update.rs` (new), `schema.rs`, `git.rs`, `lib.rs` | `span_hash`, `update_file`, `introduced_findings`; shared new-DB PRAGMAs, `same_dir`, re-exports |
| CLI `propose.rs`, `inbox.rs`, `review.rs`, `proposals.rs` (new) | creation; the inbox and its notes; the review document and preview; context, ID and repository rules, rendering |
| CLI `apply.rs`, `preflight.rs` (new) | approve (steps 7–10, completion, the reopen rule), reject; steps 2–6, the trailer lookup |
| CLI `lib.rs`, `main.rs`, `location.rs` | `escape_controls`, outcomes; the commands, the terminal check, the prompt, the clock; `prepared_data_dir` |
| CLI `show.rs`, `cap.rs`, `bundle.rs`; MCP `mirror.rs` | `span_hash`, the header's ` \| span b3:…` (not in bundles); the mirror key |

Tests: CLI `proposal_*.rs` (13 files), `common/proposal.rs`, the show-header pins (`show`, `show_tail`, `bundle`, `bundle_fit`, `archive`, `index`); store `queue.rs`, `worktree_git.rs`, `format.rs`; core `patch.rs`.

Deviations from the draft, now canon: reject takes `open` or `approved` (the owner's session rule), never one with a `Proposal:` commit on its branch; an orphan can be rejected; completion covers `open` too, judged on the commit's parent and own blob, over the whole branch when the base is pruned, in the current repository when the worktree is gone; runs meet by compare-and-set (step 7), reopen only their own hold, never at step 10; step 8 re-checks the place; creation refuses a no-op, `@rev`, `[[…]]`; escaping, read-time row checks, git option safety, `synchronous=FULL` added; BLAKE3 stays in the store; the bundle header has no span hash. Working answers the owner may still change: canonical-ID targets; sections lose trailing whitespace; the prompt after the read-only steps; a `proposal.approved` event; untracked targets and another `--config` refused; place refusals exit 2; the rationale in the commit.
