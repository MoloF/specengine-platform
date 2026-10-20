---
class: canon
tier: 2
scope: [crates/specengine-cli]
owner: owner
reviewed: 2026-10-21
---

# `spec verify`'s scopes: a node set's rows

A scope keeps plain verify's report (`spec-verify.md`, `spec-verify-output.md`) to a node set. `--task T` judges the whole tree, then filters its rows; `--node REF` narrows the files judged to the node's own. `--tests [--task T]` runs tests: `spec-verify-tests.md`. Code: CLI `verify.rs` (`keep_task_rows`, `row_set_nodes`), `verify_node.rs`, `node_set.rs`, `code_walk.rs` `partial_tree`; store `code_selection`.

## `--task`

`spec verify --task T [--changed] [--json]` (+ `--root`; `--config` only naming the root's own `specengine.toml`); with `--staged`: clap's conflict, exit 2. Read-only: the index refresh and the queue opened as `spec task show` opens it; the task is never changed.

1. Usage; discovery.
2. T resolved as `spec task show` resolves it. Unknown, another repository's, in `[ids]`, a look-alike, not an ID (`tasks.md` "Commands"' texts), no queue, a newer schema, `HOME`, no worktree, another `--config`: exit 2, stdout empty, `--json` too.
3. Plain's run, unchanged (`--changed`: Pre-existing marked over the whole tree).
4. With no run cause, the set (`spec-verify-tests.md` "The node set") over the same corpus. A row is kept when `NodeSet::row_nodes` finds a set node by its `key.node` as the marker half (with markers) or the entry half (with an entry). Kept rows are plain's, whole, in order; an anchor group is kept or dropped whole.
5. Counts, cascade, `delta`, hints, `(m moved)`, the verdict and the exit over the kept rows; `skipped` 0. A run cause is never filtered: plain's `cannot:` lines, `rows` `[]`, exit 2.

Exit 1 only from a kept Fix row under `enforce*` (plain: `enforce-introduced` as `enforce`); 2 from a kept Cannot row, a run cause or step 2. Notes, after plain's up to the lock note, before the mode note: the set's `verify: <ref> names nothing now`; `verify --task: <n> rows name no node now; plain spec verify lists them` (a dangling or bare scoped marker ID, an entry not a node's canonical ID: never kept); `verify --task: <n> nodes of <T> have no binding` (set nodes no kept row names, a document target's own too). Text: the summary `verify --task <T>: `; JSON `task` (`spec-verify-output.md`).

## `--node`

`spec verify --node REF [--symbol QPATH] [--json]` (+ `--root`, `--config`): REF's own bindings, each row plain's on the same state. The one verify mode refreshing the code index; it writes the data directory only.

1. Usage, exit 2, stdout empty, `--json` too: clap's conflict with `--changed`, `--staged`, `--task`, `--tests`; `--symbol` requires `--node`. The library refuses them before discovery, in clap's words: `symbol` alone; `node` with `Changed`, `Staged` or `task`.
2. REF trimmed, classified as `show` does; the md index refreshed; REF resolved to its holders H by (path, position), each once, never a section or a parent (a `.md` path: its document node). `show`'s refusals (a look-alike, `project:`, an unclean path, naming nothing, where `show` exits 1), `HOME`, the data directory, a `StoreError`: exit 2, stdout empty, `--json` too. A written `@rev`: ``note: `@<rev>` is ignored: …``, the node judged as it is now. Several holders, all judged: ``warning: `<REF>` has <n> holders, all verified: <citation>, …``.
3. Plain's run causes (1), (2), (4)-(6); an unknown `[code] frameworks` name is (1) too, after a `[verify]` one (plain never reads the key); `refresh_code(.., false)` failing is (8), its gap (3) `the code walk is incomplete: <gap>`. A run cause: plain's report, `rows` `[]`, exit 2.
4. One read transaction (`code_selection`) gives the files S: those of the stored markers `spec refs REF` lists under a holder (a look-alike under its fix's node) and of the `spec.lock` entries whose `node` can be a holder's. The lock is cut to those entries: another node's is never judged, never `file_gone`.
5. Each stored file of S: read once, its BLAKE3 checked against the stored one, bound (Rust) or anchored (RON) as `read_tree` does, with the stored names by `ord` (cross-file `duplicate` too): no walk, no naming pass; levels hashed afresh, never from the index's columns. A mismatch, or names missing or not one per item, refreshes and selects once more; a second: cause (3) ``the code walk is incomplete: `<path>` changed while it was read; run it again``. Not UTF-8: `not_utf8`. Not stored: `outside_code` on disk, else `file_gone`.
6. Plain's judge, pairing per node; rows kept by `row_set_nodes` over `NodeSet::of(H)`, as `--task`. `--symbol QPATH` keeps a row whose `symbol`, RON `<file>#<symbol>` or moved `anchor.from` (either form) is QPATH byte for byte; never a keyless row; no tiers, no suffix.
7. Counts, cascade, `delta`, hints, `(m moved)`, verdict, exit over the kept rows; `base` `null`, `skipped` 0, `enforce-introduced` as `enforce` with plain's no-base note. No row: `clean`, exit 0, `note: verify: <REF> has no binding[ on <QPATH>]`, after the refresh's notes, before the mode note.

MCP: `check_binding` (`mcp-code.md`). Output (`spec-verify-output.md`): every kept row prints, `ok` too, so the JSON's `rows`, the printed ones, keep the text's bound: whole blocks within `OUTPUT_CAP_CHARS` (40 000, counted before escaping), then a tail naming `--symbol`; a first block over it prints no row. Counts and hints cover every kept row; JSON `node` the holders' IDs, `symbol` QPATH, `omitted` the unprinted.

## Known limits

- A Cannot row outside the set is not reported: the run never reads `ok` for it; plain `spec verify` lists it (`architecture-code.md#verify`).
- A document target takes every section it contains: its rows are most of plain's.
- It judges the caller's tree: run it in the task's claimed worktree (MCP: the server's root, the session's worktree).
- `--node` takes no base (one reads all of `HEAD`'s code): `enforce-introduced` blocks on the node's pre-existing Fix row. `notes` are the refresh's, not plain's walk's; `node` lists an ID twice for two holders; a CRLF copy moves blob OIDs, so `compared`, `by_blob`.
