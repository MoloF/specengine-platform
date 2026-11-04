---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-04
---

# Hub git: a `--push` hub fetches, merges and pushes, never rebasing

ADR-0064, built by `hub-git` (RM-0051). A hub started with `--push` (`hub.md`) carries its checkout's branch to its upstream and back. Nothing else touches the network: a local daemon, `spec serve`, the CLI, MCP and hooks stay offline. A confirm commits and answers on its own (`hub-confirm.md`); the next sync carries it.

Code: store `sync.rs` `SyncGit` (the only `fetch` and `push`), `sparse.rs`, `worktree.rs`; CLI `sync.rs` `sync_once`, `hub_sync`, `sparse.rs`, `outputs.rs` `render_tree`; http `sync.rs` `Syncs`, `main.rs` `push_of`, `app.rs`, `confirm.rs`; UI `src/app/SyncLine.tsx`, `api/queries.ts` `useSync`, `inbox/ConfirmDialog.tsx`.

## Flags

`--public-origin URL --push [--push-every MINUTES] [--push-deadline SECONDS]` is judged before anything is created; a refusal exits 2: `--push needs --public-origin: only a hub pushes`, `--push-every needs --push`, `--push-every takes 1 to 1440 minutes, not <n>` (default 5), `--push-deadline` alike, 1 to 3600 seconds (default 120). A `--push` hub prints `push every <N> min` after `public <origin>`.

<a id="sync"></a>
## Sync

There is one sync per git common directory of the served roots. It runs in the first served root's worktree (`<top>`) and carries its branch `<b>` to the upstream: `<r>` from `branch.<b>.remote`, `refs/heads/<m>` from `branch.<b>.merge`. A project served from another worktree of that repository reads `stopped`, `this worktree is not synced: the sync runs in <top>`, and its roots stay out of the look. Only the merge phase takes turns: those of every served project in that worktree, in `--root` order.

1. **Look**, read-only, outside the turns: `HEAD` on a branch with a commit, an upstream on a named remote, a clone neither shallow nor partial (`extensions.partialClone`, a promisor), `<r>`'s fetch and push URLs after `insteadOf` free of a secret ("Credentials"), every sparse-set path of the synced roots that `HEAD` holds present; else `stopped`, nothing touched. An operation in progress or an unclean `git status --porcelain=v1 -z --untracked-files=all` is provisional: the sync still fetches.
2. **Fetch.** `fetch --no-tags --no-recurse-submodules --no-auto-maintenance <r> +refs/heads/<m>:refs/remotes/<r>/<m>`, forced, so a remote force push shows.
3. **Merge** (inside the turns). `still_ready` decides cleanliness and operations now, so a confirm mid-apply never makes a sync `stopped`; `HEAD` off the branch makes it `stopped`. `L` = `HEAD`, `R` = the tracking ref. `R` is `L` or behind it: nothing. `L` behind `R`: `merge --ff-only --no-autostash R`. No merge base: `held`. Else `merge-tree --write-tree -z --name-only L R` merges in memory: clean, or every conflict an output of a served root's `index = true` entry (by the merged, unconflicted config), it is a merge commit; any other conflict is `held`, paths named, nothing written. A moved `HEAD` then runs each served root's `update_paths` over `SyncGit::changed` (`ChangedPaths`; `spec-read-freshness.md`).
4. **Push**, unless `L` = `R`: `push --porcelain --no-follow-tags --recurse-submodules=no <r> refs/heads/<b>:refs/heads/<m>`. Accepted: `ok`. Not a fast-forward: 2-4 again, three rounds, then `retrying`. `[remote rejected]`: `held`. Else (network, authentication, the deadline, `pre-push`): `retrying`.

No `MERGE_HEAD` ever exists. "Pushed" is never the push's exit code, nor stored. A push that landed with its answer lost reads `ahead` > 0 until the next fetch finds `R` = `L`, and no merge commit follows.

**When.** At start, once listening; on a signal from `confirm::write` after a 200 whose document is `applied` (never awaited); and every `--push-every` minutes. One sync at a time per repository; signals coalesce into one more run.

