---
class: canon
tier: 2
scope: [crates/specengine-code, crates/specengine-store, crates/specengine-cli, crates/specengine-core, crates/specengine-eval]
owner: owner
reviewed: 2026-10-16
---

# Code roles: framework roles of Rust items, `[code] frameworks`

Which items hold a framework role (Bevy 0.19) and in which group a system is registered: per-file rows of the code index (`symbol-index.md`), behind an opt-in. Layer A: as written, never resolved (layer C), re-derived with their file; links per read, never stored. A role never binds, maps a unit, enters `spec.lock`, a verify row, a check finding or a node kind (ADR-0020, ADR-0031), never changes an exit (ADR-0012). Without `frameworks` the detector never runs and every output is as before but `code.roles: null`. Genre (ADR-0008): words, sources, categories and item kinds are `specengine-code` `frameworks` data; core, store and CLI hold none and split no `::` (CLI `code_refs_walk.rs` reads the banned words from the data); a second framework or a version gate is data, never a key or a branch. Built by `docs/features/bevy-roles.md`; per-item reads: 10a-2 `bevy-roles-reads.md`.

## `[code] frameworks`

```toml
[code]
roots      = ["crates"]
frameworks = ["bevy"]          # optional; absent or [] = off
```

- **Shape** (core `code_toml.rs`, `CodeConfig.frameworks`, `framework_line(name)`): names `[a-z][a-z0-9_-]{0,31}`, repeats dropped, byte-sorted. Exit 2, `<config>:<line>: …`, at the value's line: `` `[code] frameworks` is a <type>, not a list of names ``, `` `[code] frameworks` holds a <type>: it lists names ``; at the element's: `` `[code] frameworks`: `<value>` is not a name: a lowercase letter, then at most 31 of a-z, 0-9, `_`, `-` ``. The core names no framework: `bevy` is an unknown key (`code-bindings.md` "The `[code]` table").
- **Values** (public CLI `known_frameworks(code, config_label)` against `specengine_code::frameworks::KNOWN`, `["bevy"]`): the first unknown name in byte order, at its element's line, ``<config>:<line>: `[code] frameworks`: `<name>` is not known: known `bevy` `` (`` `, ` ``-joined), exit 2, no code row touched. `refresh_code(index, root, code, config_label, scheme, full)` checks first (`refs`, `symbols`, `unmapped`, their MCP twins); `spec index` before it, as a bad `[code]` (md index printed, `code` `null`); eval `code-index` at setup. `verify`, `lock`, `spec check` never read the values.
- **Stamp**: `code_fp`'s JSON is `{format, levels, roots, exclude, frameworks}`, `frameworks` always present, sorted, `[]` when absent (one stamp); a change re-derives every code file.

## Role facts

With a framework listed, CLI `Analyzer::rust` runs `frameworks::role_facts` on the `.rs` file's one parse (Bevy: `bevy::detect`); RON, unreadable and non-UTF-8 files hold none.

