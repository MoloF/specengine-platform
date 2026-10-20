---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# `spec verify`'s scopes: a node set's rows

A scope keeps plain verify's report (`spec-verify.md`, `spec-verify-output.md`) to a node set: the whole tree is judged, then its rows filtered. Built: `--task T`; 6b-2's `--node` joins here. `--tests [--task T]` runs tests: `spec-verify-tests.md`. Code: CLI `verify.rs` `keep_task_rows`, `node_set.rs`.

## `--task`

`spec verify --task T [--changed] [--json]` (+ `--root`; `--config` only naming the root's own `specengine.toml`); with `--staged`: clap's conflict, exit 2. Read-only: the index refresh and the queue opened as `spec task show` opens it; the task is never changed.

1. Usage; discovery.
2. T resolved as `spec task show` resolves it. Unknown, another repository's, in `[ids]`, a look-alike, not an ID (`tasks.md` "Commands"' texts), no queue, a newer schema, `HOME`, no worktree, another `--config`: exit 2, stdout empty, `--json` too.
3. Plain's run, unchanged (`--changed`: Pre-existing marked over the whole tree).
4. With no run cause, the set (`spec-verify-tests.md` "The node set") over the same corpus. A row is kept when `NodeSet::row_nodes` finds a set node by its `key.node` as the marker half (with markers) or the entry half (with an entry). Kept rows are plain's, whole, in order; an anchor group is kept or dropped whole.
5. Counts, cascade, `delta`, hints, `(m moved)`, the verdict and the exit over the kept rows; `skipped` 0. A run cause is never filtered: plain's `cannot:` lines, `rows` `[]`, exit 2.

Exit 1 only from a kept Fix row under `enforce*` (plain: `enforce-introduced` as `enforce`); 2 from a kept Cannot row, a run cause or step 2. Notes, after plain's up to the lock note, before the mode note: the set's `verify: <ref> names nothing now`; `verify --task: <n> rows name no node now; plain spec verify lists them` (a dangling or bare scoped marker ID, an entry not a node's canonical ID: never kept); `verify --task: <n> nodes of <T> have no binding` (set nodes no kept row names, a document target's own too). Text: the summary `verify --task <T>: `; JSON `task` (`spec-verify-output.md`).

## Known limits

- A Cannot row outside the set is not reported: the run never reads `ok` for it; plain `spec verify` lists it (`architecture-code.md#verify`).
- A document target takes every section it contains: its rows are most of plain's.
- It judges the caller's tree: run it in the task's claimed worktree.
