---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-eval]
ref: 08 §2 Phase 1 "From the Phase 0 spikes"; owner's answers Q1-Q3 of the analysis, 2026-10-03
shipped: 2026-10-03
adrs: []
---

# Layer A identity: qpath target units, marker tiers, ambiguous RON paths

## Why

Phase 0 refuted "`qpath` names items unambiguously": 16.5–32.2 % of pilot items ambiguous, almost all duplicates across targets (`phase-0-spikes.md`): `file_role` judged a file by its path alone, giving every crate root of a package one empty module path and taking `tests/common/mod.rs` for one, so Phase 3 would mark a sixth to a third of a project `cannot_verify`. The marker parser kept `[tiers]` as note text and counted a glued `X[sig]`, `X*/`, `<slug>/ID` (ADR-0026) and empty IDs as mixed-script (ADR-0009). RON keys cut at 128 bytes or whitespace-collapsed rendered one path for two values. Nothing reads these formats before Phase 3 stores them (05 §3, 07): fix them first. No ADR: within `docs/canon/architecture.md` `#code-identity`, `#markers`, `#ids`, `#universal`; ADR-0021's recipe untouched.

How it works now: `docs/canon/code-identity.md`.

## Acceptance criteria

Scratch copies of `fixtures/cargo-units`, scratch `HOME`, `--out` outside the corpus. M: the mutation that must turn it red.

