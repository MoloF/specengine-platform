---
class: spec
status: shipped
scope: [crates/specengine-eval, crates/specengine-core, docs]
ref: pilots analysis 2026-10-03, split row 1 (pilot-schemes), owner's answers Q1-Q9; 08 §2 Phase 1 "Pilot projects"; 08 §3 AC-10
shipped: 2026-10-03
adrs: []
---

# Pilot schemes: the reading core on both pilots

## Why

ADR-0008 makes Phase 1 exit on two pilots, yet `index` and `check` had never read a real corpus: their pilot runs skipped until the schemes gained `[paths]`, `[budgets]`, `[check]`, and 08 §3 AC-10's index half read "pilot run pending". The later pilot tasks (import records, the after-tree, W on ten tasks) all stand on both.

Each pilot gets a "before" `specengine.toml` (SpecEngine's view of the pilot as it is) in the owner's pilot-config directory, reached through `SPECENGINE_SCHEME_A` / `_B`; the `parse`, `index`, `check` pilot runs measure instead of skipping, read-only; whatever a real corpus breaks is fixed generically or recorded; the numbers go to 08. Nothing is written into a pilot. Pilots are A and B only, in every file of this repository.

## Acceptance criteria

Fixture ACs (decide "implemented"):

- [x] AC-01 Setup: a temp copy of `fixtures/spec-b` (`git init -q`, nothing committed), a test-written scheme with every recipe table and key of the example (one `aliases_from`, one `[[generators]]` without `index`), a test-written census config. `parse`, `index`, `check` with `--label pilot-a` and `SPECENGINE_PILOT_A`, `SPECENGINE_SCHEME_A`, `SPECENGINE_CENSUS_CONFIG_A` set on the child each exit 0; `check` `verdicts.observe` `clean` or `observed`. M: the `index` scheme reader rejecting a table other than `[ids]` / `[paths]` → red.
- [x] AC-02 In each of the three files a non-ignored test runs the helper on the AC-01 setup (green) and with `[paths]` removed (`#[should_panic]`, the message naming the scheme variable); the `#[ignore]` tests call the same helper; `grep -n -e 'has no \[paths\]' -e informational crates/specengine-eval/tests/{index,check}_cli.rs` prints nothing. M1: the skip restored → red. M2: a `panic!` injected into the front-matter reader → the green `parse` test red.
- [x] AC-03 The helpers take the proof and set the child environment. Proof unit tests: `File::set_modified` +1 s on a file under a root between the proofs → unequal; a file created at the corpus root outside every root → unequal. M: `index` writing its database into the corpus or under `HOME` instead of `--out` → AC-02's green `index` test red.
- [x] AC-04 `cargo nextest run -p specengine-eval --test anonymity` green, also with `SPECENGINE_PILOT_A` / `_B` set; this spec, 08, the eval README hold no pilot name, path, slug, raw glyph, heading or excerpt; the pilot stdout whitelists unchanged or stricter. M: a corpus path on a pilot stdout → red.
- [x] AC-05 Each D1–D4 a pilot run showed is fixed with a regression red before the fix, or an open minor with the owner's dated sign-off; the summary lists each (class, generic shape, fix or minor, test) or "none", and counts each pilot's `exclude` globs with a generic reason. M (review): a glob or narrowed root that only keeps a breaking file out.
- [x] AC-06 `cargo nextest run -p specengine-eval` passes (core, store too if their `src` changed); clippy, fmt clean; `git diff --exit-code -- Cargo.lock '*Cargo.toml'` empty, `build_graph.rs` green; `git status --porcelain -- fixtures/` only shows fixtures this task added.

Owner's checks (pilot runs):

- [x] AC-07 (owner's check) Both schemes follow the recipe; `specengine-eval check --label pilot-a|pilot-b --out <scratch>` exits 0; a scratch copy with one unknown key exits 2 at `<scheme>:<line>`. The summary records per pilot: roots, globs, prefixes, aliases (counts).
- [x] AC-08 (owner's check) The six pilot tests (three files × A, B) green, one file at a time. The summary records per pilot, dated: `index` `files`, `nodes`, `links`, `full_ms`, `one_file_ms`; `check` `files`, both verdicts, every non-zero `codes.<code>.{error, warning, debt}`; `parse` `files`, `unreadable`, `panics`, every non-zero `diagnostics.<code>`.

At shipping:

- [x] AC-09 08 §3 AC-10 carries both pilots' `full_ms`, `one_file_ms`, dated, instead of "pilot run pending"; the eval README loses both "skipped until…" clauses, states fail-not-skip and the proof, ≤ 10 020 B; 08 and this spec each < 15 737 B; `export index && check` clean; with this spec and `pointer-sweep` compacted, worst W ≤ 109 486 B.

## Implementation

Two iterations, accepted; workspace 1224 tests pass (15 skipped: pilot), eval 151; clippy, fmt clean; every named mutation red, also the env defaults'. No core, store, manifest or fixture change. Canon: eval README "CLI contract", "Pilot runs and tests"; `docs/canon/spec-check-graph.md` "Graph warnings"; 08 §2 Phase 1, §3 AC-10. AC-09: eval README 10 016 B, 08 15 635 B, check clean, worst W 109 486 B.

| Module (eval `src/`) | What it does |
|---|---|
| `harness.rs` | `resolve_file`: the flag, else the label's set variable (`PilotVariable`: the only `SPECENGINE_*` names), else the corpus-root default; a pilot-label refusal names the variable |
| `census.rs`, `parse.rs`, `index.rs`, `check.rs`, `main.rs` | census config (`census`, `parse`), scheme via `resolve_file`; help texts |

Tests: new `tests/pilot/mod.rs` (variables, `scheme_roots`, `proof`, `run_read_only`, `invented_setup`), shared by `index_cli.rs`, `check_cli.rs`, `parse_cli.rs` (invented setup, no `[paths]`, refusals; the proof, an unknown key, the census config); `census_cli.rs` (census config).

**Recipe step 2** (`scheme_roots` cites it): `roots` = the Tier 0 file, the docs root(s), each Tier 1 README outside them singly; never `.` or a directory holding a non-dot `node_modules`, a nested worktree or clone, build output or vendored Markdown.

**Owner's checks**, 2026-10-03, dev profile. AC-07: both `check` exit 0; an unknown key → exit 2 at `<copy>:45` (A) / `:10` (B). Roots A 9, B 2; `exclude` 1 each (template); prefixes A 5, B 3; aliases A 0, B 1 (U+0420); `link_base` A only; generators A 4, B none. A: one owner-accepted prefix beyond the census (decisions, by `id:`). AC-08:

| | A | B |
|---|---|---|
| files (each run); `index` nodes, links | 375; 375, 41 196 | 150; 150, 6 390 |
| `full_ms`, `one_file_ms` | 3 637, 184 | 1 373, 76 |
| `check` errors | budget 16, canon-form 18, class-unknown 98, frontmatter-type 9, frontmatter-yaml 22, homoglyph 6 | class-missing 143, frontmatter-yaml 7 |
| `check` warnings | link-anchor 8, mention-dangling 34 925, unknown-key 103 | mention-dangling 5 876, unknown-key 381 |

Both: `check` `observed` / `blocked`, debt 0; `parse` unreadable 0, panics 0, non-zero diagnostics = the check's frontmatter-*, homoglyph, unknown-key counts; `index` diagnostics = `parse` sums. Read-only: helper proofs equal, scratch `HOME`s empty, each pilot's `git status` equal before and after, owner configs untouched.

**AC-05**: D1, D2, D3 none. D4 candidate: one unresolved ID cited twice on a line gives one `mention-dangling` (A 4 399, B 364 fewer, all accounted for), the report's whole-finding dedup: intended, canon clarified (one per distinct (line, subject), `link-dangling` alike), no code change. All else is "before" debt.

**Corrected at shipping**: the proof cannot see a `.git/index` refresh; `--no-optional-locks` prevents one. The census config is required for `parse`, optional for `index` / `check`. `parse`'s `sections.differ` is a measurement, not a D4. `census` took the env default too (review minor).

## Open

- A's `one_file_ms` 184 against the 200 ms budget: the walk dominates (`noop_ms` 181).
- For `import-records`, census config: local-number tables in A; reference tables headed "document" in B; hyphenless code tables in B.
- Finding (f) in A's scheme: declared once `import-records` gives criteria `{#ID}` sections.
- Finding (h): hyphenless IDs need a reference-grammar change, a superseding ADR before B migrates; until then counted, never linked.
