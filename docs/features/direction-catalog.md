---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0106, v5 core slice 7 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 2, DC-1 to DC-6); the orchestrator's working answers, the owner's review open
adrs: []
---

# direction-catalog: a direction as a table of its documents, with version, state, open items, tasks and last change (RM-0106)

## Why

The owner's vision of 2026-11-11 asks for a game's catalog of mechanics, items, sound and maps, each with its version and its fixes (`08-roadmap.md` Phase 7). A direction was a flat list of names in the Spec tree: to learn a document's state, its open questions, its tasks or who changed it last, a person opened each one. The browser cannot build the table itself: at 100k nodes the whole trace takes about a second and its JSON runs to tens of megabytes.

`spec catalog --kind K… [--archive] [--json]` and the daemon's `GET …/catalog` serve one row per node of a kinds direction: title, version, work state, open items, tasks and last change (when and who). In the Spec tree, choosing a direction shows that table where the node pane stands, 50 rows then Show more, sortable by column, a title opening the document modal. Computed per read, never stored; it moves, refuses and gates nothing (ADR-0012). No ADR: it rests on ADR-0076, ADR-0073, ADR-0056, ADR-0005, ADR-0008 and ADR-0012.

## Description and interactions

Each decision is canon now, in `docs/canon/spec-catalog.md`: D1 the tree's rows, D3 the version, D4 state and tasks, D5 open items, D6 the last change per file: "The rule"; D2 four reads in one call: "The rule", "The walk"; D7 the cache: "The cache"; D8 no MCP tool: "Daemon, hub". D9 the table: `docs/canon/directions.md` "Tree".

The owner's questions, open, their working answers built: **Q1** "Version N" shown openly, hashes out; **Q2** kinds directions only (rooted modules: 1-2 more hours and a rule for depth); **Q3** the last change per file, node-history's commit reading on `HEAD`'s first-parent line, cached per `HEAD` in the daemon; **Q4** open items are open proposals naming the node as target or in `node_ids`, a document not counting its sections'. The spec-writer's: W1 per file (`of_file`); W2 a merge is the change on the first-parent line; W3 the table in the node column, the list keeping its keyboard path and Show as root; W4 `rev-parse` per call; W5 keys under `tree.catalog`.

## Data

In the canon: `spec-catalog.md` "The walk", "The cache", "Text and JSON", "Notes and exits", "Daemon, hub". **The copy**, the criteria's scenario (CLI `tests/catalog_scene/mod.rs`, `PINNED_TEXT`, `pinned_json`): a git copy of `fixtures/studio-lamp`, then `Ann Owner <ann@example.org>` applying PR-0001 to `docs/spec/scn-arrival.md` at 2026-01-02T10:00:00Z and `Bob Hand <bob@example.org>` editing `scn-storm.md` by hand at 2026-01-03T10:00:00Z; C-0001 open on `SCN-ARRIVAL`, PR-0040 an open question naming it:

```text
catalog scene | rows 2
SCN-ARRIVAL | version 1 | in work | open items 1 | tasks 1, open 1 | 2026-01-02T10:00:00Z | Ann Owner <ann@example.org> | PR-0001 | Arrival
SCN-STORM | version 1 | no card | open items 0 | tasks 0, open 0 | 2026-01-03T10:00:00Z | Bob Hand <bob@example.org> | hand | The storm
```

**Corrected at ship.** A row's `rev` is the revision number, none written 1, an unreadable one `null` (the pinned `"rev": 1`, D3, DC-7); the draft's "keeps `spec tree`'s … `rev` … as served there" was wrong, the tree serving `null` when none is written. The header is pinned as `catalog <kinds joined ", "> | rows <n>`, with a literal ` [archive]` appended under `--archive`.

## Rules and edge cases

In the canon: `spec-catalog.md` "The rule", "Known limits". **Rollout**: no schema step, backup format, config key or tool; a daemon or hub before this slice answers the route 404, the table showing it with Retry; a hub takes it at its next upgrade, the owner's call.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Test repositories are built at run time with invented names; "the copy" is "Data"'s.

