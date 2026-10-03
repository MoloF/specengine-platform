---
class: canon
tier: 2
scope: [crates/specengine-code, crates/specengine-eval]
owner: owner
reviewed: 2026-10-03
---

# Layer A identity: target units, markers, RON paths

Layer A (05 §5.1) names code without a build: a heuristic `qpath` per Rust item, the marker grammar of `.rs` and `.ron` comments, a field path per RON marker. The marker is the link identity, the `qpath` only a name (ADR-0020, `docs/canon/architecture.md#code-identity`): what the heuristic cannot decide is reported, never guessed (Phase 3: `cannot_verify`); malformed input is counted, never fatal (ADR-0012). Unit sources know only Cargo's names (`src`, `bin`, `examples`, `tests`, `benches`, `build.rs`, `main.rs`, `lib.rs`, `mod.rs`, `Cargo.toml`; ADR-0008, `identity_literals.rs`). Code: `specengine-code` `qpath.rs`, `markers.rs`, `ron/`; cargo runner: `specengine-eval` `ast_hash/targets.rs`. Today read only by the eval; Phase 3 stores it.

## Target table

`ast-hash` runs, per package dir (a `Cargo.toml` dir owning a listed `.rs` file), shortest first, skipping dirs an earlier run covered (its packages, its `workspace_root`):

    ${SPECENGINE_CARGO:-cargo} metadata --format-version 1 --no-deps --offline --color never --manifest-path <dir>/Cargo.toml

An answer gives `PackageTargets { source: Metadata, targets: [Target { kind, name, root }] }`: Cargo's first kind and name, the root relative to the canonical package dir (outside it: dropped); a covered virtual root has no targets. A dir no run covers (cargo missing, a broken manifest, a nested workspace, no answer) gets `layout_targets(package, files)`, Cargo's auto-discovery (`src/lib.rs` → `lib` `<package>` with `-`→`_`, `src/main.rs` → `bin` `<package>`, `src/bin/<n>.rs`, `src/bin/<n>/main.rs` → `bin` `<n>`, alike under `examples`, `tests`, `benches`, `build.rs` → `custom-build` `build-script-build`), `<package>` = `[package] name` if the manifest parses, else the dir name. Exit 0 either way, `detail.targets_from` tells which.

- **Never builds, patches or runs a corpus**: `--no-deps` writes no lock file, `--offline` stays off the network, stdin null.
- **cwd always `/`**, root-owned, wherever the harness starts, for cargo and rustfmt (`ast_hash/fmt.rs`: `--version`, each format call): no corpus `rust-toolchain` or `.cargo/config.toml` is read via the cwd; never a world-writable dir (a `rust-toolchain.toml` planted there makes the rustup proxy run a foreign toolchain). rustfmt is picked by `RUSTUP_TOOLCHAIN` or a `SPECENGINE_RUSTFMT` wrapper, a relative path (`bin/fmt`) resolved against the harness's cwd. A corpus at `/` → no cargo call, layout for all; rustfmt not run, its rows `null`, one stderr line.
- **Budget** per call: min(60 s, the remaining `--timeout` − 1 s); nothing left → no call. An overrun is killed and reaped and, like a cargo that cannot start, ends the run's cargo calls: layout for the rest. No cargo child outlives the run.
- **stderr** captured, never on stdout; one harness stderr line per failure: cargo's first `error…` line (else first non-blank, else the exit status), the corpus root replaced by the label where it stands as a whole path (`/mnt<root>/y`, `<root>.bak` stay), cut to 200 chars.

## Units and `qpath`

A file (relative to its package dir) takes the first rule that matches; target dirs `<d>`: `src/bin`, `examples`, `tests`, `benches`:

1. root of one target → it, module `[]`; of several → `shared:<file>`;
2. under `<d>/<n>/` holding a target root at any depth → that target, modules below `<n>/`; two or more → `shared:<d>/<n>`;
3. under `<d>/<x>/` holding none → `shared:<d>/<x>`, modules below `<x>/`;
4. any other file of `<d>` → `Unrooted`;
5. under `src/` → the primary target, modules below `src/` (`a/b.rs` → `a::b`, `a/mod.rs` → `a`); no primary → `Unrooted`;
6. else `Unrooted`.

