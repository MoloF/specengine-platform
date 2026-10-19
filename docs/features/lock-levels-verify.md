---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli]
ref: split from lock-levels 2026-10-20 (Q22); Phase 3 slice 5b-1b (WA-10, WA-11; Q5, Q6 2026-10-12; Q13, Q20 2026-10-16; Q25, Q26 2026-10-20; decided under the owner's pre-approval, review open); 05 s5.3; 08 AC-2
adrs: [ADR-0050]
---

# lock-levels-verify (5b-1b): verify by levels

## Why

5b-1a (`lock-levels.md`) records levels and every level's digest, but verify still judges as v2 did: a `[sig]` marker reads a body edit as drift, as `[sig, body]` does, and any edit of a RON file or a module's file is drift for everything bound in it (05:137's "every refactor an alarm the owner learns to ignore"). This slice makes the declared levels count. It changes no lock byte: every digest it compares is already in format 2.

Delivered: verify judging every kind by levels, C = (L_lock ∪ L_mark) ∖ `path`; `code_changed <levels>` and `levels_changed <from>-><to>`; `module` and `ron` judged by digest; JSON `levels`; a stored drift row's `levels`; a row's `code` without a blob OID; 4a's CRLF limit lifted for `module` and `ron`. Order (`lock-levels.md` Q22, 08 s2): after 5b-2 `lock-rebase.md`; then the carry-overs ("Later"), then 5c.

## Description and interactions

- **An agent** sees `code_ahead code_changed <levels>` and `code_ahead levels_changed <from>-><to>` in `spec verify` and in `report_drift`'s evidence; no read's `sync` (7a, `bundle-bindings.md`) or Health row (9b) shows them yet. **The owner** meets new exit-2 cases: a `module` whose file has any parse error and a `ron` value of a file `RonAnalysis` rejects read `cannot_verify`, in every mode (AC-14), where 5b-1a reads `code_ahead` by blob.
- **Crates** (WA-12). Core `verify.rs`: a pure level comparison (S5): level sets and digest strings in, step 4's outcome out; the judge reads the outcome, so "the judge never compares hashes itself" stays true; `MarkerFact` keeps `bad_levels`. Core `drift.rs`: `HashedRow.code` for every kind. CLI: `verify_rows.rs` (the cascade's level 3 for every kind: `item_levels`, `module_levels`, `ron::value_hashes`), `verify.rs` (the JSON row), `drift.rs` (the stored row's `levels`, `code`), `lock.rs` (`Target::Ron` carries the comment's range, if 5b-1a's accept did not add it). Store, MCP, HTTP, UI and plugin code unchanged (`daemon-keys.json` pins `DriftEvidence`'s keys only; MCP `DriftShape.rows` are untyped); no new crate or dependency.
- **Unchanged**: every lock byte, the readers and their refusals, accept (5b-1a); the queue's schema. Nothing is blocked (ADR-0012).
- **Overridden at shipping**: SW-3's sections.

## Data

### Level-aware verify

WA-10, WA-11, Q5, Q6. Per row with a lock entry E and bound markers: L_lock = E's `levels`; L_mark = the union of the markers' effective levels; C = (L_lock ∪ L_mark) ∖ `path`. The first that applies:

1. A marker's levels `Invalid`: 4a's `bad_marker bad_levels`, in its order of states.
2. The blob equals `file_blob`: code unchanged, nothing parsed (`compared: blob`). Markers bind in their own file (WA-10): L_mark moves only through an ID's resolution (a new alias), and L_lock ≠ L_mark then reads `levels_changed`.
3. Else the digests from this run's parse, by accept's functions (`lock-levels.md` "Accept"). `CannotVerify`: `cannot_verify item_error`, detail the first `ErrorCategory` of the item or of a module's file; a RON file `RonAnalysis` rejects: its first `error_categories` entry (`code-hash-ron.md` "Entry point").
4. C's digests against E's; those differing, in canonical order, are `changed`. Non-empty: code changed, `code_changed`, detail `changed` joined by `,` (`body,deps`). Empty and L_lock ≠ L_mark: code changed, `levels_changed`, detail `<L_lock>-><L_mark>` (`sig,body->sig`). Else code unchanged.

- **4a's matrix stands**, "code changed" now steps 2 and 4: spec unchanged `code_ahead code_changed|levels_changed` (Check, the hint accept's); spec changed and raised `code_ahead raised`; changed, not raised `conflict` (Fix). Q6: `levels_changed` is Check.
- `path` is never compared (5c's `MOVED`): a `[path]`-only binding is `ok` under any edit but a levels change; a sibling RON value's or a neighbour's edit is `ok`.
- Text: only the new detail (`check | code_ahead levels_changed sig,body->sig | R-12 | …`). JSON: `compared` keeps `blob`, `ast` (digests compared, any kind; `cascade.by_ast` counts them), null; each row gains `levels` after it, `{"lock":["sig","body"],"markers":["sig"],"changed":[]}`, null without E or a bound marker, `changed` null when nothing was compared.

### Drift evidence

Q13, Q20, Q25 (S9). `drift_hash`'s 18 places and `lock_code` stay 5b-1a's (`lock-levels.md` "Drift evidence").

- `code` (`HashedRow.code`): a `code_changed` row's `<level>:<digest>` per changed level of C, every kind, this run's digest, `,`-joined in canonical order (`body:b3:07c3…,deps:b3:d1e0…`); null for `levels_changed`, a row equal by blob, `cannot_verify` and every row not compared. Never a blob OID: an edit beside the bound item or value is the same drift (`proposal-drift-item.md` "The item").
- **Stored rows** (Q20, Q25): a row of this build carries `levels` after `compared`, as verify's JSON; core `DriftRow` reads past it (optional, any JSON, never validated) and the store's corruption check never requires it, so an item raised before reads as stored and the four tests storing an array `levels` stay green.
- An open item raised before this build meets new evidence once (`evidence changed since it was raised`), a rejected one is raised again (`proposal-drift.md` "Known limits").

### The pin

Q26 (S6). `verify_rows.rs` joins `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`): it calls `item_levels(`; `hash_item(`, `hash_node(`, `hash_bound_items(` banned; its errored item's `item.state` read (`unbound()`'s `first_category`) and `HashState` allowed in this file only.

