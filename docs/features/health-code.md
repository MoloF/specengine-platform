---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 9a-1 (WA-1..WA-13 but WA-4, WA-8; Q1-Q21 but Q7; refreshed and split 2026-10-16 against shipped 6a-2, 6a-3, 10a-1, S1-S15; decided under the owner's pre-approval); 05 s5.3, s5.5; 06 s3, s7; 08 s2; Rust 2 iterations, review accepted, tests 2 rounds; TE-7 the owner's
shipped: 2026-10-19
adrs: [ADR-0047, ADR-0051]
---

# health-code (9a-1): Health's rows 4-5 over the code index

## Why

Health's rows 4-5 (06 s7) read "Not measured yet". `// @assumes PR-…` is an agent's only trace of work on a working answer (06 s3.2); nothing showed it once the owner decided otherwise, rejected or had not decided (ADR-0012's cost); an accepted node no code binds showed only to `spec unmapped`, file by file. Visibility, never a gate. Health must say what `spec refs` and `spec unmapped` say about a marker: since 6a-2, the code index the call refreshes (ADR-0051).

Delivered: `spec health [--json]` and its route gain `code`, `assumes`, `unbound`; `[[health.bound]]`; the shared predicate `maps`; the UI's provisional types and key pins, nothing drawn. Slice 9 is split (Q1, Q19): **9a-1** (this); **9a-2** `health-code-ui.md` (Health's parts for rows 4-5); 9b `health-drift.md` (row 6).

## Description and interactions

Canon: `docs/canon/health-code.md` (the contract); `health.md` "spec health", "Route", "Known limits", "Tests"; `architecture-code.md#health`; `symbol-reads.md` "Crates and seams", "spec unmapped"; `queue-backup.md` "Store"; `serve.md` "Concurrency"; the core, CLI and http READMEs; 05 s5.3, 06 s3, s7, 07, 08 s2.

- **Crates**: core `[health]` reader and section selection; CLI rows 4-5, `maps` beside `place`; store an empty read on an index-only DB; http the handler's comment, tests; ui the provisional types and pins (UI-1); `fixtures/daemon-keys.json` 46 -> 56 sets. No new crate, dependency or check code (`CHECK_CODES` 36); no `crates/specengine-code` edit (Q21).
- **Unchanged**: row 3 byte for byte; `spec refs`, `spec unmapped`; `spec check`, hook, CI, gate, `INDEX_FORMAT`, `CODE_INDEX_FORMAT`, the queue's schema, exit codes. **Writes**: the data directory's md and code indexes only.

## Data

Each section's contract moved into `docs/canon/health-code.md`; the headings stay for the citations (the UI's types cite "Data").

### The document

`health-code.md` "Document".

### Reading the code

"Reading".

### The shared predicate

"Reading" ("Judging"); `symbol-reads.md` "spec unmapped".

### `[[health.bound]]`

"Row 5: unbound" ("Rules", "Candidates").

### What binds

"Row 5: unbound" ("Binding").

### Row 4: `assumes`

"Row 4: assumes".

### Row 5: `unbound`

"Row 5: unbound".

### Text

"Text".

### Failures and exits

"Failures", "Rule"; each row's own causes in its section.

### Route and HTTP

"Route"; `health.md` "Route".

### UI (9a-1's part)

