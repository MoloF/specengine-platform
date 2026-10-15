---
class: canon
tier: 2
scope: [crates/specengine-code, crates/specengine-eval]
owner: owner
reviewed: 2026-10-16
---

# Code hash: recipe v2 and the v3 levels

Layer A's digests, tree-sitter only. **v2** (`hash.rs`), one per item: what `spec.lock` format 1 records (`spec-lock.md` "Entry"). **v3** (`levels.rs`): an item's levels and fingerprints beside v2, no v2 byte changed; recorded from 5b-1 (`lock-levels.md`). A recorded digest changes only with its recipe name and a rebase of every lock (ADR-0050).

## Recipe v2

`specengine-hash/v2`, from limpet, mago-fingerprint and the research (04 s1.8), amended by measurement: `RECIPE`, `recipe_header()`, `hash_item` → `HashState`, `hash_bound_items`.

    hash(item) = BLAKE3( header || walk(attrs(item)) || walk(item, skip = name_node(item)) )
    header     = put("specengine-hash/v2") || put(tree-sitter-rust version) || put(abi_version())
    put(b)     = len(b) as u32 little-endian || b
    walk(n) = n.kind() in {line_comment, block_comment} -> ""   // by kind(), never is_extra()
              n is the anonymous ","                    -> ""   // rustfmt trailing commas
              n matches N1-N4  -> walk(normal form of n)
              n is a leaf      -> put(kind(n)) || put(text(n))
              else             -> "(" || put(kind(n)) || walk(c) for each child || ")"
    item.has_error() -> cannot_verify, no hash

`attrs(item)`: the attached attribute run (tree-sitter-rust keeps attributes as siblings). Normalisations, exactly the constructs rustfmt toggles:

- **N1** a `;` after an absent or diverging tail is transparent (edition 2024 adds it after diverging tails);
- **N2** a label-free single-expression block in a closure body or a match-arm value is unwrapped (`|x| { x + 1 }` = `|x| x + 1`);
- **N3** `{...}` token trees after `|`, `||` or `=>` inside `(`/`[`-delimited expression macros are transparent;
- **N4** a run of adjacent `use` items is sorted, each member its attached attributes || its `use`; bounded at `MAX_USE_RUN_NESTING` (64), past it `nesting_too_deep`.

Hashed: identifiers inside the body; attributes (`#[derive]`, `#[cfg]`, `#[require(..)]`: they change behaviour). Not hashed: the item's own name (a rename keeps the hash), comments with doc comments, whitespace, the `,` and N1-N4 forms. Never as input: `to_sexp()` (drops anonymous nodes and leaf text, `a+b` = `a-b`), `kind_id()`/`grammar_id()` (table indices, drift on every grammar update), `grammar_name()`, a derived `Hash` of `Node` (the pointer). **Trap 1**: `is_extra()` is true for ERROR nodes, so filtering by it gives every unparsed file one hash.

## Entry points

v3, at the crate root (module `levels`); digests are `Digest`. Callers keep a file's text and binding, never a node, and re-parse to hash by index.

