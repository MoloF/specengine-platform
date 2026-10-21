---
class: spec
status: shipped
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, fixtures]
ref: Phase 3 slice 7b-2 of RM-0007 (7b-1 `docs/features/impact.md`); Q-G..Q-I, A1-A8, owner pre-approved, review open; 05 s5.2, s5.3; 07 s1.2; 2 iterations
shipped: 2026-10-21
adrs: [ADR-0046]
---

# impact-since (7b-2): the radius of the code changed since a revision

## Why

What does the code changed since a revision reach (nodes, bound symbols with their `sync`, tests, open tasks), when an agent or the owner hands in a branch? 7b-1 answered for a node or a symbol; this slice adds `spec impact --since REV`, MCP `get_impact {since}`, and fills `spec verify --changed --json`'s `delta.symbols` (05:154) from the same comparer. No new ADR: the rule is canon (`architecture-code.md#sync-in-reads`, ADR-0046).

## Description and interactions

Canon: `docs/canon/spec-impact.md` "Since", "Delta"; `spec-verify-output.md` "JSON". Walk, rows, tasks, notes and exits are 7b-1's. Unchanged: plain, `--staged`, `--node` verify, `check_binding`'s schema, the gate, `INDEX_FORMAT`, the queue, HTTP, UI, dependencies.

## Data

### Command and guard

`spec impact [REF] [--symbol QPATH] [--since REV] [--depth N] [--json]`. Help, verbatim: the command "Show what a change to a node, a code symbol or the code since a revision reaches: nodes, bound code with its sync, tests, open tasks; read-only."; `--since REV` (hyphen values allowed) "The code changed since this git revision, against the files on disk."

Before discovery: the form (exit 2, `spec: impact takes exactly one of REF, --symbol and --since`), then the guard (A8; exit 2, no git child, ``spec: --since `<REV>` is refused: a revision never starts with `-` or holds a control character``, each control character, LF, CR, TAB too, `\u{XX}`). `list_revision(REV)` `None` -> exit 1, ``spec: --since `<REV>` is not a commit``, the exit-1 JSON: an unknown name, a tree, an unborn `HEAD`, a range, `^REV`. Other failures: exit 2 (`spec-impact.md` "Since").

### Compared

