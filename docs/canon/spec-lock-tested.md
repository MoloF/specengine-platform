---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-cli, crates/specengine-model, crates/specengine-mcp]
owner: owner
reviewed: 2026-10-21
---

# `[[tested]]`: test evidence in `spec.lock`

What the owner's passing `spec verify --tests` leaves in `spec.lock` (ADR-0044; the lock's second writer, ADR-0041): a block per node, pinned by its revision, `norm_hash` and a hash of its bound code, judged on each read by one pure function; never stored, never a verdict, exit, status or gate input. No agent writes it (no MCP tool, route, hook, CI step or role; `report_run` changes nothing). Code: core `lock` (`Tested`, `bound_bytes` over `Bound` pairs, `tested_state`, `merge_tested`, `rebase_tested`); CLI `tested.rs`, `bindings.rs` (`set_tested`), `lock.rs` (`LockSubject::Tests`, `LockMode::Tests`, `node_bindings`), `verify_tests.rs`; MCP `mirror.rs`. The file, its entries, the reader's texts: `spec-lock.md`.

## The block

```toml
[[tested]]
node = "R-12"
rev = 3
spec_hash = "b3:9f2c<60 hex>"
bound_hash = "b3:c7e1<60 hex>"
tests = [
  { file = "crates/game/src/stamina.rs", symbol = "crates/game::stamina::tests::regen_stops_at_zero" },
]
commit = "a0cea48<33 hex>"
verified_at = "2026-10-21"
```

`node` the canonical ID (an entry's rules); `rev`, `spec_hash` R and `norm_hash` from the node file's `HEAD` blob; `bound_hash` over `HEAD`'s blobs; `tests` the tests that ran and passed, binding keys' `file`, `symbol` (twins each), by bytes, distinct, non-empty; `commit` `HEAD`'s full OID at the run (the evidence commit's parent); `verified_at` the run's UTC date.

- **Format**: a lock holding a block is `format = 3`; none, `format = 2`, byte for byte as before. Format 3 is format 2 plus blocks: `Lock::format_number` is the file's number; `LockFormat::entries_number` (`of(3)` is `Two`) the entries', never the file's.
- **Bytes** (`render_lock`): the entries, then the blocks by `node` bytes, an empty line before each, keys as shown, strings as `spec-lock.md` "The file"; `tests = [`, one `{ file = <s>, symbol = <s> },` per line two spaces in, `]` alone. Top keys `format, norm_recipe, code_recipe, binding, tested`.

## Bound hash

Store `b3_hash` of core `bound_bytes`, through CLI `tested::bound_hash`, the one function the write, each read and the rebase call. The bytes: `specengine-tested/v1` and LF, then per binding naming the node (every relation; an alias marker under the canonical ID; a key from several markers once), in `EntryKey` order, its key lines (`node`, `relation`, `file`, `kind`, `symbol`, `item_kind` when present) and `sig_hash`, `body_hash`, `deps_hash`, each as an entry renders it, then an empty line.

- **Never**: `rev`, `spec_hash`, `levels` (a body edit under a `[sig]` marker stales evidence, a narrowed marker does not), fingerprints, `file_blob`, `commit`, `verified_*`, an OID: a SHA-256 repository hashes alike.
- **At the write**: the fresh entries of 8a's predicate (`committed_bindings`, `HEAD`'s raw blobs).
- **On a read**: one pass over every judged node's marker bindings on the working tree (`Source::Disk`), one parser, each file parsed once (none whose every lockable binding reuses its entry); a binding whose entry's `file_blob` is the disk blob takes the entry's digests, else accept's (`level_code`) from this parse. Levels are computed for each node reaching the lockability check (`cannot_verify` wins over `stale`); the bound bytes and hash only for a block no earlier state judges.
- So a comment elsewhere, rustfmt or a neighbour's edit keeps an `item`'s hash; a `module`'s survives comments and rustfmt, not a code edit in its file; a `ron`'s comments and sibling values.

## The state

Core `tested_state`, pure; per block the first that applies:

