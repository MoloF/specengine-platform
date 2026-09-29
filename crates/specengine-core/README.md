---
class: canon
tier: 1
scope: [crates/specengine-core]
owner: owner
reviewed: 2026-09-29
---

# specengine-core — the spec parser

The reading core of Phase 1; today the parser: one file's bytes → `ParsedFile`. No file access and no SpecEngine crate but `specengine-model` (types, `[ids]`, the reference grammar: its README). Output depends only on (path, bytes, scheme), maps serialise sorted or in source order; a broken file is reported, never fatal (ADR-0012). Pins (Q1): `pulldown-cmark =0.13.4` (over the default members; `specengine-ra` gets 0.9.6 through `ra_ap_ide`), `serde-saphyr =1.3.0` (`deserialize` only), `toml` for `[ids]` and `[paths]`. Callers: `specengine-store`, `specengine-eval parse`.

## API

`parse(path: &str, bytes: &[u8], scheme: &IdScheme) -> ParsedFile`; `IdScheme::from_toml(&str)` (trait `IdSchemeToml`, also `scheme_from_toml`) reads only `[ids]`; `Paths::from_toml(&str)` (also `paths_from_toml`) only `[paths]`; `tokens_est(&str) -> u32`; `MAX_DEPTH = 32`, `MAX_ALIAS_EXPANSION = 10_000`.

## `[paths]`

