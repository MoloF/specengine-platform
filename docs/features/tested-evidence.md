---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 8b (WA-1..WA-12; Q1-Q8 decided under the owner's pre-approval, review open); 05 s5.4; 07 s2; 08 s2, AC-8
adrs: []
---

# tested-evidence: a passing test run becomes self-staling evidence in `spec.lock`

## Why

After 8a (`verify-tests.md`) a pass is only a `verify.tests` event in the data directory: `spec.lock` is untouched, the run's `lock` and `lock_commit` are null. Nothing durable says "R-12's tests passed at commit X for this text and this code", nothing travels with the branch, nothing notices a later edit of the text, the implementation or a test: 08 AC-8 stays open.

Delivered: one `[[tested]]` block per node in `spec.lock`, pinned by `rev`, `spec_hash` and a `bound_hash` over the node's bindings, written by 8a's run in its own commit under its one `[y/N]`; one pure function giving `verified`, `stale`, `node_gone` or `unverifiable` on any read, no test run. No ADR (WA-1): the writer, the pass/fail rule and "computed, read by nothing" are ADR-0041 and ADR-0044; accept, editorial and regen treatment follow from ADR-0041 ("a later owner step … must run the same accept") and ADR-0043 ("entries that vouched for the old text at that revision"); the rebase's from ADR-0050. Only a CI or MCP writer, a state or gate reading `verified`, or drift items for stale evidence would need one (each contradicts ADR-0041, ADR-0044 or ADR-0045). The canon already names this writer (`architecture-code.md#spec-lock`, "Tests as evidence").

## Description and interactions

- **Boundary.** 8a: config, selection, runner, verdicts, event, the single `[y/N]`. 8b: the block and format 3, the write and its commit, the per-node state, `tested` in plain verify, blocks under accept, `--editorial`, a gone node, regen and rebase, 8a's `lock` and `lock_commit`. 4c unchanged: its approve runs accept's function and inherits; stale evidence never becomes a drift item (that would amend ADR-0045). 7a reads, 9b Health: "Surfaces".
- **The owner** answers 8a's one question, now naming the lock commit. **An agent** reads `tested`; it never writes evidence: no MCP tool, route, hook, CI step or role (ADR-0044); `report_run` changes nothing (07:38).
- **Crates.** Core `lock.rs`: the block in `Lock`, `parse_lock`/`render_lock` for format 3, `bound_bytes` (the canonical input; core has no BLAKE3), `tested_state`, blocks in `merge_accept`, `drop_node`, `refresh_editorial`, `merge_stages` and the rebase. CLI: the write after the run, `write_and_commit`/`check_lock_commit` taking a set of nodes (5b gave them a subject), `LockMode` `tests`, hashing with store's `b3_hash`, `tested` output. Store nothing new; model, MCP, HTTP, UI only as mirrors with 7a or 9b. No new crate or dependency.
- **Order** (2026-10-12): after 4a, 8a and 5b (levels, `spec lock rebase`); soft after 4b-1 (8b changes `refresh_editorial`, which 4b-1 runs inside the apply commit filtered to vouching entries, `rev-discipline.md` Q10: the later wires vouching blocks into the editorial apply) and 4b-2 (its rule reads blocks: `rev-rule.md`); 4c, 7a, 9b on either side, the later adding its part.
- **4a is not edited** (being implemented): at 8b's shipping plain `spec verify` gains a `tested` key and section ("Plain verify"), null under `--changed`, no exit effect; 4a's code and goldens change then.

## Data

### Format 3 and the block

Q1, WA-2, WA-3. A lock holding a `[[tested]]` block is `format = 3`; one with none stays `format = 2`, byte for byte 5b's (`lock-levels.md` "Format 2"). Format 3 is format 2 plus blocks: header, recipes, `[[binding]]` entries unchanged.

```toml
[[tested]]
node = "R-12"
rev = 3
spec_hash = "b3:9f2c<60 hex>"
bound_hash = "b3:c7e1<60 hex>"
tests = [
  { file = "crates/game/src/stamina.rs", symbol = "game::stamina::tests::regen_stops_at_zero" },
  { file = "crates/game/tests/stamina.rs", symbol = "game::test:stamina::drain_is_clamped" },
]
commit = "a0cea48<33 hex>"
verified_at = "2026-10-12"
```

| Key | Value |
|---|---|
| `node` | the canonical ID, an entry's key rules (`spec-lock.md` "Entry") |
| `rev`, `spec_hash` | R and `norm_hash` from the node file's `HEAD` blob at the run (accept's step 5) |
| `bound_hash` | "Bound hash" |
| `tests` | the node's runnable `@verifies` tests that ran and passed, `{ file, symbol }`, by (`file`, `symbol`) bytes, distinct, non-empty |
| `commit` | `HEAD`'s full OID at the run: the lock commit's parent |
| `verified_at` | the UTC date, accept's injected clock |

