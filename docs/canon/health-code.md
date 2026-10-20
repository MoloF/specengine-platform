---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http]
owner: owner
reviewed: 2026-10-19
---

# Health's code rows: `assumes` and `unbound`

Rows 4-5 of Health's "What is left" (06 s7) in `spec health [--json]` and its route, after row 3 (`health.md`): row 4 `assumes`, code resting on a queue item, or an `@assumes` on a node archived or in question; row 5 `unbound`, the nodes the project selects that no marker binds. Architecture rule: `architecture-code.md#health` (ADR-0047, ADR-0051). Row 6 (drift) is 9b's; Health draws rows 4-5 from 9a-2.

## Rule

Visibility only: no check finding, rule or gate; nothing blocks (ADR-0012, ADR-0047). Exit 2 is row 3's alone, and no row exits 1: a step rows 4-5 cannot take is their cause, exit 0, row 3's bytes as without them. Words are the model's (`ProposalStatus`, `KindFamily`, `Choice`, `Relation`, `IdState`), never a project's (ADR-0008). Writes only the data directory's md and code indexes, as `spec refs`; never the queue (no schema step), `spec.lock`, git or an event.

## Reading

- **Config**: row 3's bytes (`locate`, `load_config`), read once for every row.
- **No `[code]`**: both rows `off`, cause ``<config> has no [code] table: the code layer is off (add [code] with roots)`` (refs' sentence), `code` null; nothing opened, no data directory resolved.
- **Else one handle**, in `spec refs`' order: `open_index` (HOME, the data directory, the slug's DB created when absent); `corpus::indexed_in(…, true)` (the md index refreshed; its `CheckInput` feeds the judge and row 5); `refresh_code(…, false)` (`[code] frameworks` included), `require_complete()`; `code_refs()` (`None`: empty). Never `read_tree`, `collect_*`, `code_symbols`, role rows, `spec.lock` or git.
- **Judging**: each stored marker's relation and binding word checked (one this build does not write: both rows `cannot`), its ID judged once by `refs::Corpus::judge`, then `symbols::maps` (`symbol-reads.md` "spec unmapped"): `Some` when the ID names nodes (`resolved`, `alias`, `ambiguous`) and the binding sits somewhere (`ron_ambiguous` too), never `unanchored` or `cannot_verify`. Each mention's binding goes through `place`. A row's `binding` is `spec refs`' word: `ron_ambiguous` reads `ron`.
- **The queue**: opened only when row 4 judges a marker, by `SqliteQueue::open_existing` (no file, or an index-only DB: empty, `queue-backup.md` "Store"), `git_common_dir: None` (every repository).

## Document

`{documents, amendments, notes, code, assumes, unbound}`, the first three row 3's (`health.md` "spec health"); every key present, absent `null`, uncut; key sets in `fixtures/daemon-keys.json` (56).

- `code` (`HealthCode`): `{files, rs, ron, skipped}`, `spec refs`' count of the stored files; `null` unless the index was read.
- A row (`HealthAssumes`, `HealthUnbound`): `{state, causes, counts, rows}`: `state` `measured`, `off`, `not_configured` (row 5) or `cannot`; `causes` the lines behind any other state, else `[]`; `counts`, `rows` `null` unless `measured`, never a 0.
- `AssumesCounts {markers, rows, pending, as_assumed, otherwise, applied, rejected, unknown, archived, in_question}`, `AssumesRow {id, state, path, line, column, relation, proposal, node}`, `AssumedProposal {kind, status, choice, record_id, record_path, reason}`, `AssumedNode {id, path, line, status, successor, proposals}`.
- `UnboundCounts {rules, selected, bound, unbound}`, `UnboundRow {id, kind, title, path, line, rules, mentions, markers}`, `UnboundMarker {path, line, column, relation, id_state, binding, reason}`.

## Row 4: assumes

The unit is a stored marker, no symbol, binding or containment filter (an `@assumes` in a function body counts). Judged:

