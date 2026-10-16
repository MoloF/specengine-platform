---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-store, crates/specengine-code]
owner: owner
reviewed: 2026-10-16
---

# Symbol reads: lookup tiers, `spec symbols`, `spec unmapped`

The reads over the code index (`symbol-index.md`: tables, refresh, `refs`): which item sits at a name or `PATH:LINE` and what binds it, what binds a node, where code has no spec (05 s5.1). Each command refreshes the md index, then the code index, as `refs` (`refresh_code`; a walk gap exits 2), and writes only the data directory. Two roots, cwds, creation orders, LF or CRLF copies give the same bytes; no order follows rowid or hash order. MCP mirrors them (`code-tools.md`, 6b-1); 7b's `--symbol` reuses the lookup and the containment (`impact.md`). Built by `docs/features/symbol-reads.md`.

## Crates and seams

- **Store** `symbols.rs`: `SqliteIndex::lookup_symbols(query, name) -> Vec<SymbolHit {tier, symbol}>`, `file_symbols(path)` (one file's `StoredSymbol`s by `ord`); `SymbolTier {Exact, Suffix, Name}` (`ALL`, `as_str`); `LOOKUP_SQL: [&str; SymbolTier::ALL.len()]`, one statement per tier. `code.rs`: `code_snapshot(SnapshotSymbols::{Lookup {query, name}, File(path), Bound, All}) -> Option<CodeSnapshot {refs, hits, symbols}>` (`None`: no code indexed), one deferred transaction: every stored file, marker and mention, then the hits, one file's symbols, each marker-bound symbol once or every symbol, by (path, ord); `code_snapshot_with_roles(symbols, kinds)` adds `roles`, the role rows on them (`code-roles.md` "Links"). It compares strings only.
- **Code**: `query_name` (`naming.rs`), the one reader of `::` in a query. `ALL` on `Relation`, `LevelError`, `Unanchored`, `ErrorCategory`, `Ambiguity` feed the CLI's stored-word lists (`refs.rs` `STORED_WORDS`): a new variant is never read as unknown.
- **CLI** `symbols.rs`, `unmapped.rs`, exported for callers without clap (6b-1): `symbols(&Env, &Globals, &SymbolsRequest {form}) -> SymbolsOutcome`, `form` a `SymbolsForm::{Query {terms, limit}, Node(REF), File(PATH), At("PATH:LINE")}` (the one `PATH:LINE` reader is here); `unmapped(.., &UnmappedRequest {path}) -> UnmappedOutcome`; `SymbolTarget`, `SymbolBinding`, `SymbolRole`, `UnmappedRow` (`per_mille()`), `UnmappedItem`; `SYMBOLS_LIMIT_{MIN,MAX,DEFAULT}` 1, 200, 20, `symbols_limit_error(limit)` (the `--limit` line, MCP's too); `Outcome::{Symbols, Unmapped}` (exit, stderr, text, JSON). Each outcome serializes to its `--json` document.
- **For 7b**, `pub(crate)`: `unmapped::reached(items, &Seeds)`, the one containment function; `symbols::place`, where a stored binding sits.
- **Order of work**, as `refs`: the form, the config (`[code]`), `--node`'s REF, the md index, `refresh_code`, `require_complete`, the stored rows as one snapshot, roles in when `[code] frameworks` lists one (QUERY: `Lookup`; `--file`, `--at`: `File`; `--node`: `Bound`, its items found by (path, ord); `unmapped`: `All`), then PATH; each stored ID judged once (`Corpus::judge`).
- **Genre** (`architecture.md#universal`): no fixture prefix, kind, `::` split, stack word, Cargo, `tests` or `src` literal in the store, core or these modules (`code_refs_walk.rs` `NEW_MODULES`).

## Lookup

The query is the positionals joined by a space. Each tier is one indexed statement starting at its index, then the symbol and its file by key:

| Tier | Matches | Index |
|---|---|---|
| `exact` | `symbols.qpath` = query | `symbols_by_qpath` |
| `suffix` | a `symbol_suffixes.suffix` = query | `symbol_suffixes_by_suffix` |
| `name` | `symbols.name` = `query_name(query)` | `symbols_by_name` |

Each symbol once, at its best tier; order: tier (`exact`, `suffix`, `name`), qpath bytes, path bytes, start line, `ord`. A bare name hits `suffix` at depth 1; `name` alone catches only the twins whose discriminator differs. Shells read an unquoted `#2` as a comment: quote it.

`query_name`: the text after the last `::` at bracket depth 0 (`<>`, `()`, `[]`); a segment starting `impl ` runs to the end; the first ` #` at depth 0 drops the rest. The `>` of `->` closes nothing; an unmatched closer counts as none. `crates/x::twins::impl Sheet #2` gives `impl Sheet`, `run #cfg(feature = "a::b")` gives `run`.

## spec symbols

`spec symbols QUERY… [--limit N] | --node REF | --file PATH | --at PATH:LINE`: exactly one form; `--limit` 1..=200, default 20, QUERY only (clap `conflicts_with_all` the other forms).

- **QUERY**: the lookup's hits, cut at the limit.
- **`--node REF`** (`show`'s forms; `@rev` ignored with a `note:`; several holders: `refs`' `warning:`): the items, modules and RON paths holding a stored marker that `refs` lists under the REF's nodes, so a target whose only such marker is a look-alike is listed, its `id_state` shown (`unmapped` never counts it). Each target shows every direct marker, whatever it names. Order: path, line (a module first), qpath.
- **`--file PATH`**: its items by `ord`. **`--at PATH:LINE`**: the items whose lines hold LINE, innermost first (line span, byte span, the later `ord`). PATH is a walked code file's stored path as given; a RON file or a LINE past the end: zero rows.

```
crates/tally::counter::impl Tally #2 | impl_item | crates/tally/src/counter.rs:40-45 | layer a | mentions 0 | exact
  sig impl Tally
  implements | 45:6 | LAW-BOUND@3a | resolved | bad_rev | default | LAW-BOUND
symbols: 1 of 1; code files <f> (.rs <r>, .ron <o>), skipped <s>
```

- Target: `<qpath> | <item_kind> | <path>:<start>-<end> | layer <l> | mentions <n>[ | ambiguous <reason>][ | <tier>]` (tier for QUERY only), then `  sig <signature>` (`-` when NULL); a module `<qpath> | module | <path>`; a RON path `<path>#<field path> | ron | <path>:<line>`, its first marker's line, no `sig`. `mentions`: those bound to the target itself, not by containment.
- Binding, by line: `  <relation> | <line>:<column> | <token> | <id_state>[ <fix>] | <rev_state> | <levels> | <node or ->[ | <note>]`, `levels` and `note` as `refs` prints them (`spec-cli-refs.md` "Rows"); `node` the holders' IDs once each, `, `-joined (a look-alike: its fix's node); `rev_state` the holders' common state, else `rev_unknown`. Fields go through `one_line`.
- Role, after `sig`, before the bindings, one per row on an item (`code-roles.md`): its `symbol_ord`'s, or `linked` to it, or `ambiguous` on it (never `unlinked`, never on nothing): `  role <word> | <layer> | <path>:<line> | <source>[ <form>][ <origin>] | <group or -> | <state>[ <detail>]` (`  role system | a | crates/farm/src/fields.rs:26 | call | Update | ambiguous 3`). `<form>` unless NULL or `frameworks::PATH_FORM`; `<origin>` unless `CODE_ORIGIN`; `<state>` `linked`, `ambiguous`, `certain`, `uncertain`; `<detail>` the candidates' count when `ambiguous`, else the category; `<path>:<line>` the row's own. Order: layer (`b` first), word, path bytes, line, source, `ord`. Fields through `one_line`.
- Summary: `<shown>` after the cap; `<total>` every target the form names, the limit's cut included; the code counts are the whole walk's.
- JSON, every key present: `{form, query, node, path, line, reason, notes, symbols: [{target, qpath, name, discriminator, item_kind, path, start_line, end_line, signature, ambiguity, layer, tier, mentions, bindings: [{relation, line, column, token, id, rev, id_state, fix, rev_state, levels, levels_state, note, node}], roles: [{role, layer, path, line, source, form, origin, group, state, detail, candidates}]}], total, omitted: {symbols}, code}`. `form` `query` | `node` | `file` | `at`; `line` a number for `--at`, else null; `target` `item` | `module` | `ron`, the fields its text omits null; `tier` null outside QUERY; `roles` null on every target without `frameworks` (absent or `[]`), else a list, `[]` on a module, a RON path or an item with none; a role's `form`, `origin`, `path` as stored, `detail` its category, `candidates` the count when `ambiguous`, else null; `omitted` counts the cap's cut only.
- Exits: 0 answered, zero rows too. 1 `--node`'s REF names nothing: stdout empty, `spec: <reason>`; JSON keeps the document, `reason` set, `total` 0, `symbols` `[]`, `code` and `omitted` null; never a `CliError`. 2, stdout empty: usage, discovery, config, no `[code]` (`code-bindings.md` "The `[code]` table"), a refused REF (a look-alike: its Latin fix), PATH not a walked code file, LINE 0 or not a number, a limit out of bounds, a walk gap, a stored word the build does not know; from the library, a QUERY without terms.
- Cap `OUTPUT_CAP_CHARS`: whole targets, role lines in, the first always, then `[truncated: <k> symbols not shown]`; the JSON holds the printed ones.

## spec unmapped

**Units**: every stored item; each RON file is one unit. **Mapped**: a marker binds the item, an enclosing item or inline mod (the `parent_ord` chain, never across files) or its file module, with an ID `resolved`, `alias` or `ambiguous` (each holder: `IdState::names_nodes`), any relation. Dangling, look-alike, queue and other-project IDs map nothing at any depth. `unanchored` and `cannot_verify` markers sit on no stored item and map nothing, so a broken item with a valid marker is unmapped, and Health's row 5 finds its node unbound (`architecture-code.md#health`). A RON file is mapped by an anchored `ron` or `ron_ambiguous` marker with such an ID. Containment is structural, never by qpath prefix. **`mentioned`**: the units a mention reaches by the same containment, whatever its ID state (a `file` mention: all its file's); it never maps.

