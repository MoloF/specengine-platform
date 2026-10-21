---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-http, ui, fixtures]
ref: Phase 3 slice 9b-1 of RM-0009, split 2026-10-21 from the 9b analysis (Q-A..Q-K decided under the owner's pre-approval, review open); 05 s5.3; 06 s4, s7; 08 s2; 1 iteration
shipped: 2026-10-21
adrs: [ADR-0047, ADR-0051, ADR-0046, ADR-0044, ADR-0042]
---

# health-drift (9b-1): spec health reports plain verify's drift

## Why

Health's row 6 (06 s7: drift) read "Not measured yet" / "Phase 3": the owner saw drift only through `spec verify` or a raised `drift` item. He asked to see it where he already looks: the answer `spec verify`, `check_binding` and the reads' `sync` give for the same tree, never a verdict or a hold, stale test evidence (8b) beside it. ADR-0047 decides the row, ADR-0051 the one handle, ADR-0046 the judge over the refreshed index, ADR-0044 computed evidence, ADR-0042 verify's verdicts and exits; no new ADR (Q-B).

Delivered: `spec health [--json]` and its route gain `drift`, plain verify's judge run in-process over the code index the call already refreshed, the seam widened to every lock key and stored marker file; the daemon keys; the UI's provisional types and mocks (`off`), nothing drawn. 9b-2 `docs/features/health-drift-ui.md` draws row 6.

## Description and interactions

- **Canon**: `docs/canon/health-drift.md`; `health-code.md` "Text" (`code:` now after row 6's summary).
- **Owner**: each drifting binding, verify's counts and the stale evidence in `spec health` and its route; settling stays on a terminal. Agents keep `spec verify`, `check_binding`, `sync`: no MCP or plugin change.
- **Unchanged**: `spec verify`'s text, JSON and exits, byte for byte; rows 3-5's lines; `spec check`, the hook, CI, the gate, `spec.lock`, the queue, events. Health raises no drift item.
- **Decided** (Q-A..Q-K): recorded as the canon's facts; the split (Q-G), no ADR (Q-B), no new trigger (Q-F).

## Data

### `drift`

`spec health --json` is `{documents, amendments, notes, code, assumes, unbound, drift}`; `drift` (`HealthDrift`) holds 6 keys in this order; every key present, absent `null`, uncut (contract: `docs/canon/health-drift.md` "Document"):

| Key | `measured` | `off`, `cannot` |
|---|---|---|
| `state` | `"measured"` | `"off"` (no `[code]`), `"cannot"` |
| `causes` | `[]` | the cause lines, each without `cannot: ` |
| `counts` | verify's `counts` verbatim, `{rows, ok, fix, check, cannot, pre_existing, skipped}`; `pre_existing` always 0 | `null` |
| `rows` | verify's rows whose `class` is not `null` (never `ok`), its 17 keys in its order, uncut | `null` |
| `tested` | verify's `tested` entries not `verified` (`TestedRow`), by node bytes; `[]` none; `null` blocks not judgeable | `null` |
| `notes` | the row's own notes without `note: `: `tested: <cause>` | `[]` |

The **example tree**: `verify_common::golden` (`fixtures/code-a`, `fixtures/spec-lock/code-a.lock` committed as `spec.lock`) plus `CAP: u32 = 10;` to `12;` in `crates/tally/src/counter.rs`, uncommitted: `counts` `{"rows":39,"ok":8,"fix":19,"check":6,"cannot":6,"pre_existing":0,"skipped":2}`, 31 non-`ok` rows (LAW-BOUND's `code_ahead` holds `lock` and `levels`), `tested` `[]`. An invalid lock: `` {"state":"cannot","causes":["spec.lock:1: unclosed array, expected `]`"],"counts":null,"rows":null,"tested":null,"notes":[]} ``.

### Text and stderr

Rows 4, 5, 6's lines; their summaries; `code:`; row 3's last line (canon `health-drift.md` "Text"). Row 6's lines are `drift | ` + verify's printed row blocks and non-`verified` `tested |` lines; its summary on the example tree `drift: 19 fix, 6 check, 6 cannot, 8 ok; 2 skipped`, else `drift: off|cannot: <cause>`. stderr: `note: drift: <cause>`, `note: drift: tested: <cause>`, escaped as verify's (rows 4-5's are not: a 9a follow-up).

### Exit and route

Exit unchanged (0, or row 3's 2), never 1 under any `[verify] mode` (plain verify exits 2 on the example tree). The route: the CLI's bytes, any row-6 state a 200, verify's read-only git spawned in the project's turn.

### Daemon keys

`fixtures/daemon-keys.json` 68 to 74 sets: `HealthReport` 7 keys, `drift` last; after `TestedRow` (reused): `HealthDrift` 6, `DriftCounts` 7, `DriftRow` 17, `DriftMarker` 2, `DriftLock` 3, `DriftLevels` 3. The anchor and its places stay unpinned.

### UI provisional types (9b-1)

ADR-0033's exception, citing this "Data": the types of canon `health-drift.md` "UI"; mocks `off` everywhere; nothing drawn.

## Rules and edge cases

- WHEN plain `spec verify --json` on the same tree answers without a run cause, `drift.rows` SHALL be its `rows` without `ok`, byte for byte, `drift.counts` its `counts`, `drift.tested` its `tested` without `verified`.
- WHEN `spec.lock` is absent, row 6 SHALL be measured, every binding `unlocked` (the flood).
- WHEN `[verify] mode` is `enforce` or `enforce-introduced`, Health SHALL exit as without it, with no mode, verdict, hint or mode note.

### Degrade matrix

Exit 0 and a 200 in every case but row 3's config or walk (exit 2, a 503, rows 4-6 unrun). The table with its causes: `docs/canon/health-drift.md` "Failures".

- **Rows 4-6 alike**: no `[code]` (2): `off`, the same sentence. A bad `[code]` (1): `cannot`, row 6 plain's lines and no (8), nothing opened. An unknown framework (1), HOME, the data directory, the md refresh, `refresh_code` (8), a walk gap (3): `cannot`, row 6 listing that cause and every other plain lists, in its order.
- **Rows 4-5 only**: `code_refs`, an unknown stored word, the queue, `[health]`.
- **Row 6 only**, `cannot`: a bad `[verify]` (1); no worktree, no `git`, a git failure (4); `spec.lock` unmerged (5), unreadable or invalid, format 1, another recipe (6); the selection's store read (8); a file changed twice (3). Blocks not judgeable: `measured`, `tested` `null`, note `tested: <cause>`.

## Acceptance criteria

CLI tests in `crates/specengine-cli/tests/health_drift/mod.rs`, mounted from `tests/health.rs`. Mutations M1-M6: 6 of 6 killed; M5 also as M5b (a `hint: … blocked` line after row 6's summary), the same test red.

- [x] RD-1: plain's rows minus `ok`, its counts, `tested` minus `verified`, `causes` its `cannot`. M1 (an `ok` row listed) → `drift_lists_no_ok_row`; M2 (the rows kept to a node set, `skipped` 0) → `health_drift_equals_plain_on_every_corpus`; M6 (a `verified` entry listed) → `drift_tested_lists_only_unverified`.
- [x] RD-2: no walk, one refresh unless a selected file changed; `code.parsed` 0 after `spec index`. M3 (`verify_as` or `read_tree` called) → `health_reads_the_code_index_not_the_tree`.
- [x] RD-3: the degrade matrix, exit 0 under each mode, no `mode`, `verdict`, `blocked`, `hint:`. M4 (a run cause as 0 or `[]`) → `a_run_cause_is_drift_cannot_never_zero`; M5 (exit 1, a verdict, mode or hint) → `drift_never_exits_1_or_carries_a_verdict`.
- [x] RD-4: read-only on the example tree (`health_writes_no_lock_queue_row_or_event`).
- [x] RD-5: the verify, bindings, bundle, tested and impact goldens pass unedited; rows 3-5's lines unchanged, `code:` now after row 6's summary.
- [x] RD-6: `drift_text_is_verify_lines_prefixed`.
- [x] RD-7: http `ac_r6_every_health_is_the_clis_document_byte_for_byte`, `a_row_that_cannot_be_measured_is_still_a_200`.
- [x] RD-8: determinism (`health_code_rows_are_byte_identical_across_roots_and_order`); no `git` on `PATH`: row 6 `cannot`.
- [x] TE-1: parity over five corpora (golden, anchor-states, verify-tests with its tested lock, verify-ecs, a walk gap); fixtures untouched.
- [x] TE-2: the matrix under `observe`, `enforce-introduced`, `enforce`; `fixtures/spec-c` row 6 `off`.
- [x] TE-3: read-only and determinism over TE-1's trees.
- [x] TE-4: genre; `code_refs_walk.rs` `NEW_MODULES` 18.
- [x] TE-5: daemon keys 74, http `daemon_keys.rs` and UI `daemonKeys.test.ts` together.
- [x] TE-6: `health_drift_cost` (`#[ignore]`), figures in "Implementation".
- [ ] TE-7 (owner's): CLI and http `health_code_pilot.rs` gain row 6's state, counts and ms; a warm Health read ≤ 2 s per pilot.
- [x] TE-8: M1-M6 each turn their test red.

Not covered by tests, no deterministic trigger: the selection's store-read failure (8), a file changed twice (3), a queue cause leaving row 6 measured.
- [x] UI-1: the provisional types and mocks; lint, build, test twice.
- [x] SW-1, SW-2: this spec, the 9b-2 draft, RM-0009; at shipping canon `health-drift.md` (new) and six amended, the CLI and http READMEs, `CLAUDE.md`, 06-08; `export index`, `check` clean.

## Out of scope

`--changed`/`--staged`, Pre-existing, mode, verdict, cascade, delta, hints, `ok` rows; per-node views (RM-0011); drawing row 6 (9b-2); UI actions, MCP, plugin, other worktrees, watchers, SSE; plain `spec verify` changes; index speed (RM-0020).

## Implementation

| Module | What it does |
|---|---|
| CLI `health_drift.rs` (new) | `HealthDrift`; `drift_row(project, text, git, CodeShare)`: `plain_reads`, `judge_every_key`, `tested_entries` (every block), `concluded` (no mode), the projection; `drift_lines`, `drift_summary`, the notes |
| CLI `bindings.rs` | `read_selection` (one snapshot, BLAKE3 recheck, one re-refresh, then (3); `set_files` uses it, unchanged); `KeyScope`, `KeysJudged`, `judge_every_key` (every lock entry's and stored marker's file, the uncut lock, no node filter), `every_key_file` |
| CLI `health_code.rs` | `CodeShare`, `SharedIndex`, `Opened`; `read_code` split into `open_code` (the kept handle; causes (1), (3), (8) in plain's order) and `read_stored` (`code_refs`, rows 4-5 alone); `code_line` |
| CLI `verify.rs` | `CountsJson::of` (verify's JSON too, bytes unchanged), `counts_json`, `moved_ok`, `moved_suffix` (verify's summary too); `row_block`, `VerifyOutcome::notes` `pub(crate)` |
| CLI `health.rs` | `HealthRequest {git}`, `health(&Env, &Globals, &HealthRequest)`, `HealthOutcome.drift`, the key, the text order |
| CLI `lib.rs`, `main.rs`; http `app.rs` | the module, re-exports, Health's stderr gaining row 6's notes; both callers pass `process_git(env)` |

Deviations (accepted at review):

1. `Counts` and `counts()` stay private (the draft: `pub(crate)`); `counts_json`, through `CountsJson::of`, is the one door.
2. A second refresh's notes are dropped: the first refresh's stand, no duplicate line (canon "Known limits", with the mixed snapshot).
3. A failing shared step lists every cause plain lists (an invalid lock beside HOME), `causes` being plain's `cannot:` lines; the draft said "the same cause".
4. A bad `[code]` gives no (8): nothing is opened, so no data-directory cause plain could add.
5. Row 6's stderr goes through `escape_controls`, as verify's; rows 4-5's does not (a 9a follow-up).

Cost (TE-6, `health_drift_cost`, release, warm, median of 5, ms; "Data"'s example tree plus N unrelated committed `.rs` files):

| N | plain `spec verify` | Health | Health, no lock | Health, row 6 off (no git) | row 6's share |
|---|---|---|---|---|---|
| 1 000 | 72 | 59 | 47 | 26 | 33 |
| 10 000 | 380 | 242 | 230 | 208 | 33 |

A6 holds: row 6's share stays below plain verify and does not grow with unrelated files (it reads only lock-entry and marker files); a warm Health read stays ≤ 2 s.

Tests: nextest workspace 3 848 of 3 848 (63 skipped); ui 2 263 of 2 263; fmt, clippy (probes too), export, check, parity and the eval policy tests (86 of 86) clean.
