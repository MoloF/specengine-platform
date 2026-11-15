---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-11-15
---

# `spec auto land` and `spec auto hold`

A pack's finished task landed on the rule's branch (ADR-0081), and the runner's hold of an item. The rule, reasons, events and trailers: `auto-mode.md`; packs and the other commands: `spec-auto.md`. Code: CLI `auto_land.rs` (`auto_land`, `LandLock`), `auto_pack.rs` (`auto_hold`), `auto_apply.rs` (`delegated_approve`), `group.rs` (`Forwarding`, the held signals).

## Land

`spec auto land T`, run from any worktree of the repository; it works in the task's claimed worktree and a scratch one. In order, the first failing step ending it:
1. **Membership**: in no pack, exit 1 not-in-pack, no event; landed, exit 1 `` T-0042 is landed: <sha12> on auto; nothing changed ``, no event.
2. `status` needs `done` (exit 1, no event); the last run `completed`, else a hold `run-outcome`.
3. **Lock** `<data dir>/<slug>.auto-land`, made create-new, holding `<pid> <now>`; there: exit 1 `another land holds <path> since <its text>; if none runs, remove it; nothing changed`, no event. One whose pid is gone is taken over atomically: an exclusive `flock` on `<lock>.takeover` (left in the data directory), the lock read again, `<lock>.<pid>` made create-new, renamed over the lock and read again. Under the lock, stale `land-<pid>` scratch worktrees, their `-home` directories and `.msg` files are removed, then `git worktree prune`. From here `SIGINT`, `SIGTERM` and `SIGHUP` are held: one arriving stops the land before its next step, before the branch moves: exit 2 `` interrupted: `T-0042` not landed, `auto` unmoved; the lock and the scratch worktree removed, nothing recorded ``.
4. **Place**: the claim's worktree exists, belongs to the repository, is on the claim's branch, with no operation in progress (`proposal-apply.md` step 2); else a hold `place`, git's or the check's words.
5. **Paths**: `git diff --name-only -z --no-renames <merge base of the claim and the rule's branch>` in the worktree, committed and not, and `git ls-files --others --exclude-standard -z`, root-relative in byte order (one outside the root written `../…`, in no zone; a `specengine.toml` or the root's `spec.lock` stays `config-target`), judged `config-target`, `excluded-zone`, `uncovered`. Then each changed spec document the index walks: the IDs it declares as the merge brings it and those it declared at the merge base (`git show <base>:<root_rel>/<path>`, none when missing), the union judged, `` `SC-002` in `docs/x.md` is a scenario, an excluded kind ``: deleting such a document, or a section of an excluded kind, holds too. A hold commits nothing.
6. **Code commit**, only when something is uncommitted: `git add -A -- .`, `git commit --cleanup=verbatim -F <file>`, the message of `auto-mode.md` "Trailers" (the title the task's, else its ID; the body the last run's summary), hooks run, never `--no-verify`; failing: `git reset -q`, a hold `place` with git's error.
7. **Proposals**: the task's `open` `update` and `create` rows raised in the claim's worktree, by ID number, judged (local, the pack) and applied by `delegated_approve` (`Delegation {rule, pack}`, the worktree's identity, `expected` the row as judged: one changed since is `Moved`, nothing written); a hold logs one `auto.held` (`act` `apply`). A row refused at a step, or moved, is the note `` `PR-0007` not applied by the rule: <reason> `` in every outcome after this step. `left`: every row bound to the task still `open`, whichever worktree raised it, deciding kinds too. Nothing here stops the land.
8. **Merge** in the scratch worktree `<data dir>/auto/<slug>/land-<pid>` (`git worktree add --detach` at the branch's tip, `old`): `git merge --no-ff --cleanup=verbatim -F <msg> <claim tip>`, the merge message of "Trailers". A conflict: `git merge --abort`, a hold `merge-conflict`, `<n> path(s) conflict: <p1>, …` (8 named, then `and <k> more`); a failure naming no path: `place`. **Nothing new**: a claim tip already on the branch is a recovery when an earlier land merge (`git log --grep '^Task: T-0042$'`) has it as its second parent (a land stopped between its move and its event): that merge is recorded, noted `recovered: <sha12> merged T-0042 into auto before, unrecorded; recorded now`; else a hold `place`, `` nothing to land: `<claim branch>` has no commit `auto` lacks ``. A merge making no commit holds `place` too; the old tip is never recorded as a merge.
9. **Check**: `spec check`'s library call on `<scratch>/<root_rel>`, its index and queue in a scratch data directory (`land-<pid>-home`), never the project's, against the branch's old tip: the scratch `HEAD` set back to `old` (`reset --soft`), `CheckedTree::Changed`, then moved forward again. Anything but `clean` holds `check`: `spec check: <n> error(s); the first <path>:<line>: <code>` (expired debt among them), else `spec check: <verdict>: debt the base's baseline lacks`; one that cannot run: `spec check cannot run: <why>`. The merged tree's config is the rule's (config changes hold at step 5). A red tip holds every land until the owner fixes it.
10. **Move**: the branch checked out in any worktree: a hold `branch-checked-out`, `` `auto` is checked out in <w> ``; else `git update-ref refs/heads/<b> <merge> <old>`; refused: `branch-moved`, `` `auto` moved from <old12> since this land read it ``.
11. `auto.landed`; stdout `landed T-0042 on auto as <sha12> (applied PR-0007; left PR-0009)` (`none` for an empty list); JSON `{"id":"T-0042","landed":true,"branch":"auto","commit":"<sha>","code_commit":"<sha>|null","applied":["PR-0007"],"left":["PR-0009"],"rule":"b3:…","reason":null,"detail":null,"notes":[]}`.

A hold at steps 2 and 4-10 logs one `auto.held` (`act` `land`) and exits 1 `` T-0042 held (<reason>): <detail>; auto unmoved ``, JSON `landed` false with `reason` and `detail`. The scratch worktree (`git worktree remove --force`, `prune`) and the lock go in every outcome. The commits of steps 6-7 stay on the task's branch, so a land run again (the owner's, after a fix) continues. A land never pushes, checks out a branch or writes `spec.lock`, and moves no ref but the rule's branch (step 10) and the claim branch (steps 6-7). Paths: the lock, `<data dir>/auto/<slug>/`; nothing under the project root but the claim's commits and the branch's ref.

## Hold

`spec auto hold ITEM --reason R [--note TEXT]`: `ITEM` a task ID or `<pack>/<n>`; `R` one of `agent-error`, `denied-tool`, `no-mcp`, `max-turns`, `max-budget`, `not-planned`, `not-done` (else exit 2); `TEXT` at most 1 024 bytes, no control character (else exit 2), default the reason. A pack's item not landed: `auto.held` (`act` `run`, `detail` the note), stdout `held T-0042 (denied-tool): <note>`, exit 0; in no pack, or landed: exit 1, no event. A held item is never offered again (`spec auto next`); the owner's `spec auto land T` re-judges a held `done` task, its success recorded `auto.landed`.

## Known limits

- A merge that makes no new commit, and the start's re-check of one live pack per task inside its transaction, need a race: untested.
- Paths are judged whole-file: a changed document declaring an excluded kind holds the land even when the change missed that section.

## Tests

CLI `tests/auto_land.rs` (a bare repository as `origin`; LD3-LD5, LD7, LD8, LD11, LD12), `auto_next.rs` LD11; `src/auto_land.rs` units (the takeover).
