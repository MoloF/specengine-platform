---
class: spec
status: draft
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
ref: RM-0051; Phase 7 step 10b (08 s2); the hub-git analysis, the owner's decision of 2026-11-03 (Q1) and the orchestrator's decisions of 2026-11-03
---

# hub-git: fetch, merge and push, never rebase, with a push status on screen (RM-0051)

## Why

Since `hub-confirm`, a confirm commits in the hub's own checkout, and nothing fetches, merges or pushes that commit (`docs/canon/hub-confirm.md` "Rules", **Where it ends**). So the commit lives on one host, outside RM-0064's dumps, and leads confirm against a brief that never sees a developer's commit. A hand push invites `git pull --rebase` or a force push, which rewrites SHAs the queue stores: `applied_commit`, a create's `Base-commit:`, completion's `base_commit..refs/heads/<branch>` (`docs/canon/proposal-completion.md` "Completion"). Since `apply-outputs` (RM-0072, 10a), a hub commit carries the index and a hub-range `PR-`, so it is safe to push. This slice adds the first network verb.

Decided, 2026-11-03.
- **Q1** (the owner): the hub pushes to the game's main branch, the one its checkout tracks. A merge commit appears only on divergence, and a linear-history protection there is lifted for the hub (RM-0064).
- **The orchestrator:** D1, D2, D3, D6 and Q4, as "Rules" states them.
- **In scope:** A12, an index-only conflict rendered again from the merged tree, and the screen line with 10a's gap warning.

## Description and interactions

### The sync

A hub with `--push` keeps one sync per git repository it serves, keyed by the worktrees' common directory: two roots in one checkout share one sync. The sync carries the checkout's branch `<b>` to its upstream, `<r>` (`branch.<b>.remote`) and `refs/heads/<m>` (`branch.<b>.merge`). Only the merge phase takes turns: those of every served project in that worktree, in `--root` order. So a slow remote never delays a read or a confirm (`docs/canon/serve.md` "Concurrency").

1. **Look** (local, read-only): `HEAD` on a branch, an upstream on a named remote, a clone neither shallow nor partial, `<r>`'s URLs free of secrets ("Rules"), no operation in progress (store `operation_in_progress`), `git status --porcelain=v1 -z --untracked-files=all` empty, and every sparse-set path `HEAD` holds present. Else `stopped`, nothing touched.
2. **Fetch** (network). The upstream branch alone, into `refs/remotes/<r>/<m>`, a forced update allowed, so a remote force push shows.
3. **Merge** (local, in the turns). `L` is `HEAD` read again; `R` is the tracking ref.
   - `R` is `L` or an ancestor of `L`: nothing.
   - `L` is an ancestor of `R`: a fast-forward.
   - No merge base: `held`.
   - Otherwise `git merge-tree --write-tree` decides in memory. A clean merge, or one whose every conflict is an output of a served root's `index = true` entry (by the merged, unconflicted config), becomes a merge commit, made by `git commit-tree` and reached by `git merge --ff-only --no-autostash`. Any other conflict is `held`, the paths named, nothing touched.

   No `MERGE_HEAD` ever exists, so a crash leaves none.
4. **Push** (network). `refs/heads/<b>:refs/heads/<m>` to `<r>`, with `--porcelain`:
   - accepted: `ok`;
   - not a fast-forward: phases 2-4 again, at most three rounds, then `retrying`;
   - refused by the remote (a hook, a protection): `held`;
   - anything else (network, authentication, the deadline, `pre-push`): `retrying`.

Pushed is never read from the push's exit code, nor stored. `ahead` and `behind` are computed from git on each status read. A push that landed but whose answer was lost reads `ahead` > 0 until the next fetch, which finds `R` = `L` and merges nothing.

**When.** A sync runs at start once listening; after a confirm that committed, which `confirm::write` signals on a 200 whose document is `applied`, never awaiting it; and every `--push-every` minutes. One sync runs at a time per repository, and signals coalesce into one more run.

