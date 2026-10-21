---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, plugin, fixtures]
ref: Phase 3 slice 7b-2 of RM-0007 (split 2026-10-21; 7b-1 is `docs/features/impact.md`); the 2026-10-21 analysis, Q-G..Q-I decided under the owner's pre-approval, review open; refreshed before it starts; 05 s5.2, s5.3; 07 s1.2
adrs: [ADR-0046]
---

# impact-since (7b-2): the radius of the code changed since a revision

## Why

An agent handing in a branch, or the owner on one, asks what the code changed since a revision reaches: the nodes, bound symbols with their `sync`, the tests to run, the open tasks to warn. 7b-1 (`impact.md`) answers for a node or a symbol; this slice adds `--since REV`, and fills `spec verify --changed --json`'s `delta.symbols` (05:154, `null` since 4a) from the same comparer. `architecture-code.md#sync-in-reads` already states the rule: "`impact --since` compares a git revision with the working tree, untracked files included; `spec.lock` plays no part", exit 2 on code (ADR-0046). No new ADR.

## Description and interactions

- `spec impact --since REV [--depth N] [--json]`; MCP `get_impact {since}`; `spec verify --changed --json`'s `delta.symbols`. Everything else is 7b-1's: walk, rows, tasks, notes, cut, degrade.
- **Reused**: store `VerifyGit::open`, `list_commit(REV)` (any commit-ish, `verify_git.rs:240-258`), `read_files` (one `cat-file --batch`), `hash_files` (one `hash-object --no-filters --stdin-paths`), `finish`; CLI `verify_rows.rs` `read_head` (`:1486`), its code half generalised from `"HEAD"` to a REV (REV's documents, lock and config never read); `--changed`'s HEAD side (`verify.rs:452-474`).
- **New**: core, a pure comparer of item records; the CLI guard before `VerifyGit::open` (which already runs `rev-parse --is-inside-work-tree`).
- **Unchanged**: plain, `--staged`, `--node` verify (`delta.symbols` `null`), `check_binding`'s `outputSchema` (`AlwaysNull`, `mirror.rs:1723`: `--node` only, `code.rs:444-450`), the gate, `INDEX_FORMAT`, the queue.

## Data

### Command and guard

- Form: exactly one of REF, `--symbol`, `--since`, else exit 2 ``spec: impact takes exactly one of REF, --symbol and --since``; MCP ``spec: get_impact takes exactly one of `id`, `symbol` and `since` ``.
- Guard (RD-1): REV with a leading `-` or any control character (U+0000-U+001F, U+007F) -> exit 2, stdout empty, no git child, ``spec: --since `<REV>` is refused: a revision never starts with `-` or holds a control character`` (`<REV>` through `escape_controls`). The store's own refusal (`git.rs:626`: `-`, `\n`, `\0`) stays. The owner's git is 2.54, so `rev-parse --end-of-options` exists (A8); the CLI guard is the contract either way.
- Then `[code]` as `--symbol` (exit 2 without it); no worktree, no `git`, a git failure: exit 2. `list_commit(REV)` `None`: exit 1, reason ``--since `<REV>` is not a commit``, the JSON document.

### Compared

A1: REV's tree under today's `[code]` roots and excludes against the files on disk, untracked included; `.gitignore`, the git index, REV's `specengine.toml`, documents and `spec.lock` never read; the rows' `sync` reads today's lock.

- REV's `ls-tree -r -z` once; disk OIDs from one `hash-object --no-filters --stdin-paths`. An equal OID: unchanged, unparsed. A differing or one-sided path: REV's blob from the one `cat-file --batch`, both sides parsed (layer A), each item's `sig` and `body` digests hashed afresh on both sides (Q-G: the index's digest columns stay unread, `symbol-index.md` "Guarantees").
- Records `(file, qpath, item_kind, line, sig, body)`, keyed `(file, qpath, item_kind)`: `added` disk only, `removed` REV only, `changed` both with `sig` or `body` differing (Q-H: the default levels, `spec-verify.md`; `deps`, `skeleton`, `quote` not compared). A RON file is one unit, compared by blob OID (a RON comment edit counts). A comment, marker or formatting edit is no change (A3); a module move is removed plus added; a CRLF copy over LF blobs differs in OID, not in digest.
- Order: file bytes, line, qpath. Reading cost: `read_head` reads every code blob at REV (`verify_rows.rs:1518-1547`) unless the manifests are equal.

### Seeds

A2: `changed` and `removed` items reach the nodes REV's markers bind by containment (7b-1's rule), their IDs resolved against today's corpus; `changed` and `added` the nodes the disk's markers bind. Deduplicated, then 7b-1's walk, rows, tasks.

### Output