- [x] AC-01 `file_role`, metadata table: `src/{lib,a/b}.rs` → primary, `[]`, `a::b`; `src/main.rs` → `bin:<name>` beside a lib, else primary; `build.rs` → `custom-build:build-script-build`; `src/bin/t.rs` → `bin:t`; `src/bin/m/{main,cli}.rs` → `bin:m`, `[]`, `cli`; `examples/d.rs` → `example:d`; `tests/a.rs` → `test:a`, `tests/a/h.rs` → `shared:tests/a`, `h`; `tests/s/{main,h}.rs` → `test:s`; `tests/common/{mod,house}.rs` → `shared:tests/common`, `[]`, `house`; `benches/b.rs` → `bench:b`; a `[[bin]]` at `tools/x.rs` → `bin:x`; `scripts/x.rs`, a root `lib.rs`, with no targets `examples/x/y.rs` → `Unrooted`. `layout_targets`: the same, `tools/x.rs` `Unrooted`. M: `tests/common/mod.rs` a crate root; shared dir dropped; table roots ignored; rule 3 without targets.
- [x] AC-02 `ast-hash --pilot <copy>`: manifest `qpath`, `unit`, `targets` == `expected.json`; every `fn main`, `fn setup` in its own unit; a renamed default bin (`path = "src/main.rs"`, beside a lib) → `bin:<its name>`; a bin-only package and every lib without unit; `duplicate` only two adjacent-impl pairs, one in a shared dir; `path_attribute` only one `#[path]` target, one in a shared dir unflagged; `qpath_units` exact; `targets_from` all `metadata`. M: no unit for tests; layout forced; lib marked; no duplicate detection; shared exempt from `duplicate`; shared `#[path]` flagged.
- [x] AC-03 `fixtures/ast-hash`: `expected.json` unchanged; manifest `qpath`s == a list pinned in the test; `hash_v2.rs`, `hash_traps.rs` hash cases, `RECIPE` unchanged. M: lib marked.
- [x] AC-04 A broken root manifest, `SPECENGINE_CARGO` a missing file, a stub hanging on its first call (`--timeout 8`): exit 0; `targets_from.layout` every package dir; standard-layout qpaths as AC-02; the renamed bin → `bin:<package name>`; the hanging stub called once, killed, gone at exit. M: exit ≠ 0; fallback reported `metadata`; calls after an overrun; child not killed.
- [x] AC-05 `ast-hash` on `fixtures/ast-hash` leaves `git status --porcelain -- fixtures/` empty; stdout one JSON object, no path, target name or `.rs`. A stub logging argv and cwd, the harness started inside the corpus copy, a `rust-toolchain.toml` planted in each `TMPDIR` (inside, outside): argv `metadata --format-version 1 --no-deps --offline --color never`, cwd `/`, the planted toolchain never run; stderr off stdout; a failure line's corpus root replaced by the label only as a whole path. M: a flag dropped; stderr to stdout; cwd in the corpus or the temp dir; root forwarded; prefix-only scrub.
- [x] AC-06 Pilots (`--run-ignored only pilot`): both runs finish, fields numeric, `git status --porcelain` unchanged; every `duplicate` group within one file. A 0.2 % (`duplicate` 29, 11 groups), B 0.0 % (2 of 7 599, 1 group). M: no unit for tests.
- [x] AC-07 Levels, LF, CRLF: `X@3 [sig]` → rev 3, `[sig]`, no note; `X [body, sig]` → `[sig, body]`; all four; `X@2` → `Default`; `X@3[sig]`, `X[sig]` → ID `X`, `id_latin`; `X [sig] mutation: "y"` → that note; `X see [docs]` → that note, `Default`; `@implements A [sig] @verifies B [body] n` → own levels, B's note `n`; `/* @configures C [sig] */` → no note; `[sig, body,]` declared. M: brackets in the note; order kept; `[` in the ID.
- [x] AC-08 `[]`, `[ ]` → `empty`; `[sig, sig]` → `duplicate`; `[sgi]`, `[Sig]`, `[sig body]`, `[sig,,body]`, `[,]`, `[sig, sig, x]` → `unknown`; `[sig` at line end, before `*/` or the next keyword → `unclosed`; a 1 MB list → `unknown`, the order of time of a 1 MB note. Each keeps ID and rev, levels `Invalid`; the next marker intact. M: invalid → default; marker dropped; text in the error.
- [x] AC-09 `/* @implements X*/` → `X`; `@verifies feat/AC-07@2` → `feat/AC-07`, rev 2, `id_latin`; `// @implements` at line end → empty, `id_latin`, eval `id_empty` 1, `id_not_latin` 0; a non-Latin ID → `id_latin` false. M: `*/` in the ID; `/` rejected; empty counted non-Latin.
- [x] AC-10 `ron` on `fixtures/ron`: rows carry `levels`, `levels_state`, `ambiguous`; `markers.levels`, `levels_invalid`, `id_empty` == `expected.json` with `ambiguous.ron`; `CONFIG_ANCHORS`, note assertions unchanged. M: rows without levels; invalid counted declared.
- [x] AC-11 `analyze`: siblings sharing their first 128 bytes, marked or one marked → `Ambiguous`, same path text; `"fire  ice"` beside `"fire ice"`, `speed: 1, speed: 2` → `Ambiguous`; a lone truncated key, keys differing within 128 bytes → `Path`; deep under a colliding sibling → `Ambiguous`, on their map → `Path`; `colliding_groups` one per group. M: flag on truncation; only marked siblings compared; dedup by path.
- [x] AC-12 `Ambiguous` keeps the path text, counted `ambiguous`, not `anchored`; `ADJACENCY` (53 × LF/CRLF) unchanged. M: ambiguous counted anchored.
- [x] AC-13 `ron_cost.rs`, `assert_same_order`: 50 000 siblings sharing 128 bytes, marked and not, vs distinct keys; 50 000 vs 10 × 5 000 colliding marked siblings (factor 4); unmarked colliding siblings ≈ 400 maps deep vs distinct. M: pairwise compare (scaling); a full path per value (deep).
- [x] AC-14 `build_graph.rs`: `specengine-code` `[dependencies]` exactly `tree-sitter`, `tree-sitter-rust`, `blake3`; no `[workspace.dependencies]` key added; `CLI_FORBIDDEN` holds. M: `serde_json` in `specengine-code`.
- [x] AC-15 `identity_literals.rs` scans `qpath.rs`, `ast_hash/targets.rs`, the qpath step of `ast_hash/mod.rs` (`// 5. qpath` to `// 6.`, both asserted): no directory or package literal beyond Cargo's; `anonymity.rs` green. M: an `"xtask"` literal in each; a step comment renamed.
- [x] AC-16 At shipping (measurement): the canon ≤ 12 288 B; 05 ≤ ≈ 36 500 B; worst W ≤ ≈ 109 300 B; 08, this spec < 15 737 B; code, eval READMEs ≤ 10 240 B, the Phase 1 gaps gone; check clean. Measured: "Implementation".

