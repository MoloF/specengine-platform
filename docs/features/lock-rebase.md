---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-code, crates/specengine-cli, fixtures]
ref: Phase 3 plan 2026-10-10, slice 5b-2, split from lock-levels 2026-10-16 (its Q21; WA-1, WA-7, WA-8; Q1, Q2, Q4, Q8 2026-10-12; Q9, Q14-Q16, Q18, Q19 2026-10-16; decided under the owner's pre-approval, review open); 05 s3.5; 07 s2; 08 s2
adrs: [ADR-0050]
---

# lock-rebase (5b-2): `spec lock rebase`

## Why

Nothing can carry a lock across a change of format or recipe (WA-1): accept, verify and the drift raise exit 2 on another `code_recipe` and, after 5b-1 (`lock-levels.md`), on format 1, naming this command; regen needs one format and recipe set; a hand edit is forbidden (ADR-0041). A grammar bump strands every lock (`spec-lock.md` "Known limits"), and ADR-0042's "until … re-accepted" cannot happen as built. 5b-1 is itself such a change.

Delivered: `spec lock rebase`, the first migration writer (ADR-0050; `architecture-code.md#spec-lock`), re-deriving each entry from its own `file_blob`; and the base rule that lets a project's verify hook pass its commit (Q14). Order (Q9, Q21): right after 5b-1, back to back, under its gate (5a-2's whole TE-1); before 5c, 6b, 7a, 7b, 8b (8b rebases its `[[tested]]` blocks on this: `tested-evidence.md` "Rebase").

## Description and interactions

- **The owner** meets an old lock as exit 2 naming `spec lock rebase` (`lock-levels.md` "Readers") and runs it on a terminal: a row per entry, one question, one verified commit. **An agent** never rebases (no MCP tool or plugin command); after the commit, verify and `report_drift` read the format-2 lock.
- **Crates** (WA-12). Store: a blob read by OID, new API on the one `cat-file --batch` session (today private: `git.rs` `CatFile`; `VerifyGit::read_files` reads by listing path; S13). Code: the blob-local locator ("Location") over `analyze`'s items and RON anchors; no new hashing (5a-1's `item_levels`, `module_levels`, 5a-1b's `ron::value_hashes`, v2's `hash_bound_items` for the cross-check). Core: the rebase's pure plan (the old `Lock` and each entry's located outcome in; the new `Lock`, rows and counts out), the drop reasons. CLI: a new `lock_rebase.rs`; `write_and_commit`, `check_lock_commit` take a subject (a node, or the rebase); `LockMode::Rebase`, outside `LockMode::TRAILERS` (S14); `verify_rows.rs` `read_head` and the `--changed` base (Q14). No new crate or dependency.
- **One door** (ADR-0050): rebase writes `spec.lock` alone, never via MCP, a role, a hook, CI or the daemon. Its commit completes no `drift` item: completion needs `Lock:` and `Lock-mode: accept` or `editorial` (`proposal-drift.md` "Approve"); items raised after it meet new evidence once (`proposal-drift.md` "Known limits").
- **Overridden at shipping**: `spec-verify.md` "Base" and cause (7) (Q14), "A project's hook"; `spec-lock.md` `:11`'s "Built:" list and "Known limits"' grammar line; `architecture-code.md` "As built" (`:47`, "The rebase … not built yet"); `07-interfaces.md` s2's command list.

## Data

### `spec lock rebase`

WA-7, WA-8, Q1, Q2, Q4, Q15. `spec lock rebase [--json]` (+ `--root`, `--config`):