**Retries.** `retrying` doubles the timer's wait per consecutive failure, up to eight intervals; a signal runs at once. `held` keeps its `(L, R)`: later syncs fetch, and merge or push only once a tip moved. `stopped` looks again on every tick and signal.

Host commits travel too, RM-0065's `[team]` rows included; remote `[team]` commits arrive through the merge.

### Merge

**The merge commit:**
- **Parents:** `L` then `R`, so `applied_commit` stays the first parent and keeps its SHA.
- **Message:** git's default, `Merge remote-tracking branch '<r>/<m>'` and LF, with no trailer.
- **Identity:** the checkout's configured one, or the process's `GIT_AUTHOR_*` and `GIT_COMMITTER_*`, never a login's (`identified()` in CLI `confirm.rs` stays inside a confirm). `-c user.useConfigOnly=true` makes a guessed identity fail, and the sync is `stopped`.

**Outputs.** Every merge commit the sync makes renders each served root's `index = true` outputs from the merged tree, with 10a's committed-tree render (store `outputs.rs` over a given tree, core `render_index_set`). A conflicted output is read at `L`'s bytes, and a changed render replaces the merged blob, so the commit equals its own `spec export index --stdout`. On a walk gap, a clean merge keeps git's outputs; an index-only conflict is `held`. No project command runs (ADR-0013).

**A merged `specengine.toml`.** When a merge changes one, the sparse set is recomputed. A new path the worktree lacks makes the next sync `stopped` until the cone is widened. A config this binary refuses (older binaries refuse `[team]`: `hub-team.md` "Team") makes reads and confirms answer 503 until the hub is upgraded; the sync never judges a merged file.

### Status and the screen

`GET /api/projects/:p/sync` is mounted on a hub with `--push` alone. It takes no turn and no query, and answers for `:p`'s repository ("Data"). A local daemon, or a hub without `--push`, answers the unknown-route 404. Any login reads it (`hub.md` "Routes"). The confirm's response keeps its shape.

**The line.** The page asks once a login is known (`useHubLogin`), on window focus and every 30 s.
- `ahead` > 0: `1 commit not pushed` or `<N> commits not pushed`.
- `error` set: `Sync: <error>` beneath, as text, escaped as `shownConfirmText`.
- Otherwise, and on a 404, a failed read or a local page: nothing.

The line sits in the shell beside the project's view, with no control.

**The confirm dialog after a 200.** When the POST's document holds notes (10a's gap warning, a reject's follow-up), the dialog stays open: `Confirmed.`, the notes as text, Confirm gone, focus on Close. Close hands the document on, as `onConfirmed` does today. With no notes, it hands it on at once; the UI developer verifies that a clean confirm's document carries none.

### The sparse set

`spec sparse-set`, read-only, prints the paths a hub's sparse checkout must hold, from the repository's top. They come from the root's `specengine.toml` at `HEAD`:
- `specengine.toml`, `spec.lock` and `.spec-debt.toml`;
- each `[paths] roots` entry;
- `[decision_records] dir` and `template`;
- the `index = true` entry's `writes`.

Paths come in config order. A directory ends in `/`, and a path inside a listed one is dropped. A path `HEAD` holds that the worktree lacks is also a warning. RM-0064's clone script reads the list; the look checks the same set.

## Data

**Flags** (http `main.rs`), judged with `--public-origin` before anything is created:

```text
specengine-http --root DIR... [--port N] [--public-origin URL [--push [--push-every MINUTES] [--push-deadline SECONDS]]]
```

| Flag | Values, default | Refused, exit 2 |
|---|---|---|
| `--push` | off | without `--public-origin`: `specengine-http: --push needs --public-origin: only a hub pushes` |
| `--push-every` | 1-1440 minutes, 5 | without `--push`, or out of range: naming the flag |
| `--push-deadline` | 1-3600 seconds, 120 | the same |

The deadline bounds every git child of a sync (`WorktreeGit::new_until`). A hub with `--push` prints `push every <N> min` after `public <origin>`, before `listening`.