## Implementation

Canon: `docs/canon/code-identity.md` (new: target table, units, markers, RON binding and ambiguity, eval keys, limits); code and eval READMEs; 05 §5.1, §5.3 → pointers; 08 Phase 1. Three iterations, each accepted; 1146 of 1146 tests (15 skipped), clippy and fmt clean, every named mutation red. AC-16: canon 12 255 B; 05 36 723 (was 39 430; ≈ 36 500 missed by 223: all of this task's slice is pointers, the rest is other tasks'); worst W 109 486 (was 112 213; ≈ 109 300 missed by 186); 08 15 403; this spec 9 968; READMEs code 6 491, eval 10 019; check clean.

| Module | What it does |
|---|---|
| code `qpath.rs` | `PackageTargets`, `Target`, `TargetSource`, `Unit`; `file_role` rules 1–6, primary target, `TargetIndex`, `layout_targets` |
| code `markers.rs` | `Levels`, `Level`, `LevelError`; ID boundary; level lists; a line-end cache (linear) |
| code `ron/{mod,structure}.rs` | `Anchor::Ambiguous`, `colliding_groups`; sibling segment sets, groups linked to parents, decided after the walk |
| eval `ast_hash/targets.rs` (new) | the `cargo metadata` runner (budget, kill, cwd `/`, scrub), absorption, layout fallback |
| eval `ast_hash/mod.rs`, `main.rs` | units, `qpath_units`, `targets_from`, manifest `unit`, `targets`; `run_end` from `--timeout` |
| eval `ron.rs` | `ambiguous`, `id_empty`, `levels`, `levels_invalid`, `colliding_groups`, row keys |

Also: code `lib.rs` re-exports; comment pointers only in core `markdown.rs`, `check/{text,rules_toml}.rs`, mcp `read.rs`, `server.rs`. Tests: code `qpath_units.rs`, `marker_levels.rs`, `ron_ambiguity.rs` (new), `hash_traps.rs`, `ron_cost.rs`, `ron_markers.rs`; eval `ast_hash_units.rs`, `identity_literals.rs` (new), `ast_hash_cli.rs`, `ron_cli.rs`, `build_graph.rs`; `fixtures/cargo-units/` (new, own `[workspace]`), `fixtures/ron/ambiguous.ron`.

Deviations, now canon: the developer's reading 9 (`tests/a/h.rs` beside `tests/a.rs` → `shared:tests/a`) was flagged as a limit; the reviewer showed it correct (a `<d>/<n>.rs` root never owns `<d>/<n>/`, E0583). Shared items are `duplicate` (iteration 1 exempted them from every reason), never `path_attribute`; rules 2–4 give `Unrooted` without targets; `levels` rows the effective list; an empty ID also before `[`, `*/`. Iteration 2 hardened the runner: the per-call budget, no call when nothing is left, an overrun killed and ending the cargo calls, `--color never`, the stderr scrub; its temp-dir cwd became `/` only in iteration 3 (a world-writable temp dir can plant a toolchain), and the scrub whole-path only. Unpinned: the 60 s constant (AC-04 cuts it by `--timeout`); "no call after an overrun" seen only through the stderr note. Pilots measured in iteration 2, not re-run after iteration 3 (cwd and scrub only). Residue: canon "Known limits", "Open".
