---
class: spec
status: shipped
scope: [crates/specengine-eval, crates, ui, docs]
ref: 9a-3 health-ui-follow-up.md U7 (its Out of scope, the other comments whose headings go unread); requirement analysis 2026-10-19 against 11eeaed, C1-C8 owner pre-approved 2026-10-19, review open; docs/README.md Enforcement; tests 2 iterations; review 2026-10-19 — changes required (doc_pointers.rs:12), then accepted
shipped: 2026-10-19
adrs: []
---

# citation-sweep: every comment citation shape read, the stale ones fixed

## Why

`crates/specengine-eval/tests/doc_pointers.rs` guards the pointers from code comments into the documents (`pointer-sweep.md` AC-04). It read two shapes, `` `path` "H" `` and the CLI's `` canon `name`, "H" ``. Most comments are written otherwise: a bare `docs/…md "H"`, a backticked path with a comma, a heading opening the next comment line, a `.css` comment, a path with no heading. None was checked, and about a hundred went stale: shipped specs folded their headings into bold run-in labels (`ui-graph.md`, `ui-home.md`) or moved the rule to the canon (`ui/README.md` "Screen rules"). 9a-3 fixed one (its UI-10) and left the rest here.

Now the test reads every shape in comment text, checks that a cited documentation path names a file, walks CSS and the `Cargo.toml` comments, and the stale lines are fixed, canon first. It stays a repository test, never a `spec check` rule: the hook and CI run only `spec check` (ADR-0012), nothing reaches core (ADR-0008). No ADR: the convention is ADR-0022's.

## Description and interactions

How it works now: `docs/README.md` "Enforcement" (Code pointers; shapes C1, wrapping C2, path existence C3, the walk C4, anchors unresolved C6, comment text only C7); the test: `crates/specengine-eval/README.md` "Pilot runs and tests". Retargets went canon first (C5); a citation follows the document: no canon or shipped-spec heading was renamed or added. `spec check`, the hook, CI, `specengine.toml` and `Cargo.lock` are unchanged.

## Acceptance criteria

Verifier, unless named: `cargo nextest run -p specengine-eval --test doc_pointers`. M: the mutation that turned it red (all red; Implementation).

