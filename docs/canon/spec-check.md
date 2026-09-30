---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-eval, xtask]
owner: owner
reviewed: 2026-09-30
---

# spec check: what the documentation check enforces

Increment 1 of 4 (pending groups: 05 §4, 08 Phase 1): one check of the convention — §11.1–4 of `docs/canon/documentation-system.md`, the ADR-0009 ID checks, an expiring debt baseline — driven only by `specengine.toml`; the source names no prefix, path or file of a project (`#universal`). Engine `specengine_core::check::run` (pure), loader `specengine_store::load_check` (fresh parse; no database or daemon), commands `spec check`, `spec export index` (`docs/canon/spec-check-cli.md`), measurement `specengine-eval check` (store and eval APIs: their READMEs). Nothing is written and no status or flag is set (`#control`, `#apply`): the homoglyph fix is data for `apply_proposal`. This repository's gate stays `xtask` until CLI pass 2b.

## Engine types (`specengine_core::check`)

- `CheckInput {files: [CheckFile {path, size, parsed?, read_error?, bytes}], problems: [Problem {kind: MissingRoot | UnreadableDir | SkippedName, path}]}`, `CheckFile::{parse, parsed, unreadable}`. `bytes` (the parsed text) give real lines, a parser diagnostic's subject and the written keys (`title`, `kind`, `id` are not in `fields`).
- `CheckConfig::from_toml` → `{budgets: Budgets {tier0_bytes, tier1_bytes, index_bytes, decision_bytes, canon_bytes?, bundle_node?, bundle_task?}, classes: Classes {canon, decision, spec, generated: ClassContract {required, optional, closed}}, mode: Mode}`; `Baseline::from_toml` → `[DebtEntry {code, path, subject, reason, expires, line}]`; both errors `{line?, message}`, `at(file)` → `file:line: message`.
- `Report {mode, verdict, counts, findings, stale, cannot_check: [Cause {path, message}]}`, `lines(detail)`, `to_json()`, `exit_code()`, `Report::cannot(mode, causes)`; `CHECK_CODES` (29), `PARSER_SEVERITY` (13 rows).

## Configuration

`[paths]` gains `tier0` (the one file canon tier 0 may be), `tier1_name` (the only name a canon tier 1 file may have) and `index` (capped by `index_bytes`, whatever its class); each rule is off while its key is absent. This repository's parity config (built by `crates/specengine-store/tests/check_parity.rs`: no root `specengine.toml` yet, Q-7):

```toml
[paths]
records = "docs/decisions"   # a record is named after its id
tier0 = "CLAUDE.md"
tier1_name = "README.md"
index = "docs/index.md"
exclude = ["**/_*.md"]       # + "**/<name>/**" per xtask SKIP_DIRS entry
[ids]
ADR = { kind = "decision", width = 4 }
[budgets]                    # bytes of the whole file, BOM and front-matter included
tier0_bytes = 16384
tier1_bytes = 10240
index_bytes = 10240
decision_bytes = 1536
canon_bytes = 12288
[classes]                    # a class written here replaces its default contract
decision = { required = ["class", "id", "title", "status", "date", "scope"], optional = ["canon", "supersedes", "ref"], closed = true }
[check]
mode = "enforce"             # observe | enforce (default)
```

Defaults: the four §4 caps above, `canon_bytes` none; `bundle_node`, `bundle_task` accepted (tokens, 07 §5). Default contracts, open: canon `owner`, `reviewed`; decision `id`, `status`, `scope`; spec `status`, `scope`; generated nothing; each plus `class`. `closed = true` admits only `class`, `required`, `optional`. A written class replaces its default whole: an omitted `required` or `optional` is empty, `closed` false, so `canon = { closed = true }` admits only `class`. An unknown key or class, a wrong type, a cap < 1, an unknown mode → `specengine.toml:<line>: message`, and the run cannot check. The index fingerprint reads `[ids]` only: editing the check tables re-parses nothing.

