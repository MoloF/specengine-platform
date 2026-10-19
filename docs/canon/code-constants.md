---
class: canon
tier: 2
scope: [crates/specengine-eval, crates/specengine-code]
owner: owner
reviewed: 2026-10-19
---

# Code constants: string constants carrying record codes

`specengine-eval constants` (12a, `docs/features/constant-census.md`) reports which free `const` / `static` items of a project's code hold one of its records' codes as one string literal. It is read-only and deterministic: counts go to stdout, rows go under `--out`. It binds, locks, marks and writes nothing (ADR-0016), and a string value never yields a mention. Flags, labels, the `--out` guard, `--timeout` and exits are as `census` (eval README "CLI contract"): `--config` is the census config, there is no `--scheme`; the fixtures are `fixtures/code-consts/{one,two}`, `one` by default. Neither the reader nor the measurement holds a kind (`check`), prefix, template or code shape (`architecture.md#universal`); Rust literal forms are language support.

## Reader

`specengine_code::const_value(&Tree, &str, &ItemRecord) -> Option<ConstValue>`; `ConstValue::{Valued(String), NotValued(NotValued)}`; `NotValued::{Broken, Macro, ByteString, Escaped, MultiLine, Other}`, `ALL` in the table's order below, `as_str()` its names. All are re-exported at the crate root.

- `None` for a kind other than `const_item` / `static_item`, or when no node of that kind spans exactly `item.byte_range` (an item of another tree); the measurement then has no row.
- It finds the node by `descendant_for_byte_range`, reads it, its `value` field and the literal's bytes, never below: linear, no recursion (10 000 nested parentheses read `other`), no I/O, no hash, nothing stored, so `CODE_INDEX_FORMAT` stays 4.

## Value

