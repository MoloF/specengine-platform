---
class: canon
tier: 1
scope: [crates/specengine-code]
owner: owner
reviewed: 2026-10-20
---

# specengine-code — layer A: Rust and RON parsing, AST hash, markers, Bevy detector

Layer A of 05 §5.1: tree-sitter only — no build, no name resolution. A default workspace member in the core graph: it never depends on `ra_ap_*`, `syn` or Bevy (`specengine-eval` `tests/build_graph.rs` asserts it). Pins: `tree-sitter =0.27.0`, `tree-sitter-rust =0.24.2` (ABI 15), `blake3`. Callers: `specengine-eval`, the CLI (`spec index`, `spec refs`, `spec lock`, `spec verify`, `spec resolve`), `specengine-ra`'s tests (a dev-dependency: `layer_c`, `analyze_file`). `CODE_INDEX_FORMAT` (5) stamps every rule whose output the code index stores: `docs/canon/symbol-index.md` "Analyzer stamp". Target units, marker grammar, RON binding and ambiguity: `docs/canon/code-identity.md`; in-file twins and signatures: `docs/canon/symbol-names.md`; hashes: `docs/canon/code-hash.md`.

## Modules

| Module | What it does |
|---|---|
| `grammar` | `TREE_SITTER_VERSION`, `TREE_SITTER_RUST_VERSION` (a test checks them against the built artefact), `grammar_info()` (ABI), `RustParser`, `NODE_TYPES` (the body-table pin) |
| `hash` | recipe `specengine-hash/v2` ("Hash recipe"): `RECIPE` header, iterative walk. `hash_item` → `HashState`: a `Digest`, or `cannot_verify` with an `ErrorCategory` for an item with `has_error()` or a `use` run nested past `MAX_USE_RUN_NESTING = 64` (`nesting_too_deep`) — never hashed; `NotHashed` only from the binder. `normalize` returns `false` for an incomplete stream |
| `levels` | recipe `specengine-hash/v3`: `LevelState`, `LevelHashes` (`is_trivial`, `TRIVIAL_BODY_BYTES`), `module_levels`, `BODY_FIELDS` / `body_nodes`, `call_targets`, `empty_deps_digest` (a `ron` entry's `deps_hash`); N5, N6, the CR rule; one explicit-stack pass per unit |
| `comments` | `comment_ranges`, `strip_comments` (`///` included) — the comment perturbation |
| `items` | `analyze_file` / `analyze_tree` → `FileAnalysis`: items (`ITEM_KINDS`; members `MEMBER_KINDS`) with their hash state, `mod` declarations (`MOD_KIND`), `#[path]` attributes; collected on an explicit stack; `hashed`; `ItemRecord.cfg`, the `#[cfg(..)]` chain outermost first |
| `qpath` | `file_role` / `TargetIndex` (`primary()` too) over a caller's `PackageTargets` (`cargo metadata`, else `layout_targets`): `FileRole` (`CrateRoot`, `Module`, `Unrooted`) with its `Unit` — none for the primary target, `<kind>:<name>`, `shared:<dir>`; `qpath`, `resolve_path_attribute`; `QPath::segments`, `suffixes(discriminator)` (the index's suffix rows, never a split); one `Ambiguity` per item (`PathAttribute`, `Unrooted`, `Duplicate`), reported, never guessed |
| `markers` | `markers_in`: `Relation` (`implements`, `verifies`, `configures`, `assumes`) and `Marker` (ID, optional rev, `Levels`: `Default` = `[sig, body]`, `Declared` in canonical order, `Invalid(LevelError)`; note, `id_latin`); several per comment, shared by `.rs` and `.ron`; `token_range`; `#` Latin; a non-Latin ID clears `id_latin`, an empty ID is counted apart, nothing is fatal (ADR-0009) |
| `ron` (`mod`, `lexer`, `structure`) | own RON lexer (`lex`: comments, `#![enable(..)]`, raw strings, `LexError` categories) and a tolerant structure walk. `analyze` → `RonAnalysis`: every marker with an `Anchor` (`Path` (`root.player.speed`), `Ambiguous`, `Unanchored`, `CannotVerify`), `colliding_groups`, `Rejected` categories, comment byte ranges. Segments, rules 1–5, ambiguity, caps `MAX_SEGMENT_BYTES = 128`, `MAX_DEPTH = 512`: `docs/canon/code-identity.md`; examples: `ron/structure.rs`'s module doc. `comment_anchors`: every comment's anchor, marked or not (mentions); `value_hashes` → `RonValues`: `Path` comments' value hashes or a refusal |
| `bind` | `bind_file` / `bind_tree` → `RustBinding {analysis, comments: [BoundComment {range, binding, mention}]}`: each comment's `Binding` (`Item`, `Module`, `Unanchored`, `CannotVerify`; `File`: mentions), `docs/canon/code-bindings.md` "Rust binding". Errors-only analysis: an item with `has_error()` keeps its `ErrorCategory`s, an error-free one is `HashState::NotHashed`, `hashed == false`; only `bind.rs` reads it (pinned: `code_bind.rs`, CLI `code_refs_walk.rs`). `hash_bound_items`: chosen bound items' v2 hashes (`spec.lock` format 1); `item_levels`, its v3 twin (format 2); `item_signatures`, the display text by the same indexes (`SIGNATURE_CAP_BYTES` 512; never hashed) |
| `const_value` | `const_value` -> `ConstValue`: a `const` / `static`'s string literal as written, or a `NotValued` category; linear, stores nothing (`docs/canon/code-constants.md`) |
| `naming` | `name_files` → `Names`: a tree's items and modules named at once, `qpath` and one ambiguity each, the composition of `name_file` (`indexed_name_file` over a package's prepared `TargetIndex`; `file_place`: label, role; `placed_name_file` -> `PlacedNames`: both and each item's `QPath`, once; one file → `FileNames {module, items, shared}`, in-file twins discriminated: `Name.discriminator`, rendered after `DISCRIMINATOR_SEPARATOR` ` #`) and `tree_passes` (`path_targets`, `path_attribute`, cross-file `duplicate` by `Name::undiscriminated`); `ROOT_PACKAGE` `.`; `dir_text`: a root-relative directory's one spelling (a package label, the CLI code index's directories); `query_name`: a lookup query's `name` key, the one reader of `::` in a query (`symbol-reads.md` "Lookup") |
| `frameworks` | framework roles as data: `KNOWN` (`[code] frameworks`' values), `FRAMEWORKS` (`BEVY`: words, sources, categories, each word's item kinds; `words`, `item_kinds`), `role_facts` -> a file's `RoleFact`s on its one tree, by first byte; `PATH_FORM`, `CODE_ORIGIN`, the plain form and origin a read's text leaves out; core, store and CLI hold none of the words (`docs/canon/code-roles.md`) |
| `bevy` (`mod`, `expr`, `tokens`) | syntactic registration detector (05 §5.1): `detect` / `detect_file` → `BevyAnalysis` (systems, observers, plugins, plugin uses; `Uncertain` by `UncertainCategory`); `expr` reads code from the syntax tree, `tokens` macro bodies and arguments, names only; `MAX_NESTING = 256`, `MAX_TEXT_BYTES = 128`. What it reads and the role facts over it: `docs/canon/code-roles.md` "Role facts" |
| `layer_c` | layer C's words as data: `KIND_MAP` (13 rows, `ItemKind::ALL` order: a layer C kind → the layer A node kinds it stands for), `layer_a_kinds`, `kind_matches` (a row, else `<kind>_item`; `docs/canon/layer-c-protocol.md` "Kind map"); for `spec resolve` (`docs/canon/layer-c.md`): `TREE_FILES` (6: `Cargo.lock`, `Cargo.toml`, the toolchain pins, cargo configs), `tree_files(walked paths)` (each name in the root and every directory from a walked file's up to it); `join_file` (`JoinItem`s on equal `(start, end)` and `kind_matches`); `target_crate` -> `TargetCrate` (a file's one target, `-` → `_`, its modules), `moniker_crate`, `moniker_modules` (leading `name/`s before the item's own), `agrees`; `collision_groups`, `collision_values`, `shown_moniker`, `COLLISION_SEPARATOR` (` #`) |