| Name | Shape |
|---|---|
| `LEVEL_RECIPE`, `level_header()` | `"specengine-hash/v3"`; `&'static [u8]`, "Levels" |
| `LevelState` | `Hashed(LevelHashes)` or `CannotVerify { categories }`, sorted, the first the detail |
| `LevelHashes` | `sig`, `body`, `deps`, `skeleton`, `quote`: `Digest`; `body_bytes: u32`; `is_trivial()` |
| `TRIVIAL_BODY_BYTES` | `u32` 124, "Triviality" |
| `item_levels(tree, source, items)` | `bind.rs`, `hash_bound_items`' twin: indexes into `bind_tree`'s `RustBinding::analysis` (`analyze_tree`'s order too), in the order asked, `None` past the items, never reading `ItemRecord.state`; an inline `mod`'s module (`Module(Some(i))`) is item `i`'s |
| `module_levels(tree, source)` | the file's module (`Module(None)`), "File module" |
| `BODY_FIELDS`, `BodyField`, `BodySlot`, `body_nodes(item)` | "The body field"; `body_nodes` in source order, empty for none |
| `call_targets(node, source)` | the texts `deps` hashes, sorted; on `source_file` every call in the file |
| `grammar::NODE_TYPES` | `tree_sitter_rust::NODE_TYPES` for the body pin: the crate reads no JSON |

The RON value hash (`ron::value_hashes`): `docs/canon/code-hash-ron.md`, over the crate-private `hash::put`, `levels::{cr_rule, Stream::tagged, deps_digest}`.

## Levels

Each digest = BLAKE3(`level_header() ‖ put(tag) ‖ stream`); `level_header()` = `put(LEVEL_RECIPE) ‖ put(TREE_SITTER_RUST_VERSION) ‖ put(abi_version())`, its own, never v2's. No path, line, offset or time enters a stream: equal bytes, equal digests.

| Level | Covers | Stream after the tag |
|---|---|---|
| `sig` | attached attributes and the item without its name and body nodes; parameter names, `#[doc = ".."]` count | `walk3(attr)… ‖ walk3(item, skip = {name, body})` |
| `body` | the body node(s), "The body field"; none: the empty stream | `walk3(body)…` |
| `deps` | the call targets inside the body nodes | `put(target)…`, byte-sorted, deduplicated |
| `path` | the lock key `(file, kind, symbol)`, never hashed | — |

`sig` and `body` split exactly the leaves `hash_item` reads: the default `[sig, body]` sees a change iff v2 does, N5 and the CR rule aside.

- **`walk3`**: v2's walk (N1-N3 through v2's frames, N4) plus **N5**: a run of `mod x;` (a `mod_item` without a body), or of `extern_crate_declaration`s, siblings with comments between, is emitted in the byte order of its members' walks, a member its attached attributes ‖ itself (rustfmt `reorder_modules`, `reorder_imports`). A member carrying `#[macro_use]`, `#[macro_use(..)]` or a `#[cfg_attr(pred, ..)]` with either after `pred` (a nested `cfg_attr` unread) breaks the run and stays in place: macro scope follows textual order. Runs of every class count against `MAX_USE_RUN_NESTING`. An explicit task stack, runs included; one pass feeds every stream.
- **CR rule** (rustc reads CRLF as LF): a CR right before an LF is dropped in every leaf text (so in `body_bytes`), `deps` target and `quote`'s `head` and `tail`; `before` and `after` lose one trailing CR, the last line's and a lone CR at EOF included. An LF and a CRLF checkout read alike.
- **A `deps` target**: a `call_expression`'s `function`, a `generic_function` replaced by its own: an `identifier` or `scoped_identifier` → its leaf texts, comments and every `type_arguments` right after a `::` dropped with that `::` (`Vec::<u8>::new` → `Vec::new`), one space where two word characters (letter, digit, `_`) would touch (`<T as Trait>::f`); a `field_expression` → `.` ‖ its `field` (`x.collect::<V>()` → `.collect`); any other (`f()()`, `(|x| x)(..)`, `v[0](..)`) → its `kind()` only, so such a target stays bounded (a qualified one is not: "Known limits"). A `macro_invocation` → its `macro` field joined alike ‖ `!`. Every call in the body counts, closures and nested items included; a constructor (`Some(x)`) is a call; a function passed as a value is `body`, not `deps`.

## The body field

`BODY_FIELDS`: one `BodyField { kind, field: BodySlot, types, required }` per `ITEM_KINDS` and `MEMBER_KINDS` kind; `BodySlot::{Absent, Field(name), Children}`, `Children` every named child of a listed type; `types` as tree-sitter-rust 0.24.2's `node-types.json` names them (`_expression`, `_type`). Pinned to `grammar::NODE_TYPES` by eval `body_field_table_matches_the_grammar`, re-pinned with every grammar bump.

| Kind | Slot: types (required) | No body: the empty stream |
|---|---|---|
| `function_item` | `body`: `block` (yes) | — |
| `function_signature_item` | `Absent` | always |
| `struct_item` | `body`: `field_declaration_list`, `ordered_field_declaration_list` (no) | unit struct |
| `enum_item`, `union_item`, `trait_item` | `body`: `enum_variant_list`, `field_declaration_list`, `declaration_list` (yes) | — |
| `impl_item`, `mod_item` | `body`: `declaration_list` (no) | `impl … ;`, `mod x;` |
| `const_item`, `static_item` | `value`: `_expression` (no) | `const X: T;`, `static X: T;` |
| `type_item` | `type`: `_type` (yes) | — |
| `macro_definition` | `Children`: `macro_rule` (no); delimiters and `;` in `sig` | — |