The text between the quotes as written, never unescaped, when the whole initializer is one plain `"…"` or raw `r#*"…"#*` literal (raw: after `r`, N `#` and `"`, up to `"` and N `#`; a `\` kept). Otherwise the first category that applies:

| Category | When |
|---|---|
| `broken` | `item.has_error` (attached attributes included) or an error in the node |
| `macro` | a macro invocation or any call: `concat!`, `include_str!`, `"…".len()` |
| `byte_string` | a char or byte char; a literal starting `b` or `c` (`b"…"`, `br"…"`, `c"…"`, `cr"…"`) |
| `escaped` | a plain literal holding `\`, a line continuation included |
| `multi_line` | a literal holding LF or CR |
| `other` | no initializer, or any other expression: a number (negative too), a path, `&[..]`, `&"…"`, `(…)`, `{ "…" }`, `x?`, a cast, a tuple |

Items are `bind_tree`'s `analysis.items`, unhashed (`code-bindings.md` "Rust binding": file level and inline `mod`s), one parse per file. Associated consts, consts in a function body and enum variants are not items and are never read.

## Files

- Each census `[code] roots` entry, corpus-relative. A directory is listed by `harness::rust_files`: sorted; hidden directories, `target` and `target.*` (not `targets`) and symlinks below the root skipped; an unreadable subdirectory gets a stderr note and is skipped. A configured root is entered whatever its name, a symlinked one followed.
- A `.rs` file root is kept as is (unreadable: `skipped` `unreadable`); any other file root is neither listed nor missing.
- A path any `[code] exclude` glob matches is dropped (the corpus-relative `/` path, as `CodeConfig::keeps`); `extensions` and `strip` are unused; `.rs` only, `.ron` never. Each path is listed and read once, whatever the roots' order or overlap.
- `roots_missing`: the root's metadata read fails, or its walk does (its own `read_dir`, or an entry error anywhere below); a stderr note names it, and a root configured twice counts once.
- A file not UTF-8, unreadable or given no tree is `skipped`.
- This is not the scheme's walk (`code-bindings.md` "The walk"): `files` may differ from `spec index`'s on one tree.

## Records

`import::run(corpus, config)`: the `Role::Definition` entries of `Import.records`, by `id` and by each `[ids] legacy` alias, each definition keyed `(path, line, id)`. `Import.records` holds every definition, a duplicated ID's second one included, and the references; `Import.duplicates` entries of a named ID join the set and are already in it. A reference never adds a record: a value naming an ID that is only referenced reads `unmatched`, `records: []`. Hyphenless codes: the `[ids] hyphenless` `Pattern.regex`es.

## Match

Whole value, byte for byte (no prefix, substring, case fold or trim). Each valued item takes the first that applies:

1. `two_records`: two or more definitions carry it, by `id` or alias;
2. `two_items`: it would read `id`, `alias` or `hyphenless` and another valued item holds it (a cfg twin, another file); holders are counted over all valued rows before any row is classified, so order never decides;
3. `id`: one record's `id`; 4. `alias`: one record's alias;
5. `hyphenless`: a pattern's leftmost `find` spans the whole value (import's `is_hyphenless` restated, its module private); counted only, no record (ADR-0009);
6. `unmatched`.

Two items sharing a value that names two records read `two_records`; two sharing an unmatched value stay `unmatched`; `two_items` is never 1. `records`: the sorted distinct IDs the value names (an alias: its record's `id`), on `two_items` too; `[]` for `hyphenless` and `unmatched`. The value as written is the `{id}` template; `{prefix}{body}` and `{body}` are 12b's, so their candidates read `unmatched`.

## Output

stdout is the harness envelope `{measurement: "constants", label, versions, wall_ms, result}`; on overrun `result` is `"timeout"`. `result` has 19 leaves, all present, zeros too, keys alphabetical (a serde_json map): `ambiguous.{two_items, two_records}`, `constants_ms`, `files`, `hyphenless`, `items`, `marked`, `matched.{alias, id}`, `not_valued.{broken, byte_string, escaped, macro, multi_line, other}`, `roots_missing`, `skipped`, `unmatched`, `valued`. Counts only: never a path, name, ID or value.

- `files`: listed, skipped ones included; `skipped`: files only; `marked`: valued items with a bound marker.
- `constants_ms`: from the root listing (after the import; the grammar load included) to the last row matched, the detail write excluded; outside every determinism claim.
- Invariants: `items` = `valued` + Σ `not_valued`; `valued` = Σ `matched` + `hyphenless` + `unmatched` + Σ `ambiguous`; `marked` ≤ `valued`; `skipped` ≤ `files`.

Detail, `--out/constants/<label>/`, two files:

- `constants.json`: a row per item by path bytes, line, start byte; keys in order `path` (corpus-relative, `/`), `line`, `name`, `mod_path`, `cfg` (`ItemRecord`'s; cfg twins differ by `cfg`; no qpath), `item_kind`, `value`, `not_valued`, `match`, `records`, `marked`. A not-valued row has `value` and `match` null.
- `marked`, on every row: each `Marker.id` `markers_in` finds in the comments bound `Binding::Item` to the item, any relation, verbatim, in source order. An ID-less marker counts, as `""`. A broken item's markers bind `cannot_verify`, so its row shows `[]`.
- `skipped.json`: `{path, cause}` by path bytes, cause `not_utf8`, `unreadable`, `unparsed` or `root_missing` (the root as configured, `/`-joined, `.` dropped).
- `result` (but `constants_ms`) and both files are byte-identical whatever the absolute root, cwd, creation order, LF or CRLF; nothing under the corpus is written.

## Refusal and known limits

- An empty `[code] roots`: `constants: refused: <config>: [code] roots is empty`, exit 2, before `--out` is created.
- One walk error anywhere below a root (a directory entry or its file type unreadable) drops the whole root as `roots_missing`: the harness walk's rule, shared with other measurements.
- A file root not ending `.rs` is silent: no row, no count, no stderr note.
- `unparsed` is reachable only when the parser is cancelled; `name` is `null` only if `ItemRecord.name` is absent, which a parsed `const` / `static` never is.
