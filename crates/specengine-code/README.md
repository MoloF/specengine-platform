---
class: canon
tier: 1
scope: [crates/specengine-code]
owner: owner
reviewed: 2026-10-10
---

# specengine-code — layer A: Rust and RON parsing, AST hash, markers, Bevy detector

Layer A of 05 §5.1: tree-sitter only — no build, no name resolution. A default workspace member in the core graph: it never depends on `ra_ap_*`, `syn` or Bevy (`specengine-eval` `tests/build_graph.rs` asserts it). Pins: `tree-sitter =0.27.0`, `tree-sitter-rust =0.24.2` (ABI 15), `blake3`. Callers: `specengine-eval` and the CLI (`spec refs`); the Phase 3 index and `spec verify` build on it. Target units, marker grammar, RON binding and ambiguity: `docs/canon/code-identity.md`.

## Modules

| Module | What it does |
|---|---|
| `grammar` | `TREE_SITTER_VERSION`, `TREE_SITTER_RUST_VERSION` (a test checks them against the built artefact), `grammar_info()` (ABI), `RustParser` |
| `hash` | recipe `specengine-hash/v2` ("Hash recipe"): `RECIPE` header, iterative walk. `hash_item` → `HashState`: a `Digest`, or `cannot_verify` with an `ErrorCategory` for an item with `has_error()` or a `use` run nested past `MAX_USE_RUN_NESTING = 64` (`nesting_too_deep`) — never hashed. `normalize` returns `false` for an incomplete stream |
| `comments` | `comment_ranges`, `strip_comments` (`///` included) — the comment perturbation |
| `items` | `analyze_file` / `analyze_tree` → `FileAnalysis`: items (`ITEM_KINDS`; members `MEMBER_KINDS`) with their hash state, `mod` declarations, `#[path]` attributes; collected on an explicit stack; `hashed` |
| `qpath` | `file_role` / `TargetIndex` over a caller's `PackageTargets` (`cargo metadata`, else `layout_targets`): `FileRole` (`CrateRoot`, `Module`, `Unrooted`) with its `Unit` — none for the primary target, `<kind>:<name>`, `shared:<dir>`; `qpath`, `resolve_path_attribute`; one `Ambiguity` per item (`PathAttribute`, `Unrooted`, `Duplicate`), reported, never guessed |
| `markers` | `markers_in`: `Relation` (`implements`, `verifies`, `configures`, `assumes`) and `Marker` (ID, optional rev, `Levels`: `Default` = `[sig, body]`, `Declared` in canonical order, `Invalid(LevelError)`; note, `id_latin`); several per comment, shared by `.rs` and `.ron`; `token_range`; `#` Latin; a non-Latin ID clears `id_latin`, an empty ID is counted apart, nothing is fatal (ADR-0009) |
| `ron` (`mod`, `lexer`, `structure`) | own RON lexer (`lex`: comments, `#![enable(..)]`, raw strings, `LexError` categories) and a tolerant structure walk. `analyze` → `RonAnalysis`: every marker with an `Anchor` (`Path` such as `root.player.speed`, `Ambiguous` when sibling segments render alike, `Unanchored`, `CannotVerify`), `colliding_groups`, `Rejected` categories, comment byte ranges. Segments, binding rules 1–5, ambiguity, the caps `MAX_SEGMENT_BYTES = 128`, `MAX_DEPTH = 512`: `docs/canon/code-identity.md`; every rule with examples: module doc of `ron/structure.rs`. `comment_anchors`: every comment's anchor, marked or not (mentions) |
| `bind` | `bind_file` / `bind_tree` → `RustBinding {analysis, comments: [BoundComment {range, binding, mention}]}`: each comment's `Binding` (`Item`, `Module`, `Unanchored`, `CannotVerify`; `File`: mentions), `docs/canon/code-bindings.md` "Rust binding". Errors-only analysis: an item with `has_error()` keeps its `ErrorCategory`s, an error-free one holds `HashState::CannotVerify { categories: [] }` (no hash gives it), `hashed == false`; only `bind.rs` reads it (pinned: `code_bind.rs`, CLI `code_refs_walk.rs`) |
| `naming` | `name_files` → `Names`: a tree's items and modules named at once, `qpath` and one ambiguity each; `ROOT_PACKAGE` `.` |
| `bevy` (`mod`, `expr`, `tokens`) | syntactic registration detector (05 §5.1): `detect` / `detect_file` → `BevyAnalysis` — systems of `.add_systems(Schedule, ...)` and the one-argument `Schedule::add_systems(...)` (tuples on an explicit stack to `MAX_NESTING = 256`, `COMBINATORS` peeled, `ADAPTERS` recorded; a leaf is a path such as `f::<T>`, a closure named by its innermost enclosing `fn`, or a factory call named by its callee), observers (`.add_observer`), plugins (`impl Plugin for`, and every `fn` whose only parameter is `&mut App`, qualified or generic, not a `self` method, not `-> &mut App`; `PluginKind`), plugin uses (`.add_plugins`), `Uncertain` with an `UncertainCategory` (`macro_in_arguments`, `metavariable`, `unknown_method`, `expression`, `arguments`, `nesting_too_deep`, `parse_error`). `expr` reads ordinary code exactly from the syntax tree; `tokens` reads `macro_rules!` transcribers and macro arguments from flattened tokens, names only. Texts capped at `MAX_TEXT_BYTES = 128` plus `…` |

## Rules

