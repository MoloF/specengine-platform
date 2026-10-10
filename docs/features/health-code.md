---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui, fixtures]
ref: Phase 3 plan 2026-10-10, slice 9a (WA-1..WA-13 but WA-8; Q1-Q11 but Q7, owner pre-approved, review open); 05 s5.3, s5.5; 06 s3, s7; 08 s2
adrs: [ADR-0047]
---

# health-code: Health shows code assuming a decision and nodes no code binds

## Why

Health's rows 4-5 (06 s7) read "Not measured yet". `// @assumes PR-…` is an agent's only trace of work on a working answer (06 s3.2); nothing shows it once the owner decides otherwise, rejects or has not decided (ADR-0012's cost). Slice 1 left the rows here (`docs/canon/code-bindings.md` "ID state"). Visibility, never a gate.

Slice 9 split (Q1): 9a this; 9b `health-drift.md`, row 6. **In scope:** `spec health [--json]` and its route gain `code`, `assumes`, `unbound`; `[[health.bound]]`; ADR-0047 (`architecture.md#control`, `#universal`; `architecture-code.md#health`); Health's rows 4-5, types, mocks; a corpus-wide marker pass shared with `spec refs`.

## Description and interactions

- **Crates** (WA-12): core the `[health]` reader, matcher, row-4 judge (pure); store nothing new expected (`get`, `list_readable`, `git_common_dir: None`); CLI `health.rs` over `code_walk::read_tree` and a corpus-wide pass lifted from `refs.rs` (`collect_rust`, `collect_ron`); http tests; ui the parts. No new crate or dependency; `CHECK_CODES` 35.
- **Unchanged**: `spec refs`' rules and bytes; row 3 byte for byte; `spec check`, hook, CI, gate, `INDEX_FORMAT`, queue schema, exit codes. **Writes**: the md index refresh only (WA-3).
- **Overridden canon lines**, fixed at shipping (SW-4): `health.md` "spec health" inputs; `serve.md` "Concurrency" (`health`, refreshing none); `spec-check.md:91`; 05:184, :209; 06:103, :146; 07:103, :111, :144; 08:123, :124.

## Data

### The document

WA-1. `spec health --json`, the route's body too: `{documents, amendments, notes, code, assumes, unbound}` (9b adds `drift`). `code`: `{files, rs, ron, skipped}` as `spec refs` counts the walk, null when it did not complete. A row: `{state, causes, counts, rows}`; `state` `measured`, `off` (no `[code]`), `not_configured` (row 5: no rule) or `cannot`; `causes` the lines behind any other state, else `[]`; `counts`, `rows` null unless `measured`, never 0. Every key present, absent null; uncut.

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
- **Candidates**: live (`is_live`) nodes with an ID in a file whose front-matter reads. A document by its own fields (kind: `kind:`, else its ID's prefix kind); a `{#ID}` section by its ID's prefix kind and its document's class, path, front-matter (new: check rules judge no section).
- No rule: `not_configured`, cause ``no [[health.bound]] rule: which nodes must be bound is the project's to declare``.

### What binds

WA-7, Q4. Bound for a rule selecting it: a marker of the rule's `relations` names it with `id_state` `resolved`, `alias` or `ambiguous` (each holder) and binding `item`, `module`, `ron` or `cannot_verify` (`code-bindings.md`); unbound when some selecting rule finds it so. `rev_state`, levels and the lock never matter (row 6's). A document's marker binds none of its sections, a section's not its document. Never binding: mentions, `unanchored`, `homoglyph` (nor the fix's node), `queue`, `dangling`, `other_project`; `@assumes` unless listed. **Parity**: unbound for a rule ⟺ `spec refs <node> --json` lists no marker of its relations meeting this.

### Row 4: `assumes`

WA-5, Q5. Judged: (a) every marker whose `id_state` is `queue`, any relation; (b) every `@assumes` naming a node (`resolved`, `alias`, each `ambiguous` holder). The queue: the slug's database by `open_existing` (absent: empty), every repository (SW refinement): `get` by exact ID, `list_readable` for (b).

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
 "counts":{"rules":2,"selected":5,"bound":4,"unbound":1},
 "rows":[{"id":"LAW-CAP","kind":"law","title":"Cap","path":"docs/spec/counter.md","line":21,"rules":[1],"mentions":1,
   "markers":[{"path":"crates/tally/src/counter.rs","line":44,"column":8,"relation":"assumes","id_state":"resolved","binding":"item","reason":"relation"}]}]}
```

Rows by path bytes, line. `rules`: 1-based indices (written order) of the rules it fails; `mentions`: those naming it (ADR-0016's adoption signal); `markers`: those naming it without binding, by path, line, column, `binding` `spec refs`' word, `reason` the first of `homoglyph`, `unanchored`, `relation`.

### Text

Row 3's lines, then:

```text
assumes | otherwise | PR-0042 | crates/tally/src/counter.rs:40:8 | assumes | question applied {"answer":"No"} | record DEC-0003 docs/decisions/DEC-0003.md
assumes | unknown | PR-7 | crates/tally/src/lib.rs:13:4 | implements
unbound | LAW-CAP | law | Cap | docs/spec/counter.md:21 | rules 1 | mentions 1
  assumes | crates/tally/src/counter.rs:44:8 | resolved | item | relation
assumes: 2 rows of 3 markers: 0 pending, 0 as_assumed, 1 otherwise, 0 applied, 0 rejected, 1 unknown, 0 archived, 0 in_question
unbound: 1 of 5 selected nodes, 2 rules
code: 8 files (.rs 7, .ron 1), skipped 0
spec health: 6 documents, 0 unapplied amendments
```

Row 4's tail: `pending` `<kind> <status>`; `as_assumed`, `otherwise` `<kind> <status> <choice>[ | record <id> <path>]`; `applied` `<kind> applied`; `rejected` `<kind> rejected: <reason>`; `archived` `<node> | <path>:<line> | status <status>[ | successor <X>]`; `in_question` `<node> | <path>:<line> | under <IDs, ", ">`. Fields one-lined; `code:` only when walked; the last line unchanged. A row not measured: `<row>: off|not configured|cannot: <cause>` per cause, `cannot` also on stderr as `note: <row>: <cause>`.

### Failures and exits

WA-2, Q6. Today's exit 2 stays (discovery, base config, unreadable root, md walk gap); other causes fail only their rows, exit 0, never 1, verbatim as the failing command words them:

- no `[code]`: both `off`, code-refs' sentence less `spec: `, `code` null; a bad `[code]`, a code walk gap: both `cannot`, `code` null; HOME, the data directory, the md index refresh: both `cannot`;
- the queue (newer schema, a corrupt row, `PR` in `[ids]`): row 4 `cannot`, whole (SW refinement: (b) reads it too);
- no rule: row 5 `not_configured`; a bad `[health]`: row 5 `cannot`.

### Route and UI

WA-9, WA-10, Q11.

- Route unchanged (`health.md` "Route"): any row state a 200, a 503 only on exit 2.
- `ui/src/health/`: row 4 "Code assuming a decision", row 5 "Unbound nodes" become parts beside row 3's, same `getHealth`; row 6 waits for 9b. Four states apart: being measured (skeleton, `aria-busy`); not measured (`off`, `not_configured`: the cause); could not be measured (`cannot`, a 503: causes, a Retry); measured, alone with a digit. Zero: "None" and what was measured over ("over <markers> markers in <files> code files", "over <selected> selected nodes").
- Daemon order, 10 (`SHOWN_OPEN`), "Show all <n>" focusing the first added row; node IDs link to the tree, PR IDs to the inbox card, paths plain; state words through one map (`labels.ts`), unknown ones raw. Re-read also on `proposal.rejected` on screen; never `.created`, focus, an interval, `task.*`.
- **Exceptions to `.claude/agents/ui-developer.md`** (ADR-0033): provisional types in `ui/src/api/provisional.ts` citing `docs/features/health-code.md` "Data"; no type generation (none exists).

## Rules and edge cases

- WHEN a row cannot be measured, Health SHALL show its causes, no count, the other rows and exit unchanged.
- WHEN run from two roots sharing a data directory, another cwd, reverse creation order or CRLF copies, stdout, stderr and JSON SHALL be byte-identical (one tree, config, queue).
- Nothing blocks (ADR-0012, ADR-0047); read-only (RD-8); genre (WA-11, TE-5).

## Acceptance criteria

rust-developer, M: mutation -> the test turning red (CLI `tests/health.rs` unless named; RD-7 in 9b):

- [ ] RD-1 -- M: keys dropped without `[code]` -> `health_code_keys_present_without_code_table`; row 3 altered -> `amendments_unchanged_by_code_rows`.
- [ ] RD-2 -- M: a code walk gap exits 2 -> `code_walk_gap_is_rows_cannot_row_3_measured`; `cannot` with a count -> `a_row_that_cannot_be_measured_has_no_count`; an exit 1 -> `health_never_exits_1`.
- [ ] RD-3 -- M: `PR-7` matched loosely -> `short_queue_id_is_unknown`; a discrepancy by label -> `discrepancy_option_against_recommendation_index`; the working-answer test inverted -> `working_answer_choice_is_as_assumed`; `SqliteQueue::open` -> `health_creates_no_queue_database`.
- [ ] RD-4 -- M: `target_ids` skipped -> `assumed_node_under_an_open_proposal_is_in_question`; the successor dropped -> `superseded_decision_shows_its_successor`; a status word in a new source -> `ac_r7_the_new_sources_write_no_class_status_kind_or_link_type`, extended to them.
- [ ] RD-5 -- M: all selected by default -> `no_rule_is_not_configured_never_all_unbound`; `health` not name-checked -> `binding_rule_table_is_name_checked`; an unknown key taken -> `binding_rule_unknown_key_is_refused`; a section by its document's kind -> `section_selected_by_its_own_prefix_kind`.
- [ ] RD-6 -- M: mentions bind -> `mentions_never_bind`; `unanchored` binds -> `unanchored_marker_binds_nothing`; a look-alike binds the fix's node -> `look_alike_marker_binds_nothing`; a document's marker covers its sections -> `a_document_marker_binds_no_section`; `@assumes` binds -> `assumes_marker_binds_nothing_by_default`; Tier 3 selected -> `archived_nodes_are_never_unbound`. Parity: `unbound_is_refs_parity_on_code_a`.
- [ ] RD-8 -- `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, `spec.lock`, the queue dump, the event `seq` unchanged. M: an event written -> `health_writes_no_lock_queue_row_or_event`.
- [ ] RD-9 -- M: walk order leaks -> `health_code_rows_are_byte_identical_across_roots_and_order`.
- [ ] RD-10 -- http `tests/health.rs`, the body the CLI's bytes. M: `cannot` as a 503 -> `a_row_that_cannot_be_measured_is_still_a_200`; `ac_r6_*`, `door.rs`, `turn.rs` green.
- [ ] RD-11 -- M: `spec check` reads `[health]` -> `check_output_equal_with_binding_rules`; a new crate or dependency -> `build_graph.rs`.
- [ ] RD-12 -- M: the shared pass changes `refs` -> code-refs' `refs_is_byte_identical_across_roots_and_order`, the `code_refs.rs` goldens.

ui-developer (rows 4-5; row 6: `health-drift.md`):

- [ ] UI-1 -- `getHealth`'s path unchanged; "Data"'s types cite it. M: a key missing -> `daemonKeys.test.ts`.
- [ ] UI-2 -- four states apart per row; `HealthView.test.tsx`'s "holds no digit in the value of rows 4 to 6" narrowed to row 6. M: 0 drawn for `cannot` -> `HealthView.test.tsx`.
- [ ] UI-3 -- M: a sort in the UI -> `health.smoke.test.tsx`; a state word compared outside the map -> `policy.test.ts` (health-left AC-U6 extended, not loosened).
- [ ] UI-4 -- M: the `proposal.rejected` trigger dropped -> `health.live.test.tsx` (its "never" list loses `.rejected` only).
- [ ] UI-5 -- mocks: harbor-sim every row-4 state, row 5 measured; ledger-api `off`; `empty`, `large` (3 000 unbound), `cannot-check`, `error`, `slow`; new `code-cannot` (rows 4-5 `cannot`, row 3 measured). M: a mock off its type -> `mocks/health.test.ts`.
- [ ] UI-6 -- `pnpm lint` 0; `build`; `test` twice; no new package.

test-engineer:

- [ ] TE-1 -- temp git copies of `fixtures/code-a`, rules appended, the queue seeded through the store: (a) open, as assumed, otherwise, a discrepancy option not its recommendation, rejected, an applied update, `PR-7` (`crates/tally/src/lib.rs:13`); (b) a superseded decision, a rejected one, a node under an open proposal, `@assumes PREM-001` (`counter.rs:37`, not listed). `git diff --stat -- fixtures/` empty.
- [ ] TE-2 -- row 5's parity with `spec refs --json` per selected node, robust to 6a's discriminator.
- [ ] TE-3 -- "Failures and exits" x rows 3-5: states, causes, stderr, exit 0; an md walk gap exit 2.
- [ ] TE-4 -- RD-8's probe, RD-9's determinism over every TE-1 scenario.
- [ ] TE-5 -- `fixtures/spec-c` both `off`; the new sources hold no fixture prefix, kind, status word, `cargo`, `nextest`, `bevy`; `proposal_genre.rs`, http `no_domain.rs` green.
- [ ] TE-6 -- `daemon_keys.rs`: `HealthReport` + 3 keys, exactly 10 new sets ("Data"'s objects), each mapped to its type; health-left's pinned tests change only by the new lines.
- [ ] TE-7 -- pilots, manual: `#[ignore]`, release, foreground, `--timeout`, scratch `HOME`, configs copied outside; per-row counts, CLI and route wall time; read-only proved; no pilot name or path committed.
- [ ] TE-8 -- every M above turns red.

spec-writer:

- [x] SW-1 -- this spec, `health-drift.md`. SW-2 -- ADR-0047; `architecture.md#control`, `#universal`; `architecture-code.md#health`. SW-3 -- dated notes: 08 s2, `symbol-index.md`, `spec-verify.md`, `verify-tests.md`. SW-5 -- `export index`, `check` clean.
- [ ] SW-4 -- at shipping: Tier 2 `docs/canon/health-code.md` (`health.md` cannot absorb the rows); `health.md` "spec health", "Route", "UI", "Known limits", "Tests"; `serve.md` "Concurrency"; the overridden lines; CLI, core, http, `ui/` READMEs; `CLAUDE.md` State.

## Risks

Genre: a shipped spec code assumes reads `archived`; section selection is new. Cost: a code parse, the md refresh, queue reads per Health entry, in the project's turn, row 3 waiting; ~1 s per 300 kLOC (05:162), TE-7 against AC-10's 2 s. Flood: without markers row 5 lists nearly every selected node; mentions keep it useful. Queue state is an input (two-root tests share a data directory). Pinned: `fixtures/code-a`, `build_graph.rs`, `Cargo.lock`.

## Out of scope

Follow-up tasks, removing or retargeting markers (06:103, Q10); inbox order by `@assumes` (08:123); row 6 (9b); MCP, skills; the home; check findings; the gate; per-node `needs:`, evidence levels (05 s5.5); 6a's store; a watcher; ages, a clock; other worktrees; generated types; project status words; path targets in `in_question` (SW refinement); re-reading on `proposal.created`.

## Open

| Q | Working answer (owner pre-approved, review open) | The other answer costs |
|---|---|---|
| Q1 split | 9a rows 4-5, table, ADR; 9b row 6 after 4a | rows 4-5 wait for 4a…8b |
| Q2 ADR | ADR-0047, shared | `#control` edited undecided |
| Q3 required | `[[health.bound]]`; none: not measured | `needs:` (05:209) in every contract; a core default (ADR-0008) |
| Q4 binds | "What binds" | mentions: all "bound"; `@assumes` as implementing |
| Q5 row 4 | eight states | 06:146's reading hides pending, `PR-7` |
| Q6 exits | one document; rows fail alone | exit 2 hides row 3 |
| Q8, Q9 | 6a's store unread; read-only, not pure | a refresh per read; row 4 impossible |
| Q10, Q11 | follow-ups out; provisional types | Health writes tasks; a type generator (ADR-0033) |

Q7: `health-drift.md`. WA-n: the analysis' working answers (2026-10-10), [A] assumed: WA-1, -2, -3 (IDs as `spec refs`: data directory, md refresh), -4 (one fresh walk), -5, -6, -7, -10, -13; [P] settled: WA-9, -11, -12. "SW refinement": the spec-writer's.

## Dependencies and ordering

- **Needs**: slice 1 `code-refs` (shipped, `code-bindings.md`); the queue (Phase 2). Nothing of 3-8.
- **Order**: may be pulled forward right after `code-refs`, before slice 3 (Q1: only slice 1 and the queue); else at 08's position before 9b. Beside slice 3, both touch `ProjectConfig`'s name-checked tables: the later rebases.
- **Interplay**: 4c's `family()` rejudges the `decides()` call (`drift` does not decide); 6a reaches row 5 through slice 1; 9b shares walk, keys, parts.

## Implementation

Not started. To record: modules, TE-7's figures, deviations with reasons.
