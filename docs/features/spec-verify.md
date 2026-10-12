---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 4a (WA-1..WA-13; Q1-Q3, and Q4-Q7 of the 2026-10-12 refresh against shipped slice 3, decided under the owner's pre-approval, review open); 05 s5.3; 07 s2; 08 s2, AC-14
adrs: [ADR-0042]
---

# spec-verify: drift between spec.lock and the tree

## Why

`spec.lock` (slice 3, `docs/canon/spec-lock.md`, `docs/canon/spec-lock-accept.md`) records what was verified at a commit; nothing reads it back, so drift (05 s5.3) stays invisible, slices 5, 7, 8, 9 lack a judge, and AC-14's exit 2 is open.

Delivered: `spec verify [--changed] [--json]`, read-only: a state per lock entry and marker binding, findings Fix, Check or Pre-existing, exit 0/1/2 under `[verify] mode`; ADR-0042 bounds what a finding may stop (`architecture.md#control`, detail `architecture-code.md#verify`). Built on shipped slices, read in their canon: `code-bindings.md`, `node-revision.md`, `spec-lock.md`, `spec-lock-accept.md`. Slice 4 split (Q1): 4a this, 4b `rev-discipline`, 4c `drift-queue`.

## Description and interactions

- **An agent** runs `spec verify --changed` before handing in; **the owner** follows the hints or sends the code back; **a project** opts its own CI or hook into stopping on Fix (a hook: "Rules"). No `[code]`, `[verify]` here (WA-12).
- **Crates** (WA-11). Core: the pure `[verify]` reader and judge (check's `Mode`), `parse_lock`. Store: `hash-object --no-filters --stdin-paths` (no `-w`); check's base reader `Staged::head_only` (`rev-parse`, `ls-tree -r -z`, one `cat-file --batch`) extended to code, manifests, `spec.lock`; all under `GitEnv`, never `WorktreeGit` (its `blob_at`, proposal-apply's, drops the caller's `GIT_*`; slice 3's `ls-files`, `status`, `check-ignore` lie outside `architecture-code.md#sync-in-reads`' git list). CLI: the command over `code_walk::read_tree`, the binding, `hash_bound_items`, `orphan_errors`, `revision`, `norm_input`, accept's key builder, `read_lock` (made `pub(crate)`), `build_recipes`. No tree-sitter in core or store, no new crate or dependency.
- **Seams** (RD-10) for 4b, 4c, 7a, 7b, 9b: (a) core's judge is pure over per-key facts from any source (this cascade, 4b's index OIDs, 7a's indexed `ast_hash`); (b) store's base reader takes any commit (7b generalises it); (c) CLI's `pub(crate)` judge takes an already-walked `CodeTree` plus a base, never calling `read_tree` (9b, `health-drift.md` "One walk").
- **Unchanged**: `spec check` (`CHECK_CODES` 35; 36 once 4b ships `rev-not-raised`), the hook, CI, `INDEX_FORMAT`, MCP, HTTP, UI, plugin, the gate. **Writes** only the data directory (index refresh): never `spec.lock`, the worktree, a git object, a queue row or event (WA-10, ADR-0041).
- **Overridden live-spec lines**, fixed at shipping: 05:160 (`ms`, `spec check --base`), 05:173 (text level), 05:188 (exit 1 "something blocks"; the backlog: 4c), 07:104 (`--changed PATH`, `--task`, `--tests`), `spec-check.md` "Not checked yet".

## Data

### The verify table

WA-1, ADR-0042.

```toml
[verify]
mode = "enforce-introduced"   # observe (default) | enforce-introduced | enforce
```

Closed: another key (`tests`, `timeout` too, until 8a `verify-tests.md`), a wrong type or value -> `specengine.toml:<line>: message`, exit 2 in `spec verify`; other commands only name-check `verify` (`project_toml.rs` `RawFile`; else a `[verify]` config fails every read). No table: `observe`.

### Command and rows

WA-2, WA-3. `spec verify [--changed] [--json]` (+ `--root`, `--config`). Plain: the disk against `spec.lock`; `--changed`: judged against `HEAD` too ("Base"). No PATH (07:104) or REF (2026-10-10: 6b adds `--node REF [--symbol QPATH]`, plain only, and the row keys `ast_hash`, markers' `id_state`, `rev_state`: `code-tools.md`); `--staged`: 4b (`rev-discipline.md`); `--task`, `--tests`: 8a (`verify-tests.md`, ADR-0044).

One row per lock key (`spec-lock.md` "Entry": `node`, `relation`, `file`, `kind`, `symbol`, `item_kind`) from a lock entry or a marker binding of the walk; markers sharing a key are one row. Keys by accept's own builder (`classify_found`'s, shared): the canonical ID, never an alias; an unresolved ID keys on its token. Order: core `EntryKey`'s `Ord`, null first (as `item_kind`). A keyless marker (`unanchored`, a Rust `cannot_verify`): one row per node, relation, file, kind, category, printed as accept's (`spec-lock-accept.md` "Steps" 8: kind `unanchored`/`cannot_verify`, symbol the category); JSON `kind`, `symbol`, `item_kind` null (SW refinement). A key `key_problem` refuses (a path holding LF): `unlocked` (Q6). Skipped, counted: `queue`, `other_project` IDs; mentions never; nodes without bindings unlisted (Health: `queue` IDs and unbound nodes are 9a `health-code`'s rows 4-5, 2026-10-10).

### States

WA-4. Per row: R, H the node's revision and `norm_hash` ("Spec side"); E its lock entry (`rev_L`, `spec_hash_L`, `file_blob`, `ast_hash`); N each marker's `@N`. **Code changed**: E exists, the blob OID differs from `file_blob` and, for `item`, the re-parsed digest ("Cascade") from `ast_hash` (`module`, `ron`: the blob alone); no E: never. **Spec changed**: (R, H) differs from (`rev_L`, `spec_hash_L`). **Raised**: a marker names R > `rev_L`. **Lagging**: a marker N < R. First that applies:

| State | `category` (`detail`) | When | Class |
|---|---|---|---|
| `cannot_verify` | `item_error` (the hash's first `ErrorCategory`, snake case) | the bound item `has_error()` (`code-bindings.md` "Rust binding" rule 7, a keyless marker); E of that marker's node, relation and file naming an item with a parse error (accept's `keep_broken`), never `broken_binding`; RON `nesting_too_deep` | cannot |
| | `ambiguous_symbol` (`path_attribute`, `unrooted`, `duplicate`); `ambiguous_id` | an ambiguous qpath or RON path; an ID naming several nodes | |
| | `rev_unknown`, `hash_null` | R unreadable; H null (`docs/canon/node-revision.md` "Null") | |
| | `unreadable` (`unreadable`, `not_utf8`, `outside_code`) | the file unreadable, not UTF-8, or on disk outside `[code]` | |
| | `hidden_by_error` | E's symbol missing from a file with `orphan_errors > 0` | |
| `predated` | | a marker N > R | Fix |
| `bad_marker` | `dangling`, `homoglyph`, `bad_rev`, `unanchored` (its category), `bad_levels` | `code-bindings.md` "ID state", "Revision state", "Rust binding" | Fix |
| `broken_binding` | `node_gone`, `file_gone`, `symbol_gone`, `marker_gone` | E, no binding: `node` no node's `id` (deleted, renamed, an alias now); file absent; symbol absent, no orphan error; no marker | Fix |
| `unlocked` | (`key_problem`) | a key `key_problem` refuses, never hashed; else no E, not lagging | Check |
| `spec_ahead` | `marker_behind`, `spec_changed`, `same_revision` | lagging, code unchanged; spec changed, not raised, code unchanged (`same_revision`: R = `rev_L`) | Check |
| `code_ahead` | `code_changed`, `raised` | code changed, spec unchanged; spec changed and raised (05:188: not drift) | Check |
| `conflict` | | spec and code changed, not raised | Fix |
| `ok` | | neither changed, not lagging | |

### Cascade and spec side

WA-5, Q4. Level 1, the blob OID: on disk one `git hash-object --no-filters --stdin-paths`; at `HEAD` `ls-tree`'s. Equal: unchanged, nothing parsed or hashed. Level 3, an `item` whose blob changed: its file re-parsed (only such bound files: the walk keeps no tree) and `hash_bound_items` over it, the whole item, levels ignored until slice 5; never the walk's item state, where an error-free item is `NotHashed`, neither a hash nor a refusal. No level 2: lock format 1 has no text hash, and 05:173's fallback would pass an unparseable item. `compared`: `blob` (`module`, `ron` always), `ast`, null (no E, decided earlier).

WA-8, ADR-0040. R by `revision`; H = `norm_input` + `b3_hash` over a fresh parse of the bytes read (`HEAD`'s blob for the base), never the index's column; IDs by `spec show`'s resolver (CLI README "Rules").

### Base

WA-7. `--changed` reads `HEAD` by the store base reader, one `cat-file --batch`: documents as `spec check --changed`'s base (`spec-check-git.md` "The changed check"); `.rs`/`.ron` under the code roots; every `Cargo.toml` under a root or in a directory at or above one, up to the project root (qpaths depend on them: `code-bindings.md` "The walk", accept's inputs); `spec.lock` (absent: no E). Judged with the current config, scheme and `[code]`; a file's walk result reused at `HEAD` only if its bytes and every manifest are equal, else bound from `HEAD`'s blob. `HEAD`'s rows are judgeable alone: 4c judges `HEAD` only (`drift-queue.md` "spec propose drift"). **Pre-existing**: `HEAD` gives the same key, state and category. Unborn: all introduced. A partial base, `HEAD`'s lock invalid: run causes. Mode: the stricter of the current and `HEAD`'s (unknown: a `note:`). A merge or rebase: still `HEAD`. **Unmerged** (Q5): every run's batch, plain too, asks `:2:spec.lock`, a stage only an unmerged file has; present: a run cause.

### Verdict and exit

WA-6, Q2, ADR-0042.

| Verdict | Exit | When |
|---|---|---|
| `clean` | 0 | every row `ok` |
| `observed` | 0 | findings, none blocking |
| `blocked` | 1 | `enforce`: a Fix row; `enforce-introduced`: a Fix row not Pre-existing (plain: as `enforce`, ``note: mode `enforce-introduced` has no base without --changed: judged as `enforce` ``) |
| `cannot-verify` | 2 | a `cannot_verify` row, or a run cause; wins in every mode |

Check rows never block. **Run causes**, `cannot:` lines in this order, normative (7a's note names the first, `bundle-bindings.md` "Degrade"): (1) a config error, `[code]`'s or `[verify]`'s; (2) no `[code]`, `code-bindings.md` "The `[code]` table"'s exact text; (3) a walk gap; (4) no worktree, no `git`, a git failure; (5) `spec.lock is unmerged: run spec lock regen`; (6) `read_lock`'s: `cannot read spec.lock: <error>`, `spec.lock: not UTF-8: SpecEngine writes it`, `spec.lock:<line>: <message>`, another `format` or recipe (both named); (7) a partial base, an invalid `HEAD` lock included; (8) HOME or the data directory. Usage, discovery: stdout empty; later the report is printed with its `cannot:` lines (`spec-check-cli.md` "spec check").

### Output

WA-9. **Text**, stdout: each non-`ok` row in key order as `<class> | <state>[ <category>][ <detail>] | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>]` (class `fix`, `check`, `cannot`, prefixed `pre-existing `; keyless: "Command and rows"); the `cannot:` lines; per state present one `hint: <state>[ <category>]: <next step>`, matching what accept does (`spec-lock-accept.md`): commit, then `spec lock accept <ID>` on a terminal (`code_ahead`, `unlocked`, `broken_binding`); `conflict`, `spec_ahead same_revision`: `--editorial` or a proposal raising `rev` first (accept refuses text changed at R); `node_gone`: commit first (gone only when `HEAD` agrees); else restore, raise or lower the marker, `spec refs <ID>`; `unlocked key_problem` names the field, never accept; other recipes: no accept. Once, when a row is `code_ahead`, `spec_ahead`, `conflict` or `broken_binding`, after them `hint: drift: spec propose drift queues it for the owner` (4c, `drift-queue.md` WA-14); the summary:

```
fix | conflict | R-12 | implements | crates/game/src/stamina.rs | item game::stamina::regen_system function_item
pre-existing check | unlocked | R-20 | configures | assets/data/movement.ron | ron root.stamina.regen_per_second
verify: blocked [enforce-introduced]: 1 fix, 1 check (1 pre-existing), 0 cannot, 11 ok; 9 by blob, 3 by ast; 1 skipped
```

**JSON** (exit 0, 1, and 2 after discovery): every key present, absent null, uncapped, no timing (CLI README "Exit codes and streams"):

```json
{"mode":"enforce-introduced","base":"HEAD","verdict":"blocked",
 "counts":{"rows":13,"ok":11,"fix":1,"check":1,"cannot":0,"pre_existing":1,"skipped":1},
 "cascade":{"by_blob":9,"by_ast":3},
 "rows":[{"node":"R-12","relation":"implements","file":"crates/game/src/stamina.rs","kind":"item",
   "symbol":"game::stamina::regen_system","item_kind":"function_item","state":"conflict","category":null,
   "detail":null,"class":"fix","pre_existing":false,"markers":[{"line":41,"rev":2}],"node_rev":3,
   "lock":{"rev":2,"commit":"a0cea48<33 hex>","verified_at":"2026-10-10"},"compared":"ast"}],
 "delta":{"symbols":null,"bindings":{"moved":null,"ambiguous":[],"lost":[],"stale":[<R-12's six key fields>]},
   "affected_nodes":["R-12"],"tests_reaching":null},
 "cannot":[],"notes":[]}
```

- `rows`: every row, `ok` too; `markers` `[]` for a lock-only row, `rev` null if unversioned; `node_rev` null if unknown or gone; `lock` null without E; `class` null for `ok`. `base`: null, `"HEAD"`, `"unborn"`.
- `delta`: 05:160's keys but `ms`, over rows not Pre-existing (SW refinement): `lost` = `broken_binding`, `stale` = `code_ahead`, `conflict`, `ambiguous` = `ambiguous_*`, each a list of row keys; `affected_nodes` sorted distinct resolved nodes of non-`ok` rows; `bindings.moved` null until 5c, `tests_reaching` until 7a (`bundle-bindings.md`), `symbols` until 7b (`impact.md`; `--changed` only).

## Rules and edge cases

- WHEN a row or the lock cannot be verified ("States", run causes), the system SHALL never report `ok` and SHALL exit 2 in every mode (AC-14); two broken items are never compared.
- WHEN another item of the file has an error, a bound error-free item SHALL be judged as in a clean file (an assumption TE-3 pins).
- WHEN run from two roots, another cwd or in reverse creation order, the output SHALL be byte-identical.
- WHEN a project hooks verify into its commits, the hook SHALL run `spec verify --changed` under `enforce-introduced` (4b: `--staged`; Q7): accept's lock commit runs hooks and covers one node, so plain verify or `enforce` refuses it while another node has a Fix row (two `conflict` nodes deadlock). This repository's hook runs `spec check` only (TE-9).
- Named limits, each read `code_ahead`: under `core.autocrlf` a multi-line literal holding CRLF (blobs differ, items hashed); under `ident` or LFS an expanded `$Id$` in a bound item, an LFS file (accept hashes `HEAD`'s raw blob, verify the disk: `spec-lock-accept.md` "Known limits").
- No `spec.lock` (WA-13) is no error. Genre-neutral: no stack, kind or prefix in states or hints. Each call walks the code tree once, re-parses the bound files whose blob changed, and with `--changed` binds `HEAD`'s differing files.

## Acceptance criteria

rust-developer:

- [ ] RD-1 -- "The verify table". M: default `enforce` -> `verify_mode_defaults_to_observe`; `verify` not in `RawFile` -> `verify_table_is_name_checked` (`spec show` with `[verify]`); `tests` accepted -> `verify_unknown_key_is_refused`.
- [ ] RD-2 -- the pure core judge, "States". M: `cannot_verify` as `ok` -> `cannot_verify_is_never_ok`; `conflict` when a marker names R -> `raised_marker_with_both_changed_is_code_ahead`; lagging with equal hashes `ok` -> `lagging_marker_is_spec_ahead`; `std::fs` in core -> `model_and_core_sources_do_no_file_io`; queue IDs judged -> `queue_ids_are_skipped`.
- [ ] RD-3 -- AC-14. M: exit 0 under `observe` -> `unparseable_bound_item_exits_2_in_every_mode`; `broken_binding` behind an orphan error -> `symbol_hidden_by_a_parse_error_cannot_verify`; non-UTF-8 as gone -> `unreadable_bound_file_cannot_verify`; recipe unchecked -> `lock_of_another_recipe_exits_2`.
- [ ] RD-4 -- the cascade. M: every item hashed -> `blob_equal_entries_are_not_hashed` (`cascade.by_blob`); a RON blob change `ok` -> `blob_only_binding_change_is_code_ahead`; comments hashed -> `comment_edit_is_ok`.
- [ ] RD-5 -- "Base". M: base ignored -> `drift_at_head_is_pre_existing`; a Pre-existing Fix blocking -> `pre_existing_fix_passes_enforce_introduced`; `HEAD`'s mode ignored -> `head_stricter_verify_mode_wins`.
- [ ] RD-6 -- exits. M: exit 1 under `observe` -> `observe_never_exits_1`; a Check blocking -> `check_findings_never_block`; no `[code]` exit 0 -> `verify_without_code_table_is_exit_2`.
- [ ] RD-7 -- output. M: an `ms` key -> `verify_json_has_no_timing`; walk order -> `verify_is_byte_identical_across_roots_and_order`; a key dropped -> `verify_json_every_key_present`.
- [ ] RD-8 -- read-only. M: the lock rewritten -> `verify_never_writes_the_lock`; a file under the root -> `verify_leaves_the_worktree_untouched`; an event -> `verify_writes_no_queue_row`.
- [ ] RD-9 -- no new crate; check unchanged. M: a new dependency -> the CLI pin in `build_graph.rs`; `CHECK_CODES` stays 35.
- [ ] RD-10 -- the seams ("Description"). M: the judge walks itself -> `verify_judges_a_given_walk`; facts from another source change a state -> `judge_is_source_neutral`; keys built apart from accept's -> `golden_lock_verifies_all_ok` (`fixtures/spec-lock/code-a.lock` over `fixtures/code-a/`, every row `ok`).
- [ ] RD-11 -- slice 3's carried review items, code only, code-reviewer checks (`features/spec-lock.md` "Implementation"). CLI `lock.rs`: (1) `:1810-1815` "changed meanwhile" names the index entry left too; (2) `:1701-1706` a failed commit, its branch unreadable or naming no commit: "unverified, left as written", not restored; (3) `:1845` "not restored (see the warning)" when a restore step failed; (4) `:1779` a commit deleting `spec.lock` fails a check (`blob_oid` `None`), not `Unverified` (TE-11); (5) `:41-52` step 6 names "branch unmoved, HEAD moved: nothing restored" and "a moved branch unverifiable: left as written". (6) Store `worktree.rs:26-28`: "every read" gets `GIT_OPTIONAL_LOCKS=0`. (7) Pointers to `docs/features/spec-lock.md` retargeted to the two canons: core `lock.rs:1`, `lib.rs:56`; store `worktree.rs:11`; CLI `lib.rs:27,120`, `lock.rs:1-8` (its "task spec `drift-queue`" too), `lock_regen.rs:1`; optional, code `lib.rs:43`. A cited heading is verbatim on one line: `doc_pointers.rs` pins it (`docs/README.md` "Enforcement").

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/` and slice 3's golden lock `fixtures/spec-lock/code-a.lock`: a scenario per "States" row and category, expected text and JSON, each naming its mutation.
- [ ] TE-2 -- TE-1 with CRLF files.
- [ ] TE-3 -- AC-14, exit 2 in every mode, report printed: a bound `fn` made unparseable; two different broken bound items; an earlier unclosed item hiding the bound one; an error in an unbound neighbour (bound item judged); a non-UTF-8 bound file; a recipe mismatch. M: an `is_extra()` filter; `cannot_verify` -> `ok`.
- [ ] TE-4 -- {three modes} x {plain, `--changed`} x {clean, Check only, Fix introduced, Fix pre-existing, `cannot_verify`}; unborn `HEAD`; a merge in progress; an unmerged `spec.lock`; two `conflict` nodes and a project pre-commit hook running `spec verify --changed` under `enforce-introduced`: `spec lock accept --editorial` of one commits, exit 0; the hook plain under `enforce` refuses it (Q7).
- [ ] TE-5 -- AC-2 at verify level: rustfmt (default, `max_width = 60`), a comment or neighbour edit -> `ok`; a body edit -> `code_ahead`; counters pinned; autocrlf: same verdicts, the literal limit pinned.
- [ ] TE-6 -- two roots, another cwd, reverse creation: byte-identical stdout, stderr, JSON.
- [ ] TE-7 -- after each scenario `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, `spec.lock` bytes, queue rows unchanged.
- [ ] TE-8 -- `fixtures/spec-c` exits 2 naming `[code]`; no fixture prefix, kind, `cargo`, `nextest`, `bevy` or path literal in the sources. M: a "mechanic" literal.
- [ ] TE-9 -- `spec check` output equal with drift present; `.githooks/pre-commit` untouched.
- [ ] TE-10 -- pilots, manual (`#[ignore]`, release, foreground, scratch clones), nodes accepted in its own clone first (slice 3's TE-11 never ran, consent declined), else only `unlocked`: counts by state, wall time as AC-10's baseline, read-only proof; no pilot name committed.
- [ ] TE-11 -- RD-11 (4) `lock_commit_deleting_the_lock_fails_a_check`: accept over an existing lock, a pre-commit hook drops `spec.lock` from the commit: exit 1 naming the commit and the failed check, `git reset --soft <commit>^`. M: absent read as unreadable ("made but cannot be verified").

spec-writer:

- [x] SW-1 -- this spec; refreshed 2026-10-12 against shipped slice 3 (`health-drift.md` RD-13 renamed, Q4).
- [x] SW-2 -- ADR-0042, `architecture.md#control` in the same change.
- [x] SW-3 -- 08 s2's slice list (4a, 4b, 4c); `spec-lock.md`'s "Out of scope" and Q3's amendment.
- [ ] SW-4 -- at shipping: Tier 2 `docs/canon/spec-verify.md`; the overridden lines; 08:44, AC-14 met; CLI, core and store READMEs; `CLAUDE.md` State.
- [x] SW-5 -- `export index`, `check` clean.

## Risks

Noise: RON and modules by blob, `[sig]` hashed whole until slice 5, all `unlocked` on adoption. A grammar bump holds exit 2 until 5b's `spec lock rebase`, since accept refuses a lock of other recipes (`spec-lock.md` "Known limits"); a bound item among the ~1 % of valid Rust tree-sitter-rust fails holds it while bound. A changed bound file is parsed twice (Q4). Vocabulary: `blocked` beside ADR-0012; `unlocked` beside accept's `not locked`.

## Out of scope

- **4b `rev-discipline`** (ADR-0043, AC-13): the rule in `check --staged`, `--changed` (ADR-0019); apply raising `rev`; an editorial approve; `spec bump`; `verify --staged`. Decided there: an editorial apply commits text and refreshed `spec_hash` together (amending ADR-0041's "lock alone"); the rule judges only nodes a `HEAD` entry vouches for; this repository's hook trigger stays.
- **4c `drift-queue`** (ADR-0045): `spec propose drift` (MCP `report_drift`) queues `HEAD`'s rows, this command's Pre-existing, as `drift` items: the backlog; approve runs the same accept; other outcomes become draft tasks (`spec reject --follow-up`). This command stays read-only (RD-8), gaining one hint ("Output").
- 5 (`hash-levels.md` Q1): 5a levels, fingerprints, the RON recipe; 5b lock format 2, level-aware verify; 5c `MOVED`/`AMBIGUOUS`/`LOST`; never a level 2. 6b `code-tools`: `--node REF [--symbol QPATH]`, MCP `check_binding`. 7 (ADR-0046): 7a `bundle-bindings`, `tests_reaching`, the plain state as `sync` in bundles, show, tree, the package; 7b `impact`, `delta.symbols` (`symbol-index.md` Q6), `spec impact`. 8a `verify-tests` (ADR-0044): `--tests`, `--task`; 8b `tested-evidence`. 9a `health-code` (Health rows 4-5); 9b `health-drift`: plain findings as row 6, the judge in-process over 9a's walk, never the verdict (ADR-0047). Phase 4: drift screens, diffs since `lock.commit`. Not planned: a PATH, a debt baseline, a watcher, `spec check --base REF`.

## Open

| Point | Answer |
|---|---|
| Q1 slice 4 | split: 4a this (ADR-0042), 4b `rev-discipline` (ADR-0043), 4c `drift-queue` (ADR-0045); 4a first |
| Q2 which `cannot_verify` stops | any row in scope, locked or not, Pre-existing or not: exit 2 (AC-14) |
| Q3 orphans | `spec lock accept <gone ID>` drops a gone node's entries, same `[y/N]` and commit; gone only when `HEAD` agrees (`spec-lock-accept.md` "A gone node") |
| Q4 hashing needs a tree; the walk keeps none | re-parse only bound files whose blob changed; 9b's RD-13 counts `read_tree` calls (`health_calls_read_tree_once`) |
| Q5 unmerged `spec.lock` | `:2:spec.lock` in the base `cat-file --batch`: inside the canon's git list, no canon change |
| Q6 a key `key_problem` refuses | `unlocked`, detail `key_problem`, the hint naming the field |
| Q7 hooks vs accept's lock commit | hooks run `--changed` under `enforce-introduced` (4b: `--staged`); TE-4 pins it; no ADR |

Q1-Q3 (2026-10-10) and Q4-Q7 (2026-10-12) decided under the owner's pre-approval, review open; WA-n: the analysis' working answers (2026-10-10); "SW refinement": the spec-writer's.

## Implementation

Not started. To record: modules, TE-10's figures, deviations with reasons.