- [x] DC-1 — **Rows are the tree's.** On the copy, `spec catalog --kind scene --json`'s row IDs equal `spec tree --kind scene --json`'s node IDs in order (`SCN-ARRIVAL`, `SCN-STORM`); with a section of kind `scene` added, it is listed where the tree lists it; `--archive` matches `tree --archive`. Mutation, red: rows taken from the whole tree.
- [x] DC-2 — **State and tasks are the trace's.** For each row, `state` equals `spec trace <ID> --json`'s first node's `rollup`, and `tasks` its card count; after `spec card accept`, `in work` becomes `confirmed`, `tasks_open` 0; a cancelled card is in neither. Mutations, each red: `state` always `no card`; cancelled cards counted.
- [x] DC-3 — **Open items.** On `SCN-ARRIVAL`: an open question naming it in `node_ids` and an open update targeting it count 2; one staged counts; an applied, a rejected, one naming `SCN-STORM`, one of another repository's worktree do not; an item naming both scenes counts once on each. Mutations, each red: applied items counted; the repository filter dropped.
- [x] DC-4 — **Last change.** On a linear temp repository, each document row's `last_change` equals `spec show <ID> --history --json`'s newest `source: commit` entry (`at`, `by`, `by_name`, `id`, `proposal`), a hand commit included; touching the file's mtime changes nothing; a section row carries its file's with `of_file: true`; an uncommitted new file reads `null`; on a merged branch the merge is the change. Mutations, each red: the file's mtime used; the oldest commit taken; `--first-parent` dropped (the merge case).
- [x] DC-5 — **No kind in code.** `workspace_names.rs` and http `no_domain.rs` stay green. Mutation, red: `"scene"` planted in `catalog.rs`; the guard names it.
- [x] DC-6 — **50, then Show more**, on the mock. 130 rows show 50; Show more shows 100 and moves focus to row 51's title link; again, 130 and the button gone. Mutation, red: every row rendered at once.
- [x] DC-7 — **Text and JSON as pinned.** On the copy, "Data"'s text byte for byte and the scene's `pinned_json` (OIDs matched by shape); no `--kind`: exit 2, nothing read; `--kind nothing`: `rows 0`, exit 0; git absent from `PATH`: every `last_change` `null`, the note, exit 0. Mutation, red: `by_name` printed for `by`.
- [x] DC-8 — **One walk, then the cache.** A seam counts git commands, trace readings and proposal reads: one CLI call over 40 rows in 12 files runs one presence read, `ls-tree -r --name-only <oid>`, one walking `log`, one `--no-walk` `log` and `rev-parse`, one trace reading, one proposal read. The daemon's second request at the same `HEAD` runs `rev-parse` alone; after a new commit, the walk again; a request with one more kind walks only for the new paths. Every answer byte-equals the CLI's. Mutations, each red: a `file_log` per path; the cache keyed without `HEAD`.
- [x] DC-9 — **The daemon serves it, the UI types it.** `GET …/catalog?kinds=scene` byte-equals `spec catalog --kind scene --json`; no `kinds` and an unknown parameter are 400s. The fixture holds the 3 sets; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutation, red: `archive` not passed through.
- [x] DC-10 — **The table**, on the mock. A direction chosen, no node open: the table, captioned, in the node column; a rooted module or an open node: none. Pressing Version sorts descending (`aria-sort="descending"`), again ascending; Title by the viewer's collation; ties in served order. A title opens the document modal, its anchor the opener. An unknown state is raw under `lang="en"`; no email or OID is in the DOM. The keys in `en.ts`, the Russian forms in `ru.ts`; `policy.test.ts` and `scale.test.ts` pass, the slice's CSS naming no legacy alias. After `pnpm build`, the table's chunk is not in first load, first-load JS grows at most 2 kB, ≤ 845 kB. Mutations, each red: a static import of `DirectionCatalog`; a state word compared outside `catalog.ts`; `by` drawn.
- [x] DC-11 — **[measure] At scale.** `specengine-eval scale --corpus-only` trees committed into git with a seeded history (`crates/specengine-eval/tests/catalog_scale.rs`, `#[ignore]`: the eval writes a `git fast-import` stream of 10 000 commits, each editing 1 to 5 seeded part files): `spec catalog --kind node --json`, warm, release, at 1k and 10k nodes, the git half timed by DC-8's seam, at 100k reported. Bars: the git half ≤ 1 s at 10k, the whole ≤ 2 s at 10k; times in "Implementation". Past a bar, "Known limits" names the next step (a per-`HEAD` index of last changes).
- [x] DC-12 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: the mock's direction table, a sort, Show more, a section row's "(file)", a title's modal, an empty direction.
- [x] DC-13 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Rooted modules (Q2); a new nav section (`SECTIONS` untouched); an MCP tool; per-section last change (W1); history beyond the newest commit (node-history's part); filtering rows by state, status or text; editing from the table; a stored or committed catalog; Flow drawn through a direction (RM-0102).

## Implementation

The UI lane built ahead in a worktree at `1d78b5e2`, merged onto `main`; the Rust lane on `main`, reviewed (ACCEPT) and given iteration 2. DC-1 to DC-10 ticked at ship, DC-11 to DC-13 on the orchestrator's word.

| Module | Does |
|---|---|
| core `catalog.rs` (new), `lib.rs` | `LAST_CHANGE_SCAN_MAX`, `CommitFacts`, `LastChange` (6 keys), `last_change`, `OpenItem`, `open_items_by_node` |
| store `worktree.rs`, `lib.rs` | `last_commits`, `last_commits_at`, `LastCommits` (`found`, `absent`, `unreached`, `walked`, `capped`, `shallow`): the tree read, the streaming walk (an early stop kills and reaps the child), the boundary read, the commit read by OID; the re-export |
| cli `catalog.rs` (new), `tree.rs`, `trace.rs`, `history.rs`, `subset.rs`, `lib.rs`, `main.rs` | `CatalogRequest`, `CatalogRow`, `CatalogOutcome`, `LastChangeCache`, `LAST_CHANGE_HEADS`, `catalog`, `catalog_with_view`, text, JSON; `lines_of` extracted, `direction_lines` (tree output unchanged); `NodeWork`, `node_work`; `git_cause` `pub(crate)`; seams `CatalogTrace`, `CatalogItems`, `CatalogWalk(&[String])` and their counters; `Outcome::Catalog`; the subcommand |
| http `app.rs`, `answer.rs` | the route, `App.catalogs`, the handler (`run`), `catalog` in the 404 list; doc |
| ui `api/{provisional,client,http,queries}.ts`, `tree/{DirectionCatalog.tsx,catalog.ts,TreeView.tsx,NodePane.tsx}`, `app/Shell.tsx`, `i18n/{en,ru}.ts`, `styles/app.css`, `mocks/{catalog,MockClient}.ts`, `test/{catalogStub,stubClient,builders}.ts` | `CatalogDocument`, `CatalogRow`, `CatalogChange`, `KnownRollup`; `getCatalog`, `useCatalog`, the re-read; the lazy table, `moduleTitle`; `tree.catalog.*`; `.catalog-*`; the 130-row mock |

**Rust departures**, as settled after iteration 2, canon now (`spec-catalog.md`): (1) `rev`, corrected under "Data"; (2) the store adds `last_commits_at(commit, paths, max)`, `last_commits` its call from `HEAD`, the CLI walking from the OID it read, so a commit during a read cannot mix two `HEAD`s or poison the cache; (3) the walk's flags beyond the draft's, `--root --no-relative --no-color --no-ext-diff --no-textconv --no-show-signature --end-of-options <commit> --`, format `%x01%H%x02%P` (the parents, for 4); (4) the shallow boundary: a parentless commit names every file it holds, a boundary's go to `unreached`, judged by the raw commit since iteration 2, not the shallow file; (5) the cache keeps three answers per path, not `Option<LastChange>`, so a cached answer's notes byte-equal the CLI's, and since iteration 2 no shallow one; (6) the notes scoped to the rows shown, "no queue yet" meaning schema 0; (7) git not read: every repository's open items counted, no note: the orchestrator's ruling, accepted, a Known limit; (8) the unreadable-row note; (9) paths no `cat-file` batch line could ask, `unreached` before iteration 2, are now absent or found like any other (`ls-tree -z`); (10) the handler walks (`run`, not `run_read`), the trace's state shown; the struct is `App`, field `catalogs`, not `AppState`; (11) an empty kind is a kind with no node, rows 0, as `tree`; (12) the index read with bytes, as `spec tree` reads it, so rows equal the tree's; DC-11 measures that cost.

**UI departures**, canon now (`directions.md` "Tree", "Known limits"): (1) the table replaces `NoNodePane`'s empty state, not the pane, its `h1` outside the lazy chunk for the Shell's focus; (2) the empty state at level 2, under that `h1`; (3) `tasks` and `more` plural (`_one|_other`, four Russian forms), `catalog.test` forbidding `{{count}}` outside one; (4) the "(file)" hint once under the table; (5) `aria-sort` on the sorted column alone; (6) missing values last both ways, titles by `Intl.Collator(language, {numeric: true})`; (7) the mock's 130 rows, harbor-sim's real mechanics and invented `MEC-CAT-*` ones; (8) the empty direction `?scenario=empty`, the pinned scenario only in stub tests; (9) "Not committed" also for not reached and git not read, an open wording question.

**Polish after the look**, in `directions.md` "The catalog": a row's tasks cell reads `0` when both counts are 0, else `{{count}} · {{open}} open` (the Russian puts the word before the open count); the state lozenge and the ID never wrap; titles hold at least 18ch and break only between words; the last change is small text, the name below it; on a narrow pane the table scrolls sideways inside its labelled region.

**DC-12**, the orchestrator's look: a pilot clone, 67 mechanics: the caption, 50 rows and "Show 17 more", the columns as specified; dates right on a clean build (an earlier wrong date came from a daemon built while a mutation was in place). Before the polish the layout crushed at 1440; after it the rows are clean (the UI developer's screenshots).