- **(a)** every marker whose ID state is `queue` (`code-bindings.md` "ID state"), any relation: `get` by the ID exactly as written (`PR-7` never finds `PR-0007`).
- **(b)** every `assumes` marker whose ID names nodes, a row per holder: `archived` when its document is Tier 3 (`is_tier3_file`); else `in_question` when an `open` or `approved` item's `target_ids` hold its canonical ID (`list_readable`, by ID number, deduplicated); else no row.

| `state` | (a) when |
|---|---|
| `pending` | `open`; `approved`, a kind that does not decide (`drift`) |
| `as_assumed` | a deciding kind `approved` or `applied`, its record's choice the working answer or the option at its `recommendation` index |
| `otherwise` | the same, another choice or no record (a legacy row) |
| `applied` | `applied`, a kind that does not decide |
| `rejected` | `rejected` |
| `unknown` | no item of that ID |

`proposal` ((a) found): `kind`, `status`, `choice` (the stored one-key JSON), `record_id`, `record_path`, `reason` (a rejection's note), as stored, `null` when absent. `node` ((b)): canonical ID, path, line as `spec refs`' header, its document's raw `status:`, `successor` (X of `superseded-by X`), `proposals` (the `open`/`approved` items holding it). `counts.markers` counts every judged marker, an unlisted (b) one too. Rows by ID bytes, path, line, column, holder (path, line).

Row 4 alone `cannot`, whole: `PR` in `[ids]` (`<config>: <clash>`); a queue error (a newer schema, `get`, `list_readable`); an unreadable `open` or `approved` row while (b) reads them, one no marker names too, never skipped; a queue below the build's schema: `the queue stands at schema N; a queue command upgrades it`. No marker judged: the queue is not opened; `measured`, 0 rows, whatever its state.

## Row 5: unbound

**Rules**: `[[health.bound]]` in `specengine.toml`, read only here.

```toml
[[health.bound]]  # every live law
kinds = ["law"]
[[health.bound]]  # accepted canon, by code or data
paths     = ["docs/spec/**"]
classes   = ["canon"]
when      = { status = "accepted" }
relations = ["implements", "configures"]
```

- `[health]` holds `bound` alone; `ProjectConfig` only name-checks `health`: a bad rule fails no other command, `spec check` judges none. No table, an empty one or `bound = []`: `not_configured`, cause ``no [[health.bound]] rule: which nodes must be bound is the project's to declare``, never "all unbound".
- Selectors `kinds`, `classes`, `paths`, `when`: `[[check.rules]]`' grammar (`spec-check-process.md` "A rule"), at least one, ANDed, a list's items ORed. `relations`: optional, non-empty, of `implements`, `verifies`, `configures`, `assumes`; default the first three.
- Errors: the first by line, `<config>:<line>: <message>`: row 5 `cannot`. Check's messages prefixed `` `[[health.bound]]` `` (`` without a selector: give `kinds`, `classes`, `paths` or `when` ``); another key: the TOML reader's `unknown field …`.

**Candidates** (core `bound_candidates`): live (`is_live`) nodes with an ID in a file whose front-matter reads, from the md index's `CheckInput`. A document by its own fields (`kind:`, else its ID's prefix kind); a `{#ID}` section by its ID's prefix kind and its document's class, path and front-matter.

