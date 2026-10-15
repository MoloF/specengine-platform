---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-code, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b (WA-1..WA-13; Q1-Q8 decided under the owner's pre-approval, review open); 05 s3.5, s5.2, s5.3; 07 s2; 08 s2, AC-2, AC-14
adrs: [ADR-0050]
---

# lock-levels: spec.lock format 2, level-aware verify, `spec lock rebase`

## Why

Format 1 (`docs/canon/spec-lock.md` "Entry") keeps one v2 `ast_hash` per item and judges a `module` or `ron` binding by its blob alone: declared levels are dead (`[sig]` reads a body edit as drift, as `[sig, body]` does), and any edit of a RON file or a module's file is drift for everything bound in it: 05:137's "every refactor an alarm the owner learns to ignore".

Nor can anything carry a lock across a change of format or recipe (WA-1): accept exits 2 on other recipes, regen needs one format and recipe set, a hand edit is forbidden (ADR-0041). A grammar bump strands every lock (`spec-lock.md` "Known limits"), and ADR-0042's "until … re-accepted" cannot happen as built. 5b is itself such a change.

Delivered: `format = 2` (levels, every level's digest, 5a's fingerprints; no `ast_hash`), accept and verify reading levels, and `spec lock rebase`, the first migration writer (ADR-0050; `architecture-code.md#spec-lock`). After 5a-2 (5a-1's code it calls, adding no hashing: WA-2), 4a and 6a (`symbol-index.md` Q11); before 8b, 5c, 6b, 7a, 7b.

## Description and interactions

- **The owner** meets an old lock as verify's exit 2 with a rebase hint, and runs `spec lock rebase` on a terminal: a row per entry, one question, one verified commit. **An agent** sees `code_ahead code_changed <levels>`, `code_ahead levels_changed` in verify and reads' `sync`; it never rebases (no MCP tool or plugin command).
- **Crates** (WA-12: core sees digests, level names, `body_bytes` only). Core: the format-2 model (`parse_lock`, `render_lock`, `same_pair`), the level comparison, its own copy of the four names (pinned against `markers.rs`); no tree-sitter. Code: 5a-1's `item_levels`, `module_levels` (`hash-levels.md` "Entry points"), 5a-1b's `ron::value_hashes` (`hash-levels-ron.md` "Entry point"); the blob-local locator. Store: blobs by OID through the existing `cat-file --batch` session. CLI: accept, verify, regen, `build_recipes`, a new `lock_rebase.rs`; `write_and_commit`, `check_lock_commit` take a subject (a node, or the rebase). No new crate or dependency.
- **Unchanged**: MCP, HTTP, UI, plugin, eval, the gate, `spec check` (4b-2's `rev-not-raised` reads format 1's and 2's `node`, `rev`, `spec_hash` alike: `rev-rule.md` Q9), the index, the queue. **Nothing is blocked** (ADR-0012): spec work, the gate and MCP never read the lock; reads show `sync` unknown with a note, Health a cause; only a project's own `[verify]` CI or hook stops. **One door** (ADR-0050): rebase writes `spec.lock` alone, never via MCP, a role, a hook, CI or the daemon; its commit settles no `drift` item (completion needs `Proposal:` and `Lock:`; WA-9).
- **4a's lines overridden at 5b's shipping** (WA-11; 4a shipped them as written): `docs/canon/spec-verify.md` "States" (code changed by levels, every kind; `code_ahead levels_changed`); "Cascade" (level 3 by levels, `module`, `ron` too, `compared` `ast`); "Verdict and exit" cause (7) and "A project's hook" (Q8 here); "Known limits"' autocrlf and noise lines; `docs/canon/spec-verify-output.md` "Hints" (4a gives a run cause no hint; cause (6) gains the rebase hint). For its implementation, no text change: (a) core's judge takes a code-comparison outcome from its producer (`unchanged`, the changed levels, `cannot_verify`), never two `ast_hash` strings; (b) the base lock source stays pluggable (its RD-10 seams); optionally `"levels": null` reserved per JSON row. Other overridden lines: SW-4.

## Data

### Format 2

WA-3..WA-6.

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

- Header: `code_recipe` = `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>` (`build_recipes` leaves v2's `RECIPE`); `norm_recipe` and the comment line (it names `spec lock`) unchanged. The entry key is unchanged (core `EntryKey`; WA-3): regen's merge, 4a's rows and 4c-1's items keep theirs.
- Keys in this order (WA-4: `*_hash` as `spec_hash` and 05:152's `body_hash`): `node, rev, spec_hash, relation, file, kind, symbol, item_kind?, levels, sig_hash, body_hash, deps_hash, skeleton_hash?, quote_hash?, body_bytes?, file_blob, commit, verified_by, verified_at`.

| Key | Value |
|---|---|
| `levels` | non-empty, distinct, in the canonical order `path, sig, body, deps` (`code-identity.md` "Marker grammar"): the union of the effective levels of the markers giving the key, `Default` as `sig, body` |
| `sig_hash`, `body_hash`, `deps_hash` | every kind, every level, in `levels` or not (a widened marker is judged against them): `item` 5a's `item_levels`; `module` `module_levels`, an inline `mod`'s `mod_item` by `item_levels` (`hash-levels.md` "File module"; SW refinement); `ron` sig = body = `value_hash`, deps the empty set's digest (`hash-levels-ron.md` "RON value") |
| `skeleton_hash`, `quote_hash`, `body_bytes` | `item` and `module` only; `body_bytes` a TOML integer, 0 to 4 294 967 295; stored for 5c, never compared by 5b (WA-13) |
| `ast_hash` | gone: `sig` + `body` see exactly what v2 saw (`hash-levels.md` "Levels") |

- Bytes as format 1 (`spec-lock.md` "The file"), plus `levels = ["sig", "body"]` (basic strings, `", "` between, no trailing comma) and `body_bytes` in decimal (no sign, `_` or leading zero).
- **Reader** (`parse_lock`): format 1's rules for shared keys; `levels` as above, else `spec.lock:<line>: levels: <empty|unknown <name>|duplicate <name>|order>`; the three digests always; `skeleton_hash`, `quote_hash`, `body_bytes` iff `kind` is `item` or `module`; `ast_hash` an unknown key. No reader upgrades a lock in memory (WA-5): format 1 is read by `spec lock rebase` alone, every other caller exits 2 with the rebase hint (but 4b-2's rule, which reads its spec side: `rev-rule.md` Q9); above 2: `spec.lock: format <n> is newer than this build's 2`.

### Accept

`spec-lock-accept.md` "Steps" 5-6 under format 2. Step 5 hashes each lockable binding into levels as above. **Lockability** (`spec-lock.md`), rows in this order, the first refusal in marker order naming one:

| Row | Locked | Never |
|---|---|---|
| `id_state`, `rev_state` | as format 1 | as format 1 |
| `levels` (new) | `Default`, a declared list | `Invalid` (`unclosed`, `unknown`, `duplicate`, `empty`): `not locked: bad_levels` |
| binding | `item`, `module` with levels hashed; `ron` with a `value_hash` | `ambiguous <reason>`, `unanchored`, `cannot_verify`: format 1's, plus a `module` whose file has any parse error, a `ron` value of a file `RonAnalysis` rejects |

**Merge**: `same_pair` (core `lock.rs`) compares `rev`, `spec_hash`, `levels`, the three digests, the fingerprints, `body_bytes`, `file_blob`; equal is `unchanged`, byte-identical; a changed `deps_hash` alone refreshes. `--editorial` and 4b-1's editorial apply keep every code field byte for byte.

### Level-aware verify

WA-10, WA-11, Q5, Q6. Per row with a lock entry E and bound markers: L_lock = E's `levels`; L_mark = the union of the markers' effective levels; C = (L_lock ∪ L_mark) minus `path`. The first that applies:

1. A marker's levels `Invalid`: 4a's `bad_marker bad_levels`, in its order of states.
2. The blob equals `file_blob`: code unchanged, nothing parsed (`compared: blob`). Markers bind in their own file (WA-10): L_mark moves only through an ID's resolution (a new alias), and L_lock ≠ L_mark then reads `levels_changed` (SW refinement).
3. Else the digests from this run's parse, as accept's. `CannotVerify`: `cannot_verify item_error`, detail the first `ErrorCategory` of the item or a module's file; a RON file `RonAnalysis` rejects: its first `error_categories` entry (`hash-levels-ron.md` Q7).
4. C's digests against E's; those differing, in canonical order, are `changed`. Non-empty: code changed, `code_changed`, detail `changed` joined by `,` (`body,deps`). Empty and L_lock ≠ L_mark: code changed, `levels_changed`, detail `<L_lock>-><L_mark>` (`sig,body->sig`). Else code unchanged.

- **4a's matrix stands**, "code changed" now steps 2 and 4: spec unchanged `code_ahead code_changed|levels_changed` (Check, the hint accept's); spec changed and raised `code_ahead raised`; changed, not raised `conflict` (Fix). Q6: `levels_changed` is Check.
- `path` is never compared (5c's `MOVED`): a `[path]`-only binding is `ok` under any edit but a levels change; a sibling RON value's or a neighbour's edit is `ok`.
- Text: only the new category and detail (`check | code_ahead levels_changed sig,body->sig | R-12 | …`). JSON: `compared` keeps `blob`, `ast` (digests compared, any kind), null; each row gains `levels` after it, `{"lock":["sig","body"],"markers":["sig"],"changed":[]}`, null without E or a bound marker, `changed` null when nothing was compared.

### Old locks and the base

Q3, Q8.

- **Format 1, or another `code_recipe`**: 4a's run cause (6), then `hint: spec.lock: run spec lock rebase on a terminal` (4a prints no hint for a run cause). Accept, `--editorial` exit 2: `spec.lock: format 1 ("<recipe>"), this build writes 2 ("<recipe>"): run spec lock rebase on a terminal`; 4b-1's editorial apply too, on format 1 only (it needs this build's `format` and `norm_recipe`). Reads: `sync` unknown, `note: bindings: <the cause>`. Another `norm_recipe`: exit 2, no hint.
- **Regen** writes format 2 only: a format-1 stage exits 2 with the hint; format-2 stages sharing another `code_recipe` merge as today (no hashing); differing stages exit 2 (abort, rebase the lagging branch, merge again).
- **Base** (Q8; overrides 4a's cause (7)): under `--changed` (4b-2: `--staged`), a `HEAD` lock of format 1 or another `code_recipe` beside a current lock of this build's: `HEAD`'s rows are judged with the current lock, `note: HEAD's spec.lock is format <n> ("<recipe>"): judged with the current lock`. A rebase changes no key, `rev`, `spec_hash` or `file_blob`: a project hook running `spec verify --changed` under `enforce-introduced` (4a's Q7) passes its commit. Unreadable, another `norm_recipe`: still cause (7).

### `spec lock rebase`

WA-7, WA-8, Q1, Q2, Q4. `spec lock rebase [--json]` (+ `--root`, `--config`):

1. Not a terminal: exit 2, nothing read. Usage, discovery, config (`--config` naming another file too), no `[code]` (`spec refs`' text): exit 2.
2. Git: a worktree, `HEAD` born, on a branch, no operation in progress (unmerged `spec.lock`: `spec lock regen` first): else exit 2.
3. **Inputs** `spec.lock` and `specengine.toml` (accept's gone-node set), tracked and equal to `HEAD`, else each named with its state, exit 1 `…: commit them first; nothing written`; no lock: exit 1 `no spec.lock: nothing to rebase`. No walk; dirty code ignored.
4. The lock read, formats 1 and 2: unreadable, newer, another `norm_recipe` (no spec blob to re-derive `spec_hash`): exit 2. Already this build's format and recipes: exit 0 `nothing to change`, no question.
5. **Each entry**, in key order, from its own `file_blob` (WA-7): one `cat-file --batch` session (failing: exit 2), raw, no filters, no lazy fetch; a blob several entries name parsed once; never `HEAD`'s tree, the index, the working tree, nor `commit` (it decays). Located and hashed ("Location"). **Levels** (Q4): format 2 keeps its own; format 1 takes the union of the effective levels of the blob's markers giving its key (node by token or `spec show`'s resolution, its relation, binding the located unit), `Invalid` ones adding nothing (verify shows them; SW refinement); none: `sig, body`.
6. Kept byte for byte: key, `rev`, `spec_hash`, `file_blob`, `commit`, `verified_*`; written: `levels`, digests, fingerprints, `body_bytes`, the header. **Dropped** (Q1): named, no hash written, so verify shows `unlocked`.
7. Committer identity (else exit 2); rows on stdout; the question on stderr, only `y`, `yes`, no `--yes`; `HEAD`, branch, lock bytes re-read (changed: exit 1 `spec.lock changed while being rebased; nothing written`); the commit.

**Location** (Q2), inside the blob:

| `kind` | Candidates | Hashed by |
|---|---|---|
| `item` | items of `item_kind` (`code-bindings.md` "Rust binding") whose in-file name (inline `mod`s, owner, name, rendered as the walk renders a `qpath`) is `symbol` or ends it at a `::` boundary | `item_levels` |
| `module` | the file's module; each inline `mod` whose in-file path ends `symbol` at a `::` boundary | `module_levels`; the `mod_item`'s `item_levels` |
| `ron` | the anchor of `symbol`'s field path (`code-identity.md` "RON binding") | `ron::value_hashes` (`hash-levels-ron.md`) |

- **Ties**: of several candidates, those a marker of the blob binds whose token, or `spec show`'s resolution of it, is the entry's `node`, with its `relation`; one left is located, else `ambiguous`.
- **Cross-check**: a format-1 `item` entry under this build's own v2 recipe (`RECIPE tree-sitter-rust/<v> abi/<n>`) must give its `ast_hash` again via `hash_item`, else `hash_mismatch`. Under another recipe nothing can be cross-checked: the location stands, with one `note: rebase: "<old code_recipe>" is not this build's: located without a hash cross-check`.

| Drop reason | When |
|---|---|
| `blob_missing` | no object of that OID (gc, a partial or shallow clone) |
| `not_utf8` | the blob is not UTF-8 |
| `cannot_verify <category>` | the located unit gives `CannotVerify` (a RON file: any rejection) |
| `symbol_gone` | no candidate |
| `ambiguous` | the markers leave several candidates or none; an `Ambiguous` RON anchor |
| `hash_mismatch` | the cross-check fails |

**Output**: stdout rows in key order, `rebased|dropped | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ | <reason>]`, ending `committed <oid>`; stderr `rebase spec.lock (format 1 -> 2, "<old code_recipe>" -> "<new>": 10 rebased, 1 dropped) at <commit> on <branch> in <worktree>? [y/N]`, both counts always.

```
rebased | GOAL-01 | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::value function_item
dropped | R-20 | configures | assets/data/movement.ron | ron root.stamina.regen_per_second | blob_missing
committed 3f9a1c0<33 hex>
```

`--json` (exits 0, 1, and 2 after discovery; every key present, absent null; `refusal` as accept's): `{"from":{"format":1,"code_recipe":"…"},"to":{"format":2,"code_recipe":"…"},"counts":{"rebased":10,"dropped":1},"rows":[{"change":"dropped","node":"R-20","relation":"configures","file":"assets/data/movement.ron","kind":"ron","symbol":"root.stamina.regen_per_second","item_kind":null,"levels":null,"reason":"blob_missing"}],"commit":"<oid>","unchanged":false,"refusal":null}`.

**The commit**: atomic write (`replace_file`), `git commit --only spec.lock`, hooks run, never `--no-verify`: `spec: lock rebase`, an empty line, `Lock-mode: rebase`, `Decided-by: <committer_ident>`; no `Lock:` trailer, no caller trailers (no queue item runs a rebase: ADR-0045 runs accept). Accept's four checks, the third "a `Lock-mode: rebase` trailer" for `Lock: <node>`; exit 0 only for a verified commit; accept's failure table and restore rules (`spec-lock-accept.md` "The lock commit"). `LockConsent`, `LockPrompt` reused; no `&[LockTrailer]`. Exits: as the steps say; a commit failed, failing a check or unverified: 1; exit 0 only for a verified commit or `nothing to change`. After a refusal `HEAD`, refs, index, status and files are as before.

### Fixtures and migration

WA-6: no real format-1 lock exists (no `[code]` here, no pilot marker, slice 3's pilot accept never ran). `fixtures/spec-lock/code-a.lock` (pinned by `lock_accept.rs`) becomes accept's format-2 golden; its format-1 bytes move to `fixtures/spec-lock/code-a.format-1.lock`, rebase's input; 4a's RD-10 keeps its path.

Until this slice's `levels` row (RD-3 `invalid_levels_are_not_locked`) accept locks a marker whose levels are `Invalid`: the format-1 golden's 11 entries hold `counter.rs:77` `[path, path]` and `bounds.rs:14` `[Sig]`; the format-2 golden leaves both out (9), and 4a's RD-10 reads them `bad_marker bad_levels` either way (`spec-verify.md` Q8).

## Rules and edge cases

- WHEN a rebase runs, the system SHALL read code only from each entry's `file_blob` by OID and SHALL drop, never guess, an entry it cannot re-derive (Q1).
- WHEN a marker narrows its levels, verify SHALL compare the union with the lock's (Q5); WHEN the lock is of another format or recipe, no row SHALL be `ok` (AC-14).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, accept, verify and rebase SHALL give byte-identical locks and output.
- No stack, kind or prefix in rows, hints or the lock (WA-12).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- "Format 2". M: `ast_hash` accepted -> `format_2_refuses_ast_hash`; a RON fingerprint -> `ron_entry_has_no_fingerprint`; unsorted levels -> `levels_out_of_canonical_order_refused`; a digest outside `levels` omitted -> `every_level_digest_is_written`.
- [ ] RD-2 -- "Old locks and the base". M: format 1 judged -> `format_1_lock_exits_2_with_the_rebase_hint` (verify, accept, `--editorial`, regen); `HEAD`'s format-1 lock as cause (7) -> `head_lock_of_another_format_is_judged_with_the_current_lock`.
- [ ] RD-3 -- "Accept". M: the first marker's levels only -> `levels_are_the_union_of_the_markers`; `Invalid` as the default -> `invalid_levels_are_not_locked`; a digest ignored by `same_pair` -> `changed_deps_digest_refreshes`.
- [ ] RD-4 -- "Level-aware verify". M: whole-item comparison -> `sig_marker_ignores_a_body_edit`; markers-only set -> `narrowed_marker_with_a_body_edit_is_code_changed`; unequal sets as `ok` -> `widened_marker_is_levels_changed`; `path` compared -> `path_only_binding_ignores_a_body_edit`; RON by blob -> `ron_sibling_edit_is_ok`; a broken RON file `ok` -> `broken_ron_file_binding_cannot_verify`.
- [ ] RD-5 -- rebase's sources. M: working-tree bytes -> `rebase_ignores_uncommitted_code`; `HEAD`'s blob -> `rebase_uses_each_entrys_own_blob`; today's date -> `rebase_keeps_verified_at_and_commit`.
- [ ] RD-6 -- rebasing `code-a.format-1.lock` gives `code-a.lock` byte for byte (same commit, date). M: any fabricated field -> `rebase_of_the_format_1_golden_is_accepts_golden`.
- [ ] RD-7 -- drops. M: a refusal on a missing blob -> `missing_blob_drops_its_entry`; a v2 mismatch kept -> `v2_mismatch_drops_the_entry`.
- [ ] RD-8 -- refusals. M: reading before the terminal check -> `rebase_off_a_terminal_reads_nothing`; another `norm_recipe` rebased -> `other_norm_recipe_exits_2`; a merge in progress rebased -> `rebase_refuses_an_operation_in_progress`.
- [ ] RD-9 -- the commit. M: check 3 dropped -> `rebase_commit_without_its_mode_trailer_fails_a_check`; a `Lock:` trailer -> `rebase_commit_names_no_node`.
- [ ] RD-10 -- genre, graph. M: tree-sitter in core -> `build_graph.rs` `core_and_store_have_no_tree_sitter`; a new dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; level names diverging -> `lock_level_names_match_the_marker_grammar`.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/` and `code-a.format-1.lock`: a scenario per rebase row and drop reason, text and JSON pinned, each naming its mutation.
- [ ] TE-2 -- the verify level matrix, state and detail per cell: markers default, `[sig]`, `[body]`, `[deps]`, `[path]`, two on one key; edits: signature, body, a call added, a comment, rustfmt `max_width = 60`, a neighbour, a marker narrowed, widened; a RON value vs a sibling; a module's inner attribute vs its body.
- [ ] TE-3 -- AC-14 under format 2, exit 2 in every mode: two different broken bound items (no digest, never compared), a broken RON file, a module whose file has a parse error elsewhere. M: an `is_extra()` filter; `cannot_verify` -> `ok`.
- [ ] TE-4 -- migration: the format-1 golden committed -> verify exit 2 with the hint -> rebase, `y` -> one commit passing the four checks -> verify all `ok` -> `spec lock accept <each node>` `nothing to change`.
- [ ] TE-5 -- a simulated grammar bump (a committed format-2 lock naming another tree-sitter-rust): verify exit 2, the hint; rebase, under the note, gives accept's golden.
- [ ] TE-6 -- an OID no object has: `dropped blob_missing`, then verify `unlocked`.
- [ ] TE-7 -- byte-identical lock, stdout, stderr, JSON: two roots, another cwd, `core.autocrlf=true`, the input's entries reversed. Under `core.autocrlf=true` module and RON rows read `ok` by digest, lifting 4a's `code_ahead blob` limit (`spec-verify.md` Q9).
- [ ] TE-8 -- a project pre-commit hook running `spec verify --changed` under `enforce-introduced` passes the rebase commit, exit 0 (Q8). M: `HEAD`'s lock as cause (7).
- [ ] TE-9 -- after every refusal and an `N`: `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs, index, `spec.lock` bytes unchanged.

spec-writer:

- [x] SW-1 -- this spec.
- [x] SW-2 -- ADR-0050; `architecture.md#apply`, `#control`'s ADR list; `architecture-code.md#spec-lock` (writer, ADRs, "As built"), `#verify`.
- [x] SW-3 -- drafts refreshed 2026-10-12, each pointing here: 4b, 4c (`drift_hash`, Q7; "Order"), 6a, 6b, 7a, 7b, 8a's 8b line, `hash-levels.md`, 08:41; 4a overridden here, not edited.
- [ ] SW-4 -- at shipping: `spec-lock.md` ("The file", "Entry", "Lockability", `regen`, a rebase section, "Known limits"), `spec-lock-accept.md` (step 6, "Exits"), 4a's verify canon, 05 s3.5, s5.2, 07:106-107, 08:41, AC-2, the CLI, core, store, code READMEs, `CLAUDE.md` State.
- [x] SW-5 -- `export index`, `check` clean.

## Risks

- The RON lexer is not in `code_recipe`: a lexer change must bump `LEVEL_RECIPE`; 5a-1b's stream pin and the golden's RON digests catch it.
- Blob-local location may drop entries: safe (`unlocked`), noisy.
- Alias resolution reads the working-tree corpus (`spec-lock-accept.md` "Known limits"), rebase's ties and format-1 levels too.
- Entries grow about 1.8x; core copies the level names (RD-10).

## Out of scope

5a's functions; 5c's anchor states (fingerprints stored, never matched; `moved` stays null; drafted 2026-10-12, `anchor-states.md`, which overrides "Level-aware verify"'s `path` line at its shipping); `--deep`; rebasing `norm_recipe`; regen across formats; an MCP tool, UI or plugin command. **8b** (`tested-evidence.md`): its `[[tested]]` blocks make format 3; a rebase re-derives or drops them.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 an entry rebase cannot re-derive | dropped, named, counted | refusing all strands the lock (no other writer) |
| Q2 locating the item | in its blob: name suffix, markers, v2 cross-check | a walk per distinct `commit`; decayed ones drop anyway |
| Q3 a format-1 lock under 5b | exit 2, the rebase hint | a second, v2 judge; `ast_hash` alive in 4c-1, 6a, 6b, 7a |
| Q4 levels of a format-1 entry | its blob markers' union, else `sig, body` | `sig, body`: every narrower marker `levels_changed` |
| Q5 levels verify compares | the lock's ∪ the markers' | markers only: narrowing hides a body edit |
| Q6 class of `levels_changed` | Check | Fix stops `enforce` CIs on a contract edit |
| Q7 4c's `drift_hash` | 4c's draft amended now: the entry's code fields as rendered | open v1 items re-raised once after a rebase |
| Q8 a project hook vs the rebase commit | `HEAD`'s old lock judged with the current one (overrides 4a's cause 7) | a hook stopping on exit 2 refuses it: deadlock |

Q1-Q8 (2026-10-12) decided under the owner's pre-approval, review open; WA-n: the analysis' working answers (2026-10-12), each glossed where cited, 4, 5, 7, 8, 11 assumed, the rest proven from the sources; "SW refinement": the spec-writer's.

To apply at 5b's refresh (2026-10-15, from 5a's refresh):

- the pin `nothing_outside_the_binder_reads_an_items_state` (cli `code_refs_walk.rs`) adds CLI `verify_rows.rs`, the cascade (`hash-levels.md` "Rules and edge cases");
- define the drift `code` fact of a level comparison: the changed level digests, never a fingerprint (`hash-levels.md` Q16);
- after 5b `module` and `ron` rows are compared by digest, so `proposal-drift-item.md` "The item"'s `code` wording changes (4 095 of 12 288 B on 2026-10-15, split from `proposal-drift.md` for it);
- "Why"'s order adds "after 5a-2's TE-1 figures" (`hash-levels-eval.md` Q4).

## Implementation

Not started. To record: modules, deviations with reasons.