## Rules

- **Front-matter fails** (`not-utf8`, `frontmatter-unclosed`, `-yaml`, `-not-mapping`): the file gives only its parser findings, and its body the graph warnings (with `frontmatter-unclosed`, the whole file: `docs/canon/spec-check-graph.md`). `xtask` reads such YAML leniently and judges the file: an accepted divergence.
- **Class.** Every document declares one (owner, Q-4): none, or no front-matter → `class-missing`, no contract and no class cap, while IDs, references and `canon:` are still checked; not one of the four → `class-unknown`. Per class: `key-missing`, `key-extra` (closed), `scope-empty`, `date-invalid` (`reviewed`, `date`, `shipped` not shaped `YYYY-MM-DD`), `status-invalid` (spec draft | in-progress | shipped | abandoned; decision accepted | rejected | `superseded-by <ID>`), `shipped-missing`, `canon-missing` (accepted decision: absent or blank → "has no `canon:`"; a value the parser could not read → "… is unreadable"), `tier-invalid` (canon tier not 0–2; `tier: 0` off `tier0`; `tier0` not tier 0, the only rule on `tier0`; `tier: 1` on a file not named `tier1_name`).
- **IDs.** A number-shape definition (`id:`, `{#ID}`; references never) has `width` digits, else `id-width`. Mixed script → `homoglyph`, an error with its Latin `fix`. An ID defined in two files → `id-taken` on each later file by path, naming the first; a feature-scoped ID is unique per feature, misplaced → `id-scope` (`docs/canon/spec-check-links.md`). Under `records`, `id: X` names its file `X` + `.` or `-`, else `file-name` (`xtask`'s `starts_with` passes `ADR-00011.md` for `ADR-0001`).
- **References** — `supersedes`, `status: superseded-by`, `adrs`, `refs`, `working_answer`, `parent`, `links.*`, a reference-form `canon:` — resolve: the ID is defined; or the text is in a document's `aliases:`; or, through `aliases_from`, the configured prefix + the written body is defined (no re-padding); `#Y` is defined in the ID's file. Else `ref-dangling`. An alias `parent` is re-read through the scheme. Where an ID may resolve (`slug/`, bare feature-scoped IDs): `docs/canon/spec-check-links.md`. Not yet: `project:`, `@rev`.
- **Path-form `canon:`**, on any document: has `#` (`canon-form`), names a walked canon document (`canon-file`), and one of its anchors or section IDs (`canon-anchor`). Anchors: heading slugs, `{#…}`, `<a id>`, `<a name>` (`crates/specengine-model/README.md`); a start tag split over lines in an HTML block or a blockquote is missed, as by `xtask`: an accepted divergence.
- **Budgets** (`budget`, subject = the slot): whole-file bytes > cap. `index` by path; canon tier 0 → `tier0_bytes`, tier 1 → `tier1_bytes`, else `canon_bytes`; decision → `decision_bytes`; spec, generated, class-less: none.
- **Walk.** Non-UTF-8 names → one warning `name-skipped` per problem path, its message counting them; a missing written root, an unreadable file or directory → cannot check; a missing default role root is ignored.
- **Parser codes** pass as themselves through one table, `PARSER_SEVERITY`, with the parser's severity but `homoglyph`, `duplicate-id` (errors).

The check's own codes (`CHECK_CODES`, 29) are the errors named above, `name-skipped`, the seven of `docs/canon/spec-check-graph.md` (§11.5–6, inline mentions, graph) and the two file-link warnings of `docs/canon/spec-check-links.md`.

## Findings, debt, verdict

`Finding {code, severity, path, line, subject, message, fix?: {span, text}, debt?: {reason, expires, expired}}`; `subject` = the object as written (ID, key, `canon:` value, budget slot `tier0|tier1|index|decision|canon`, a parser diagnostic's span text), `""` for the whole file (line 1). A spanless `unknown-key` or `frontmatter-type` takes the written top-level key whose entry holds its line: after `!tag` / `&anchor` (`!!str k:`), and `a` for `? a`; a flow key (`[a, b]:`), an `*alias` line, a non-scalar `? [q]`, or no bytes → `""`, as for every other spanless finding (`frontmatter-yaml`, `-not-mapping`, `-unclosed`, `not-utf8`). The class contract reads the same keys. A finding identical in every field is reported once.

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

**Verdict.** `observe` never blocks; `enforce` blocks on errors not in live debt; warnings, debt and stale entries never block. `clean` → exit 0, `observed` (errors under `observe`) → 0, `blocked` → 1, `cannot-check` → 2, winning in any mode (a missing written root, an unreadable file or directory, an invalid config, baseline or `today`). Counts: `errors`, `warnings` exclude live debt; `expired` is part of `errors`.

**Output**, sorted by (path, line, code, subject, message) whatever the input order. Lines: `error  path:line: code: message` per finding blocking in the mode, `cannot  path: message` per cause, then one summary line:

    spec check [enforce]: 14 documents, 1 errors, 3 warnings, 0 debt, 0 expired, 0 stale — blocked

`lines(true)` adds the other findings (`warning`, `debt`, `error` with `(debt until|expired <date>: <reason>)`) and `stale  path: debt-stale: …`. JSON: `{mode, verdict, counts: {documents, errors, warnings, debt, expired, stale}, findings, stale, cannot_check}`.

This repository under the parity config, no baseline (Q-2): every document walked, `enforce` → `clean`, 0 debt (`check_parity.rs`).

## Not checked yet

- Increment 3: part 1 shipped (CLI 2a.1); 2a.2 `--staged`, `enforce-introduced`, no new baseline entries; 2b the root config (Q-7), hook (also on both TOML files) and CI, `xtask` retired: `docs/canon/spec-check-cli.md`.
- Increment 4, `spec-check-process`, from config: decision without cost, question without `to` or working answer, accepted feature with an empty "Implementation" (heading from config), numbered record with its own text; per-kind schemas (core Q2). Queue state (`@assumes`, an unapplied amendment) → Phase 2, non-blocking. Code (marker → node, `impl_status` bound, glossary term in code, `spec.lock` drift) and 08 AC-13 → Phase 3.

## Open owner questions

Working answer (the code) → what the other answer triggers.

- Q-1 caps in bytes, the unit in the key names → tokens: an ADR amending ADR-0022.
- Q-2 = core Q6, answered (owner, 2026-09-29): the four invalid YAML scalars quoted; no baseline.
- Q-3 the fix is data → "`spec check` applies it": an ADR amending ADR-0004 / ADR-0005.
- Q-4 answered: `class:` in every document. Per record kind in the fixtures (the importer later): decision → `decision` + `scope`; a file with `generator:` → `generated`; others → `canon` + `owner`, `reviewed`, since only decisions are superseded (§2), though `immutable_text` records are not rewritten in place.
- Q-5 answered (owner's Q4, 2026-09-30): "introduced" is relative to `HEAD`. Q-6 an overflow may be baselined with expiry; caps never move. Q-7 root `specengine.toml`: CLI 2b, an ADR amending ADR-0023's role table. Q-8 the pending groups are the pilots' full check list; per-pilot parity at migration.
- `serde_json` as a normal `specengine-core` dependency (`to_json`; `=1.0.151`, locked): awaiting acknowledgement.

## Open minors

- `CheckFile.bytes` empty (a future index feed) → line 1 and keys from the parse: a required `title` then fails falsely → the first index-fed check.
- Accepted, not fixed (diminishing returns): a quoted key with an escaped quote (`'it''s':`, `"a\"b":`) is not seen, so a finding on it takes the previous key; an alias key (`*v : y`) gets the anchor's line from serde-saphyr, so line and subject are the anchor line's; `canon: "\t"` (escaped blank) reads as unreadable; a block-scalar key (`? |`) gives `""`, its finding on the `: v` line.