A1: the disk side is today's `[code]` walk, untracked files in, the git index unread; REV's its tree under the same `[code]`. Compared files, records: `spec-impact.md` "Delta" (Q-G: afresh from each side's bytes, the index's digests unread; Q-H: a RON file one record, its bytes' BLAKE3).

### Core comparer

`specengine_core::symbol_delta`, pure: `SymbolRecord {file, qpath, item_kind, line, sig, body}` (digests `Option<String>`, opaque), `Compared {added, removed, changed: [(old, new)]}` (indexes), `is_empty`, `compare(old, new)`; key, twins, order: `spec-impact.md` "Delta".

### Seeds

A2: a `removed` or `changed` item reaches through REV's markers, an `added` or `changed` one through the disk's, by 7b-1's containment (one parent derivation shared with the code index); a RON file through every value-anchored marker (D4). IDs as `--symbol`'s; no seed: `nodes` `[]`, exit 0.

### Output

Header `since <7 hex>: <f> files, <c> changed, <a> added, <r> removed` (A4: `revision`'s first seven); delta lines last, `<changed|added|removed> | <file>:<line> | <symbol> <item_kind>`. Notes: the md refresh's, the disk walk's, the queue's, `bindings:` last. JSON (A5): `input.since`, `revision` (the full OID), `delta {added, removed, changed, renamed: null}` of `{file, symbol, item_kind, line}`, `null` but under `--since`, where `seeds.symbols` is `null`.

### Cut

7b-1's; delta items last (`changed`, `added`, `removed`), the JSON the printed ones; the tail gains `<d> delta items`, `--since`'s ends `lower --depth N or name a later --since REV`.

### verify --changed

Q-I, D1: under `--changed` (`--task` or not, unfiltered) `delta.symbols` is made for `--json` alone (`verify_as(.., VerifyFormat)`; the library `verify()` is `Json`; a Text outcome is never rendered as JSON): `HEAD`'s code side against the walk, uncapped; all equal: three `[]`. `null` otherwise; rows, verdict, counts, exit unchanged. `renamed` `null`, overriding `anchor-states.md:22` (Q14): RM-0031.

### MCP, INSTRUCTIONS, plugin 0.16.0

- `ImpactArgs.since` between `symbol` and `depth` (`IMPACT_SINCE_DOC`, 69 B), `ONE_INPUT`, `IMPACT_DESCRIPTION` 1 898 -> 2 043 B (5 left): verbatim in `read.rs`, pinned by `mcp_impact.rs`. Mirror: `spec-impact.md` "MCP get_impact".
- `INSTRUCTIONS`' reads line ends `get_impact(id|symbol|since, depth?) = impact.`: 2 023 B, 1 885 without the tasks line, 25 left.
- Plugin 0.16.0 (MINOR: `get_impact` takes `since`, its keys typed): `PINS` `515793a2…3ef4`, `SCHEMA_PINS` `e86927e8…9f83` (both verified through `as_of_0_15_0`). `implement` step 6 (body 3 414 of 4 096 B), verbatim:

> 6. **Report the run.** First call `get_impact` once with `since` the commit your work started from: run the tests it lists and name its open tasks. Then call `report_run` with the `task_id`, its `outcome`, a `summary` of what the run did and the `changed_files`. The task stays in progress after the report, and its writes still pass.

## Rules and edge cases

- WHEN `since` comes from the CLI or an agent, the guard SHALL refuse it before discovery: no git child.
- WHEN `impact --since HEAD --json` is uncut, its `delta` SHALL equal `verify --changed --json`'s `delta.symbols` on that state.
- WHEN only comments, markers, formatting or line endings change in Rust, no item SHALL be `changed` (A3); a RON file's CRLF copy is `changed` (Q-H). The rest: `spec-impact.md` "Delta".
- Git: `rev-parse`, `ls-tree -r -z`, `cat-file --batch`, `hash-object` (never `-w`), plus 7b-1's tasks probe; never `status`, `diff`, `ls-files`; `GIT_OPTIONAL_LOCKS=0`; index, objects, refs unchanged.
- Determinism: two clones, another cwd, reverse creation order, CRLF Rust copies: identical text and JSON.

## Acceptance criteria

M: a counted mutation and the test it turns red; 8 of 8 killed. CLI `impact_since.rs`, `verify_delta.rs`, core `symbol_delta.rs`, MCP `mcp_impact.rs` unless named; histories scripted over copies of `fixtures/impact/`.

- [x] RD-1 -- `option_like_since_is_refused_before_git`; MCP `get_impact_refuses_an_option_like_since_before_git`. M1: the guard removed.
- [x] RD-2 -- `since_names_no_commit`: `nope`, an unborn `HEAD`, a range, `^REV`: exit 1, the exit-1 JSON. M2: not-a-commit exits 2. M8: `verify_rev`'s `Named` arm removed (a range unreadable again). Uncounted: `list_commit` for REV.
- [x] RD-3 -- `since_compares_the_given_revision`, `since_counts_untracked_code`, `since_ignores_unchanged_hashes`. M3: `HEAD` read for REV. M4: untracked disk files never compared.
- [x] RD-4 -- `removed_symbol_reaches_its_old_binding`, `since_radius_is_its_seeds_radius`, `ron_whole_file_record_reaches_its_markers`. M5: REV's markers ignored.
- [x] RD-5 -- `plain_verify_has_no_symbol_delta`, `changed_verify_fills_the_symbol_delta`, `text_verify_computes_no_symbol_delta`, `unborn_head_adds_every_item`, `renamed_stays_null`, `symbol_delta_changes_no_verdict`. M6: filled in plain.
- [x] RD-6 -- `since_head_delta_equals_verify_changed` (`fixtures/impact/`, `code-a`); core `symbol_delta.rs` (order, `None` digests, twins). M7: the impact delta sorted by symbol.
- [x] RD-7 -- `reads_leave_git_index_and_objects_untouched`: status, `.git/index` mtime, objects unchanged; the shim's commands only.
- [x] RD-8 -- `since_hashes_both_sides_afresh`: the index's digests tampered, the delta unchanged. Uncounted: "digests read from the index" cannot be written (no CLI read path).
- [x] TE-1 -- `since_history_gives_each_revisions_delta` (an older commit, `HEAD`, a branch, a tag); `since_*` legs for a non-UTF-8 file, roots and excludes, a SHA-256 commit, unparsable code, the cut, tasks, notes.
- [x] TE-2 -- `since_is_byte_identical_across_clones_order_and_crlf`, `crlf_copy_over_lf_blobs_changes_no_rust_item`.
- [x] TE-3 -- `impact_cost.rs` `impact_since_cost` (`#[ignore]`, release, warm, median of five): `impact --since HEAD` nothing differing 159 ms, 1 000 compared files 207 (`--json` 203); `verify --changed` on `code-a` plus 10 000 `.rs` files, text / `--json`: before the delta 1 042 / 999, 1 000 edited 1 012 / 1 051, 10 000 edited 1 154 / 1 437.
- [x] TE-4 -- `since_needs_code_and_git` (`spec-c`, no repository, no `git`), `since_takes_exactly_one_input`.
- [x] TE-5 -- MCP parity per exit for `{since}`, `mcp_door`, `mcp_size`, the walker typing the four keys; the description and INSTRUCTIONS figures; `plugin_skills`, `plugin_files` 0.16.0.
- [x] TE-6 -- a `git` shim logging argv proves RD-1's no child and RD-7's door.
- [x] SW-1, SW-2 -- `spec-impact.md` 12 279 B, `spec-verify-output.md` 11 889; READMEs core 10 237, CLI 10 236, store 10 240; 05, 07, 08; RM-0007.

## Out of scope

`renamed` (RM-0031); `delta.symbols` under `--staged`; equal-bytes files whose qpaths moved; documents changed since REV as seeds; a call-graph radius; other repositories' tasks; a cap on work; `--end-of-options`.

## Implementation

| Module | What it does |
|---|---|
| core `symbol_delta.rs` (new) | `SymbolRecord`, `Compared`, `compare`: the pure comparer |
| CLI `code_delta.rs` (new) | `code_delta(old, new)`: records afresh, then `compare`; `CodeDelta`, `DeltaItem`, `DeltaJson::printed`; `seeds` |
| CLI `verify_rows.rs` | `read_head`'s code half as `read_code(listing, code, git, rev, share) -> CommitCode`, `CommitCode::tree(walk, rev)`, any listed commit; `HeadSide::code(walk)` |
| CLI `impact.rs` | `ImpactRequest.since`, the guard, `since_starts`/`since_read` (one `VerifyGit` session, `list_revision`); header, delta lines, cut, JSON |
| CLI `verify.rs`, `main.rs`, `lib.rs` | `VerifyFormat {Text, Json}`, `verify_as`, `verify()` its `Json`; `delta.symbols`; the `--since` flag |
| CLI `code_index.rs`, `verify_node.rs` | one parent-derivation helper, shared with the seeder; `--node`'s `symbols` `None` |
| store `git.rs`, `verify_git.rs` | `RevAnswer` in `verify_rev`: `Named` (`list_revision`, `impact --since` only) reads exit 1, whatever printed, and exit 0 without one OID as no commit; `Strict` (`head()`, `list_commit`: `read_head`, `commit_config`) keeps a misbehaving git a failure |
| MCP `read.rs`, `mirror.rs`, `server.rs` | `ImpactArgs.since`, `IMPACT_SINCE_DOC`, `ONE_INPUT`, `IMPACT_DESCRIPTION`; `ImpactDelta`, `ImpactDeltaItem`; `INSTRUCTIONS` |
| `plugin/` | 0.16.0; `implement` step 6 |

Divergences (owner-approved):

- D1: `delta.symbols` is made for `--json` only (the draft: whatever the format), so a text run pays nothing.
- D2: `read_head` reads documents, lock and config, then `read_code`: two `read_files` in one `cat-file` session (a shared OID read twice); an unreadable document plus a git failure reading code names the document's cause.
- D3: a re-parse failure gives no digest: the item reads `changed` (practically unreachable).
- D4: under `--since` a RON file's markers seed; under `--symbol` they still do not.
- Iteration 2: a range or `^REV` exits 1, not 2, through `list_revision` (`store` in scope again).

Known limits: each compared Rust file is parsed twice per side (the bind, then `item_levels`); a nested repository or submodule under a code root reads as `added`; D2's order; D3.

`mcp_size`'s `get_impact` maximum stays 100 621 characters (`{symbol}`); `{since}` peaks at 90 175 (text 40 026, `structuredContent` 50 149). Tests: nextest workspace 3 749 of 3 749 after the README fix (56 skipped).
