---
class: spec
status: shipped
scope: [crates/specengine-cli, crates/specengine-http, crates/specengine-store, ui, fixtures]
ref: RM-0033 slice 1 of 2 (the RM-0011 breakdown's slice 4; Q-E..Q-I decided 2026-10-21 under the owner's pre-approval, review open); 05 s5.3; 06 s4; 07 s2, s3; 2 iterations, review 2 accepted
shipped: 2026-10-21
adrs: [ADR-0053, ADR-0046, ADR-0012]
---

# lock-diff (RM-0033, 1 of 2): `spec lock diff` and the daemon's `lock-diff`

## Why

A drifted row says that the spec, the code or both moved since the owner accepted the binding, not what moved. Before approving a drift item or running `spec lock accept`, the owner ran git by hand with the entry's `commit` and `file_blob`. This read puts both old sides next to the working tree as git's hunks, for the terminal and for the Bindings tab's panel in slice 2 (`docs/features/lock-diff-ui.md`). The rule: ADR-0053; the contract as built: `docs/canon/spec-lock-diff.md`.

## Description and interactions

`spec lock diff REF [--json]`; library `lock_diff(&Env, &Globals, &LockDiffRequest {reference, git})` -> `Outcome::LockDiff`; `GET /api/projects/:p/lock-diff?ref=<REF>`, one call in the project's turn. No terminal check, `[y/N]`, MCP tool or plugin change. Steps:

1. Usage, discovery, config: exit 2, as every read.
2. REF resolved as `spec show` resolves it, its exits and texts kept; holders by (path, ord).
3. The code layer as `show --bindings` reads it: a run cause gives `rows` `null`, one `bindings:` note, exit 0.
4. Rows kept: a lock entry and core `drift::raises(state, detail, false)` (`code_ahead`, `spec_ahead`, `conflict`, `broken_binding`), in verify's order, under each holder whose set holds them.
5. Per kept row its sides ("Sides") and git's hunks; output: the canon's "Output", "Notes".

## Data

