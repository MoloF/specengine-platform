---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-20
---

# `spec lock rebase`: an old lock carried into this build's

The migration writer (ADR-0050, `architecture-code.md#spec-lock`; CLI `lock_rebase.rs`), named by every `Some` reader (`spec-lock.md` "Entry"): a lock of this build's `norm_recipe`, format 1 or another `code_recipe`, made format 2 at this build's recipes from each entry's `file_blob`.

## Steps

`spec lock rebase [--json]`; accept's refusals (`spec-lock-accept.md` "Exits"): exit 2 off a terminal (nothing read), on usage, config, `[code]`, git, an unmerged lock; 1 if `spec.lock` or the config is not `HEAD`'s. Then:

1. No lock: exit 1 `no spec.lock: nothing to rebase`; this build's: exit 0 `nothing to change`; not UTF-8, unreadable by `None`, another `norm_recipe`: exit 2, the reader's text (the last `Some`'s).
2. Each `file_blob` by OID, one `read_blobs` session (raw `cat-file --batch`, `GIT_NO_LAZY_FETCH`; failing: exit 2); never `HEAD`'s tree, index, working tree or `commit`. A blob parses once; a panic or no tree: exit 2 naming file and OID.
3. Per entry, key order: located, given levels, hashed as accept's, or dropped; all but its code kept.
4. Rows, the question, re-reads (changed: exit 1 `spec.lock changed while being rebased; nothing written`), the commit.

## Location, levels, drops

In the blob: `item`, items of its `item_kind` whose in-file path (no package, `#<disc>` kept) is `symbol` or its `::` suffix; `module`, the file's module always, inline `mod {}`s so named (not `mod x;`); `ron`, comments anchored at the field path (several: the first's digest). Ties: those a blob marker binds for the node (ID or working-tree alias) and relation; one left wins. A marker on an item with a parse error binds no unit.

Levels: format 2 its own; format 1 the union of the blob's markers giving the key (none: `sig, body`). Only a format-1 lock at this build's whole-item recipe is cross-checked, each `item`'s `ast_hash` again by `hash_bound_items`. Others, even empty: `note: rebase: "<old code_recipe>" is not this build's: located without a hash cross-check`.

Dropped and named (then `unlocked`), first match: `blob_missing` (no object, another type, no OID), `not_utf8`, `symbol_gone` (no candidate), `ambiguous` (ties leave none or several; an `Ambiguous` RON anchor), `bad_levels` (format 1: a marker giving the key, `Invalid` levels), `cannot_verify[ <category>]`, `hash_mismatch` (the cross-check fails or gives no digest).

## Output and the commit

stdout: `rebased | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]`, `dropped | … | <reason>`; `committed <oid>`. stderr: `rebase spec.lock (format <n> -> 2, "<old>" -> "<new>": <r> rebased, <d> dropped) at <commit> on <branch> in <worktree>? [y/N]`; not `y`, `yes`: exit 1 `not confirmed; nothing written`. `--json` (exits 0, 1): `from{format,code_recipe}` (null: no lock read), `to`, `counts{rebased,dropped}`, `rows[{change,node,relation,file,kind,symbol,item_kind,levels,reason}]`, `commit`, `unchanged`, `refusal`.

Accept's commit (`spec-lock-accept.md` "The lock commit"; exit 0 only verified): `spec: lock rebase`, `Lock-mode: rebase`, `Decided-by: <committer_ident>`, no `Lock:` or `Proposal:` (no `drift` item completed); check 3: ``it has no `Lock-mode: rebase` trailer``, ``it has a `Lock:` trailer``.

## The base

Verify's `HEAD` lock (`spec-verify.md` "Base"; `--changed`, `--staged`): `Some` first; else, at this build's `norm_recipe`, a format-1 or other-`code_recipe` lock gives way to a judged lock of this build's (the disk's, the index's under `--staged`; absent: empty), so a rebase is Pre-existing; `note: HEAD's spec.lock is format <n> ("<code_recipe>"): judged with the current lock`, on stderr and in `notes`. Else (unreadable, other `norm_recipe`) cause (7); a judged lock not this build's: cause (6) alone.

## Known limits

- A module and its inline `mod` marked for one node and relation: the `mod`'s entry drops `ambiguous`, the module's too if the `mod` ends its symbol (`mod a` in `a.rs`).
- git < 2.44 ignores `GIT_NO_LAZY_FETCH`: a partial clone may fetch a missing blob.
- From the binary upgrade to the rebase commit a project's verify hook refuses every commit (cause 6), under `enforce` or plain `spec verify` a Fix row that one too: switch to `enforce-introduced` before the upgrade (`HEAD`'s stricter mode counts), else it takes `--no-verify` or the hook off.
- A panic hashing or naming reads "the Rust parser failed on …" (`lock-levels-verify.md` RD-9).
