---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-12
---

# `spec.lock`: the file, its entries, `spec lock regen`

`<root>/spec.lock`, one per project root beside `specengine.toml`, committed: per marker binding, a node's revision and `norm_hash` paired with its code's hashes at a commit, so drift can be told from `ok`. Neither a spec file nor `generated`: only the owner's terminal `[y/N]` writes it, by the writers of `architecture-code.md#spec-lock` (ADR-0041). Built: `spec lock accept REF [--editorial]` (`docs/canon/spec-lock-accept.md`) and `spec lock regen` (below). Model: core `lock`, pure; git: store `WorktreeGit`; commands: CLI `lock.rs`, `lock_regen.rs` (their READMEs). Plain `spec check`, the index, MCP, HTTP, UI, the gate and the daemon never read or write it. No `[code]`, no lock.

## The file

```toml
# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit
format = 1
norm_recipe = "specengine-norm/v1"
code_recipe = "specengine-hash/v2 tree-sitter-rust/0.24.2 abi/15"

[[binding]]
node = "R-12"
rev = 3
spec_hash = "b3:9f2c<60 hex>"
relation = "implements"
file = "crates/game/src/stamina.rs"
kind = "item"
symbol = "game::stamina::regen_system"
item_kind = "function_item"
ast_hash = "b3:41aa<60 hex>"
file_blob = "5f1c9e<34 hex>"
commit = "a0cea48<33 hex>"
verified_by = "owner"
verified_at = "2026-10-10"
```

- `norm_recipe` = core `NORM_RECIPE` (`node-revision.md`); `code_recipe` = `<RECIPE> tree-sitter-rust/<version> abi/<n>` of `specengine-code`.
- Bytes (`render_lock`, never `toml`'s serializer): keys in the order shown; blocks in key order, an empty line before each; LF, a final LF; UTF-8, no BOM; basic strings escaping only `"`, `\`, U+0000-U+001F, U+007F (`\b \t \n \f \r`, else `\u00XX`, uppercase hex); full OIDs (64 hex in a SHA-256 repository); `verified_at` the command's UTC date. Every entry dropped: the header stays.
- No absolute path, name, e-mail, time of day, abbreviated OID, line or column. One commit gives one lock from two roots, another cwd, in reverse creation order, with `core.autocrlf=true` or through an alias marker; a SHA-256 repository the same `b3:` hashes.

## Entry

One `[[binding]]` per lockable marker binding; markers giving one key are one entry; mentions never.

| Key | Value |
|---|---|
| `node` | the canonical cited ID (`slug/AC-07` if feature-scoped), never an alias |
| `rev`, `spec_hash` | the node's revision R at the commit (an `unversioned` marker is locked at R), its `norm_hash` |
| `relation`, `file` | `implements`, `verifies`, `configures`, `assumes`; root-relative, `/` |
| `kind`, `symbol` | `item` + qpath, `module` + qpath, `ron` + field path (`root.stamina.regen_per_second`) |
| `item_kind`, `ast_hash` | items only: the tree-sitter kind; `b3:` + the whole item's v2 digest (levels ignored) |
| `file_blob`, `commit` | the file's blob OID in HEAD's tree; HEAD's full OID |
| `verified_by`, `verified_at` | `"owner"`, never a test run; `YYYY-MM-DD` |

Key and order: `node` bytes; `relation` as listed; `file`, `kind`, `symbol`, `item_kind` bytes. Not in format 1: levels, fingerprints, anchor state, text hash, positions, test evidence (`[[tested]]`, the next format); `impl_status` never (computed, ADR-0044).

**Reader** (`parse_lock`), any TOML layout, CRLF too: exit 2 `spec.lock:<line>: <message>` on bytes not UTF-8; another `format`; an unknown, missing or repeated key; a duplicate entry key; a wrong form (`b3:` + 64 lowercase hex; a 40- or 64-hex OID; `rev` 1-999 999 999; a known relation, kind; `item_kind`, `ast_hash` iff `kind = "item"`; `verified_by = "owner"`; a calendar date); another build's recipes, both named.

**Key rules** (`key_problem`, shared by writer and reader: what is written reads back): `node`, `item_kind` non-empty, no control character; `file` a clean root-relative path, no `\`, no control character; `symbol` any non-empty string. A fresh key failing them is `not locked: <problem>`, never written.

## Lockability

A binding `spec refs` lists under the node (`code-bindings.md` "ID state", "Revision state") is locked only if each row allows; else `not locked: <state>`, never fresh.

| Row | Locked | Never |
|---|---|---|
| `id_state` | `resolved`, `alias` | `ambiguous`, `homoglyph` |
| `rev_state` | `current`, `unversioned` | `spec_ahead`, `predated`, `bad_rev`, `rev_unknown` |
| binding | `item` with a v2 digest; `module`, `ron` by `file_blob` alone | `ambiguous <reason>`, `unanchored`, `cannot_verify` |

One key from several markers is lockable only if every marker allows it; the first refusal in marker order names the row. A `[sig]` marker gets the whole item's hash (levels: slice 5).

## `spec lock regen`

After a merge conflicting in `spec.lock`, on a terminal (else exit 2, before anything is read). Only while `spec.lock` is unmerged (`ls-files -u -z`), else exit 1; each stage present parses and all share `format` and recipes, else exit 2 (the build's own recipes are not required). Per entry key, base -> ours, theirs:

| Case | Result |
|---|---|
| same on both, or changed on one side | that entry |
| deleted on one side, unchanged on the other; deleted on both | deleted |
| deleted on one side, changed on the other | the changed one |
| changed differently (add/add without base too) | the later `verified_at`, then the block first by bytes |

Side-symmetric; reads no code or spec file, computes no hash: each choice keeps an owner-accepted pair (drift possible, never a false `ok`); never rebuilds from the tree. Rows on stdout `<choice> | <node> | <relation> | <file> | <kind> <symbol>`, choice `both`, `ours`, `theirs`, `deleted`, `later`, `bytes`; stderr `merge spec.lock (<n> entries, <k> by verified_at or bytes) in <worktree>? [y/N]`, only `y`, `yes`. Writes the worktree file only, then stdout `next: git add spec.lock, then commit the merge`. Exit 0 written; 1 no conflict, declined, changed while running; 2 not a terminal, usage, discovery, config, no `[code]`, no worktree, a stage unreadable, neither ours nor theirs, recipes differ.

## Travel and writes

Each lock travels on its worktree's branch; git's `index.lock` orders commits; branches meet in a merge (regen). Writes: the data directory, `spec.lock`, accept's one commit; no queue row, event or index change.

## Known limits

- A grammar bump (tree-sitter-rust, its ABI) re-hashes every item: a lock of other recipes exits 2 until slice 5b's rebase.
- Commits decay (rebase, squash, gc): a `commit` may name nothing; a renamed ID's entries stay until `accept <old ID>` drops them (`spec-lock-accept.md` "A gone node").
- Cooperative: a faked terminal passes (ADR-0034); the gate skips `spec.lock`.