## File module

`module_levels`, a `//!` marker's binding (`code-bindings.md` "Rust binding" rule 5). Any `ERROR` or `MISSING` node in the file → `CannotVerify` with `error_categories(root)`; runs past the cap → `[NestingTooDeep]`.

- `sig`: the leading run of `source_file`'s `inner_attribute_item`s; a comment or a `shebang` does not end it (the shebang's leaf stays in `body`), any other node does.
- `body`: the file's walk without that run, N5 included, counted by `body_bytes`. `deps`: every call in the file.
- `skeleton`: the masked file. `quote`: `put("") ‖ put(text) ‖ put("") ‖ put("")`, `text` from the first child's start to the last child's end, ASCII whitespace trimmed at both ends (no child: empty).
- An inline `mod x { #![..] }` is an item: its `#![..]` is in `body`.

## Fingerprints

Same header, own tag. Evidence for following (5c), never a judge: verify compares none (5b-1 stores them), none is a drift item's fact.

| Fingerprint | Definition |
|---|---|
| `skeleton` | `walk3` of the attributes and the item without its name, a named leaf as `put(kind)` only (identifiers, literals, primitive types masked); anonymous leaves and structure kept, so `boolean_literal`'s `true`/`false` stay; runs sorted by their masked walks |
| `quote` | `put(before) ‖ put(head) ‖ put(tail) ‖ put(after)`: `head`, `tail` the source from the first attached attribute (else the item) to the item's end, split around the name (no name: `tail` empty); `before`, `after` the nearest non-blank line before `head`'s line and after the item's last line, spaces and tabs trimmed (none: empty). A neighbour's edit changes it by design |
| `body_bytes` | `u32`, not hashed: the text lengths of the leaves `walk3(body)` emits (comments, `,`, N1-N3 forms excluded), summed, saturating |

**Triviality**: `body_bytes < TRIVIAL_BODY_BYTES` (124, 08 AC-15), `LevelHashes::is_trivial()` the one comparison (5c pins a copy); a trivial item is never followed. No recipe, lock or index byte holds it.

**Broken** (08 AC-14): an item with `has_error()` on itself or an attached attribute, or a run past the cap, gets no level and no fingerprint: `CannotVerify` with `hash_item`'s categories.

## Measured

- v2, `specengine-eval ast-hash`, both pilots (11 852 + 7 599 items): 100.0 % stable under default rustfmt, a contrasting rustfmt (`max_width = 60`) and comment stripping with `///`; broken items never share a hash; v1 (no N1-N4) 84.9-85.4 % under the contrasting format. A comment, whitespace or neighbour edit keeps an item's hash.
- v3, a developer run (release, this repository, 325 kLOC): `bind_tree` plus every item's and module's levels ≈ 4.9 ms/kLOC (1.61 s; v2's `analyze_tree` 0.56 s). 5a-2's method and figures: `code-hash-eval.md` "Measured".

## Known limits

- N3 hashes token-level DSL macros alike with and without their significant `| {` / `=> {` braces (remedy, unbuilt: a per-project macro opt-out list); a nested brace-delimited macro inside an expression macro inherits N3. v2 leaves `mod x;` and `extern crate` runs unsorted (v3: N5, resting on rustfmt's rules, unchecked).
- `deps` misses calls inside a macro's token tree. tree-sitter-rust 0.24.2 fails on some valid code (macro `$`/`~`, multi-line `where`; ≈ 1 % by research, 0 items on the pilots): recovery is local, an error refuses the whole unit.
- Crafted input: a qualified (`scoped_identifier`) callee keeps its whole text, so `<[u8; { <…>::f() }] as T>::f()` nested n deep costs O(n²) in `deps` (owner-accepted, 5a-1 H7); bounding it would change the pinned `<T as Trait>::f` targets.
- tree-sitter-rust 0.24.2's `shebang` also matches `#! [x]` (a space), which rustc reads as an inner attribute: a shebang leaf in the module's `body`; rustfmt's `#![x]` moves it to `sig`.
