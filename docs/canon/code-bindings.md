---
class: canon
tier: 2
scope: [crates/specengine-code, crates/specengine-cli, crates/specengine-core]
owner: owner
reviewed: 2026-10-16
---

# Code bindings: `[code]`, markers bound to code, `spec refs`

`spec refs REF` reads the code index, refreshed from the `[code]` tree per call (`docs/canon/symbol-index.md`); `lock accept` and `verify` walk the tree afresh. Each marker gets one Rust item, module or RON path, or a reason; its ID resolves as `spec show`'s REF, its `@N` meets the node's revision; ID citations in comments are weak mentions (ADR-0010, -0016, -0018, -0020: `docs/canon/architecture-code.md#markers`). Every state is listed, nothing blocked (ADR-0012). Code: core `code_toml.rs`; `specengine-code` `bind.rs`, `naming.rs`; CLI `code_walk.rs`, `code_index.rs`, `refs.rs`. Units, `qpath`, marker grammar, RON anchors: `docs/canon/code-identity.md`. Built by `docs/features/code-refs.md`.

## The `[code]` table

```toml
[code]
roots      = ["crates", "assets/data", "build.rs"]         # required, non-empty
exclude    = ["**/target/**", "crates/*/tests/fixtures/**"] # optional
frameworks = ["bevy"]                                       # optional: docs/canon/code-roles.md
```

