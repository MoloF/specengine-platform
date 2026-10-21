---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-21
---

# `spec lock diff`: what changed since a lock entry

ADR-0053, adding to ADR-0046 (`architecture-code.md#sync-in-reads`): beside a drifted binding, the owner sees what moved on each side since the owner accepted it, before deciding a drift item or an accept. Built by `lock-diff` (`docs/features/lock-diff.md`, shipped 2026-10-21: CLI `lock_diff.rs`, the daemon's route, the UI's types and `getLockDiff`); the browser panel comes with `lock-diff-ui` (RM-0033).

## The read

- `spec lock diff REF [--json]` (+ `--root`, `--config`); library `lock_diff(&Env, &Globals, &LockDiffRequest {reference, git})` -> `Outcome::LockDiff`; the daemon's `GET /api/projects/:p/lock-diff?ref=<REF>`, one call in the project's turn, the same JSON. A read: no terminal check, no `[y/N]`, no MCP tool, no plugin change.
- **REF**: trimmed, classified and resolved as `spec show` resolves it over the refreshed md index (CLI README "Rules"): show's exit-1 cases exit 1 (`reason` its words), its refusals exit 2; a written `@rev` and several holders give show's `note:` and `warning:` texts. Holders by (path, ord), as `spec-verify-scope.md` "`--node`".
- **Code layer**, as `show --bindings` reads it (`code-reads.md` "Degrade"): `verify --node`'s judge (`judge_set`) over the holders in one git session, the lock parsed with this build's recipes and cut to their entries. Any run cause, the session's end (its blob batch read too) included, leaves every holder's `rows` `null` with one `bindings: <cause>` note (N0), exit 0. `[verify]` is never read.
- **Rows**: a row with a lock entry whose state raises drift (core `drift::raises(state, detail, false)`: `code_ahead`, `spec_ahead`, `conflict`, `broken_binding`), in verify's order, under each holder whose set holds it; `ok` and `unlocked` rows are not listed. A row holders share is computed once, its notes emitted once, at its first place.
- **Baseline**: the working tree, uncommitted edits too, never `HEAD`, so the hunks agree with the row's `sync`.

## Sides

A spec side when the row's node moved: its (R, H) (`node_rev`, `norm_hash` of a fresh parse) is not the entry's (`rev`, `spec_hash`). A code side when the code moved: `file_gone`, `symbol_gone`, `marker_gone`, or verify's "Code changed" (`spec-verify.md` "States").

| Row | `spec` | `code` |
|---|---|---|
| `conflict` | yes | yes |
| `code_ahead` `code_changed`, `levels_changed` | `null` | yes |
| `code_ahead` `raised` | yes | when `levels.changed` is non-empty or `levels.lock` ≠ `levels.markers` |
| `spec_ahead` `spec_changed`, `same_revision` | yes | `null` |
| `spec_ahead` `marker_behind` | when (R, H) moved | `null` |
| `broken_binding` `file_gone`, `symbol_gone` | when (R, H) moved | yes; new text `""` |
| `broken_binding` `marker_gone` | when (R, H) moved | yes |