`spec unmapped [PATH]`: PATH root-relative and clean, one trailing `/` dropped: a root, a walked code file under one, or a directory under a root holding one, read from the index, never the disk: a stored code file is a file, else the directory rules apply; else exit 2 (an empty directory, one above a root, a skipped or unread file: ``spec: `<PATH>` is no `[code]` root, nor a directory or code file the walk reads under one``). None: a row per root; a root that is an unread file is a file row, `units 0` and `-`.

```
crates/tally/src/ | units 14 | mapped 6 | 42.9 % | mentioned 3
crates/tally/tests/ | units 2 | mapped 1 | 50.0 % | mentioned 0
crates/tally (total) | units 16 | mapped 7 | 43.8 % | mentioned 3
unmapped: code files <f> (.rs <r>, .ron <o>), skipped <s>
```

- Rows: each direct child directory (summed, `/`-ended) and code file, by the label's bytes (`split.rs` before `split/`), then the total, always printed: `(total)`, or `<PATH> (total)` for a directory PATH. A file PATH: its row alone, then `  <qpath> | <item_kind> | <line>` per unmapped item by `ord`. A non-UTF-8 file is no unit and no row.
- Percent: mapped per mille of units, half up in integers (`(2000 m + u) / 2u`), printed `<p>.<d> %`; `-` for no unit. A directory sums units and mapped, never averages percents.
- JSON `{path, rows: [{path, dir, units, mapped, percent, mentioned}], total, unmapped: [{qpath, item_kind, path, line}], notes, omitted: {rows, unmapped}, code}`: `path` without a trailing `/`; `dir` a boolean; `percent` a number, null for `-`; `total` a row (`path` PATH or null, `dir` true; a file PATH: its row).
- Exits and cap as `symbols`, no exit 1; the tails `[truncated: <k> rows not shown]`, `[truncated: <k> unmapped items not shown]`.

## Known limits

- The gap and `skipped` come from the call's own refresh; its snapshot may hold another process's newer refresh (one committed state, never a mix).
- `query_name`: an unclosed opener in a package directory name (`a(b`) makes the rest of the query one segment.
