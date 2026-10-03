---
class: spec
status: draft
scope: [crates/specengine-eval, crates/specengine-core, crates/specengine-store, docs]
ref: pilots analysis 2026-10-03, split row 1 (pilot-schemes), owner's answers Q1-Q9; 08 §2 Phase 1 "Pilot projects"; 08 §3 AC-10
adrs: []
---

# Pilot schemes: the reading core on both pilots

## Why

ADR-0008 makes Phase 1 exit on two pilots, yet only the census, `parse`, `ast-hash`, `ron`, the Bevy detector and `ra` have met them. The `index` and `check` pilot runs skip until the schemes gain `[paths]`, `[budgets]`, `[check]` (eval README "Pilot runs and tests"); 08 §3 AC-10's index half reads "pilot run pending". The later pilots tasks (import records, the after-tree, W on ten tasks) all stand on index and check, which have never read a real corpus.

This task gives each pilot a "before" `specengine.toml` (SpecEngine's view of the pilot as it is), makes the three pilot runs measure instead of skipping, fixes or records whatever a real corpus breaks, and puts the numbers into 08. It writes nothing into a pilot. Pilots are A and B only, in every file of this repository.

## Description and interactions

- **Schemes**: two files in the owner's pilot-config directory, outside both repositories, reached through `SPECENGINE_SCHEME_A` / `_B` (beside `SPECENGINE_CENSUS_CONFIG_A` / `_B` and, from `pilot-w`, `SPECENGINE_TASKS_A` / `_B`). The existing `[ids]`-only files are extended in place by the owner, or by an agent on his direct in-session instruction (`CLAUDE.md` "Process"), following "Data". No role writes there; their paths never enter a file here.
- **Runs**: `specengine-eval parse|index|check --label pilot-a|pilot-b --out <scratch>` (eval README "CLI contract"): `parse` reads `[ids]`, walks by the census config; `index` reads `[ids]` + `[paths]`, indexes a scratch copy into `--out`; `check` validates the whole scheme as `spec check` does, read-only. The `#[ignore]` tests of `index_cli.rs`, `check_cli.rs`, `parse_cli.rs` drive them (`--run-ignored only pilot`).
- **Helper**: each file's pilot-run helper takes corpus, scheme (and for `parse` the census config) as arguments; the `#[ignore]` tests read them from the environment, new non-ignored tests pass an invented-fixture setup. Fixture ACs decide "implemented"; pilot ACs are the owner's checks (as `mcp-read.md` AC-16).
- The owner may reproduce a verdict with `spec check --root <pilot> --config <scheme>` (no database); `spec index` / `bundle` on a pilot belong to `pilot-w`. Defects go back into the engine generically (`docs/canon/architecture.md#universal`); numbers go to 08 §3 AC-10 and this spec's summary at shipping.

## Roles

| Role | Does |
|---|---|
| owner, or an agent on his direct instruction | the two schemes per "Data"; dated sign-off of an open minor (AC-05) |
| rust-developer | `crates/specengine-eval/src` where the pilot plumbing needs it; `crates/specengine-{core,store}/src` only for a real-corpus defect |
| test-engineer | the three helpers (fail, not skip; proof; child environment), AC-01–03 tests, invented-fixture regressions, pilot runs with paths only in the environment |
| spec-writer, at shipping | 08 §3 AC-10, the eval README, the summary's counts, compaction |

## Data

### The "before" scheme recipe

One file per pilot, the ordinary closed top level of `specengine.toml` (no `[import]`, no census keys). Invented values; the alias is the `spec-b` fixture's, as TOML `\u` escapes:

```toml
[project]
slug = "pilot-a"
[paths]
roots      = ["AGENTS.md", "docs", "crates/engine/README.md"]
tier0      = "AGENTS.md"
tier1_name = "README.md"
index      = "docs/index.md"
exclude    = ["**/_*.md"]
link_base  = "docs"
[ids]
REQ  = { kind = "requirement", width = 3, aliases_from = ["\u0422\u0420\u0411"] }
TERM = { kind = "term", shape = "name" }
[budgets]
tier0_bytes = 16384
tier1_bytes = 10240
index_bytes = 10240
decision_bytes = 1536
[classes]
spec = { required = ["class", "status", "scope"], optional = ["ref", "shipped", "adrs"] }
[[generators]]
command = "make docs-map"
writes  = ["docs/generated/map.md"]
[check]
mode = "observe"
```

1. **`[project] slug`** = the label, `pilot-a` / `pilot-b`; no `name`. The slug names the database and is printed (`indexed <slug>`); a pilot-derived one is a leak `anonymity.rs` cannot see (it matches directory names only).
2. **`roots`**: from a count of `.md` files per top-level directory (in the owner's terminal) take the Tier 0 file, the docs root(s) and each Tier 1 README outside them as a single-file root. Never `.`; never a directory holding, at any depth, a non-dot `node_modules`, a non-dot nested worktree or clone, a build directory or vendored Markdown: list the READMEs beside it singly. `exclude` filters files after the walk, the subtree is still listed (cost; an unlistable directory → cannot check). Each root exists, no symlink on its path (else `missing_roots`; store README "Walk").
3. **`tier0`, `tier1_name`, `index`**: as the pilot's own reading protocol names them; no counterpart → omitted, the rule off.
4. **`exclude`**: templates and non-documentation Markdown inside a chosen root; each glob has a generic reason (template, vendored, generated copy, archive copy), counted in the summary.
5. **`link_base`**: set to its docs root by the pilot whose broken file links resolve from there (finding (a), 08 §4.3); not by the other, whose dangling wiki targets are genuine debt (finding (b)).
6. **`[ids]`**: one entry per Latin prefix of the census `labels.json` (under the owner's census `--out`): `kind` free, `width` the digit count the corpus mostly writes, `shape = "name"` for name-shaped IDs. Each non-Latin legacy prefix the census reports (A 20 IDs, B 287, B's in one decision register, U+0420 for P) goes into `aliases_from` of the Latin prefix it stands for. Not declared: `scope = "feature"`, finding (f)'s criteria prefix (U+041A), finding (h)'s hyphenless IDs (A4).
7. **`[budgets]`**: the pilot's own caps where stated, else the four §4 caps; `canon_bytes` only if it caps Tier 2.
8. **`[classes]`**: the contract the pilot's front-matter follows; `closed = true` only where its own check closes a class. A document without `class` gives `class-missing`: "before" debt (finding (g): the importer normalises).
9. **`[[generators]]`**: one entry per distinct `generator:` value of the pilot's `class: generated` documents, `writes` their paths; none → omitted (§11.5–6 off). Never `index = true`: §11.5 would compare the pilot's own index with SpecEngine's render, `index-drift` by construction. Never run (ADR-0013).
10. **`[check] mode = "observe"`**, no `[[check.rules]]`. A scheme error: `<scheme>:<line>: message`, exit 2.

### Read-only proof and child environment

`proof(corpus, roots)` = (a) stdout of `git --no-optional-locks status --porcelain --untracked-files=all` in the corpus (no git worktree → the helper fails); (b) the sorted (root-relative path, kind, size, mtime ns) of every entry under each scheme root, directories included, symlinks not followed. Taken before and after, equal. It replaces the content snapshot of `index_cli.rs` / `check_cli.rs`, which reads every byte of the corpus, build directories included, and follows symlinked directories. The child gets `HOME` = an empty `<scratch>/home`, no `XDG_DATA_HOME`, `--out <scratch>/out`, `--timeout 600`.

## Rules and edge cases

- WHEN a pilot test runs with a corpus, scheme or census-config variable unset or empty, it SHALL fail naming it (as today). WHEN the scheme has no `[paths]` table, each helper SHALL fail naming the scheme variable; none skips.
- WHEN a run ends, the helper SHALL fail unless: exit 0; stdout passes the file's whitelist (`assert_anonymous`); the proof is equal; `<scratch>/home` is empty; `index` `missing_roots` 0, `one_file_parsed` 1, `full_ms` ≤ 10 000, `one_file_ms` ≤ 200; `check` `files` > 0, `verdicts.observe` `clean` or `observed`; `parse` `files` > 0, `panics` 0.
- **Defects**: D1 a panic (caught or not), crash, exit 1; D2 exit 2 or `cannot-check` not traced to the scheme; D3 a missed AC-10 budget or `"timeout"`; D4 a finding or count contradicting the canon rule for its bytes, or two measurements disagreeing over one walk. WHEN a pilot run shows one, the task SHALL fix it with a regression on an invented convention, red before the fix, or record it as an open minor in the owning README with the owner's dated sign-off. A finding true under the convention is "before" debt: counted, never "fixed".
- WHEN a regression needs a pilot's shape, it SHALL be re-invented: no pilot path, name, ID, heading, cell or excerpt; non-Latin text from `\u{…}` escapes at test time (no new `anonymity.rs` exemption); the genre checks stay green.
- Nothing is written into a pilot: no file, no git index refresh, no marker (ADR-0016), no build or generator run. Pilot tests stay `#[ignore]`, never in CI; one file at a time on the owner's laptop.
- The repository receives counts, generic shapes and code points only: `anonymity.rs` scans `docs/`, `crates/`, `fixtures/` for absolute paths, raw Cyrillic and (with `SPECENGINE_PILOT_A` / `_B`) directory names, nothing else.

## Assumptions

- A1 Phase 0's `[ids]`-only schemes exist and are extended in place; the census configs are unchanged (`import-records` extends them).
- A2 "Before" = the pilot as it is; the after-config is `import-layout`'s draft under `--out`.
- A3 Observe: pilots are measured, not gated; `observed` is the expected verdict.
- A4 Prefixes from the census only. Finding (f) would need a feature-scoped Latin `AC` defined by `{#ID}` sections; A's criteria are list items, so every cite would dangle (9 525 from A's Tier 0): deferred to `import-records`. Finding (h)'s IDs are not recognisable by the reference grammar (model README). A legacy prefix's Latin target is the owner's call, default the census's normalised prefix.
- A5 Timings as `cargo nextest` builds the harness (dev profile), stated with the date.

## Risks

- **Wide roots** (B: a non-dot `node_modules` full of READMEs; A: nested worktrees, build directories) → narrow roots (step 2).
- **Hidden writes**: `git status` refreshing the index file; `<slug>.db` in the real data directory (CLI README "Database") → the proof, the scratch `HOME`.
- **Leaks beyond the test** (slug, display name, glyph, `generator:` command, excerpt) → slug = label, code points, re-invented regressions.
- **Bending the core to a pilot** → generic fixes, two invented conventions where a rule is new.
- **Live corpora** → numbers dated, compared same-day. **Migration creep** (08 §5) → read-only runs, counts only.

## Acceptance criteria

Fixture ACs (decide "implemented"):

- [ ] AC-01 Setup: a temp copy of `fixtures/spec-b` (`git init -q`, nothing committed), a test-written scheme with every recipe table and key of the example (one `aliases_from`, one `[[generators]]` without `index`), a test-written census config. `parse`, `index`, `check` with `--label pilot-a` and `SPECENGINE_PILOT_A`, `SPECENGINE_SCHEME_A`, `SPECENGINE_CENSUS_CONFIG_A` set on the child each exit 0; `check` `verdicts.observe` `clean` or `observed`. M: the `index` scheme reader rejecting a table other than `[ids]` / `[paths]` → red.
- [ ] AC-02 In each of the three files a non-ignored test runs the helper on the AC-01 setup (green) and with `[paths]` removed (`#[should_panic]`, the message naming the scheme variable); the `#[ignore]` tests call the same helper; `grep -n -e 'has no \[paths\]' -e informational crates/specengine-eval/tests/{index,check}_cli.rs` prints nothing. M1: the skip restored → red. M2: a `panic!` injected into the front-matter reader → the green `parse` test red.
- [ ] AC-03 The helpers take the proof and set the child environment. Proof unit tests: `File::set_modified` +1 s on a file under a root between the proofs → unequal; a file created at the corpus root outside every root → unequal. M: `index` writing its database into the corpus or under `HOME` instead of `--out` → AC-02's green `index` test red.
- [ ] AC-04 `cargo nextest run -p specengine-eval --test anonymity` green, also with `SPECENGINE_PILOT_A` / `_B` set; this spec, 08, the eval README hold no pilot name, path, slug, raw glyph, heading or excerpt; the pilot stdout whitelists unchanged or stricter. M: a corpus path on a pilot stdout → red.
- [ ] AC-05 Each D1–D4 a pilot run showed is fixed with a regression red before the fix, or an open minor with the owner's dated sign-off; the summary lists each (class, generic shape, fix or minor, test) or "none", and counts each pilot's `exclude` globs with a generic reason. M (review): a glob or narrowed root that only keeps a breaking file out.
- [ ] AC-06 `cargo nextest run -p specengine-eval` passes (core, store too if their `src` changed); clippy, fmt clean; `git diff --exit-code -- Cargo.lock '*Cargo.toml'` empty, `build_graph.rs` green; `git status --porcelain -- fixtures/` only shows fixtures this task added.

Owner's checks (pilot runs):

- [ ] AC-07 (owner's check) Both schemes follow the recipe; `specengine-eval check --label pilot-a|pilot-b --out <scratch>` exits 0; a scratch copy with one unknown key exits 2 at `<scheme>:<line>`. The summary records per pilot: roots, globs, prefixes, aliases (counts).
- [ ] AC-08 (owner's check) The six pilot tests (three files × A, B) green, one file at a time. The summary records per pilot, dated: `index` `files`, `nodes`, `links`, `full_ms`, `one_file_ms`; `check` `files`, both verdicts, every non-zero `codes.<code>.{error, warning, debt}`; `parse` `files`, `unreadable`, `panics`, every non-zero `diagnostics.<code>`.

At shipping:

- [ ] AC-09 08 §3 AC-10 carries both pilots' `full_ms`, `one_file_ms`, dated, instead of "pilot run pending"; the eval README loses both "skipped until…" clauses, states fail-not-skip and the proof, ≤ 10 020 B; 08 and this spec each < 15 737 B; `export index && check` clean; with this spec and `pointer-sweep` compacted, worst W ≤ 109 486 B.

## Out of scope

Importer code, the census config's extension (`import-records`); the after-tree, emitted config and baseline (`import-layout`); W, task lists, `spec index` / `bundle` on a pilot (`pilot-w`); rule parity with the pilots' own checks (migration); `spec init --import`; any write into a pilot; MCP, hook, CI.

## Open

- Finding (f) in A's scheme: declared once `import-records` gives criteria `{#ID}` sections (A4).
- Finding (h): honouring ADR-0009's legacy aliases for hyphenless IDs needs a reference-grammar change, a superseding ADR before B migrates; until then counted, never linked.

## Implementation

Filled in after implementation: a "module — what it does" table over the files actually changed, the per-pilot counts (AC-07, AC-08), the defects (AC-05). Deliberate deviations go here, with the reason.
