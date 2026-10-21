---
class: canon
tier: 2
scope: [crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-10-21
---

# Health's row 6: drift

Row 6 of Health's "What is left" (06 s7) in `spec health [--json]` and its route, after rows 4-5 (`health-code.md`): plain `spec verify`'s judgement of the working tree, run in-process over the code index the call already refreshed; its rows but `ok`, its counts, and the `[[tested]]` entries not `verified`. Architecture rule: `architecture-code.md#health` (ADR-0047, ADR-0051; the judge ADR-0046, verdicts ADR-0042, evidence ADR-0044). Health draws it with `DriftPart.tsx` ("UI").

## Rule

Visibility only (ADR-0012, ADR-0047): never verify's mode, verdict, hint, mode note or exit. Exit stays 0, or row 3's 2, under any `[verify] mode`; no row-6 state changes it, and plain verify's exit 2 on the same tree is not Health's. Health raises no drift item (06 s4: never a daemon raising), check finding or gate, and writes only the data directory's indexes, as rows 4-5: never a queue row, `spec.lock`, a git object or index, or an event. Words are verify's (states, categories, classes) and the model's `TestedState`. Agents keep `spec verify`, `check_binding` and the reads' `sync`: no MCP tool, resource or plugin change.

## Reading

`health(&Env, &Globals, &HealthRequest {git})`: `git` is the caller's `GitEnv` (the CLI and the route pass `process_git(env)`; the library never calls it). Row 6 runs after rows 4-5, from the same config bytes:

- **No `[code]`**: `off`, rows 4-5's sentence as its cause; nothing read.
- **Plain's reads**: verify's own `plain_reads`: `[code]` and `[verify]` (1), git (4: no worktree, no `git`, a failure), the lock (5: unmerged; 6: unreadable, invalid, format 1, another recipe).
- **One handle** (`health_code.rs` `CodeShare`): rows 4-5's `open_code` (`open_index`, `indexed_in`, `refresh_code`, `require_complete`) stays open for row 6 as `SharedIndex {index, input}`. Rows 4-5's own read of it, `read_stored` (`code_refs`), is separate: its failure leaves row 6 measured. A failing `open_code` step is row 6's cause too, in plain's order: an unknown `[code] frameworks` name (1), the walk gap (3), any other (8: HOME, the data directory, a refresh). A `[code]` that does not read opens nothing (`Unread`): plain's read names it.
- **Whole-key selection** (`bindings.rs` `judge_every_key`, beside `judge_set`): from one read snapshot of the refreshed index, the file of every `spec.lock` entry and of every stored marker, whatever its ID state (`queue`, `other_project` markers counted `skipped`, as plain counts them). The lock is uncut and no node filter (`key_set_nodes`) applies: dangling, bare `""` and `node_gone` rows stay, as plain keeps them. `read_selection` reads each file again and checks its stored BLAKE3; a mismatch refreshes the code index once more and reads the selection again; a second mismatch is a walk gap (3).
- **Judge**: plain's `judge_walk(&walked, git, None)` (no base); `tested_entries` over that partial tree, every block kept; the git session's `finish()` (its failure: (4)); `concluded` with no mode (`configured: None`, so no mode note). Never `read_tree`, `verify_as` or a walk of its own; no second refresh unless a selected file changed.

## Document

`spec health --json` is `{documents, amendments, notes, code, assumes, unbound, drift}`; `drift` (`HealthDrift`) holds 6 keys, in this order; every key present, absent `null`, uncut:

| Key | `measured` | `off`, `cannot` |
|---|---|---|
| `state` | `"measured"` | `"off"` (no `[code]`), `"cannot"` |
| `causes` | `[]` | the cause lines (plain's `cannot:` lines less `cannot: `) |
| `counts` | verify's JSON `counts`, verbatim (`counts_json`): `{rows, ok, fix, check, cannot, pre_existing, skipped}`, `pre_existing` always 0 | `null` |
| `rows` | verify's rows whose `class` is not `null` (never `ok`): its 17 keys in its order, its row order, uncut | `null` |
| `tested` | verify's `tested` entries (`TestedRow`) not `verified`, by node bytes; `[]` with no block or all `verified`; `null` when the blocks cannot be judged | `null` |
| `notes` | the row's notes less `note: `: `tested: <cause>` | `[]` |

Never 0, never partial: a run cause leaves `counts`, `rows` and `tested` `null`, never an unjudged list. A `cannot_verify` row (class `cannot`) is a row; the row state `cannot` is not. Left out: verify's `mode`, `base`, `task`, `node`, `symbol`, `verdict`, `cascade`, `delta`, `omitted`, `cannot` (it is `causes`), its hints. Without `spec.lock` every binding is `unlocked` (Check): the flood before the first accept.

Key sets (`fixtures/daemon-keys.json`, 74): `HealthReport` 7; after `TestedRow` (reused), `HealthDrift` 6, `DriftCounts` 7, `DriftRow` 17 (verify's row), `DriftMarker` (`line, rev`), `DriftLock` (`rev, commit, verified_at`), `DriftLevels` (`lock, markers, changed`). The row's anchor and place stay unpinned (verify's goldens pin them). Core's `drift::DriftRow` is the 15-key stored-evidence view, never this row.

## Text

Rows 4, 5, 6's lines; their summaries in the same order; `code:` when the index was read; row 3's last line (`health-code.md` "Text"). Row 6's lines:

- each of verify's row blocks (`row_block`), the row line prefixed `drift | `, its `  moved from …` and `  candidate …` sub-lines unprefixed, two spaces in; then each entry not `verified` as `drift | ` + `tested_line`: `drift | tested | <state>[ <category>] | <node>`. Never `pre-existing `. Through `escape_controls`, as verify's stdout: each is `drift | ` + plain's printed line.
- **Summary**: `drift: <f> fix, <c> check, <k> cannot, <o> ok[ (<m> moved)]; <s> skipped`: verify's less the verdict, the mode, `by blob` and `by ast`; `(<m> moved)` counts the `ok` rows moved (`moved_ok`, `moved_suffix`, verify's own), absent at 0. Not measured: `drift: off: <cause>` or `drift: cannot: <cause>`, one line per cause.

The example tree (`code-a`, its golden lock committed, `CAP` 10 to 12 uncommitted):

```text
drift | check | code_ahead code_changed body | LAW-BOUND | configures | crates/tally/src/counter.rs | item crates/tally::counter::CAP const_item
unbound: not configured: …
drift: 19 fix, 6 check, 6 cannot, 8 ok; 2 skipped
code: 8 files (.rs 7, .ron 1), skipped 0
spec health: 5 documents, 0 unapplied amendments
```

**stderr**, after rows 4-5's: `note: drift: <cause>` per cause of a `cannot` row (an `off` row prints none); `note: drift: tested: <cause>` when a measured row's `tested` is `null`; never in the top-level `notes`. Row 6's lines go through `escape_controls`, as verify's; rows 4-5's stderr does not (a 9a follow-up).

## Failures

Exit 0 and a 200 in each case; row 3's config or walk failing stays exit 2, a 503, rows 4-6 unrun.

| What fails | Rows 4-5 | Row 6 |
|---|---|---|
| no `[code]` (2) | `off` | `off`, the same sentence |
| a bad `[code]` (1) | `cannot` | `cannot`, plain's lines; no (8), nothing opened |
| an unknown framework (1); HOME, the data directory, a refresh (8); a walk gap (3) | `cannot` | `cannot`: that cause and every other plain lists (an invalid lock too), in its order |
| `code_refs`, an unknown stored word, the queue, `[health]` | their causes | unaffected |
| a bad `[verify]` (1); no worktree, no `git`, a git failure (4); the lock unmerged (5), unreadable, invalid, format 1, another recipe (6); the selection's store read (8); a file changed twice (3) | unaffected | `cannot` |
| `[[tested]]` blocks not judgeable | unaffected | `measured`, `tested` `null`, note `tested: <cause>` |

Causes as plain words them: `` `git` could not be run: no `git` program was found on PATH ``; ``spec.lock:1: unclosed array, expected `]` ``.

## Route

`GET /api/projects/:p/health` (`health.md` "Route"): the CLI's bytes; any row-6 state is a 200, a 503 only row 3's exit 2. The handler passes `HealthRequest { git: process_git(env) }` and spawns verify's read-only git in the project's turn (`serve.md` "Concurrency"): `rev-parse`, the unmerged check, `hash-object --no-filters --stdin-paths`, never `-w`. No query, route, event or SSE subscription.

## UI

`ui/src/health/DriftPart.tsx` draws "Drift" after "Unbound nodes" from the same `getHealth` read, with rows 4-5's frame, states, Retry and triggers (`health.md` "UI"; none of its own). Measured: "<rows> of <n> bindings, <skipped> skipped", a tally by class (`driftTally`), zero rows "None: every binding `spec verify` judges is in sync."; rows in the daemon's order, never sorted, 10 then "Show all <n>": class and state badges, the node linked to the tree, relation, category, detail, place, a moved anchor's old place, candidates; then `tested` by node, `[]` "None", `null` "Not judged". Words through `labels.ts` (`driftClassLook`, `testedLook`, `syncLook`), an unlisted one raw, `cannot`/`unverifiable` never ok's look. Texts, looks, mocks: `docs/features/health-drift-ui.md`. Provisional types (ADR-0033; `docs/features/health-drift.md` "Data"): `HealthDrift {state, causes, counts, rows, tested, notes}`, `DriftCounts`, `DriftRow` (17 keys), `DriftMarker`, `DriftLock`, `DriftLevels`.

## Known limits

- **Mixed snapshot**: a selected file edited during the call refreshes the code index again, so row 6 may judge a later snapshot than rows 4-5 counted; that refresh's notes are dropped (the first refresh's stand: no duplicate lines).
- **Cost**: every Health read, the daemon's trailing re-read too, spawns verify's git and reads every lock-entry and marker file, nearing plain verify (`code-reads.md` "Known limits"). Measured (TE-6, release, warm, median of 5): the example tree plus 1 000 unrelated files, Health 59 ms (row 6 off: 26), plain verify 72; plus 10 000, 242 (208) against 380. Row 6's share, 33 ms at both sizes, does not grow with files no lock entry or marker names.
- **No new trigger**: a code edit, `spec lock accept` or an evidence commit fires no event; Health is stale until "Check again", re-entering it or another trigger (`health.md` "Known limits").
- **Deviations from plain**, as `--node` has them: an unknown framework is a shared step (rows 4-6 `cannot`); a file changed twice is (3); notes are the refresh's, on stderr, never in `drift.notes`; a bad `[code]` lists no (8).

## Modules

| Module | What it does |
|---|---|
| CLI `health_drift.rs` | `HealthDrift`; `drift_row(project, text, git, CodeShare)`: plain's reads, the whole-key judge, `tested_entries`, `concluded`, the projection; `drift_lines`, `drift_summary`, the notes |
| CLI `health.rs` | `HealthRequest {git}`, `HealthOutcome.drift`, the key last, the text order |
| CLI `health_code.rs` | `code_rows(..) -> (CodeRows, CodeShare)`; `CodeShare {Off, Unread, Failed(RunCause), Open(SharedIndex)}`; `open_code`, `read_stored`; `code_line` |
| CLI `bindings.rs` | `read_selection` (the BLAKE3 recheck, one re-refresh; `set_files` too), `KeyScope`, `KeysJudged {judged, tree}`, `judge_every_key`, `every_key_file` |
| CLI `verify.rs` | `CountsJson::of` (verify's JSON too), `counts_json` (`Counts` stays private), `moved_ok`, `moved_suffix`; `row_block`, `VerifyOutcome::notes` `pub(crate)` |
| CLI `lib.rs`, `main.rs`; http `app.rs` | the module, `HealthRequest` and `HealthDrift` re-exported, stderr after rows 4-5's; both callers pass `process_git(env)` |

## Tests

CLI `tests/health_drift/` (in `tests/health.rs`): parity with plain verify over five corpora, no `ok` row, only unverified `tested`, the prefixed lines, run causes never 0, no exit 1 or verdict under any mode, the index left fresh, the caller's `GitEnv`; `health_drift_cost` (`#[ignore]`); `health_rows/` (read-only, determinism, the scan forbidding `verify_as`), `code_refs_walk.rs` (`NEW_MODULES` 18); http `tests/health.rs` (parity, a `cannot` row 6 a 200, no `git`), `daemon_keys.rs` (74 sets); UI `provisional`, `daemonKeys`, `mocks/health`, `HealthView` ("row 6: drift"), `labels`, `health.smoke`, `health.live`, `policy` (AC-U6; Health sorts only in `findings.ts`). TE-7, manual (`#[ignore]`): CLI and http `health_code_pilot.rs` (row 6's state, counts, ms). Untested, no deterministic trigger: the selection's store read (8), a file changed twice (3), a queue cause leaving row 6 measured.