As built: the canon's "Output" (the example's exact hunks too) and "Notes". Five shapes: the document `{ref, reason, notes, nodes}`, a holder `{id, path, line, rows}`, a kept row `{row, spec, code}` (`row` verify's 17 keys), a spec side `{commit, path, diff}`, a code side `{blob, file, diff}`; a side `null` when it did not move; `diff` `""` equal, `null` not shown (one note); `rows` `null` not judged.

### Sides

Sides per state and the old and new texts: `docs/canon/spec-lock-diff.md` "Sides". The old spec text is read at the path made top-relative (the root's prefix below its worktree's top, `/`-joined). Both spec spans run through their last line's LF: no "No newline at end of file" line.

### Route

`GET /api/projects/:p/lock-diff?ref=<REF>`, `ref` required, one value: 200, 404, 503 the exits' documents or error body, byte-equal to `--json` less its final LF; a 400 runs nothing; `unknown_route`'s 404 lists `lock-diff` after `roadmap`. Texts: the canon's "Output".

### Daemon keys

`fixtures/daemon-keys.json` 76 to 81 sets, after `ShownBindings`: `LockDiffView`, `LockDiffNode`, `LockDiffRow`, `LockDiffSpecSide`, `LockDiffCodeSide`, the five shapes' keys in "Data"'s order; `row` `DriftRow`'s 17. Collected from `lock-diff?ref=LAW-BOUND` on `daemon_keys.rs`' `drift_repo` (its lock's `commit` no object there: `spec.diff` `null`).

### UI provisional types

ADR-0033's exception, in `api/provisional.ts`, each citing this "Data": the five sets as types (`row: DriftRow`), `reason`, `id`, a side, `diff` `| null`, `rows: LockDiffRow[] | null`. `getLockDiff(project, ref, signal?): Promise<LockDiffView>`, `ref` encoded once, a 404 resolved as data, a 400 or 503 rejected in the daemon's words. Mocks `mocks/lockDiff.ts`: harbor-sim's rows are Health's by key, four states, one `diff` null with N1; ledger-api `rows` null; a REF naming nothing the exit-1 document.

## Rules and edge cases

Writes only the data directory, gates nothing; a moved side not shown is `null`, never `""`; output is place-free: the canon's "Never".

### The git door

The canon's "Never": `rev-parse` (its probe outside `GIT_OPTIONAL_LOCKS`), `hash-object` without `-w`, `cat-file` under `GIT_NO_LAZY_FETCH=1` (a blobless clone fetched nothing), an isolated `diff --no-index`; the store's `diff_hunks` isolates it for review's `fill_diff` and the package's `snapshot_diff` too.

### As built, against the draft

Accepted in review: spec spans through their last LF (a 3-line spec hunk in the example); the row block is verify's `row_block` (candidates too); `LockDiffRequest` carries `git`; a holder's `id` is the canonical cited ID; show's texts for the `@rev` note and the several-holders warning; a shared row computed once, its notes once; a non-UTF-8 `.md` REF exits 1 (`spec show` 0, `verify --node` 2); hunk headers may carry git's function context; a failing blob batch read is the session's end, a run cause (N0, `rows` `null`), not N4; N7 gains `git diff --no-index could not run` and `git shows no hunk for bytes that differ` (a NUL pair, `diff` `null`), path-free; `api/types.ts` unchanged.

## Acceptance criteria

CLI `crates/specengine-cli/tests/lock_diff.rs` over a scratch git copy of `fixtures/code-a` after `spec lock accept LAW-BOUND`; mutations M1-M8 each red once, then reverted.

- [x] RD-1: the sentence and `CAP` edited, uncommitted: one `conflict` row, CAP's, both sides, `+` lines exactly the edits; the other kept rows `spec_ahead`, a spec side each. M1 (the spec text read at `HEAD`) red.
- [x] RD-2: `CAP` alone: `code_ahead`, `spec` null; the sentence alone: `spec_ahead`, `code` null; `ok` and `unlocked` rows unlisted; "Sides" per state. M2 (a spec side on `code_ahead`) red.
- [x] RD-3: `CAP` and `LABEL` edited: CAP's hunks hold no `LABEL` line; a `moved` row over a non-trivial function renamed in its file, then moved to `lib.rs` (`CAP` never pairs: body under 124 B): `code.file` the entry's, hunks only the changed lines; `module`, `ron`, an item not in its blob: whole files, N6. M3 (whole files for every item) red.
- [x] RD-4: a `commit` naming no object: `spec.diff` null, N1, exit 0; a `file_blob` naming none: `code.diff` null, N4; N2, N3, N5, N7 each; no `[code]`: `rows` null, N0, exit 0. M4 (exit 2 on a gone commit) red.
- [x] RD-5: `[verify] mode` `"enforce"` or `"nope"`: exit 0, the same bytes, no verdict; a REF naming nothing: exit 1, `nodes: []`; a look-alike: exit 2, stdout empty; no terminal: answered. M5 (exit 1 on a Fix row under `enforce`) red.
- [x] RD-6: a logging `git` shim records only the door's commands; `status`, `.git/index` bytes and mtime, `objects`, `refs` and the queue unchanged; no scratch file left; `--no-textconv` accepted, the probe exempt from `GIT_OPTIONAL_LOCKS`. M6 (`git diff <commit> -- <path>`) red.
- [x] RD-7: http `parity.rs`: byte-equal to `--json`, the 404, each 400; four route lists name `lock-diff`; `door.rs`, `no_domain.rs` green. M7 (the route ignoring `ref`) red.
- [x] RD-8: two roots in reverse creation order byte-identical; `fixtures/spec-c` `rows` null, N0, exit 0; `lock_diff_cost` (`#[ignore]`, release, warm, code-a plus 1 000 files, median of 5): 120 ms, `--json` 116, `verify --node` 53.
- [x] TE-1: `lock_diff.rs` holds RD-1 to RD-6, RD-8, fixtures made at test time; also a root below its top, an ECS crate's RON, NUL pairs never `""`, a `-diff` attribute or an enclosing repository's textconv unapplied.
- [x] TE-2: `daemon_keys.rs` and the fixture: the five sets, 81 in all; `kind_amendment.rs` 81. M8 (`diff` dropped from `LockDiffCodeSide`) red.
- [x] TE-3: M1-M8 8 of 8 red (iteration 2), then reverted; eval `doc_pointers`, `anonymity` green; neighbours unedited but for counts and route lists.
- [x] UI-1: the five types in `provisional.ts`; `daemonKeys.test.ts` counts 81 and matches each type's keys, in order, to its set.
- [x] UI-2: `getLockDiff` in the client, HTTP, mock and stub clients; `http.test.ts`: `ref` encoded once (`#` `%23`, `/` `%2F`), a 404 as data, a 400 and a 503 rejected.
- [x] UI-3: `mocks/lockDiff.ts` and its test: harbor-sim's rows equal Health's of the same key, sides per "Sides". `pnpm lint` 0, build clean, `pnpm test` 2 510/2 510 twice.
- [x] SW-1 (2026-10-21): ADR-0053; `architecture-code.md` "Sync in reads"; `spec-lock-diff.md`; 05, 06, 07, 08; RM-0033; the two specs.
- [x] SW-2 (2026-10-21): the canon as built; two READMEs; `proposal-{review,queue}.md`; 81 in six canon files; 07, 08; RM-0033; `CLAUDE.md`; this spec compacted.

## Out of scope

The panel (slice 2); an MCP tool, the plugin; `verify --diff`, `show --diff`; diffs against `HEAD` or another revision; a diff library; a RON value's own span; storing hunks; any write.

## Implementation

| Module | What it does |
|---|---|
| CLI `lock_diff.rs` (new) | holders, kept rows, both sides (cached per commit and path, node, blob, file), hunks, N0-N7, JSON, text |
| CLI `lock_rebase.rs` | `BlobUnits`, `NodeNames::new` shared: the entry's item located in a blob |
| CLI `bindings.rs` | `open_read` (the code layer, its causes), `with_judged_set` (judged set, open git session) |
| CLI `show.rs` | `SHOW_REV_IGNORED`, show's `@rev` note shared |
| CLI `lib.rs`, `main.rs` | `lock_diff`, `LockDiffRequest`, the five types, `Outcome::LockDiff`; `LockCommand::Diff`, no terminal check |
| http `app.rs` | the `lock-diff` route, `unknown_route`'s list (`args.rs`, `answer.rs` unchanged) |
| store `worktree.rs` | `diff_hunks`: `--no-textconv`, no attributes file, `GIT_ATTR_NOSYSTEM`, `GIT_CEILING_DIRECTORIES`, `GIT_DIFF_OPTS` removed, for every caller |
| UI `provisional.ts`; `client.ts`, `http.ts`, `MockClient`, `stubClient`; `mocks/lockDiff.ts`, `mocks/health.ts` | the five types; `getLockDiff`; the mock lock diff, `LOCKED_AT` exported (`api/types.ts` unchanged) |
| tests, fixture | CLI `lock_diff.rs` (new), `kind_amendment.rs` 81, `layer_c_scan.rs` `LATER`; http `parity.rs`, `daemon_keys.rs`, four route lists; `fixtures/daemon-keys.json`; five UI test files |

Gates: nextest 3 871/3 871 (64 skipped; 3 earlier `index-drift` failures, a concurrent export's, green on rerun); ui 2 510/2 510; fmt, clippy (probes too), export, check, parity, eval policy 86/86 clean.

Untested, no fixture produces them: holders sharing a row, an ambiguous row's candidate lines, N6's "not found in <file> now". `node_gone` is never kept (`verify --node`'s set never holds it): out of the canon's "Sides", in its known limits.