**Status document** (daemon-keys set 110 `HubSync`; all five keys always present):

```json
{"ahead":2,"behind":0,"state":"held","error":"the remote refused `main`: pre-receive hook declined\nremote: frozen","at":"2026-11-04T09:00:00Z"}
```

- **`ahead`, `behind`:** computed per request by `git rev-list --left-right --count refs/remotes/<r>/<m>...HEAD` (`GIT_OPTIONAL_LOCKS=0`); null with no tracking ref.
- **`state`, `error`, `at`:** daemon memory only. `at` is the last sync's end, in RFC 3339 UTC. After a start: `none`, null, null.

| `state` | The last sync | Next |
|---|---|---|
| `none` | none ended since start | |
| `ok` | the remote branch holds `HEAD` | timer, signal |
| `retrying` | fetch or push failed, no ref refused: network, authentication, deadline, `pre-push`, three races | timer with backoff; a signal at once |
| `held` | a conflict, unrelated histories, a remote's refusal, an index-only conflict with a walk gap | once `L` or `R` moved |
| `stopped` | the look failed, or a merge lacked an identity | every tick and signal |

**`error`** is the daemon's words, then git's stderr where git spoke, cut at 4096 bytes (`STDERR_MAX`, `worktree.rs:75`). Every control but LF, and every Cf, is written as `\u{XX}`, as the confirm's question is (`hub-confirm.md` "Rules", **Escaping**). A URL is never shown.

```text
`main` has no upstream on a remote: set one with git branch --set-upstream-to
a shallow clone: a sync needs the whole history | a partial clone: a sync needs every blob
the remote `origin` has a URL holding a user name or password: refused before any connection
a <operation> is in progress in <worktree>: finish or abort it first
<n> paths are not clean in <worktree>: <first 10>[ and <k> more]
outside this checkout: <path>, ...: widen the sparse checkout (spec sparse-set)
a merge commit needs an identity: set user.name and user.email in <worktree>
`origin/main` shares no history with `main`: nothing merged
merging `origin/main` conflicts in <path>, ...: resolve it on the host's terminal in <worktree>
the index's outputs conflict and cannot be rendered: <gap>
fetch from `origin` failed: <stderr> | push to `origin` failed: <stderr>
the remote refused `main`: <reason>; <stderr>
`origin/main` moved during three rounds: trying again later | the <fetch|push> passed its deadline of <N> s
```

`<worktree>` is the checkout's top, which every login already reads (`hub.md` "Trust"). Stderr gets one line per change: `specengine-http: sync <worktree>: <state>[: <error>]`.

**Git children** run as `git -C <worktree>` under `WorktreeGit`'s environment: cleared, the caller's variables, `GIT_TERMINAL_PROMPT=0`, stdin null, the deadline.
- **Fetch:** `fetch --no-tags --no-recurse-submodules --no-auto-maintenance <r> +refs/heads/<m>:refs/remotes/<r>/<m>`.
- **Merge:** `merge-base [--is-ancestor]`, `merge-tree --write-tree -z --name-only L R`, `hash-object -w`, `mktree`, `-c user.useConfigOnly=true commit-tree <tree> -p L -p R -F -`, `merge --ff-only --no-autostash <commit>`.
- **Push:** `push --porcelain --no-follow-tags --recurse-submodules=no <r> refs/heads/<b>:refs/heads/<m>`.
- **Look:** reads only, URLs by `remote get-url --all` and `--push --all`, after `insteadOf`.

Every child but `fetch` and `push` sets `GIT_NO_LAZY_FETCH=1`, and from this slice so does every `WorktreeGit` child (`command`, `local_env_vars`). The read side (`git.rs` `CHILD_VARIABLES`) and 10a's render already do. `merge-tree --write-tree` needs git 2.38 or later, which RM-0064's image must provide.

**`spec sparse-set`**, run at the repository's top:

```text
specengine.toml
spec.lock
.spec-debt.toml
docs/
templates/decision.md
warning: templates/decision.md is in HEAD but not in this checkout: widen the sparse checkout
```

