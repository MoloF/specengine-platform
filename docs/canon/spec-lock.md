---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# `spec.lock`: the file, its entries, `spec lock regen`

`<root>/spec.lock`, one per project root beside `specengine.toml`, committed: per marker binding, a node's revision and `norm_hash` paired with its code's level digests at a commit, so drift can be told from `ok`. Neither a spec file nor `generated`: only the owner's terminal `[y/N]` writes it, by the writers of `architecture-code.md#spec-lock` (ADR-0041, ADR-0050). Built, writing format 2 only: `spec lock accept REF [--editorial]` (`docs/canon/spec-lock-accept.md`; a drift approve runs it), `spec lock regen` (below) and `spec approve --editorial`, refreshing vouching entries in its apply commit (`docs/canon/proposal-revisions.md` "Editorial approve"). Model: core `lock` (`parse_lock`, `render_lock`, `lock_code`, `LockFormat`, `EntryCode`, `REBASE_STEP`, `key_problem`/`KeyProblem`, the merges, `vouches`, `refresh_editorial`), pure; git: store `WorktreeGit` (`blob_at`, `blob_oid`, `status_entries`, `is_ignored`, `unmerged_stages`, `reset_path`); commands: CLI `lock.rs`, `lock_regen.rs` (their READMEs). Readers: "Entry"; verify: `spec-verify.md`; the rule: `node-revision.md` "Raising". Plain `spec check`, the index, the UI, the gate and the daemon never read it (MCP, HTTP: a review's vouching count; MCP `report_drift`). No `[code]`, no lock.

## The file

```toml
# spec.lock: written only by SpecEngine (spec lock, spec approve, spec verify --tests); do not edit
format = 2
norm_recipe = "specengine-norm/v1"
code_recipe = "specengine-hash/v3 tree-sitter-rust/0.24.2 abi/15"

[[binding]]
node = "R-12"
rev = 3
spec_hash = "b3:9f2c<60 hex>"
relation = "implements"
file = "crates/game/src/stamina.rs"
kind = "item"
symbol = "game::stamina::regen_system"
item_kind = "function_item"
levels = ["sig"]
sig_hash = "b3:41aa<60 hex>"
body_hash = "b3:07c3<60 hex>"
deps_hash = "b3:d1e0<60 hex>"
skeleton_hash = "b3:5b92<60 hex>"
quote_hash = "b3:a8f4<60 hex>"
body_bytes = 212
file_blob = "5f1c9e<34 hex>"
commit = "a0cea48<33 hex>"
verified_by = "owner"
verified_at = "2026-10-10"
```