1. Not a terminal: exit 2, nothing read. Usage, discovery, config (`--config` naming another file too), no `[code]` (`spec refs`' text): exit 2.
2. Git: a worktree, `HEAD` born, on a branch, no operation in progress (unmerged `spec.lock`: `spec lock regen` first): else exit 2.
3. **Inputs** `spec.lock` and `specengine.toml` (accept's gone-node set), tracked and equal to `HEAD`, else each named with its state, exit 1 `…: commit them first; nothing written`; no lock: exit 1 `no spec.lock: nothing to rebase`. No walk; dirty code ignored.
4. The lock read by `parse_lock(text, None)` (`lock-levels.md` "Readers"): unreadable, newer, another `norm_recipe` (no spec blob to re-derive `spec_hash`): exit 2. Already this build's format and recipes: exit 0 `nothing to change`, no question.
5. **Each entry**, in key order, from its own `file_blob` (WA-7): the store's read by OID in one `cat-file --batch` session (failing: exit 2), raw, no filters, no lazy fetch; a blob several entries name parsed once; never `HEAD`'s tree, the index, the working tree, nor `commit` (it decays). Located and hashed ("Location"). **Levels** (Q4, Q15): format 2 keeps its own; format 1 takes the union of the effective levels of the blob's markers giving its key (node by token or `spec show`'s resolution, its relation, binding the located unit); one of them `Invalid`: dropped `bad_levels`, never the default (`code-identity.md` "Marker grammar"); none gives the key: `sig, body`, what format 1's whole-item hash compared.
6. Kept byte for byte: key, `rev`, `spec_hash`, `file_blob`, `commit`, `verified_*`; written: `levels`, digests, fingerprints, `body_bytes`, the header. **Dropped** (Q1): named, no hash written, so verify shows `unlocked`.
7. Committer identity (else exit 2); rows on stdout; the question on stderr, only `y`, `yes`, no `--yes`; `HEAD`, branch, lock bytes re-read (changed: exit 1 `spec.lock changed while being rebased; nothing written`); the commit.

**Location** (Q2), inside the blob:

| `kind` | Candidates | Hashed by |
|---|---|---|
| `item` | items of `item_kind` (`code-bindings.md` "Rust binding") whose in-file name (inline `mod`s, owner, name, rendered as the walk renders a `qpath`) is `symbol` or ends it at a `::` boundary | `item_levels` |
| `module` | the file's module; each inline `mod` whose in-file path ends `symbol` at a `::` boundary | `module_levels`; the `mod_item`'s `item_levels` |
| `ron` | the anchor of `symbol`'s field path (`code-identity.md` "RON binding") | `ron::value_hashes` (`hash-levels-ron.md`) |

- **Ties**: of several candidates, those a marker of the blob binds whose token, or `spec show`'s resolution of it, is the entry's `node`, with its `relation`; one left is located, else `ambiguous`.
- **Cross-check** (Q16): a format-1 `item` entry under this build's own v2 recipe (`RECIPE tree-sitter-rust/<v> abi/<n>`) must give its `ast_hash` again through `hash_bound_items` over the located item's index, else `hash_mismatch`; never `hash_item` (the CLI holds no tree-sitter node: `hash-levels.md` Q6). Under another recipe nothing can be cross-checked: the location stands, with one `note: rebase: "<old code_recipe>" is not this build's: located without a hash cross-check`.

| Drop reason, the first that applies (SW refinement) | When |
|---|---|
| `blob_missing` | no object of that OID (gc, a partial or shallow clone) |
| `not_utf8` | the blob is not UTF-8 |
| `symbol_gone` | no candidate |
| `ambiguous` | the markers leave several candidates or none; an `Ambiguous` RON anchor |
| `bad_levels` | a format-1 entry whose key a marker with `Invalid` levels gives (Q15) |
| `cannot_verify <category>` | the located unit gives `CannotVerify` (a RON file: any rejection) |
| `hash_mismatch` | the cross-check fails |

**Output**: stdout rows in key order, `rebased|dropped | <node> | <relation> | <file> | <kind> <symbol>[ <item_kind>][ | <reason>]`, ending `committed <oid>`; stderr `rebase spec.lock (format 1 -> 2, "<old code_recipe>" -> "<new>": 10 rebased, 1 dropped) at <commit> on <branch> in <worktree>? [y/N]`, both counts always.

```
rebased | GOAL-01 | implements | crates/tally/src/counter.rs | item crates/tally::counter::Tally::value function_item
dropped | R-20 | configures | assets/data/movement.ron | ron root.stamina.regen_per_second | blob_missing
committed 3f9a1c0<33 hex>
```

`--json` (exits 0, 1, and 2 after discovery; every key present, absent null; `refusal` as accept's): `{"from":{"format":1,"code_recipe":"…"},"to":{"format":2,"code_recipe":"…"},"counts":{"rebased":10,"dropped":1},"rows":[{"change":"dropped","node":"R-20","relation":"configures","file":"assets/data/movement.ron","kind":"ron","symbol":"root.stamina.regen_per_second","item_kind":null,"levels":null,"reason":"blob_missing"}],"commit":"<oid>","unchanged":false,"refusal":null}`.

**The commit**: atomic write (`replace_file`), `git commit --only spec.lock`, hooks run, never `--no-verify`: `spec: lock rebase`, an empty line, `Lock-mode: rebase`, `Decided-by: <committer_ident>`; no `Lock:` trailer, no caller trailers (no queue item runs a rebase: ADR-0045 runs accept). `check_lock_commit` takes the subject: accept's four checks, the third "a `Lock-mode: rebase` trailer and no `Lock:`" for "a `Lock:` trailer naming the node"; exit 0 only for a verified commit; accept's failure table and restore rules (`spec-lock-accept.md` "The lock commit"). `LockConsent`, `LockPrompt` reused; no `&[LockTrailer]`. Exits: as the steps say; a commit failed, failing a check or unverified: 1; exit 0 only for a verified commit or `nothing to change`. After a refusal `HEAD`, refs, index, status and files are as before.

### The base

Q8, Q14 (S4, S5); overrides 4a's cause (7) and 4b-2's `read_head` (`spec-verify.md` "Base", "The index"). Under `--changed` and `--staged` alike: a `HEAD` lock of format 1 or another `code_recipe`, beside a judged lock (the disk's, the index's) of this build's format and recipes: `HEAD`'s rows are judged with the judged lock, `note: HEAD's spec.lock is format <n> ("<code_recipe>"): judged with the current lock`. A rebase changes no key, `rev`, `spec_hash` or `file_blob`, so each row reads as at `HEAD`: Pre-existing; a hook running `spec verify --staged` (`spec-verify.md` "A project's hook") or `--changed` under `enforce-introduced` passes the rebase commit. `HEAD`'s lock unreadable or of another `norm_recipe`: cause (7); a judged lock not this build's: cause (6), as today.

### Fixtures

`fixtures/spec-lock/code-a.format-1.lock` (5b-1 moved it) is the input; rebased at its commit it gives `fixtures/spec-lock/code-a.lock` byte for byte: 9 `rebased`, 2 `dropped bad_levels` (`counter.rs:77` `[path, path]`, `bounds.rs:14` `[Sig]`: Q15, S9). Both goldens share `commit` and `verified_at`; their digests are generated, never hand-written.

## Rules and edge cases

- WHEN a rebase runs, the system SHALL read code only from each entry's `file_blob` by OID and SHALL drop, never guess, an entry it cannot re-derive (Q1, Q15).
- WHEN run from two roots, another cwd, under `core.autocrlf=true` or over reversed entries, rebase SHALL give byte-identical locks and output.
- WHEN a rebase commit lands, no `drift` item SHALL be completed by it.
- WHEN `HEAD`'s lock is format 1 or another `code_recipe` and the judged one is this build's, verify SHALL judge `HEAD`'s rows with the judged lock in both base modes (Q14).
- **The pin** (Q16): `lock_rebase.rs` joins `nothing_outside_the_binder_reads_an_items_state` (CLI `code_refs_walk.rs`), calling `item_levels(` and `hash_bound_items(`; `hash_item(`, `hash_node(` banned, no `.state` read.
- No stack, kind or prefix in rows or texts (WA-12).

## Acceptance criteria

rust-developer (M = the mutation that must turn the named test red):

- [ ] RD-1 -- the sources. M: working-tree bytes -> `rebase_ignores_uncommitted_code`; `HEAD`'s blob -> `rebase_uses_each_entrys_own_blob`; today's date -> `rebase_keeps_verified_at_and_commit`.
- [ ] RD-2 -- rebasing `code-a.format-1.lock` gives `code-a.lock` byte for byte. M: any fabricated field -> `rebase_of_the_format_1_golden_is_accepts_golden`; default levels for an `Invalid` marker -> `rebase_drops_invalid_level_entries` (and the golden test).
- [ ] RD-3 -- drops. M: a refusal on a missing blob -> `missing_blob_drops_its_entry`; a v2 mismatch kept -> `v2_mismatch_drops_the_entry`.
- [ ] RD-4 -- refusals. M: reading before the terminal check -> `rebase_off_a_terminal_reads_nothing`; another `norm_recipe` rebased -> `other_norm_recipe_exits_2`; a merge in progress rebased -> `rebase_refuses_an_operation_in_progress`.
- [ ] RD-5 -- the commit. M: check 3 dropped -> `rebase_commit_without_its_mode_trailer_fails_a_check`; a `Lock:` trailer -> `rebase_commit_names_no_node`; `rebase` in `LockMode::TRAILERS` -> `rebase_never_completes_a_drift_item`.
- [ ] RD-6 -- "The base". M: `HEAD`'s format-1 lock as cause (7) -> `head_lock_of_another_format_is_judged_with_the_current_lock`, under `--changed` and `--staged`.
- [ ] RD-7 -- genre, graph, the pin. M: tree-sitter in store -> `build_graph.rs` `core_and_store_have_no_tree_sitter`; a new dependency -> `cli_normal_graph_is_model_core_store_without_measurement_mcp_or_rust_analyzer`; the cross-check through `hash_item(` -> `nothing_outside_the_binder_reads_an_items_state`.

test-engineer:

- [ ] TE-1 -- temp repos from `fixtures/code-a/` and `code-a.format-1.lock`: a scenario per rebase row and drop reason (`bad_levels` included), text and JSON pinned, each naming its mutation.
- [ ] TE-2 -- migration: the format-1 golden committed -> verify exit 2 with the step -> rebase, `y` -> one commit passing the four checks -> verify's rows as over accept's golden -> `spec lock accept <each node>` `nothing to change`.
- [ ] TE-3 -- a simulated grammar bump (a committed format-2 lock naming another tree-sitter-rust): verify exit 2 with the step; rebase, under the note, gives accept's golden.
- [ ] TE-4 -- an OID no object has: `dropped blob_missing`, then verify `unlocked`.
- [ ] TE-5 -- a project pre-commit hook running `spec verify --staged`, and one running `--changed`, under `enforce-introduced` passes the rebase commit, exit 0 (Q14). M: `HEAD`'s lock as cause (7).
- [ ] TE-6 -- after every refusal and an `N`: `git status --porcelain=v1 -z --untracked-files=all`, `HEAD`, refs, index, `spec.lock` bytes unchanged.
- [ ] TE-7 -- 4b-2's carry-over (`rev-rule.md` TE-3, Q18): the rebase commit through a real `spec check --staged --root .` hook under `[check] mode = "enforce"` (CLI `rev_rule_hook.rs`): clean, the commit made (a lock-only commit changes no `norm_hash`: no lock parsed). M: the lock parsed with no candidate.
- [ ] TE-8 -- byte-identical lock, stdout, stderr, JSON: two roots, another cwd, `core.autocrlf=true`, the input's entries reversed (`lock-levels.md` TE-7's rebase half).

spec-writer:

- [x] SW-1 -- this spec, split from `lock-levels.md` 2026-10-16 (its Q21).
- [ ] SW-2 -- at shipping (Q19), bytes measured 2026-10-15: a Tier 2 `docs/canon/spec-lock-rebase.md` (as `spec-lock-accept.md`; about 3-4 KB of 12 288): the steps, "Location", the drops, the output, the commit; its index line lands in `docs/index-canon-spec.md` (`docs/canon/spec-*.md`; 2 592 of 10 240); `spec-lock.md` (7 300 before 5b-1's edits: a pointer, the "Built:" list, "Known limits"); `spec-verify.md` "Base", cause (7), "A project's hook" (11 587 before 5b-1's edits; over 12 288, "Base" moves to a Tier 2 `spec-verify-base.md`); `spec-lock-accept.md` "The lock commit" (11 237: the subject, one clause); `architecture-code.md` (10 711: "As built"); `07-interfaces.md` s2 (`spec lock rebase` beside `:107-108`); 05 s3.5; the store (10 174: one pointer, net ≤ 0), code (10 199 before 5a-1's move) and CLI (8 612) READMEs; `CLAUDE.md` State; 08 s2.
- [x] SW-3 -- no new ADR (ADR-0050 decides this writer); `export index`, `check` clean.

## Risks

- **The hook window**: a project with a `[verify]` hook refuses every commit (exit 2) from the binary upgrade until its rebase commit. Under plain `enforce` with any Fix row the hook refuses the rebase commit too, while accept refuses format 1: a deadlock (`spec-verify.md` "A project's hook"); the way out is `enforce-introduced` (Q14).
- Blob-local location may drop entries: safe (`unlocked`), noisy.
- Alias resolution reads the working-tree corpus (`spec-lock-accept.md` "Known limits"): the ties and a format-1 entry's levels too.
- git before 2.44 ignores `GIT_NO_LAZY_FETCH` (store `git.rs`; `spec-check-git.md` "Open"): a partial clone may fetch a missing blob from its promisor rather than drop it `blob_missing` (S13).

## Out of scope

Rebasing across `norm_recipe` or into an older format; regen across formats; an MCP tool, UI or plugin command; `--yes`; 5c's anchors (a rebase keeps keys: `anchor-states.md`); 8b's `[[tested]]` blocks under a rebase (`tested-evidence.md` "Rebase", built on this).

## Open

| Point | Answer | The other answer's cost |
|---|---|---|
| Q1 an entry rebase cannot re-derive | dropped, named, counted | refusing all strands the lock (no other writer) |
| Q2 locating the item | in its blob: name suffix, markers, v2 cross-check | a walk per distinct `commit`; decayed ones drop anyway |
| Q4 levels of a format-1 entry | its blob markers' union, else `sig, body`; an `Invalid` one: Q15 | `sig, body`: every narrower marker `levels_changed` |
| Q8 a project hook vs the rebase commit | `HEAD`'s old lock judged with the current one (overrides 4a's cause 7); widened by Q14 | a hook stopping on exit 2 refuses it: deadlock |
| Q9 order (owner) | right after 5b-1, under its gate (5a-2's whole TE-1) | `lock-levels.md` Q9 |
| Q14 Q8's reach | `--changed` and `--staged` (`read_head`); TE-5 runs both | `--changed` only: a `--staged` hook, which the canon advises, refuses every rebase commit |
| Q15 invalid levels in a rebase | the entry dropped `bad_levels`, as accept's lockability; RD-2 holds at 9 entries | guessed `sig, body`, against ADR-0050's "never guessed" and `code-identity.md` "Marker grammar"; an 11-entry expected file |
| Q16 the pin, this file | `lock_rebase.rs` calls `item_levels(` and `hash_bound_items(` | the cross-check through `hash_item(`: a tree-sitter node in the CLI, the ban lifted |
| Q18 4b-2's deferred test | the rebase commit through a real `spec check --staged` hook under `enforce`: TE-7 | a reader change makes unrebased projects cannot-check, no test catching it |
| Q19 canon home | a new Tier 2 `docs/canon/spec-lock-rebase.md`, one line in `docs/index-canon-spec.md` | a section in `spec-lock.md`: about 11.8 KB of 12 288, no room for 8b's format 3 |

Q1-Q8 (2026-10-12, in `lock-levels.md` before the split) and Q9-Q21 (2026-10-16) decided under the owner's pre-approval, review open; numbers shared with `lock-levels.md`, where Q3, Q5-Q7, Q10-Q13, Q17, Q20, Q21 live (Q9, Q16, Q18 in both). S-n: the 2026-10-16 refresh's stale points; WA-n: the 2026-10-12 analysis' working answers.

## Dependencies and ordering

- **Needs 5b-1** (`lock-levels.md`): format 2, `parse_lock(text, None)` reading format 1, the refusal texts, accept's format-2 golden and `code-a.format-1.lock`, the pin per file. From 5a-1 and 5a-1b as 5b-1 ("Dependencies and ordering"), v2's `hash_bound_items` kept.
- **Assumptions**: 5b-1's; no real format-1 lock exists, so the gap between 5b-1 and this slice strands nothing.

## Implementation

Not started. From the split (2026-10-16, `lock-levels.md` Q21): RD-1..RD-5 <- its RD-5..RD-9 (RD-2 gains `rebase_drops_invalid_level_entries`, RD-5 `rebase_never_completes_a_drift_item`); RD-6 <- its RD-2's base mutation; RD-7 new (the store half of its RD-10, the pin); TE-1..TE-6 <- its TE-1, TE-4, TE-5, TE-6, TE-8, TE-9 (TE-5 in both modes); TE-7 <- `rev-rule.md` TE-3's rebase commit; TE-8 <- its TE-7's rebase half. To record: modules, deviations with reasons.
