---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-model, crates/specengine-mcp, crates/specengine-http, ui, plugin, fixtures]
ref: Phase 3 slice 8b-2 (RM-0032), split from 8b on 2026-10-21; Q-A..Q-L decided 2026-10-21 under the owner's pre-approval, review open; 08 AC-8; 1 iteration
shipped: 2026-10-21
adrs: [ADR-0044, ADR-0046]
---

# tested-reads (8b-2): the reads show each node's `tested` state

## Why

8b-1 (`tested-evidence.md`) made the owner's passing test run durable evidence in `spec.lock`, judged only by plain `spec verify` and `check_binding`; every read an agent works from dropped the blocks (`cut_lock`). So an agent could not see where it reads whether the owner's run still vouches for a node, nor that its edit is about to void that evidence. 8b-2 shows 8b-1's entries in `show --bindings`, the bundle, the task package and `impact`, computed by the same function as `verify`: no new judge, write, tool, argument, route or gate input.

## Description and interactions

- **Canon**: `docs/canon/spec-lock-tested.md` "Reads" (seam, entry, per read, degrade, notes, cost), "Readings", "Known limits", and the read canon it points to.
- **Reads**: `show REF --bindings` (`get_node`, `nodes/{*ref}?with=bindings`), `bundle` (`get_context_bundle`, the bundle route), `task show` (`get_task`, `tasks/{id}`), `impact` (`get_impact`, no route). `verify` and `check_binding` are the reference, unchanged. Plain `show` never reads the lock.
- **Nothing new is written or depends on it**: no lock write, queue row, tool, argument or route; no exit, status, claim, approval, gate, finding, count or drift item (ADR-0044, ADR-0046).
- **Decided** (Q-A..Q-L, 2026-10-21): every entry printed; no bundle JSON key; UI types only; one slice; no ADR ("Readings").

## Data

### The entry: model `TestedRow`

`specengine_model::TestedRow`, 8b-1's seven keys in order, words as strings like `BindingRow`; a list by node bytes, `null` not judged, `[]` no block in the set; never `node_gone`.

```json
{"node":"R-12","state":"stale","category":"code_changed","rev":3,"node_rev":3,"commit":"a0cea48<33 hex>","verified_at":"2026-10-21"}
```

### The line