- **Spec texts.** Old: the file holding the row's node now, read at the entry's `commit` (store `WorktreeGit::blob_at`, the path made top-relative: the root's prefix below its worktree's top), parsed under the working tree's `specengine.toml`; the node whose canonical cited ID is the entry's `node`, cut as `spec show` cuts it and carried through its last line's LF. New: the row's node so cut in the bytes this read parsed. Read once per (commit, path), diffed once per (commit, path, node).
- **Code texts.** Old: the entry's `file_blob` by OID in the judge's session (`VerifyGit::read_blobs`), its item located as `spec lock rebase` locates it (`spec-lock-rebase.md` "Location, levels, drops"), whole lines (the first line's start through the last line's LF), each blob bound once per file. New: the row's key located the same way in the bytes the judge read; a moved row: old by the entry's key, new by the row's; `file_gone`, `symbol_gone`: `""`. A `module` or `ron` binding (no RON value locator), an item not located in the blob or not located now: whole files, with N6.
- **Hunks**: store `WorktreeGit::diff_hunks` (as `spec review`) over two scratch files in the data directory, `specengine-<pid>-<seq>-base` and `-proposed`, removed before it returns: every line from the first `@@` (a header may carry git's function context), as git gives them, never re-made; `""` equal. Bytes that differ with no hunk (a binary pair: a NUL byte) are `null` with N7.

## Output

**JSON**, `--json`: one compact document for exits 0 and 1, every key present, uncut (no `OUTPUT_CAP_CHARS`, CLI and daemon alike).

| Key | Value |
|---|---|
| `ref`, `reason`, `notes`, `nodes` | REF raw (show's echo); `null`, exit 1 show's reason; the `note:` texts in order, never a `warning:`; the holders in show's order, `[]` on exit 1 |
| `id`, `path`, `line`, `rows` | a holder's canonical cited ID (`null` an ID-less document), root-relative path, 1-based first line; the kept rows, `[]` none, `null` not judged |
| `row`, `spec`, `code` | verify's 17-key row (`spec-verify-output.md` "JSON"), byte-equal to `spec verify --node REF --json`'s, `pre_existing` `false`; a side, `null` when it did not move |
| `commit`, `path`, `diff` | the entry's full `commit`; the root-relative file read at it; git's hunks, `""` equal, `null` not shown (one note) |
| `blob`, `file`, `diff` | the entry's full `file_blob`; its `file` (a moved row's `anchor.from.file`); as above |

Example: code-a after `spec lock accept LAW-BOUND`, its sentence and `CAP` edited, uncommitted. CAP's row is `conflict`; `spec.diff` `"@@ -1,3 +1,3 @@\n ## Bound {#LAW-BOUND rev=2}\n \n-The count never exceeds the cap.\n+The count never exceeds the cap, twelve by default.\n"`, `code.diff` `"@@ -1 +1 @@\n-pub const CAP: u32 = 10; // @configures LAW-BOUND@2\n+pub const CAP: u32 = 12; // @configures LAW-BOUND@2\n"`; three more rows are `spec_ahead`, a spec side each.

**Text**, stdout, control characters escaped: per holder `<id> | <path>:<line>` (`-` a null `id`), then `  no drifted row with a lock entry` (`[]`) or `  not judged` (`null`). Each kept row after an empty line: verify's row block (`row_block`: the row line, its `  moved from` and candidate lines; `spec-verify-output.md` "Text"); `  spec <path> at <commit>`, `  code <file> blob <blob>`, each followed by its hunks unindented, or ending `: unchanged` (`""`) or `: not shown` (`null`). Last, after an empty line: `lock diff <REF>: rows <r>, spec sides <s>, code sides <c>` (+ `, not shown <k>`, the null sides, when k > 0), or `lock diff <REF>: not judged`; `<REF>` trimmed. The example's: `lock diff LAW-BOUND: rows 4, spec sides 4, code sides 1`.

**Exits**: 0 answered, whatever the rows, sides, notes, run cause or `[verify] mode`. 1 show's exit 1, and a non-UTF-8 `.md` REF (`spec show` answers it, `verify --node` exits 2): stderr `spec: <reason>`, stdout nothing, `--json` `{ref, reason, notes, nodes: []}`. 2 usage, discovery, config, `HOME`, the data directory, a `StoreError`, show's refusals (a look-alike or mixed-script ID, `project:`, an unclean path): stdout nothing, `--json` too.

**Route**: query `ref`, required, one value. 200 exit 0's document, byte-equal to `--json`'s less its final LF; 404 exit 1's; 503 exit 2's error body. 400, nothing run, in `args.rs`' words: an unknown name (`it takes ref`), `ref` missing or repeated, a bad `%` or non-UTF-8. The unknown route's 404 lists `lock-diff` after `roadmap`. It reads the root's working tree and objects under the daemon's environment.

**Keys** (`fixtures/daemon-keys.json`, after `ShownBindings`, 81 sets): `LockDiffView` `[ref, reason, notes, nodes]`, `LockDiffNode` `[id, path, line, rows]`, `LockDiffRow` `[row, spec, code]`, `LockDiffSpecSide` `[commit, path, diff]`, `LockDiffCodeSide` `[blob, file, diff]`; `row` `DriftRow`'s 17. UI: the five types in `api/provisional.ts`, `getLockDiff(project, ref, signal?)` (a 404 resolved as data), mocks `mocks/lockDiff.ts`.

## Notes

stderr `note: <text>`, JSON `notes`, one line each. Order: show's (`@rev`), the `bindings:` cause, then per side in output order. `<key>`: the row's key as verify's text prints it, `<node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]`; `L` stands for `lock diff: <key>`.

| # | When | Text |
|---|---|---|
| N0 | a run cause | `bindings: <cause>` (`code-reads.md` "Degrade") |
| N1 | the file cannot be read at `commit` (the commit or object gone, the file not in it, git or the root's prefix failing) | `L: no spec diff: <path> cannot be read at <commit>` |
| N2 | the parse at `commit` has no such node | `L: no spec diff: <path> at <commit> holds no <node>` |
| N3 | those bytes are not UTF-8 | `L: no spec diff: <path> at <commit> is not UTF-8` |
| N4 | the blob is missing or not a blob | `L: no code diff: blob <blob> of <file> cannot be read` |
| N5 | the blob is not UTF-8 | `L: no code diff: blob <blob> of <file> is not UTF-8` |
| N6 | a whole-file code side, shown | `L: the code diff is the whole file: <why>`: `a module binding`, `a ron binding`, `<symbol> is not found in blob <blob>`, `<symbol> is not found in <file> now` |
| N7 | the diff gives none, path-free | `L: no <spec\|code> diff: <why>`: `git diff --no-index could not run` (scratch I/O, or git naming a scratch file), `git shows no hunk for bytes that differ` (a binary pair), else `git diff --no-index failed: <GitError, one line>` |

N1-N5, N7 leave that side's `diff` `null`; the other side and the exit stay.

## Never

- Stored: computed per read; no queue row, event, `spec.lock` write or index column. Only the data directory is written: the md and code index refresh, the scratch pair per diff. Never the repository, its git index, objects or refs.
- A verdict: `[verify]` is never read; no exit, status, claim, approval or gate depends on a row, side or note (ADR-0012, ADR-0046).
- A failure on the code layer or a side: a run cause leaves `rows` `null`, never `[]`; a side that moved but cannot be shown is `diff` `null` with one note, never `""` and never a dropped row.
- Git beyond `rev-parse` (`WorktreeGit::new`'s `--local-env-vars` probe outside `GIT_OPTIONAL_LOCKS`: it reads no repository), the judge's `hash-object` without `-w`, `cat-file` (`blob_at`'s `--end-of-options blob <commit>:<path>`, `read_blobs`' batch, both under `GIT_NO_LAZY_FETCH=1`: a partial clone never fetches) and `git -C <data dir> -c core.attributesFile=/dev/null diff --no-index --no-color --no-ext-diff --no-textconv --diff-algorithm=myers -U3` (`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_ATTR_NOSYSTEM=1`, `GIT_CEILING_DIRECTORIES` the data directory's parent, `GIT_DIFF_OPTS` removed: no config, attributes or driver of the user's, the system's or an enclosing repository applies); every other process under `GIT_OPTIONAL_LOCKS=0`. Never `status`, a `diff` of revisions, `log`, `show`, `fetch`, `ls-files` or a write.
- Output varying by place: byte-identical from two roots, another cwd or reverse creation order; no absolute path, scratch name or time.

## Known limits

- Old objects must stay reachable: decayed ones (rebase, squash, gc: `spec-lock.md` "Known limits") leave a side `null` (N1, N4); a node moved to another file since its entry has no spec diff.
- A CRLF working copy against an LF blob differs on every line (`spec-verify-scope.md` "Known limits").
- A NUL pair is `null` with N7 here; `spec review`'s `diff` and the package's `snapshot_diff`, through the same `diff_hunks`, read it as equal.
- The store strips `GIT_NO_REPLACE_OBJECTS` (a local variable), so the spec side's `cat-file` honours `refs/replace`; the judge's batch does not.
- N6's "is not found" also names an ambiguous match. Each code-moved row clones its blob and its file's text.
- Hunk line numbers count from the span's first line, not the file's; output is uncut, so a document node's hunks can be long.
- A `node_gone` row is never kept: `verify --node`'s set never holds one, so a removed node shows no side (the code's alias read is unreached).
- Cost (release, warm, code-a plus 1 000 files, median of 5): `lock diff LAW-BOUND` 120 ms, `--json` 116, against `verify --node`'s 53; a git process or two per side, in the project's turn (`serve.md` "Concurrency"): on demand only.
