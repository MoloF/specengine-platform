---
class: canon
tier: 1
scope: [crates/specengine-core]
owner: owner
reviewed: 2026-10-05
---

# specengine-core — the spec parser and the check

The reading core: the parser (a file's bytes → `ParsedFile`) and `check` (parses → `Report`). No file access and no SpecEngine crate but `specengine-model` (types, `[ids]`, the reference grammar: its README). Output depends only on the arguments, maps serialise sorted or in source order; a broken file is reported, never fatal (ADR-0012). Pins (Q1): `pulldown-cmark =0.13.4`, `serde-saphyr =1.3.0` (`deserialize` only), `toml` for `specengine.toml`, `serde_json` for `Report::to_json`, `petgraph =0.8.3` for `depends-cycle`. Callers: store, CLI, `specengine-eval`.

## API

`parse(path: &str, bytes: &[u8], scheme: &IdScheme) -> ParsedFile`; `IdScheme::from_toml(&str)` (trait `IdSchemeToml`, also `scheme_from_toml`) reads only `[ids]`; `Paths::from_toml(&str)` (also `paths_from_toml`) only `[paths]`; `tokens_est(&str) -> u32`; `MAX_DEPTH = 32`, `MAX_ALIAS_EXPANSION = 10_000`. `check::run(&CheckInput, &IdScheme, &Paths, &CheckConfig, &Baseline, today: &str) -> Report`: `spec check`'s engine, blind to input order, with `check::worst_w`, `check::judge` (against a base): `docs/canon/spec-check.md`; `render_index{,_set}`, `walk_gap`, `is_tier3_file` (with `is_tier3`, `is_live`), `CheckConfig.generators` in `-graph.md`; `check::resolve` (`Resolver`, `resolve_detached` for `spec show`), scopes, file links in `-links.md`; `CheckConfig.rules` (`check::CheckRule`, `check/rules{,_toml}.rs`), `own_spans` (own text as spans; the store's `own_text`) in `-process.md`; `check::SpecGraph` (graph reads) in `docs/canon/spec-cli-graph.md`; `check::bundle_layers` (`BundleLayer`, `BUNDLE_LINK_TYPES`, `BundleCandidate`, `BundleLayers`), `bundle_node_from_toml` (`BundleNode`), `SpecGraph::{file, scheme}` (`spec bundle`) in `docs/canon/spec-cli-bundle.md`. Proposals (`docs/canon/proposal-queue.md`): `patch` (`locate`, `span_bytes`, `splice`, `check_structure`, `update`, `update_refusal`, `PatchCheck::introduced`), `proposal` (`PR-NNNN`, look-alikes, `prefix_clash`, `Author`, `commit_message`, `TEXT_MAX_BYTES`); `intake` (kinds, enums, caps, `DISTINCT_MAX` 64, field checks, `author_problem`, `normalized_summary`): `docs/canon/agent-intake.md`.

`ProjectConfig::from_toml` (also `project_from_toml`) reads the whole `specengine.toml` for the CLI: `{project: Project {slug?, name?, language?}, scheme, paths, paths_written, project_line?}`. Closed: `[project]` and the top level (`budgets`, `classes`, `check`, `generators`, `zones`, `gate`, `code` only name-checked); `ProjectError {line?, message}`, `at(file)`. `slug()` errs when absent; `slug_problem`: `grammar::is_slug`, ≤ `MAX_SLUG_BYTES` (64).

## `[paths]`

What the index and the check walk (the walk itself: store README). `Paths {spec, records, features, generated, archive, roots, exclude, tier0?, tier1_name?, index?, link_base?, roots_written}`: role key `k` defaults to `DEFAULT_{SPEC,RECORDS,FEATURES,GENERATED,ARCHIVE}` = `docs/<k>`; `roots` are walked directories or single `.md` files, default the role directories but `generated` (SpecEngine's own output; `roots_written` when written); `exclude` holds census globs (`*`, `**`, `?`) over root-relative file paths; `tier0`, `tier1_name`, `index`, `link_base` (file links' fallback base): the check's. Every path is root-relative with `/` (`is_clean_relative`: no leading `/`, no `..`, `.` or empty component); one trailing `/` is dropped. An unknown key, a wrong type or a bad path → `PathsError {line?, message}`, `at(file)` → `file:line: message`; other tables are ignored. Pure: existence is the walker's business. `Paths::walk_scope()` → `WalkScope`, the walk's rules without the disk (`docs/canon/spec-check-links.md` "API").

## Output

`ParsedFile = {path, bom, front_matter?, body, nodes, links, anchors, diagnostics}`; BOM + front-matter + body = the file, even when it is not UTF-8. `anchors`: heading slugs, non-ID `{#…}`, `<a id|name>` (model README). `nodes[0]` is the document (`document()`), sections follow in source order (`sections()`); absent optional fields are omitted. `diagnostics` come in line order; the codes: model README.

```json
{"path":"spec/stamina.md","bom":false,"front_matter":[0,236],"body":[236,512],"nodes":[
 {"id":"MEC-STAMINA","kind":"mechanic","title":"Stamina","summary":[247,312],"parent":{"id":"DOM-MOVEMENT","span":[81,93]},"span":[0,512],"tokens_est":131,"fields":{"tier":2},"extra":[]},
 {"id":"RULE-STAM-REGEN","kind":"rule","title":"Regeneration","level":2,"heading":[326,366],"body":[366,440],"attrs":[["rev","3"]],"rev":3,"parent":{"id":"MEC-STAMINA"},"span":[326,440],"tokens_est":29}],
 "links":[{"src":"MEC-STAMINA","type":"derived_from","origin":"frontmatter","dst":{"id":"R-12","script":"latin","span":[118,122]}}],
 "anchors":[{"name":"stamina","origin":"slug","level":1,"span":[236,245]},{"name":"regeneration","origin":"slug","level":2,"span":[326,366]}],"diagnostics":[]}
```