- What cannot be read is `cannot_verify`, `uncertain` or `unanchored` with a category — never a guessed name and never a shared hash (trap 1, "Hash recipe").
- Only `kind()` strings, never `kind_id()`. A grammar bump changes the recipe header and is re-measured with `specengine-eval ast-hash` and `ron` before it lands.
- Cost is linear in the input and nothing recurses over it; the caps above bound crafted input.
- No subject domain (ADR-0008): RON and Bevy are language and framework support; project specifics, the Cargo target table included, arrive from the caller; `qpath` knows only Cargo's names.

## Hash recipe

`specengine-hash/v2`, from limpet (`src/memory/anchor.rs`), mago-fingerprint and the research (04 s1.8), amended by measurement; the Phase 3 levels, anchor states and rename following on top of it: 05 s5.2.

    hash(item) = BLAKE3( header || walk(attrs(item)) || walk(item, skip = name_node(item)) )
    header     = "specengine-hash/v2" || tree-sitter-rust version || abi_version()
    walk(n) = n.kind() in {line_comment, block_comment} -> ""   // by kind(), never is_extra()
              n is the anonymous ","                    -> ""   // rustfmt trailing commas
              n matches N1-N4  -> walk(normal form of n)
              n is a leaf      -> len||kind(n) || len||text(n)  // len = u32 little-endian
              else             -> "(" || len||kind(n) || walk(c) for each child || ")"
    item.has_error() -> cannot_verify, no hash

`attrs(item)` is the attached attribute run (tree-sitter-rust keeps attributes as siblings of the item). Normalisations, exactly the constructs rustfmt toggles:

- **N1** a `;` after an absent or diverging tail is transparent (edition 2024 adds it after diverging tails);
- **N2** a label-free single-expression block in a closure body or a match-arm value is unwrapped (`|x| { x + 1 }` = `|x| x + 1`);
- **N3** `{...}` token trees after `|`, `||` or `=>` inside `(`/`[`-delimited expression macros are transparent;
- **N4** a run of adjacent `use` items is sorted, each member its attached attributes || its `use`; the member walk is bounded at `MAX_USE_RUN_NESTING`.

Hashed: identifiers inside the body; attributes (`#[derive]`, `#[cfg]`, `#[require(..)]`: they change behaviour). Not hashed: the item's own name (a pure rename keeps the hash), comments with doc comments, whitespace (not a node), the `,` and N1-N4 forms. Never as input: `to_sexp()` (drops anonymous nodes and leaf text, `a+b` = `a-b`), `kind_id()`/`grammar_id()` (table indices, drift on every grammar update), `grammar_name()` (the raw alias name), a derived `Hash` of `Node` (the pointer). Trap 1: `is_extra()` is true for ERROR nodes, so filtering by it gives every unparsed file one hash. tree-sitter-rust 0.24.2 fails on some valid code (macro `$`/`~`, multi-line `where`; about 1 % by research estimate, 0 items on the pilots); recovery is local.

**Measured** (`specengine-eval ast-hash`, both pilots, 11 852 + 7 599 items): 100.0 % stable under default rustfmt, a contrasting rustfmt (`max_width = 60`) and comment stripping with `///`; broken items never share a hash; v1 (no N1-N4) 84.9-85.4 % under the contrasting format. Editing a comment, whitespace or a neighbouring item does not change an item's hash.

## Known limits and open minors

- Units, markers, the cargo runner: `docs/canon/code-identity.md` "Known limits".
- Hash: N3 hashes token-level DSL macros the same with and without their significant `| {` / `=> {` braces (remedy: a per-project opt-out list of macro names in `specengine.toml`, not built); a nested brace-delimited macro inside an expression macro inherits N3; `mod x;` and `extern crate` runs reordered by rustfmt are not sorted (v3 candidate).
- RON: markers pending when error recovery consumes the next token are `Unanchored` (or their trailing fallback), not `CannotVerify`; crafted input costs more than linear in three places — an unterminated raw string with thousands of `#`, tens of thousands of markers on one comment line, paths up to ≈ 512 × 136 bytes per marker at maximal nesting.
- Bevy token reader (macro bodies and arguments only; pinned by tests): a comma inside a type position or closure parameters splits an element — `|q: Query<A, B>| …` as a whole argument gives `arguments` uncertain, and with ≥ 3 generic items a middle path fragment can be registered as a name; `a::<` unclosed at an element's end reads as `a`; tokens after a path or postfix chain → `expression`; `sys.$m(c)` and `systems::$name` → `expression` (should be `metavariable`); `::<<` opens a turbofish; `plugin_uses` names `P::<T>::default()` as `">"` and has no leftover rule; its `piped` list differs from the code reader's.
- Bevy output is sorted by line only: calls of one chain on one line come out reversed, while the module doc claims source order.
- Invisible by design: macro-generated items, `inventory` / `linkme` registration, generic instances of one system (layer B or C, 05 §5.1).

## Tests

`tests/{hash_traps,hash_v2,hash_depth}.rs`; `tests/{qpath_units,marker_levels,marker_tokens,code_bind}.rs`; `tests/{ron_markers,ron_depth,ron_cost,ron_ambiguity}.rs` (`ADJACENCY`: 53 cases × LF / CRLF, each adjacency rule with a named mutation); `tests/bevy_detector.rs`. Fixtures `fixtures/{ast-hash,ron,bevy-mini,cargo-units}`, each with `expected.json`; `bevy-mini` and `cargo-units` (own `[workspace]`, also run by `specengine-eval`) are Cargo projects outside this workspace, only parsed. Run one file: `cargo nextest run -p specengine-code --test <file>`.