- **No table: the code layer is off**, no default root (ADR-0008). `spec refs` then exits 2 with exactly `spec: <config> has no [code] table: the code layer is off (add [code] with roots)`, `<config>` as given (`specengine.toml` or `--config`'s).
- **Closed**, read by `CodeConfig::from_toml` only where code is read (`ProjectConfig` only name-checks `code`: a bad table fails no other command). Refused at the key's line, `<config>:<line>: message`, exit 2: `languages`, `crates`, `scip` (07 s5, reserved, "is not built"); any other key, `bevy` too (``… `[code]` has no key `<key>`: it takes `roots` and `exclude` and `frameworks` only``); a wrong type; `roots` missing or empty; a path not `is_clean_relative`, as `[paths]` (one trailing `/` and repeats dropped); a glob outside the `[paths] exclude` dialect (`*`, `**`, `?`; `[ ] { } \` refused; `docs/canon/gate.md` "Config"). `frameworks`: framework names whose item roles the code index stores, its shape checked here, its values against `specengine-code`'s list (`code-roles.md` "`[code] frameworks`").
- A root is a directory or a `.rs`/`.ron` file, looked up by exact name per component: missing (a symlink on the way too) -> `note: [code] root <path> does not exist`; another file -> `note: [code] root <path> is not a .rs or .ron file`; neither contributes.

## The walk

- Regular files named `*.rs`, `*.ron` under the roots minus `exclude`, as the spec walk lists `.md` (`crates/specengine-store/README.md` "Walk"): below a root, symlinks and `.`-names skipped; no `.gitignore`; root-relative `/` paths, byte-sorted, repeats dropped. A directory an exclude `G/**` covers is not listed (one unlistable inside it is no gap).
- **Not UTF-8**: content -> `note: <path> is not UTF-8: skipped`, counted `skipped`; a name -> counted `skipped`, no note, even where an exclude would drop it (a non-UTF-8 name is never matched). `.`-names and excluded files are neither files nor skipped.
- **Gaps**: a directory that cannot be listed (one on the way to a root too), a file that cannot be read, a Rust file the parser gives no tree -> exit 2, `spec: the code walk is incomplete: <gap>`, the first by path (a file before a directory of the same path), stdout empty, never a partial list (`spec index`: a `warning:`, exit 0).
- **Packages**: a `.rs` file's package is the nearest directory, its own up to the root, holding a regular `Cargo.toml`; its targets `layout_targets` over the package's walked files (no subprocess, no `cargo metadata`: `code-targets.md` "Target table"). No package: unrooted, package `.`.
- Each walked file and probed manifest is read once per call. The code index lists through this walk and stores what the rules below give (`symbol-index.md`); the walk stays `refs`' test oracle.

## Rust binding

*Item*: what `analyze_tree` collects, `ITEM_KINDS` at file level and directly in an inline `mod`, `MEMBER_KINDS` directly in an `impl` or `trait`, nothing in function bodies. *Comment*: line or block, doc comments too; a multi-line block comment trails by its `/*` line and leads by its `*/` line. The first rule that applies, 5 judged before 1-4 (as rustdoc reads it):

| # | The comment | Binds | Example |
|---|---|---|---|
| 5 | an inner doc comment (`//!`, `/*!`) at file level or directly in an inline `mod` | the file's module or that `mod` | `//! m` above `mod a;`: the file's module, never `a` |
| 1 | a block comment, an item starting later on its `*/` line (its first token or first outer attribute next) | that item | `/* m */ #[inline] fn f() {}` |
| 2 | on its own line, and the next token past blank lines, comments and outer attributes `#[..]` starts an item | that item | `// m`, `#[derive(Debug)]`, `struct S;` |
| 3 | after an item's last token on its line (several end there: the one right before) | that item | `fn f() {} // m`; `} // m` closing an `impl` |
| 4 | after an item's opening `{` on its line | that item | `impl S { // m` |
| 6 | anything else | `unanchored` + category | below |
| 7 | the item of 1-5 has `has_error()` (never a file module), or an `ERROR` node stands where an item would | `cannot_verify` + the hash's first `ErrorCategory` | an `fn` missing its `)` |

Categories, the first that applies: `in_macro` (a macro call or `macro_rules!` body: `foo! { // m`); `in_body` (a function body, block, `const`/`static` initializer: `let x = 1; // m`); `no_item` (followed, past what rule 2 crosses, by `}` or the file's end: a trailing comment on an impl's last associated `type`/`const`); `not_an_item`, everything else (`use`, `extern crate`, an item-position macro call, a field, a variant, an associated `const`/`type`, `#![..]`, an attribute's arguments `#[cfg(any( // m`, `#[derive(Debug)] // m` above `struct S;`). Strict first: fields and variants bind nothing yet.

A bound item gives its `qpath`, `kind()`, node's first line (attributes excluded) and one ambiguity, the first that applies: `path_attribute` (its file a `#[path]` target outside shared units), `unrooted`, `duplicate` (an item of another file renders the same undiscriminated qpath; in-file twins are named apart, `<qpath> #<value>`: `symbol-names.md`); shown, never resolved. A module: `<package>[::<unit>][::<module>…]`, the unit omitted for the primary target (`src/lib.rs` -> `crates/tally`, `tests/bounds.rs` -> `crates/tally::test:bounds`); an unrooted file's is its package alone; ambiguous by `path_attribute` or `unrooted`; an inline `mod`'s module takes its item's discriminated qpath and ambiguity (`duplicate` too). RON anchors as built (`code-identity.md` "RON binding", "RON ambiguity"): `ron <path>#<field path>`, `ambiguous duplicate`, `unanchored no_value`, `cannot_verify nesting_too_deep`. Linear (one explicit-stack walk, one lookup per comment); LF and CRLF alike.

## ID state

The token and the ID: `code-identity.md` "Marker grammar". The ID is re-read by `grammar::parse_reference` and resolved by `spec show`'s resolver (`crates/specengine-cli/README.md` "Rules"); a malformed rev leaves the ID before `@` (`GOAL-01@` resolves as `GOAL-01`).

| `id_state` | Meaning | Listed under |
|---|---|---|
| `resolved` | one node by its `id` (`slug/ID`, `ID#SECTION` too) | that node |
| `alias` | one node via `aliases_from` or `aliases:` | that node |
| `ambiguous` | several nodes (duplicates, several holders) | each |
| `homoglyph` | look-alike or mixed script, Latin `fix` shown, never resolved (ADR-0009) | the fix's node |
| `other_project` | a `project:` qualifier | none |
| `queue` | no node, and the queue's form `PR-` + digits (a project prefix `PR` resolves first); not looked up (slice 9, `health-code`) | none |
| `dangling` | else: empty ID, no reference, unknown prefix, no node | none |

## Revision state

R, the node's own revision (`docs/canon/node-revision.md` "Revision"): a document's front-matter `rev`, a section's `rev=N`, never inherited; none written -> 1. The marker's `@N`, the first that applies; `ambiguous` judged per node, `@assumes` like the others:

| `rev_state` | When |
|---|---|
| `unversioned` | no `@` after the ID |
| `bad_rev` | `@` not followed by 1-9 ASCII digits ending the token: `X@`, `X@3a`, `X@1234567890` |
| `rev_unknown` | `id_state` not `resolved`, `alias`, `ambiguous`; or R written but unreadable (`rev=0`) |
| `current` | N = R |
| `spec_ahead` | N < R: the marker was not raised |
| `predated` | N > R: no such revision |

## Mentions

`grammar::scan` over every Rust and RON comment, doc comments too; never string literals or code. The hit at a marker's own token is skipped; others count, a note's included. A mention has an `id_state`, no `rev_state`, and belongs to its comment's binding when that is an item, a module, a RON path or `cannot_verify`; an unanchored Rust comment's mention to the innermost item enclosing it, else the file; an unanchored RON comment's to the file. Weak: `MENTIONS`, never a binding.

## `spec refs`

`spec refs REF [--json]`. REF: every `spec show` form (`@rev` ignored with a `note:`; look-alike, mixed script, `project:` -> exit 2; several holders -> one `warning:`, a block each; a `.md` path with no node -> exit 1, `has no node`). A block lists what names its node, never its sections or parents. Order of work: config, REF, the code index's refresh (`symbol-index.md` "spec refs and spec index"). The rows, JSON and exits: `docs/canon/spec-cli-refs.md`.

- Order: markers by relation (`implements`, `verifies`, `configures`, `assumes`), path bytes, line, column; then mentions by path, line, column; the summary last, counting every row of every block (a marker under two holders twice), cut ones too.
- Cap `OUTPUT_CAP_CHARS` (40 000): rows while the text stays within it, then `[truncated: <k> markers, <j> mentions not shown]`.

## Guarantees

- **Read-only**: only the data directory is written (the md and code index refresh); nothing under the root.
- **Deterministic**: one state, byte-identical stdout, stderr, JSON whatever the absolute root, cwd, creation order, LF or CRLF.
- `spec check` reads no code and has no code finding; the hook's triggers are unchanged.
- The CLI depends on `specengine-code`, hence tree-sitter (`docs/canon/stack.md`).

## Known limits

- `lock accept` and `verify` parse the whole tree per call (debug: 457 `.rs` files in ~5.5 s); `refs` re-derives only the files whose cache key moved, but lists and hashes every one (`symbol-index.md` "Known limits").
- An unreadable root manifest renders units such as `.::bin:.::main`: deterministic, kept.
- Untested: non-UTF-8 names, readdir order (the test host's file system refuses the one, sorts the other).