**Spans** `[start, end)` are file byte offsets, BOM included; lines are 1-based; CRLF is never normalised. Document `span` = the whole file. A **section** is a heading whose `id` attribute is a definable ID (other `{#…}` → `anchors`); it ends at the next heading of the same or higher level, trailing whitespace trimmed, nested sections included. `heading` = the heading line without its line ending; `body` = from the line after it to the section end; disjoint, so a rename leaves the body hash alone (05 §3.5). Section fields: `title` (heading text), `level`, `attrs` (`key=value`, bare key → null), `classes`, `rev` (`rev=N`, Q3), `parent` (nearest enclosing ID section, else the document), `kind` (prefix kind); `script` only when the ID as written is not Latin. **Document:** `kind` declared, else the prefix kind (another declared → `kind-mismatch`, declared kept); `title` from `title:`, else the first H1; `summary` = first paragraph after the first H1 (no H1: before the first heading).

## Front-matter

Optional UTF-8 BOM, then front-matter iff the next line is exactly `---`, closed by the next exact `---`. Typed keys (pub `TYPED_KEYS`, each with its `KeyType`, read-only: importers pin their copy to it; a wrong type, or a bare string for a list → `frontmatter-type`, raw kept in `extra`): `Text` `kind`, `class`, `title`, `status`, `owner`, `reviewed`, `date`, `shipped`, `ref`, `to`, `severity`, `generator`, `source`, `acceptance`, `id`; `Integer` `tier`, `rev`; `List` (of strings) `scope`, `aliases` (not lexed); `Reference` `parent`, `working_answer`, `canon`; `ReferenceList` `supersedes`, `adrs`, `refs`; `Mapping` `links` (type → reference list), `raised_by`. Other keys → `extra` in source order + one `unknown-key`. Declared links (origin `frontmatter`): each `links` item, `supersedes`, `working_answer`, `canon`; `refs`, `adrs` → `mentions`; `status: superseded-by X` → X `supersedes` this document, `src_span` at X. Floats are finite (NaN, ±inf → strings `.nan`, `.inf`, `-.inf`). Each map built (`extra` values, `raised_by`, `links`) holds a key text once: a repeat or a collection key drops its entry with one `frontmatter-type`; top-level repeats stay (`extra` is a list). Limit: a float key's text is Rust `Display` (`{1.0: a, 1: b}` drops `b`). Failed YAML: one `frontmatter-yaml` at its line, no guessed ID, the body still parsed; no lax re-parse (the importer's job).

## Body references

Scanned in text and inline code (tables and link text included), never in fenced or indented code, HTML (comments included), link destinations or attribute blocks; verbatim only (`R\-12` is none); `{#X-3}` in a paragraph is a mention. Each → `mentions`, origin `inline`, `src` = innermost ID section, else the document ID, else omitted; so is a local link destination or definition, `dst` a path (`docs/canon/spec-check-links.md`). Cost is linear: no rescan from line start per `[[`, `{#`, `@`, `-`.

## Input caps and libraries

Nesting cap 32: the root mapping counts as depth 1 (serde-saphyr `enter_depth`), so 32 levels parse and the 33rd is one `frontmatter-yaml`. Why 32 (only four top levels are spanned): serde-saphyr 1.3.0's frames cost ~20–30 KB of stack per level (debug); 64 levels of mappings-as-keys peak at ~2.0 MiB, ~5 KB short of a 2 MiB thread, 32 at ≤ 1.05 MiB.

Alias expansion cap 10 000 replayed events, over it one `frontmatter-yaml`. Both caps are the parser's; the library defaults (depth 64, own alias limits) stay behind them.

## Token estimator

`tokens_est = ceil(Σ weight(char))`, in thousandths per class: ASCII letter or digit 350, other ASCII 1400, whitespace 150, Cyrillic 450, other 1000; a document costs the whole file, a section its span; saturating `u32`. Calibrated against `claude-opus-5-5` (`count_tokens`, 2026-10-04: `fixtures/token-calibration/reference.json`): each sample ±15 %, sum not below (AC-15); a new tokenizer: a new reference and `INDEX_FORMAT`.

## Open owner questions

Working answer (the code) → what the other answer triggers.

- Q1 (parser libraries): the pins above, default features off, gaps reported, never swapped silently; `serde-saphyr`'s unpinned transitive crates (04 §6) and `serde_json` as a normal dependency await acknowledgement.
- Q2 (kind vocabulary) answered by ADR-0031 (`docs/canon/architecture.md#universal`); Q4 by the token estimator.
- Q3 (section revision syntax): the `rev=N` heading attribute. Settled → an ADR extending ADR-0026 / ADR-0018 with a `#layout` diff.
- Q5 (raw Russian test text): self-written, only in `fixtures/{spec-b,token-calibration}/`, exempt in `anonymity.rs`. "Yes" → an ADR amending ADR-0024 (`#language`); "no" → escapes built at test time.
- Q6 answered: invalid YAML scalars quoted; `dogfood.rs` parses strictly.

## Tests

`tests/`, one file per concern: `crafted_yaml.rs` (every shape at cap and cap + 1, 2 MiB thread), `genre.rs` (spec-a game design; spec-b a CLI tool, Russian prose, Cyrillic aliases), `dogfood.rs` (every document the root config walks), `check_*.rs`, `project_config.rs`, `detached.rs`, `spec_graph.rs`, `bundle_layers.rs`, `patch.rs`.