## Rules

- What cannot be read is `cannot_verify`, `uncertain` or `unanchored` with a category — never a guessed name and never a shared hash (trap 1, "Hash recipe").
- Only `kind()` strings, never `kind_id()`. A grammar bump changes both recipe headers, re-pins `BODY_FIELDS` and is re-measured with `specengine-eval ast-hash` and `ron` before it lands.
- Cost is linear in the input and nothing recurses over it; the caps above bound crafted input. Exceptions: "Known limits" (RON; a qualified callee in `deps`).
- No subject domain (ADR-0008): RON and Bevy are language and framework support; project specifics, the Cargo target table included, arrive from the caller; `qpath` knows only Cargo's names.

## Hash recipe

`specengine-hash/v2`, the whole-item hash (`hash`), and `specengine-hash/v3`, an item's levels `sig`, `body`, `deps` and fingerprints `skeleton`, `quote`, `body_bytes` beside it (`levels`): `docs/canon/code-hash.md` (formulas, N1-N6, the CR rule, traps, limits). A RON value's hash (`ron::value_hashes`): `docs/canon/code-hash-ron.md`.

## Known limits and open minors

- Units, markers, the cargo runner: `docs/canon/code-identity.md` "Known limits".
- Hash (N3 DSL macros; v2's unsorted `mod x;`, `extern crate` runs; a qualified callee's O(n²) `deps` on crafted nesting; `#! [x]` as a shebang): `docs/canon/code-hash.md` "Known limits".
- RON: markers pending when error recovery consumes the next token are `Unanchored` (or their trailing fallback), not `CannotVerify`; crafted input costs more than linear in three places — an unterminated raw string with thousands of `#`, tens of thousands of markers on one comment line, paths up to ≈ 512 × 136 bytes per marker at maximal nesting.
- Bevy: the token reader (macro bodies, arguments) misreads commas in type positions, unclosed or `::<<` turbofishes, metavariable paths and `P::<T>::default()`, each pinned by a named test in `tests/bevy_detector.rs`; `BevyAnalysis` is sorted by line only (one line's chain reversed), role rows keep source order (`docs/canon/code-roles.md` "Role facts").
- Invisible by design: macro-generated items, `inventory` / `linkme` registration, generic instances of one system (layer B or C, 05 §5.1).

## Tests

`tests/{hash_traps,hash_v2,hash_depth}.rs`; `tests/hash_levels_{units,library,props,depth,ron}.rs`, `tests/lock_levels_deps.rs`; `tests/{qpath_units,marker_levels,marker_tokens,code_bind,const_value}.rs`; `tests/symbol_{discriminator,signature,halves}.rs`; `tests/{ron_markers,ron_depth,ron_cost,ron_ambiguity}.rs` (`ADJACENCY`: 53 cases × LF / CRLF, each adjacency rule with a named mutation); `tests/bevy_{detector,roles_detector}.rs`. Fixtures `fixtures/{ast-hash,hash-levels,ron,bevy-mini,cargo-units}`, each with `expected.json`; `bevy-mini` and `cargo-units` (own `[workspace]`, also run by `specengine-eval`) are Cargo projects outside this workspace, only parsed.