| `state` | `category` | When |
|---|---|---|
| `node_gone` | `null` | no node has `node` as its canonical ID |
| `unverifiable` | 8a's `not verifiable` word | the revision unreadable (`rev_unknown`), the hash null (`hash_null`); else the first binding naming the node not lockable now (`spec-lock.md` "Lockability"), keyed then keyless |
| `stale` | `spec_changed` | (R, `norm_hash`) ≠ (`rev`, `spec_hash`) |
| `stale` | `code_changed` | the bound hash now differs: code, a test or a bound RON value edited, a binding added or removed (all gone too) |
| `verified` | `null` | none |

Several holders of the ID: the first by path, then source order. Never inputs: `report_run`, the event, `Cargo.lock`, the toolchain, `[verify]`, unbound code a test reaches. A node can be `code_ahead` and `verified`, or `ok` and `stale`.

## Plain verify

JSON `tested`, the 11th of 14 keys, after `delta`: per block by `node` bytes `{node, state, category, rev, node_rev, commit, verified_at}`, `rev`, `commit`, `verified_at` the block's, `node_rev` the node's now (`null` gone or unreadable). Plain: every block; `--task T`: its set's (built only when the lock holds a block); `--node REF [--symbol]`: its holders', all their bindings; `[]` without a block. `null` under `--changed`, `--staged`, a run cause, and when the blocks cannot be judged (a git `hash-object` failure, the walk, the grammar): then one note `tested: <cause>`, never a run cause (the git session's end raises nothing); exit, verdict, counts and rows unchanged. Text, per entry not `verified`, after the hints, before the summary, outside `--node`'s cut: `tested | stale code_changed | R-12`, `tested | unverifiable spec_ahead | R-14`, `tested | node_gone | R-9`. No finding, class, count, hint, verdict, exit or `check_binding` `isError` depends on it (mirror: `mcp-code.md` "Mirrors"); without a block stdout is as before.

## The run's write

The lock read first (step 6), `text_changed`, the question's tail: `spec-verify-tests.md` "Order". After step 10, only after a completed run (`verified`, `unverified`), per reported node: `verified` without a block `added`; with another `rev`, `spec_hash`, `bound_hash` or `tests` `refreshed`; equal on those four `unchanged`, bytes kept; `failed` (a time-out too) with a block `removed`; else no row. Entries and header kept but `format`; a first lock created as accept's; only `unchanged` rows: no commit.

## The commit

`write_and_commit` with `LockSubject::Tests`, `LockMode::Tests`, its consent closure returning step 7's `y` unasked (`commit the evidence of <n> nodes to spec.lock at <head>? [y/N]` never shown); `git commit --only spec.lock`, hooks run, never `--no-verify`:

```
spec: verify tests CONE-01, KILN-HOLD, KILN-RAMP and 1 more

Lock: CONE-01
Lock-mode: tests
Decided-by: <committer_ident>
Test-command: ["scripts/run-test", "{package}", "{target_kind}", "{target}", "{test}"]
```

The subject: the changed nodes by bytes, past three ` and <n> more`; a `Lock:` per changed node; `Test-command` the question's template, a caller trailer (the fifth check). `tests` is outside `LockMode::TRAILERS`: no drift item completes on it. Check 3: ``it has no `Lock: <node>` trailer`` per node, ``it has no `Lock-mode: tests` trailer``. Else accept's checks, table and restores (`spec-lock-accept.md` "The lock commit"); `spec.lock changed while being written; nothing written`; an unreachable fallback `the commit of spec.lock was not verified`.

- **Exit**: a completed run whose write or commit failed keeps every verdict, sets `lock.refusal`, leaves `lock.commit`, `lock_commit` `null`, writes the event: exit 1.
- **Output**: after the summary, `lock | <change> | <node>` by node bytes, then `committed <oid>` or `lock: <refusal>` (all `unchanged`: rows alone). JSON `lock` `{"rows":[{"node":"R-12","change":"added"}],"commit":"<oid>","unchanged":false,"refusal":null}`, `null` without a row; `unchanged` iff every row is; `commit` the verified OID, else `null`.
- **Event**: `verify.tests`' `lock_commit` the verified OID, else `null`, written after the commit.

## Other writers

A block is one entry of its node in every count but the rebase's; its row `<change> | tested` follows the entries'. A writer renders format 3 iff a block remains; a merge choice misfitting the code reads `stale`. Only the run adds a block.

- **Accept** keeps it byte for byte and refuses a block at R with another `spec_hash` as an entry; a gone node's drop removes it; `--editorial` refreshes it at R (`spec-lock-accept.md`).
- **The editorial approve** refreshes a vouching block in the apply commit (`proposal-revisions.md`).
- **Regen** merges blocks per node, stages of formats 2 and 3 (`spec-lock.md`).
- **Rebase** re-derives a block only under proof, else drops it (`spec-lock-rebase.md`).
- **Drift** hashes entries alone: no item for stale evidence (ADR-0045).

## Reads

`show --bindings`, the bundle, the package and `impact`, with their MCP and HTTP twins, show these entries as computed state (ADR-0046). **Seam**: in `set_bindings_with(.., tested_also)`'s judged branch (`set_bindings` passes `&[]`), before the session's `finish()`, only when the uncut lock holds a block, `set_tested` calls `tested_entries` (`verify --node`'s call) for the canonical IDs of the read's keys and `tested_also`; each read takes its part of `SetBindings.tested` (`tested_of`). No second judge: a read and plain verify agree key for key.

- **Entry**: model `TestedRow`, plain verify's seven keys; a list by `node` bytes, `null` not judged, `[]` no block; never `node_gone`. **Line** (`tested_line`): `tested | <state>[ <category>] | <node>`, each entry, `verified` too.
- **show**: each shown node's own set; lines after its closing `  bindings` line, two spaces in, cut with it (`closing_cost`); JSON `bindings` `{rows, omitted, tested}`.
- **bundle**: layer 9's set; lines under a text or outline target's header (it and the nodes within it) and a placed criterion's, in its fit; never in the header form, so the minimum and exit 2 ignore them. No JSON key; `bundle_hash` moves.
- **package**: `task_set`, equal to `verify --task T`'s (criteria refs join only with a block); `tested` the 17th of 27 keys, after `bindings`, never capped or cut; the brief's Bindings prints with rows or entries.
- **impact**: the rows' set; the 9th of 15 keys, after `nodes`; lines after the `node` lines, cut as one list (tail `<e> tested`, `truncated.tested`); `null` at exit 1.
- **Degrade**: no `--bindings`, no `[code]` or a listed cause: `null`, no line, only the `bindings:` note. Blocks not judgeable: `null`, note `tested: <cause>` in that slot (stderr, `notes`). Degraded, never `verified`; exit, rows, spec part unchanged; plain `show` never reads the lock.
- **Cost**: no block, nothing more read; else `verify --node`'s (`tested-reads.md` "Cost").

## Readings

- ADR-0043's "entries that vouched" include a block: the rule `rev-not-raised`, `spec bump`, a review's `lock_entries` and the editorial approve count and refresh it as one entry.
- ADR-0046's computed state per read includes the evidence: judged as `sync` is, never stored, never a failure; `bundle_hash` moves with it.
- ADR-0050's "never … into an older format" is about how entries are recorded: a rebase, a failing run or a drop removing the last block writes format 2, byte for byte as before; no downgrade.

## Known limits

- An older `spec` or `specengine-mcp` refuses format 3 (its verify hook every commit, its reads cause (6)): install both from one commit.
- A hook stopping on a Fix row refuses the evidence commit: exit 1 under `verified`; `HEAD` moving writes nothing; the event stays.
- An evidence-only lock loses its blocks at each `code_recipe` bump (`unproven`): rerun the tests.
- A hand-written block reads `verified`; the gate skips `spec.lock` (ADR-0034).
- A grammar that fails to load fails every judged node's evidence, even a node whose bindings would reuse entries: `tested` `null` and the note.
- Markers are judged once per holder: a file is parsed once, its markers sorted again for each judged node.
- A duplicate ID is judged on its first holder by path; every shown holder lists that entry, as `verify --node`.
- The package's `tested` is never cut: many blocks can push `get_task` past `PACKAGE_BUDGET` (`mcp-read.md` "Size").
- A header-only target (the bundle's frame) carries no line.
- A node `show` cuts before its closing line has `tested` `[]`, marked by its own `truncated` or `omitted`.
