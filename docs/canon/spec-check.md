---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-eval]
owner: owner
reviewed: 2026-10-02
---

# spec check: what it enforces

Increments 1–4: one check of the convention — §11.1–4 of `docs/canon/documentation-system.md`, the ADR-0009 ID checks, an expiring debt baseline, the project's process rules (`docs/canon/spec-check-process.md`) — driven only by `specengine.toml`; the source names no prefix, path or file of a project (`#universal`). Engine `specengine_core::check::run` (pure), loader `specengine_store::load_check` (fresh parse), commands: `docs/canon/spec-check-cli.md`, measurement `specengine-eval check` (APIs: their READMEs). Nothing is written, no status or flag set (`#control`, `#apply`): the homoglyph fix is data for `apply_proposal`. It gates this repository (`docs/README.md` "Enforcement").

## Engine types (`specengine_core::check`)

- `CheckInput {files: [CheckFile {path, size, parsed?, read_error?, bytes}], problems: [Problem {kind: MissingRoot | UnreadableDir | SkippedName, path}]}`, `CheckFile::{parse, parsed, unreadable}`. `bytes` (the parsed text) give real lines, a parser diagnostic's subject and the written keys (`title`, `kind`, `id` are not in `fields`).
- `CheckConfig::from_toml` → `{budgets: Budgets {tier0_bytes, tier1_bytes, index_bytes, decision_bytes, canon_bytes?, bundle_node?, bundle_task?}, classes: Classes {canon, decision, spec, generated: ClassContract {required, optional, closed}}, mode: Mode, rules: [CheckRule]}`; `Baseline::from_toml` → `[DebtEntry {code, path, subject, reason, expires, line}]`; both errors `{line?, message}`, `at(file)` → `file:line: message`.
- `Report {mode, verdict, counts, findings, stale, new_debt?: [NewDebt {DebtEntry, head_expires?}], cannot_check: [Cause {path, message}]}`, `lines(detail)`, `to_json()`, `exit_code()`, `verdict_in(mode)`, `without_base()` (`enforce-introduced` → `enforce`), `cannot(mode, causes)`, `Finding::blocks_in(mode)`; `CHECK_CODES` (35), `PARSER_SEVERITY` (13 rows); `worst_w(&CheckInput, &Paths, Option<&Generator>) -> u64`.
- `judge(report, &Baseline, &Base {findings, baseline?, mode?}) -> Report`, pure (`docs/canon/spec-check-git.md` "The base"): a finding is `introduced` unless its (code, path, subject) is a base finding (by set); new debt: an entry the base's baseline lacks by triple or holds with an earlier `expires` (not an earlier one, a new `reason`, a removal; `None`: unjudged); the stricter mode.

## Configuration

`[paths]` gains `tier0` (the one file canon tier 0 may be), `tier1_name` (the only name a canon tier 1 file may have) and `index`; each rule is off while its key is absent. This repository's root `specengine.toml`, abridged (Q-7):

```toml
[paths]
records = "docs/decisions"
tier0 = "CLAUDE.md"
tier1_name = "README.md"
index = "docs/index.md"
exclude = ["**/_*.md", "**/fixtures/**"]   # + target, target.noindex, node_modules, dist
[ids]
ADR = { kind = "decision", width = 4 }
[budgets]                    # bytes of the whole file, BOM and front-matter included
tier0_bytes = 16384
tier1_bytes = 10240
index_bytes = 10240
decision_bytes = 1536
canon_bytes = 12288
[classes]
decision = { required = ["class", "id", "title", "status", "date", "scope"], optional = ["canon", "supersedes", "ref"], closed = true }
[check]
mode = "enforce"             # observe | enforce-introduced | enforce (default)
```

Defaults: the four §4 caps above, `canon_bytes` none; `bundle_node` (≤ 4294967295, `u32::MAX` as `spec bundle` reads it), `bundle_task` accepted (tokens). Default contracts, open: canon `owner`, `reviewed`; decision `id`, `status`, `scope`; spec `status`, `scope`; generated nothing; each plus `class`. `closed = true` admits only `class`, `required`, `optional`. A written class replaces its default whole: an omitted `required` or `optional` is empty, `closed` false, so `canon = { closed = true }` admits only `class`. An unknown key or class, a wrong type, a cap out of range, an unknown mode → `specengine.toml:<line>: message`; the run cannot check.