`tested | <state>[ <category>] | <node>` (`tested_line`), no path, date or commit; verify still prints only entries not `verified`, the reads every entry (`  tested | verified | LAW-BOUND` under a shown node's `  bindings 1`).

### Per read

| Read | Set judged | Lines | JSON |
|---|---|---|---|
| `show --bindings` | each shown node's own set | after its closing line, two spaces in, cut with it | `bindings` `{rows, omitted, tested}`; node keys stay 19 |
| bundle | layer 9's | under a placed text or outline target's and a placed criterion's header, none in the frame | no key; `body`, sizes, `bundle_hash` move |
| package | `task_set`, = `verify --task T --json`'s | the brief's Bindings, after the rows, two spaces in | `tested` after `bindings`, 17th of 27; `schema_version` 1 |
| `impact` | reached nodes and the nodes within them | after the `node` lines, one list, cut | `tested` after `nodes`, 9th of 15; `truncated.tested` |

### MCP

Mirrors reuse `VerifyTested`: `ShowBindings.tested` after `omitted`, `TaskPackage.tested` after `bindings`, `ImpactDocument.tested` after `nodes`; `ImpactTruncated.tested: Option<usize>`; `PACKAGE_V1` 8 paths more. Descriptions ASCII, no digit added, one line each (`BINDINGS_LINE`, `BUNDLE_CODE_LINE`, `TASK_BINDINGS_LINE`): `get_node` 1 527 -> **1 670 B**, `get_context_bundle` 1 305 -> **1 433 B**, `get_task` 1 754 -> **1 909 B**; `get_impact` 2 043 B and `INSTRUCTIONS` unchanged.

### Daemon and UI

HTTP source unchanged. `fixtures/daemon-keys.json`: 68 sets; `TaskPackage` 27 keys, `tested` the 17th; `TestedRow` the 68th and last. UI types only (`provisional.ts`): `TestedState`, `TestedRow`, `ShownBindings.tested`, `TaskPackage.tested`, each `TestedRow[] | null`; no display.

### Plugin 0.18.0

0.17.0 -> 0.18.0 (MINOR: a key three tools return); `PINS` and `SCHEMA_PINS` entries. No enum value, no digit:

- `read-spec`, the new second bullet of "Using what you read" (body 3 123 -> 3 399 B, file 3 444 -> 3 720):

```text
- A node's `tested`, in `get_node` with its bindings, in `get_task`, in `get_impact` and as lines under a bundle's headers, says whether the owner's own passing test run still vouches for that node, judged now: evidence to read, never yours to claim, and nothing waits on it.
```

- `implement`, step 5, after "name them in the report, never hide them." (body 3 414 -> 3 682 B, file 3 655 -> 3 923):

```text
A node's `tested` (in `get_task`, `get_impact` and `check_binding`) is the owner's own test evidence: an edit to the node or its bound code can void it, so name each such node in the report for the owner to run its tests again, and never claim that evidence yourself.
```

### Cost

No block: nothing more read. Blocks: `verify --node`'s cost. TE-3 (`tested_reads_cost`, release, warm, median of seven; four present blocks and one gone, against none):

| Read | With blocks | Without | Ratio |
|---|---|---|---|
| `show --bindings` | 34.5 ms | 22.9 ms | x1.51 |
| bundle | 33.9 ms | 23.0 ms | x1.47 |
| `task show` | 51.5 ms | 40.6 ms | x1.27 |
| `impact` | 53.6 ms | 46.5 ms | x1.15 |

`show_bindings_cost` and `bundle_bindings_cost` reran green.

## Rules and edge cases

- A read and `spec verify` at one state SHALL agree key for key; the package's `tested` SHALL equal `verify --task T --json`'s.
- A degraded read, or blocks not judgeable, SHALL give `null` and no line.
- With no block, every read's text, exit and the bundle's JSON SHALL be 35f366c's bytes, the other JSON equal once `tested` is stripped, and nothing more SHALL be read.

## Acceptance criteria

Fixtures: `fixtures/verify-tests/` with `fixtures/spec-lock/verify-tests.tested.lock`, the evidence kiln (`tests/tested_kiln`), `fixtures/code-a`. Mutations M1-M8: 8 of 8 killed, as 10 variants (M8 once per mirror).

- [x] RD-1 seam. **M1** (a read judging on its own) -> `read_and_verify_agree_on_tested`.
- [x] RD-2 degrade. **M2** (`tested` kept past a cause) -> `degraded_read_never_verified`.
- [x] RD-3 plain show. **M3** (the lock read without `--bindings`) -> `plain_show_reads_no_evidence`.
- [x] RD-4 evidence-free. **M4** (changed bytes) -> `evidence_free_reads_unchanged` (`tests/tested_reads_before.json`, 35f366c's).
- [x] RD-5 show cap. **M5** (lines cut apart from the closing line) -> `show_cut_keeps_tested_with_closing`.
- [x] RD-6 bundle. **M6** (a line in the header form) -> `bundle_minimum_ignores_tested`.
- [x] RD-7 package. **M7** (the `bindings` set used) -> `package_tested_is_verify_tasks`.
- [x] RD-8 MCP. **M8** (`#[schemars(skip)]` on a mirror's `tested`) -> `get_node_and_bundle_carry_tested`, `get_task_carries_tested`, `get_impact_carries_tested`.
- [x] RD-9 (uncounted): `impact_tested_is_cut_as_a_list`; 0.18.0, `PINS`, `SCHEMA_PINS`; `PACKAGE_V1`; the three description pins.
- [x] TE-1 parity: CLI = MCP = daemon over a format-3 lock.
- [x] TE-2 determinism: two roots, another cwd, reverse creation order, `core.autocrlf=true`, a SHA-256 repository.
- [x] TE-3 cost: `tested_reads_cost` (`#[ignore]`), figures in "Cost".
- [x] TE-4 8b-1's pins adapted: `lock_format.rs` `every_lock_reader_reads_format_3`; `mcp_read.rs`' `{rows, omitted}` key set; `plugin_skills.rs` `impact_implement_names_get_impact` (now `implement` and `read-spec`).
- [x] TE-5 daemon keys: `fixtures/daemon-keys.json` regenerated; 68 sets, `TaskPackage` 27, `TestedRow` 7.
- [x] TE-6 green, unchanged: `no_code_table_bundle_is_unchanged`, `evidence_free_verify_text_unchanged`, `check_genre`, `verify_tests_genre`, `mcp_genre`, `mcp_door`.
- [ ] TE-7 (owner's): `tested_reads_pilot.rs` (`#[ignore]`): pilot A after a passing run, entries equal verify's; pilot B `[]`.
- [x] UI: the types; `provisional.test.ts`, `daemonKeys.test.ts` (68 types, 27 package keys); builders, mocks (`tested` `null`).
- [x] SW-1..SW-5: `spec-lock-tested.md` 9 785 -> 12 235 B; `code-reads.md` 12 278, `spec-impact.md` 12 278, `spec-cli-bundle.md` 12 276, `mcp-read.md` 12 266, `task-package.md` 12 225; `mcp-code.md`, `plugin-files.md` (0.18.0 pins verified via `as_of_0_17_0`), `plugin.md`, 07, 08, RM-0032, `CLAUDE.md`, model README; `architecture-code.md`, four crate READMEs untouched; `export index && check` clean.

Not covered by tests: cause (8), unreachable from outside the binary; `View::Browser` above the cap; the package's `tested` past 64 blocks.

## Out of scope

Health's lines (9b, `health-drift.md`); tree counters and a node screen (RM-0011); a UI display; evidence levels; anything reading `tested`; new tools, arguments, routes; a lock write; `INSTRUCTIONS`; a bundle JSON key.

## Implementation

| Module | What it does |
|---|---|
| CLI `bindings.rs` | `SetTested {row, nodes}`, `SetBindings.tested`, `tested_of` (method and free), `set_tested`; `set_bindings_with(.., tested_also)`, `set_bindings` passing `&[]` |
| CLI `tested.rs` | `tested_entries` returns `Vec<TestedRow>`; `tested_line`, `tested_note_text`, `is_verified`; `TestedEntry` gone |
| CLI `verify.rs` | text through `tested_line`, JSON `&[TestedRow]`, bytes unchanged (`TestedJson` gone) |
| CLI `show.rs`, `cap.rs` | `ShownBindings.tested`; `BindingsBlock.closing` a `Vec` costed by `closing_cost`; JSON `tested` |
| CLI `bundle.rs` | `Beside.tested_refs`, `BesideRows.tested`, `CodeRows.tested`; `target_forms`, `prepare` take the lines; `beside_rows(.., tested_refs)` |
| CLI `package.rs`, `impact.rs` | the 27th key and the brief's lines; `ImpactOutcome.tested`, the cut list, tail part, `truncated.tested` |
| model `task.rs` | `TestedRow`, `TaskPackage.tested` |
| MCP `mirror.rs`, `read.rs`, `tasks.rs` | the four mirror fields; the three description lines |
| `plugin/` | 0.18.0, the two sentences |

Deviations (accepted at review): `set_bindings_with`, unnamed in the draft, joins the package's criteria refs to the judge's keys inside the one lock read, only with a block; the `tested:` note uses `SetBindings.note`, the one note slot, as the two never coexist.

Known costs (accepted at review, unless TE-3's ratio is poor): `set_tested` builds a second `Corpus`, only with a block; the bundle builds its tested key sets in memory without a block too (no read, no judge).

Tests: nextest workspace 3 836 of 3 836 passed, 62 skipped; ui 2 209 of 2 209; fmt, clippy (probes too), export, check, parity and the eval policy tests (86 of 86) clean.