## Rules and edge cases

- WHEN a marker narrows its levels, verify SHALL compare the union with the lock's (Q5).
- WHEN a binding is `[path]`-only, no code edit SHALL change its row; WHEN only a sibling RON value or a neighbour changes, the row SHALL read `ok`.
- WHEN only a fingerprint or `body_bytes` differs, no verify row or `drift_hash` SHALL change (`lock-levels.md` Q12).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, verify SHALL give byte-identical output, and `module` and `ron` rows SHALL read `ok` by digest under `core.autocrlf=true` (lifting `spec-verify.md` "Known limits"' CRLF line).
- No stack, kind or prefix in rows or texts (WA-12).

## Acceptance criteria

rust-developer (M6-M8; M1-M5 are 5b-1a's, 8 in all):

- [ ] RD-5 -- "Level-aware verify", new CLI `tests/verify_levels.rs` and pure cells in core `tests/verify_judge.rs`: a narrowed marker with a body edit is `code_changed body` (`narrowed_marker_with_a_body_edit_is_code_changed`); a widened marker is `levels_changed` (`widened_marker_is_levels_changed`); a `[path]`-only binding ignores a body edit (`path_only_binding_ignores_a_body_edit`). **M6** whole-item comparison -> `sig_marker_ignores_a_body_edit`.
- [ ] RD-6 -- `module` and `ron` by digest: a broken RON file and a module with a parse error elsewhere in its file read `cannot_verify` (`broken_ron_file_binding_cannot_verify`, `module_file_error_elsewhere_cannot_verify`). **M7** RON by blob -> `ron_sibling_edit_is_ok`.
- [ ] RD-7 -- "Drift evidence", CLI `drift_raise.rs`: 5b-1a's `lock_code_has_no_fingerprint`, core `drift_stored.rs`' and store `queue_drift.rs`' rows with `levels`, `mcp_drift.rs`' `NULL_ROW` stay green (Q20, Q25). **M8** the blob OID as `code` -> `module_neighbour_edit_keeps_the_drift_hash`.
- [ ] RD-8 -- "The pin" (Q26), no mutation run.

test-engineer:

- [ ] TE-5 -- the level matrix (`verify_levels.rs`), state, detail and `code` per cell: markers default, `[sig]`, `[body]`, `[deps]`, `[path]`, two on one key; edits: signature, body, a call added, a comment, rustfmt `max_width = 60`, a neighbour, a marker narrowed, widened; a RON value vs a sibling; a module's inner attribute vs its body.
- [ ] TE-6 -- AC-14 for `module` and `ron` (`verify_cannot.rs`): a broken RON file, a module whose file has a parse error elsewhere: exit 2 in every mode.
- [ ] TE-7 -- verify determinism (`verify_cascade.rs`): byte-identical stdout, stderr and JSON from two roots, another cwd, `core.autocrlf=true`, the lock's entries reversed; under `core.autocrlf=true` `module` and `ron` rows read `ok` by digest.

spec-writer:

- [x] SW-1 -- this spec, split from `lock-levels.md` 2026-10-20 (Q22); `export index`, `check` clean.
- [ ] SW-3 -- at shipping, bytes measured 2026-10-20: `spec-verify.md` (11 745 before 5b-1a and 5b-2) "States", "Cascade", "Known limits": about −400 B, net ≤ 0; `spec-verify-output.md` (9 204) "JSON" `levels`: ≤ +400 B; `proposal-drift-item.md` "The item" `code`, "Store"; 05 s5.3; 08 AC-2; `CLAUDE.md` State.

## Files

- **rust-developer**: core `src/verify.rs`, `src/drift.rs`; CLI `src/verify_rows.rs`, `src/verify.rs`, `src/drift.rs`, `src/lock.rs` (`Target::Ron`).
- **test-engineer**: new CLI `tests/verify_levels.rs`; `verify_common/mod.rs`, `verify_output.rs` (the `levels` key), `verify_cascade.rs`, `verify_cannot.rs`, `drift_raise.rs`, `code_refs_walk.rs`; core `tests/verify_judge.rs`, `tests/drift_stored.rs`.
- **spec-writer**: SW-3.

## Risks

- **New exit-2 cases**: a parse error anywhere in a module's file, a lexer or structure error in a RON file, make every value bound in it `cannot_verify` in every mode. On the pilots 0 of 386 module files; RON 67 of 6 301 anchors on pilot A (1.1 %), none bound; 0 on pilot B. TE-6.
- Every open drift item meets new evidence once at this build (`code`, the stored `levels`).
- `spec-verify.md` near its cap after 5b-2's "Base": SW-3 net ≤ 0.
- `verify_rows.rs` rewritten twice in a row (5b-2's base, then this): 5c starts after both.

## Out of scope

Any lock byte (5b-1a); `spec lock rebase` (5b-2); 5c's anchors (`anchor-states.md` overrides "Level-aware verify"'s `path` line at its shipping); `--deep`; reads' `sync` (7a); Health's drift row (9b); an MCP tool, UI or plugin command.

## Later

**The carry-overs**: an XS slice after this one, before 5c (`lock-levels.md` Q22; 08 s2); the one place they are listed. No behaviour change but RD-12's.

- RD-12 -- `spec bump`'s hint (Q17, 4b-2's follow-up): CLI `bump.rs` `REQUIRED_HINT`, when no `required` row has a `write` (each shows `-`): `hint: required: spec.lock vouches for the old text: send the change through the queue (spec propose update) and have the owner approve it with --editorial`; else today's text. Canon `node-revision.md` "Raising": "(both ways out)" -> "(its ways out)", 1 B saved (12 283 of 12 288). M: both ways out named -> `required_hint_without_a_line_names_only_editorial` (CLI `rev_bump.rs`).
- From 6b-1's review 1 (`code-tools.md`; nits; owner pre-approved 2026-10-16):
  - One function for a stored directory's text: code `naming.rs` `package_label` and CLI `code_index.rs` `dir_text` become one public function in `specengine-code` the CLI calls. Done when `code_index.rs` defines no `dir_text`; `code_format_history.txt` gains no line.
  - CLI `symbols.rs:1` cites task spec `symbol-reads`, "Lookup": re-point it to `` `docs/canon/symbol-reads.md` "Lookup" `` (data form, `doc_pointers.rs`); then that shipped spec's "Lookup" heading may go (spec-writer).
  - CLI `lib.rs:136-137`: re-wrap the `Exit` doc bullet's short "`refs`, `symbols`)," line.
- From 5b-1a's review (`lock-levels.md` "Implementation"; nits; owner pre-approved 2026-10-20): core `render_block` gains `debug_assert!`s (the entry's kind matches the lock's format, `levels` non-empty); CLI `revisions.rs`' unreachable re-parse fallback text dropped or its comment pointed at the invariant; one `pub(crate)` `lock_digest` for CLI `lock.rs` and `verify_rows.rs`; `found.levels` moved, not cloned, in CLI `lock.rs`; CLI `lock.rs`' citation task spec `verify-tests`, "Verifiable" (no such heading) re-pointed to canon `spec-verify-tests`, "Runnable and verifiable". No behaviour change.
- Files: code `naming.rs`; core `lock.rs`; CLI `code_index.rs`, `symbols.rs`, `lib.rs`, `bump.rs`, `revisions.rs`, `lock.rs`, `verify_rows.rs`; test `rev_bump.rs`.

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q5 levels verify compares | the lock's ∪ the markers' | markers only: narrowing hides a body edit |
| Q6 class of `levels_changed` | Check | Fix stops `enforce` CIs on a contract edit |
| Q13 the `code` fact | `<level>:<digest>` per changed level, every kind; null for `levels_changed` and rows equal by blob | the blob OID for module and RON rows: a neighbour's edit is new evidence |
| Q20 stored rows without `levels` | a later key `DriftRow` reads past, never required | every item raised before reads as corrupt (`proposal-drift-item.md` "Store") |
| Q25 `DriftRow.levels` | optional, lenient: any JSON after `compared`, never validated | four tests' array rows (S9) and mixed-build queues read as corrupt |
| Q26 `verify_rows.rs`' pin | requires `item_levels(`; bans `hash_item(`, `hash_node(`, `hash_bound_items(`; its errored item's `item.state` and `HashState` allowed there only | a code accessor, `unbound()` rewritten |

Numbers shared with `lock-levels.md` (Q22-Q24, Q27-Q31 and the rest there; Q25, Q26 moved here at its shipping) and `lock-rebase.md`; S-n: `lock-levels.md`'s 2026-10-20 list.

## Dependencies and ordering

- **Needs 5b-1a** (`lock-levels.md`, shipped 2026-10-20; canon `spec-lock.md`): format 2's levels and digests, accept's hashing (verify's digests are accept's), `lock_code`, the golden. **After 5b-2** (`lock-rebase.md`), which rewrites `verify_rows.rs`' `read_head` and base first; none of its code is needed here.
- **Before**: the carry-overs, then 5c (`anchor-states.md`: the judge's steps, `levels` JSON), 8a-2 (`verify-task.md`: `levels` in rows), 6b-2, 7a (`bundle-bindings.md` "Sync"), 8b, 9b.

## Implementation

Not started. From the split (2026-10-20, `lock-levels.md` Q22): RD-5 <- its RD-4 (M6; the cells without a mutation run); RD-6 <- RD-4's RON and module cells (M7); RD-7 <- RD-11 (M8); RD-8 <- RD-10's `verify_rows.rs` half (Q26); TE-5 <- TE-2; TE-6 <- TE-3's `module` and `ron` half; TE-7 <- TE-7's verify half; SW-3 <- SW-2's verify half; "Later" <- "Carry-overs" and RD-12. To record: modules, deviations with reasons.
