---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, plugin, fixtures]
ref: Phase 3 plan 2026-10-10, slice 3 (WA-1..WA-16; Q1-Q3 decided under the owner's pre-approval, review open), Q4 from `spec-verify.md` Q3; 05 s3.5, s5.3; 06 s4, s6; 07 s2; 08 s2
adrs: [ADR-0041]
---

# spec-lock: what was verified, written on the owner's terminal

## Why

Nothing ties a node's `norm_hash` at revision R to its bound items' AST hashes at a commit: slice 4 cannot tell `ok` from `code_ahead` (05 s5.3), slice 8 has nowhere to write a passing run (06 s6), and "rewrote the meaning, coverage stayed green" (05 s3.5) cannot be ruled out.

Delivered: a canonical, committed `<root>/spec.lock`; `spec lock accept REF [--editorial]` (committed bytes, a commit with provenance each) and `spec lock regen` (a merge resolver, no hashing); ADR-0041 names the writers (`architecture.md#apply`). Needs slices 1 (`code-refs.md`), 2 (`norm-hash.md`).

## Description and interactions

- **The owner**, on a terminal, after committing: `spec lock accept R-12` (a list, `[y/N]`, a commit); after a merge conflicting in `spec.lock`, `spec lock regen`.
- **An agent** never writes it: no MCP tool, route, UI stage or queue kind (WA-12); plugin roles say so (WA-16). **Not a spec file** (WA-1): a proposal would let an agent write hashes as its text.
- **Crates** (WA-13): core the model (parse, render, both merges; pure, pinned `toml`); store two git reads beside `WorktreeGit` (a blob OID at a commit, a path's unmerged stages); CLI the commands over slices 1-2 (walk, binding, `hash_item`, `norm_input`, `revision`). No new crate or dependency. **Unchanged**: `spec check` never reads the lock; hook triggers, CI, `INDEX_FORMAT`, MCP, HTTP, UI; no `spec lock show`; no `[code]` here, so no lock (WA-15).
- **Spelling** `spec lock regen`, not 05's `--regen`: owner-confirmed writes are verb subcommands with their own flags, exits and `require_terminal` name (`--staged` flags `round answer`, never a parent), so a bare `spec lock` stays a usage error.
- **Overridden live-spec lines**, fixed at shipping (SW-3; the convention binds canon, not planning specs): 05:20 "apply_proposal — one door" over `spec.lock`; 05:95 `owner | ci` (CI writes nothing); 05:99 "Generated", `--regen`; 05:103, 07:105 `spec bump --editorial` (only `spec lock accept --editorial`; `bump` never writes the lock); 07:106 `ID` -> `REF`, `regen`; 06:121 "lock updated" = the owner's accept (Q3).

## Data

### The file

WA-4. One per root, `<root>/spec.lock` (`architecture.md#layout`), rendered by SpecEngine, never by `toml`'s serializer:

```toml
# spec.lock: written only by SpecEngine (spec lock, spec verify --tests); do not edit
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

- `code_recipe`: `recipe_header()`'s parts (`RECIPE`, `TREE_SITTER_RUST_VERSION`, `abi_version()`) as `<RECIPE> tree-sitter-rust/<version> abi/<n>`; `norm_recipe` = `NORM_RECIPE`.
- Bytes: keys in the order shown; blocks by key (below); an empty line before each block; LF, a final LF; UTF-8, no BOM; basic strings escaping only `"`, `\` (`\"`, `\\`) and U+0000-U+001F, U+007F (`\b \t \n \f \r`, else `\u00XX`); full OIDs; `verified_at` the command's UTC date.
- Reader (any TOML layout): exit 2 `spec.lock:<line>: <message>` on another `format`; an unknown, missing or repeated key; a duplicate binding key; a wrong form (`b3:` + 64 lowercase hex; a 40- or 64-hex OID; `rev` 1-999 999 999; a known relation, kind; `item_kind`, `ast_hash` iff `kind = "item"`; a clean relative `file`; `verified_by = "owner"`; a date); another build's recipes (both named).

### Entry

WA-5. One `[[binding]]` per lockable marker binding; markers giving one key are one entry; mentions never.

| Key | Value |
|---|---|
| `node` | the canonical cited ID (`slug/AC-07` if feature-scoped), never an alias |
| `rev`, `spec_hash` | the node's revision R (an `unversioned` marker is locked at R) and `norm_hash`, at the commit |
| `relation`, `file` | `implements`, `verifies`, `configures`, `assumes`; root-relative, `/`-separated |
| `kind`, `symbol` | `item` + qpath, `module` + qpath, `ron` + field path (`root.stamina.regen_per_second`; `code-refs.md` "spec refs") |
| `item_kind`, `ast_hash` | items only: the tree-sitter kind; `b3:` + `Digest::to_hex` of the whole item (levels ignored) |
| `file_blob`, `commit` | the file's blob OID in HEAD's tree; HEAD's full OID |
| `verified_by`, `verified_at` | `"owner"`; `YYYY-MM-DD` |

Key and order: `node` bytes; `relation` as listed; `file`, `kind`, `symbol`, `item_kind` bytes. Until a later `format`, not recorded: levels and their hashes, fingerprints, anchor state, text hash, positions, `impl_status`.

### Lockability

WA-6, Q2. A binding `spec refs` lists under the node (`code-refs.md` "States") is locked only if each row allows; else `not locked: <state>`, never fresh (05 s5.3).

| Row | Locked | Never |
|---|---|---|
| `id_state` | `resolved`, `alias` | `ambiguous`, `homoglyph` |
| `rev_state` | `current`, `unversioned` | `spec_ahead`, `predated`, `bad_rev`, `rev_unknown` |
| binding | `item` with a v2 digest; `module`, `ron` by `file_blob` alone until slice 5 | `ambiguous <reason>`, `unanchored`, `cannot_verify` |

### spec lock accept

WA-2, WA-3, WA-7, WA-9, WA-10, Q1. In order:

1. `require_terminal("lock accept")` before anything is read.
2. Discovery, config, `[code]` (none: `code-refs.md`'s exact text); REF as `spec show` (a gone node: "A gone node").
3. Git: a worktree, HEAD born, on a branch, no operation in progress (`place`, `operation_in_progress`); the lock read.
4. Slice 1's walk finds the node's markers. **Inputs** tracked and clean, else each named with its state (`modified`, `staged`, `untracked`, `deleted`, `not tracked`), `commit them first`: the node's file, its markers' files, `specengine.toml`, `spec.lock`, and a holder deleted or renamed in the worktree whose HEAD blob marks the node (SW refinement: else its entries vanish).
5. Each input parsed, bound, hashed from its HEAD blob's raw bytes (`blob_at`, no filters); OIDs from HEAD's tree. The node's revision readable, `norm_hash` non-null.
6. **Merge** into the node's entries: lockable bindings fresh; one equal on `rev`, `spec_hash`, `ast_hash`, `file_blob` byte-identical (`commit`, `verified_at` too); not lockable now: the old entry kept; vanished: dropped. Refused: an old entry at R with another `spec_hash` (changed without a raise: `--editorial`, or a proposal raising `rev`). Other nodes' entries keep their values.
7. Nothing to change: no question; exit 0 `nothing to change` if some binding is locked as it is, else exit 1 saying why none can be.
8. stdout, key order: `<change> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ | <state>]`, change `new`, `refreshed`, `unchanged`, `kept`, `dropped`, `not locked`. stderr: `lock R-12 rev 3 (accept: 2 new, 1 refreshed, 1 kept, 1 dropped) at <commit> on <branch> in <worktree>? [y/N]`; only `y`, `yes`; no `--yes`.
9. Re-read HEAD, branch, lock bytes; a change: `spec.lock changed while being accepted; nothing written`.
10. Write atomically (`replace_file`; a first lock `intent_to_add`); `commit_only`, hooks run, never `--no-verify`: `spec: lock R-12`, an empty line, `Lock: R-12`, `Lock-mode: accept|editorial`, `Decided-by: <committer_ident>`; one parent, the old HEAD; only `spec.lock` changed; other staged paths stay staged. A failure restores the bytes (a new file removed).

### --editorial

WA-8. Sets `spec_hash` to the current `norm_hash` on the node's entries at R, nothing else (code hashes, `file_blob`, `commit`, `verified_*` kept); `Lock-mode: editorial`. Inputs: the node's file, `specengine.toml`, `spec.lock`; no walk (SW refinement). No entry at R (after a raise): exit 1; all match: exit 0 `nothing to change`. Asks `lock R-12 rev 3 (editorial: spec_hash of 3 entries) at <commit> on <branch> in <worktree>? [y/N]`.

### A gone node

`spec-verify.md` Q3. When the lock holds entries whose `node` is REF's bytes and no node has that canonical ID (deleted, or renamed with the old ID now an alias: SW refinement), accept drops exactly those entries: the exception to exit 1 "REF names nothing", checked once the lock is read (step 3). Steps 1-3, 9, 10 as accept; inputs `specengine.toml`, `spec.lock` only; no walk, no hash: a drop writes no hash, so no false `ok` follows (the bindings read `unlocked`). stdout `dropped | <relation> | <file> | <kind> <symbol>[ <item_kind>]`; asks `lock R-12 (gone: 3 dropped) at <commit> on <branch> in <worktree>? [y/N]`; trailers as accept's.

### spec lock regen

WA-11. Only while `spec.lock` is unmerged (`ls-files -u -z`); each stage present parses and shares `format` and recipes. Per entry key:

| Base -> ours, theirs | Result |
|---|---|
| same on both, or changed on one side | that entry |
| deleted on one side, unchanged on the other | deleted |
| deleted on one side, changed on the other | the changed one |
| changed differently (add/add without base too) | the later `verified_at`, then the block first by bytes |

Side-symmetric. Reads no code or spec file, computes no hash: each choice keeps an owner-accepted pair (drift possible, never a false `ok`); never rebuilds from the tree. stdout `<choice> | <node> | <relation> | <file> | <kind> <symbol>`, choice `both`, `ours`, `theirs`, `deleted`, `later`, `bytes`; asks `merge spec.lock (<n> entries, <k> by verified_at or bytes) in <worktree>? [y/N]`; writes the worktree file only; prints `next: git add spec.lock, then commit the merge`.

### Exits

WA-14; one-line messages (CLI README "Output").

| Exit | `accept` | `regen` |
|---|---|---|
| 0 | committed; nothing to change | written |
| 1 | REF names nothing (and no gone node's entries) or several; no marker names the node; nothing lockable; a dirty input; revision unreadable or `norm_hash` null; text changed at R; `--editorial`, no entry at R; an ignored `spec.lock` (before asking); not `y`; changed while accepting; a failed commit | no conflict; declined; changed while running |
| 2 | not a terminal; usage; discovery, config; no `[code]`; a walk gap; no git worktree; unborn or detached HEAD; an operation in progress; an unreadable lock, other recipes; no committer identity | accept's first four; a stage unreadable; recipes differ |

## Rules and edge cases

- WHEN one commit is accepted from two roots, another cwd, in reverse creation order, with `core.autocrlf=true` or through an alias marker, the system SHALL write identical bytes; a SHA-256 repository gives 64-hex OIDs, the same `b3:` hashes (WA-4).
- No absolute path, name, e-mail, time of day, abbreviated OID, line or column; IDs, paths, opaque symbols, the model's relations only (WA-5, WA-15).
- Each lock travels on its worktree's branch; git's `index.lock` orders commits; branches meet in a merge; daemon and gate never touch it (WA-10). Writes: the data directory, `spec.lock`, one commit; no queue row, event or index change.
- Each accept walks the code tree, hashing the node's items only; a `[sig]` marker gets the whole item's hash until slice 5.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- the core model is pure. M: `std::fs` -> `model_and_core_sources_do_no_file_io`.
- [ ] RD-2 -- bytes = the golden; a reordered, CRLF or re-spaced equivalent renders back. M: insertion order -> `lock_entries_are_sorted_by_key`; CRLF -> `lock_bytes_are_lf_with_a_final_lf`.
- [ ] RD-3 -- the reader. M: an unknown key accepted -> `lock_unknown_key_is_refused`.
- [ ] RD-4 -- terminal first. M: REF resolved before it -> `not_a_terminal_wins_over_a_bad_ref`.
- [ ] RD-5 -- `*.rs ident`, `"$Id$"` in a bound `fn`: the committed bytes' lock. M: worktree bytes -> `lock_hashes_the_committed_blob`.
- [ ] RD-6 -- each dirty input (modified, staged, untracked, a deleted holder) named, exit 1. M: only the node's file checked -> `dirty_bound_file_refuses_accept`.
- [ ] RD-7 -- "Lockability". M: `cannot_verify` locked -> `cannot_verify_binding_is_never_locked`; `spec_ahead` -> `marker_behind_the_revision_is_not_locked`; an ambiguous qpath -> `ambiguous_binding_is_not_locked`; RON `ast_hash` -> `ron_binding_is_locked_by_blob_only`.
- [ ] RD-8 -- step 6. M: a new `spec_hash` at R -> `meaning_change_without_a_revision_is_refused`; others re-rendered -> `other_nodes_entries_are_untouched`.
- [ ] RD-9 -- a second accept: exit 0, no prompt, HEAD unmoved. M: re-stamped -> `second_accept_changes_nothing`.
- [ ] RD-10 -- `--editorial`. M: `ast_hash` refreshed -> `editorial_keeps_code_hashes`; after a raise -> `editorial_after_a_revision_raise_is_refused`.
- [ ] RD-11 -- the commit. M: `--no-verify` -> `lock_commit_runs_hooks`; a staged path committed -> `lock_commit_takes_only_the_lock`.
- [ ] RD-12 -- step 9. M: no re-read -> `lock_changed_while_accepting_is_refused`.
- [ ] RD-13 -- regen. M: always ours -> `regen_is_symmetric`; a hash computed -> `regen_reads_no_code_and_no_spec` (code and spec removed, same output); exit 0 -> `regen_without_a_conflict_is_exit_1`.
- [ ] RD-14 -- no new crate. M: `sha1` -> the CLI pin in `build_graph.rs`.
- [ ] RD-15 -- writes per "Rules". M: a queue event -> `lock_accept_writes_no_queue_row`.
- [ ] RD-16 -- plugin roles and session: "... or writes a spec file" + "or the lock"; a PATCH after 0.7.0, `PINS`. M: version kept -> `plugin_files`.
- [ ] RD-17 -- "A gone node": a deleted node's and a renamed one's (old ID now an alias) entries dropped after `y`, one commit, other entries byte-identical. M: exit 1 "names nothing" -> `gone_node_entries_are_dropped`; the alias resolved first -> `alias_of_a_gone_id_drops_its_entries`.

test-engineer:

- [ ] TE-1 -- a temp repo from `fixtures/code-a/`, a golden lock: a document `rev: 3`, a section `rev=2`, a feature-scoped AC `@verifies`, an alias marker, a RON `@configures`, a `//!` marker.
- [ ] TE-2 -- "Lockability": each `id_state`, `rev_state`, binding kind, with its mutation.
- [ ] TE-3 -- every exit 1 and 2 cause; after each, `git status --porcelain=v1 -z --untracked-files=all` and HEAD unchanged.
- [ ] TE-4 -- determinism per "Rules". M: a path in a hash input.
- [ ] TE-5 -- no position, identity or time of day. M: a line -> `lock_has_no_positions`.
- [ ] TE-6 -- editorial: at R, after a raise, all matching, dirty code ignored.
- [ ] TE-7 -- regen after a real `git merge`: add/add, modify/modify, delete/unchanged, delete/modify, identical sides, other headers (exit 2); swapped sides, same file; non-adjacent blocks merge cleanly.
- [ ] TE-8 -- trailers parse by `git interpret-trailers --parse`; a hook's file proves hooks ran; a failing hook restores the bytes.
- [ ] TE-9 -- `spec check` equal with and without `spec.lock`; `.githooks/pre-commit` untouched. M: check reads the lock.
- [ ] TE-10 -- `fixtures/spec-c` exits 2 naming `[code]`; the core module holds no fixture prefix, kind, `function_item`, `.rs`, `crate::`.
- [ ] TE-11 -- pilots, manual (`#[ignore]`, release, foreground), a scratch clone: counts by state; no pilot name or path committed.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- ADR-0041, `architecture.md` `#apply` and `#layout` in the same change.
- [ ] SW-3 -- at shipping: Tier 2 `docs/canon/spec-lock.md`; the overridden lines (05 s3.5 a format pointer), 06:140, 08:44; CLI, store, core READMEs; `plugin.md:38`, `:47`; `CLAUDE.md` State.
- [x] SW-4 -- `export index`, `check` clean.

## Risks

- A grammar bump (tree-sitter-rust 0.24.2, ABI 15) re-hashes every item; accept refuses such a lock (exit 2) until slice 5.
- Commits decay (rebase, squash, gc): Phase 4's diffs show "history unavailable"; a renamed ID's entries read `node_gone` (slice 4a) until `accept <old ID>` drops them.
- Friction: commit first; a prompt and commit per node; ~1 % of valid code fails tree-sitter-rust.
- Cooperative: a faked terminal passes (ADR-0034); the gate skips `spec.lock`; hooks run on the lock commit; stdin signing fails; LFS filters hash pointer bytes.

## Out of scope

Slice 4a (`spec-verify.md`, ADR-0042): comparing (cascade, matrix, verdicts), lock findings, orphans. 4b `rev-discipline`: the revision rule, `spec bump`, editorial apply (ADR C), the hook trigger. 4c `drift-queue`: a drift queue kind (ADR D). 5: levels, fingerprints, anchors, the RON recipe, rebase. 6/7: lock data in reads. 8: `verify --tests` writing, `impl_status` (ADR F). Phase 4: history diffs. Not planned: a merge driver, a `spec.lock` gate rule, several REFs, UI staging, a queue event.

## Open

| Point | Answer |
|---|---|
| Q1 who commits | accept, the lock alone, hooks run, inputs from a clean HEAD |
| Q2 RON and modules | by `file_blob` alone until slice 5 |
| Q3 naming writers | by consent: the owner's `[y/N]`; a later confirming step (ADR C, D) runs the same accept |
| Q4 a gone node's entries (`spec-verify.md` Q3) | `accept <gone ID>` drops them, same `[y/N]` and commit |

Decided under the owner's pre-approval, review open; WA-1..WA-16: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. To record: modules, TE-11's figures, deviations with reasons.