What the index walks (the walk itself: store README). `Paths {spec, records, features, generated, archive, roots, exclude}`: role keys default to `DEFAULT_{SPEC,RECORDS,FEATURES,GENERATED,ARCHIVE}` = `docs/spec`, `docs/records`, `docs/features`, `docs/generated`, `docs/archive`; `roots` are walked directories or single `.md` files, default the role directories but `generated` (SpecEngine's own output); `exclude` holds census globs (`*`, `**`, `?`) over root-relative file paths. Every path is root-relative with `/`: no leading `/`, no `..`, no `.` or empty component; one trailing `/` is dropped. An unknown key, a wrong type or a bad path → `PathsError {line?, message}`, `at(file)` → `file:line: message`; other tables are ignored. Pure: existence is the walker's business.

```toml
[paths]
spec = "docs/spec"
roots = ["docs", "crates", "CLAUDE.md"]
exclude = ["docs/archive/old/**"]
```

## Output

`ParsedFile = {path, bom, front_matter?, body, nodes, links, anchors, diagnostics}`; BOM + front-matter + body = the file, even when it is not UTF-8. `nodes[0]` is the document (`document()`), sections follow in source order (`sections()`); absent optional fields are omitted. Nodes cover 05 §3.3 `nodes` except `project`, `worktree`, `norm_hash`. `diagnostics` come in line order; the codes: model README.

```json
{"path":"spec/stamina.md","bom":false,"front_matter":[0,236],"body":[236,512],"nodes":[
 {"id":"MEC-STAMINA","kind":"mechanic","title":"Stamina","summary":[247,312],"parent":{"id":"DOM-MOVEMENT","span":[81,93]},"span":[0,512],"tokens_est":131,"fields":{"tier":2},"extra":[]},
 {"id":"RULE-STAM-REGEN","kind":"rule","title":"Regeneration","level":2,"heading":[326,366],"body":[366,440],"attrs":[["rev","3"]],"rev":3,"parent":{"id":"MEC-STAMINA"},"span":[326,440],"tokens_est":29}],
 "links":[{"src":"MEC-STAMINA","type":"derived_from","origin":"frontmatter","dst":{"id":"R-12","script":"latin","span":[118,122]}}],"anchors":[],"diagnostics":[]}
```

**Spans** `[start, end)` are file byte offsets, BOM included; lines are 1-based; CRLF is never normalised. Document `span` = the whole file. A **section** is a heading whose `id` attribute is a definable ID (other `{#…}` → `anchors`); it ends at the next heading of the same or higher level, trailing whitespace trimmed, nested sections included (the census rule). `heading` = the heading line without its line ending; `body` = from the line after it to the section end; disjoint, so a rename leaves the body hash alone (05 §3.5). Section fields: `title` (heading text), `level`, `attrs` (`key=value`, bare key → null), `classes`, `rev` (`rev=N`, Q3), `parent` (nearest enclosing ID section, else the document), `kind` (prefix kind); `script` only when the ID as written is not Latin. **Document:** `kind` declared, else the prefix kind (another declared → `kind-mismatch`, declared kept); `title` from `title:`, else the first H1; `summary` = first paragraph after the first H1 (no H1: before the first heading).

## Front-matter

Optional UTF-8 BOM, then front-matter iff the next line is exactly `---`, closed by the next exact `---`. Typed keys (wrong type, or a bare string for a list → `frontmatter-type`, raw kept in `extra`): strings `kind`, `class`, `title`, `status`, `owner`, `reviewed`, `date`, `shipped`, `ref`, `to`, `severity`, `generator`, `source`, `acceptance`; integers `tier`, `rev`; string lists `scope`, `aliases` (not lexed); `id`; references `parent`, `working_answer`, `canon`; reference lists `supersedes`, `adrs`, `refs`; `links` (type → reference list); `raised_by` (map). Other keys → `extra` in source order + one `unknown-key`. Declared links (origin `frontmatter`): each `links` item, `supersedes`, `working_answer`, `canon`; `refs`, `adrs` → `mentions`; `status: superseded-by X` → X `supersedes` this document, `src_span` at X. Failed YAML: one `frontmatter-yaml` at its line, no guessed ID, the body still parsed; no lax re-parse (the importer's job).

## Body references

Scanned in text and inline code (tables and link text included), never in fenced or indented code, HTML (comments included), link destinations or attribute blocks; verbatim only (`R\-12` is none); `{#X-3}` in a paragraph is a mention. Each → `mentions`, origin `inline`, `src` = innermost ID section, else the document ID, else omitted. Cost is linear: no rescan from line start per `[[`, `{#`, `@`, `-`.

## Input caps and libraries

Nesting cap 32: the root mapping counts as depth 1 (serde-saphyr `enter_depth`), so 32 levels parse and the 33rd is one `frontmatter-yaml`. Why 32, for the current design (only the four top levels spanned), measured with serde-saphyr 1.3.0 in a debug build: its own frames cost ~20–30 KB of stack per YAML level (mappings as keys worst, ~30 KB); 64 levels of mappings-as-keys peak at ~2.0 MiB, ~5 KB short of a 2 MiB spawned thread, while at 32 every shape peaks ≤ 1.05 MiB.

Alias expansion cap 10 000 replayed events, over it one `frontmatter-yaml`. Both caps are the parser's budgets; the library defaults (depth 64, own alias limits) stay behind them. Verified at adoption: serde-saphyr 1.3.0 — `deserialize_any` into an own value tree with `Spanned<T>`, `Error::location()` gives line and column but syntax errors carry no byte offset, duplicate keys are an error, `strict_booleans`; pulldown-cmark 0.13.4 — heading attributes give id, classes, `key=value`, `into_offset_iter` byte ranges keep CRLF.

## Token estimator

`tokens_est = ceil(Σ weight(char))`, in thousandths per class: ASCII letter or digit 270, other ASCII 500, whitespace 150, Cyrillic 500, other letters 1000, rest 1000; a document costs the whole file, a section its span; saturating `u32`. **Uncalibrated** and conservative: `fixtures/token-calibration/` (English, Russian, mixed, code block, table) has null `reference.json` counts, and AC-15 (±15 % per sample, sum ≤ 5 % below) is `#[ignore]` until the owner fills them (Q4). Budgets stay in bytes until an ADR amending ADR-0022 moves them to tokens, due before `spec check`.

## Open owner questions

Working answers are what the code does now; the owner's answer triggers the step named.

- Q1 (parser libraries): the pins above, default features off, gaps reported, never swapped silently; `serde-saphyr`'s unpinned transitive crates (04 §6) await acknowledgement.
- Q2 (kind vocabulary): a free string, not validated. Settled → an ADR amending `docs/canon/architecture.md#universal`.
- Q3 (section revision syntax): the `rev=N` heading attribute. Settled → an ADR extending ADR-0002 / ADR-0018 with a `#layout` diff.
- Q4 (reference token counts): filled → AC-15 un-ignored.
- Q5 (raw Russian test text): self-written, only in `fixtures/spec-b/` and `fixtures/token-calibration/`, exactly what `anonymity.rs` exempts from the ADR-0024 check. "Yes" → an ADR amending ADR-0024 with a `#language` diff; "no" → the exemption list empties and the text becomes escapes generated at test time.
- Q6 (editing accepted ADRs): no, so ADR-0015, -0018, -0020 and `docs/features/phase-0-spikes.md` keep one invalid YAML scalar each (`frontmatter-yaml`, allowlisted in `dogfood.rs`). Edits allowed → quote the four scalars, empty the allowlist.

## Tests

`tests/`: `spans.rs`, `front_matter.rs`, `crafted_yaml.rs` (every shape at the cap and cap + 1 on a 2 MiB thread; no free-stack margin is claimed), `sections.rs`, `references.rs`, `links.rs`, `records.rs`, `genre.rs` (`fixtures/spec-a`: game design, English; `fixtures/spec-b`: command-line tool, Russian prose, Cyrillic aliases), `tokens.rs`, `determinism.rs`, `cost.rs`, `dogfood.rs` (every document of this repository).

## Open minors

- `scheme_toml` validates entries alphabetically: of two bad entries the alphabetically first line is reported.
- The `yaml.rs` `Root` doc numbers levels 0–3 while `MAX_DEPTH` counts from 1.