- No `verified_by`: `"owner"` stays binding-only.
- **Bytes** (`render_lock`): blocks after every `[[binding]]`, by `node` bytes, an empty line before each, keys as above; strings escaped as `spec-lock.md` "The file"; `tests` one inline table per line, two spaces in, a trailing comma, `]` alone.
- **Reader** (`parse_lock`): format 2's rules for the rest; a block under format 2: `spec.lock:<line>: [[tested]] needs format 3`; format 3 without one: `spec.lock: format 3 holds no [[tested]] block`; above 3: `spec.lock: format <n> is newer than this build's 3`; an unknown, missing or repeated key, a duplicate `node`, a bad `tests` (empty, unsorted, duplicate, another key, a `file` failing the key rules): `spec.lock:<line>: <message>`; exit 2 as any lock error. Format 1 is rebase's alone (5b) and holds no block.

### Bound hash

WA-4 as decided 2026-10-12 (Q7). Core `bound_bytes` builds, the CLI hashes: `bound_hash` = `b3:` + BLAKE3 hex of

```
specengine-tested/v1 LF
per binding naming the node (any relation, through an alias too), in EntryKey order:
  its key lines as render_lock writes them: node, relation, file, kind, symbol, item_kind (if any)
  its sig_hash, body_hash, deps_hash lines as render_lock writes them (lock-levels.md "Format 2")
  an empty line
