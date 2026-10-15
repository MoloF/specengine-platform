---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b-1 (WA-1..WA-13; Q1-Q8 2026-10-12; refreshed and split 2026-10-16 against shipped 4b-1..4c-2 and the canon split d055d6f, Q9-Q21; decided under the owner's pre-approval, review open); 05 s3.5, s5.2, s5.3; 07 s2; 08 s2, AC-2, AC-14
adrs: [ADR-0050]
---

# lock-levels (5b-1): spec.lock format 2 and level-aware verify

## Why

Format 1 (`docs/canon/spec-lock.md` "Entry") keeps one v2 `ast_hash` per item and judges a `module` or `ron` binding by its blob alone: declared levels are dead (`[sig]` reads a body edit as drift, as `[sig, body]` does), and any edit of a RON file or a module's file is drift for everything bound in it: 05:137's "every refactor an alarm the owner learns to ignore".

Delivered: `format = 2` (levels, every level's digest, 5a's fingerprints; no `ast_hash`) written by accept, `--editorial`, regen and the editorial approve; verify judging every kind by levels; drift evidence over levels; format 1 still read where only its spec side is needed, refused elsewhere by one text naming `spec lock rebase` (Q10, Q11). Slice 5b is split (Q21): **5b-1** (this) and **5b-2** `lock-rebase.md` (`spec lock rebase`, the base rule for its commit), back to back; "5b" in other drafts names both.

Order (Q9, 08 s2): after 5a-1, 5a-1b, 6a-2 and the whole of 5a-2's TE-1, fingerprint figures included (`hash-levels-eval.md` TE-1, Q4; `symbol-index.md` Q11). Implementation may start on fixtures once 5a-1b ships; shipping waits for TE-1, as the bytes freeze at the first format-2 lock. 4a to 4c-2 have shipped; 4b-1, 4b-2 and 4c-1 read the lock ("Readers"). Then 5b-2; before 5c, 6b, 7a, 7b, 8b.

## Description and interactions

- **The owner** meets a format-1 lock, or one of another `code_recipe`, as exit 2 from verify, accept, `spec propose drift`, a drift approve, regen and the editorial approve, the message naming the way out (Q11), which 5b-2 ships. **An agent** sees `code_ahead code_changed <levels>` and `code_ahead levels_changed <from>-><to>` in `spec verify` and in `report_drift`'s evidence; no read's `sync` (7a, `bundle-bindings.md`) or Health row (9b; `health.md` reads no lock) shows them yet (S12).
- **Crates** (WA-12: core sees digests, level names, `body_bytes` only). Core: `lock.rs` (format 2 in `parse_lock`, `render_lock`, `same_pair`; format 1 read with no candidate; the two refusal texts), the level comparison in verify's judge, `drift.rs` (`HashedEntry.code`, `HashedRow.code`), its own copy of the four level names pinned against code `markers.rs` `Level`; no tree-sitter. Code: 5a-1's `LEVEL_RECIPE`, `item_levels`, `module_levels` (`hash-levels.md` "Entry points"), 5a-1b's `ron::value_hashes` (`hash-levels-ron.md` "Entry point"), canon `code-hash.md` once they ship (S11); no code-crate change. CLI: `lock.rs` (accept, `build_recipes`), `verify_rows.rs` (the cascade, `read_commit`, `read_head`), `lock_regen.rs`, `revisions.rs` (the editorial approve), `drift.rs` (`code: entry.ast_hash`, ~661), `bump.rs` (Q17). Store, MCP, HTTP, UI, plugin and eval code unchanged; no new crate or dependency.
- **Unchanged**: the gate, plain `spec check`, the index, the queue's schema and store (a stored row's `levels` is read past: Q20), `daemon-keys.json` (it pins `DriftEvidence`'s keys only). **Nothing is blocked** (ADR-0012): spec work, the gate and reads never read the lock; the rule reads format 1 (Q10), so spec commits keep checking; only a project's own `[verify]` CI or hook stops (`lock-rebase.md` "Risks").
- **Overridden at shipping** (WA-11; shipped canon is not edited while built): the canon sections SW-2 lists, each by the section here that changes it; also `spec-lock.md:11`'s "MCP … never read" (`report_drift` and the review count read it: S2) and `spec-check-git.md`'s "Format 1 is read; 5b adds 2". "Base", cause (7) and "A project's hook" are 5b-2's.

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

- Header: `code_recipe` = `<LEVEL_RECIPE> tree-sitter-rust/<version> abi/<n>` (`build_recipes` leaves v2's `RECIPE`); `norm_recipe` and the comment line unchanged. The entry key is unchanged (core `EntryKey`; WA-3): regen's merge, 4a's rows and 4c-1's items keep theirs.
- Keys in this order (WA-4: `*_hash` as `spec_hash` and 05:152's `body_hash`): `node, rev, spec_hash, relation, file, kind, symbol, item_kind?, levels, sig_hash, body_hash, deps_hash, skeleton_hash?, quote_hash?, body_bytes?, file_blob, commit, verified_by, verified_at`.

| Key | Value |
|---|---|
| `levels` | non-empty, distinct, in the canonical order `path, sig, body, deps` (`code-identity.md` "Marker grammar"): the union of the effective levels of the markers giving the key, `Default` as `sig, body` |
| `sig_hash`, `body_hash`, `deps_hash` | every kind, every level, in `levels` or not (a widened marker is judged against them): `item` by `item_levels`; `module` by `module_levels`, an inline `mod`'s `mod_item` by `item_levels` (`hash-levels.md` "File module"; SW refinement); `ron` sig = body = the value hash, deps the empty set's digest (`hash-levels-ron.md` "RON value") |
| `skeleton_hash`, `quote_hash`, `body_bytes` | `item` and `module` only; `body_bytes` a TOML integer, 0 to 4 294 967 295; stored for 5c, never compared, never drift evidence (WA-13; `hash-levels.md` Q16) |
| `ast_hash` | gone: `sig` + `body` see what v2 saw, N5 and the CR rule aside (`hash-levels.md` "Levels", TE-6; S10) |

- Bytes as format 1 (`spec-lock.md` "The file"), plus `levels = ["sig", "body"]` (basic strings, `", "` between, no trailing comma) and `body_bytes` in decimal (no sign, `_` or leading zero).

### Readers

Q10, Q11 (S2, S3). `parse_lock(text, build)`:

- **`None`**: formats 1 and 2 in full, every field validated by its own format's rules, recipes never compared. Format 1: `spec-lock.md` "Entry" (`ast_hash` iff `item`; `levels` and the digests unknown keys). Format 2: format 1's rules for shared keys; `levels` as above, else `spec.lock:<line>: levels: <empty|unknown <name>|duplicate <name>|order>`; the three digests always; the fingerprints iff `kind` is `item` or `module`; `ast_hash` an unknown key. Above 2: `format <n> is newer than this build's 2`.
- **`Some(build)`**: format 2 at this build's recipes only. Refusals, one core text each, as `LockError` renders them (`spec.lock:<line>: …`): format 1 `format 1 ("<its code_recipe>"), this build's is 2 ("<build's>"): run spec lock rebase on a terminal`; another `code_recipe` `` `code_recipe` is "<its>", this build's is "<build's>": run spec lock rebase on a terminal ``; another `norm_recipe` today's text, no step (no rebase across it: ADR-0050).
- No reader upgrades a lock in memory (WA-5). Verify keeps "a run cause gives no hint" (`spec-verify-output.md` "Hints"): the step is the cause's own text.

| Caller | Reads | Format 1 | Format 2, another `code_recipe` |
|---|---|---|---|
| rule `rev-not-raised` (core `rule_lock`), `spec bump` | `None` | read | read |
| the review's vouching count (CLI `lock_for_review`) | `None` | counted | counted |
| `spec verify`, every mode (`read_lock`, `read_commit`, `read_head`) | `Some` | cause (6), at `HEAD` (7), the text | the same |
| accept, `--editorial`, a gone node | `Some` | exit 2, the text | the same |
| `spec propose drift`, MCP `report_drift` (`read_commit`); a drift approve (`lock_accept`) | `Some` | exit 2, the text | the same |
| `spec approve --editorial` (step 3) | `None` | exit 2 before rendering, the format-1 text | refreshed: code fields, `code_recipe` kept |
| `spec lock regen` | `None`, per stage | exit 2 before rendering, the format-1 text | merged as today when every stage shares it (no hashing); else exit 2 (abort, rebase the lagging branch, merge again) |
| `spec lock rebase` (5b-2) | `None` | its input | its input |

A `HEAD` lock of another format or recipe beside a judged lock of this build's stays cause (7) until 5b-2's base rule (`lock-rebase.md` "The base", Q14).

### Accept

`spec-lock-accept.md` "Steps" 5-6 under format 2. Step 5 hashes each lockable binding from its `HEAD` blob: `item` by `item_levels`, `module` by `module_levels` (an inline `mod` by its item's), `ron` by `ron::value_hashes`, paired by the comment's range. **Lockability** (`spec-lock.md`), rows in this order, the first refusal in marker order naming one:

| Row | Locked | Never |
|---|---|---|
| `id_state`, `rev_state` | as format 1 | as format 1 |
| `levels` (new) | `Default`, a declared list | `Invalid` (`unclosed`, `unknown`, `duplicate`, `empty`): `not locked: bad_levels` |
| binding | `item`, `module` with levels hashed; `ron` with a value hash | `ambiguous <reason>`, `unanchored`, `cannot_verify`: format 1's, plus a `module` whose file has any parse error, a `ron` value of a file `RonAnalysis` rejects |

**Merge**: `same_pair` (core `lock.rs`) compares `rev`, `spec_hash`, `levels`, the three digests and `file_blob`, never a fingerprint or `body_bytes` (Q12; one recipe gives equal ones for equal blobs: S17); equal is `unchanged`, byte-identical; a changed `deps_hash` alone refreshes. `--editorial` and 4b-1's editorial apply keep every code field byte for byte.

### Level-aware verify

WA-10, WA-11, Q5, Q6. Per row with a lock entry E and bound markers: L_lock = E's `levels`; L_mark = the union of the markers' effective levels; C = (L_lock ∪ L_mark) minus `path`. The first that applies:

1. A marker's levels `Invalid`: 4a's `bad_marker bad_levels`, in its order of states.
2. The blob equals `file_blob`: code unchanged, nothing parsed (`compared: blob`). Markers bind in their own file (WA-10): L_mark moves only through an ID's resolution (a new alias), and L_lock ≠ L_mark then reads `levels_changed` (SW refinement).
3. Else the digests from this run's parse, as accept's. `CannotVerify`: `cannot_verify item_error`, detail the first `ErrorCategory` of the item or of a module's file; a RON file `RonAnalysis` rejects: its first `error_categories` entry (`hash-levels-ron.md` Q7).
4. C's digests against E's; those differing, in canonical order, are `changed`. Non-empty: code changed, `code_changed`, detail `changed` joined by `,` (`body,deps`). Empty and L_lock ≠ L_mark: code changed, `levels_changed`, detail `<L_lock>-><L_mark>` (`sig,body->sig`). Else code unchanged.

- **4a's matrix stands**, "code changed" now steps 2 and 4: spec unchanged `code_ahead code_changed|levels_changed` (Check, the hint accept's); spec changed and raised `code_ahead raised`; changed, not raised `conflict` (Fix). Q6: `levels_changed` is Check.
- `path` is never compared (5c's `MOVED`): a `[path]`-only binding is `ok` under any edit but a levels change; a sibling RON value's or a neighbour's edit is `ok`.
- Text: only the new category and detail (`check | code_ahead levels_changed sig,body->sig | R-12 | …`). JSON: `compared` keeps `blob`, `ast` (digests compared, any kind; `cascade.by_ast` counts them), null; each row gains `levels` after it, `{"lock":["sig","body"],"markers":["sig"],"changed":[]}`, null without E or a bound marker, `changed` null when nothing was compared.

### Drift evidence

Q12, Q13, Q20 (S7). `drift_hash` keeps `DRIFT_HASH_DOMAIN` and its 18 places (`proposal-drift-item.md` "The item"); two change, for every kind:

- `lock_code` (core `HashedEntry.code`): the entry's `levels`, `sig_hash`, `body_hash`, `deps_hash` lines as `render_lock` writes them, LF-joined, no final LF; never a fingerprint or `body_bytes`, so tuning 5c's fingerprints re-keys no item.
- `code` (`HashedRow.code`, the cascade's current fact): a `code_changed` row's `<level>:<digest>` per changed level, this run's digest, `,`-joined in canonical order (`body:b3:07c3…,deps:b3:d1e0…`); null for `levels_changed`, a row equal by blob, `cannot_verify` and every row not compared. Never a blob OID: an edit beside the bound item or value is the same drift (`proposal-drift-item.md` "The item"); today a `module` or `ron` row's is its file's OID.
- **Stored rows** (Q20): a row of this build carries `levels` after `compared`; core `DriftRow` reads past a later key and the store's corruption check never requires it, so an item raised before 5b-1 reads as stored.
- An item raised before the project's rebase meets new evidence once: an open one `evidence changed since it was raised`, a rejected one raised again (`proposal-drift.md` "Known limits").

### `spec bump`'s hint

Q17, 4b-2's follow-up (`rev-rule.md` "Implementation"). CLI `bump.rs` `REQUIRED_HINT`, when no `required` row has a `write` (each shows `-`): `hint: required: spec.lock vouches for the old text: send the change through the queue (spec propose update) and have the owner approve it with --editorial`; else today's text, both ways out. Canon `node-revision.md` "Raising": "(both ways out)" -> "(its ways out)".

### Fixtures and the golden

WA-6: no real format-1 lock exists ("Dependencies and ordering"). `fixtures/spec-lock/code-a.lock` becomes accept's format-2 golden, its digests generated by accept on `fixtures/code-a/`, never hand-written; its format-1 bytes move to `fixtures/spec-lock/code-a.format-1.lock` (this slice's format-1 readers; 5b-2's rebase input). 4a's RD-10 keeps its path. The format-1 golden's 11 entries hold `counter.rs:77` `[path, path]` and `bounds.rs:14` `[Sig]`, locked before the `levels` row; the format-2 golden leaves both out (9), and 4a's RD-10 reads them `bad_marker bad_levels` either way (`spec-verify.md` Q8).

**Churn** (S8): the golden is read by 12 test files and a `verify_rows.rs` unit test: CLI `verify_common`, `drift_common`, `rev_rule_common`, `drift_raise`, `drift_approve`, `rev_rule`, `rev_rule_hook`, `rev_editorial`, `lock_accept`; HTTP `drift`, `daemon_keys`; MCP `mcp_drift`. Expectations judging `module`, `ron` by blob change in `verify_states`, `verify_cascade`, `verify_modes`.

## Rules and edge cases

- WHEN a marker narrows its levels, verify SHALL compare the union with the lock's (Q5); WHEN the lock is of another format or `code_recipe`, no row SHALL be judged (exit 2, AC-14).
- WHEN a reader needs only a lock's spec side, it SHALL read formats 1 and 2; every writer and code judge SHALL refuse format 1 with the one text (Q10, Q11).
- WHEN only a fingerprint or `body_bytes` differs, no verify row, `same_pair` outcome or `drift_hash` SHALL change (Q12; `hash-levels.md` Q16).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, accept and verify SHALL give byte-identical locks and output.
- **The pin** (Q16, S6) `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`), per file since 6a-2's RD-13: `lock.rs` calls `item_levels(` and `module_levels(`, no longer `hash_bound_items(` (format 2 holds no v2 digest); `verify_rows.rs` joins, calling `item_levels(`; `hash_item(`, `hash_node(` banned, no `.state` read. 5b-2 adds `lock_rebase.rs`.
- No stack, kind or prefix in rows, texts or the lock (WA-12).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- "Format 2". M: `ast_hash` accepted -> `format_2_refuses_ast_hash`; a RON fingerprint -> `ron_entry_has_no_fingerprint`; unsorted levels -> `levels_out_of_canonical_order_refused`; a digest outside `levels` omitted -> `every_level_digest_is_written`.
- [ ] RD-2 -- "Readers". M: `parse_lock(None)` refusing format 1 -> `format_1_head_lock_is_read` (4b-2's carry-over, `rev-rule.md` RD-2: `HEAD` holds the format-1 golden, a staged spec change without a raise: `rev-not-raised` found, not cannot-check); the review's read refusing format 1 -> `review_counts_a_format_1_lock`; format 1 judged -> `format_1_lock_exits_2_with_the_rebase_step` (verify, accept, `--editorial`, regen); one caller without the step -> `format_1_text_is_the_same_everywhere` (every refusing caller of the table, MCP `report_drift` included; another `code_recipe` alike).
- [ ] RD-3 -- "Accept". M: the first marker's levels only -> `levels_are_the_union_of_the_markers`; `Invalid` as the default -> `invalid_levels_are_not_locked`; a digest ignored by `same_pair` -> `changed_deps_digest_refreshes`.
- [ ] RD-4 -- "Level-aware verify". M: whole-item comparison -> `sig_marker_ignores_a_body_edit`; markers-only set -> `narrowed_marker_with_a_body_edit_is_code_changed`; unequal sets as `ok` -> `widened_marker_is_levels_changed`; `path` compared -> `path_only_binding_ignores_a_body_edit`; RON by blob -> `ron_sibling_edit_is_ok`; a broken RON file `ok` -> `broken_ron_file_binding_cannot_verify`.
- RD-5..RD-9 -- `spec lock rebase`: moved to `lock-rebase.md` RD-1..RD-5 (Q21); RD-2's base mutation to its RD-6.
- [ ] RD-10 -- genre, graph, the pin. M: tree-sitter in core -> `build_graph.rs` `core_and_store_have_no_tree_sitter`; a new dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; level names diverging -> `lock_level_names_match_the_marker_grammar`; accept through `hash_bound_items(`, or `verify_rows.rs` without `item_levels(` -> `nothing_outside_the_binder_reads_an_items_state` (Q16).
- [ ] RD-11 -- "Drift evidence". M: `quote_hash` in `lock_code` -> `lock_code_has_no_fingerprint`; the blob OID as `code` -> `module_neighbour_edit_keeps_the_drift_hash`; `levels` required -> `stored_row_without_levels_reads`.
- [ ] RD-12 -- "`spec bump`'s hint". M: both ways out named -> `required_hint_without_a_line_names_only_editorial`.

test-engineer:

- TE-1 -- moved to `lock-rebase.md` TE-1.
- [ ] TE-2 -- the verify level matrix, state, detail and the `code` fact (Q13) per cell: markers default, `[sig]`, `[body]`, `[deps]`, `[path]`, two on one key; edits: signature, body, a call added, a comment, rustfmt `max_width = 60`, a neighbour, a marker narrowed, widened; a RON value vs a sibling; a module's inner attribute vs its body.
- [ ] TE-3 -- AC-14 under format 2, exit 2 in every mode: two different broken bound items (no digest, never compared), a broken RON file, a module whose file has a parse error elsewhere. M: an `is_extra()` filter; `cannot_verify` -> `ok`.
- TE-4, TE-5, TE-6 -- moved to `lock-rebase.md` TE-2, TE-3, TE-4.
- [ ] TE-7 -- byte-identical lock, stdout, stderr, JSON of accept and verify: two roots, another cwd, `core.autocrlf=true`, the lock's entries reversed. Under `core.autocrlf=true` `module` and `ron` rows read `ok` by digest, lifting 4a's `code_ahead` blob limit (`spec-verify.md` Q9), and a CRLF multi-line literal by the CR rule. The rebase's half: `lock-rebase.md` TE-8.
- TE-8, TE-9 -- moved to `lock-rebase.md` TE-5, TE-6.

spec-writer:

- [x] SW-1 -- this spec (2026-10-12: ADR-0050 and its canon, the neighbours pointing here); refreshed and split 2026-10-16 (Q21): `lock-rebase.md`, 08 s2, the neighbours' pointers.
- [ ] SW-2 -- at shipping, bytes measured 2026-10-15 (Tier 2 cap 12 288, Tier 1 10 240): `spec-lock.md` (7 300: "The file", "Entry", "Lockability", `regen`, `:11`, "Known limits"' grammar line pointing at 5b-2); `spec-lock-accept.md` (11 237: steps 5-6, "Exits"); `spec-verify.md` (11 587: "States", "Cascade", cause (6), "Known limits", which shrinks about 400 B; 5b-2's "Base" follows, so keep the 701 B left for it); `spec-verify-output.md` (5 380: "JSON"); `proposal-drift-item.md` (4 095: "The item", "Store"); `spec-check-git.md` (11 550: "The rule's locks"); `node-revision.md` (12 283: "Raising", Q17, net ≤ 0); `architecture-code.md` (10 711: "As built" under `#spec-lock`, `#verify`); `proposal-drift.md` (8 794) unchanged; 05 s3.5, s5.2, s5.3; 08 s2, AC-2, AC-14; the core (9 968: pointers only, net ≤ 0) and CLI (8 612) READMEs; `CLAUDE.md` State (5 648 of 16 384). No new canon file.
- [x] SW-3 -- no new ADR (ADR-0041, ADR-0042, ADR-0045, ADR-0050 hold); `export index`, `check` clean.

## Risks

- **Frozen bytes**: the first format-2 lock freezes 5a's bytes; a later change costs a `LEVEL_RECIPE` bump, a rebase of every lock and every rejected drift item back once (Q9). The RON lexer is not in `code_recipe`: a lexer change must bump `LEVEL_RECIPE`; 5a-1b's stream pin and the golden's RON digests catch it.
- **New exit-2 cases**: a parse error anywhere in a module's file, a lexer or structure error in a RON file, make every value locked in it `cannot_verify` in every mode, where 4a reads `code_ahead` by blob.
- **The window**: from the upgrade until a project's rebase commit, its `[verify]` hook or CI exits 2 on every commit (`lock-rebase.md` "Risks"); no format-1 lock exists, and 5b-2 follows at once.
- Entries grow about 1.8x.

## Out of scope

5a's functions; `spec lock rebase` and the base rule (5b-2, `lock-rebase.md`); 5c's anchor states (fingerprints stored, never matched; `moved` stays null; `anchor-states.md`, which overrides "Level-aware verify"'s `path` line at its shipping); `--deep`; reads' `sync` (7a); Health's drift row (9b); an MCP tool, UI or plugin command. **8b** (`tested-evidence.md`): its `[[tested]]` blocks make format 3.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1, Q2, Q4, Q8 | `lock-rebase.md` "Open" | — |
| Q3 a format-1 lock under 5b | exit 2 naming `spec lock rebase` wherever code is judged or a lock written; read where only its spec side counts (Q10) | a second, v2 judge; `ast_hash` alive in 4c-1, 6a, 6b, 7a |
| Q5 levels verify compares | the lock's ∪ the markers' | markers only: narrowing hides a body edit |
| Q6 class of `levels_changed` | Check | Fix stops `enforce` CIs on a contract edit |
| Q7 4c's `drift_hash` | the entry's code fields as rendered; narrowed by Q12 | open v1 items re-raised once after a rebase |
| Q9 order (owner) | after 5a-1, 5a-1b, 6a-2 and 5a-2's whole TE-1, fingerprint figures included; implementation on fixtures once 5a-1b ships | shipping on fixture evidence: a pilot shortfall costs a v4 recipe, a rebase of every lock, every rejected drift item back once (`hash-levels-eval.md` Q4) |
| Q10 format-1 readers | `None` reads formats 1 and 2 (the rule, bump, the review count, the rebase); `Some(build)` this build's only; regen and the editorial approve refuse format 1 before rendering ("Readers") | `None` refusing format 1: every spec commit of an unrebased project cannot check, the review count `null` |
| Q11 the refusal text | the reader's message carries the step, every caller alike; "a run cause gives no hint" stays true | a separate `hint:` line: an exception to the hint rule, wiring and a pin per caller |
| Q12 `lock_code` | `levels` and the three digests as rendered, no fingerprint; `same_pair` drops the fingerprints too | all code fields: tuning 5c's fingerprints re-keys every item, rejected ones come back |
| Q13 the `code` fact | `<level>:<digest>` per changed level; null for `levels_changed` and rows equal by blob | the blob OID for module and RON rows: a neighbour's edit is new evidence (`proposal-drift-item.md` "The item") |
| Q16 the pin | per file: `lock.rs` `item_levels(`, `module_levels(`; `verify_rows.rs` `item_levels(`; 5b-2's `lock_rebase.rs` `item_levels(`, `hash_bound_items(` | the cross-check through `hash_item(`: a tree-sitter node in the CLI (`hash-levels.md` Q6), the ban lifted |
| Q17 `REQUIRED_HINT` | `--editorial` alone when no required row has a `write`; canon "(both ways out)" -> "(its ways out)", byte-neutral | the hint keeps advising a raise no row can write |
| Q18 4b-2's deferred tests | `format_1_head_lock_is_read` here; the rebase commit through a real `spec check --staged` hook: 5b-2 | a reader change makes unrebased projects cannot-check, no test catching it |
| Q20 stored rows without `levels` | a later key `DriftRow` reads past, never required | every item raised before 5b reads as corrupt (`proposal-drift-item.md` "Store") |
| Q21 5b's size (owner) | split: 5b-1 (this); 5b-2 `lock-rebase.md` (the rebase, the store read by OID, the locator, the base rule); back to back | one L slice: 13 suites churn in one review, the iterations may run out with the format half-shipped |

Q1-Q8 (2026-10-12) and Q9-Q21 (2026-10-16, the refresh against shipped 4b-1..4c-2) decided under the owner's pre-approval, review open; Q14, Q15, Q19 live in `lock-rebase.md`. S-n: the refresh's stale points. WA-n: the 2026-10-12 analysis' working answers, each glossed where cited; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **From 5a-1** (`hash-levels.md` "Entry points"): `LEVEL_RECIPE`, `LevelState`, `LevelHashes`, `item_levels`, `module_levels`, `Digest`; v2's `RECIPE`, `hash_bound_items` (5b-2's cross-check), `grammar_info` kept; `markers.rs` `Level`, `Levels`, `DEFAULT_LEVELS`; TE-6, the ground for dropping `ast_hash`. **From 5a-1b** (shipped, `docs/canon/code-hash-ron.md`): `ron::value_hashes`, keyed by `CommentAnchor::comment`, its whole-file refusal; open: a `ron` row's `deps_hash` (the empty set's digest) has no public source, `levels::deps_digest` being `pub(crate)`: a public accessor or a pinned constant.
- **From 5a-2's TE-1** (`hash-levels-eval.md`, its "Implementation"): `levels.stable_pct` per level and perturbation (100.0 % or exceptions the owner accepts: 08 AC-2), `module_stable_pct`, `neighbour` (`edited_changed` = `edited`), `value_hash.crlf_stable_pct` (TE-7), the `cannot_verify` rates, `cost` against 6.67 ms per kLOC, the fingerprint figures.
- **Waits for TE-1**: shipping; the goldens' digests (generated); three policies the figures may reopen: refusing a module with any parse error, a whole RON file, the fingerprint bytes. The rest is decidable now.
- **Assumptions** (checked 2026-10-15): 5a-1's and 5a-1b's entry points ship as specified; 6a-2 ships first (the pin per file, `symbol-index.md` RD-13); no real format-1 lock exists (no `[code]` here; pilot A has no marker and no `spec.lock`, pilot B no `spec.lock`); 4c-2 does not interact (`drift-follow-up.md` "Dependencies and ordering").

## Implementation

Not started. The split (2026-10-16, Q21): RD-1..RD-4, RD-10, TE-2, TE-3, TE-7 kept; RD-5..RD-9 -> `lock-rebase.md` RD-1..RD-5, RD-2's base mutation -> its RD-6; TE-1, TE-4..TE-6, TE-8, TE-9 -> its TE-1..TE-6, TE-7's rebase half -> its TE-8; RD-11, RD-12 new; RD-2's refusal test renamed (Q11), reader tests added (Q10, Q18); old SW-2, SW-3 (done) -> SW-1, SW-4 -> SW-2 (the rebase's half: its SW-2), SW-5 -> SW-3. To record: modules, deviations with reasons.
