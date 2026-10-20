---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b-2, split from lock-levels 2026-10-16 (its Q21; WA-1, WA-7, WA-8; Q1, Q2, Q4, Q8 2026-10-12; Q9, Q14-Q16, Q18, Q19 2026-10-16; lock-levels Q22, Q23, Q30); refreshed 2026-10-20 against d45dfce, 5b-1a as built (R1-R14, Q32-Q41, M1-M6; decided under the owner's pre-approval, review open); 05 s3.5; 07 s2; 08 s2
adrs: [ADR-0050]
---

# lock-rebase (5b-2): `spec lock rebase`

## Why

Nothing can carry a lock across a change of format or recipe (WA-1). Since 5b-1a (`lock-levels.md`) every `Some` reader exits 2 on a format-1 lock or another `code_recipe` with core's `REBASE_STEP` (`run spec lock rebase on a terminal`): verify in every mode (cause 6), accept on all three paths (a node, `--editorial`, a gone node), `spec propose drift`, MCP `report_drift`, a drift approve (inside its wrapper), an editorial approve that would refresh an entry (step 6, `LockRead::unrebased`); regen's `UNREBASED` sends a format-1 stage to `git merge --abort, spec lock rebase on the lagging branch`. The command does not exist; a hand edit is forbidden (ADR-0041); a grammar bump strands every lock (`spec-lock.md` "Known limits").

Delivered: `spec lock rebase`, the first migration writer (ADR-0050; `architecture-code.md#spec-lock`), re-deriving each entry from its own `file_blob`; and the base rule that lets a project's verify hook pass its commit (Q14). Order (Q9, Q21; `lock-levels.md` Q22, Q23): right after 5b-1a, closing its window; before 5b-1b (`lock-levels-verify.md`), 5c, 6b-2, 7a, 7b, 8b (which rebases its `[[tested]]` blocks on this: `tested-evidence.md` "Rebase").

## Description and interactions

- **The owner** meets an old lock as exit 2 naming `spec lock rebase` (`docs/canon/spec-lock.md` "Entry", **Reader**) and runs it on a terminal: a row per entry, one question, one verified commit. **An agent** never rebases (no MCP tool or plugin command); after the commit, verify and `report_drift` read the format-2 lock.
- **Crates** (WA-12; "Files"): store `VerifyGit::read_blobs`; core the pure plan; CLI `lock_rebase.rs`, the commit's subject, the base. **Code: no change** (Q36). No new crate, dependency, hook or CI step.
- **One door** (ADR-0050): rebase writes `spec.lock` alone, never via MCP, a role, a hook, CI or the daemon. Its commit completes no `drift` item: completion selects `Proposal:` commits first, then needs `Lock:` and `Lock-mode: accept` or `editorial` (`proposal-drift.md` "Approve"; R5); items raised after it meet new evidence once (its "Known limits").
- **Overridden at shipping**: the canon lines SW-2 lists (R6).

## Data

### `spec lock rebase`

WA-7, WA-8, Q1, Q2, Q4, Q15, Q32, Q40. `spec lock rebase [--json]` (+ `--root`, `--config`); `LockCommand::Rebase` behind `require_terminal("lock rebase")` as regen's.

1. Not a terminal: exit 2, nothing read. Usage, discovery, config (`--config` naming another file too), no `[code]` (`spec refs`' text): exit 2.
2. Git (`Repo::open`): a worktree, `HEAD` born, on a branch, no operation in progress, else exit 2; an unmerged `spec.lock` is a merge in progress: `a merge is in progress in <worktree>: finish or abort it first; nothing written`, regen never named (R7, Q28).
3. **Inputs** `spec.lock`, `specengine.toml` (`dirty_inputs`), tracked and equal to `HEAD`, else each named with its state, exit 1 `…: commit them first; nothing written`; no lock: exit 1 `no spec.lock: nothing to rebase`. No walk; dirty code ignored.
4. **The lock** (Q32, R8): not UTF-8: exit 2, `read_lock`'s `spec.lock: not UTF-8: SpecEngine writes it`. `parse_lock(text, Some(build))` succeeds: exit 0 `nothing to change`, no question. Else `parse_lock(text, None)` fails (unreadable, `format = 3`, `format = 0`): exit 2, that error. Else another `norm_recipe` (`None` compares no recipe): exit 2, the `Some` error verbatim (Q27's (2): no spec blob re-derives `spec_hash`). Left: format 1 at any `code_recipe`, format 2 at another.
5. **Each entry**, in key order, from its own `file_blob` (WA-7): all OIDs through one `read_blobs` (one `cat-file --batch` session, raw, no filters, `GIT_NO_LAZY_FETCH`; failing: exit 2); `Missing` or `Foreign`: dropped `blob_missing` (Q40). Never `HEAD`'s tree, the index, the working tree, nor `commit` (it decays). A blob is parsed once however many entries name it; a parse panicking or giving no tree: exit 2 naming the file and OID, nothing written (Q40).
6. **Kept** byte for byte: key, `rev`, `spec_hash`, `file_blob`, `commit`, `verified_by`, `verified_at`. **Written**: `levels`, the three digests, fingerprints, `body_bytes`, the header (`format = 2`, this build's recipes). **Dropped** (Q1): named with its reason, no entry, so verify shows `unlocked`.
7. Committer identity (else exit 2); rows on stdout; the question on stderr, only `y`, `yes`, no `--yes`; `HEAD`, branch, lock bytes re-read (changed: exit 1 `spec.lock changed while being rebased; nothing written`); "The commit".

**The plan** (core `lock.rs`, names the developer's; R9): the old `Lock` and per key a `LevelCode` or a drop reason in; the new `Lock{format: Two, recipes: build}`, rows and counts out. A kept entry takes a `LevelCode`, so no `EntryCode::Whole` reaches `render_lock`, which renders each entry by its own code unchecked (an `ast_hash` under `format = 2` no reader accepts). Core sees digest strings and level names only.

**Levels** (Q4, Q15). Format 2: kept as written; markers only break ties. Format 1: the union of the effective levels of the blob's markers giving the key (bound to the located unit by `Target::of_rust`/`of_ron`, the entry's relation, the ID the entry's `node` by bytes or by its working-tree resolution, as accept's `node_markers`); one `Invalid` list: dropped `bad_levels`, never the default (accept's rule; `code-identity.md` "Marker grammar"); none (only an alias gone from the corpus: the blob is the accepted one): `sig, body`, what format 1's `ast_hash` compared.

**Location** (Q2, Q36, Q37), in the blob, CLI `lock_rebase.rs`: names by `placed_name_file` over `NamedFile{package: None}`, in-file paths by `QPath::suffixes` (no package `.`; a twin's ` #<discriminator>` kept).

| `kind` | Candidates | Hashed by |
|---|---|---|
| `item` | items of `item_kind` (`code-bindings.md` "Rust binding") whose in-file name is `symbol` or ends it after a `::` | `item_levels` |
| `module` | the file's module, always; each inline `mod` whose in-file path ends `symbol` after a `::` | `module_levels`; a `mod` by its `mod_item`'s `item_levels` |
| `ron` | the anchor whose field path is `symbol` (`code-identity.md` "RON binding"); `Ambiguous`: `ambiguous` | `ron::value_hashes`, its comment range's digest, paired as accept's `FileCodes::of` (R10; `code-hash-ron.md` "RON value") |

- **Ties** (Q37): of several candidates, those a blob marker binds (node and relation as in "Levels"); one left is located, none or several `ambiguous`. Never "longest match": it picks `mod a` for `a.rs`' own module, silently.
- **Cross-check** (Q16, R11): a format-1 `item` entry whose `code_recipe` is this build's v2 string (`<RECIPE> tree-sitter-rust/<v> abi/<n>`: code `RECIPE`, `grammar_info()`) must give its `ast_hash` again through `hash_bound_items` over the located item's index, read by `.digest()` (`HashState` never named), else `hash_mismatch`; never `hash_item` (`hash-levels.md` Q6). Any other lock cannot be cross-checked: the location stands, with one `note: rebase: "<old code_recipe>" is not this build's: located without a hash cross-check`.

| Drop reason, the first that applies | When |
|---|---|
| `blob_missing` | no object of that OID (gc, a partial or shallow clone), or another type (Q40) |
| `not_utf8` | the blob is not UTF-8 |
| `symbol_gone` | no candidate |
| `ambiguous` | the ties leave none or several; an `Ambiguous` RON anchor |
| `bad_levels` | a format-1 entry whose key a marker with `Invalid` levels gives (Q15) |
| `cannot_verify <category>` | the located unit gives `CannotVerify` (a RON file: any rejection) |
| `hash_mismatch` | the cross-check fails |

**Output**: stdout rows in key order, `rebased|dropped | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ | <reason>]`, ending `committed <oid>`; stderr `rebase spec.lock (format 1 -> 2, "<old code_recipe>" -> "<new>": 10 rebased, 1 dropped) at <commit> on <branch> in <worktree>? [y/N]`, both counts always.

```
rebased | GOAL-01 | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::value function_item
dropped | R-20 | configures | assets/data/movement.ron | ron root.stamina.regen_per_second | blob_missing
committed 3f9a1c0<33 hex>
```

`--json`: one document for exits 0 and 1, none for 2 (CLI README "Exit codes and streams"), every key present, absent null, `refusal` as accept's, pinned by `lock_rebase_json_keeps_its_keys`: `{"from":{"format":1,"code_recipe":"…"},"to":{"format":2,"code_recipe":"…"},"counts":{"rebased":10,"dropped":1},"rows":[{"change":"dropped","node":"R-20","relation":"configures","file":"assets/data/movement.ron","kind":"ron","symbol":"root.stamina.regen_per_second","item_kind":null,"levels":null,"reason":"blob_missing"}],"commit":"<oid>","unchanged":false,"refusal":null}`.

**The commit** (Q35, R2). `write_and_commit` and `check_lock_commit` (CLI `lock.rs`) take a subject; today a node is wired into the message (`spec: lock <node>`, `Lock: <node>`), check 3, `CHANGED_WHILE_ACCEPTING`, "appeared while being accepted" and the prompt's `rows_text`. A node: accept's bytes unchanged. The rebase: `spec: lock rebase`, an empty line, `Lock-mode: rebase`, `Decided-by: <committer_ident>`, no `Lock:`, `Proposal:` or caller trailer (no queue item runs a rebase: ADR-0045 runs accept); check 3 "a `Lock-mode: rebase` trailer and no `Lock:`"; "changed while being rebased"; its own rows. `LockMode::Rebase` (trailer `rebase`) stays outside `LockMode::TRAILERS` (S14, Q39); the rebase maps the `LockOutcome` it gets back. The rest is accept's (`spec-lock-accept.md` "The lock commit"): `replace_file`, `git commit --only spec.lock`, hooks run, never `--no-verify`, the four checks, the failure table, the restore rules; `LockConsent`, `LockPrompt` reused. Exit 1: a commit failed, failing a check or unverified; exit 0 only for a verified commit or `nothing to change`. After a refusal `HEAD`, refs, index, status and files are as before.

### The base

Q8, Q14, Q33, Q34 (S4, S5); overrides 4a's cause (7) and 4b-2's `read_head` (`spec-verify.md` "Base", "The index"). `read_head` (CLI `verify_rows.rs`) reads `HEAD`'s lock in Q32's order, not `parse_lock(Some)` alone (`:936-941` at d45dfce; R4). Under `--changed` and `--staged` alike, a `HEAD` lock of format 1 or another `code_recipe` at this build's `norm_recipe`, beside a judged lock (the disk's; under `--staged` the index's) of this build's format and recipes, gives way to the judged lock: `HEAD`'s rows are judged with it, one `note: HEAD's spec.lock is format <n> ("<code_recipe>"): judged with the current lock` on stderr and in JSON `notes` (Q34). A rebase changes no key, `rev`, `spec_hash` or `file_blob`, so each row reads as at `HEAD`: Pre-existing; a hook running `spec verify --staged` (`spec-verify.md` "A project's hook") or `--changed` under `enforce-introduced` passes the rebase commit. `HEAD`'s lock unreadable or of another `norm_recipe`: cause (7), its text as today. A judged lock not this build's: cause (6) as today, `HEAD`'s old lock still (7). An index sharing `HEAD`'s lock (equal OID, `:896-898`) shares one read with `Some` (`verify_rows.rs:1293`).

### Fixtures

Q30 (`lock-levels.md`), R12. Input: `fixtures/spec-lock/code-a.format-1.lock` (5b-1a created it from the old bytes); it and `code-a.lock` stay unchanged. Rebased at its commit on `fixtures/code-a/`, it gives the new `fixtures/spec-lock/code-a.rebased.lock` byte for byte: 9 `rebased`, 2 `dropped bad_levels` (`counter.rs:77` `GOAL-01` `[path, path]`, `bounds.rs:14` `cap-check/PASS-01` `[Sig]`: Q15, S9).

The new file is `code-a.lock` with each entry's `commit` taken from the same key in `code-a.format-1.lock`: 5 lines differ (`GOAL-01` `79ea966e…` -> `85a2c984…`, `LAW-BOUND`'s 4 `20825b3f…` -> `82f6593a…`; `PIECE-COUNTER`'s 4 keep `2ba8a3bc…`); `rev`, `spec_hash`, `file_blob`, `verified_at` equal for all 9 keys (checked 2026-10-20). Levels are the blob markers' (`counter.rs:29` `AIM-01@1 [body]`, `:13` `[sig, body]`, `bounds.rs:4` `[body]`, the rest default), digests accept's. Never hand-written: `rebased_golden_is_accepts_with_format_1_commits` derives it from the two goldens and compares it with the pinned file (a regenerated `code-a.lock` cannot leave it stale); `SPECENGINE_WRITE_REBASED_GOLDEN=1 cargo nextest run -p specengine-cli --test lock_rebase rebased_golden_is_accepts_with_format_1_commits` writes it, after `code-a.lock` (as 5b-1a's `SPECENGINE_WRITE_LOCK_GOLDEN`).

## Rules and edge cases

- WHEN a rebase runs, the system SHALL read code only from each entry's `file_blob` by OID and SHALL drop, never guess, an entry it cannot re-derive (Q1, Q15).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, rebase SHALL give byte-identical locks and output.
- WHEN a rebase commit lands, no `drift` item SHALL be completed by it.
- WHEN `HEAD`'s lock is format 1 or another `code_recipe` and the judged one is this build's, verify SHALL judge `HEAD`'s rows with the judged lock in both base modes (Q14).
- WHEN the rebase renders, every entry SHALL be format 2 by type (R9).
- **The pin** (Q16, R11): `lock_rebase.rs` joins `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`, its array 6 -> 7) with `["item_levels(", "module_levels(", "hash_bound_items("]`; `hash_item(`, `hash_node(` banned; no `HashState`; no `.state` read but those the pin allows.
- ADR-0012: nothing gated beyond the command itself. ADR-0008: no stack, kind or prefix in rows or texts (WA-12).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red; M1-M6, 6 in all; CLI `tests/` unless named):

- [ ] RD-1 -- the sources, `lock_rebase.rs`. **M1** an entry read from `HEAD`'s blob at its `file`, not its `file_blob` -> `rebase_uses_each_entrys_own_blob` (a later commit edits `counter.rs`). Scenarios `rebase_ignores_uncommitted_code`, `rebase_keeps_verified_at_and_commit` (an old date, a decayed `commit`).
- [ ] RD-2 -- the golden, `lock_rebase.rs`: `rebase_of_the_format_1_golden_is_the_rebased_golden` (lock bytes, rows, the question, 9 rebased, 2 dropped), `rebased_golden_is_accepts_with_format_1_commits`. **M2** an `Invalid` list taken as `sig, body` -> `rebase_drops_invalid_level_entries` and the golden test.
- [ ] RD-3 -- drops, `lock_rebase.rs`. **M3** a missing blob exits 2 instead of dropping -> `missing_blob_drops_its_entry`. **M4** the v2 cross-check skipped -> `v2_mismatch_drops_the_entry` (an item's `ast_hash` another well-formed digest). Scenarios `not_utf8`, `symbol_gone`, `ambiguous` (a module and an inline `mod` both marked: Q37), `cannot_verify <category>`, a `Foreign` OID as `blob_missing`.
- [ ] RD-4 -- refusals, scenarios, `lock_rebase.rs`: `rebase_off_a_terminal_reads_nothing`; `other_norm_recipe_exits_2` (the `Some` text verbatim), `format = 3`, not UTF-8: exit 2; `rebase_refuses_an_operation_in_progress` (`Repo::open`'s text); a dirty `spec.lock` or `specengine.toml`, no lock: exit 1; a lock already this build's: exit 0 `nothing to change`, no question.
- [ ] RD-5 -- the commit, `lock_rebase.rs`. **M5** check 3 passing a rebase commit without `Lock-mode: rebase` -> `rebase_commit_without_its_mode_trailer_fails_a_check` (a `commit-msg` hook drops it: exit 1, unverified). Scenario `rebase_commit_names_no_node` (no `Lock:`, no `Proposal:`). Guard, not a counted mutation (Q39): `drift_approve.rs`' rewritten-`Lock-mode` loop gains `Some("rebase")`. `lock_commit.rs`, `lock_exits.rs`, `drift_approve.rs` green after the subject refactor.
- [ ] RD-6 -- "The base", `lock_rebase.rs`. **M6** `HEAD`'s old-format lock as cause (7) -> `head_lock_of_another_format_is_judged_with_the_current_lock`, under `--changed` (the rebased lock on disk) and `--staged` (staged): every row Pre-existing, no cause (7), the note on stderr and in `notes`.
- [ ] RD-7 -- guards, no mutation: the pin (`code_refs_walk.rs`); eval `build_graph.rs` (`core_and_store_have_no_tree_sitter`, `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`); store `tests/verify_git.rs`: `read_blobs` by OID (bytes, missing, foreign).

test-engineer:

- [ ] TE-1 -- `lock_rebase.rs`, temp repos from `fixtures/code-a/` and `code-a.format-1.lock`: a scenario per row and drop reason (`bad_levels` included), text and JSON pinned, `lock_rebase_json_keeps_its_keys`.
- [ ] TE-2 -- the migration on a pseudo-terminal (`PtyRun`): the format-1 golden committed -> verify exit 2 naming the step -> rebase, `y` -> one commit passing the four checks -> verify's keys, states, categories as over accept's golden (rows carry `lock.commit`, which differs: `spec-verify-output.md` "JSON"; exit 2 throughout on code-a's Cannot rows) -> `spec lock accept` of `PIECE-COUNTER`, `LAW-BOUND`, `GOAL-01`: `nothing to change`; `cap-check/PASS-01`: exit 1.
- [ ] TE-3 -- a simulated grammar bump: accept's golden committed as `Variant::OtherCode` -> verify exit 2 naming the step -> rebase, under the note, gives `code-a.lock` byte for byte.
- [ ] TE-4 -- an entry's `file_blob` no object has: `dropped blob_missing`; after the commit verify shows the binding `unlocked`.
- [ ] TE-5 -- new `lock_rebase_hooks.rs` on verify-ecs (Q41; `drift_approve.rs` `lock_commit_under_spec_verify_hooks`' corpus), its accepted lock committed as `Variant::OtherCode` beside a pre-existing Fix row: a project `pre-commit` hook running `spec verify --staged`, and one running `--changed`, under `enforce-introduced` passes the rebase commit, exit 0; under `enforce` the Fix row refuses it: exit 1, bytes restored.
- [ ] TE-6 -- after every refusal and an `N`: `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs, index, `spec.lock` bytes unchanged.
- [ ] TE-7 -- 4b-2's carry-over (`rev-rule.md` TE-3, Q18), `rev_rule_hook.rs`: the rebase commit through a real `spec check --staged --root .` hook under `[check] mode = "enforce"`: clean, the commit made. A scenario, no mutation (Q38).
- [ ] TE-8 -- byte-identical lock, stdout, stderr, JSON: two roots, another cwd, `core.autocrlf=true`, the input's entries reversed (`lock-levels.md` TE-2's twin).

spec-writer:

- [x] SW-1 -- this spec: split 2026-10-16 (`lock-levels.md` Q21); refreshed 2026-10-20 against d45dfce (R1-R14 fixed in place, Q32-Q41, M1-M6).
- [ ] SW-2 -- at shipping (Q19), bytes measured 2026-10-20 at d45dfce:

| File | Now | Budget | Edit |
|---|---|---|---|
| new `docs/canon/spec-lock-rebase.md` (Tier 2, like `spec-lock-accept.md`) | -- | ≤ 4 608 | the steps, "Location", the drops, the output, the commit, the base rule in full; a line in `docs/index-canon-spec.md` (2 831 of 10 240) |
| `spec-lock.md` | 9 979 | ≤ +240, ≥ 2 048 free for 8b | "Built, writing format 2 only:" gains the rebase; "Known limits"' "(5b-2)" a pointer |
| `spec-verify.md` | 11 743 | ≤ +400 | "Base" a pointer; cause (7); "A project's hook" the way out ("Risks"); "Known limits"' "(5b-2)" |
| `spec-lock-accept.md` | 11 728 | ≤ +150 | "The lock commit": the subject, one clause |
| `architecture-code.md` | 11 825 | ≤ +250 | "As built": the rebase built |
| CLI README | 9 934 | ≤ +120 | `lock …` -> `spec-lock{,-accept,-rebase}.md`; the writes line: `rebase` the file and a commit |

  Also 05 s3.5, 07 s2 (`spec lock rebase` after `regen`), 08 s2, `CLAUDE.md` State; this spec compacted, "The base" and "Out of scope" kept as pointers (`anchor-states.md` cites them). No edit to `code-hash.md`, `node-revision.md`, `code-hash-ron.md`, the code or core README (`lock`: `spec-lock.md` holds), nor the store README (10 067; it names no `VerifyGit` reader).
- [x] SW-3 -- no new ADR (ADR-0050 decides this writer; ADR-0012, ADR-0041, ADR-0042 hold); `export index`, `check` clean.

## Files

Lines are d45dfce's.

- **rust-developer**: store `src/verify_git.rs` (`read_blobs` over `Staged::blob`, `git.rs:1134`); core `src/lock.rs` (the plan, the drop reasons); CLI `src/lock_rebase.rs` (new); CLI `src/lock.rs`: the subject (`write_and_commit` `:1813`, `check_lock_commit` `:2124`), `LockMode::Rebase` (`:184-204`), `pub(crate)` for `Repo::open` `:344`, `dirty_inputs` `:1548`, `level_code` `:1333`, `lock_digest` `:1316`, `lock_level` `:1321`; `src/verify_rows.rs` (`read_head`'s lock, `:925-942`), `src/verify.rs` (the judged lock passed in, the note); `src/main.rs` (`LockCommand` `:349-362`, dispatch beside regen's `:1395-1398`), `src/lib.rs` (`pub use` `:293`; `Outcome` arms `:535`, `:584`, `:622`, `:665`, `:707`, `:747`).
- **test-engineer**: new CLI `tests/lock_rebase.rs`, `tests/lock_rebase_hooks.rs`; `tests/lock_common/mod.rs` (`LockRepo::rebase`, `RebaseAttempt`); `tests/drift_approve.rs` (the loop at `:1877`), `tests/code_refs_walk.rs` (the array at `:1373`), `tests/rev_rule_hook.rs`; core `tests/lock_model.rs` (the plan); store `tests/verify_git.rs`; the new `fixtures/spec-lock/code-a.rebased.lock`, written only by its helper.

## Risks

- **The hook window**: a project with a `[verify]` hook refuses every commit (exit 2, cause 6) from the binary upgrade until its rebase commit.
- **The plain-`enforce` deadlock**: under `enforce`, or a hook running plain `spec verify`, any Fix row refuses the rebase commit while accept refuses format 1 (`spec-verify.md` "A project's hook"). The way out is `enforce-introduced`, but verify judges the stricter of the judged config's and `HEAD`'s mode, so it counts once at `HEAD`: switch before the binary upgrade; inside the window that config commit is refused too and takes the owner's `--no-verify` or the hook off. The canon says so at shipping.
- **Module or inline `mod`** (Q37): a file whose module and an inline `mod` are both marked for one node and relation drops the `mod`'s entry `ambiguous` (the file's module, always a candidate, cannot be told from it in the blob), the module's too when the `mod`'s in-file path ends its symbol (`a.rs` holding `mod a`); named, then `unlocked`; a canon Known limit.
- Blob-local location may drop entries: safe (`unlocked`), noisy. Alias resolution reads the working-tree corpus (`spec-lock-accept.md` "Known limits"): the ties and a format-1 entry's levels too.
- git before 2.44 ignores `GIT_NO_LAZY_FETCH` (`spec-check-git.md` "Open"): a partial clone may fetch a missing blob from its promisor, not drop it `blob_missing` (S13).

## Out of scope

Rebasing across `norm_recipe` or into an older format; regen across formats; an MCP tool, UI or plugin command; `--yes`; 5c's anchors (a rebase keeps keys: `anchor-states.md`); 8b's `[[tested]]` blocks under a rebase (`tested-evidence.md` "Rebase", built on this).

## Open

Answer; the other answer's cost.

- Q1 an entry rebase cannot re-derive: dropped, named, counted; refusing all strands the lock.
- Q2 locating: in its blob by name suffix, markers, v2 cross-check; a walk per distinct `commit`, decayed ones dropping anyway.
- Q4 a format-1 entry's levels: its blob markers' union, else `sig, body` (Q15 for `Invalid`); `sig, body` always: every narrower marker `levels_changed`.
- Q8 a project hook vs the rebase commit: `HEAD`'s old lock judged with the current one (widened by Q14); a hook stopping on exit 2 deadlocks.
- Q9 order (owner): right after 5b-1a (`lock-levels.md` Q22, Q23), its gate met 2026-10-20.
- Q14 Q8's reach: `--changed` and `--staged`; `--changed` only: a `--staged` hook, which the canon advises, refuses every rebase commit.
- Q15 invalid levels: dropped `bad_levels`, as accept's lockability; a guessed `sig, body` breaks ADR-0050's "never guessed".
- Q16 the pin: `item_levels(`, `module_levels(`, `hash_bound_items(`; `hash_item(` puts a tree-sitter node in the CLI.
- Q18 4b-2's deferred test: TE-7; else a reader change makes unrebased projects cannot-check unseen.
- Q19 canon home: a new Tier 2 `docs/canon/spec-lock-rebase.md`; a section in `spec-lock.md` leaves no room for 8b's format 3.
- Q32 read order, the norm text: "Data" step 4, the `Some` (2) text verbatim; a fifth text to pin.
- Q33 base detection: `read_head` parses with `None`, compares recipes itself; a typed `LockError` kind in core (its README ~100 B free).
- Q34 the base note: one `note:`, one JSON `notes` entry, as the mode note; a new key among verify's pinned top keys.
- Q35 the commit half: one subject-taking `write_and_commit`/`check_lock_commit`; a second ~300-line commit path drifting from accept's.
- Q36 the locator's home: `lock_rebase.rs` over `placed_name_file`, `QPath::suffixes`; a code API and ≤ 357 B of the code README.
- Q37 module candidates: the file's module always, inline `mod`s by markers, both marked `ambiguous`; "longest match" picks the wrong unit silently.
- Q38 TE-7: a scenario; its mutation is unkillable (a lock-only commit parses no lock; `None` reads format 1).
- Q39 `TRAILERS`: `drift_approve.rs`' loop gains `Some("rebase")`; RD-5's mutation was equivalent (a rebase commit has no `Proposal:`).
- Q40 `Foreign`; a parser panic: `blob_missing`; exit 2 naming file and OID; else a parser bug hides as `unlocked`.
- Q41 TE-5's corpus: verify-ecs as `Variant::OtherCode`; on code-a's Cannot rows TE-5 never passes.

Q1-Q8 (2026-10-12, in `lock-levels.md` before the split), Q9-Q21 (2026-10-16), Q32-Q41 (2026-10-20, the analyst's recommendations) decided under the owner's pre-approval, review open; numbers shared with `lock-levels.md` (Q3, Q7, Q10-Q13, Q21-Q24, Q27-Q31; Q9, Q16, Q18 in both) and `lock-levels-verify.md` (Q5, Q6, Q20, Q25, Q26; Q17 in its "Later"). S-n, R-n: the 2026-10-16 and 2026-10-20 refreshes' stale points, fixed in place; WA-n: the 2026-10-12 analysis' working answers.

## Dependencies and ordering

- **Needs 5b-1a** (`lock-levels.md`, shipped at d45dfce; canon `spec-lock.md`), not 5b-1b: format 2, `parse_lock(text, None)`, the refusal texts, both goldens, the pin; 5a-1's, 5a-1b's entry points.
- **Size** (2026-10-20, owner's pre-approval, review open): one slice, at most 3 iterations: store, core and the commit refactor (accept's suites green), the command, the base rule. **Fallback**: RD-1..RD-4 not green after iteration 2: the base rule splits off as 5b-2b `lock-rebase-base` (Q14, RD-6, TE-5, SW-2's `spec-verify.md` part), safe while no project holds a lock (Q23); no spec file for it until then.
- **Assumptions**: v2 unchanged since 2026-10-08 (`code-hash.md`), so the golden's 5 items pass the cross-check; code-a's Cannot rows hold verify at exit 2 in every mode; no `spec.lock` here or in either pilot project (`lock-levels.md` Q23, not re-checked).

## Implementation

Not started. To record: modules, deviations with reasons.