```

- **Never**: `levels` (evidence is about behaviour: a body edit under a `[sig]` marker stales it, a narrowed marker does not; SW refinement), `skeleton_hash`, `quote_hash` (textual, its context lines and comments), `body_bytes` (5c's), `file_blob`, `commit`, `verified_*`, any git OID (a SHA-256 repository gives the same `b3:`, `spec-lock.md` "The file"). Same treatment as 4c's `drift_hash` (`lock-levels.md` Q7).
- **At the write**: from `HEAD`'s blobs, raw, as accept's step 5 hashes them (its exits too: a filter giving other items, a blob not UTF-8), before the question.
- **On a read**: over the reader's walk of the disk; a binding whose lock entry's `file_blob` equals the file's on-disk blob takes that entry's digests (4a's level 1, same recipe), else this read's parse, each file once (SW refinement: reads parse only what verify parses, plus bindings without an entry).
- **AC-2, "a comment elsewhere"** under 5b's digests: an `item` holds as under v2 (comments, rustfmt, a neighbour); a `module` binding now holds for comments and rustfmt (`walk3`), where the analysis' raw blob staled on any byte, but any code edit in its file stales it (the file is the unit); a `ron` binding now holds for comments and sibling values (`value_hash`). `quote_hash` would break both, so it is out.

### The state

WA-9. `tested_state` (core, pure), one function for verify, reads and Health; per block the first that applies:

| State | Category | When |
|---|---|---|
| `node_gone` | | no node has the block's ID |
| `unverifiable` | the first non-lockable state, as 8a's `not verifiable <state>` | a binding naming the node is not lockable now (`spec-lock.md` "Lockability", 5b's `bad_levels`) |
| `stale` | `spec_changed` | the node's (R, H) differ from (`rev`, `spec_hash`) |
| `stale` | `code_changed` | the current `bound_hash` differs: an implementation, test or config edited, a binding added or removed |
| `verified` | | none of the above |

No block: no entry (a per-node read's `tested` null).

### Plain verify

WA-11; at 8b's shipping. JSON `tested` after 4a's `delta`: per block by `node` bytes `{"node":"R-12","state":"stale","category":"code_changed","rev":3,"node_rev":3,"commit":"a0cea48<33 hex>","verified_at":"2026-10-12"}` (`rev`, `commit`, `verified_at` the block's; `node_rev` 4a's); `[]` without a block; null under `--changed` and on a lock run cause; `--task` scopes it as rows. Text: each non-`verified` entry after the rows' `hint:` lines, before the summary; an evidence-free lock's text stays byte-identical. No finding, class, count, hint or exit effect.

```
tested | stale code_changed | R-12
tested | unverifiable spec_ahead | R-14
```

### The run's write

WA-5..WA-8, into 8a's "Order":

- **Before the question** (after step 6): `spec.lock`, if present, tracked (else accept's exit 1); read at this build's `format` 2 or 3 and recipes, else exit 2 (format 1, other recipes: 5b's rebase hint); a first lock git ignores: exit 1; **text changed at R** (Q3): a verifiable selected node with an entry or a block at its R and another `spec_hash`: exit 1 naming each, accept's message, nothing run. Each verifiable node's `rev`, `spec_hash`, `bound_hash` from `HEAD`.
- **The question** (step 7) names the write, the one consent for both (ADR-0044; no second question, no `--yes`): `run <n> bound tests of <k> nodes as <template> (timeout <t> s each) in <worktree> on <branch> at <commit>, then commit their evidence to spec.lock? [y/N]`.
- **The write**, after step 10, before step 11 (the event names it), all in one commit:

| Node or run | Lock |
|---|---|
| node `verified` | block `added` or `refreshed`; equal on `rev`, `spec_hash`, `bound_hash`, `tests`: `unchanged`, byte-identical (`commit`, `verified_at` too) |
| node `failed` (a time-out too) | block `removed` |
| node `not verifiable`, `no runnable test`, not selected | untouched |
| run `changed`, `interrupted`, `refused`, `declined`, not started | nothing written, `lock` null |

`[[binding]]` entries and the header but `format` are never touched; no change: no commit; a first lock: header and blocks, created as accept's.

- **The commit** (WA-7): accept's ("The lock commit" in `spec-lock-accept.md`): `git commit --only spec.lock`, hooks run, never `--no-verify`; message `spec: verify tests R-12, R-13` (changed nodes by bytes), an empty line, `Lock: <node>` per changed node, `Lock-mode: tests`, `Decided-by: <committer_ident>`, `Test-command: <template>` (the argv as a TOML array on one line, lock escaping). Accept's four checks, the third a `Lock:` per changed node and `Lock-mode: tests`, the fourth the bytes this run wrote; its failure table and restore rules. No `Proposal:`: it settles no drift item. Accept's message and bytes unchanged.
- **Exits** (WA-8): 8a's verdict and exit stand; exit 0 only for `verified` with a verified lock commit or none needed; a refused write: exit 1, `lock.refusal` its message, `lock_commit` null; an unreadable lock: exit 2 before the question.
- **Output**: text after 8a's summary, per node `lock | <added|refreshed|unchanged|removed> | <node>` by bytes, then `committed <oid>`; JSON `lock` = `{"rows":[{"node":"R-12","change":"added"}],"commit":"<oid>","unchanged":false,"refusal":null}`, null when nothing was to be written; the event's `lock_commit` the verified commit, else null.

### Other lock writers

WA-10, Q3, Q8.

| Writer | Blocks |
|---|---|
| accept (`merge_accept`), so 4c's approve | kept byte for byte; an entry or the node's block at R with another `spec_hash` refuses: "text changed at R" |
| a gone node (`drop_node`) | the node's block dropped with its entries, row `dropped \| tested`; a block alone suffices to drop |
| `--editorial` (`refresh_editorial`); 4b-1's editorial apply | a block at R (the editorial apply: a vouching one, `spec_hash` the old `norm_hash`) takes the new `spec_hash` with the entries, row `refreshed \| tested`; "no entry at R" means neither; asks `(editorial: spec_hash of 3 entries and its tested block)` |
| regen (`merge_stages`) | per node by the `spec-lock.md` "`spec lock regen`" table (base -> ours, theirs; later `verified_at`, then bytes); format 2 and 3 stages of equal recipes merge, the result 3 iff a block remains; rows `<choice> \| <node> \| tested`, a block counted as an entry |
| `spec lock rebase` | below |

Any merge choice is safe: a block misfitting the merged code reads `stale`, never `verified`.

**Rebase** (ADR-0050; SW refinement, pre-approved, review open). Blocks live in format 3, so a rebase meets them across a `code_recipe` (a grammar bump) or a later format. Per block, after the entries: its `bound_hash` recomputed from its node's old entries (key lines, old digests); equal to the stored one with every one of those entries `rebased`: re-derived from the rebased entries, all else kept byte for byte. Else dropped, named, counted: `entry_dropped` (an entry of the node dropped), `unproven` (the old entries do not give the stored hash: a binding never accepted, or accepted at other code). Rows after the entries': `rebased | <node> | tested`, `dropped | <node> | tested | <reason>`; the question adds `; tested: <r> rebased, <d> dropped` when the lock holds a block; JSON `counts` gains `tested_rebased`, `tested_dropped`, a `tested` array after `rows` (`change`, `node`, `reason`); the result is format 3 iff a block remains. It never fabricates evidence: a re-derived block names exactly the code its old entries vouched for.

### Surfaces

Q4, Q5. Reads (7a: `show --bindings`, bundle layer 9, tree, package) and Health (9b `drift`) call `tested_state`, never their own: whichever of 8b and 7a, or 8b and 9b, ships second adds it, with `daemon-keys.json`. Reads: per node `tested`, verify's object less `node`, null without a block; text `tested <state>[ <category>]`. Health: verify's non-`verified` entries verbatim; text `drift | tested | <state>[ <category>] | <node>`. No evidence-level number.

### Fixtures

WA-12: `fixtures/code-a` has no cleanly verifiable node (`bounds.rs:1` a module marker; LAW-BOUND `@3a`, `counter.rs:45`): 8a's temp repos and fake runners, or a minimal fixture; a format-3 golden beside `code-a.lock`.

## Rules and edge cases

- WHEN any writer touches the lock, a block SHALL change only as "The run's write" and "Other lock writers" say: only 8a's run adds one, after its tests passed at `HEAD`; the others keep, refresh `spec_hash` at R, merge, re-derive under proof or drop.
- WHEN no block exists, every lock byte and verify's text SHALL equal 5b's (JSON gains `"tested":[]`).
- WHEN a block's checks do not all pass, its state SHALL never be `verified`.
- WHEN run from two roots, another cwd, in reverse creation order, under `core.autocrlf=true` or in a SHA-256 repository, the written lock SHALL be byte-identical.
- No source names a runner or a stack file (ADR-0008). Nothing blocks (ADR-0012): no state, claim, approval, gate or check reads `tested`.

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- "Format 3 and the block". M: format 3, or any byte changed, with no block -> `evidence_free_lock_bytes_unchanged` (5b's goldens); a block under format 2 -> `tested_needs_format_3`; format 3 without a block -> `format_3_without_a_block_is_refused`; a duplicate node -> `duplicate_tested_node_is_refused`; `tests` unsorted or on one line -> `tested_block_bytes_match_the_golden`.
- [ ] RD-2 -- "Bound hash". M: `file_blob` hashed -> `comment_elsewhere_keeps_verified`; `quote_hash` or `body_bytes` hashed -> `neighbour_comment_and_rustfmt_keep_verified`; `levels` honoured -> `body_edit_under_a_sig_marker_stales_evidence`; `verifies` or `configures` dropped -> `test_or_config_edit_stales_evidence`; a git OID hashed -> `sha256_repository_same_bound_hash`; an entry's digests at another blob -> `entry_digests_reused_only_at_its_blob`.
- [ ] RD-3 -- the write. M: a failed node's block kept -> `failing_test_removes_evidence`; an equal block rewritten -> `equal_evidence_makes_no_commit`; a `[[binding]]` touched -> `test_run_never_touches_bindings`; a write after `changed`, `interrupted`, `refused` or `declined` -> `no_evidence_without_a_completed_run`; a not-verifiable node's block removed -> `unverifiable_node_keeps_its_block`.
- [ ] RD-4 -- the door. M: a second question or `--yes` -> `one_question_covers_the_write`; MCP, HTTP, the daemon or `check` reaching the writer -> `only_terminal_run_writes_tested` (source scan); a block over text changed at R -> `run_refuses_text_changed_at_r`.
- [ ] RD-5 -- the commit. M: one `Lock:` for several nodes -> `test_lock_commit_names_each_node`; `--no-verify` -> `test_lock_commit_runs_hooks`; the event before the commit -> `event_names_the_lock_commit`; accept's message changed -> the existing `lock_commit.rs` tests.
- [ ] RD-6 -- "The state". M: `spec_hash` ignored -> `editing_the_node_stales_evidence`; an unlockable binding counted -> `unlockable_binding_never_verified`; `stale` before `unverifiable` -> `unverifiable_wins_over_stale`; verify and a read disagree -> `tested_state_is_one_function`.
- [ ] RD-7 -- "Other lock writers". M: accept rewrites a block -> `accept_keeps_tested_bytes`; a gone node's block kept -> `gone_accept_drops_tested`; the editorial refresh skips it -> `editorial_refreshes_tested`; regen loses a side -> `regen_merges_tested_per_node`; format 2 against 3 refused -> `regen_merges_format_2_and_3_stages`.
- [ ] RD-8 -- rebase. M: a block kept with its old hash -> `rebase_rederives_bound_hash`; kept with an entry of its node dropped -> `rebase_drops_tested_with_a_dropped_entry`; re-derived without the proof -> `rebase_drops_unproven_tested`.
- [ ] RD-9 -- verify, exits. M: a stale entry changes verify's exit -> `tested_never_changes_verify_exit`; `tested` under `--changed` -> `changed_verify_tested_is_null`; an evidence-free run's text changed -> `evidence_free_verify_text_unchanged`; an unverified lock commit exits 0 -> `unverified_lock_commit_exits_1`; the lock read after the question -> `unreadable_lock_exits_2_before_the_question`.
- [ ] RD-10 -- dependencies. M: a new crate, or BLAKE3 in core -> the pins in `build_graph.rs`.

test-engineer:

- [ ] TE-1 -- AC-8 end to end with 8a's fake runners (8a's TE-2 "`spec.lock` untouched" becomes this): `report_run` alone -> no block; a pass -> a block, one commit passing the four checks, plain verify `verified`; the text edited, no raise -> `stale spec_changed`; raised, markers lagging -> `unverifiable spec_ahead`; the implementation, the test or a bound RON value edited, a `@verifies` added -> `stale code_changed`; a comment elsewhere, rustfmt, a sibling RON value -> `verified`; a failing run -> the block removed. Each step names its mutation.
- [ ] TE-2 -- two roots, another cwd, reverse creation order, `core.autocrlf=true`, a SHA-256 repository: identical lock bytes, stdout, JSON.
- [ ] TE-3 -- a hook refusing the commit -> bytes restored, exit 1, `lock_commit` null, the event written; `HEAD` moving during the run -> nothing written; text changed at R -> exit 1, nothing run. After each, `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs, index unchanged.
- [ ] TE-4 -- blocks present under accept, `--editorial`, a gone node, regen (format 2 against 3), and a rebase of a simulated grammar bump (5b's TE-5 pattern) giving `rebased`, `entry_dropped`, `unproven`; a drift approve if 4c has shipped, 4b-1's editorial apply if 4b-1 has.
- [ ] TE-5 -- reads and Health show verify's state (whichever ships second); `daemon-keys.json` updated.
- [ ] TE-6 -- the genre scan green; no pilot run (pilots have 0 bound tests).

spec-writer:

- [x] SW-1 -- this spec, no ADR (WA-1).
- [x] SW-2 -- 2026-10-12, pointing here: `verify-tests.md` (":21", "Event", "Output", "Out of scope"), `bundle-bindings.md` (sync, "Out of scope", Q10, "Order"), `health-drift.md` (`drift`, "Text"), `drift-queue.md` ("Out of scope", "Order"), `rev-discipline.md` (editorial refresh, the rule, RD-1, RD-4), `lock-levels.md` ("Out of scope"), 08:41. `spec-verify.md` untouched: "Plain verify" here.
- [ ] SW-3 -- at shipping: `spec-lock.md` ("The file", "Entry", a `[[tested]]` section, regen, "Known limits"), `spec-lock-accept.md` (step 6, `--editorial`, "A gone node", "Exits"), 4a's verify canon (`tested`), 5b's rebase canon (blocks), `architecture-code.md#spec-lock`, "Tests as evidence"; 05:75, :194, :196-207; 06:140; 07:38; 08:77; the core and CLI READMEs; `CLAUDE.md` State.
- [x] SW-4 -- `export index`, `check` clean.

## Risks

- Forgery: the gate skips `spec.lock` (`spec-lock.md` "Known limits"); a hand-written block reads `verified`. AC-8 holds for SpecEngine's writers only (cooperative, ADR-0034).
- Read cost: each read hashes a tested node's bindings; entries at their `file_blob` cost nothing, a binding without an entry parses its file.
- CRLF checkouts: as 4a/5b's verify, a multi-line literal's digest differs on disk; module and RON no longer stale wholesale.
- A project hook stopping on any Fix row refuses the lock commit: the evidence is lost, the event stays. `HEAD` moving during a long run: nothing written.
- Churn: regen conflicts on blocks; 4a's JSON goldens, 5b's rebase JSON, `daemon-keys.json` change at shipping.

## Out of scope

Batching; MCP, HTTP, UI or CI triggers; evidence-level numbers (05:196-207); `impl_status`; `needs:`; drift items for stale evidence; a provenance audit of blocks; per-binding blobs in a block; running module or RON bindings (8a).

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 lock format | 3 only when a block exists, after 5b's 2; none: 2, byte-identical; regen merges 2 and 3 (the analysis' "2 or 1", adapted 2026-10-12) | always 3: every lock changes on its next write, a 5b build refuses evidence-free locks |
| Q2 `bound_hash` scope | all relations, as 8a's verifiability | without `configures`, a RON edit keeps `verified` |
| Q3 a block vouches (ADR-0043) | yes: 4b-2's rule, the editorial refresh, "text changed at R" in accept and the run | a meaning change on a node with only test evidence passes the hook; the next run vouches for the new text |
| Q4 reads, Health | whichever of 7a or 9b and 8b ships second | AC-8 waits for 6a, then 7a |
| Q5 evidence levels (05:196-207) | none; the state only | a ranking from 2 of 5 sources (level 2 needs 10b) |
| Q6 what staleness ignores | `Cargo.lock`, the toolchain, the template, `timeout`, unbound code the tests reach; `commit` and `Test-command:` the audit trail | every bump stales all evidence; core learns a stack file (ADR-0008) |
| Q7 `bound_hash`'s code part | key lines, `sig_hash`, `body_hash`, `deps_hash` as rendered; never an OID, `file_blob`, `commit` (2026-10-12, as 5b's Q7) | `file_blob`: a comment elsewhere stales; `quote_hash`: a neighbour's comment does |
| Q8 rebase vs blocks | re-derived under the old-hash proof when each entry was rebased, else dropped, named, counted | dropping all loses every block at each grammar bump; keeping old hashes: all `stale` |

Q1-Q8 (2026-10-12) decided under the owner's pre-approval, review open: Q1 adapted, Q7 and the order set by the orchestrator; Q8 and "SW refinement": the spec-writer's. WA-n: the analysis' working answers (2026-10-12), glossed where cited.

## Implementation

Not started. To record: modules, deviations with reasons.
