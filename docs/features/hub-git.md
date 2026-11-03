---
class: spec
status: shipped
shipped: 2026-11-03
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: RM-0051; Phase 7 step 10b (08 s2); the hub-git analysis, the owner's decision of 2026-11-03 (Q1) and the orchestrator's decisions of 2026-11-03, with iteration 2 on the review's findings
adrs: [ADR-0064]
---

# hub-git: fetch, merge and push, never rebase, with a push status on screen (RM-0051)

## Why

Since `hub-confirm`, a confirm committed in the hub's own checkout, and nothing fetched, merged or pushed the commit. So it lived on one host. Leads confirmed against a brief that never saw a developer's commit. And a push by hand invited `git pull --rebase` or a force push, rewriting SHAs the queue stores (`applied_commit`, `Base-commit:`, completion's range).

`apply-outputs` (RM-0072, 10a) made a hub commit safe to push; this slice is the first network verb.

Decided:
- **Q1** (the owner): the game's main branch, merge commits only on divergence.
- **D1:** only a hub with `--push` fetches and pushes, from one store module, apart from the confirm.
- **D2:** merge, never rebase.
- **D3:** the sole writer is a convention.
- **D6:** credentials are git's.
- **Q4:** the operator resolves a conflict on the host's terminal.
- **In scope:** A12 (index-only conflicts rendered again), and the screen.

## Description and interactions

Canon: `docs/canon/hub-git.md` (ADR-0064).

### The sync

Look, fetch, merge inside the turns, push: `hub-git.md` "Sync", "Merge commit".

### Status and the screen

`GET .../sync`, `SyncLine`: `hub-git.md` "Status", "Screen". The confirm dialog after a 200: `hub-confirm.md` "Screen".

### The sparse set

`spec sparse-set`: `hub-git.md` "Sparse set".

## Data

Flags, the status document, states, messages: `hub-git.md`.

**Bumps:**
- `GET_ROUTES` from 25 to 26;
- daemon-keys set 110 `HubSync`;
- three flags and a start line;
- one CLI command.

No queue schema, index format, event, `daemon.json` key, MCP or plugin change.

## Rules and edge cases

`hub-git.md` "Rules", "Known limits".

## Acceptance criteria

- [x] P1 — The hub only ahead: the remote tip is `applied_commit`, `ok`; a host commit travels too. M: no push.
- [x] P2 — Diverged: a two-parent merge, first parent `applied_commit`. M: `git pull --rebase`.
- [x] P3 — P2 under `pull.rebase`, `branch.main.rebase`, `merge.autoStash`, `merge.ff=only`: the same. M: the sync as `git pull`.
- [x] P4 — A conflict: `held`, path named, nothing in progress, status empty, remote unchanged; a confirm commits. M: leave the merge in progress; `-X ours`.
- [x] P5 — An index-only conflict: the merge's `docs/index.md` is its own render, no `index-drift`. M: abort on any conflict; keep the conflicted bytes.
- [x] P6 — A receivepack wrapper lands a developer's commit first: both reachable, no `--force*`. M: no retry; a retry with `--force`.
- [x] P7 — A refusing `pre-receive` with U+202E: `held`, `\u{202e}` shown, the commit kept. M: reset to the remote.
- [x] P8 — A wrapper failing after receive-pack succeeded: `retrying`, then `ok`, no merge commit. M: status from the exit code.
- [x] P9 — A 30 s `pre-receive`: killed at 3 s, `retrying`; reads and a confirm answer meanwhile. M: push inside the turn; no deadline.
- [x] P10 — An orphan remote: `held`, unchanged. M: `--allow-unrelated-histories`.
- [x] P11 — A remote force-pushed back: `applied_commit` reachable again. M: reset to the remote.
- [x] P12 — A failing `pre-push`: nothing pushed, `retrying`, its words shown. M: `--no-verify`.
- [x] P13 — A modified and an untracked file: `stopped`, both named and untouched. M: `stash`, `reset --hard`, `clean`.
- [x] P14 — A local daemon, a hub without `--push`, `spec serve`, terminal approve and reject: no uploadpack or receivepack run; `--push` without `--public-origin` exits 2. M: sync on every daemon; push from `apply.rs`.
- [x] P15 — String literals: `"fetch"`, `"push"` in store `sync.rs` alone, no credential names. Argv log by verb and push flag: no rebase or pull family, no forbidden push flag, `GIT_NO_LAZY_FETCH=1` but on fetch and push. M: `--force`; `GIT_SSH_COMMAND`; drop the variable.
- [x] P16 — No manifest names `git2`, `gix`, `reqwest`, `ureq`, `ssh2`, `russh`. M: add `gix`.
- [x] P17 — A password, a token user, an `insteadOf` rewrite: `stopped` before any fetch, the secret nowhere; an ssh user passes. M: no check; `remote.origin.url` alone.
- [x] P18 — `GET .../sync`: five keys on a `--push` hub, 404 elsewhere; 26 routes pinned, set 110. M: mount it locally; on every hub.
- [x] P19 — The merge commit: no trailer, git's message, the checkout's identity; none set: `stopped`. M: `identified()`'s login.
- [x] P20 — `git status` empty after every outcome. M: a scratch file left.
- [x] P21 — `spec sparse-set` lists roots beyond `docs/`, names a missing one; a hub stops on it. M: hardcode `docs/`.
- [x] P22 — The harness refuses `ssh://` before DNS. M: no `GIT_ALLOW_PROTOCOL`.
- [x] P23 — Check, health, gate and reads byte-identical, ahead or offline. M: a row from push state.
- [x] P24 — Non-ASCII before `://`: no panic, the token replaced whole. M: byte-index slicing.
- [x] P25 — A fast-forward past the deadline: SIGTERM, SIGKILL 2 s later, no `index.lock`, a confirm commits. M: SIGKILL at once.
- [x] P26 — A second worktree: `stopped`, `this worktree is not synced: the sync runs in <top>`; its gap does not stop the first. M: share the first's status.
- [x] P27 — A confirm mid-apply while a sync looks: never `stopped`. M: cleanliness decided in the look.
- [x] U1 — The line: `<N> commits not pushed`, `Sync: <error>` as text, nothing locally or on a 404. The dialog stays open on a 200 with notes. M: markup; the line locally; closing whatever the notes.

## Out of scope

The board snapshot (RM-0052, deferred), `/mcp` (RM-0053), `deploy/` (RM-0064), `spec admin` (RM-0065), tags (RM-0056), `pushRemote`, spec-ID collisions (RM-0073), signed commits, a conflict UI.

## Implementation

Two iterations: the review returned REWORK (a non-ASCII panic in URL redaction), then ACCEPT with one docs minor and four nits, all recorded in `hub-git.md` "Known limits". Tests: 4794 of 4794 (`hub_git` 33, store `sync::tests` 3, `sync_urls` 2, `sparse_set`); UI 3514+; clippy clean. Mutations: 35, then 20, all killed.

Accepted deviations from the draft:
1. the render inside a merge has no deadline, since it does local reads only;
2. no push when `L` = `R`, so the state is `ok`;
3. the separator is `; `;
4. the developer's own words for messages the draft did not list, and a local merge failure reads `stopped`;
5. `commit-tree` takes `--no-gpg-sign` and signs nothing;
6. the `--git-path rebase-merge` probe is a read only.

As amended in iteration 2, the deadline sends SIGTERM through the shell's built-in `kill`, since the store links no libc. Two worktrees run one sync per common git directory. Redaction turns a token holding the remote's own URL into the remote's name, and any other URL token into `<url>`.

### Files in scope

| Module | What it does |
|---|---|
| store `sync.rs` (new) | `SyncGit`: the look, `still_ready`, fetch, `relation`, `merge_tree`, the tree's blobs, the merge commit, push and its `--porcelain` parse; `Upstream::redacted` |
| store `sparse.rs` (new), `worktree.rs`, `lib.rs` | `sparse_set`; `until` (SIGTERM, 2 s grace), `GIT_NO_LAZY_FETCH=1` on every child, `STDERR_MAX` public |
| CLI `sync.rs` (new), `sparse.rs` (new), `outputs.rs`, `confirm.rs`, `lib.rs`, `main.rs` | `sync_once`, `hub_sync`, `SyncState`, `sync_line`; `spec sparse-set`; `render_tree`; the applied signal |
| http `sync.rs` (new), `main.rs`, `app.rs`, `confirm.rs` | `Syncs`, `RepoSync` (timer, backoff, signals, turns); `push_of`; the route; the signal |
| ui `app/SyncLine.tsx` (new), `api/{client,http,provisional,queries}.ts`, `inbox/ConfirmDialog.tsx`, `inbox/decisions.ts`, `app/Shell.tsx`, mocks | the line, `useSync`, the dialog's strings and its state after a 200 |
| Tests | http `hub_git.rs`, `hub.rs`, `daemon_keys.rs`, `hub_confirm.rs`, `common/mod.rs`; store `sync_urls.rs`; CLI `sparse_set.rs`; UI `SyncLine.test.tsx`, `sync.http.test.ts`, `ConfirmDialog.test.tsx`; `fixtures/daemon-keys.json` |