**Review.** Rust ACCEPT, asking iteration 2: presence from trees, not `cat-file` (a blobless partial clone read "uncommitted"); no cached shallow answer; the boundary by the raw commit; the store saying why the walk stopped (the cap or its natural end); no absolute path in the root-outside note. Verified: the walk's arguments (`--end-of-options`, `-z`, the cleared environment, `GIT_NO_LAZY_FETCH`, `GIT_OPTIONAL_LOCKS=0`), the early stop killing and waiting, merges, shallow depths, a root in a subdirectory through a symlink, one `HEAD` per request in the cache, the refactors verbatim, the counts (open only, target and `node_ids`, once per node, another clone out, a linked worktree in). Known limit: the FIFO of four heads shared by every project, about 3.5 MB a head at 10k files. No UI review is recorded.

**Iteration 2** built them: one `ls-tree` per walk; shallow answers not cached; the boundary by `rev-parse --is-shallow-repository`, then `cat-file commit` only when shallow; `LastCommits.capped`, `.shallow`; the root-outside cause in fixed words. Store 9, CLI 22 + 2, http 9 tests green; clippy clean.

**The notes**, exact: `catalog: last change not read: <cause>` (`git missing`, `not a repository`, git's first stderr line, `the project root is not in its git worktree`); `catalog: a shallow clone: <k> files its history does not reach read no last change`; `catalog: <k> files not reached in the last 20000 commits read no last change`; `catalog: no queue yet: no open item or task read`; `catalog: <id>: its row does not read (<column>: <reason>); not counted`. The refusals: `` spec catalog: give at least one `--kind` `` (exit 2); the daemon's 400s `` `kinds` is required (repeat it for more) `` and `` `<name>` is no query name of catalog: it takes kinds, archive ``.

**Git**, per call: `rev-parse --local-env-vars`, `--show-toplevel`, `--git-common-dir`, `--verify -q --end-of-options HEAD^{commit}`; `ls-tree -r -z --full-tree --name-only --end-of-options <oid>`; `log --first-parent --diff-merges=first-parent --no-renames --name-only -z --root --no-relative --no-color --no-ext-diff --no-textconv --no-show-signature --format=%x01%H%x02%P --end-of-options <oid> --`; `log --no-walk=unsorted --stdin -z --no-show-signature --no-color --encoding=UTF-8 --format=%H%n%an%n%ae%n%ct%n%B --`; when a walk reaches a parentless commit an answer depends on, `rev-parse --is-shallow-repository`, then in a shallow clone `cat-file --end-of-options commit <oid>`. The second daemon request at one `HEAD`: the four `rev-parse` alone.

**Pins moved**: http `tests/hub.rs` `GET_ROUTES` 29 to 30 (`/api/projects/{p}/catalog`, every login); `kind_amendment`'s set count 137 to 140; `daemonKeys.test.ts` `ALL` 140, `AHEAD_OF_FIXTURE` empty.

**Numbers.** First load 835.33 kB, +1.87 from 833.46 measured in the worktree, gzip 242.83; `DirectionCatalog-*.js` 5.28 kB (2.08 gzip), outside it; `ru-*.js` 158.82 kB. UI 197 files, 5 544 tests after the merge; the UI's 15 mutations red.

**DC-11**, release, median of 5 warm runs, the worst case reading all 10 001 commits (`catalog_scale.rs`: one check by default, three measures `#[ignore]`):

| Nodes | Load | Whole read | Git half | Peak |
|---|---|---|---|---|
| 1k | ~2.3 | 209 ms | 167 ms | 71 MB |
| 1k | 8–15 | 314 ms | 248 ms | 71 MB |
| 10k | ~2.3 | 410 ms | 312 ms | 146 MB |
| 10k | 8–15 | 588 ms | 375 ms | 146 MB |
| 100k | ~2.3 | 1 691 ms | 927 ms | 619 MB |
| 100k | 8–15 | 2 390 ms | 579 ms | 614 MB |

Both bars hold at 10k in both runs (git half ≤ 1 s, whole ≤ 2 s). The git half is the store's `last_commits` plus the library's uncached-minus-cached delta: DC-8's seam is `#[cfg(test)]`, out of the eval crate's reach.

**DC-13.** The workspace, 5 795 tests: 4 failures, churn (`layer_c_scan` gaining `catalog.rs`, green now) and 3 http `ui t3_*` hit while another lane rebuilt `ui/dist` (5 of 5 on rerun); clippy clean; the eval guards 58 of 58; `shards_repo`, `layer_c_scan`, `kind_amendment`, `genre` 35 of 35; `daemonKeys` 429 of 429; the UI after the polish 5 549 of 5 550, one load flake passing alone; first load 835.33 kB. The orchestrator's light gate runs before the commit.

**Test differences**, the test-engineer's: (1) DC-11's command lists nothing on the generated tree as written, which has no `[paths] spec`: the test adds `spec = "areas"`, about a quarter of the generated documents archived; (2) the git half timed as above; (3) presence is `ls-tree`, not the draft's `cat-file --batch-check`; (4) `rev` as corrected; (5) the header as pinned; (6) "no queue yet" pinned as no `proposals` or `cards` table, the read creating `<slug>.db`; (7) an empty kind `rows 0`; (8) departure 7 exercised only on a single-repository copy. **Uncovered**: the root-outside note (not reachable), the unreadable-row note, hub-mode requests (only `GET_ROUTES` at 30). **Mutations**: 14, named and extra, all red; a first batch restored files with old mtimes and left mutant builds, caught, every result from clean runs.