| Source | Read | Word | Row |
|---|---|---|---|
| `derive` | each `#[derive(..)]` element of a struct, enum or union whose last segment is `Component`, `Resource`, `Message`, `Event`, `EntityEvent`, `Reflect` (lists, qualified paths, several attributes) | `component`, `resource`, `message`, `event`, `entity_event`, `reflect` | on the item, `certain` |
| `derive` | the same inside `#[cfg_attr(<p>, derive(..))]`, nested too | as above | on the item, `uncertain` `cfg_attr` |
| `impl` | `impl Component for T`, `impl Plugin for T` (trait by last segment) | `component`, `plugin` | subject `T` |
| `fn` | a `fn` whose only parameter is `&mut App` (`PluginKind::FnApp`) | `plugin` | on the fn, `certain` |
| `call` | `.add_systems(G, ..)` leaves and each `pipe` argument (its leaf's group; under `add_observer` a `system` with none); `.add_observer(x)`; `.add_plugins(..)` leaves (builder chains `.set(..)`, `.build()…` peeled); `.add_message::<T>()` | `system` (group `G`), `observer`, `plugin`, `message` | a path or factory: subject; a closure: on its enclosing fn, `certain` |
| `call` | what could not be read (below) | the call's word | on nothing, `uncertain` |

Item kinds a subject may name (`frameworks::item_kinds`): `system`, `observer` -> `function_item`; `plugin` -> `function_item`, `struct_item`, `enum_item`; the other six -> `struct_item`, `enum_item`, `union_item`, `type_item`.

- **Subject** (`bevy::Subject`): segments joined by `::`, generic arguments dropped (`tick::<T>` -> `tick`), each leading `crate`, `self`, `super` another segment follows dropped (no qpath holds them); a segment that is no plain name (`<T as Tr>`) kept as written, whitespace runs one space; ≤ 128 B + `…`. `subject_name`: the last segment, never cut. Macro tokens (`macro_rules!` bodies, macro arguments): the name only; a metavariable is `uncertain` `metavariable`. `add_plugins`: `P`, `a::P`, `P { .. }`, `P(..)` -> that path, form `path`; a lowercase callee whose qualifier's last segment starts uppercase (`P::default()`, `a::P::new(..)`) -> the qualifier, `path`; any other lowercase callee (`make_plugin()`, `a::make_plugin()`; a qualifier that is no path: `<T as Tr>::new()` -> `<T as Tr>::new`) -> the whole callee, `factory`; a turbofish is looked through (`P::<T>::new()`, `P::new::<T>()` -> `P`, `path`). Macro tokens: the qualifier's last token if uppercase (`path`), else the callee's name (`factory`; `a::make_plugin()` -> `make_plugin`). The shipped `PluginUse.name` is unchanged and may differ (`a`).
- **Order**: a file's rows by each fact's first byte (`ord`), stable: one chain on one line keeps its written order, a `pipe` argument follows its leaf.
- **`line`**: a derive's element (its last segment); a code impl's `impl`; a token impl's trait token; a call's leaf, `pipe` argument or `add_message` method name.
- **`form`**: `path`, `closure`, `factory` on call rows (`add_message`'s type: `path`); NULL on derive, impl, fn and uncertain rows. **`origin`**: `code`, `macro_rules`, `macro_call`. **`group_text`**: a `system` row's group, whitespace dropped, ≤ 128 B + `…`; else NULL.
- **`symbol_ord`**: a derive or fn row's item whose node range equals its own (none inside a fn body or macro tokens); a closure row's (a `pipe` closure too) innermost enclosing `function_item` by byte range, any origin; NULL on rows with a subject, which are all `certain`.
- **Uncertain**, `category` from the data (`cfg_attr` and the seven `UncertainCategory` words): the shipped `Uncertain` rows, under their method's word; an unreadable `add_message` (`parse_error`; `arguments`: no turbofish, not one type, an argument, an unclosed token turbofish; `expression`: a type that is no path, a `<<` qualified path; `metavariable`); a plugin use with no name; a leaf or `pipe` argument that is no path (`expression`); a derive attribute with a parse error, read by name only: its identifiers in source order (`ERROR` subtrees in, `MISSING` out); after a leading `derive`, or the first `derive` after a leading `cfg_attr`, each one naming a role is a row on the item, `line` at the name, `uncertain` `parse_error` (over `cfg_attr`), none if it names no role; a `cfg_attr` derive. None of the new ones enters `BevyAnalysis.uncertain`, `add_message` never counts in `CallSites`; eval `bevy-detector` prints the shipped fields only.
- **Not read**: a derive in a macro body (its item is no symbol); a derive whose parse error leaves no struct, enum or union item; a token `add_message` not followed by `(`; `#[require(..)]`, `#[reflect(..)]`, sets, conditions, ordering. By last segment only: a project's own `Component` derive or `add_message` method reads as Bevy's; `Resource` is not also `component`. Bevy 0.19 has no `add_event`.

## Table

| Table | Key | Columns |
|---|---|---|
| `code_roles` | `(file, ord)`; index `code_roles_by_subject_name` | `word`, `source`, `form`, `origin`, `group_text`, `symbol_ord`, `subject`, `subject_name`, `certainty` (`certain`, `uncertain`), `category`, `line` |

`STRICT`, cascading with its file, a pure function of (bytes, path, package table, `code_fp`), so the dump equals a rebuild. Store: `CodeFileRows.framework_roles: Vec<CodeRoleRow>` (`role` is a file's package role), strings only. `FORMAT_TABLES` holds it before `code_files`; the dump maps `(path, ord)`, after `code_mentions`. Formats: `INDEX_FORMAT` 10, `CODE_INDEX_FORMAT` 3 (`symbol-index.md` "Analyzer stamp").

## Links

Per read, never stored: a renamed or removed item shows on the next read, the row's own file not re-derived.

- **Store** `SqliteIndex::code_roles(kinds) -> Option<Vec<StoredRole>>` (`None`: no code indexed; `roles_in` inside a caller's transaction), one deferred transaction: every row by `(path, ord)` with its file's `code_files.package`; per distinct (`subject`, `subject_name`), once, its candidates (`RoleCandidate {tier, package, symbol}`): symbols named `subject_name` (`symbols_by_name`) whose `item_kind` is among `kinds[word]` (none for a word not in the map), tier `exact` (`qpath` = subject), `suffix` (a stored suffix = subject), else `name`; a cfg twin meets a tier through `subject ‖ ' #' ‖ discriminator` (as `TWIN_SQL`, never a split); ordered by tier, qpath, path, start line, `ord`. Index searches only; no word in the store.
- **CLI** `roles::state`: the candidates at the best tier; of those, the row's own package's if any (an `Option` comparison: files outside every package match each other), else all; one: `linked`; several: `ambiguous` on each, never a pick (`code-identity.md`); none: `unlinked`, counted only (`DefaultPlugins`, a dependency's system). A row without a subject is `certain` or `uncertain` (its `certainty`) on its `symbol_ord`, or on nothing.

## spec index

After `code <slug>:`, before `db`, with `frameworks` set:

```
roles <slug>: <word> <n>, …; subjects <n> (linked <n>, ambiguous <n>, unlinked <n>), uncertain <n>
```

- `<word> <n>`: every word of the listed frameworks, byte order, zeros too; n distinct symbols `(path, ord)` holding the word: a `certain` row's `symbol_ord`, a `linked` row's candidate, each `ambiguous` row's candidates. A closure registration makes its enclosing fn a holder (an observer closure in `build` counts `build` under `observer`); a generic system registered for three types counts once.
- `subjects`: rows with a subject, by state. `uncertain`: rows `uncertain`. Over the stored rows, a gap too; `fixtures/code-bevy/expected.json` pins a line.
- JSON `code` (CLI `CodeJson`): `CodeReport` flattened plus `roles` `{words: {<word>: n}, subjects: {total, linked, ambiguous, unlinked}, uncertain}`; without `frameworks` no line, `roles` `null`; without `[code]` `code` `null`.
- Public CLI `role_counts(&SqliteIndex, frameworks) -> RoleCounts {words, subjects: SubjectCounts, uncertain}` (the eval's).

## Eval

`specengine-eval code-index` keeps the scheme's `frameworks`. Under `--label pilot-a` / `pilot-b`, `SPECENGINE_CODE_FRAMEWORKS_A` / `_B` (`harness::PILOT_CODE_FRAMEWORKS`; comma-separated, trimmed, empty names dropped; set: non-empty after trim, else off) joins the table `SPECENGINE_CODE_ROOTS_A` / `_B` builds, errors naming "the code roots and frameworks variables"; set without it: exit 2, ``the code roots and frameworks variables: `<FRAMEWORKS var>` is set without `<ROOTS var>` ``; an unknown framework: exit 2. `result` gains `framework_roles` (the last refresh's `code.roles`) and `roles_ms` (one `role_counts` read after the refreshes), both `null` off. The owner's pilot run (`code_index_pilot.rs` `te7_*`): both, AC-10's figures with and without the opt-in, a warm `spec index` against `mcp-read.md` "Latency"'s 1 s.

## Known limits

- Text only: `Self::x`, a crate-name path or a `use` alias meets no suffix and links by name; so does a `<T as Tr>::f` whose types hold inner spaces (`impl_label` compacts them; 10a-2 renders it as `impl_label` does, B5).
- Each read reads every role row and one candidate lookup per distinct subject; `code_roles_by_subject_name` has no reader yet (10a-2's targeted read).
- Bevy 0.20 (`On<Add<A>>` observers): a version gate in the data.