- Text: the header `since <7 hex>: <f> files, <c> changed, <a> added, <r> removed` (A4: a fixed 7-hex prefix of the full OID, never git's abbreviation) first, the summary next (7b-1); the delta's lines last, after the lists (Q-F): `<changed|added|removed> | <file>:<line> | <symbol> <item_kind>`.
- JSON (7b-1's keys typed, A5): `input.since` REV as given; `revision` the full OID; `delta` `{added, removed, changed, renamed}`, an item `{file, symbol, item_kind, line}` (`line` the disk's, REV's for `removed`), `renamed` `null`; `seeds.nodes` the printed distance-0 nodes; `truncated.delta` the delta items not shown.

### verify --changed

Q-I: `spec verify --changed --json`, with or without `--task`, sets `delta.symbols` to the comparer's `{added, removed, changed, renamed}` with REV = `HEAD`, over all code, bound or not; unborn `HEAD`: every item `added`. Text, verdict and exit unchanged; plain, `--staged`, `--node` `null`; JSON uncapped as verify's (`spec-verify-output.md`). `renamed` stays `null`, overriding `anchor-states.md:22` (Q14), with a follow-up item on core `anchor::pair`.

### MCP, INSTRUCTIONS, plugin 0.16.0

- `get_impact {id?, symbol?, since?, depth?}`, `since` `["string","null"]`; the mirror types `input.since`, `revision`, `delta`, `truncated.delta`; the description gains a `since` line (150 B of room after 7b-1's 1 898 B: measure, trim).
- INSTRUCTIONS: `get_impact(id|symbol|since, depth?) = impact` (+6 B): 2 023 B, 1 885 without the tasks line, 25 left.
- Plugin 0.16.0: `PINS`, `SCHEMA_PINS`; `implement`'s report step names `since` with the task's base, the tests it lists and the tasks to warn (Q-J; the body, 3 284 B after 7b-1, measured against 4 096).

## Rules and edge cases

- WHEN `since` comes from an agent, the guard SHALL refuse it before any git child.
- WHEN `impact --since HEAD` runs, its `delta` SHALL equal `verify --changed --json`'s `delta.symbols`: one comparer.
- Git: only `rev-parse`, `ls-tree`, `cat-file --batch`, `hash-object --no-filters --stdin-paths` (no `-w`); never `status` or `diff`; the index, objects and refs unchanged.
- Determinism: two clones, reverse creation order, CRLF copies: the same text and JSON; no root.
- Cost grows with the diff; the cut bounds the answer, MCP's `MAX_RESULT_CHARS` 500 000 the uncut verify JSON.

## Acceptance criteria

M: a mutation and the test it turns red; at most 8 counted, chosen at the refresh. Tests: CLI `tests/impact_since.rs`, `tests/verify_delta.rs`, core `tests/symbol_delta.rs`, store `tests/verify_git.rs`.

**rust-developer:**

- [ ] RD-1 -- a leading `-` or a control character: exit 2, stdout empty, no git child. M: passed through -> `option_like_since_is_refused_before_git`.
- [ ] RD-2 -- not a commit: exit 1, the reason and document. M: exit 2 -> `since_names_no_commit`.
- [ ] RD-3 -- REV against the disk. M: `HEAD` for REV -> `since_compares_the_given_revision`; untracked ignored -> `since_counts_untracked_code`; an equal-digest edit counted -> `since_ignores_unchanged_hashes`.
- [ ] RD-4 -- removed or changed items reach REV's markers' nodes, added or changed the disk's. M: REV's markers ignored -> `removed_symbol_reaches_its_old_binding`.
- [ ] RD-5 -- `delta.symbols` per Q-I, verdict and exit unchanged. M: filled in plain -> `plain_verify_has_no_symbol_delta`; under `--node` -> `mcp_code.rs`' `check_binding` walker; `renamed` set -> `renamed_stays_null`; the verdict moving -> `symbol_delta_changes_no_verdict`.
- [ ] RD-6 -- one comparer. M: two orders -> `since_head_delta_equals_verify_changed`.
- [ ] RD-7 -- the door. M: a `git status` -> `reads_leave_git_index_and_objects_untouched`.
- [ ] RD-8 -- digests afresh. M: read from the index -> `since_hashes_both_sides_afresh` (tampered columns, as `show_bindings.rs:1479` `fake_digest`).

**test-engineer:**

- [ ] TE-1 -- a scripted history over `fixtures/impact/` with the expected delta per REV (an older commit, `HEAD`, another branch): an item changed, added, removed; a comment-only and a formatting-only edit; an untracked and a deleted file; a RON value and a RON comment edit; a module move.
- [ ] TE-2 -- CRLF over LF blobs: zero `changed`; two clones, reverse creation order: identical output.
- [ ] TE-3 -- cost, recorded: `--since` over 1 000 changed files; `verify --changed` before and after the delta on `code-a` plus 10 000 files.
- [ ] TE-4 -- `spec-c` with `--since`: exit 2 naming `[code]`; outside git: exit 2.
- [ ] TE-5 -- MCP parity for `{since}`, `mcp_door` with it, `SCHEMA_PINS` 0.16.0, INSTRUCTIONS re-pins at 2 023 B.
- [ ] TE-6 -- a `git` shim on `PATH` logging argv: the door, and no child for `-x` or `a\x01b`.

**spec-writer:**

- [ ] SW-3 -- at shipping: `docs/canon/spec-impact.md` "Since", "Delta" (under 12 288 B); `spec-verify-output.md` "JSON" (11 472 B; about +64); `05-architecture.md:154`; 08:46; the store and core READMEs if they changed; `CLAUDE.md` State; RM-0007 `status: shipped`; "Implementation" <= 3 072 B.

## Out of scope

`renamed`; `delta.symbols` under `--staged`; documents changed since REV as seeds; a call-graph radius; other repositories' tasks; a cap on work.

## Risks

- `verify --changed` gains item digests of the differing files on both sides, and its uncapped JSON grows; agents run it routinely (`spec-verify.md`).
- `--symbol` and `--since` read code before the walk, then `set_bindings` refreshes again: two BLAKE3 passes.
- Pilot B holds no marker: there only nodes and the delta's cost can be measured.

## Refresh before starting

Re-measure after 7b-1 ships: line numbers above, the description's room, INSTRUCTIONS, the plugin version (0.16.0 if nothing else took a MINOR), the `implement` body, `spec-impact.md`'s room.

## Implementation

Not started.
