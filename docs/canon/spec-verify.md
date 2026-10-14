---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli]
owner: owner
reviewed: 2026-10-13
---

# `spec verify`: drift between `spec.lock` and the tree

`spec verify [--changed] [--json]` (+ `--root`, `--config`) judges every lock key against the tree: a state per row, findings Fix, Check or Pre-existing, exit 0, 1 or 2 under `[verify] mode` (ADR-0042, `architecture-code.md#verify`). Read-only: it writes only the data directory (the index refresh), never `spec.lock`, the worktree, a git object, a queue row or an event. `spec check`, this repository's hook, CI, MCP, HTTP, UI and the gate never run it. Judge: core `verify` (`VerifyConfig`, `verify_from_toml`, `RowKey`, `judge_row(&RowFacts)`, `verdict`, `is_pre_existing`), pure; git: store `VerifyGit`; command: CLI `verify.rs`, `verify_rows.rs` (their READMEs). Its text, hints and JSON: `docs/canon/spec-verify-output.md`.

## The `[verify]` table

```toml
[verify]
mode = "enforce-introduced"   # observe (default) | enforce-introduced | enforce
```

Closed, `mode` only: `tests`, `timeout` (`is not built`, 8a), another key, a wrong type or value give `specengine.toml:<line>: message`, run cause 1. Other commands only name-check `verify` (`ProjectConfig`). No table: `observe`.

## Rows

One row per lock key (`spec-lock.md` "Entry": `node`, `relation`, `file`, `kind`, `symbol`, `item_kind`) from a lock entry or a marker binding of the walk; markers giving one key are one row. Keys come from accept's builder (CLI `lock::Target`): the canonical cited ID, never an alias. An unresolved ID (dangling, look-alike, ambiguous) keys on its ID part, without `@rev`: `X@2` and `X@3` on one item are one row, a gone node's leftover marker joins its entry's row; a bare `// @implements` has node `""`; a marker resolving to a node without an ID keys on its token and stays `unlocked`. A keyless marker (`unanchored`, a Rust `cannot_verify`) is one row per node, relation, file, state and category, without kind, symbol or item kind. Order: core `EntryKey`'s, keyless first. Skipped and counted: `queue`, `other_project` IDs. Mentions never make a row; nodes without bindings are not listed (Health rows 4-5).

## States

Per row: R, H the node's revision and `norm_hash` (`norm_input` + `b3_hash` over a fresh parse of the bytes read, `HEAD`'s blob for the base; never the index's column; IDs by `spec show`'s resolver); E its lock entry (`rev_L`, `spec_hash_L`, `file_blob`, `ast_hash`); N each marker's `@N`. **Code changed**: E exists, the file's blob OID differs from `file_blob` and, for an `item`, the re-hashed item from `ast_hash` (`module`, `ron`: the blob alone). **Spec changed**: (R, H) differs from (`rev_L`, `spec_hash_L`). **Raised**: a marker names R > `rev_L`. **Lagging**: a marker N < R. The first row that applies:

| State | Category (detail) | When | Class |
|---|---|---|---|
| `cannot_verify` | `item_error` (the hash's first `ErrorCategory`) | the bound item `has_error()` (`code-bindings.md` "Rust binding" rule 7); E's item now broken beside a keyless `cannot_verify` marker of its node and relation (accept's kept entry); RON `nesting_too_deep` | Cannot |
| | `ambiguous_symbol` (`path_attribute`, `unrooted`, `duplicate`); `ambiguous_id` | an ambiguous qpath or RON path; an ID naming several nodes | |
| | `rev_unknown`; `hash_null` | R unreadable; H null (`node-revision.md` "Null") | |
| | `unreadable` (`unreadable`, `not_utf8`, `outside_code`) | E's file: metadata unreadable; not UTF-8; on disk but no file the walk reads | |
| | `hidden_by_error` | E's symbol missing from a file with an orphan parse error | |
| `predated` | | a marker N > R | Fix |
| `bad_marker` | `dangling`, `homoglyph`, `bad_rev`, `unanchored` (its category), `bad_levels` | `code-bindings.md` "ID state", "Revision state", "Rust binding" | Fix |
| `broken_binding` | `node_gone`, `file_gone`, `symbol_gone`, `marker_gone` | E, no binding: no node's `id` is `node` (deleted, renamed, an alias now); the file absent; the symbol absent; no marker names it | Fix |
| `unlocked` | (`key_problem`) | a key `key_problem` refuses (`spec-lock.md` "Entry"); else no E, not lagging | Check |
| `spec_ahead` | `marker_behind`; `spec_changed`, `same_revision` | lagging, code unchanged; spec changed, not raised, code unchanged (R = `rev_L`: `same_revision`) | Check |
| `code_ahead` | `code_changed`; `raised` | code changed, spec unchanged; spec changed and raised | Check |
| `conflict` | | spec and code changed, not raised | Fix |
| `ok` | | neither changed, not lagging | |

`compared`: `blob`, `ast`, or null (no E, or decided before comparing). A node renamed away: its leftover marker makes the row `bad_marker dangling`; with the marker fixed it reads `node_gone`, and `spec lock accept <old ID>` drops the entries (`spec-lock-accept.md` "A gone node").

## Cascade

Level 1, the blob OID: on disk one `git hash-object --no-filters --stdin-paths` (never `-w`) over the files of rows with an E; at a commit its tree's OID. Equal: unchanged, nothing parsed or hashed. Level 3, an `item` whose blob changed: its file re-parsed from the walk's text and the whole item hashed (`hash_bound_items`, levels ignored until 5b), never the walk's item state. No text level: 05 s5.3's fallback would pass an unparseable item. One code walk (`read_tree`) per call; only bound files whose blob changed are parsed again.

## Base

`--changed` reads `HEAD` through `VerifyGit` on the check's base reader (`rev-parse`, `ls-tree -r -z`, one `cat-file --batch`; the caller's `GitEnv`, `GIT_OPTIONAL_LOCKS=0`): the documents `spec check --changed` reads (`spec-check-git.md` "The changed check"); `.rs`, `.ron` under the code roots; every `Cargo.toml` under a root or in a directory at or above one; `spec.lock` (absent: no E); the config, for its mode. Judged with the current config, scheme and `[code]`; a file's binding is reused when its bytes and every manifest are equal, else bound from `HEAD`'s blob. **Pre-existing**: `HEAD` gives the same key, state and category. Unborn `HEAD`: every row introduced, base `unborn`. Mode: the stricter of the current and `HEAD`'s (a `note:` when `HEAD`'s is stricter or unknown). A merge or rebase in progress: still `HEAD`. **Unmerged**: every run, plain too, asks the session for `:1:` to `:3:./spec.lock`, stages only an unmerged path has (delete/modify too).

## Verdict and exit

| Verdict | Exit | When |
|---|---|---|
| `clean` | 0 | every row `ok` |
| `observed` | 0 | findings, none blocking |
| `blocked` | 1 | `enforce`: a Fix row; `enforce-introduced`: a Fix row not Pre-existing |
| `cannot-verify` | 2 | a Cannot row or a run cause, in every mode |

Check rows never block. Plain, `enforce-introduced` has no base and is judged as `enforce`, with ``note: mode `enforce-introduced` has no base without --changed: judged as `enforce` ``. Usage or discovery failing: exit 2, stdout empty. After discovery every failure is a **run cause**: the report, no row judged, its `cannot:` lines in this order: (1) a config error, `[code]`'s or `[verify]`'s; (2) no `[code]` (`code-bindings.md` "The `[code]` table"'s text); (3) a walk gap, a walked file `fs::read` cannot read included; (4) no worktree, no `git`, a git failure; (5) `spec.lock is unmerged: run spec lock regen`; (6) `read_lock`'s: `cannot read spec.lock: <error>`, `spec.lock: not UTF-8: SpecEngine writes it`, `spec.lock:<line>: <message>`, another `format` or recipe (both named); (7) a partial base, `HEAD`'s lock invalid; (8) `HOME` or the data directory. Every check that can run does (the walk whenever `[code]` reads), so the first line is the first cause present; 7a's degraded read names it (`bundle-bindings.md` "Degrade").

## A project's hook

A project hooking verify into its commits runs `spec verify --changed` under `enforce-introduced` (4b-2 `rev-rule`: `--staged`). Accept's lock commit runs hooks and covers one node, so a plain or `enforce` hook refuses it while another node has a Fix row (two `conflict` nodes deadlock). This repository's hook runs `spec check` only.

## Known limits

- Under `core.autocrlf` or `eol` attributes, `module` and `ron` rows read `code_ahead code_changed` by blob (`--no-filters` hashes the disk's CRLF, the lock holds `HEAD`'s LF blob), as does an item whose multi-line literal holds CRLF; other items verify. 5b's digests lift it (`lock-levels.md`).
- Under `ident` or LFS an expanded `$Id$` in a bound item, an LFS file read `code_ahead`: accept hashes `HEAD`'s raw blob, verify the disk.
- A RON entry without its marker is `marker_gone`, never `symbol_gone` (the walk keeps comment anchors only). An unclosed `impl X {` before a locked fn re-parents it: `symbol_gone` plus `unlocked`, not exit 2. A stray line between a marker and its item is skipped by the binding.
- A grammar bump, or a bound item among the ~1 % of valid Rust tree-sitter-rust fails, holds exit 2 until 5b's rebase or the marker moves. `[sig]` is hashed whole until 5b. On adoption every binding is `unlocked`.