`--json` gives `{"paths": [...], "missing": [...]}`. Exit 0 whatever is missing (ADR-0012). Exit 2 with no repository, no commit or an invalid config at `HEAD`, in `spec export index`'s words.

**Bumps:** `GET_ROUTES` from 25 to 26, daemon-keys set 110, three flags, a start line and one CLI command. No queue schema, index format, `spec.lock`, event, `daemon.json`, MCP or plugin change; nothing is stored.

## Rules and edge cases

- **D1.** WHEN `specengine-http` runs without `--push` THEN no git child SHALL fetch, merge or push, and a hub SHALL behave as 9b plus 10a, byte for byte. `spec serve` passes only `--root` and `--port` (`serve.md` "Launcher"). No CLI command, MCP tool or hook reaches the network. `fetch` and `push` SHALL be spawned from store `sync.rs` alone. ADR-0058 holds: `--public-origin` alone makes a hub.
- **The confirm is unchanged.** It never waits on a sync, never fails because of one, and is never refused because the hub is behind, diverged, `held` or `stopped`. WHEN the operator is mid-merge THEN a confirm answers apply step 2's 503, as today (`proposal-apply.md` "Apply steps"). `applied` still means "committed on this hub". The sync's fast-forward or clean merge brings in others' commits, spec files and `spec.lock` included, through git. It authors no text and is no apply (`architecture.md#apply`). In a merge commit, the rendered outputs are SpecEngine's only bytes.
- **D2.** No `git pull`, whose `pull.rebase` or `branch.<b>.rebase` can rebase. No rebase, force, `reset`, `stash`, `clean`, `checkout`, `--allow-unrelated-histories`, or `-X` or other strategy option; the merge's flags override `merge.ff=only` and `merge.autoStash`.
  - WHEN a merge would conflict outside the outputs THEN nothing SHALL be written to the worktree, index or refs.
  - WHEN the remote was force-pushed past the hub's commits THEN the next sync SHALL merge them back and push.
  - A non-fast-forward is never retried with `--force` or `--force-with-lease`.
- **D3, the premise.** The merge stands on the hub as the brief zone's sole writer, a convention nothing enforces. A terminal's apply there is never refused; a conflict it causes shows as `held`, and the project's CI may enforce more.
- **D6.** No `src` file reads, stores, logs, passes or names a credential: no string literal `GIT_SSH_COMMAND`, `core.sshCommand`, `credential.` setting or key path. A prompt fails at once (`GIT_TERMINAL_PROMPT=0`, stdin null); ssh `BatchMode` is the deployment's.
  - WHEN a URL of `<r>`, after `insteadOf`, holds a password (`user:pass@`, any scheme) or any user name on `http`, `https`, `ftp` or `ftps` THEN the look SHALL stop before any fetch or push, and no answer, log or status SHALL hold that URL.
  - An ssh user name (`ssh://git@host/x`, `git@host:x`) is the deploy key's form, and passes.
  - The key is a write deploy key for this one repository: a Docker secret mounted only into `specengine`, wired outside every `--root`, with a pinned `known_hosts` (RM-0064).
- **A12.** WHEN every conflict is an output of a served root's `index = true` entry, read from the merged tree's config, THEN the merge commit SHALL carry those outputs rendered from the merged tree. A conflicted `specengine.toml` makes every conflict a plain one, and a walk gap leaves the sync `held`.
- **Q4.** On `held`, the operator runs `git merge <r>/<m>` on the host's terminal, resolves and commits; the next sync pushes. Meanwhile confirms commit, and the page shows the count.