`ui/src/api/provisional.ts`: `HealthReport` gains `code: HealthCode | null`, `assumes`, `unbound` and the ten types, each citing this "Data" (ADR-0033's exception); `daemonKeys.test.ts` pins the ten; `HealthReport` objects gain `code: null`, both rows `off`. Nothing drawn until 9a-2.

## Rules and edge cases

- WHEN a row cannot be measured, Health SHALL show its causes, no count, the other rows and exit unchanged.
- WHEN Health and `spec refs` or `spec unmapped` read the same stored marker, they SHALL agree on whether it binds (`maps`).
- WHEN run from two roots sharing a data directory, another cwd, reverse creation order or CRLF copies, stdout, stderr and JSON SHALL be byte-identical (one tree, config, queue).
- Nothing blocks (ADR-0012, ADR-0047); read-only but the data directory (RD-8); genre (WA-11, TE-5).

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (CLI `tests/health.rs` unless named; RD-7 in 9b):

- [x] RD-1 -- M: keys dropped without `[code]` -> `health_code_keys_present_without_code_table`; row 3 altered -> `amendments_unchanged_by_code_rows`.
- [x] RD-2 -- failures. M: a code walk gap exits 2 -> `code_walk_gap_is_rows_cannot_row_3_measured`; `cannot` with a count -> `a_row_that_cannot_be_measured_has_no_count`; an exit 1 -> `health_never_exits_1`; a refresh note in JSON `notes` -> `refresh_notes_go_to_stderr_only`.
- [x] RD-3 -- M: `PR-7` matched loosely -> `short_queue_id_is_unknown`; a discrepancy by label -> `discrepancy_option_against_recommendation_index`; the working-answer test inverted -> `working_answer_choice_is_as_assumed`; `SqliteQueue::open` -> `health_runs_no_queue_schema_step` (AC-7, S13).
- [x] RD-4 -- M: `target_ids` skipped -> `assumed_node_under_an_open_proposal_is_in_question`; the successor dropped -> `superseded_decision_shows_its_successor`; row 4 filtered by binding -> `assumes_in_a_function_body_is_judged` (Q12); a status word in a new source -> `ac_r7_the_new_sources_write_no_class_status_kind_or_link_type`, extended to them.
- [x] RD-5 -- M: all selected by default -> `no_rule_is_not_configured_never_all_unbound`; `health` not name-checked -> `binding_rule_table_is_name_checked`; an unknown key taken -> `binding_rule_unknown_key_is_refused`; a section by its document's kind -> `section_selected_by_its_own_prefix_kind`.
- [x] RD-6 -- binding. M: mentions bind -> `mentions_never_bind`; `unanchored` binds -> `unanchored_marker_binds_nothing`; `cannot_verify` binds -> `cannot_verify_marker_binds_nothing` (AC-3); `ron_ambiguous` refused -> `ron_ambiguous_marker_binds`; a look-alike binds the fix's node -> `look_alike_marker_binds_nothing`; a document's marker covers its sections -> `a_document_marker_binds_no_section`; `@assumes` binds -> `assumes_marker_binds_nothing_by_default`; Tier 3 selected -> `archived_nodes_are_never_unbound`; `relation` before `cannot_verify` -> `unbound_marker_reason_order`.
- [x] RD-8 -- `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, `spec.lock`, the queue dump, the event `seq` unchanged; outside the data directory nothing written. M: an event written -> `health_writes_no_lock_queue_row_or_event`.
- [x] RD-9 -- M: row 4's rows in stored order, not ID, path, line (row 5's stored order already is path order, review 1) -> `health_code_rows_are_byte_identical_across_roots_and_order`.
- [x] RD-10 -- http `tests/health.rs`, the body the CLI's bytes. M: `cannot` as a 503 -> `a_row_that_cannot_be_measured_is_still_a_200`; `ac_r6_*`, `door.rs`, `turn.rs` green.
- [x] RD-11 -- M: `spec check` reads `[health]` -> `check_output_equal_with_binding_rules`; a new crate or dependency -> `build_graph.rs`.
- RD-12 -- replaced by AC-4, AC-5 (S8).
- [x] RD-13 -- review 1's G1-G4 (owner pre-approved). M: the store's empty read on a schema-run DB without `proposals` -> store `queue_bare_reads.rs`; a queue below the build's schema read as a column error -> CLI `tests/health.rs` (the cause `the queue stands at schema N; …`); a reopened item's choice printed as `pending` -> CLI `tests/health.rs`; a control character in a rejection's note printed raw -> CLI `tests/health.rs`.

The refresh's criteria (2026-10-16), rust-developer unless named:

- [x] AC-1 -- the call refreshes. A temp copy of `fixtures/code-a`: `spec index`, an edit of `crates/tally/src/counter.rs`, `spec health`, then `spec index --json` shows `code.parsed` 0. M: Health reads `code_refs()` without `refresh_code` -> `health_refreshes_the_code_index_as_refs_does`.
- [x] AC-2 -- rows 4-5's source calls `refresh_code(`, `require_complete(`, `code_refs(`, `maps(`, never `read_tree(`, `collect_rust(`, `collect_ron(`, `code_symbols(`. M: the walk revived -> `health_reads_the_code_index_not_the_tree` (a source scan).
- [x] AC-3 -- X3. A temp copy of `fixtures/code-a`, `[[health.bound]] kinds = ["law"]`, `crates/tally/src/broken.rs:4` rewritten to `// @implements LAW-DEFAULT`: row 5 lists `LAW-DEFAULT` with that marker, `binding` and `reason` `cannot_verify`; `spec unmapped crates/tally/src/broken.rs` reads `units 2`, `mapped 1`, `broken` unmapped. M: `cannot_verify` binds -> `cannot_verify_marker_binds_nothing`.
- [x] AC-4 -- parity (TE-2): on `fixtures/code-a`, a rule per relation set, every selected node meets `health-code.md` "Row 5: unbound"'s parity with `spec refs <node> --json`. M: Health's own predicate admitting `cannot_verify` -> `unbound_is_refs_parity_on_code_a`.
- [x] AC-5 -- `maps` is `unmapped`'s: `code_refs.rs`, `symbol_unmapped.rs`, `symbol_reads.rs` (`unmapped_matches_expected_json`) and `symbol_reads_determinism.rs` stay green unchanged. M: `maps` ignores the ID state -> `symbol_unmapped.rs` `dangling_marker_never_maps` and `look_alike_marker_binds_nothing`, both red.
- [x] AC-6 -- failures. A code walk gap (an unreadable `.rs` file), a bad `[code]`, an unknown `[code] frameworks` name, an unwritable data directory: rows 4-5 `cannot` with their causes, row 3 measured, exit 0. No `[code]` (`fixtures/spec-c`, a scratch `HOME`): rows 4-5 `off`, `code` null, no data directory created. M: the index opened without `[code]` -> `no_code_table_resolves_no_data_directory`.
- [x] AC-7 -- a fresh data directory (scratch `HOME`, `[code]` present, nothing ever queued): row 4 `measured`, 0 rows; the slug's DB has no queue table afterwards. M: `SqliteQueue::open` -> `health_runs_no_queue_schema_step`.
- [x] AC-8 -- `CHECK_CODES.len()` 36 (pin `tests/verify_judge.rs`); `git diff --stat <base> -- crates/specengine-code Cargo.lock` empty. M: a check code added -> the pin red.
- [x] AC-9 -- test-engineer: `fixtures/daemon-keys.json` 46 -> 56 sets, each mapped to its type; http `daemon_keys.rs` and UI `daemonKeys.test.ts` green together; health-left's pins change only by the new lines. M: a key dropped from the fixture -> both red.

ui-developer:

- [x] UI-1 -- "UI (9a-1's part)": `getHealth`'s path unchanged; `pnpm lint` 0, `build`, `test` green; `HealthView.test.tsx`'s "holds no digit in the value of rows 4 to 6" unchanged. M: a key missing from a type -> `daemonKeys.test.ts`.
- UI-2..UI-6 -- moved to `health-code-ui.md` UI-1..UI-5 (Q19).

test-engineer:

- [x] TE-1 -- temp git copies of `fixtures/code-a`, rules appended, the queue seeded through the store: (a) open, as assumed, otherwise, a discrepancy option not its recommendation, rejected, an applied update, `PR-7` (`crates/tally/src/lib.rs:13`); (b) a superseded decision, a rejected one, a node under an open proposal, `@assumes PREM-001` (`counter.rs:37`, not listed). `git diff --stat -- fixtures/` empty.
- [x] TE-2 -- AC-4 per selected node, and with `spec unmapped --json`: a marker row 5 counts as binding sits on a mapped unit; one listed `unanchored` or `cannot_verify` maps nothing.
- [x] TE-3 -- failures x rows 3-5: states, causes, stderr, exit 0; an md walk gap exit 2; the queue's causes fail row 4 alone.
- [x] TE-4 -- RD-8's probe, RD-9's determinism over every TE-1 scenario; a repeated call re-derives nothing (AC-1).
- [x] TE-5 -- `fixtures/spec-c` both `off`; the new sources hold no fixture prefix, kind, status word, `cargo`, `nextest`, `bevy` or framework name, and join `NEW_MODULES` (13 + 4, `EXEMPT_WORDS` `rule` for `health_code.rs`); `proposal_genre.rs`, http `no_domain.rs` green.
- [x] TE-6 -- AC-9.
- [ ] TE-7 -- the owner's: pilots, manual (`health_code_pilot.rs`, CLI and http, `#[ignore]`, release, `--timeout`, scratch `HOME`, configs outside): per-row counts; CLI and route wall time (`full_ms` first, then a repeat) against AC-10's 2 s and `noop_ms` ≤ 200; read-only proved; no pilot name or path committed.
- [x] TE-8 -- every M above turns red.

spec-writer:

- [x] SW-1 -- 2026-10-10: this spec, `health-drift.md`, ADR-0047 and its canon. 2026-10-16: the refresh, `health-code-ui.md`, ADR-0051 and its canon, 08 s2.
- [x] SW-2 -- 2026-10-19, at shipping: new Tier 2 `docs/canon/health-code.md`; `health.md`, `serve.md` "Concurrency", `symbol-reads.md`, `architecture-code.md#health`, `queue-backup.md` "Store", `spec-check.md` (net -5 B); the core, CLI and http READMEs (store and `ui/README.md` untouched); `health-code-ui.md`, `health-drift.md` re-pointed to the canon; 05 s5.3, s5.5, 06 s3, s7, 07, 08 s2 and risks; `CLAUDE.md` State; `export index`, `check` clean.

## Out of scope

Health's parts (9a-2); follow-up tasks, retargeting markers (06 s3, Q10); inbox order by `@assumes`; row 6 (9b); MCP, skills; the home; check findings; the gate; per-node `needs:`, evidence levels (05 s5.5); symbols, containment, code roles; a watcher; a clock; other worktrees; generated types; project status words; path targets in `in_question`.

## Open

Decided under the owner's pre-approval: Q1, Q19 the split (9a-1, 9a-2, 9b); Q3 `[[health.bound]]`, no rule: not configured; Q5 eight row-4 states; Q8 the index the call refreshes (ADR-0051); Q9 read-only but the data directory's indexes, the queue by `open_existing`; Q12 row 4's unit a stored marker; Q13 `maps` shared with `unmapped`; Q14 X3, `cannot_verify` binds nothing, `ron_ambiguous` binds; Q15-Q17 a failure fails its rows, exit 0, notes on stderr; Q18 row 6 at 9b's refresh; Q21 no code-crate edit.

## Implementation

| Module | What it does |
|---|---|
| core `health_toml.rs` | `HealthConfig::from_toml`, `BoundRule`: `[health]`, the first error by line |
| core `check/health.rs` | `bound_candidates`: row 5's nodes, sections by their own prefix kind |
| core `project_toml.rs` | `RawFile.health`, name-checked |
| CLI `health_code.rs` | `code_rows`: reading as `refs`, the row-4 judge `decided`, binding, text, the JSON types |
| CLI `health.rs`, `lib.rs` | `HealthOutcome.code_rows`; rows 4-5 after row 3; stderr notes; re-exports |
| CLI `symbols.rs`, `unmapped.rs` | `maps<'s>`, the shared seed predicate |
| CLI `refs.rs` | `no_code_table`, `Corpus::node_line` split out, bytes unchanged |
| store `queue.rs` | `index_only` (`user_version` 0, no `proposals`): `get` -> `None`, `raw_rows` empty; else the `SELECT` errors |
| store `queue/state.rs` | `SqliteQueue::schema_version()`: `user_version`, read only |
| CLI `proposals.rs` | `queue_below_schema(schema)`: G2's cause, beside `queue_cannot` |
| http `app.rs` | the `health` handler's comment |

Deviations, accepted in review 1:

1. One config read (`locate`, `load_config`), not `discover_with_text`: one state, one result; the cause "the config `discover_with_text` refuses" cannot occur.
2. The row-4 judge lives in the CLI (`health_code::decided`): the queue's types are the store's, which core cannot see.
3. `fn maps<'s>(…)`: the spec's `Place<'_>` with two reference inputs is E0106; the body is the spec's.
4. `[[health.bound]]` messages are prefixed `` `[[health.bound]]` ``, not `check rule`; the first error by line, unlike check rules.
5. An unreadable `open` or `approved` row makes row 4 `cannot` while (b) reads them, even one no marker names: a skipped row could hide an `in_question`.
6. A deciding item approved or applied with no record reads `otherwise` (legacy rows only).
7. `drift` does not decide: approved `pending`, applied `applied`.
8. The queue is not opened when no marker is judged: a too-new queue leaves row 4 `measured`, 0 rows.

Iteration 2 built review 1's minors (the owner's answers) as worded: G1 the empty read only at `user_version` 0, a schema-run DB without `proposals` errors; G2 after `open_existing`, `schema_version()` 1-7: row 4 `cannot`, `the queue stands at schema N; a queue command upgrades it`, no step, nothing written; G3 choice and record printed only for `as_assumed`, `otherwise`, JSON as stored; G4 rows 4-5's lines through `escape_controls`, row 3's bytes fixed. Deviations, accepted:

9. G2's sentence is `proposals.rs::queue_below_schema`, not a `health_code.rs` literal: TE-6's scan reads `command` there as spec-b's kind.
10. G4 escapes stdout lines only, causes too; stderr `note:` lines keep `Message::line`'s convention, JSON stays raw.

Tests: round 1 11/11 mutations red; round 2 2784/2784 over six crates, 5/5 mutations red, a new pin `a_row_5_cause_line_is_escaped_on_stdout_raw_on_stderr`; TE-7 the owner's.
