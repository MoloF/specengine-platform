---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-http]
owner: owner
reviewed: 2026-11-03
---

# Proposal apply: the index's outputs in the commit

ADR-0062, built by `apply-outputs` (RM-0072). Every apply (`spec approve`, `spec round answer`, a hub's confirm: `hub-confirm.md`) commits beside its spec file the outputs of the `[[generators]]` entry with `index = true` that its change alters: the root index, each shard, the roadmap view (core `render_index_set`, `spec-check-graph.md` "API"). SpecEngine renders them; a project's own generator command never runs (ADR-0013). Still one commit, its subject, body and trailers as before (ADR-0005); the consent question is unchanged. A drift item, `lock accept`, `lock rebase` and `verify --tests` write `spec.lock` alone and never render. Code: CLI `outputs.rs` (`Outputs`: `render`, `write`, `stage`, `restore`, `compare`; `never_output`, `parent_outputs`, `Beside`); store `outputs.rs` (`committed_tree`), `worktree.rs` (`intent_to_add`, `is_intent_to_add`, `remove_cached`, `is_ignored`, `is_skip_worktree`, `local_blob_at`, `disk_name` -> `DiskName`). A `--push` hub's merge commit carries the same outputs, rendered from its merged tree (CLI `render_tree`, `hub-git.md` "Merge commit").

<a id="outputs"></a>
## The committed tree

The render's input is `HEAD` at step 2's commit (step 8 re-checks it) under the recorded root, its `specengine.toml` and index entry included, with each path the apply writes (the target, a new file or record; `--editorial` also `spec.lock`) at the bytes step 8 writes. That is the tree `git commit --only` records and a staged hook judges, git naming its temporary index in `GIT_INDEX_FILE` (`spec-check-git.md` "The staged check"). Never the disk walk: an uncommitted document would add a line the commit lacks, and a sparse cone is a written `MissingRoot`. Never the git index: `--only` leaves other staged paths out. Store `committed_tree` builds it as `--changed` builds its base (`Staged::head_only`, `ls-tree -r -z`, one `cat-file --batch`, `GitIndex::from_walk` by `HEAD`'s `[paths]`), with `GIT_NO_LAZY_FETCH=1` on every child: a blob a partial clone lacks is missing, never fetched.

**Which:** the outputs whose render differs from `HEAD`'s blob, or that `HEAD` lacks. None when `HEAD`'s config has no index entry: the commit as before, byte for byte. An update that changes no index line commits its path alone; an index stale at `HEAD` is brought up to date by the next apply.

## Never an output

The config never makes an apply write a spec file or a person's file. A path `HEAD`'s config names as an output narrows the commit, `` `<p>` can never be an output (<why>) ``, when it is:

- the apply's target, a new file or record included (`the apply's target`);
- a file named `spec.lock`, `specengine.toml` or `.spec-debt.toml`, in any directory;
- outside `HEAD`'s walk scope (`outside the walk`);
- a document `HEAD` holds whose class is not `generated`, its front-matter unread included (`a spec document`);
- one `HEAD` lacks whose bytes on disk do not read as a generated document (`a spec document`; `not a generated document on disk` when they do not read at all), so an uncommitted file is never overwritten.

## Steps

Apply's steps (`proposal-apply.md` "Apply steps", `decision-record.md` "Steps", `proposal-create-apply.md` "Apply") with the outputs:

- **8.** The render, before anything is written. Then each changed output is judged: inspected as `spec export index` inspects (`spec-check-cli.md` "spec export index": each component below the root a real directory, the file absent or a regular file, its parent there, no two outputs one file); one `HEAD` lacks not ignored by git; its name on disk the configured one byte for byte (`disk_name`: `AsAsked`; `Held(name)`, a case or Unicode-form variant; `CannotTell`, no listing entry matching by name or by device and inode, which narrows); none skip-worktree. Then the target is written, then each output atomically as the target is (a dot-named sibling, synced, renamed).
- **9.** The outputs staged in a step of their own (one `HEAD` lacks: `git add --intent-to-add`), then `git commit --only` of the target and them. A failed staging drops each index entry this run made only while it is still intent-to-add (a person's own staging is never dropped), puts the outputs back and commits the target alone. A failed commit (a hook's refusal) is the apply's own refusal: each output put back only while it holds this run's bytes (one absent before removed), entries dropped by the same rule.
- **10.** The target as before (`--editorial`: and `spec.lock`), no other changed path but written outputs; each written output's blob in the new commit compared with this run's bytes: a difference (two applies in one worktree, a hook's rewrite) warns, and the proposal stays `applied`. `update_paths` over the target and the outputs.

## Messages

stdout and `--json` as before; on stderr, root-relative paths in config order:

```text
note: the commit carries the index's outputs too: docs/index.md
warning: the commit carries `docs/features/stamina.md` alone: the index's outputs were not rendered: <cause>; `<command>` writes them
warning: the committed `docs/index.md` is not the bytes this apply rendered; `<command>` writes them
```

`<cause>`, the first met: `HEAD`'s config not valid; `` `[paths] index` is not set ``; a path that can never be an output; `walk_gap`'s first gap as `spec export index` words it; a blob missing from the object database; an inspection refusal; `` `<p>` is ignored by git ``; `` `<p>` is `<q>` on disk (a case variant) ``; `` cannot tell which file `<p>` names on disk ``; `` `<p>` is skip-worktree ``; `` cannot write `<p>` ``; `` staging `<p>` failed: <git's error> ``. `<command>` is the entry's `command`, byte for byte. A hub's confirm also appends the first warning to the POST's review document's `notes` (`hub-confirm.md` "Routes").

**An output never refuses** (ADR-0012): every cause narrows the commit to the target with one warning, and the apply goes on to `applied`. Without this, a project that ignores its local `docs/index.md` would have every create refused at step 9.

## Completion

The judges (`trailer_commit`, `file_commit`, `record_commit`; `proposal-completion.md` "Completion") accept beside the target the paths that are outputs of the index entry in the **parent's** root config (`parent_outputs`), the tree the apply rendered from, never the commit's own config: added or modified, never deleted, their bytes unjudged (`spec check` judges them). Never one "Never an output" excludes, judged on the parent: so a hand commit cannot authorise its own extra paths. The parent's config is read by `local_blob_at`, `GIT_NO_LAZY_FETCH=1`; none or invalid: no output accepted. `drift_commit` and the hint are unchanged.

## Known limits

- A hand commit can add a `class: spec` document at an output path the parent's config names but the parent lacks, and still complete.
- An intent-to-add entry whose file is gone when the drop runs stays in the index.
- A document staged at an output path `HEAD` lacks, then deleted on disk, is overwritten by the output.
- On a case-sensitive file system (a Linux hub) an output whose name differs only in case from another path is its own file there; a case-insensitive clone holds one.
- A fallback `blob_oid` read, and step 10's compare, run without `GIT_NO_LAZY_FETCH=1`.
- A staging that narrows warns once, for the first output that failed, never per output.
- `DiskName::CannotTell` is untested: no file system yields it on demand.
- Two branches' applies conflict on the index; a partial clone missing blobs gets no index (a hub: a sparse checkout with every blob, RM-0064).
- Under a clean or eol filter the committed target differs from the rendered bytes (`proposal-completion.md` "Known limits").