**Risks.**
- **A refusing remote** (`pre-receive`, a forge protection) is shown escaped and retried once a tip moves.
- **Hooks:** `pre-push` runs, never `--no-verify`. `commit-tree` runs no hook, so `pre-merge-commit` never sees a sync's merge; `post-merge` runs on the fast-forward.
- **ADR-0012, ADR-0039:** no status, check, health row, verify, gate, hook verdict or read derives anything from push state. A red CI run on the remote stops nothing, and no developer's work is removed.
- **Security:** a forged `Remote-User` now reaches the shared remote. "No real use before RM-0064's Done" holds (`hub-confirm.md` "Known limits"), so `--push` stays off until then. The forge scopes the deploy key's branches.
- **ADR-0008:** remote and branch come from the upstream, the sparse set from the config; there is no `[hub]` table (`hub.md` "Entry").
- **Partial clones:** an apply on a partial clone missing a blob now fails instead of fetching it.

**Known limits.** Merged-in history: completion's trailer search now walks other queues' `spec: apply` commits. Their IDs below the hub's floor never meet a hub ID, but a dump-imported orphan below the floor can meet a developer's same-ID commit in the merged branch, which blocks its reject (`proposal-completion.md` "Known limits"); the cure is to raise it again.

**Tests.** Remotes are bare repositories in the scratch directory, never a network URL. Product processes get `GIT_ALLOW_PROTOCOL=file`, so `ssh://` fails before DNS. Global and system git config are off (`tests/common/mod.rs` `product_env`), under a scratch `HOME` (`Scratch::home`). Bare remotes take `QUIET` plus `receive.autogc=false`. Hooks sleep at most 30 s. No pilot repository is a remote or a checkout; pilot-shaped configs are synthetic. Timing margins are generous.

## Acceptance criteria

Unless one says otherwise, each criterion runs `specengine-http --public-origin https://hub.example.org --push --push-deadline 3`, over a checkout cloned from a bare repository in the scratch directory. It waits for `GET .../sync`'s `at` to change, then asserts with git. M marks the mutation that must turn it red.

- [ ] P1 — The hub only ahead: after a confirm's sync, the remote `main` equals `applied_commit`, `ahead` is 0 and the state `ok`. A commit made by hand on the host goes with the next sync. M: no push.
- [ ] P2 — Diverged on an unrelated path: the remote tip has two parents, first `applied_commit`, unchanged in the queue, then the developer's commit. M: `git pull --rebase`.
- [ ] P3 — P2 with `pull.rebase`, `branch.main.rebase` and `merge.autoStash` set to `true` and `merge.ff=only`: the same result. M: the sync as `git pull`.
- [ ] P4 — A conflict on one span: `held`, the path named. No operation is in progress, `git status --porcelain` is empty, and `HEAD` and the remote are unchanged. The next confirm answers 200. M: leave the merge in progress; `-X ours`.
- [ ] P5 — Two creates, the hub's and a developer's, conflict only in `docs/index.md`. The remote tip is a merge commit holding both documents. Its `docs/index.md` equals `spec export index --stdout` of a clean clone, and `spec check` finds no `index-drift`. M: abort on any conflict; keep git's conflicted bytes.
- [ ] P6 — A `remote.origin.receivepack` wrapper lands a developer's commit first, then execs `git-receive-pack`. Afterwards both commits are reachable from the remote tip, and no push carries `--force*`. M: no retry after a non-fast-forward; a retry with `--force`.
- [ ] P7 — A `pre-receive` prints `frozen` and U+202E, then exits 1: `held`, and `error` holds `\u{202e}`, never the raw character. `HEAD` keeps the commit; reads and confirms answer 200. M: reset the branch to the remote.
- [ ] P8 — The wrapper exits 1 after `git-receive-pack` succeeded: that sync is `retrying`. The next is `ok`, with `ahead` 0 and no merge commit. M: status from the push's exit code.
- [ ] P9 — `pre-receive` sleeps 30 s: the push is killed at 3 s, `retrying`, the deadline named. During the hang `GET .../tree` answers within 1 s, and a confirm commits. M: push inside the turn; no deadline.
- [ ] P10 — The remote replaced by an orphan history: `held`, the remote unchanged. M: `--allow-unrelated-histories`.
- [ ] P11 — The remote force-pushed back to the hub commit's parent: the next sync makes `applied_commit` reachable from the remote tip again. M: reset the branch to the remote.
- [ ] P12 — The checkout's `pre-push` exits 1: the remote is unchanged, `retrying`, the hook's words shown. M: `--no-verify`.
- [ ] P13 — A modified tracked file and an untracked file: `stopped`, both named. Both stay unchanged, bytes and mtime, and so does the remote. M: `stash`, `reset --hard` or `clean`.
- [ ] P14 — With logging `remote.origin.uploadpack` and `receivepack` wrappers and a commit to push, neither wrapper runs for any of:
  - a local daemon;
  - a hub without `--push`, its confirm included;
  - `spec serve`;
  - terminal `spec approve` and `spec reject`.

  `--push` without `--public-origin` exits 2 with "Data"'s line and creates no data directory. M: sync on every daemon; push from `apply.rs`.
