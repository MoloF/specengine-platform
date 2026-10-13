---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 slice 6a-1, split from symbol-index 2026-10-13 (WA-9, WA-10; Q3, Q9, Q11, Q12); 05 s5.1, s5.2; 08 s2
adrs: []
---

# symbol-discriminator: in-file twins named apart, display signatures

## Why

Two items of one file that render one qpath (adjacent `impl Tally` blocks, `cfg` twins) are `ambiguous duplicate` (`docs/canon/code-identity.md` "Units and `qpath`"): a marker on either is never locked (`spec-lock.md` "Lockability") and verify reads it `cannot_verify ambiguous_symbol`. `code-a`'s two `impl Tally` blocks in `counter.rs` are such a pair. Nothing renders an item's signature for a read either (05 s5.1).

Slice 6a is split (Q12): **6a-1** (this) names in-file twins apart with a discriminator in the qpath and adds the display-signature entry, both in `specengine-code`, with the goldens and canon lines of shipped slices 1, 3 and 4a that change; **6a-2** `symbol-index.md` stores the rows and serves the reads. Needs 5a-1 (`body_nodes`, `hash-levels.md` "Entry points"); listed after 5a-2, before 6a-2 and 5b (Q11). No ADR: ADR-0020 (a qpath is only a name: `architecture-code.md#symbol-identity-and-digest`) and ADR-0008 applied; planned at 05 s5.2 and `code-identity.md` "Open".

## Description and interactions

- **Who sees it**: `spec refs`, `spec lock accept`, `lock regen` and `spec verify` (health row 6 through it) print `item crates/tally::counter::impl Tally #1 impl_item` where they printed `… impl Tally impl_item ambiguous duplicate`, and such a marker becomes lockable. Unique qpaths, file modules and RON paths stay byte-identical (`data/limits.ron#root.step ambiguous duplicate` stays). The signature is computed but not printed until 6a-2 stores it for `spec symbols` and 7a's rows.
- **Locks**: `symbol` in a lock entry is the discriminated qpath (`spec-lock.md` "Entry"). No existing entry changes: an ambiguous key was never locked, and the qpath is not hashed (ADR-0020), so no recipe moves.
- **Crates**: `specengine-code` only. `bind_tree`'s one walk reads each item's own `#[cfg(..)]` text; `name_files` discriminates in-file groups before its tree-wide `duplicate`; the signature entry sits beside 5a-1's `item_levels` in `bind.rs`. The CLI's walk, `refs`, lock and verify take the names as they come, so no CLI source change is expected; its goldens move. No new dependency (`build_graph.rs` `code_crate_depends_on_exactly_the_parser_and_the_hash`).
- **For 6a-2**: the discriminator needs no other file, so it belongs to `name_files`' per-file half; 6a-2 splits that half from the two tree-wide ones (`path_targets`, `duplicate`) that its store pass recomputes.
- **Eval**: `ast_hash/mod.rs` keeps its own undiscriminated `duplicate` count. Pilot figures for the discriminator come from 6a-2's index (its TE-8).

## Data

### Discriminator

WA-9, Q3. A **group** is 2 or more items of one file whose qpaths (`code-identity.md` "Units and `qpath`", "Rendering") are equal. Each member takes the value of the first rule that gives every member a non-empty value, all distinct (SW refinement):

1. `cfg(<p>)`, where `<p>` is the predicates of the item's own `#[cfg(..)]` and those of its enclosing impl, trait and inline mods, outermost first, whitespace removed, joined by `,`;
2. its 1-based ordinal within the group in source order.