**Retries.** `retrying` doubles the timer's wait per consecutive failure, up to eight intervals; a signal runs at once. `held` keeps `(L, R)`: later syncs fetch, but merge or push only once a tip moved. `stopped` looks again each tick and signal.

**The deadline** bounds every git child of a sync, local ones too: a hanging `post-merge` hook, smudge filter or gpg wait would otherwise hold every turn. The exception is the render inside a merge, which reads local objects only. At the deadline the child gets SIGTERM (the shell's built-in `kill`, since the store links no libc), then SIGKILL after 2 s, so git removes its own locks. The next look finds anything left as `stopped`. A killed fetch or push is `retrying`.

Every child but `fetch` and `push` sets `GIT_NO_LAZY_FETCH=1`, as every `WorktreeGit` child does anywhere.

## Merge commit

`-c user.useConfigOnly=true commit-tree --no-gpg-sign <tree> -p L -p R`, reached by `merge --ff-only --no-autostash`: message `Merge remote-tracking branch '<r>/<m>'` and LF, no trailer, never signed; first parent `L`, so `applied_commit` keeps its SHA. Identity: the checkout's, or the process's `GIT_AUTHOR_*`/`GIT_COMMITTER_*`, never a login's (`identified()` stays inside a confirm); a guessed one fails: `stopped`. The tree carries each served root's `index = true` outputs rendered from the merged tree (CLI `render_tree`, 10a's render: `proposal-outputs.md`), a conflicted output read at `L`'s bytes, so the commit equals its own `spec export index --stdout`; on a walk gap a clean merge keeps git's outputs and an index-only conflict is `held`. `commit-tree` runs no hook; `post-merge` runs on the fast-forward. A merge changing a served `specengine.toml` reads the sparse set again.

The sync brings in others' commits through git, spec files and `spec.lock` among them. It authors no text, so it is no apply (`architecture.md#apply`). It never runs `git pull`, rebase, force, `reset`, `stash`, `clean`, a commit `checkout`, `--allow-unrelated-histories` or a strategy option. Its flags override `merge.ff=only` and `merge.autoStash`. A remote force-pushed past the hub's commits gets them back.

## Status

`GET /api/projects/:p/sync` exists on a `--push` hub alone; elsewhere it is the unknown-route 404. It takes no turn and no query, and any login reads it (`GET_ROUTES` 26; `HubSync`, set 110 of `fixtures/daemon-keys.json`):

```json
{"ahead":2,"behind":0,"state":"held","error":"the remote refused `main`: pre-receive hook declined; remote: frozen","at":"2026-11-04T09:00:00Z"}
```

`ahead`, `behind`: `rev-list --left-right --count` against the tracking ref per request, null without one; `state`, `error`, `at` (the last end, UTC): daemon memory, `none`, null, null at start; a change prints `specengine-http: sync <worktree>: <state>[: <error>]`.

| `state` | The last sync | Next |
|---|---|---|
| `none` | none ended | |
| `ok` | the remote holds `HEAD`, or `L` = `R` | timer, signal |
| `retrying` | fetch or push failed, no ref refused; three races | timer, backing off; signal at once |
| `held` | a conflict, unrelated histories, a remote refusal, an index-only conflict with a gap | a tip moved |
| `stopped` | the look, `still_ready`, an identity, a local merge failure, another worktree | every tick and signal |

**`error`** gives the daemon's words, then git's stderr or the `[remote rejected]` reason, joined by `; `. The git part is cut so `error` stays within 4096 B (`STDERR_MAX`). Every control but LF, and every Cf, is written as `\u{XX}` (`hub-confirm.md` "Rules").

**Redaction** goes token by token and never cuts inside one. A token holding the remote's own URL reads as the remote's name; any other token holding `://` reads as `<url>`.

Messages name their cause (no upstream, a URL with a user or password, unclean paths, a conflict, a remote refusal, no branch, shallow or partial, an operation, outside the sparse checkout, an identity, unrelated histories, an outputs gap, fetch or push failed, three rounds, a deadline): CLI `sync.rs`, store `sync.rs`.

## Credentials

Git's, set by the deployment; SpecEngine holds none (supplementing ADR-0034, ADR-0059). The deployment's (`hub-deploy.md`): a deploy key for the repository and a pinned `known_hosts`, Docker secrets of `specengine` alone, outside every `--root`. No `src` string names one: no `GIT_SSH_COMMAND`, `core.sshCommand`, `credential.` setting or key path. A prompt fails at once (`GIT_TERMINAL_PROMPT=0`, stdin null). A URL with a password (`user:pass@`, any scheme), or with any user name on `http`, `https`, `ftp` or `ftps`, is refused before any connection and never shown; an ssh user (`git@host:x`) passes.

## Sparse set

`spec sparse-set [--json]`, read-only, prints, from the root's `specengine.toml` at `HEAD`, paths from the repository's top in config order: `specengine.toml`, `spec.lock`, `.spec-debt.toml`, every `[paths] roots` entry, `[decision_records] dir` and `template`, the `index = true` entry's `writes`. A directory ends in `/`; a path inside a listed directory is dropped. A path `HEAD` holds that the worktree lacks adds `warning: <path> is in HEAD but not in this checkout: widen the sparse checkout`. `--json`: `{"paths", "missing"}`. Exit 0 whatever is missing; exit 2 for no repository, no commit or a bad config. The deployment's clone reads the list, and the look checks the same set.

## Screen

`SyncLine` sits in the shell beside a project's view, with no control. It reads once a login is known (`useHubLogin`), then on window focus and every 30 s: `1 commit not pushed` or `<N> commits not pushed` when `ahead` > 0, `Sync: <error>` as text, escaped as `shownConfirmText`; nothing for `ahead` 0 with no error, a 404, a failed read or a local page. Project routes read `GET /api/identity` once per page for it, a local page too. The dialog: `hub-confirm.md` "Screen".

## Rules

- ADR-0012, ADR-0039: no status, check, health row, verify, gate, hook verdict or read derives anything from push state; a confirm never waits on a sync, fails by one, or is refused for being behind, diverged, `held` or `stopped`; an operator mid-merge gets apply step 2's 503.
- The premise: the hub as the brief zone's sole writer is a convention nothing enforces; a terminal's apply there is never refused, its conflict shows as `held`, and the operator merges `<r>/<m>` on the host's terminal while confirms go on.
- ADR-0008: no `[hub]` table; remote and branch are the checkout's upstream, the set comes from the config.
- `--push` stays off until the gate, step 11b-3: a forged `Remote-User` would reach the shared remote.

## Known limits

- A fast-forward stopped at the deadline leaves `R`'s files written or deleted, `HEAD` and the index unmoved: nothing lost; the next sync is `stopped`, naming them, every root's index dirty. Until restored, reads show them and a confirm touching them (target, `spec.lock`, root config) is refused. Recovery: `git restore --staged --worktree :/`, then remove the named untracked files.
- A `post-merge` killed after the ref moved reports "could not move"; SIGTERM reaches git alone, not its hooks or filters.
- An own URL in non-scheme form glued to text stays visible; it never holds a credential.
- A second worktree's `behind` is read against a ref the hub never fetches for it.
- Git LFS: a smudge filter on the fast-forward can reach the network, past `GIT_NO_LAZY_FETCH`; the image ships no git-lfs, and LFS content in the cone is unsupported.
- Merged-in history: a dump-imported orphan below the hub's floor can meet a developer's same-ID commit and block its reject; raise it again (`proposal-completion.md` "Known limits"). Spec IDs: `spec-ids.md` "Known limits".
- A merged config this binary refuses: 503 until upgraded. A partial clone missing a blob: an apply fails instead of fetching.

## Tests

http `tests/hub_git.rs` (bare remotes, `GIT_ALLOW_PROTOCOL=file`, logging `uploadpack`/`receivepack` wrappers, a PATH wrapper pinning git verbs and push flags), `tests/hub.rs`, `tests/daemon_keys.rs`, `tests/hub_confirm.rs`; store `tests/sync_urls.rs`, `sync::tests`; CLI `tests/sparse_set.rs`; UI `src/app/SyncLine.test.tsx`, `src/api/sync.http.test.ts`, `src/inbox/ConfirmDialog.test.tsx`. Not covered: backoff doubling, signal coalescing, turn order, a held sync staying idle, a lost push answer, the older-binary 503.