- [ ] P15 — Pins.
  - **Source,** the string literals of every `crates/*/src/**/*.rs`, comments not read: `"fetch"` and `"push"` only in store `sync.rs`; `GIT_SSH_COMMAND`, `core.sshCommand`, `credential.` and `/run/secrets` in none.
  - **Argv,** a PATH wrapper logging every git child of P1-P13 with its `GIT_NO_LAZY_FETCH`:
    - no `pull`, `rebase`, `reset`, `stash`, `clean`, `checkout`, `--allow-unrelated-histories`, `-X` or `--strategy-option`;
    - no push with `--force*`, a `+` refspec, `--delete`, `--mirror`, `--all`, `--tags` or `--no-verify`;
    - `GIT_NO_LAZY_FETCH=1` on every child but `fetch` and `push`, terminal applies' included.

  M: add `--force`; add `GIT_SSH_COMMAND`; drop the variable from `WorktreeGit::command`.
- [ ] P16 — No `crates/*/Cargo.toml` and not the root one names `git2`, `gix`, `reqwest`, `ureq`, `ssh2` or `russh`; read as `b9_the_mcp_crate_names_no_http_crate` reads its manifest (`tests/hub_confirm.rs`). M: add `gix`.
- [ ] P17 — Three `origin` URLs: `https://u:s3cret@example.invalid/r.git`, `https://tok@example.invalid/r.git`, and a clean name `url.<base>.insteadOf` rewrites to the first. Each is `stopped`, with no fetch logged and neither `s3cret` nor `tok` in any body, stderr or log. `ssh://git@example.invalid/r.git` passes the look. M: no check; a check of `remote.origin.url` alone.
- [ ] P18 — `GET .../sync` answers 200 with exactly `ahead`, `behind`, `state`, `error`, `at` on a hub with `--push`, and 404 on a local daemon and on a hub without it. `the_get_routes_are_pinned` lists 26 routes, and daemon-keys set 110 `HubSync` pins the keys. M: mount it locally; mount it on every hub.
- [ ] P19 — P2's merge commit has no SpecEngine trailer. Its message is `Merge remote-tracking branch 'origin/main'`, and its author and committer the checkout's `user.name` and `user.email`, never the confirming login. With neither set: `stopped`, nothing merged. M: reuse `identified()`'s login.
- [ ] P20 — After every outcome of P1-P12 and P17, `git status --porcelain=v1 --untracked-files=all` is empty and no operation is in progress. M: leave a scratch file in the worktree.
- [ ] P21 — A synthetic config: `roots = ["docs", "design", "README.md"]`, `[decision_records]` `dir = "records"` and `template = "templates/decision.md"`, an index writing `docs/index.md`.
  - `spec sparse-set` prints them in "Data"'s order.
  - With `design/` outside a sparse checkout, a warning and `--json`'s `missing` name it.
  - On a hub, that gap leaves the sync `stopped`, also when a merge brings it.

  M: hardcode `docs/`.