- **Rendering** `<qpath> #<value>`: `crates/tally::counter::impl Tally #2`, `crates/a::sys::regen #cfg(not(test))`. No undiscriminated qpath can produce it. A last segment never holds ` #`: an identifier has no space, and an `impl` label's only spaces (`impl `, ` as `) come before a type, `as` or a trait. A segment inside the whitespace-free `<value>` ends in `)` or is digits, never a name.
- **Reach**: items, and an inline `mod`'s module, which takes its item's qpath. Items inside twin inline mods form their own group, named by rule 1 through the mods' `cfg`. Never a file module (it has no twin by path) and never a RON path.
- **Ambiguity** (order unchanged: `path_attribute` > `unrooted` > `duplicate`): `duplicate` now means that an item of **another file** renders the same undiscriminated qpath. It applies to every item of such a cross-file group, discriminated members too (`src/x.rs` beside `src/x/mod.rs`). A twin in an unrooted file is discriminated and stays `unrooted`.
- **API**: each `name_files` item carries `discriminator: Option<String>` (the value, without ` #`), and its `qpath` is rendered with it.

### Signature

WA-10, Q9. The text that reads show for an item; `None` for a broken item (`has_error()` on it or on an attached attribute: `code-bindings.md` "Rust binding" rule 7).

- **Span**: from the item's first token after its attributes and comments up to its body (`body_nodes`, `hash-levels.md` "The body field"), exclusive. An item without a body: the whole item minus a final `;`. A `macro_definition`: up to and including its name. For `const_item` and `static_item` the body is the `value` field, and for `type_item` the `type` field, so the trailing `=` is dropped: `const CAP: u32`, `type Id`.
- **Text**: comments dropped; each whitespace run (CR included) becomes one space, with none after `(` or `[` and none before `)`, `]` or `,`; trimmed. Over `SIGNATURE_CAP_BYTES` (512) it is cut at a char boundary and `…` is appended, the whole within 512 bytes. Example: `pub struct Tally`.
- **Not the `sig` level** (`hash-levels.md` "Levels"): the signature keeps the name and leaves attributes out, and it is display text, never hashed or compared.
- **Entry** (5a-1's shape: indexes and text in, no `Node` handed out, `hash-levels.md` "Description and interactions"): `item_signatures(tree: &Tree, source: &str, items: &[usize]) -> Vec<Option<String>>` in `bind.rs`. It takes the same indexes into `RustBinding::analysis` as `item_levels` and answers in the order asked, `None` past its items. `SIGNATURE_CAP_BYTES` is exported at the crate root.

### Goldens and canon that move

- `fixtures/code-a/expected.json`: the `counter.rs:22:17` and `:45:6` rows read `item crates/tally::counter::impl Tally #1 impl_item` and `#2`, without `ambiguous duplicate`. Their `rule` and `mutation` are kept. The RON `root.step ambiguous duplicate` rows and the LAW-STEP `refs_stdout_example` do not change.
- CLI tests: `code_refs.rs` (the `22:17` row); `lock_accept.rs` (`… impl Tally #1 impl_item | predated`, plus a `#2 … | bad_rev` line now that the two markers give two keys); `verify_states.rs` (the `impl Tally` duplicate case becomes two rows, and its `ambiguous_symbol duplicate` case moves to a cross-file pair).
- Canon at shipping (SW-2).

## Rules and edge cases

- WHEN a file holds 2 or more items of one qpath, each SHALL render ` #<value>`, and none SHALL be `duplicate` unless another file renders that qpath. WHEN no file holds twins, every output SHALL be byte-identical to before.
- WHEN a twin is inserted above another, the ordinals shift. A lock entry of a shifted key SHALL then read `code_ahead` or `broken_binding marker_gone`, with the marker's new key `unlocked`, never a silent `ok`. 5c may pair such shifts (`anchor-states.md` "The pairing").
- Deterministic: one state gives byte-identical names and signatures under LF or CRLF, from any root or cwd.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax (`cfg`, kinds, fields) stays in `specengine-code`. The new code carries no stack word or Cargo literal: `bind.rs` and `naming.rs` are in `code_refs_walk.rs` `NEW_MODULES`.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- discriminator. M: a unique qpath discriminated -> `unique_qpath_is_unchanged`; the ordinal tried first -> `cfg_twins_are_named_by_cfg`; cross-file twins discriminated -> `cross_file_twins_stay_duplicate`; in-file twins kept ambiguous -> `marked_adjacent_impl_is_lockable`; a RON path discriminated -> `ron_paths_are_never_discriminated`; an enclosing mod's `cfg` ignored -> `items_in_cfg_twin_mods_take_the_mods_cfg`.
- [ ] RD-2 -- signature. M: the body kept -> `signature_stops_at_the_body`; comments kept -> `signature_drops_comments`; broken text rendered -> `broken_item_has_no_signature`; a byte cut -> `long_signature_is_cut_at_a_char_boundary`; CR kept -> `signature_is_equal_under_lf_and_crlf`; `=` kept -> `const_signature_drops_the_value`; the macro body kept -> `macro_signature_ends_at_its_name`.
- [ ] RD-3 -- seams. M: a new dependency -> `code_crate_depends_on_exactly_the_parser_and_the_hash`; a stack word or Cargo literal in `bind.rs` or `naming.rs` -> `code_refs_walk.rs`'s genre scan; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`.

test-engineer:

- [ ] TE-1 -- goldens of slices 1, 3, 4a as "Goldens and canon that move" lists; every other golden stays byte-identical (M: a unique qpath discriminated turns `code_refs.rs` red).
- [ ] TE-2 -- `specengine-code` tests over inline sources: cfg twins (fn, impl, inline mods with `//!` inside); ordinal twins with one member lacking `cfg`; `cfg` on an enclosing impl; twins in an unrooted file; a cross-file pair; a signature for every row of `hash-levels.md` "The body field", a multi-line `where`, and a 600-byte signature with a multi-byte char at the cut.
- [ ] TE-3 -- determinism: LF and CRLF copies, two roots: equal `spec refs` output, names and signatures.

spec-writer:

- [x] SW-1 -- this spec (split from `symbol-index.md` 2026-10-13); 08 s2 lists 6a-1.
- [ ] SW-2 -- at shipping: a Tier 2 `docs/canon/symbol-names.md` ("Discriminator", "Signature"), because `code-identity.md` has 29 B left; there "Units and `qpath`" Ambiguity becomes a pointer and "Open" drops the disambiguator. Also: `code-bindings.md` "Rust binding" (`duplicate` cross-file); `spec-lock.md` "Lockability" (twins lockable by their discriminated key); `spec-verify.md` "States" (`duplicate` cross-file only); the `specengine-code` README API line (after 5a-1 frees room); a 05 s5.2 pointer; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

Stored rows, lookup, `spec symbols`, `unmapped`, refs from the index (6a-2 `symbol-index.md`). Discriminating cross-file twins, RON paths or file modules. The eval's `duplicate` count. Printing the signature (6a-2, 7a).

## Open

| Q | Answer | The other answer's cost |
|---|---|---|
| 3 equal qpaths | a discriminator in the qpath | — |
| 9 signature | no attributes, no fields | — |
| 11 order | 6a after 5a-2 and before 5b; 7a after 5b; 6a stores 5a-1's level digests, no `ast_hash` (`symbol-index.md`) | kept after 5c: the discriminator rewrites 5b and 5c goldens (twins become lockable), and 5c is never tested on ordinal shifts |
| 12 split | 6a-1 (this): the discriminator and the display signature entry in `specengine-code`, plus the goldens and canon of slices 1, 3, 4a; 6a-2 `symbol-index.md`: the store, the reads, AC-10 | one L slice, likely out of iterations |

Q-numbers are shared with `symbol-index.md`. Q3 and Q9 were decided on 2026-10-10; Q11 and Q12 on 2026-10-13 (the refresh's Q1 and Q2). All were decided under the owner's pre-approval, review open. WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. Split from `symbol-index.md` (2026-10-13, Q12): its RD-6 -> RD-1 (plus `ron_paths_are_never_discriminated` and `items_in_cfg_twin_mods_take_the_mods_cfg`), RD-7 -> RD-2 (plus the `const` and macro cases), RD-3 new; its TE-1's twin and signature forms -> TE-1, TE-2; TE-3 new. To record: modules, deviations with reasons.