## Rules

- **Front-matter fails** (`not-utf8`, `frontmatter-unclosed`, `-yaml`, `-not-mapping`): the file gives only its parser findings, and its body the graph warnings (with `frontmatter-unclosed`, the whole file: `docs/canon/spec-check-graph.md`).
- **Class.** Every document declares one (owner, Q-4): none, or no front-matter → `class-missing`, no contract and no class cap, while IDs, references and `canon:` are still checked; not one of the four → `class-unknown`. Per class: `key-missing`, `key-extra` (closed), `scope-empty`, `date-invalid` (`reviewed`, `date`, `shipped` not shaped `YYYY-MM-DD`), `status-invalid` (spec draft | in-progress | shipped | abandoned; decision accepted | rejected | `superseded-by <ID>`), `shipped-missing`, `canon-missing` (accepted decision: `canon:` absent, blank or unreadable), `tier-invalid` (canon tier not 0–2; `tier: 0` off `tier0`; `tier0` not tier 0, the only rule on `tier0`; `tier: 1` on a file not named `tier1_name`).
- **IDs.** A number-shape definition (`id:`, `{#ID}`; references never) has `width` digits, else `id-width`. Mixed script → `homoglyph`, an error with its Latin `fix`. An ID defined in two files → `id-taken` on each later file by path, naming the first; a feature-scoped ID is unique per feature, misplaced → `id-scope` (`docs/canon/spec-check-links.md`). Under `records`, `id: X` names its file `X` + `.` or `-`, else `file-name` (a bare prefix is not enough).
- **References** — `supersedes`, `status: superseded-by`, `adrs`, `refs`, `working_answer`, `parent`, `links.*`, a reference-form `canon:` — resolve: the ID is defined; or the text is in a document's `aliases:`; or, through `aliases_from`, the configured prefix + the written body is defined (no re-padding); `#Y` is defined in the ID's file. Else `ref-dangling`. An alias `parent` is re-read through the scheme. Where an ID may resolve (`slug/`, bare feature-scoped IDs): `docs/canon/spec-check-links.md`. Not yet: `project:`, `@rev`.
- **Path-form `canon:`**, on any document: has `#` (`canon-form`), names a walked canon document (`canon-file`), and one of its anchors or section IDs (`canon-anchor`). Anchors: heading slugs, `{#…}`, `<a id>`, `<a name>` (`crates/specengine-model/README.md`); a start tag split over lines in an HTML block or a blockquote is missed.
- **Budgets** (`budget`, subject = the slot): whole-file bytes > cap. `index`, any class: the root and each live shard (`docs/canon/spec-check-graph.md#index-shards`); canon tier 0 → `tier0_bytes`, tier 1 → `tier1_bytes`, else `canon_bytes`; decision → `decision_bytes`; spec, generated, class-less: none.
- **Walk.** Non-UTF-8 names → one warning `name-skipped` per problem path, its message counting them; a missing written root, an unreadable file or directory → cannot check; a missing default role root is ignored.
- **Parser codes** pass as themselves through one table, `PARSER_SEVERITY`, with the parser's severity but `homoglyph`, `duplicate-id` (errors).

The check's own codes (`CHECK_CODES`, 35) are the errors named above, `name-skipped`, the seven of `docs/canon/spec-check-graph.md` (§11.5–6, inline mentions, graph), the two file-link warnings of `docs/canon/spec-check-links.md` and the six of `docs/canon/spec-check-process.md`.

## Findings, debt, verdict

