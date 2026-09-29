---
class: spec
status: shipped
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-eval]
ref: owner instruction to fix the minors and nits of the Phase 1 parser, index and check increment 1 (5c86e1c, 0b49eaf, 85e31c5)
shipped: 2026-09-29
adrs: []
---

# Phase 1 cleanup: the open minors of the parser, index and check

## Why

The three shipped Phase 1 increments left recorded defects (Tier 1 READMEs, `docs/canon/spec-check.md` "Open minors", the specs' "Implementation"): silent wrong answers (`update_paths` ignores a new directory; `file()` ≠ `parse` for `x: .nan`), repeated JSON object keys, subject `""` for key-level parser findings (too coarse for the debt baseline), counters whose names lie, citations of compacted spec sections. Baseline granularity must be right before increment 3 forbids new baseline entries; the `parse` counters before the importer increment's pilot runs (08 §2 Phase 1).

"Fix the minors and nits" answers the four items left for the owner with the recommended answers: `float_roundtrip` to `[workspace.dependencies]`, `update_paths` escalation, the `parse` key rename now, a start tag split over lines accepted (as in `xtask`). All fixes stay inside accepted decisions (`#storage`, `#universal`, `#ids`, `#control`, `#checks-migration`): no ADR, no rule of `docs/canon/architecture.md` changes.

## Acceptance criteria

Item codes: M model, P parser, K check, S store, E eval, D docs. A "Mutation" must turn its criterion red.

- [x] AC-01 (M1) — model `tests/grammar.rs`: `Q` aliased from a Latin `QST` and a Cyrillic alias (`\u{…}`), each + `-\u{FF10}\u{FF13}\u{FF11}`: one reference each, `alias_of` `Q`, `id` ending `-031`, no `homoglyph`; `Q-\u{FF10}31` still gives `homoglyph`, fix `Q-031`. Mutation: set `changed` in the alias branch.
- [x] AC-02 (P1) — `from_toml_errors_name_the_line_and_load_nothing`: `Z` bad on line 2, `A` on line 3 → line 2, for `shape = "blob"` and for a missing `width`. Mutation: key order.
- [x] AC-03 (P2) — review: `Root` doc numbers 1–4.
- [x] AC-04 (P3) — core `front_matter.rs` unit `float_value_tests`: NaN, ±inf → `.nan`, `.inf`, `-.inf`, a kept float survives a `serde_json` round trip; `tests/front_matter.rs`: `.nan`, `.inf`, `-.inf`, `1e999`, `-1e999` give those strings in `extra`, a round trip equals the parse; store `tests/projection.rs`: `file(path).parsed == parse(…)`. Mutation: keep non-finite `Float` in `float_value` (red at the unit test only: the guard is defensive, see "Implementation").
- [x] AC-05 (P4) — `tests/front_matter.rs`: `x: {1: a, "1": b}`, `raised_by: {1: a, "1": b}`, `links: {1: [R-01], "1": [R-02]}`, a nested `? [k]` key, a nested repeat: first entry kept, one `frontmatter-type` per dropped entry, no JSON object repeats a key; top-level `1:` + `"1":` keep both. Mutation: keep duplicates.
- [x] AC-06 (K1) — core `tests/check_baseline.rs`: `x_a: 1`, `x_b: 2` → `unknown-key` subjects `x_a`, `x_b`; `tier: high` → `frontmatter-type` subject `tier`; `!!str k:`, `&a k:` (both orders) → `k`; `? a`, `? "a" # c` → `a`; `? [a, b]`, `? x: 1`, `[a, b]:`, `{a: 1}:`, `*alias` lines → `""`; entry `(unknown-key, path, x_a)` makes only `x_a` debt, nothing stale; `check_classes.rs`: these keys reach `key-missing` / `key-extra`; store `check_parity.rs` `clean`, 7 debt. Mutations: `""` for all spanless; a key subject on `frontmatter-yaml`.
- [x] AC-07 (K2) — `tests/check_classes.rs`: the `tier0` file with `tier: 1`, `tier: 5`: one `tier-invalid` each; `tier: 0`: none. Mutation: rules fire independently.
- [x] AC-08 (K3) — `tests/check_classes.rs`: accepted decisions with `canon: docs/x.md#`, `"#"`, `[a]`, `{a: b}`, `"\t"`: one `canon-missing` each, "the written `canon:` of an accepted decision is unreadable (the promotion rule)"; `canon:` absent, empty, `""`, `''`, `'  '`, `" "`: "has no `canon:`"; spec-check AC-20 green. Mutation: the old message.
- [x] AC-09 (K4) — `tests/check_output.rs`: three `SkippedName` problems on one path → one `name-skipped` "3 directory or `.md` names that are not UTF-8 were skipped"; one → "1 directory or `.md` name that is not UTF-8 was skipped". Mutations: one per problem; no count.
- [x] AC-10 (K5) — `tests/anchors.rs`: `<div>\n<!-->\n<a id="after"></a>\n</div>`, and with `<!--->`: `html` anchor `after`; an `<a id>` inside `<!-- … -->` is not yielded. Mutation: + 4.
- [x] AC-11 (S1) — store `tests/incremental.rs`: a new directory with `a.md` then `update_paths([dir])` → dump = fresh rebuild; the same for an edited file named `./p`, absolute, `""`, and its parent + `/`; a clean `.md` path reports `walked` 1. Mutations: no escalation; always walk.
- [x] AC-12 (S2) — eval `tests/build_graph.rs`: the root `serde_json` has `float_roundtrip`; no member sets a `serde_json` feature; `no_workspace_dependency_is_added_against_main` green. Mutation: feature back on the store.
- [x] AC-13 (S3) — `#[cfg(test)] mod own_text_oracle` in store `rows.rs`: `own_text` = the old full scan on every node of spec-a, spec-b and a crafted nested file; 50 000 top-level ID sections: the `own_text` loop alone ≤ 2 s in debug (16 ms). Mutation: full scan.
- [x] AC-14 (S4) — store `tests/walk.rs`: after `list`, a listed directory is replaced by a symlink to a copy: `read` under it errs, `probe` false. Mutation: last component only.
- [x] AC-15 (S5) — review: the `write.rs`, `error.rs` comments per S5 (`INSERT OR REPLACE` fires no `nodes_fts_delete`; `Busy` = `SQLITE_BUSY` + `SQLITE_LOCKED`).
- [x] AC-16 (S6) — `INDEX_FORMAT == 3`; `format_history.txt` ends `3 <hash>`; `tests/format.rs` green.
- [x] AC-17 (E1) — eval `tests/parse_cli.rs`: one scratch file with 50 000 ID sections, `--timeout 60`: numeric `result` (~1 s), `sections.differ` 0. Mutation: rescan from byte 0 → `"timeout"`.
- [x] AC-18 (E2, E3) — `parse_cli.rs`: top-level `diagnostics.<code>` for every code, `front_matter` = `{present}`; `corpus-mini`: `diagnostics.homoglyph` 1, `references.homoglyph` 1; a scratch look-alike only in `id:`: `references.homoglyph` 0, `diagnostics.homoglyph` 1. Mutation: count definitions.
- [x] AC-19 (E4) — `parse_cli.rs`: a mode-000 document → `unreadable` 1, `files` unchanged. Mutation: uncounted.
- [x] AC-20 (E5) — `tests/index_cli.rs`: a mode-000 `.md` → `unreadable` 1. Mutation: stored `read_error` only.
- [x] AC-21 (E6) — `index_cli.rs`, `check_cli.rs`: `--config x` → exit 2, stderr "`<m>: refused: --config x is not read by <m> (its configuration is --scheme); nothing written`" (`<m>` in backticks), `--out` empty. Mutation: ignore the flag.
- [x] AC-22 (D1) — `grep -rnE '(//|# ).*docs/features/[a-z0-9-]+\.md(, *| \()("|$)' crates` lists only `bevy_cli.rs:1`, `ast_hash_cli.rs:1` (AC citations); each D1 target exists.
- [x] AC-23 (D2, canon) — `docs/canon/spec-check.md`: "Configuration" states that a written class replaces its default contract whole, "Findings" the K1 subject rule and "a finding identical in every field is reported once", "Rules" one `name-skipped` per path, two `canon-missing` messages, the split-tag divergence, "Open minors" only `CheckFile.bytes` + the accepted limits; READMEs: model and store "Open minors" pruned (each item naming its increment), core "Open minors" gone, store `is_dir` + escalation + `INDEX_FORMAT = 3`, eval `parse` / `index` / exit 2 per E2–E6.
- [x] AC-24 — core `genre.rs`, `check_genre.rs`, eval `anonymity.rs` green.
- [x] AC-25 — `cargo nextest run --workspace`: 504 passed, 0 failed, 15 skipped (parser AC-19, index AC-05–AC-10, check AC-18–AC-20 included); `git status --porcelain -- fixtures/`, `git diff --stat -- xtask/` empty; timed tests self-bounded.
- [x] AC-26 — `cargo xtask docs index --write && cargo xtask docs check` green; canon `spec-check.md` 12 195 B ≤ 12 288; W 117 874 B ≤ 117 883 (task start 118 126): largest Tier 1 README (store) 9 510 B ≤ 9 515.

## Implementation

Two iterations; review accepted in both, nothing left open. Iteration 1 built M1, P1–P4, K1–K5, S1–S6, E1–E6, D1; iteration 2 fixed the reviewer's K1 minor (special key forms) and K3 for a blank `canon:`, and documented P3 as defensive. `INDEX_FORMAT` 3: the spec-a + spec-b dump changed only in its stamp (`format_history.txt` `3 d63c06dc…`).

| Module | What it does |
|---|---|
| model `grammar.rs`, `reference.rs`, `value.rs` | M1: an alias match emits no `homoglyph`, its body normalised; docs: `Float` finite, map key texts unique |
| core `front_matter.rs` | P3 `float_value` + unit `float_value_tests`; P4 `ordered_map`, `links`, `dropped_entry`: a repeated or collection key → `frontmatter-type` |
| core `scheme_toml.rs`, `yaml.rs` | P1 `[ids]` validated in source order; P2 `Root` doc levels 1–4 |
| core `check/text.rs` | K1 `top_level_entries` (tags, anchors, explicit, flow, alias keys), `key_at` |
| core `check/engine.rs` | K1 `is_key_level`; K2 one `tier-invalid` on `tier0`; K3 `canon_unreadable`; K4 `name-skipped` per path with the count |
| core `markdown.rs` | K5 `-->` scan from the opener + 2 |
| store `source.rs` | S1 `Source::is_dir` (provided), `WorkingTree::is_dir`; S4 `read` refuses any symlink component |
| store `write.rs`, `lib.rs`, `error.rs` | S1 `update_paths` escalation + rustdoc; S5 comments; S6 `INDEX_FORMAT = 3` + note |
| store `rows.rs` | S3 `own_text` scans only later nodes; `own_text_oracle` |
| `Cargo.toml` (root, store, mcp) | S2 `float_roundtrip` on the workspace `serde_json`; D1 citation |
| eval `parse.rs`, `index.rs`, `main.rs` | E1 one line index per file; E2–E4 result shape; E5 read failures in `unreadable`; E6 `refuse_config` |
| D1 comments | `grammar.rs`, `node.rs`, `paths_toml.rs`, `dump.rs`, `schema.rs`, `source.rs`, `parse.rs`, `index.rs`, `anonymity.rs` cite README sections |

Deviations from the draft. P3: its "before: JSON null" premise does not reproduce — `yaml.rs` sets `reject_non_finite_typeless_float: false`, so serde-saphyr 1.3.0 already delivers `.nan`, `.inf`, `-.inf`, `1e999`, `!!float .inf` as strings; `float_value` is a guard, red only at its unit test. AC-05 uses `raised_by: {1: a, "1": b}`: `{true: a, "true": b}` is a serde-saphyr duplicate-key error (one `frontmatter-yaml`), so the repeat branch of `links()` is defensive too. K1 covers tagged, anchored, explicit, flow and alias keys; K3 reads a blank string as absent. Left by decision (diminishing returns): the limits in `docs/canon/spec-check.md` "Open minors", the float-key text (core README "Front-matter"), the `read` race (store README).
