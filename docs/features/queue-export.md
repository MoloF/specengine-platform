---
class: spec
status: shipped
scope: [crates/specengine-store, crates/specengine-cli]
ref: queue-export analysis 2026-10-05, owner answers Q1-Q7 as recommended (session rule); 08 §2 Phase 2, slice 2
shipped: 2026-10-05
---

# Queue backup and restore

## Why

The queue (`proposals`, `events` in `<slug>.db`) is the only state git cannot rebuild, and nothing backed it up: a lost DB lost open work and restarted IDs at `PR-0001`, colliding with old `Proposal:` commits. ADR-0003: a JSONL dump to a backup directory, restored by `spec import-state`. No new ADR: `spec export state` spells ADR-0003's `export --state` as a mode, like `export index`; bare `spec export` (`queue.md`) goes to `task-package`.

How it works now: `docs/canon/queue-backup.md` (commands, format, export bounds and placement, import steps, the one door, store ops, known limits); `docs/canon/architecture.md#storage`.

## Acceptance criteria

Setup: temp git repos of `fixtures/spec-a` (`spec-b` if named), scratch `HOME`, injected clock, consent yes; fresh: another `HOME`. M: the mutation turning it red.

- [x] AC-01 — every state, a second repository, an orphan, an unreadable row (bad `base_commit`), their events: export, import fresh, `dump()` equal (M: export via `list_readable`; filter by common dir; drop `decision_note`).
- [x] AC-02 — equal rows inserted in another order, exported later: byte-identical files (M: order by rowid; export time inside).
- [x] AC-03 — restored highest `PR-0007`, `seq` N: the next `propose` prints `PR-0008`, its event `seq` N+1; import adds no event (M: renumber from `PR-0001`; an import event).
- [x] AC-04 — non-empty queue: exit 2 naming both counts, no prompt, `dump()` unchanged (M: insert-or-ignore).
- [x] AC-05 — one defect on a valid dump's last line (not JSON, unknown table, unknown or missing column, a number in a TEXT column, repeated `id`, repeated `seq`, `project` ≠ header), each exit 2 naming the line; cut after a whole row: exit 2 naming the counts; queue empty (M: commit per row; drop any one check; count check removed).
- [x] AC-06 — `format` 2, `queue_schema` 2: exit 2 with `upgrade SpecEngine`; a spec-b dump into spec-a: exit 2 naming both; queue empty (M: either check removed).
- [x] AC-07 — an `approved` proposal with its commit on its branch, exported, imported fresh: both worktrees' `git status` empty, `HEAD` and tips unchanged, still `approved` (M: import runs the completion lookup).
- [x] AC-08 — `--out` in a missing subdirectory of the worktree, or via a symlink into it: exit 2, nothing written; an existing file: exit 2, bytes untouched; the file 0600, `backups/` 0700, no `.partial` left; stdout: path, counts (M: worktree check removed; truncate; partial kept).
- [x] AC-09 — a proposal's common dir gone: `inbox` notes it, `review`, `approve` exit 2 (orphan), `reject` takes it; stored `git_common_dir`, `worktree` byte-equal to the dump (M: import rewrites paths).
- [x] AC-10 — piped stdin: exit 2 with the terminal message, a missing FILE too; an answer other than `y`/`yes`: exit 1, queue unchanged (M: terminal check removed or after the read).
- [x] AC-11 — the new tests pass under process `HOME=/nonexistent`; no dump under `fixtures/`; the anonymity test green (M: a test using the process `HOME`; a committed dump).
- [x] AC-12 — gate clean; worst W ≤ 108 744 B; new Tier 2 canon ≤ 12 288 B, index ≤ 10 240 B, CLI README ≤ 10 194 B; `CLAUDE.md`, store README not grown; `grep -r "until slice 2" docs/canon crates/*/README.md` empty; `architecture.md#storage`, 07 §2 spell both commands (M: one byte added to `CLAUDE.md`).

## Implementation

Canon: `docs/canon/queue-backup.md` (new); pointers in `proposal-queue.md` ("Store", "Terminal and git safety"), `proposal-apply.md` ("Consent"; the slice-2 limit gone), `architecture.md#storage`, CLI and store READMEs; 05 §8, 07 §2, 08 Phase 2. Three iterations, the reviews of the first two accepted; clippy, fmt, gate clean.

| Module | What it does |
|---|---|
| store `queue/state.rs` (new), `queue.rs`, `lib.rs` | `PROPOSAL_COLUMNS`, `EVENT_COLUMNS`, `StoredQueue`, `QueueCounts`, `Restore`; `open_existing`, `stored_rows`, `counts`, `restore`; re-exports |
| store `worktree.rs` | `top_if_repository`, `worktrees()` → `ListedWorktree` |
| CLI `state_file.rs` (new) | `STATE_FORMAT`, `render`, `parse` (own JSON visitor keeping repeated keys; defects by line) |
| CLI `state.rs` (new) | `export_state` (bounds, placement, cleanup), `import_state` (steps 2–5), outcomes |
| CLI `main.rs`, `lib.rs` | `export state --out`, `import-state FILE`, the terminal check; `Outcome::{StateExport, StateImport}` |
| CLI `location.rs`, `proposals.rs`, `export.rs` | `checked_data_dir` (creates nothing); the shared `--config` rule; `identity()` |

Tests: CLI `tests/queue_state.rs` (AC-01–AC-11), store `tests/queue_state.rs` (column lists, open, rows, restore, continuity, `top_if_repository`, `worktrees`).

Iteration 3: a queue holding the top ID `PR-18446744073709551615` exports, imports and re-exports byte-identically, the restored queue refusing the next `propose` as the original did (`state_file.rs`: IDs 1 to 2⁶⁴−1); a listed entry that is the common dir itself (`--separate-git-dir`) or bare is named "the git directory …" (`state.rs`, `Bound::is`). Accepted deviation: CLI sources cite canon `queue-backup`, "Heading" (no `docs/` path: the genre test forbids it), the store `docs/canon/queue-backup.md` "Store".

Deviations from the draft, now canon: the bound is the whole repository (the root's top, every listed worktree, the main one by a `.git` common dir, the common dir), held by path or by device and inode; only git's "not a repository" falls back to the root, any other git failure is exit 2 (git ≥ 2.36); "renamed while absent" is a hard link (a rename without hard links), the partial removed, the directory synced, created directories removed on failure, a stale partial refused; no DB or no tables exports an empty dump; import reads only a regular file, takes IDs 1 to 2⁶⁴−1, rows in any order, CRLF; declined: exit 1, stdout empty; both refuse another `--config`; the printed path is as given; a race at step 5 is `Restore::Occupied`. Accepted residuals: the canon's "Known limits".
