---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 9a-1 (WA-1..WA-13 but WA-4, WA-8; Q1-Q11 but Q7, owner pre-approved, review open; refreshed and split 2026-10-16 against shipped 6a-2, 6a-3 and 10a-1, S1-S15, Q8 and Q9 re-decided, Q12-Q21, decided under the owner's pre-approval, review open); 05 s5.3, s5.5; 06 s3, s7; 08 s2
adrs: [ADR-0047, ADR-0051]
---

# health-code (9a-1): Health's rows 4-5 over the code index

## Why

Health's rows 4-5 (06 s7) read "Not measured yet". `// @assumes PR-…` is an agent's only trace of work on a working answer (06 s3.2); nothing shows it once the owner decides otherwise, rejects or has not decided (ADR-0012's cost); an accepted node no code binds shows only to `spec unmapped`, file by file. Visibility, never a gate. Health must say what `spec refs` and `spec unmapped` say about a marker: since 6a-2, the code index the call refreshes (ADR-0051).

Delivered: `spec health [--json]` and its route gain `code`, `assumes`, `unbound`; `[[health.bound]]`; the shared predicate `maps`; the UI's provisional types and key pins, nothing drawn. Slice 9 is split (Q1, Q19): **9a-1** (this); **9a-2** `health-code-ui.md` (Health's parts for rows 4-5), right after; 9b `health-drift.md` (row 6).

## Description and interactions

- **The owner** reads in `spec health` (Health from 9a-2) each marker naming a queue item and how it was decided, each `@assumes` on an archived node or one in question, each selected node no marker binds with the markers that name it without binding and why.
- **Crates** (WA-12):
  - core: the `[health]` reader (`RawFile` gains `health`, name-checked only), section selection, the row-4 judge (pure: proposal rows in, a state out);
  - CLI: `health.rs` (or a sibling module for rows 4-5), `maps` extracted into `symbols.rs` beside `place`, `unmapped.rs` calling it;
  - store: no new API; a fix inside `get` or `list_readable` only if one errors without queue tables (AC-7);
  - http: tests; `app.rs:418-421`'s doc comment ("no index refreshed") rewritten;
  - ui (ui-developer, minimal): "UI (9a-1's part)";
  - fixtures: `daemon-keys.json` 46 -> 56 sets.
  - No new crate or dependency; `CHECK_CODES` stays 36 (S2); no `crates/specengine-code` edit (Q21).
- **Unchanged**: row 3 byte for byte; `spec refs` and `spec unmapped`, rules and bytes; `spec check`, hook, CI, gate, `INDEX_FORMAT`, `CODE_INDEX_FORMAT`, the queue's schema, exit codes. **Writes** (Q9, S4): the data directory's md and code indexes, as `spec refs`; never the queue (no schema step), `spec.lock`, git or an event.
- **Overridden at shipping**: the lines SW-2 lists (S12; 07:104's `--unbound` is gone since 6a-3's Q4); ADR-0051's canon diff is done (SW-1).

## Data

### The document

WA-1. `spec health --json`, the route's body too: `{documents, amendments, notes, code, assumes, unbound}` (9b adds `drift`). `documents`, `amendments`, `notes`: row 3's, unchanged (Q17). `code`: `{files, rs, ron, skipped}`, `spec refs`' count of the stored files (`refs::stored_count(&stored, refresh.report.skipped)`, S5), null unless rows 4-5 read the index. A row: `{state, causes, counts, rows}`; `state` `measured`, `off` (no `[code]`), `not_configured` (row 5: no rule) or `cannot`; `causes` the lines behind any other state, else `[]`; `counts`, `rows` null unless `measured`, never 0. Every key present, absent null; uncut.

Key sets (AC-9): `HealthReport` + the three; ten new, keys as below: `HealthCode`, `HealthAssumes` and `HealthUnbound` (`state`, `causes`, `counts`, `rows`), `AssumesCounts`, `AssumesRow`, `AssumedProposal`, `AssumedNode`, `UnboundCounts`, `UnboundRow`, `UnboundMarker`.

### Reading the code

Q8, ADR-0051. After row 3 (its exit-2 causes stay first), when `CodeConfig::from_toml` on the config text gives a table (Q15), one database handle, as `refs.rs` `refs`:

1. `project::discover_with_text`, `location::open_index` (HOME, the data directory, the slug's DB created when absent);
2. `corpus::indexed_in(&mut open.index, …)` as `refs` calls it: the md index refreshed; its `CheckInput` gives row 5's candidates and `Corpus::new`'s input;
3. `code_index::refresh_code(…, false)`, then `require_complete()`; its `messages` to stderr;
4. `open.index.code_refs()` (`None`: empty): files, markers, mentions in one read; no symbol read.

Each marker's ID is judged once by `refs::Corpus::judge`. Never `code_walk::read_tree`, `refs::collect_rust`, `collect_ron` (the tests' oracle), `code_symbols`, the role rows or `[code] frameworks` (the refresh's alone), `spec.lock`, git.

### The shared predicate

Q13. `symbols::maps(marker: &StoredMarker, judged: &Judged) -> Result<Option<Place<'_>>, CliError>`: `place(&marker.binding)?` first (an unknown stored word errors whatever the ID), then `Some(place)` when `judged.state.names_nodes()` and the place is not `Place::Nothing`, else `None`. `unmapped.rs`'s seed loop (`:343-348` at HEAD) becomes `if let Some(seed) = maps(marker, &corpus.judge(&marker.id))? { … }`, its output unchanged (AC-5). Row 5 calls it; Health tests no binding word or ID state itself.

### `[[health.bound]]`

WA-6, Q3.

```toml
[[health.bound]]                     # every live law
kinds = ["law"]
[[health.bound]]                     # accepted canon, by code or data
paths     = ["docs/spec/**"]
classes   = ["canon"]
when      = { status = "accepted" }
relations = ["implements", "configures"]
```

- Read only by `spec health`; `[health]` holds `bound` alone; `ProjectConfig` only name-checks `health` (a bad rule fails no other command; `spec check` judges none).
- Selectors `kinds`, `classes`, `paths`, `when`: `[[check.rules]]`' grammar and messages (`spec-check-process.md` "A rule"), at least one; AND between, OR within. `relations`: optional, non-empty, of `implements`, `verifies`, `configures`, `assumes`; default the first three. Another key: the TOML reader's message. The first error by line, `specengine.toml:<line>: message`: row 5 `cannot`.
- **Candidates**: live (`is_live`) nodes with an ID in a file whose front-matter reads, from the md index's `CheckInput` (the judge's input). A document by its own fields (kind: `kind:`, else its ID's prefix kind); a `{#ID}` section by its ID's prefix kind and its document's class, path, front-matter (new: check rules judge no section).
- No rule: `not_configured`, cause ``no [[health.bound]] rule: which nodes must be bound is the project's to declare``.

### What binds

WA-7, Q4, Q13, Q14. A node is **bound** for a rule selecting it when some stored marker has one of the rule's `relations`, `maps(marker, judged)` is `Some`, and the node is among `judged.nodes`: an ID `resolved`, `alias` or `ambiguous` (each holder), a binding `item`, `module`, `ron` or `ron_ambiguous`. Unbound when some selecting rule finds it so.

- Never binding: `unanchored`; `cannot_verify` (X3: a broken item's marker, as in `spec unmapped`, Q14); mentions; `homoglyph` (nor the fix's node); `queue`, `dangling`, `other_project`; `@assumes` unless listed.
- Containment never applies (`unmapped`'s `parent_ord` chain spreads a marker over code units, not nodes); a document's marker binds none of its sections, a section's not its document. `rev_state`, levels and the lock never matter (row 6's).
- **Parity** (AC-4): unbound for a rule ⟺ `spec refs <node> --json` lists no marker of its relations whose `id_state` is `resolved`, `alias` or `ambiguous` and whose `binding` is neither `unanchored` nor `cannot_verify`.

### Row 4: `assumes`

WA-5, Q5, Q12. The unit is a stored marker, no symbol, binding or containment filter (an `@assumes` in a body, `unanchored in_body`, counts). Judged: (a) every marker whose `judge(id).state` is `queue`, any relation, any binding; (b) every `assumes` marker whose state `names_nodes()`, one row per holder. The queue: `SqliteQueue::open_existing(&open.db, slug)` (no file: empty; no queue tables: empty, no schema step, AC-7), `git_common_dir: None` (every repository, SW refinement): `get` by exact ID, `list_readable` for (b).

| `state` | When |
|---|---|
| `pending` | (a) `open`, or `approved` of a kind that does not decide |
| `as_assumed` | (a) deciding, `approved` or `applied`, `choice` `{"working_answer":true}` or `{"option":N}` with N its `recommendation` (indices, not labels) |
| `otherwise` | (a) deciding, `approved` or `applied`, another `choice` |
| `applied` | (a) `applied`, a kind that does not decide |
| `rejected` | (a) `rejected` |
| `unknown` | (a) no item of that ID (`PR-7`: the queue writes 4+ digits; a queue restored without it) |
| `archived` | (b) the node's document is Tier 3 (`is_tier3_file`) |
| `in_question` | (b) else an `open` or `approved` item's `target_ids` hold its ID |

(b) in neither: unlisted. A row per marker and holder, by `id` as written, path, line, column, holder path. Words: the model's (`ProposalStatus`, `decides()`, `Choice`), never a project's status.

```json
"assumes":{"state":"measured","causes":[],
 "counts":{"markers":3,"rows":2,"pending":0,"as_assumed":0,"otherwise":1,"applied":0,"rejected":0,"unknown":1,"archived":0,"in_question":0},
 "rows":[{"id":"PR-0042","state":"otherwise","path":"crates/tally/src/counter.rs","line":40,"column":8,"relation":"assumes",
   "proposal":{"kind":"question","status":"applied","choice":{"answer":"No"},"record_id":"DEC-0003","record_path":"docs/decisions/DEC-0003.md","reason":null},"node":null},
  {"id":"PR-7","state":"unknown","path":"crates/tally/src/lib.rs","line":13,"column":4,"relation":"implements","proposal":null,"node":null}]}
```

`counts`: `markers` judged, `rows` listed, one per state. `proposal` ((a) but `unknown`): `kind`, `status`, `choice` (stored one-key JSON), `record_id`, `record_path`, `reason` (a rejection's), null when absent. `node` ((b)): `{id, path, line, status, successor, proposals}`: canonical ID and place (`spec refs`' header), its document's raw `status:`, X of `superseded-by X`, the `open`/`approved` items holding it, by ID.

### Row 5: `unbound`

```json
"unbound":{"state":"measured","causes":[],
 "counts":{"rules":1,"selected":5,"bound":3,"unbound":2},
 "rows":[{"id":"LAW-SOURCE","kind":"law","title":"Source","path":"docs/spec/limits.md","line":12,"rules":[1],"mentions":0,"markers":[]},
  {"id":"LAW-DEFAULT","kind":"law","title":"Default","path":"docs/spec/limits.md","line":16,"rules":[1],"mentions":0,
   "markers":[{"path":"crates/tally/src/broken.rs","line":4,"column":4,"relation":"implements","id_state":"resolved","binding":"cannot_verify","reason":"cannot_verify"}]}]}
```

AC-3's tree (`code-a`, `kinds = ["law"]`). Rows by path bytes, line. `rules`: 1-based indices (written order) of the rules it fails; `mentions`: those naming it (ADR-0016's adoption signal); `markers`: those naming it (the fix's node for a look-alike) without binding, by path, line, column; `binding` `spec refs`' word; `reason` the first that applies of `homoglyph`, `unanchored`, `cannot_verify`, `relation` (Q14, S7).

### Text

Row 3's lines, then:

```text
assumes | otherwise | PR-0042 | crates/tally/src/counter.rs:40:8 | assumes | question applied {"answer":"No"} | record DEC-0003 docs/decisions/DEC-0003.md
assumes | unknown | PR-7 | crates/tally/src/lib.rs:13:4 | implements
unbound | LAW-SOURCE | law | Source | docs/spec/limits.md:12 | rules 1 | mentions 0
unbound | LAW-DEFAULT | law | Default | docs/spec/limits.md:16 | rules 1 | mentions 0
  implements | crates/tally/src/broken.rs:4:4 | resolved | cannot_verify | cannot_verify
assumes: 2 rows of 3 markers: 0 pending, 0 as_assumed, 1 otherwise, 0 applied, 0 rejected, 1 unknown, 0 archived, 0 in_question
unbound: 2 of 5 selected nodes, 1 rules
code: 8 files (.rs 7, .ron 1), skipped 0
spec health: 6 documents, 0 unapplied amendments
```

Row 4's tail: `pending` `<kind> <status>`; `as_assumed`, `otherwise` `<kind> <status> <choice>[ | record <id> <path>]`; `applied` `<kind> applied`; `rejected` `<kind> rejected: <reason>`; `archived` `<node> | <path>:<line> | status <status>[ | successor <X>]`; `in_question` `<node> | <path>:<line> | under <IDs, ", ">`. Fields one-lined; `code:` only when read; the last line unchanged. A row not measured: `<row>: off|not configured|cannot: <cause>` per cause, `cannot` also on stderr as `note: <row>: <cause>`.

### Failures and exits

WA-2, Q6, Q15, Q16. Exit 2 stays row 3's alone (discovery, base config, unreadable root, md walk gap). Every other cause fails only its rows, exit 0, never 1, the cause the failing step's CLI line less `spec: `:

- no `[code]`: rows 4-5 `off`, cause code-refs' sentence (`<config> has no [code] table: …`), `code` null; nothing opened, no data directory resolved;
- a bad `[code]` (`<config>:<line>: message`; an unknown `[code] frameworks` name too), the config `discover_with_text` refuses, HOME, the data directory, the md refresh, `refresh_code`, a code walk gap (`the code walk is incomplete: <gap>`), `code_refs`, an unknown stored word (`place`'s): rows 4-5 `cannot`, `code` null;
- the queue (a newer schema, a corrupt row, `PR` in `[ids]`): row 4 `cannot`, whole;
- no rule: row 5 `not_configured`; a bad `[health]`: row 5 `cannot`. Row 5's precedence: `off`; else `not_configured`; else `cannot`, every cause (`[health]`'s, then the code's); else `measured`.
- Notes (Q17): the refreshes' notes and warnings go to stderr as `spec refs` prints them, never into JSON `notes`.

### Route and HTTP

WA-9. Route unchanged (`health.md` "Route"): any row state a 200, a 503 only on exit 2; it writes the data directory's indexes in the project's turn. http `tests/daemon_keys.rs` (`:734-735`) reads the new keys.

### UI (9a-1's part)

Q19. ui-developer, nothing drawn changes (rows 4-6 stay "Not measured yet" until 9a-2):

- `ui/src/api/provisional.ts`: `HealthReport` gains `code: HealthCode | null`, `assumes: HealthAssumes`, `unbound: HealthUnbound`; the ten types, each citing `docs/features/health-code.md` "Data" (ADR-0033's exception: no type generation).
- `ui/src/api/daemonKeys.test.ts`: `:345`, `:422`, `:468` (a list beside `HEALTH_LEFT` for the ten), `:633` (citations), `:785` (the fixture's `HealthReport` keys).
- Objects typed `HealthReport` (`mocks/health.ts`, tests) gain `code: null`, both rows `off`; real mocks are 9a-2's.

## Rules and edge cases

- WHEN a row cannot be measured, Health SHALL show its causes, no count, the other rows and exit unchanged.
- WHEN Health and `spec refs` or `spec unmapped` read the same stored marker, they SHALL agree on whether it binds (`maps`).
- WHEN run from two roots sharing a data directory, another cwd, reverse creation order or CRLF copies, stdout, stderr and JSON SHALL be byte-identical (one tree, config, queue).
- Nothing blocks (ADR-0012, ADR-0047); read-only but the data directory (RD-8); genre (WA-11, TE-5).

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (CLI `tests/health.rs` unless named; RD-7 in 9b):

- [ ] RD-1 -- M: keys dropped without `[code]` -> `health_code_keys_present_without_code_table`; row 3 altered -> `amendments_unchanged_by_code_rows`.
- [ ] RD-2 -- "Failures and exits". M: a code walk gap exits 2 -> `code_walk_gap_is_rows_cannot_row_3_measured`; `cannot` with a count -> `a_row_that_cannot_be_measured_has_no_count`; an exit 1 -> `health_never_exits_1`; a refresh note in JSON `notes` -> `refresh_notes_go_to_stderr_only`.
- [ ] RD-3 -- M: `PR-7` matched loosely -> `short_queue_id_is_unknown`; a discrepancy by label -> `discrepancy_option_against_recommendation_index`; the working-answer test inverted -> `working_answer_choice_is_as_assumed`; `SqliteQueue::open` -> `health_runs_no_queue_schema_step` (AC-7, S13).
- [ ] RD-4 -- M: `target_ids` skipped -> `assumed_node_under_an_open_proposal_is_in_question`; the successor dropped -> `superseded_decision_shows_its_successor`; row 4 filtered by binding -> `assumes_in_a_function_body_is_judged` (Q12); a status word in a new source -> `ac_r7_the_new_sources_write_no_class_status_kind_or_link_type`, extended to them.
- [ ] RD-5 -- M: all selected by default -> `no_rule_is_not_configured_never_all_unbound`; `health` not name-checked -> `binding_rule_table_is_name_checked`; an unknown key taken -> `binding_rule_unknown_key_is_refused`; a section by its document's kind -> `section_selected_by_its_own_prefix_kind`.
- [ ] RD-6 -- "What binds". M: mentions bind -> `mentions_never_bind`; `unanchored` binds -> `unanchored_marker_binds_nothing`; `cannot_verify` binds -> `cannot_verify_marker_binds_nothing` (AC-3); `ron_ambiguous` refused -> `ron_ambiguous_marker_binds`; a look-alike binds the fix's node -> `look_alike_marker_binds_nothing`; a document's marker covers its sections -> `a_document_marker_binds_no_section`; `@assumes` binds -> `assumes_marker_binds_nothing_by_default`; Tier 3 selected -> `archived_nodes_are_never_unbound`; `relation` before `cannot_verify` -> `unbound_marker_reason_order`.
- [ ] RD-8 -- `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, `spec.lock`, the queue dump, the event `seq` unchanged; outside the data directory nothing written. M: an event written -> `health_writes_no_lock_queue_row_or_event`.
- [ ] RD-9 -- M: rows in stored order, not path and line -> `health_code_rows_are_byte_identical_across_roots_and_order`.
- [ ] RD-10 -- http `tests/health.rs`, the body the CLI's bytes. M: `cannot` as a 503 -> `a_row_that_cannot_be_measured_is_still_a_200`; `ac_r6_*`, `door.rs`, `turn.rs` green.
- [ ] RD-11 -- M: `spec check` reads `[health]` -> `check_output_equal_with_binding_rules`; a new crate or dependency -> `build_graph.rs`.
- RD-12 -- replaced by AC-4, AC-5 (S8: no pass to share).

The refresh's criteria (2026-10-16), rust-developer unless named:

- [ ] AC-1 -- the call refreshes. A temp copy of `fixtures/code-a`: `spec index`, an edit of `crates/tally/src/counter.rs`, `spec health`, then `spec index --json` shows `code.parsed` 0. M: Health reads `code_refs()` without `refresh_code` -> `health_refreshes_the_code_index_as_refs_does`.
- [ ] AC-2 -- rows 4-5's source calls `refresh_code(`, `require_complete(`, `code_refs(`, `maps(`, never `read_tree(`, `collect_rust(`, `collect_ron(`, `code_symbols(`. M: the walk revived -> `health_reads_the_code_index_not_the_tree` (a source scan).
- [ ] AC-3 -- X3. A temp copy of `fixtures/code-a`, `[[health.bound]] kinds = ["law"]`, `crates/tally/src/broken.rs:4` rewritten to `// @implements LAW-DEFAULT` (no other marker names it; `docs/spec/limits.md:16`): row 5 lists `LAW-DEFAULT` with that marker, `binding` and `reason` `cannot_verify`; `spec unmapped crates/tally/src/broken.rs` reads `units 2`, `mapped 1`, `broken` unmapped. M: `cannot_verify` binds -> `cannot_verify_marker_binds_nothing`.
- [ ] AC-4 -- parity (TE-2): on `fixtures/code-a`, a rule per relation set, every selected node meets "What binds"' parity with `spec refs <node> --json`. M: Health's own copy of the predicate admitting `cannot_verify` -> `unbound_is_refs_parity_on_code_a`.
- [ ] AC-5 -- `maps` is `unmapped`'s: `code_refs.rs`, `symbol_unmapped.rs`, `symbol_reads.rs` (`unmapped_matches_expected_json`) and `symbol_reads_determinism.rs` stay green unchanged. M: `maps` ignores the ID state -> `symbol_unmapped.rs` `dangling_marker_never_maps` and this spec's `look_alike_marker_binds_nothing`, both red.
- [ ] AC-6 -- failures. A code walk gap (an unreadable `.rs` file), a bad `[code]`, an unknown `[code] frameworks` name, an unwritable data directory: rows 4-5 `cannot` with "Failures and exits"' causes, row 3 measured, exit 0. No `[code]` (`fixtures/spec-c`, a scratch `HOME`): rows 4-5 `off`, `code` null, no data directory created. M: the index opened without `[code]` -> `no_code_table_resolves_no_data_directory`.
- [ ] AC-7 -- a fresh data directory (scratch `HOME`, `[code]` present, nothing ever queued): row 4 `measured`, 0 rows; the slug's DB has no queue table afterwards. M: `SqliteQueue::open` -> `health_runs_no_queue_schema_step`.
- [ ] AC-8 -- `CHECK_CODES.len()` 36 (core `check/mod.rs:114`, pin `tests/verify_judge.rs:773`); at review `git diff --stat <base> -- crates/specengine-code Cargo.lock` is empty. M: a check code added -> the pin red.
- [ ] AC-9 -- test-engineer: `fixtures/daemon-keys.json` 46 -> 56 sets ("The document"' key sets, each mapped to its type); http `daemon_keys.rs` and UI `daemonKeys.test.ts` green together; health-left's pins change only by the new lines. M: a key dropped from the fixture -> both red.

ui-developer:

- [ ] UI-1 -- "UI (9a-1's part)": `getHealth`'s path unchanged; `pnpm lint` 0, `build`, `test` green; `HealthView.test.tsx`'s "holds no digit in the value of rows 4 to 6" unchanged. M: a key missing from a type -> `daemonKeys.test.ts`.
- UI-2..UI-6 -- moved to `health-code-ui.md` UI-1..UI-5 (Q19).

test-engineer:

- [ ] TE-1 -- temp git copies of `fixtures/code-a`, rules appended, the queue seeded through the store: (a) open, as assumed, otherwise, a discrepancy option not its recommendation, rejected, an applied update, `PR-7` (`crates/tally/src/lib.rs:13`); (b) a superseded decision, a rejected one, a node under an open proposal, `@assumes PREM-001` (`counter.rs:37`, not listed). `git diff --stat -- fixtures/` empty.
- [ ] TE-2 -- AC-4 per selected node, and with `spec unmapped --json`: a marker row 5 counts as binding sits on a mapped unit; one listed `unanchored` or `cannot_verify` maps nothing.
- [ ] TE-3 -- "Failures and exits" x rows 3-5: states, causes, stderr, exit 0; an md walk gap exit 2; the queue's causes fail row 4 alone.
- [ ] TE-4 -- RD-8's probe, RD-9's determinism over every TE-1 scenario; a repeated call re-derives nothing (AC-1).
- [ ] TE-5 -- `fixtures/spec-c` both `off`; the new sources hold no fixture prefix, kind, status word, `cargo`, `nextest`, `bevy` or framework name, and join `NEW_MODULES` (`code_refs_walk.rs:647`, 13); `proposal_genre.rs`, http `no_domain.rs` green.
- [ ] TE-6 -- AC-9.
- [ ] TE-7 -- pilots, manual (`#[ignore]`, release, `--timeout`, scratch `HOME`, configs outside): per-row counts; CLI and route wall time, first call and a repeat, against AC-10's 2 s and `noop_ms` ≤ 200; read-only proved; no pilot name or path committed.
- [ ] TE-8 -- every M above turns red.

spec-writer:

- [x] SW-1 -- 2026-10-10: this spec, `health-drift.md`, ADR-0047 and its canon. 2026-10-16: the refresh, `health-code-ui.md`, ADR-0051 and its canon (`architecture-code.md#health`, `architecture.md#control`, `symbol-index.md`, `symbol-reads.md`, `code-bindings.md`, `health.md`, `serve.md`), `health-drift.md`'s note, 08 s2; `export index`, `check` clean.
- [ ] SW-2 -- at shipping, bytes measured 2026-10-16 (caps: Tier 2 12 288, Tier 1 10 240): new Tier 2 `docs/canon/health-code.md`, rows 4-5's contract (~7-8 KB), linked from `health.md` (8 530: "spec health", "Route", "Known limits", "Tests" 46 -> 56); `serve.md` (9 290: "Concurrency"); `architecture-code.md` (11 004) only where the build differs; `spec-check.md` (12 270: :91, net ≤ 0); the CLI README (9 790: :26, :36), core (9 968: :15, ≤ 272 B), http (10 201: :58, :77, :81, ≤ 39 B); the store README (10 221), `ui/README.md` untouched; `docs/index-canon.md` (8 793) gains a line; 05:178, :203; 06:103, :146; 07:104, :114, :147; 08 s2, :123, :124; `CLAUDE.md` State (5 786 of 16 384).

## Risks

- **Fresh data directory**: the refresh creates the slug's DB first; `open_existing` promises an empty read without queue tables for `stored_rows` and `counts` only (`queue/state.rs:293-298`): AC-7.
- **Cost** (S10): each entry refreshes both indexes (BLAKE3 per code file, the changed ones re-derived: `symbol-index.md` "Cache key and cross-file pass"; `noop_ms`, `one_file_ms` ≤ 200; 05:156) and reads the queue in the project's turn; the first builds the whole code index (`full_ms`) there. TE-7 measures.
- **Flood**: without markers row 5 lists nearly every selected node, under X3 each broken item's node too (the pilots' `cannot_verify` rate unmeasured); reasons and mentions keep it readable.
- **Rebase**: `refs.rs`, `symbols.rs`, `unmapped.rs`, `code_index.rs`, `NEW_MODULES` are 10a-2's and 8a-1's first.
- **Genre**: a shipped spec code assumes reads `archived`; section selection is new; queue state is an input. Pinned: `fixtures/code-a`, `build_graph.rs`, `Cargo.lock`.

## Out of scope

Health's parts (9a-2); follow-up tasks, retargeting markers (06:103, Q10); inbox order by `@assumes` (08:123); row 6 (9b); MCP, skills; the home; check findings; the gate; per-node `needs:`, evidence levels (05 s5.5); symbols, containment, code roles; a watcher; a clock; other worktrees; generated types; project status words; path targets in `in_question`.

## Open

| Q | Working answer | The other answer costs |
|---|---|---|
| Q1-Q6 (2026-10-10) | split 9a/9b (Q1); ADR-0047 shared (Q2); `[[health.bound]]`, none: not measured (Q3); "What binds" (Q4); eight states (Q5); rows fail alone (Q6) | rows 4-5 wait for 4a…8b; `needs:` everywhere; all "bound"; pending hidden; row 3 hidden |
| Q8 code (re-decided, ADR-0051) | the index the call refreshes, as `refs` | `collect_*` revived, a second reader, a full parse per entry |
| Q9 purity (re-decided) | read-only but the data directory's two indexes; the queue by `open_existing`; never the lock, git, events | pure: row 4 impossible |
| Q10, Q11 | follow-ups out; provisional types | Health writes tasks; a type generator |
| Q12 row 4's unit | a stored marker, no binding filter | a filter hides an `@assumes` in a body |
| Q13 row 5's predicate | per node; `maps`, extracted from `unmapped`, called by both; no containment, no new SQL | `unmapped()`: exit 2 to unpick, every symbol read, rows per file; own SQL: a second judge |
| Q14 X3 | `cannot_verify` binds nothing, listed with its reason; `ron_ambiguous` binds | two answers to "is this node's code there?" |
| Q15, Q16 `[code]`, failures | `CodeConfig::from_toml`: absent `off`; an error, a failed refresh, a gap, an unknown word: rows 4-5 `cannot`, row 3 measured, exit 0 | `refs::code_config`'s exit 2: row 3 lost |
| Q17 refresh notes | stderr only; JSON `notes` row 3's | notes mislabelled in the UI, or a new key set |
| Q18 row 6 (9b) | `health-drift.md`'s dated note: 7a's `sync` judge over the index, decided at 9b's refresh | a full walk per entry beside the refresh |
| Q19 split | 9a-1 with the types and key pins (the fixture's keys change); 9a-2 the parts, after | one L slice; or the local UI suite red between them |
| Q20 the ADR | ADR-0051, amending ADR-0047 | an accepted record edited after acceptance |
| Q21 frozen bytes | no `crates/specengine-code` edit | bytes 5a-2's TE-1 describes move before 5b-1 |

Q1-Q11 but Q7 (2026-10-10) and Q8, Q9, Q12-Q21 (2026-10-16, the refresh against shipped 6a-2, 6a-3, 10a-1) decided under the owner's pre-approval, review open; Q7: `health-drift.md`. S1-S15: the refresh's stale points, fixed where cited. WA-n: the 2026-10-10 analysis' working answers, WA-4 (one fresh walk) withdrawn; "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs** (shipped, S11): `code-refs`, 6a-2 (the index), 6a-3 (`place`, `unmapped`), 10a-1 (the refresh's `known_frameworks`), the queue.
- **Order** (08 s2): 10a-2 -> 8a-1 -> 9a-1 -> 9a-2 -> 12a; 9b after 4a and 7a (Q18).
- **Interplay**: 4c-1's `family()` rejudges `decides()` (`drift` does not decide); 9b shares the row shape, keys, parts.

## Implementation

Not started. To record: modules, TE-7's figures, deviations with reasons.
