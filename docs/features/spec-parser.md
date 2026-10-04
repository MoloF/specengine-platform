---
class: spec
status: shipped
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-eval]
ref: 08-roadmap.md Phase 1, first bullet (the spec parser)
shipped: 2026-09-29
adrs: []
---

# Spec parser: the corpus model and the single reference grammar

## Why

Nothing turned a corpus into addressable nodes; later Phase 1 pieces need each file as a document node plus `{#ID}` section nodes, with links, byte-exact spans, token cost and diagnostics. The stake is W: `get_node` returns a section, not a file (07 §1.2), the bundle prices a node before loading it (05 §6), Phase 2 patches by section. Corpora are read through their `specengine.toml` alone (ADR-0008); a broken file is reported, never fatal (ADR-0012). The reference grammar (05 §3.2 had one sentence and contradicting examples) is settled here.

## Acceptance criteria

Tests in `crates/specengine-core/tests/` unless named; AC-01–03 in `specengine-eval` `tests/build_graph.rs`.

- [x] AC-01 Default members are exactly seven with `-model`, `-core`; neither pulls `ra_ap_*`, `syn` 3, Bevy. Mutation: `syn` 3 or `specengine-ra` in `-core` → red.
- [x] AC-02 `-model` normal graph has no `specengine-*`, `pulldown-cmark`, `serde-saphyr`; `-core` none of `specengine-{code,import,mcp,eval,ra}`; no `std::fs`/`File::`/`OpenOptions` in their `src`. Mutation: model on `pulldown-cmark` → red.
- [x] AC-03 `=0.13.4` / `=1.3.0` pins over the default members (`specengine-ra` has 0.9.6).
- [x] AC-04 `spans.rs`: every fixture file and its CRLF, BOM, CRLF+BOM variants — each span is verbatim; BOM + front-matter + body rebuild the file. Mutation: CRLF→LF or BOM strip before offsets → red.
- [x] AC-05 `front_matter.rs`: the Q-031 record gives typed keys; `x_custom: 1` → `extra` + one `unknown-key`. Mutation: `deny_unknown_fields` → red.
- [x] AC-06 `front_matter.rs`: `title: Hybrid: revision` on line 3 → one `frontmatter-yaml` at line 3, no node ID, sections kept, siblings clean. Mutations: failed YAML read as absent, or `?` → red.
- [x] AC-07 `crafted_yaml.rs`: nesting ≤ 32 of every shape parses on a 2 MiB debug thread; 33 levels, 100 000 nested `[`, an alias bomb → one `frontmatter-yaml` each within 1 s; ~11 000 alias expansions error, ~4 500 parse. Mutations: depth budgets unbounded → SIGABRT (stack overflow) on 100 000 nesting; budget lines deleted → the library default (64) still stops input and the cap+1 cases go red; alias budget removed or raised → `alias_expansion_over_ten_thousand_is_an_error_and_under_is_not` red (the bomb alone is also stopped by the library defaults).
- [x] AC-08 `sections.rs`: only `{#ID}` headings are sections, nested ones included, ending before a same-level heading; fence, HTML comment, paragraph ignored; `duplicate-id` keeps both. Mutations: raw-line regex; end at any heading → red.
- [x] AC-09 `specengine-model` `tests/grammar.rs`: every grammar example, `canon:` path + anchor, `superseded-by` → `supersedes`. Mutation: `#` joined without an ID → red.
- [x] AC-10 `references.rs`: only configured prefixes, boundaries hold, none in fence or HTML. Mutations: generic `[A-Z]+-[0-9]+`, boundary dropped → red.
- [x] AC-11 `references.rs`: look-alike IDs → Latin, `mixed`, `homoglyph` + fix; an alias → `alias_of`, `non-latin`, no warning. Mutations: ASCII only, or all non-ASCII flagged → red.
- [x] AC-12 `links.rs`: the stamina mechanic's declared links with spans, `parent` not a link, the citation's `src` = `RULE-STAM-REGEN`, `uses_terms` kept as unknown. Mutation: `src` always the document → red.
- [x] AC-13 `records.rs`: kind from `[ids]`; another declared → `kind-mismatch`, declared kept. Mutation: prefix kind wins → red.
- [x] AC-14 `genre.rs`: `fixtures/spec-a` and `fixtures/spec-b` pass the same tests; no prefix is a literal in model or core `src`.
- [x] AC-15 `tokens.rs`: each sample ±15 % of `reference.json`, sum ≤ 5 % below. Mutation: `chars / 4` → Russian out of band. Met by `token-calibration` (2026-10-04; tightened to sum not below).
- [x] AC-16 `tokens.rs`: empty → 0, monotone, Russian above English, stable, every node estimated. Mutation: one weight → red.
- [x] AC-17 `determinism.rs`: twice and reversed → byte-identical JSON. Mutation: a `HashMap` in output → red.
- [x] AC-18 `cost.rs`: 1 MB line, 100 000 each of `{#`, `[[`, ID-like tokens, ≤ 2 s. Mutation: rescan per opener → timeout.
- [x] AC-19 `dogfood.rs`: every budgeted document parses cleanly except exactly ADR-0015, -0018, -0020 and `phase-0-spikes.md` (Q6). Mutation: lax YAML fallback → red.
- [x] AC-20 `specengine-eval` `tests/parse_cli.rs` on `fixtures/corpus-mini`: one envelope, `panics` 0, `differ` 0, no path or ID, `fixtures/` clean. Pilot runs are owner-run (`#[ignore]`).
- [x] AC-21 Docs check green; worst W 118 112 B ≤ 118 244 B.

## Implementation

Two iterations. Built: `specengine-model` (types, `[ids]` scheme, one reference grammar, look-alike table), `specengine-core` (`parse`: `front_matter` + `yaml` over serde-saphyr, `markdown` over pulldown-cmark offsets, `scheme_toml`, `tokens`), `specengine-eval parse`; both crates in `default-members`, parsers `=`-pinned. Fixtures `spec-a` (game design, English), `spec-b` (command-line tool, Russian prose), `token-calibration/`, `corpus-mini/specengine.toml`.

**Stack overflow.** serde-saphyr recurses per YAML level at ~20–30 KB of debug stack (mappings as keys worst), and a `Spanned<T>` wrapper adds ~17 KB per level; spanning every level overflowed a 2 MiB thread. Fix: only the four top levels are spanned, and the cap is 32 instead of 64 — even so, 64 levels of mappings-as-keys peak at ~2.0 MiB. Rationale and figures: `crates/specengine-core/README.md`, "Input caps".

**Additive deviations** (reviewed, recorded in the READMEs): `Diagnostic.message` always present; `Node.script` only for a non-Latin ID; sections carry `title` and `classes`; `Link.src_span` for `superseded-by`; `bad-rev` also for a `rev` out of range or unreadable; `src` omitted for a mention in a file with no ID — conflicts with the 05 §3.3 `links` key, flagged for the index increment.

Truth lives in `crates/specengine-{model,core,eval}/README.md`, 05 §3.1–§3.2, 04 §6, 07 §5; open owner questions Q1–Q6 with the ADR each answer triggers (and R7, tokens vs bytes): core README.