No targets at all (a virtual root, a target-less layout): rules 2–4 give `Unrooted`. A `<d>/<n>.rs` root never owns `<d>/<n>/` (rustc looks for a crate root's `mod x;` beside it, E0583): `tests/a/h.rs` beside `tests/a.rs` is `shared:tests/a`, module `h`. A non-root file is a module: `tests/common/mod.rs` → `Module(shared, [])`, role `module:`.

**Primary target**, by root: the lib (any kind but `bin`, `example`, `test`, `bench`, `custom-build`), else the bin rooted at `src/main.rs` whatever its name, else the sole bin.

**Rendering** `<package>::<unit>::<module…>::<owner>::<name>`: `<package>` the package dir relative to the corpus (`.` for the root); `<unit>` omitted for the primary target, else Cargo's `kind:name` verbatim (`bin:tool`) or `shared:<dir>`. A unit holds a `:`, a module segment never, so module paths stay suffixes (05 §5.2 lookup). Items: `<k>_item` for `function`, `struct`, `enum`, `union`, `trait`, `impl` (+ methods), `const`, `static`, `type`, `mod`; `macro_definition`. An impl is `impl Type` / `impl <Type as Trait>`, its methods `Type::method`, `<Type as Trait>::method`; inline `mod {}` extends the path; `mod x;` is not followed (layer C).

**Ambiguity**, one reason per item, the first that applies: `path_attribute` (the file is a `#[path]` target) > `unrooted` > `duplicate` (an equal rendered `qpath`: adjacent impls, `cfg` twins). Shared items: `duplicate` yes (analysed once, only real twins collide), `path_attribute` no (named by location).

## Marker grammar

`@implements|@verifies|@configures|@assumes ID[@rev] [levels] note`, any number per comment, `.rs` and `.ron` alike (canon form `docs/canon/architecture.md#markers`): `// @verifies EDGE-STAM-ZERO@2 [body] mutation: "drop the immediate Exhausted"`. In Rust a marker applies to the next item (binding: Phase 3).

- **ID**: the first non-blank after the keyword on its line, up to whitespace, `@`, `[` or `*/` (`X[sig]` = `X [sig]`; `/* @implements X*/` → `X`). Latin = ASCII letters, digits, `-_.:/` (`/` for `<slug>/ID`, ADR-0026); else `id_latin` false, counted (ADR-0009). An empty ID (the keyword ends the line or meets `[`, `*/`) is counted apart, `id_latin` true.
- **rev**: `@` and ASCII digits right after the ID (ADR-0018).
- **levels**: optional spaces/tabs, `[`…`]` on the marker's line before a `*/` and the next `@keyword`; items `path|sig|body|deps` (05 §5.2), lowercase, `,`-separated, spaces/tabs around, one trailing `,` tolerated; canonical order `path, sig, body, deps`. None → `Default` (`[sig, body]`), told apart from declared.
- **errors**, the first that applies: `unclosed` (no `]` before the line end, `*/` or the next keyword; no note), `unknown` (any other item, empty ones too: `[Sig]`, `[sig body]`, `[,]`), `duplicate`, `empty` (`[]`, `[ ]`). Never fatal: relation, ID, rev, note kept; levels `Invalid`, untrusted, never the default; a fixed category, never file text.
- **note**: text after `]` (or `ID[@rev]`), trimmed, `*/` stripped, to the next keyword; a later `[` is note text (`X see [docs]` → that note, `Default`).
- Linear: the level scan never leaves the marker's line.

## RON binding

Own lexer (05 §9). The field path is the `qpath`: `data/movement.ron#root.stamina.regen_per_second`. Segments: `root` = the file's value, `.name` a struct field, `[i]` a list element, `.i` a tuple element (tuple structs such as `Some(x)` too), `{key}` a map entry (the key's source text: string keys keep their quotes, whitespace runs collapse to one space, inside strings too); each capped at `MAX_SEGMENT_BYTES = 128` source bytes plus `…`. A *value* is an entry, an element or the root value. A comment binds by the first rule that applies; positions count, not the comment kind, except that `//` never leads:

1. **leading** — a block comment followed on its `*/` line by the start of a value binds to it: `pos: (/* @A */ 10, /* @B */ 20)` → `root.pos.0`, `root.pos.1`; `speed: /* m */ 4.5` → `root.speed`, also with `speed:` alone on the line above;
2. **trailing a value** — a comment starting on the line of a value's last token (`,` on either side; of several values closing there, the one right before it): `speed: 1, // m` → `root.speed`; `[1, 2, 3 /* m */]` → `…[2]`; `Config(..) // m` → `root`;
3. **trailing an opener** — right after `(`, `[`, `{` on its line → the container's value: `player: Player( // m` → `root.player`;
4. **own line** — no token before it on its line → the value starting at the next token; extension attributes before the root are transparent (`#![enable(..)] // m` → `root`);
5. otherwise **`unanchored`**: own line before a closer or `,`; after a map key or a `:` (a `//` under `speed:`, or a block comment whose value starts below its `*/` line); between a type name and its `(`; after trailing content.

A multi-line block comment trails by its `/*` line and leads by its `*/` line; LF and CRLF alike. Fallback: when error recovery consumes the token a block comment leads, it binds to the value it trails (`a: 1 /* m */ 2` → `root.a`). Past `MAX_DEPTH = 512` open containers the container is skipped through its closer (`nesting_too_deep`), its markers `cannot_verify`. Cost linear in tokens.

## RON ambiguity

Two siblings of one struct or map whose segments render alike (cut at the cap, whitespace collapsed, a field or key written twice) give one path to two values: every marker on either or below is `Anchor::Ambiguous { path, depth }`, the other marked or not; path text kept, not trusted, not counted anchored, never merged. Each open struct or map keeps one set of segments; colliding entries form a group linked to the enclosing entry's; decided after the walk. Linear: no pairwise compare, no path per value. `RonAnalysis.colliding_groups` counts groups, marked or not.

## Eval outputs

`ast-hash` stdout: `detail.qpath_units` (items per unit kind: `primary`, `shared`, `unrooted` always, each Cargo kind met), `detail.targets_from` `{metadata, layout}` (package dirs). `--out` `manifest.json` files: `unit` (`"primary"`, `"bin:tool"`, `"shared:tests/common"`, `null` if unrooted), `targets` (`metadata | layout`, `null` outside a package).

`ron` stdout: `markers.ambiguous` (`{grammar: null, lexer}`; `nested` counts `Path` only), `markers.id_empty`, `markers.levels` `{default, declared, invalid}`, `detail.levels_invalid` `{<category>: n}`, `detail.colliding_groups`. `markers.json` rows: `levels` (the effective list, `["sig","body"]` for the default; `null` only when invalid), `levels_state` (`default`, `declared`, a category), `ambiguous` (`lexer` keeps the path text).

## API

`specengine_code`: `TargetSource`, `Target`, `PackageTargets`, `Unit {kind: UnitKind, name}`, `FileRole::{CrateRoot(unit), Module(unit, path), Unrooted}` (unit `None` = primary), `file_role`, `TargetIndex` (prepared once per package), `layout_targets`, `QPath.unit`; `Marker.levels: Levels::{Default, Declared(Vec<Level>), Invalid(LevelError)}` (`effective()`, `state()`), `Level`, `DEFAULT_LEVELS`, `LevelError::as_str`; `Anchor::Ambiguous`, `RonAnalysis.colliding_groups`. Eval: `ast_hash::targets::tables(root, label, packages, run_end)`.

## Measured

Pilots (read-only): A 0.2 % of 11 852 items ambiguous — `duplicate` 29 in 11 groups, each within one file; `path_attribute` 0 (its 12 `#[path]` targets in shared units). B 0.0 % — 2 of 7 599, 1 group within one file. Both all `metadata`, 0 unrooted. Phase 0, without units: 16.5–32.2 %.

## Known limits

- Under a deeper root modules count below `<n>/`: `src/bin/m/cli/args.rs` → `cli::args` (rustc `args`).
- Several bins, none at `src/main.rs`, no lib → no primary (`default-run` unread).
- A module of `src/main.rs` beside a lib is named a lib module; stray `src/` files are named modules; adding a lib renames `main.rs` items.
- `#[path]` targets outside shared units stay `path_attribute`.
- The kill reaches only the direct child: a `SPECENGINE_CARGO` wrapper that does not `exec` can leave a grandchild.
- The scrub misses a root right after `/`, as in cargo's `path+file:///<root>` (stderr only).
- A relative `SPECENGINE_CARGO` path (`bin/c`) resolves against `/`: cargo cannot start, layout for all.

## Open

- A process-group kill.
- Phase 3: Rust marker binding, `symbols` and `markers` rows, `spec.lock`, per-level hashes, level meaning in RON, the impl `disambiguator`, malformed revs (`X@3a`).
