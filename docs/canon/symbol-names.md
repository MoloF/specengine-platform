---
class: canon
tier: 2
scope: [crates/specengine-code]
owner: owner
reviewed: 2026-10-16
---

# Symbol names: in-file twins and display signatures

Two items of one file that render one `qpath` (`code-identity.md` "Units and `qpath`": adjacent `impl Tally` blocks, `cfg` twins) are named apart by a discriminator in the qpath, so a marker on either is lockable (`spec-lock.md` "Lockability") and verify keys it by a row of its own. A signature is an item's display text for reads. Both are layer A in `specengine-code` (`items.rs`, `naming.rs`, `bind.rs`). A qpath is only a name (ADR-0020), and its Rust syntax stays in that crate (ADR-0008). The CLI's walk, `tree_of_blobs`, `refs`, lock and verify take the names as they come. One state gives byte-identical names and signatures under LF or CRLF, from any root or cwd.

## Discriminator

A **group** is 2 or more items of one file whose rendered qpaths are equal, whatever their kinds (`mod config;` beside `fn config` is a group). Each member takes the value of the first rule that gives every member a non-empty value, all distinct:

1. `cfg(<p>)`, where `<p>` is the item's `cfg` chain joined by `,`;
2. its 1-based ordinal within the group, in source order (the collect walk's pre-order).

**The `cfg` chain** (`ItemRecord.cfg`, recorded by the collect walk that `bind_tree` and `analyze_tree` share): the predicates of the enclosing inline mods, then of the enclosing `impl` or `trait`, then the item's own, outermost first, each item's attributes in source order. Only an attached outer `#[cfg(…)]` is read: an `attribute` whose path, comments before it skipped (`#[/*c*/cfg(a)]`, as for `#[path]`), is the bare identifier `cfg` and whose token tree opens with `(`. `cfg_attr`, an inner `#![cfg]`, `#[cfg[…]]` and `#[::core::cfg(…)]` are ignored, and an empty `cfg()` is not recorded.

**A predicate** is the tokens between the parentheses joined with no whitespace, comments dropped by kind. A `string_literal` or `raw_string_literal` is kept verbatim, inner spaces included, except that a CR right before an LF is dropped. So `#[cfg(not(test))]` gives `not(test)`, and `#[cfg(feature = /* c */ "fast path")]` gives `feature="fast path"`.

**Rendering** `<qpath> #<value>` (`DISCRIMINATOR_SEPARATOR`, ` #`): `crates/tally::counter::impl Tally #2`, `crates/a::sys::regen #cfg(not(test))`. A value is `cfg(…)` or digits, and a `cfg` string can hold spaces, `::` or ` #`. **Readers take the discriminator from `Name.discriminator` (the value, without ` #`) or the code index's `symbols.discriminator` (`symbol-index.md`), never by splitting the qpath**: package and module segments come from directory and file names, which can hold ` #` (`src/a #1.rs` gives the module `pkg::a #1`). `Name::undiscriminated()` is the qpath without it.

**Reach**: items, and an inline `mod`'s module, which takes its item's name. Items inside twin inline mods form their own group, named by rule 1 through the mods' `cfg`. Never a file module (it has no twin by path) and never a RON path (`data/limits.ron#root.step ambiguous duplicate` stays). A unique qpath has no discriminator and renders as before.

## Ambiguity

The order is unchanged (`code-identity.md` "Units and `qpath`"): `path_attribute` > `unrooted` > `duplicate`.

- **`duplicate`** means that an item of **another file** renders the same undiscriminated qpath (`qpath.rs` `Ambiguity::Duplicate`). It applies to every item of a cross-file group, discriminated members too (`src/x.rs` beside `src/x/mod.rs`), and only to an item with no other ambiguity. In-file twins alone are never `duplicate`.
- Twins in an unrooted or `#[path]` file are discriminated and keep that ambiguity.
- The cross-file pass counts every file, unrooted and `#[path]` targets included.
- The eval (`ast_hash/mod.rs`) keeps its own undiscriminated `duplicate` count, in-file twins included (`code-identity.md` "Eval outputs", "Measured").

**Halves** (`naming.rs`):

- `name_file` names one file's module and items over its package table: twins discriminated, `unrooted` read from the file's role, and `FileNames.shared` for the next pass. It is a pure function of (the file's analysis, its path, its package dir, its package table), and it needs no other file.
- `tree_passes` reads every file: `path_targets`, then `path_attribute` outside shared units, then `duplicate` by `Name::undiscriminated()`.
- `name_files` is `name_file` per file followed by `tree_passes`, pinned equal by `name_files_equals_its_halves`. The code index recomputes the tree-wide passes over stored rows (`symbol-index.md` "Cache key and cross-file pass").

## Signature

The text that reads show for an item (05 §5.1 "Signature"). The code index stores it (`symbols.signature`); 6a-3 and 7a print it. It is `None` for a broken item (`has_error()` on it or on an attached attribute: `code-bindings.md` "Rust binding" rule 7).

- **Span**: from the item's first non-comment child (its outer attributes and the comments before it are siblings, never inside it) up to its body (`body_nodes` over `BODY_FIELDS`: `code-hash.md` "The body field"), exclusive, with an `=` right before the body dropped: `const CAP: u32`, `type Id`. A `macro_definition` runs up to and including its name. An item without a body is taken whole, a final `;` dropped.
- **Text**: a comment counts as whitespace (`pub/*c*/fn g` gives `pub fn g`). Each whitespace run, CR included, becomes one space, with none after `(` or `[` and none before `)`, `]` or `,`; the result is trimmed. Trailing commas stay (`(left: T, right: U,)`). Over `SIGNATURE_CAP_BYTES` (512), the longest char-boundary prefix of at most 509 bytes is kept, a space ending it dropped, and `…` appended. Example: `pub struct Tally`.
- **Display only**: never hashed or compared, and never part of a lock or drift key. It is not the `sig` level (`code-hash.md` "Levels"): it keeps the name and leaves attributes out.
- **Entry**: `item_signatures(tree, source, items: &[usize]) -> Vec<Option<String>>` in `bind.rs`, re-exported with `SIGNATURE_CAP_BYTES`; the twin of `item_levels`. It takes indexes into `bind_tree`'s `RustBinding::analysis` and answers in the order asked, `None` past its items. It does one collect walk per call and reads no hash state. The code index's analyzer calls it once per re-derived file.

## Frozen names

From the first `spec lock accept` of a twin, its discriminated qpath is a lock key (`spec-lock.md` "Entry", `symbol`), and `spec lock rebase` keeps it byte for byte (`docs/features/lock-rebase.md`). No entry's bytes changed when this shipped: an ambiguous key was never locked, and the qpath is not hashed. Verify matches keys exactly, with no undiscriminated or ordinal fallback (`spec-verify.md` "States"):

- An item locked under a unique qpath that gains an in-file twin: its entry reads `broken_binding symbol_gone`, its marker's new key `unlocked`.
- A group whose membership or a member's `cfg` changes (a twin inserted, removed or moved; rule 1 starting or ceasing to hold) may rename members. The vanished key reads `symbol_gone`, a key now naming an unmarked twin `marker_gone`, a key now naming another twin is compared with that twin's digest, and the moved marker's new key reads `unlocked`. Never all `ok`. 5c pairs only a vanished key, by `body` (`docs/features/anchor-states.md` "The pairing").
- A later change of these rules (rule order, the predicate text, ` #`) reads `symbol_gone` plus `unlocked` until re-accepted. `CODE_INDEX_FORMAT` covers them (`symbol-index.md` "Analyzer stamp"): its value 4 holds every rule here (2 only keeps a package label's `\` on Unix: a binding under such a package reads `symbol_gone` until re-accepted; 3 only adds role rows, `code-roles.md`; 4 only compacts a role subject's `<T as Tr>`), the skipped comments and the trimmed cut included.

**Drift re-key**: a drift item of a node that holds a twin marker carries `symbol` and `state` in `drift_hash` (`proposal-drift-item.md` "The item"). When a name changes, an open item reads `evidence changed since it was raised` on the next raise, and a rejected one is raised again once, as after a rebase (`proposal-drift.md` "Known limits").

## Pins

`specengine-code` `tests/symbol_discriminator.rs`, `symbol_signature.rs` (`signature_covers_every_body_field_row` reads `BODY_FIELDS`), `symbol_halves.rs`; CLI `tests/symbol_twins.rs` (every verify side, the ECS fixture); `fixtures/code-a/expected.json` (`impl Tally #1`, `#2`).

## Known limits

- Ordinals shift: inserting or removing a twin renames its later siblings. They are revisited from `code-index`'s `twins` counts (6a-2's TE-8) before a pilot locks a twin.