**Binding**: bound for a rule when a stored marker of one of its relations `maps` and its judged nodes hold the node; unbound when a selecting rule finds it so. No containment: a document's marker binds none of its sections, a section's not its document. Mentions, look-alikes (nor the fix's node), `queue`, `dangling`, `other_project` IDs never bind; `rev_state`, levels, the lock never matter. Parity (AC-4): unbound ⇔ `spec refs <node> --json` lists no marker of the rule's relations with such an `id_state` and `binding`.

**Rows** by path bytes, line, ID. `rules`: 1-based indices (config order) of the rules it fails; `mentions`: stored mentions whose judged nodes hold it (a look-alike's fix node too), ADR-0016's adoption signal; `markers`: every marker whose judged nodes hold it, by path, line, column, `reason` the first that applies of `homoglyph`, `unanchored`, `cannot_verify`, `relation`.

State, by precedence: `off`; `not_configured`; `cannot`, every cause (`[health]`'s, then the code's); `measured`.

## Text

AC-3's tree (`code-a`, `kinds = ["law"]`): row 3's amendment lines; row 4's rows; row 5's rows, each with its markers two spaces in; each row's summary, or `<row>: off|not configured|cannot: <cause>` per cause; `code:` only when the index was read; row 3's last line.

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

Row 4's tail: `pending` `<kind> <status>`; `as_assumed`, `otherwise` `<kind> <status> <choice>[ | record <id> <path>]`; `applied` `<kind> applied`; `rejected` `<kind> rejected[: <reason>]`; `archived` `<node> | <path>:<line>[ | status <status>][ | successor <X>]`; `in_question` `<node> | <path>:<line> | under <IDs, ", ">`. Choice and record print for `as_assumed`, `otherwise` only (a reopened item keeps both in JSON). Row 5: `rules` joined `, `, `-` for no kind or title. Fields one-lined; rows 4-5's stdout lines, causes too, through `escape_controls` (`agent-intake-documents.md` "Escaping and size"); row 3's bytes, stderr notes unchanged.

**stderr**: row 3's notes, the refreshes' notes and warnings as `spec refs` prints them, then `note: <row>: <cause>` per cause of a `cannot` row; never in JSON `notes`.

## Failures

Rows 4-5 `cannot`, `code` null, row 3 measured, exit 0: a bad `[code]` (`<config>:<line>: message`, an unknown `[code] frameworks` name too), HOME, the data directory, the md refresh, `refresh_code`, a code walk gap (`the code walk is incomplete: <gap>`), `code_refs`, an unknown stored word. A cause is the failing step's CLI line less `spec: `.

## Route

`GET /api/projects/:p/health` (`health.md` "Route"): any row state a 200, the CLI's bytes; a 503 only on row 3's exit 2. It writes the data directory's indexes in the project's turn; the first call builds the whole code index there (`full_ms`), no timeout bounding it (`serve.md` "Concurrency").

## Known limits

- **Older queue schema**: neither the daemon nor `spec index` runs a queue schema step, so after an upgrade row 4 is `cannot` until a queue command runs.
- **Flood**: without markers row 5 lists nearly every selected node, each broken item's (`cannot_verify`) too; reasons and mentions keep it readable.
- Row 5's stored order is path order, so dropping its sort goes unseen; row 4's sort is pinned.

## Modules

| Module | What it does |
|---|---|
| core `health_toml.rs`, `project_toml.rs` | `HealthConfig {bound}` (`from_toml(text, scheme, relations, default_relations)`), `BoundRule {kinds, classes, paths, when, relations, line}`; `RawFile.health` |
| core `check/health.rs` | `bound_candidates(files, rules) -> Vec<BoundCandidate {file, ord, rules}>` |
| CLI `health_code.rs` | `code_rows(env, project, text) -> CodeRows {code, assumes, unbound, messages}`: reading, the pure row-4 judge `decided`, binding, text; `RowState`, `CodeRow`, `AssumedState`, row types re-exported |
| CLI `health.rs` | `HealthOutcome.code_rows`, after row 3 |
| CLI `symbols.rs` | `maps<'s>(marker, judged) -> Result<Option<Place<'s>>, CliError>`, `unmapped.rs`' too |
| CLI `refs.rs` | `no_code_table`, `Corpus::node_line`, shared |
| store `queue.rs`, `queue/state.rs` | `index_only` (`user_version` 0, no `proposals`): the empty read; `schema_version()`, read only |
| CLI `proposals.rs` | `queue_below_schema(schema)`: row 4's cause |

## Tests

CLI `tests/health.rs`, `health_rows/` (parity, failures, read-only, determinism, genre), `code_refs_walk.rs` (`NEW_MODULES` 17); store `queue_bare_reads.rs`; http `tests/health.rs`, `daemon_keys.rs` (66 sets); UI `daemonKeys.test.ts`. TE-7, manual (`#[ignore]`): CLI and http `health_code_pilot.rs`.