- [x] AC-01 — a bare citation on a `/** … */` line is read. M: `ui/src/app/shortcuts.tsx:17` back to `docs/features/ui-home.md "Keys"`, red naming the file, line and heading.
- [x] AC-02 — bare Rust citations are read, with and without a comma. M: `crates/specengine-core/src/check/engine.rs:1` "Rules" → "Rule"; M: `crates/specengine-cli/tests/proposal_moved.rs:1`'s "Reject" restored.
- [x] AC-03 — a backticked citation with a comma is read. M: `crates/specengine-core/src/check/text.rs:121` "A rule" → "A rules".
- [x] AC-04 — a heading on the next comment line is read. M: `crates/specengine-store/src/symbols.rs:2` "Lookup" → "Lookups" (the finding names the heading's line, `:2`).
- [x] AC-05 — CSS is walked. M: `ui/src/styles/app.css:1020` "UI" → "UI rules".
- [x] AC-06 — a documentation path names an existing file. M: `ui/src/graph/GraphView.tsx:134` `ui-graph.md` → `ui-graphs.md`, red "no file". Six fixture-data lines pass (five `stamina-tuning.md`, `fixtures/spec-a`; `cli/tests/rev_rule.rs:274` `cap-check.md`, `fixtures/code-a`). M: the allowlist emptied, red on its four lines only; M: a dead entry added, red.
- [x] AC-07 — grammar self-test: a `.rs`, a `.css` and a `Cargo.toml` text with every read shape and every negative case (a string-literal path, `docs/spec/a.md "X"`, `AC-13 ("…")`, a quote after prose, `"A" "B"` reading A only, an `#anchor`, a CSS continuation line, a `*`-led line outside a block (`*x = …`), a wrapped heading) yield an exact list. M: code lines read too; M: `/**` taken as code.
- [x] AC-08 — floors: the run prints its counts and asserts floors with slack (C8, Implementation). M: the bare reader returns nothing.
- [x] AC-09 — end state green: `--test doc_pointers --test ui_policy --test anonymity` 44/44. HEAD's 94 changed code files replayed: 101 missing headings on 98 lines (43 `ui/src`, 4 `crates/*/src`, `Cargo.toml:7`, 50 tests), 25 wrapped headings, 0 missing files. M: `ui/src/graph/layout.ts:3`'s "Layout" restored.
- [x] AC-10 — print nothing: `rg -n 'features/decision-staging\.md`? "(Documents|Daemon|Terminal|Staleness|Backup)"' crates ui/src` and `rg -n 'features/ui-graph\.md`? "(Canvas|States)"' ui/src`.
- [x] AC-11 — behaviour unchanged: in `ui/`, `pnpm lint` 0, `build` and `test` pass; the full `cargo nextest run` once at the end; `git diff --exit-code -- Cargo.lock` empty; `build_graph` green; `git diff -U0 -- '*.rs' '*.ts' '*.tsx' '*.css' '*.js' '*Cargo.toml' ':!crates/specengine-eval/tests/doc_pointers.rs' | grep '^[-+][^-+]' | grep -vE '^[-+][[:space:]]*(//|/\*|\*|#)'` prints nothing.
- [x] AC-12 — `rg -n 'bare|\.css' docs/README.md` hits the "Code pointers" bullet only; `spec export index` and `spec check` clean; `docs/README.md` ≤ 10 240 B.

## Out of scope

Moving valid shipped-spec citations to the canon; resolving `#anchor`s; section-number citations, unquoted labels; comments in `scripts/`, `.githooks/`, `.github/`, `plugin/`, `specengine.toml`, `fixtures/`; a `spec check` rule.

Known limits (unread, valid today): the CLI form with `.md` (`` canon `spec-check-git.md`, "The base" ``, `core/src/patch.rs:23`; `store/src/queue/state.rs:26`); relative shorthand outside the roots (`mcp/tests/mcp_code.rs:100`, `store/tests/format.rs:566`); headings after prose (`stage_confirm.rs:1-3` fixed by hand); trailing comments after code, JSX `{/* … */}`. A `//` line inside a Rust raw string is read.

## Implementation

Tests 2 iterations. Iteration 1: 13 of 13 mutations red. Iteration 2 (review 1: the floors with slack, C8; a heading reported at its own line): AC-08, AC-04, AC-07 ×2 re-proved red, and a new one, a leading `*` read outside a block again, red.

| Module | What it does |
|---|---|
| `crates/specengine-eval/tests/doc_pointers.rs` | `cited_files()` the walk (C4); `comment_lines()` per style (slashes, CSS, `#`); the path reader over `DOC_ROOTS`; `names_a_file` matches each component exactly (`read_dir`, macOS folds case); `DELIBERATE_MISSES`, `FLOORS`; tests: headings verbatim on one line, documentation paths name a file, floors, the grammar self-test (samples on `concat!` code lines), the roots (11 in, 15 out), the CLI form; the four older tests unchanged |
| 51 files under `crates/*/tests` | comment lines: retargets, list rewrites, 19 rewraps, 4 split paths |
| 35 files under `ui/src` | 43 lines, canon first; `labels.ts`, `daemonKeys.test.ts` rewrapped |
| `crates/*/src`, `Cargo.toml`, `crates/specengine-core/Cargo.toml` | 4 lines (`project_toml.rs`, `code.rs`, `mirror.rs`, `source.rs`); `start.rs`, `schema.rs` and `Cargo.toml:18` rewrapped; `Cargo.toml:7` |

Floors (AC-08), C8 (owner pre-approved 2026-10-19, review open): with slack, about 90 % of the measured count rounded down; the old three keep their meaning. Measured → asserted: quoted headings 1235 → 1111 (and the old ≥ 23); bare-form headings in `ui/src` 172 → 154, in `crates` 253 → 227; backticked 699 → 629; first heading on the next line 71 → 63; `.css` 4 → 3; `Cargo.toml` 9 → 8; distinct path-only files 122 → 109; `provisional.ts` 147 → 20; CLI form in the CLI's sources 76 → 24.

Deviations:

1. Roots: `docs/README.md` and `docs/index*.md`, not `docs/*.md`: 13 temporary-corpus names (`docs/a.md`, `docs/guide.md`, `mcp/src/resources.rs:225`, …) would each need an allowlist entry.
2. The allowlist holds 4 deliberate `docs/features/` names, not 1: `feat.md` (`check_mentions.rs`), `c.md` (`check_scopes.rs`), `regen-delay.md` (`kind_amendment.rs`, `mcp_door.rs`).
3. One stale line beyond the analysis: `mcp/tests/plugin_files.rs:142`.
4. `GraphCanvas.tsx:288`, `geometry.ts:3`, `geometry.test.ts:5` cite `docs/features/ui-graph.md "Implementation", Canvas`: only that spec states the fit and the arcs.
5. Retargets past the C5 table, each target checked: e.g. "Idempotence" → `proposal-completion.md` "Completion", "Backup" → `queue-backup.md` "Format", daemon-read's "SSE" and "Query" → `crates/specengine-http/README.md` "Live tail", "Endpoints".

The 08 s2 frozen files were walked: no stale citation there, nothing allowlisted for them.