- `norm_recipe` = core `NORM_RECIPE` (`node-revision.md`); `code_recipe` = `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>` of `specengine-code`.
- Bytes (`render_lock`, never `toml`'s serializer): keys in the order shown, an absent one skipped; `levels` as `["sig", "body"]`; `body_bytes` plain decimal; blocks in key order, an empty line before each; LF, a final LF; UTF-8, no BOM; basic strings escaping only `"`, `\`, U+0000-U+001F, U+007F (`\b \t \n \f \r`, else `\u00XX`, uppercase hex); full OIDs (64 hex in a SHA-256 repository); `verified_at` the command's UTC date. Every entry dropped: the header stays.
- No absolute path, name, e-mail, time of day, abbreviated OID, line or column. One commit gives one lock from two roots, another cwd, in reverse creation order, with `core.autocrlf=true` or through an alias marker; a SHA-256 repository the same `b3:` hashes.

## Entry

One `[[binding]]` per lockable marker binding; markers giving one key are one entry; mentions never.

| Key | Value |
|---|---|
| `node` | the canonical cited ID (`slug/AC-07` if feature-scoped), never an alias |
| `rev`, `spec_hash` | the node's revision R at the commit (an `unversioned` marker is locked at R), its `norm_hash` |
| `relation`, `file` | `implements`, `verifies`, `configures`, `assumes`; root-relative, `/` |
| `kind`, `symbol` | `item` + qpath, `module` + qpath, `ron` + field path (`root.stamina.regen_per_second`) |
| `item_kind` | items only: the tree-sitter kind |
| `levels` | the markers' effective levels (no list: `sig, body`) united, canonical order `path, sig, body, deps` (`code-identity.md` "Marker grammar") |
| `sig_hash`, `body_hash`, `deps_hash` | every kind and level, listed or not: `item_levels` (an inline `mod` by its `mod_item`), `module_levels` (`code-hash.md` "File module"); `ron` sig = body = the value hash, deps `empty_deps_digest()` (`code-hash-ron.md` "RON value") |
| `skeleton_hash`, `quote_hash`, `body_bytes` | `item`, `module` only (`code-hash.md` "Fingerprints"; `body_bytes` a u32): for 5c, never compared or drift evidence |
| `file_blob`, `commit` | the file's blob OID in HEAD's tree; HEAD's full OID |
| `verified_by`, `verified_at` | `"owner"`, never a test run; `YYYY-MM-DD` |

Key and order: `node` bytes; `relation` as listed; `file`, `kind`, `symbol`, `item_kind` bytes. Not in format 2: anchor state, text hash, positions, test evidence (`[[tested]]`, format 3); `impl_status` never (computed, ADR-0044). **Format 1** (before 5b-1a, read only): v2's `RECIPE` in `code_recipe`; of the code keys only `ast_hash`, the whole item's v2 digest, iff `item`.

**Reader** (`parse_lock(text, build)`), any TOML layout, CRLF too: exit 2 `spec.lock:<line>: <message>` on bytes not UTF-8; `format` neither 1 nor 2 (`… is newer than this build's 2`, `… is not this build's: it reads formats 1 and 2`); an unknown (`ast_hash` in 2, `levels` in 1), missing or repeated key; a duplicate entry key; a wrong form (`b3:` + 64 lowercase hex; a 40- or 64-hex OID; `rev` 1-999 999 999; a known relation, kind; `item_kind` iff `item`, fingerprints iff `item` or `module`; `levels: <empty|unknown <name>|duplicate <name>|order>`; `verified_by = "owner"`; a calendar date).

- `None`, the spec side alone (the rule `rev-not-raised`, `spec bump`, a review's vouching count, an editorial approve, regen's stages): formats 1 and 2, recipes never compared.
- `Some(build)`, the code judges and other writers (verify and accept in every mode, `spec propose drift`, MCP `report_drift`): format 2 at this build's recipes, the first failing check the error: (1) unreadable, the `format` above; (2) another `norm_recipe`, both named, no step (no rebase crosses it); (3) `format 1 ("<its code_recipe>"), this build's is 2 ("<build's>"): run spec lock rebase on a terminal`; (4) `` `code_recipe` is "<its>", this build's is "<build's>": run spec lock rebase on a terminal ``; (5) the entries. Each caller carries it whole (a drift raise, approve inside its wrapper), no hint.
- No reader upgrades a lock in memory; a writer reading `None` refuses format 1 before rendering.

**Key rules** (`key_problem`, writer's and reader's: what is written reads back): `node`, `item_kind` non-empty, no control character; `file` a clean root-relative path, no `\`, no control character; `symbol` any non-empty string. A fresh key failing them is `not locked: <problem>`, never written.

## Lockability

A binding `spec refs` lists under the node (`code-bindings.md` "ID state", "Revision state") is locked only if each row allows; else `not locked: <state>`, never fresh.

| Row | Locked | Never |
|---|---|---|
| `id_state` | `resolved`, `alias` | `ambiguous`, `homoglyph` |
| `rev_state` | `current`, `unversioned` | `spec_ahead`, `predated`, `bad_rev`, `rev_unknown` |
| levels | `Default` (no list), a declared list | `Invalid` (`unclosed`, `unknown`, `duplicate`, `empty`): `bad_levels` |
| binding | `item`, `module`, `ron` hashed | `ambiguous <reason>`, `unanchored`, `cannot_verify [<category>]`: an item with a parse error, a `module` whose file has any, a `ron` value of a file `RonAnalysis` rejects |

In-file twins are lockable, each by its discriminated key; a cross-file `duplicate` never (`symbol-names.md` "Frozen names"). One key from several markers is lockable only if every marker allows it; the first refusal in marker order names the row.

## `spec lock regen`

After a merge conflicting in `spec.lock`, on a terminal (else exit 2, before anything is read). Only while `spec.lock` is unmerged (`ls-files -u -z`), else exit 1; each stage present parses (`None`) and all share `norm_recipe` (else exit 2 naming both), then format 2 of one `code_recipe` (any build's), else exit 2 `` spec.lock: a stage is format 1, or the stages differ in `code_recipe`: run git merge --abort, spec lock rebase on the lagging branch, and merge again ``. Per entry key, base -> ours, theirs:

| Case | Result |
|---|---|
| same on both, or changed on one side | that entry |
| deleted on one side, unchanged on the other; deleted on both | deleted |
| deleted on one side, changed on the other | the changed one |
| changed differently (add/add without base too) | the later `verified_at`, then the block first by bytes |

Side-symmetric; reads no code or spec file, computes no hash: each choice keeps an owner-accepted pair (drift possible, never a false `ok`); never rebuilds from the tree. Rows on stdout `<choice> | <node> | <relation> | <file> | <kind> <symbol>`, choice `both`, `ours`, `theirs`, `deleted`, `later`, `bytes`; stderr `merge spec.lock (<n> entries, <k> by verified_at or bytes) in <worktree>? [y/N]`, only `y`, `yes`. Writes the worktree file only, then stdout `next: git add spec.lock, then commit the merge`. Exit 0 written; 1 no conflict, declined, changed while running; 2 not a terminal, usage, discovery, config, no `[code]`, no worktree, a stage unreadable, neither ours nor theirs, recipes differ, a stage format 1.

## Travel and writes

Each lock travels on its worktree's branch; git's `index.lock` orders commits; branches meet in a merge (regen). Writes: the data directory, `spec.lock`, accept's one commit; no queue row, event or index change.

## Known limits

- A grammar (tree-sitter-rust, its ABI) or `LEVEL_RECIPE` bump changes `code_recipe`: such a lock, as a format-1 one, exits 2 at every `Some` reader until `spec lock rebase` (5b-2); regen too when only the base stage differs (both sides rebased since): `git checkout --ours spec.lock`, then accept the other side's nodes.
- Commits decay (rebase, squash, gc): a `commit` may name nothing; a renamed ID's entries stay until `accept <old ID>` drops them (`spec-lock-accept.md` "A gone node").
- Cooperative: a faked terminal passes (ADR-0034); the gate skips `spec.lock`.
