---
class: spec
status: shipped
scope: [crates/specengine-code, crates/specengine-eval, crates/specengine-mcp, docs]
ref: pilots analysis 2026-10-03, split row 0 (pointer-sweep); docs/canon/code-identity.md "Known limits" and "Open"
shipped: 2026-10-03
adrs: []
---

# Pointer sweep: stale comment pointers, rustfmt cwd

## Why

Code and test comments cited spec sections that now only delegate to the canon (05 §5.3 → `docs/canon/code-identity.md`; 05 §5.1 for item kinds; 04 §4 → the MCP README) or headings of `docs/features/layer-a-identity.md` that its compaction removed: a reader following them paid an extra hop or hit nothing. The `scrub` doc in `targets.rs` overstated what it guarantees. rustfmt in `ast-hash` inherited the harness cwd, so a corpus toolchain file could pick it, while cargo already ran at `/`. `docs/canon/code-identity.md` listed all three as open.

## Acceptance criteria

- [x] AC-01 `grep -rn --include='*.rs' -e '05 §5\.3' -e '04 §4' -e '04 §3–4' crates` prints nothing. M: restore any row.
- [x] AC-02 `grep -rn --include='*.rs' '05 §5\.1' crates | wc -l` goes 17 → 14, every remaining hit citing layers A–C, the Bevy detector or the schedule dump. M: restore `items.rs:3`.
- [x] AC-03 Every `layer-a-identity.md` in `crates/**/*.rs` is followed by `,? AC-\d{2}`, and the five rewritten comments (code `qpath_units.rs`, `marker_levels.rs`, `ron_ambiguity.rs`; eval `ast_hash_cli.rs`, `identity_literals.rs`) cite `docs/canon/code-identity.md`. M: restore `qpath_units.rs:1`.
- [x] AC-04 A test (`crates/specengine-eval/tests/doc_pointers.rs`) scans `crates/*/src` and `crates/*/tests` `.rs` files: AC-01's patterns and AC-03's rule hold, and for every citation in the citation form ("Implementation") each quoted heading equals a `#`-heading line of that file. It finds ≥ 23 quoted headings. M: cite `"RON bindings"` in one row → red; restore one `05 §5.3` row → red.
- [x] AC-05 `grep -n 'no absolute corpus path reaches stderr' crates/specengine-eval/src/ast_hash/targets.rs` is empty; the `scrub` doc names `path+file:///<root>`.
- [x] AC-06 A test in `crates/specengine-eval/tests/ast_hash_units.rs` (beside the cargo `cwd_log` test) copies `fixtures/ast-hash` to a temp dir, runs `ast-hash --pilot <copy> --out <out outside it>` with cwd = the copy and, as `SPECENGINE_RUSTFMT`, a stub appending `<kind> <pwd -P>` to a log (`version` for `--version`, printing `rustfmt 0.0.0-stub`; else `format`, `cat`): exit 0; the log holds one `version /` line, ≥ 1 `format /` line, no other cwd; stdout `detail.rustfmt.version` = `rustfmt 0.0.0-stub`. M1: `current_dir` removed from the format spawn → red. M2: removed from the `--version` spawn → red.
- [x] AC-07 `cargo nextest run -p specengine-code -p specengine-eval -p specengine-mcp` and `cargo nextest run -p specengine-mcp --features probes` pass; the only new tests are AC-04's and AC-06's; `git diff --exit-code -- fixtures/` empty (`expected.json` unchanged, the real-rustfmt fixture test green).
- [x] AC-08 No new dependency: `git diff --exit-code -- Cargo.lock '*Cargo.toml'` empty; `build_graph.rs` green.
- [x] AC-09 `cargo run -q -p specengine-cli -- export index && cargo run -q -p specengine-cli -- check` clean; after shipping worst W ≤ 109 486 B (while the spec is live, its index line adds its bytes). Measured: "Implementation".
- [x] AC-10 At shipping `docs/canon/code-identity.md`: :93 keeps the scrub gap without the "overstates" clause; :97 loses the rustfmt sentence, keeps the process-group item; :99 deleted; the "Target table" cwd bullet names rustfmt (`--version` and each format call). `grep -c -e 'later sweep' -e 'inherits the harness cwd' -e overstates` gives 0; the file stays ≤ 12 288 B.

## Implementation

One iteration, accepted; 1153 tests pass (15 skipped: pilot), code + eval + mcp 344/344, probes 55/55; clippy, fmt clean; every named mutation red (M1, M2, AC-02, AC-03, AC-04 both, the relative-path resolution removed). Canon: `docs/canon/code-identity.md` "Target table" (rustfmt at `/`, choosing a rustfmt, a corpus at `/`), "Known limits" (the scrub gap, a relative `SPECENGINE_CARGO`), "Open" (the rustfmt and sweep items closed); eval README (rustfmt at `/`, `doc_pointers.rs`); `docs/README.md` "Enforcement" (code pointers). AC-09, AC-10 measured: check clean, worst W 109 685 B, of which 199 B are the index line of `docs/features/pilot-schemes.md`, another task's live draft: 109 486 without it, at the bound; canon 12 246 B, grep 0; eval README 9 999.

| Module | What it does |
|---|---|
| eval `ast_hash/fmt.rs` | `Rustfmt { cwd, command }`: `detect(cwd)` (`--version`) and every `format` spawn with `current_dir(cwd)`; a relative `SPECENGINE_RUSTFMT` with a directory part made absolute against the harness's cwd first |
| eval `ast_hash/mod.rs` step 3 | cwd = `targets::outside_dir(root)`; `None` (a corpus at `/`) → rustfmt not run, rows `null`, `available: false`, one stderr line |
| eval `ast_hash/targets.rs` | `outside_dir` `pub(super)`: one "outside the corpus" for cargo and rustfmt; the `scrub` doc states its gap |
| comment rows | code `ron/{structure,mod}.rs`, `lib.rs`, `items.rs`, `qpath.rs`; eval `main.rs` (the `ron --help` text); mcp `lib.rs`, `review.rs`, `probes.rs` |

Tests: eval `doc_pointers.rs` (new, std only, 6 tests: AC-01–AC-04, the rewritten comments, a scanner self-test; 20 citations, 23 headings, 2 files), `ast_hash_units.rs` (AC-06, absolute and relative stub), `ast_hash_cli.rs` (the comparison rustfmt also at `/`, its `--version` = `detail.rustfmt.version`); comment rows in code `ron_depth.rs`, `ron_cost.rs`, `ron_markers.rs`, `hash_traps.rs`, `qpath_units.rs`, `marker_levels.rs`, `ron_ambiguity.rs`, eval `identity_literals.rs`, mcp `mcp_stdio.rs` (also `07 §1.1-1.2`).

**Citation form**, what `doc_pointers.rs` resolves: `` `docs/canon/code-identity.md` "Heading" `` or `` `crates/specengine-mcp/README.md` "Heading" ``, more as `, "Heading"`; the heading verbatim with its inline code on one comment line; no heading = the opening paragraph; an AC citation `docs/features/layer-a-identity.md AC-NN`.

Deviations, now canon: an addition — a relative `SPECENGINE_RUSTFMT` resolves against the harness's cwd (at `/` a relative wrapper would break; a bare name still goes through `PATH`); a relative `SPECENGINE_CARGO` still resolves against `/`, layout follows (a fix adds code to `targets.rs`, out of scope): "Known limits". Untested: a corpus at `/`, the `--help` text. Nit left: "directory part" is `components().count() > 1`, execvp's "contains `/`" (they differ only for `rustfmt/`).