- [ ] P22 — Under the product environment, `git fetch ssh://example.invalid/x` fails with git's transport refusal, never a resolver error. M: remove `GIT_ALLOW_PROTOCOL`.
- [ ] P23 — Byte-identical with the hub two commits ahead and with the remote moved away: `spec check --json`, `spec health --json`, `spec gate` on a fixed payload, `GET .../tree`, `.../inbox` and `.../check`. M: a note or row derived from push state.
- [ ] U1 — UI, jsdom, mocked client:
  - `ahead` 2 shows `2 commits not pushed`.
  - `error` `<b>x</b>` plus U+202E shows `Sync: ` and that text, escaped, with no `b` element.
  - `ahead` 0 with no `error`, a 404, a failed read and a local page show no line.
  - Window focus reads again.
  - In the confirm dialog, a 200 whose `notes` hold 10a's gap warning keeps the dialog open, showing `Confirmed.` and the warning as text, with focus on Close; `onConfirmed` fires once, on Close. With no notes it fires at once.

  M: the error as markup; the line on a local page; closing whatever the notes.

## Out of scope

The board snapshot (RM-0052, step 10c); `/mcp` (RM-0053); `deploy/`, the clone script, the guide, the secret, forge rules and the image's git (RM-0064); `spec admin` (RM-0065); tags (RM-0056); `pushRemote` or a second branch; spec-ID collisions; a conflict-resolution UI; signed commits; a `spec check` before a push, which would be a gate; any route or terminal command that runs a sync; blob-filtered and shallow clones, which are refused.

## Implementation

To fill in after implementation.

### Files in scope

| Module | What it changes |
|---|---|
| store, new `sync.rs` | the only `fetch` and `push`: look, URL check, `merge-tree`, outputs' tree, `commit-tree`, fast-forward, `--porcelain` parse, ahead and behind |
| store, new `sparse.rs` | the sparse set from `HEAD`'s config, and what the worktree lacks |
| store `worktree.rs`, `outputs.rs` (10a), `lib.rs` | `GIT_NO_LAZY_FETCH=1` on `command` and `local_env_vars`; the render over a given tree |
| CLI, new `sync.rs` | a sync in phases, its outcome and status document; the merge phase as a closure run inside the turns |
| CLI, new `sparse.rs`, `lib.rs` | `spec sparse-set` |
| http `main.rs`, `start.rs`, new `sync.rs` | the flags and start line; one task per repository: timer, backoff, signals, turns in `--root` order, memory state, the stderr line |
| http `app.rs`, `confirm.rs` | the route with `--push`; the signal after an applied 200 |
| ui `api/`, `mocks/`, `app/Shell.tsx`, new `app/SyncLine.tsx`, `inbox/ConfirmDialog.tsx` | `getSync`, `HubSync`, `useSync`; the line; the dialog after a 200 |

**Tests:** http new `tests/hub_git.rs` (P1-P20, P22, P23), `tests/common/mod.rs`, `tests/hub.rs`, `tests/daemon_keys.rs` and `fixtures/daemon-keys.json`; CLI new `tests/sparse_set.rs` (P21); UI `app/SyncLine.test.tsx`, `inbox/ConfirmDialog.test.tsx` and `api/`'s.

### Canon at ship

Nothing below is written before this slice's commit. Free bytes are as of 2026-11-03, before 10a; a page without room rewords in place.

- **New `docs/canon/hub-git.md`** (Tier 2, `<a id="sync">`; `docs/index-canon.md` has 743 B free) takes this spec's truth. The pages below point at it.
- **`architecture.md`** (1 B free):
  - `#apply`: after "one click per item", add "; a hub's `--push` sync merges others' commits, `spec.lock` too, never rebasing and authoring neither (`hub-git.md`)", and add ADR `hub-push` to its list (D1, D2).
  - The generated paragraph, after 10a: "in an apply's commit" becomes "in a commit it makes" (A12, ADR `hub-push`); 10a's ADR keeps "an apply's commit".
  - `#distribution`: "(`hub.md`). ADR-0058." becomes "(`hub.md`), only its `--push` online, by git and the deployment's credentials. ADR-0058, ADR `hub-push`." (D6).
  - Room, never a raised cap: repeated passages move out to the canon pages they repeat, which the sentence points at:
    - `#control`'s "; a code-layer failure leaves ... never `ok`" (`architecture-code.md#sync-in-reads`);
    - "both from the code index the call refreshes, as `spec refs` reads it" (`architecture-code.md` rows 4-5);
    - `#storage`'s macOS-only path, wrong on a Linux hub (the CLI README "Database").

    These three free 215 B. With 10a's line (+97 B) about 94 B more are needed, found the same way.