`Finding {code, severity, path, line, subject, message, fix?: {span, text}, debt?: {reason, expires, expired}, introduced?}`; `subject` = the object as written (ID, key, `canon:` value, budget slot `tier0|tier1|index|decision|canon`, a parser diagnostic's span text), `""` for the whole file (line 1). A spanless `unknown-key` or `frontmatter-type` takes the written top-level key whose entry holds its line (after `!tag`/`&anchor`; `a` for `? a`); a flow key (`[a, b]:`), an `*alias` line, a non-scalar `? [q]` or no bytes → `""`, like every other spanless finding. The class contract reads the same keys. A finding identical in every field is reported once.

**Baseline** `.spec-debt.toml` (the root's unless one is passed; read only):

```toml
[[debt]]
code = "ref-dangling"
path = "docs/specs/specengine-platform/README.md"
subject = "ADR-0015"   # default ""
reason = "legacy import"
expires = "2026-12-31"
```

An entry matches every finding with its (code, path, subject), never by line. A match is debt until `today > expires`; then an error blocks again (counted `expired`) and a warning stays a warning. An entry matching nothing goes to `Report.stale` (detail label `debt-stale`), counted `stale`, never a finding. A missing `reason` or `expires`, a bad date, an unknown key, a repeated triple, bad TOML → cannot check.

**Verdict.** `observe` never blocks; `enforce` blocks on errors not in live debt; `enforce-introduced` on errors with expired debt or no debt and introduced (no base: = `enforce`); both on new debt; warnings, live debt, stale entries never block. `clean` → exit 0, `observed` (errors or new debt left, none blocking: `observe`, or only pre-existing) → 0, `blocked` → 1, `cannot-check` → 2, winning in any mode (a missing written root, an unreadable file or directory, an invalid config, baseline or `today`). Counts: `errors`, `warnings` exclude live debt; `expired` and `introduced` (with a base) are part of `errors`.

**Output**, sorted by (path, line, code, subject, message) whatever the input order. Lines: `error  path:line: code: message` per finding blocking in the mode (an error the base holds, without debt, ends ` (pre-existing)`), `new  path: debt-new: …` per new-debt entry when it blocks, `cannot  path: message` per cause, then one summary line (a base adds `, <i> introduced` and, its baseline judged, `, <d> new debt` before `, worst W`):

    spec check [enforce]: 14 documents, 1 errors, 3 warnings, 0 debt, 0 expired, 0 stale, worst W 115602 B — blocked

`lines(true)` adds the other findings (`warning`, `debt`, `error` with `(debt until|expired <date>: <reason>)`), `stale  path: debt-stale: …` and the `new` lines. JSON: `{mode, verdict, counts: {documents, errors, warnings, debt, expired, stale, introduced?, new_debt?, worst_w_bytes}, findings, stale, new_debt?, cannot_check}`, the `?` keys and a finding's `introduced` omitted without a base (`new_debt` also unjudged); `worst_w_bytes` a u64, 0 on `cannot-check`.

**Worst W** (`worst_w`, §3; not a check, no cap) over the walked files as read (staged blobs under `--staged`): every canon `tier: 0` + the largest canon `tier: 1` + the index root (0 if not walked) + its largest live shard + the 3 largest of the pool — every other file but canon tier 0 or 1, Tier 3 and `class: generated`, failed front-matter included.

This repository, root config, no baseline: `enforce` → `clean`, 0 debt (`check_parity.rs`).

## Not checked yet

- Queue state (`@assumes`, an unapplied amendment) → Phase 2. Code (marker → node, `impl_status` bound, glossary term in code, `spec.lock` drift) and 08 AC-13 → Phase 3.

## Open owner questions

Working answer (the code) → what the other answer triggers.

- Q-1 caps in bytes, the unit in the key names → tokens: an ADR amending ADR-0022.
- Q-3 the fix is data → "`spec check` applies it": an ADR amending ADR-0004 / ADR-0005.
- Q-4 answered: `class:` in every document; fixtures (the importer later): decision → `decision` + `scope`, `generator:` → `generated`, others → `canon` + `owner`, `reviewed`.
- Q-6 an overflow may be baselined with expiry; caps never move. Q-7 answered: the root `specengine.toml` (ADR-0029). Q-8 the process rules are the pilots' remaining checks; per-pilot parity at migration.

## Open minors

- `CheckFile.bytes` empty (a future index feed) → line 1 and keys from the parse: a required `title` then fails falsely → the first index-fed check.
- Accepted (diminishing returns): a key with an escaped quote (`'it''s':`) is not seen, a finding on it takes the previous key; an alias key (`*v : y`) takes the anchor line's line and subject (serde-saphyr); `canon: "\t"` reads as unreadable; a block-scalar key (`? |`) gives `""` on the `: v` line.
