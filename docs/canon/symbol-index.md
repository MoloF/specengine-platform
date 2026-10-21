---
class: canon
tier: 2
scope: [crates/specengine-store, crates/specengine-cli, crates/specengine-code, crates/specengine-eval]
owner: owner
reviewed: 2026-10-21
---

# Symbol index: stored code rows, the refresh, `spec index`, `spec refs`

The code index stores every `[code]` file's rows beside the same worktree's spec index (the slug's DB outside the repository: ADR-0001, ADR-0003): its items with names, lines, signatures and level digests, `#[path]` declarations, markers and mentions with what each binds. **After any sequence of file, manifest, `[code]`, `[ids]` or stamp changes it equals a fresh rebuild, row for row** (canonical dumps), and `spec refs` from it prints the walk's output byte for byte (`code-bindings.md`). Only `spec index`, `refs`, `symbols`, `unmapped`, `health`, `verify --node`, `show --bindings`, `bundle`, `task show`, `impact`, their twins and `resolve` (`layer-c.md`) refresh it. `verify`'s other modes, `lock accept`, `propose drift` and a drift item's approve keep a fresh walk or a commit's blobs; `show`, `search`, `graph`, other MCP tools and daemon routes never touch code (`architecture-code.md#sync-in-reads`, `#health`). Names: `symbol-names.md`; lookup, `symbols`, `unmapped`: `symbol-reads.md`. Built by `docs/features/symbol-index.md`.

## Crates and seams

- **Store** `code.rs`: compare, apply and the cross-file pass behind `trait CodeAnalyzer {list, packages, fingerprint, analyze}` over plain rows (`CodeListing`, `ListedCodeFile`, `CodePackageRow`, `CodeFileRows`, `Code{Symbol,Decl,Marker,Mention}Row`). No code crate, no tree-sitter, no `rusqlite` type public; it compares strings and reads only the roles `ROLE_SHARED`, `ROLE_UNROOTED`.
- **CLI** `code_index.rs`, the analyzer: the walk's one listing and package lookup (`code_walk::list`, `disk_packages`); one parse per re-derived `.rs` file for `bind_tree`, `placed_name_file` (each package's `TargetIndex` built once; each item's `QPath`, `file_place` once), `item_levels`, `item_signatures`; `comment_anchors` once per `.ron` file; never an item's binding `state`.
- `SqliteIndex::update_code(&dyn CodeAnalyzer, &IdScheme, full) -> CodeReport` refreshes; `refresh_code(index, root, code, config_label, scheme, full) -> CodeRefresh {report, messages, gap(), require_complete()}`: the one named refresh (the refreshers above), lenient. `listed_code_files(root, code)`: the walk's listing.
- Reads: `stored_names(path) -> StoredNames {module, items}` (`None`: no row or no module), each `StoredName {qpath, ambiguity, discriminator}` by `ord`, from rows only (a file edited since keeps its names); `code_refs() -> CodeRefs {files, markers, mentions}` (a binding's `BoundSymbol.ord`), `code_symbols()` (`StoredSymbol` with `start_byte`, `end_byte`, `layer`; `None`: no code indexed); `lookup_symbols`, `file_symbols`, `code_snapshot` (one transaction): `symbol-reads.md`. `clear_code()` drops the worktree's rows.
- `specengine-code`: `CODE_INDEX_FORMAT`, `placed_name_file` -> `PlacedNames` (`indexed_name_file`'s names, `file_place`'s label and role, each item's `QPath`), `QPath::segments`, `QPath::suffixes(discriminator)`.

## Tables

`INDEX_FORMAT` 12: nine `STRICT` tables created with the schema in every DB (columns: store `schema.rs`), IDs and revs as written; layer C's three: `layer-c.md` "Tables". `code_files.file_id` is an `INTEGER PRIMARY KEY`; the per-file tables key on `(file, ord)`, `file` that `file_id`, and cascade; `code_worktrees.wt` references `worktrees`, `code_files` and `code_packages` reference `code_worktrees`.

| Table | Key | Columns |
|---|---|---|
| `code_worktrees` | `wt` | `code_fp`; no row: no code indexed |
| `code_packages` | `(wt, dir)` | `manifest_b3` (NULL unread), `source`, `targets` (JSON `[{kind, name, root}]`) |
| `code_files` | `file_id`; unique `(wt, path)` | `lang`, `blake3`, `size`, `read_error`, `utf8`, `package` (its directory, `""` the root), `role`, `module` (qpath), `orphan_errors`, `targets_fp`; pass: `declared_by` |
| `symbols` | `(file, ord)` | `item_kind`, `name` (bare last segment), `owner`, `qpath` (discriminator in), `discriminator`, `parent_ord`, `start_line`, `end_line`, `start_byte`, `end_byte`, `signature`, `level_state`, `level_categories`, `{sig,body,deps,skeleton,quote}_hash`, `body_bytes`, `layer`; pass: `twin_files` |
| `symbol_suffixes` | `(file, ord, depth)` | `suffix` |
| `code_decls` | `(file, ord)` | `target` (a `#[path]` module's file), `line` |
| `code_markers` | `(file, ord)` | `relation`, `token`, `id`, `rev`, `levels` (JSON; NULL invalid), `levels_state`, `note`, `line`, `"column"`, `binding`, `symbol_ord`, `anchor` |
| `code_mentions` | `(file, ord)` | `written`, `line`, `"column"`, `binding`, `symbol_ord`, `anchor` |
| `code_roles` | `(file, ord)` | `code-roles.md` "Table" |

Indexes `symbols_by_qpath`, `symbols_by_name`, `symbol_suffixes_by_suffix`, `code_decls_by_target`; `"column"` quoted everywhere. The dump maps `wt`, `file_id`, `file` to `(project, root)`, `path`, `(path, ord)`. `FORMAT_TABLES` (a list that never removes one) holds them children first, before `worktrees`: a stamp change recreates them, queue tables survive.

- **Words.** `lang` `rust` | `ron`. `role` `crate_root` | `module` | `shared` | `unrooted`; NULL for RON, unreadable and non-UTF-8 files, which have NULL `package`, `module`, `targets_fp` too (unreadable: `utf8` 1, `size` 0). `level_state` `hashed` | `cannot_verify`, `level_categories` its `ErrorCategory` names (`[]` hashed); digests `b3:` + 64 hex, NULL with `body_bytes` when `cannot_verify`; no `ast_hash`, module or RON digest. `layer` `a` (B, C annotate from their own tables). `binding` `item` | `module` | `ron` | `ron_ambiguous` | `unanchored` | `cannot_verify` | `file`; `anchor` the file module's qpath (`module` without `symbol_ord`; an inline mod's has one), the value path (`ron*`), the category (`unanchored`, `cannot_verify`), else NULL.
- **Items**, every one, bound or not, in `bind_tree`'s pre-order: start as refs' line (attributes excluded), `end_line` its end byte's line; `parent_ord` the nearest preceding item whose byte range holds it.
- **Suffixes**: depth d = the last d structured segments (package, unit, modules, owner, name + ` #<discriminator>`) joined by `::` (`QPath::suffixes`), never a split: `<T as a::Tr>` is one segment.

## Cache key and cross-file pass

- **`code_fp`**: `b3:` BLAKE3 of parts, each prefixed by its length (8 bytes, little-endian): the stamp JSON `{format: CODE_INDEX_FORMAT, levels: level_header() hex, roots, exclude, frameworks}`, then the md writer's `[ids]` fingerprint. **`targets_fp`**: framed alike over (`source`, `targets`).
- **Key**: a row is kept when its (BLAKE3, package directory, `targets_fp`) is the file's under an equal `code_fp`; never mtime. A new `Cargo.toml` between a file and its package re-parents it; a manifest edit or a new `src/bin/x.rs` re-derives the package. An unreadable file (BLAKE3 NULL) is rewritten every refresh; a file the parser gives no tree is a `read_error` row cached by its BLAKE3. `--full`, a moved `code_fp`, or an index this handle's spec update recreated re-derives all (`rederived_all`).
- **Refresh**: a snapshot read, the analyses outside any lock, then one `Immediate` transaction: `code_fp` and keys re-read, a row moved since analysed in place, `code_packages` rewritten, removed files deleted, rows applied, the pass, the counts; a crash rolls all back.
- **Pass**, over every path and undiscriminated qpath a removed or re-derived file held or holds: `declared_by`, the smallest path among the files declaring it (`code_decls_by_target`); `twin_files`, the number of other files rendering the qpath `u` (0 if none; unrooted and `#[path]` files count), read by `TWIN_SQL`: `qpath = u` with no discriminator, plus the `qpath` index range [`u #`, `u $`) filtered by `discriminator IS NOT NULL AND qpath = u ‖ ' #' ‖ discriminator`, the index search outermost (`CROSS JOIN`). Never a scan, never a split of a qpath (names hold ` #`).
- **Ambiguity** (reads), the first that applies: `path_attribute` (`declared_by` set, role not `shared`), `unrooted` (role), `duplicate` (items, `twin_files` > 0): what `naming.rs`' halves give (`symbol-names.md` "Ambiguity").

## Analyzer stamp

`CODE_INDEX_FORMAT`, 5: the binding, naming, discriminator, signature, marker, mention and role rules a row holds (`symbol-names.md`, `code-roles.md`); `\` becomes `/` in stored directories and package labels only where `MAIN_SEPARATOR` is `\`; any change to one, or a level recipe amended in place, bumps it (all re-derived); `level_header()` covers a renamed one. CLI `tests/code_format_history.txt`: `<CODE_INDEX_FORMAT> <BLAKE3 of fixtures/code-index's code-table dump, from 3 on then fixtures/code-bevy's, roots masked>`; a changed dump under a kept stamp is red. The store's `format_history.txt` lines 10-11 hash spec-a, spec-b and a stub analyzer's rows.

## spec refs and spec index

- **`spec index [--full]`**: the md index, then `[code]`. Before `db`: `code <slug>: walked 41, parsed 41, unchanged 0, removed 0, unreadable 0, skipped 0, symbols 230, markers 31, mentions 9` (+ `, rederived all`, never on a first plain index); JSON `code`, those keys + `rederived_all`. `parsed + unchanged + unreadable = walked`; `skipped`: non-UTF-8 names plus listed files whose bytes are not (a `note:` each); `symbols`, `markers`, `mentions`: the worktree's totals.
  - No `[code]`: its rows go, no line, JSON `code` `null`.
  - A bad `[code]`: the md index written and printed, then `<config>:<line>: …` (the config-error form, no `spec: `), exit 2; `--json` prints its document, `code` `null`; code rows untouched.
  - A gap: `warning: the code walk is incomplete: <gap>; reads of the code refuse until it is not`, exit 0.
- **Gap rule**: the first directory that cannot be listed (the root shown `.`) wins only if it sorts before the first stored `read_error` path, else that file. Reads exit 2, stdout empty: `spec: the code walk is incomplete: <gap>`, the walk's forms.
- **`spec refs`**: config (`[code]` or exit 2), REF and the md index, the holders `warning:`, then `refresh_code`, `require_complete`, the stored markers and mentions, each stored ID judged once (`Corpus::judge`), rows sorted on render. A stored relation, binding, level or category the build does not know exits 2 naming `spec index --full`. The walk's `collect_rust`, `collect_ron` stay as the tests' oracle.

## AC-10 measurement

`specengine-eval code-index` copies the `[code]` walk's files and each manifest up to the corpus root to `--out/code-index/<label>/corpus` and indexes it there (`index.db`). `result`: `kloc` (`.rs` newlines / 1000), `rs_files`, `ron_files`, `bytes`, `items` (stored symbols); `hash_ms` (parse, `bind_tree`, `item_levels`, `item_signatures`, no DB); `full_ms`; `noop_ms` + `noop_parsed`; `one_file_ms` + `one_file_parsed` (a `//` line appended to the largest `.rs` by bytes, smallest path on a tie; `null` without one); `db_bytes` (DB + WAL); `refs_ms` (one `code_refs()`); `twins.{cfg, ordinal}.{groups, items}` (a discriminator of digits is ordinal), `twins.cross_file` (`duplicate` items); `signatures.{items, cut}`; `roles.{files, metadata, differ}`, each Rust file's role under the layout vs `cargo metadata`'s table over the original corpus (`$SPECENGINE_CARGO`, absolute under a scratch `HOME`, else `cargo`; `differ` `null` when no package answered); `framework_roles`, `roles_ms`: `code-roles.md` "Eval".

Judged in release on the pilots: `hash_ms` × 300 / `kloc` ≤ 2 000, `one_file_ms` ≤ 200, `noop_ms` ≤ 200 (7a's gate); `full_ms` recorded. A failed figure, or 5a-2's `levels_ms_per_kloc` above 6.67 (`code-hash-eval.md`), plans 6c `code-index-speed`.

## Guarantees

- Only the data directory is written; the analyzer only reads; a no-op refresh writes nothing.
- Two roots, cwds, creation orders, LF or CRLF copies: equal rows, equal `refs` bytes; no order follows rowid.
- Nothing blocks: `index` exits 0 on a gap; only the reads needing the whole tree refuse.
- No command reads the level digests: `verify`, a read's `sync` (lines: `SelectedFile.symbols`), `lock accept`, the drift writers hash afresh.

## Known limits

- Each refresh BLAKE3s every code file; rows of moved roots are never pruned.
