---
class: canon
tier: 2
scope: [crates/specengine-core, xtask]
owner: owner
reviewed: 2026-09-30
---

# spec check: index render, generators, graph warnings

Increment 2 part 1 of `spec check`. Engine, config, debt, verdict and output: `docs/canon/spec-check.md`; this document adds §11.5–6 of `docs/canon/documentation-system.md` over a generator registry, and three warnings over the parse the check already has: seven of the 29 `CHECK_CODES`. Pure and read-only like the rest: the render stays in memory, no generator is run (ADR-0013), nothing is written. Errors where the convention says "fail" (§11.5–6), warnings where recognition is heuristic or the finding is a content discrepancy (owner, Q-A; `#control`).

## API (`specengine_core::check`)

- `render_index(&CheckInput, index_path: &str, &Generator) -> String`: pure, blind to input order. `walk_gap(&CheckInput, &Paths) -> Option<WalkGap {Unreadable, MissingRoot, UnlistedDir}>`: the first stop condition of §11.5 by path, shared with `spec export index`.
- `CheckConfig.generators: Option<Vec<Generator {command, writes, index, gate?, line}>>` (`None`: no table, the rules are off); `CheckConfig::index_generator()` → the `index = true` entry; `Generator::gate()` → `gate`, else `DEFAULT_GATE` = `spec check`.
- Module `check::resolve`, public: `Resolver`, `Resolution`, scope-aware and taking the citing file (`docs/canon/spec-check-links.md`, "API"). The store keeps `dst` as written, so `spec refs` and `get_impact` reuse this resolver rather than resolving again.
- Normal dependency: `petgraph =0.8.3`, default features off (`Graph`, `tarjan_scc`; owner, Q-C).

## Index render (§9)

```
---
class: generated
generator: <command>
source: front-matter of the repository's documents
---

# Documentation index

<!-- Built by `<command>`. Manual edits are overwritten on rebuild, and `<gate>` rejects them. -->

Reading protocol (§9): this index, then at most three documents. Needing a third step means the index is wrong: fix it rather than reading further.
```

Then each non-empty section as `\n## <name>\n\n` plus one `\n`-ended line per document, paths in byte order: `Canon`, `Decisions`, `Specs` (live documents of the class), `No class — fix` (live, no readable class or one outside the four, failed front-matter included), `Archive — Tier 3, by id only`. The index and every other generated document are not listed.

Line `- [<label>](<link>) <title> · <scope> · <status>`: label = a decision's ID, else the path; link relative to the directory of the configured index (`a/b/index.md` → `../../CLAUDE.md`); title = `title:`, else the first H1's inline text, else `?`; scope items joined by `, `; status = canon `tier N` (`tier ?`), else `status:`, else `?`.

**Tier 3** is decided once, by status, never by folder, with `xtask`'s rule: a spec `shipped` or `abandoned`, a decision with a `status:` other than `accepted` (no `status:`: live). One public predicate, `check::is_tier3_file(&ParsedFile)` (over `is_tier3(&Fields)`; false when the front-matter failed or the file is not UTF-8), serves this render, the store's `files.tier3` and `spec search`'s archive filter; `is_live` (neither `class: generated` nor Tier 3) serves the graph rules.

