---
class: spec
status: shipped
scope: [crates/specengine-core]
ref: 08 §2 Phase 1 Next (check increment 4); docs/canon/spec-check.md "Not checked yet"; owner's answers Q1-Q7, 2026-10-02
shipped: 2026-10-02
adrs: [ADR-0031]
---

# spec check: process rules

## Why

Records lose parts the owner and agents rely on, unnoticed: a decision's cost (mandatory by `docs/decisions/_template.md`); a question's addressee and working answer (rounds go by both, 06 §5; agents work on the working answer, ADR-0012; the bundle's open-questions layer reads `status:`, `working_answer`); a shipped spec's "Implementation" (06 §8); a record's own wording, not just a link into the archive (documentation-system §5). All are document facts Phase 1 holds. The core must not learn these words (ADR-0008): it gets generic rules, each project names its vocabulary in `specengine.toml` (ADR-0031, settling core Q2). These are the pilots' remaining checks (`docs/canon/spec-check.md` Q-8), parity measured at migration (ADR-0013).

How it works now: `docs/canon/spec-check-process.md`.

## Acceptance criteria

Scratch copies of `fixtures/spec-a`, `-b` and this repository, scratch configs and `HOME`. M: the mutation that must turn it red.

- [x] AC-01 Each malformed rule (canon "Config errors") → `specengine.toml:<line>: message`, exit 2 under each mode. With valid rules, `spec show`, `tree`, `bundle` answer, exit 0. M: the declared-kind check dropped; a selector-less rule accepted.
- [x] AC-02 No rules → `spec check` lines and `--json` on spec-a, spec-b, this repository byte-identical to before; `fixtures/*/expected.json`, `INDEX_FORMAT` 6 unchanged. M: a built-in rule.
- [x] AC-03 spec-a, `kinds=["question"]`, `keys=["to","working_answer"]` → clean; Q-031 without `to:` → `key-missing`, line 1, subject `to`; `to: ""` or `to:` → `key-empty` at its line; A-102 (declared `requirement`, prefix `assumption`) judged by a `["requirement"]` rule, not `["assumption"]`. M: blank as present; prefix kind only.
- [x] AC-04 `values={to=["customer","owner","team"]}`: `to: Owner` → `value-invalid` at its line listing the three; `to: [owner, x]` → one, for `x`; `to: 3` → one, "not a string"; absent → none. M: case-folded compare; absent flagged.
- [x] AC-05 `when={status="open"}`: Q-032 (`answered`) without `to` → nothing; Q-031 without `to` → `key-missing`; `status=["open","answered"]` selects both; `status: 1` never. M: `when` ignored; prefix match.
- [x] AC-06 `parts=["Implementation"]`: text under `## Implementation` → clean; only whitespace, CRLF, an HTML comment → `part-empty` at the heading; none → `part-missing`, line 1; text only under a nested `### x` → clean; only after the next `##` → `part-empty`; `### Implementation` matches. M: content to end of file; comments counted.
- [x] AC-07 `parts=["Cost"]`: `**Cost.** We pay X.` → clean; `**Cost.**`, a blank line, a paragraph → `part-empty`; `**Cost:** x`, `__Cost__ x`, `## Cost` match; only in a fenced or HTML block, or mid-line `x **Cost.** y` → `part-missing`; Q-031's `Working answer` (line 2 of a paragraph) found, filled. M: a line regex; labels unslugged.
- [x] AC-08 A config under `fixtures/spec-b/` labels `dry-run.md`'s Cyrillic criteria heading: clean; the section emptied in a copy → `part-empty`; `anonymity.rs` green; `expected.json` unchanged. M: ASCII-only slug.
- [x] AC-09 `text=true`: a body of only `[x](archive/r.md)`, headings, `R-12, A-101`, `[[R-12]]`, `1. [x](a.md)`, or nothing → `text-empty`, line 1, subject the ID (`""` without); "Regeneration waits for R-12." → clean; text only in nested `{#ID}` sections → `text-empty`. M: link text, nested sections or list markers counted.
- [x] AC-10 spec-b `QN-08` (failed front-matter) and a `class: generated` file selected → no finding; a `status: shipped` spec judged; a `CheckFile` with empty `bytes` → no `part-*`, `text-empty`. M: the failed-front-matter skip removed.
- [x] AC-11 `severity="warning"` never blocks, counts in `warnings`, prints only with detail; the default blocks under `enforce`. Git repo, `enforce-introduced`, `--changed`: a violation at `HEAD` → `(pre-existing)`, not blocking; a new one blocks; a commit adding only a rule passes. M: the base judged without rules; `severity` ignored.
- [x] AC-12 Debt `(part-missing, <path>, Implementation)` → `debt` until `expires`, then blocks; a moved line still matches. M: subject from the message or line.
- [x] AC-13 Shuffled files, permuted rules → byte-identical output; one finding from two rules → once, an error over a warning. M: rule order in output.
- [x] AC-14 The rules sources (module, `[[check.rules]]` parsing) hold no string literal equal to an `[ids]` kind of spec-a, spec-b or the root config (class names excepted), a key or label of this task's test configs, or a `PROJECT_NAMES` entry; `check_genre.rs` pins unchanged. M: a `"Cost"` or `"question"` literal.
- [x] AC-15 spec-a, `DOM-GAME` with `parent: MEC-STAMINA` → one `parent-cycle` at `docs/spec/game.md`'s `parent:` line, subject `DOM-GAME, DOM-MOVEMENT, MEC-STAMINA`, the members of `spec tree`'s warning on the copy; a self-parent → one; never blocks; fixtures and this repository → none. M: severity error; another member placed.
- [x] AC-16 Root rules (canon "This repository") on: `export index && check` → `clean`, 0 errors, warnings, debt; `check_parity.rs` PIN `[]`. In a copy: ADR-0030 without `**Cost.**` → one `part-missing`; a shipped spec's Implementation emptied → one `part-empty`. The fixture-style config on spec-a → clean. M: the loader dropping rules.
- [x] AC-17 At shipping: worst W ≤ 112 829 B; 08 ≤ 15 737 B; this spec < 15 737 B; the new canon ≤ 12 288 B; Tier 1 READMEs ≤ 10 240 B. (Measurement.)
- [x] AC-18 `--json` keys unchanged; `CHECK_CODES` grows by exactly the six, in the `check_output.rs` code table. M: a code emitted, not listed.

## Implementation

Canon: `docs/canon/spec-check-process.md` (new); `spec-check.md` (increments 1–4, 35 codes), `-graph.md`, `-links.md`, `-cli.md` (the cannot-check mode worded), `-git.md`, `spec-cli-graph.md`; `docs/README.md` "Enforcement"; core and store READMEs; 05 §2.2, 08 Phase 1; the root rules on in `specengine.toml`. Three iterations, each accepted; 950 of 950 tests, clippy and fmt clean, every named mutation red. AC-17: worst W 112 815 B (was 112 823), 08 15 719, this spec 8 294, the canon 10 507, largest Tier 1 README 10 020 (CLI).

| Module | What it does |
|---|---|
| core `check/rules.rs` (new) | selection, `keys`, `values`, `parts`, `text`; lazy reads; equal findings merged, error over warning |
| core `check/rules_toml.rs` (new) | `CheckRule`, `rules_from`: parsing, every config error |
| core `own_text.rs` (new), `lib.rs` | `own_spans`, the one own-text split |
| core `markdown.rs` | one reader `options()` for the parse and `outline()` (labels, text runs) |
| core `check/{config,mod,engine,text}.rs` | `CheckConfig.rules`; six `CHECK_CODES`; `rules::run`, `parent_cycles` calls; `written_values` |
| core `check/{graph,spec_graph,resolve}.rs` | `parent_cycles` over `SpecGraph::tree_only` (parents only) |
| store `rows.rs` | `own_text` joins `own_spans` |

Tests: core `check_rules.rs`, `common/rules.rs` (new), `check_genre.rs` (the rules sources), `check_output.rs` (35 codes); CLI `check_rules.rs` (new), `tree.rs`; store `check_loader.rs` (config errors judged in mode `enforce`); `fixtures/spec-b/check-rules.toml` (the Cyrillic label).

Deviations from the draft, now canon: blank key names are config errors; non-string `values` items give one finding; any `[[…]]` on one line is a wiki link, code included; code text fills; a lead-in ends at the next line-opening one of any label, never in links, images, headings, table cells, `***x***`; without bytes keys come from the parse; `export index` refuses a malformed rule; `parent-cycle` needs no rule. Iteration 2 read the cannot-check mode alone (`[observe]` for a broken config); iteration 3 reverted it to `spec-check-cli.md`'s contract. Iteration 2 also built the parents-only graph (5 000 documents ~340 → ~255 ms) and bounded wiki links to the line (180 → 14 ms). Under `enforce-introduced` a pre-existing violation prints only with detail. Residue: canon "Open".
