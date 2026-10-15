---
class: spec
status: draft
scope: [crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 slice 6a-1, split from symbol-index 2026-10-13 (WA-9, WA-10; Q3, Q9, Q11, Q12); refreshed 2026-10-15 against shipped 4b-1, 4c-1 and the drafts 4b-2, 4c-2, 5a-1, 5a-1b, 5a-2 (Q19-Q22); 05 s5.1, s5.2; 08 s2
adrs: []
---

# symbol-discriminator: in-file twins named apart, display signatures

## Why

Two items of one file that render one qpath (adjacent `impl Tally` blocks, `cfg` twins) are `ambiguous duplicate` (`docs/canon/code-identity.md` "Units and `qpath`"): a marker on either is never locked (`spec-lock.md` "Lockability") and verify reads it `cannot_verify ambiguous_symbol`. `code-a`'s two `impl Tally` blocks in `counter.rs` are such a pair. Nothing renders an item's signature for a read either (05 s5.1).

Slice 6a is split (Q12): **6a-1** (this) names in-file twins apart with a discriminator in the qpath and adds the display-signature entry, both in `specengine-code`, with the goldens and canon lines of shipped slices 1, 3 and 4a that change; **6a-2** `symbol-index.md` stores the rows, **6a-3** `symbol-reads.md` serves the reads (Q23). Needs 5a-1 alone (`body_nodes`, `BODY_FIELDS`: `hash-levels.md` "Entry points"), neither 5a-1b nor 5a-2; listed (08 s2) 5a-1 -> 5a-1b -> 5a-2 -> 6a-1 -> 6a-2 -> 5b, before 5b and 5c because twins become lockable (Q11). No ADR: ADR-0020 (a qpath is only a name: `architecture-code.md#code-identity`) and ADR-0008 applied; planned at 05 s5.1 (:117 adjacent impls, :129 the signature), s5.2 (:156) and `code-identity.md` "Open".

## Description and interactions

- **Who sees it**: `spec refs` and `spec lock accept` print `item crates/tally::counter::impl Tally #1 impl_item` where they printed `… impl Tally impl_item ambiguous duplicate`, and such a marker becomes lockable. `spec verify` keys such a marker's row by the discriminated qpath where one `cannot_verify ambiguous_symbol duplicate` row stood, on every side: the disk, `--changed`'s base, 4b-2's `--staged` and 4c-1's one-commit seam (`tree_of_blobs` names through `name_files` too: CLI `code_walk.rs`), so `spec propose drift`'s stored rows, summary and `drift_hash` and a drift item's approve follow (Risks); 9b's Health row 6 will through verify. Unique qpaths, file modules and RON paths stay byte-identical (`data/limits.ron#root.step ambiguous duplicate` stays). `item_signatures` is exported, called by no command until 6a-2 stores it for 6a-3's `spec symbols` and 7a's rows.
- **Locks**: `symbol` in a lock entry is the discriminated qpath (`spec-lock.md` "Entry"). No entry's bytes change: an ambiguous key was never locked (code-a's twins stay `predated`, `bad_rev`: the golden lock is unchanged), and the qpath is not hashed (ADR-0020), so no recipe moves. An entry whose item has since gained an in-file twin no longer finds its key: `broken_binding symbol_gone`, its marker's new key `unlocked`, where it read `cannot_verify ambiguous_symbol duplicate` (5c pairs the two by `body`: `anchor-states.md` "The pairing"). From the first accept of a twin, its discriminator is a lock key, which `spec lock rebase` keeps byte for byte (`lock-levels.md`): the rules here are fixed at shipping (Risks).
- **Crates**: `specengine-code` only. The collect walk of `items.rs` (shared by `bind_tree` and `analyze_tree`) records on `ItemRecord` each item's `cfg` chain, its enclosing impl's, trait's and inline mods' predicates first (Q19); `name_files` discriminates in-file groups before its tree-wide `duplicate`; the docs that call adjacent impls `duplicate` (`qpath.rs` `Ambiguity::Duplicate` and `qpath()`, `naming.rs`' module doc, `lib.rs`' `naming` line) say another file; the signature entry sits beside 5a-1's `item_levels` in `bind.rs`. The CLI's walk, `tree_of_blobs`, `refs`, lock and verify take the names as they come, so no CLI source change is expected; its goldens move. No new dependency (`build_graph.rs` `code_crate_depends_on_exactly_the_parser_and_the_hash`).
- **For 6a-2** (Q21): the discriminator needs no other file, so `naming.rs` ships split here: a per-file half (a file's module and items named and discriminated over its package table) and the tree-wide passes (`path_targets`; `duplicate` by undiscriminated qpath across files), `name_files` their composition; 6a-2's store pass recomputes the tree-wide passes over stored rows (`symbol-index.md` "Description and interactions", **Seams**).
- **Eval**: `ast_hash/mod.rs` keeps its own undiscriminated `duplicate` count (its own item keys, not `name_files`), so `fixtures/ast-hash/expected.json` stays byte-identical; its pilot figure (`code-identity.md` "Measured": A 29 items in 11 groups, B 2 in 1, every group within one file) is what this slice names apart. Figures by rule come from 6a-2's index (its TE-8).

## Data

### Discriminator

WA-9, Q3. A **group** is 2 or more items of one file whose qpaths (`code-identity.md` "Units and `qpath`", "Rendering") are equal. Each member takes the value of the first rule that gives every member a non-empty value, all distinct (SW refinement):

1. `cfg(<p>)`, where `<p>` is the predicates of the item's own `#[cfg(..)]` and those of its enclosing impl, trait and inline mods, outermost first, comments and whitespace removed, joined by `,` (Q20);
2. its 1-based ordinal within the group in source order.

- **Rendering** `<qpath> #<value>`: `crates/tally::counter::impl Tally #2`, `crates/a::sys::regen #cfg(not(test))`. No undiscriminated qpath can produce it. A last segment never holds ` #`: an identifier has no space, and an `impl` label's only spaces (`impl `, ` as `) come before a type, `as` or a trait. A segment inside the whitespace-free `<value>` ends in `)` or is digits, never a name.
- **Reach**: items, and an inline `mod`'s module, which takes its item's qpath. Items inside twin inline mods form their own group, named by rule 1 through the mods' `cfg`. Never a file module (it has no twin by path) and never a RON path.
- **Ambiguity** (order unchanged: `path_attribute` > `unrooted` > `duplicate`): `duplicate` now means that an item of **another file** renders the same undiscriminated qpath. It applies to every item of such a cross-file group, discriminated members too (`src/x.rs` beside `src/x/mod.rs`). A twin in an unrooted file is discriminated and stays `unrooted`.
- **API**: `ItemRecord` gains `cfg: Vec<String>`, its chain's predicates as rule 1 writes them, outermost first (Q19); each `name_files` item carries `discriminator: Option<String>` (the value, without ` #`), and its `qpath` is rendered with it. Every `ItemRecord` and `Name` literal in tests gains the field.

### Signature

WA-10, Q9. The text that reads show for an item; `None` for a broken item (`has_error()` on it or on an attached attribute: `code-bindings.md` "Rust binding" rule 7).

- **Span**: from the item's first token after its attributes and comments up to its body (`body_nodes` over `BODY_FIELDS`, `hash-levels.md` "The body field"), exclusive. An item without a body: the whole item minus a final `;`. A `macro_definition`: up to and including its name. For `const_item` and `static_item` the body is the `value` field, and for `type_item` the `type` field, so the trailing `=` is dropped: `const CAP: u32`, `type Id`.
- **Text**: comments dropped; each whitespace run (CR included) becomes one space, with none after `(` or `[` and none before `)`, `]` or `,`; trimmed. Over `SIGNATURE_CAP_BYTES` (512) it is cut at a char boundary and `…` is appended, the whole within 512 bytes. Example: `pub struct Tally`.
- **Not the `sig` level** (`hash-levels.md` "Levels"): the signature keeps the name and leaves attributes out, and it is display text, never hashed or compared.
- **Entry** (5a-1's shape: indexes and text in, no `Node` handed out, `hash-levels.md` "Description and interactions"): `item_signatures(tree: &Tree, source: &str, items: &[usize]) -> Vec<Option<String>>` in `bind.rs`. It takes the same indexes into `RustBinding::analysis` as `item_levels` and answers in the order asked, `None` past its items. `SIGNATURE_CAP_BYTES` is exported at the crate root. One collect walk per call, as `hash_bound_items` (`bind.rs`); 6a-2 calls it once per re-derived file with every index.

### Goldens and canon that move

- `fixtures/code-a/expected.json`: the `counter.rs:22:17` and `:45:6` rows read `item crates/tally::counter::impl Tally #1 impl_item` and `#2`, without `ambiguous duplicate`. Their `rule` and `mutation` are kept. The RON `root.step ambiguous duplicate` rows and the LAW-STEP `refs_stdout_example` do not change.
- CLI tests: `code_refs.rs` (the `22:17` row); `lock_accept.rs` (`… impl Tally #1 impl_item | predated`, plus a `#2 … | bad_rev` line now that the two markers give two keys); `verify_states.rs` (the `impl Tally` duplicate case becomes two rows, and its `ambiguous_symbol duplicate` case moves to a cross-file pair). 4c-1's drift tests (`drift_raise.rs`, `drift_approve.rs`, HTTP `drift.rs`: `cap_changed` raises LAW-BOUND, whose twin row becomes `predated` and `bad_marker bad_rev`) compare against live `spec verify --json` and a twin repository's accept, so they stay green unedited; `fixtures/spec-lock/code-a.lock` is unchanged. Candidates to re-run, none expected to move: the CLI `refs.rs` `tests` module, `lock_commit.rs`, `verify_cannot.rs` (verify-ecs), core `verify_judge.rs` (its own inputs).
- Canon at shipping (SW-2).

## Rules and edge cases

- WHEN a file holds 2 or more items of one qpath, each SHALL render ` #<value>`, and none SHALL be `duplicate` unless another file renders that qpath. WHEN no file holds twins, every output SHALL be byte-identical to before.
- WHEN a group's membership or a member's `cfg` changes (a twin inserted, removed or moved; rule 1 starting or ceasing to hold), its members' discriminators MAY change; a node whose locked twin's key changed SHALL show a row not `ok`: the vanished key `broken_binding symbol_gone` (5c may pair it by `body`: `anchor-states.md` "The pairing"), a key now naming an unmarked twin `broken_binding marker_gone`, a key now naming another twin compared with that twin's digest, or the moved marker's new key `unlocked`. WHEN an item locked under a unique qpath gains an in-file twin, its entry SHALL read `broken_binding symbol_gone` and the marker's new key `unlocked`.
- Deterministic: one state gives byte-identical names and signatures under LF or CRLF, from any root or cwd.
- Genre (`docs/canon/architecture.md#universal`): Rust syntax (`cfg`, kinds, fields) stays in `specengine-code`. The new code carries no stack word or Cargo literal: `bind.rs` and `naming.rs` are in `code_refs_walk.rs` `NEW_MODULES`; `items.rs`, which now reads `cfg`, joins them if its literals (tree-sitter kinds, `path`, `cfg`) pass the scan, else the code-reviewer checks it (Q19).

## Risks

- **Frozen names**: from the first accept of a twin its discriminator is a lock key; a later change of these rules (rule order, the `cfg` text, ` #`) reads `symbol_gone` plus `unlocked` until re-accepted, and a rebase keeps keys. Pinned by `fixtures/code-a/expected.json` and TE-2; Q20 settles the one open byte.
- **Ordinals shift**: a twin inserted or removed renames its later siblings; 5c pairs only a vanished key, never one still naming an item (`marker_gone`, a wrong twin's comparison). 6a-2's TE-8 counts groups by rule; Q22 revisits ordinals if they dominate, before a pilot locks a twin.
- **Drift re-key**: a drift item of a node holding a twin marker changes evidence (`symbol`, `state` in `drift_hash`: `proposal-drift.md` "The item"): an open one reads `evidence changed since it was raised` on the next raise, a rejected one is raised again once, as after 5b's rebase ("Known limits"). The pilots hold no marker and no drift item.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- discriminator. M: a unique qpath discriminated -> `unique_qpath_is_unchanged`; the ordinal tried first -> `cfg_twins_are_named_by_cfg`; cross-file twins discriminated -> `cross_file_twins_stay_duplicate`; in-file twins kept ambiguous -> `marked_adjacent_impl_is_lockable`; a RON path discriminated -> `ron_paths_are_never_discriminated`; an enclosing mod's `cfg` ignored -> `items_in_cfg_twin_mods_take_the_mods_cfg`; rule 1 taken with a member lacking `cfg` -> `twins_with_a_cfg_less_member_take_ordinals`; ordinals off source order -> `ordinals_follow_source_order`; a comment inside `cfg(..)` kept -> `cfg_comment_edit_keeps_the_name` (Q20); an undiscriminated locked key matched to `#1` once a twin appears -> `twin_added_after_lock_breaks_the_entry` (CLI); a shifted key matched by ordinal fallback -> `inserted_twin_never_reads_all_ok` (CLI); the halves diverging from `name_files` -> `name_files_equals_its_halves` (Q21).
- [ ] RD-2 -- signature. M: the body kept -> `signature_stops_at_the_body`; comments kept -> `signature_drops_comments`; broken text rendered -> `broken_item_has_no_signature`; a byte cut -> `long_signature_is_cut_at_a_char_boundary`; CR kept -> `signature_is_equal_under_lf_and_crlf`; `=` kept -> `const_signature_drops_the_value`; the macro body kept -> `macro_signature_ends_at_its_name`; the order asked ignored, or an index past its items answered -> `item_signatures_follow_the_indexes_asked`.
- [ ] RD-3 -- seams. M: a new dependency -> `code_crate_depends_on_exactly_the_parser_and_the_hash`; a stack word or Cargo literal in `bind.rs`, `naming.rs` or `items.rs` ("Rules and edge cases", Genre) -> `code_refs_walk.rs`'s genre scan; an item's `state` read -> `nothing_outside_the_binder_reads_an_items_state`.

test-engineer:

- [ ] TE-1 -- goldens of slices 1, 3, 4a as "Goldens and canon that move" lists; every other golden stays byte-identical, `fixtures/ast-hash/expected.json`, the golden lock and 4c-1's drift tests included (M: a unique qpath discriminated turns `code_refs.rs` red). Starts on 4b-2's commit: `verify_states.rs` is edited there.
- [ ] TE-2 -- `specengine-code` tests over inline sources: cfg twins (fn, impl, inline mods with `//!` inside); ordinal twins with one member lacking `cfg`; `cfg` on an enclosing impl; twins in an unrooted file; a cross-file pair; a signature for every `BODY_FIELDS` row, the cases looked up from the exported table so a row without one fails (M: a kind left out -> `signature_covers_every_body_field_row`), a multi-line `where`, and a 600-byte signature with a multi-byte char at the cut; a `cfg` holding a comment and a string with spaces.
- [ ] TE-3 -- determinism: LF and CRLF copies, two roots: equal `spec refs` output, names and signatures.

spec-writer:

- [x] SW-1 -- this spec (split from `symbol-index.md` 2026-10-13); 08 s2 lists 6a-1.
- [ ] SW-2 -- at shipping: a Tier 2 `docs/canon/symbol-names.md` ("Discriminator", "Signature", and Risks' frozen names and drift re-key; nothing in `proposal-drift.md`, 12 205 of 12 288 B), because `code-identity.md` has 29 B left (≈ 70 after 5a-1, 5a-1b; re-measure): there "Units and `qpath`" Ambiguity becomes a pointer, "Rendering" gains `<name>[ #<discriminator>]` with it, "Eval outputs" says the eval's `duplicate` counts in-file twins too, "Open" drops the `disambiguator` (net within what is left). Also: `code-bindings.md` "Rust binding" (`duplicate`: another file; an inline `mod`'s module takes its item's discriminated qpath) and "`spec refs`" Binding (a qpath may end ` #<value>`), net ≤ 0; `spec-lock.md` "Lockability" (twins lockable by their discriminated key); `spec-verify.md` "States" (`duplicate`: another file only); the `specengine-code` README `naming`, `bind` rows (after 5a-1's move); 05 s5.1 (:117, :129) and s5.2 (:156) pointers; `docs/index-canon.md` re-measured (9 092 of 10 240 B on 2026-10-15, + ≈ 170 B; over the cap the shard splits: `docs/README.md` "Generated"); 08 s2's slice list; `CLAUDE.md` State.
- [x] SW-3 -- no ADR; `export index`, `check` clean.

## Out of scope

Stored rows, refs from the index (6a-2 `symbol-index.md`); lookup, `spec symbols`, `unmapped` (6a-3 `symbol-reads.md`). Discriminating cross-file twins, RON paths or file modules. The eval's `duplicate` count. Printing the signature (6a-3, 7a). A content-derived impl discriminator (Q22).

## Open

| Q | Answer | The other answer's cost |
|---|---|---|
| 3 equal qpaths | a discriminator in the qpath | — |
| 9 signature | no attributes, no fields | — |
| 11 order | 6a after 5a-2 and before 5b; 7a after 5b; 6a stores 5a-1's level digests, no `ast_hash` (`symbol-index.md`) | kept after 5c: the discriminator rewrites 5b and 5c goldens (twins become lockable), and 5c is never tested on ordinal shifts |
| 12 split | 6a-1 (this): the discriminator and the display signature entry in `specengine-code`, plus the goldens and canon of slices 1, 3, 4a; 6a-2 `symbol-index.md`: the store, the reads, AC-10 | one L slice, likely out of iterations |
| 19 the `cfg` chain's source | `ItemRecord.cfg`, recorded by the collect walk in `items.rs` for both analyses | a `NamedFile` field: the CLI's `name_tree` changes, and callers of `analyze_tree` cannot discriminate |
| 20 comments in `cfg(..)` (flagged: its bytes become lock keys) | dropped, as in the signature | a comment edit inside a `cfg` renames a twin: `symbol_gone` plus `unlocked` |
| 21 `name_files`' halves | shipped here, with `name_files` as their composition | `naming.rs` is rewritten again in 6a-2, and the L slice grows |
| 22 ordinals (owner) | kept (Q3); revisited from 6a-2's TE-8 counts before a pilot locks a twin | a content rule now (`impl Tally #step`): unmeasured, and it renames a block whose first member changes |

Q-numbers are shared with `symbol-index.md` and `symbol-reads.md` (Q23: the reads split from 6a-2). Q3 and Q9 were decided on 2026-10-10; Q11 and Q12 on 2026-10-13 (the refresh's Q1 and Q2). All were decided under the owner's pre-approval, review open. Q19-Q22 (2026-10-15, the refresh against shipped 4b-1, 4c-1 and the 5a drafts) decided under the owner's pre-approval, review open; Q20 and Q22 flagged. WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. Split from `symbol-index.md` (2026-10-13, Q12): its RD-6 -> RD-1 (plus `ron_paths_are_never_discriminated` and `items_in_cfg_twin_mods_take_the_mods_cfg`), RD-7 -> RD-2 (plus the `const` and macro cases), RD-3 new; its TE-1's twin and signature forms -> TE-1, TE-2; TE-3 new. Refreshed 2026-10-15, no renumbering: RD-1 gains six mutations, RD-2 one, RD-3 `items.rs`; TE-1 names 4c-1's tests and 4b-2's start; TE-2 reads `BODY_FIELDS`. To record: modules, deviations with reasons.