- **`hub.md`** (24 B): the intro, "only by a confirm (`hub-confirm.md`) or a sync's merge (`hub-git.md`)"; "Trust", "no account, token, key or session (git's: `hub-git.md`)". For room, "Tests" drops "`tests/hub_{team,confirm}.rs`: `hub-team.md`, `hub-confirm.md`.", leaving 27 B after 10a.
- **`hub-confirm.md`:**
  - The intro: "a `--push` sync carries the branch (`hub-git.md`)".
  - **Where it ends**: "`applied` means committed on this hub: a `--push` sync pushes it, never awaited by a confirm nor refusing one (`hub-git.md`); without `--push` the operator pushes by hand".
  - "Screen": the dialog after a 200.
- **`hub-team.md`** (52 B) "Confirm": "pushed by a `--push` sync (`hub-git.md`)".
- **`serve.md`:** "Lines", the start line and the exit 2 lines; "Concurrency", the merge takes its repository's turns, the network none.
- **`proposal-outputs.md`** (10a): a sync's merge renders outputs. **`proposal-completion.md`** "Known limits": merged-in trailers.
- **Tier 1:** http README (23 B), `--push` and the route; store (5 B), `sync`, `sparse`; CLI (7 B), `sparse-set`; `ui/README.md` "Screen rules", the line and the dialog.
- **`CLAUDE.md`:** `hub-git` shipped; ADRs up to `hub-push`; "The hub goes on from step 10c, RM-0052"; the apply rule adds "a hub's merge of others' commits authors none".
- **Specs:**
  - 07's hub bullet: "pushed by its sync with `--push`".
  - 05's "One writing door": the same; its Git row drops "later `gix`" (P16).
  - 08: 10b shipped. RM-0051 shipped.
  - RM-0064 "Documents": the clone of what `spec sparse-set` prints; `--push` only after "Done"; git 2.38 or later.
  - This file compressed.
- **Unchanged:** ADR-0012, ADR-0034, ADR-0039, ADR-0058, ADR-0059, ADR-0061; `proposal-apply.md`; `hub.md` "Entry".

The draft ADR takes the next free number after 10a's drafts, `apply-outputs` and `hub-ids`. It is 1 455 B with its front-matter; the date is set at ship.

```markdown
---
class: decision
id: ADR-0064
title: A hub with --push fetches, merges and pushes its branch, never rebasing; credentials are git's
status: accepted
date: 2026-11-DD
scope: [architecture]
canon: docs/canon/architecture.md#apply
ref: hub-git (RM-0051); the owner's decision of 2026-11-03 (Q1); the orchestrator's decisions of 2026-11-03
---

# ADR-0064

**Decision.** Narrows ADR-0061's "nothing pushed"; supplements ADR-0034 and ADR-0059. Only `specengine-http --public-origin --push` runs git's `fetch` and `push`, in a sync apart from any confirm. No SpecEngine process rewrites a commit it recorded: a fast-forward, else a merge commit under the checkout's identity, no trailer, carrying the index's outputs rendered from the merged tree; a conflict outside them touches nothing. Merging others' commits authors no text. Its premise, the hub as the brief zone's sole writer, is a convention, unenforced. Credentials are git's, set by the deployment; no `src` names one; a URL holding one is refused. Push state is computed from git, read by no status, check or gate (ADR-0012, ADR-0039).

**Why.** A confirm's commit lived on one host, and a rebase or force push would rewrite the SHAs the queue stores.

**Cost.** Merge commits on the main branch, whose linear-history rule is lifted for the hub. A conflict waits for the operator's terminal. A forged `Remote-User` now reaches the shared remote, so `--push` stays off until RM-0064's "Done".
```