Accepted divergences from `xtask` (none occurs here; avoid them until increment 3 moves `xtask` onto this renderer): an H1 with inline markup (`xtask` keeps the raw text, the core the inline text: H1s here stay plain, no code span, link or emphasis) or a setext H1 (`xtask` sees none); `title:` on a non-decision (`xtask` takes the H1); a decision without `title:` (`xtask` prints `?`, the core the H1); YAML escapes in a quoted value (`xtask`'s `unquote` ignores them); front-matter failing strict YAML (core: `No class — fix`); a key of the wrong type (rendered absent).

## §11.5: index drift

WHEN the registry has an `index = true` entry, the render SHALL equal the walked bytes of `[paths] index`, byte for byte. Any difference (a trailing space, CRLF, a BOM, a stale line) → `index-drift` (error, subject `""`) on line 1 + the count of `\n` before the first differing byte; `[paths] index` not walked (absent, outside the roots, excluded) → `index-missing` (error, line 1). Both messages name `<command>`. No `index = true` entry → neither rule runs: the comparison is opt-in.

Not compared when the walk is incomplete (`walk_gap`) — a file with a `read_error`, an `UnreadableDir`, a written `MissingRoot`: each is already a cause of "cannot check", and the render would lack documents the generator sees. A skipped non-UTF-8 name (`name-skipped`) does not stop the comparison: its lossy line shows as `index-drift`, and the rename `name-skipped` asks for fixes both. Causes of "cannot check" here: index bytes not supplied (`size` > 0, empty `bytes`); an `index = true` entry without `[paths] index` (only a config built in code: the TOML reader rejects it).

## §11.6: the generator registry

This repository's parity config (`crates/specengine-store/tests/check_parity.rs`) adds:

```toml
[[generators]]                              # absent → §11.5–6 off; `generators = []` → §11.6 on
command = "cargo xtask docs index --write"  # a `generator:` value, compared byte for byte
writes  = ["docs/index.md"]                 # root-relative, the [paths] path rules
index   = true                              # optional: SpecEngine renders this output (§11.5)
gate    = "cargo xtask docs check"          # optional, index entry only; default "spec check"
```

`specengine.toml:<line>: message`, and the run cannot check: an unknown key; a wrong type, a `[generators]` table included; `command` missing, blank or repeated across entries; `writes` missing, empty, a path breaking the `[paths]` rules, or a path shared with another entry; `index = true` twice; `gate` without `index = true`, or blank; `index = true` while `[paths] index` is absent or not in `writes`; `command` or `gate` not a plain YAML scalar — a newline or control character, leading or trailing whitespace, a leading YAML indicator (19: `YAML_INDICATORS`), `: ` or ` #` inside, a trailing `:`, `-->` (it would close the header comment) — or not reading back as the same string through the crate's own front-matter reader (`read_back` parses `generator: <value>` as every parse does): null, booleans, numbers, and `.inf`, `-.inf`, `.nan`, the texts the reader gives non-finite floats. The table is outside the index fingerprint (`[ids]` only): editing it re-parses nothing.

WHEN the table is present (even empty), each `class: generated` document whose front-matter was read SHALL name a registered `command` in `generator:`, else `generator-unknown` (subject: the value, `""` if absent), and its path SHALL be in that entry's `writes`, else `generator-path` (the message names `writes`). Both errors sit on the `generator:` line, else line 1. A failed front-matter gives only its parser findings. Project generators are registered, never run: drift in their output is the project's own check (ADR-0013).

## Graph warnings

**Live source**, for all three: neither `class: generated` nor Tier 3. A document whose front-matter failed is live; with `frontmatter-unclosed` the whole file is body, so its `id:` line can give `mention-dangling`: accepted noise. Warnings never block; a baseline entry (code, path, subject) turns one into debt.

**Resolution** (`Resolver`), the same as for front-matter references: a defined ID (document or section); an `aliases:` entry; through `aliases_from`, the prefix + the body as written, never re-padded; `#Y` defined in the ID's file; where it may be defined, `slug/` and bare feature-scoped IDs: `docs/canon/spec-check-links.md`. `project:` → `Skipped`, no finding. Width is never checked on recognition: a five-digit number is a real mention.

**`mention-dangling`**: one per unresolved inline mention (`mentions`, origin `inline`) of a live source, at its line; subject as written, a wiki link's `[[…]]` included; the message as `ref-dangling`'s. Front-matter keeps `ref-dangling` (error). The lexer joins `#Y` to an ID only when `Y` is ID-shaped. Fallback, inline only: WHEN a mention of a `shape = "name"` prefix does not resolve, the check drops its last `-segment` and retries while prefix + one segment remain; the first resolving ID wins, its `#section` checked there (`MEC-STAMINA-based` → `MEC-STAMINA`). Never for front-matter or number shapes; case cannot decide (`TERM-exhausted` is a real ID).

**`depends-cycle`**: nodes are documents; an edge A → B for each `links.depends_on` item of a live A that resolves, from A, to an ID B holds (no fallback, `#section` ignored; several holders → an edge to each). `petgraph::algo::tarjan_scc`: one finding per strongly connected component of ≥ 2 documents, or of one with a self-loop. Subject: the member names (ID, else path) sorted by name then path, joined by `, `; placed on the first member's first `depends_on` item into the cycle; message "`depends_on` forms a cycle through <subject>".

**`ref-superseded`**: WHEN a live source references Y — declared (`refs`, `adrs`, `links.*`, `parent`, `working_answer`, a reference-form `canon:`) or inline (with the fallback) — and Y resolves to a document whose `status:` is `superseded-by X`, the check warns once per occurrence, subject as written: "`Y` is superseded by X". Exempt: `supersedes:` items, `links.supersedes`, the `status:` value, references from X's own files. Several holders of Y: the first superseded one in path order.

**Determinism**: render, findings and cycle subjects do not depend on input order. **Genre** (ADR-0008): no `cargo xtask`, `docs/`, `ADR` or `index.md` literal in the sources; the ADR-0022 convention text (header, H1, `source:`, protocol line, section names) and `spec check` are literal.

This repository (parity config + registry, `enforce`, no baseline): `clean`, one warning — `mention-dangling` on the `file-name` example of `docs/canon/spec-check.md` "Rules".

## Next

- Increment 2 is complete: part 2 shipped feature scopes and Markdown file links (`docs/canon/spec-check-links.md`).
- Increment 3 = CLI passes 2a.1 (shipped: `spec check`, the writer `spec export index` on this renderer), 2a.2, 2b (the root `specengine.toml`, Q-7; hook and CI switched; `xtask` retired): `docs/canon/spec-check-cli.md`. The root `[ids]` must not configure prefixes that collide with prose labels (Q-1…, AC-01…).
- Elsewhere: drift in project generators' output (ADR-0013); `@rev`, `project:`; citations of rejected decisions; `supersedes` ↔ `superseded-by` consistency; mentions in code (ADR-0016, Phase 3); fix data for `ref-superseded` (Phase 2).

## Open nits

- `Resolution::Resolved(Vec<usize>)` allocates per resolved reference; linear, left.
- `crates/specengine-eval/tests/build_graph.rs` greps `File::`, which also matches `ParsedFile::` and `CheckFile::`; the sources use closures as a workaround, `\bFile::` would be cleaner.
- The plain-scalar rule rejects a leading `-`, `?` or `:` though YAML reads `-x` as a string: conservative, accepted.
- The `NotAString` message ("a null, a boolean or a number") would be imprecise if serde-saphyr's `properties` feature were ever enabled.
