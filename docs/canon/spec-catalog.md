---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-13
---

# spec catalog: a direction as a table of its documents

Built by `direction-catalog` (RM-0106), no ADR: it rests on ADR-0076 (a direction is a module the kind reads apply), ADR-0073 (a lens hiding no work), ADR-0056 (cards), ADR-0005 (the decision history readable from git), ADR-0008 and ADR-0012. Related: `directions.md`, `spec-trace.md`, `spec-history.md`. Code: core `catalog.rs`; store `worktree.rs` (`last_commits`, `last_commits_at`, `LastCommits`); CLI `catalog.rs`, `tree.rs` (`direction_lines`), `trace.rs` (`node_work`), `subset.rs` (seams `CatalogTrace`, `CatalogItems`, `CatalogWalk`); http `app.rs`. The screen: `directions.md` "Tree".

## The rule

`spec catalog --kind K… [--archive] [--json]` and the daemon's `GET /api/projects/:p/catalog` serve one row per node of a kinds direction, computed per read, never stored; it moves, refuses and gates nothing. Writes: the index's own refresh alone; the cache lives in the daemon's memory. One corpus, queue and git state gives one text and one JSON at any root and `HOME`; every date is stored data, no clock read.

- **Rows** are exactly `spec tree --kind K… [--archive]`'s nodes with no root, in its order, sections included (`direction_lines`, the index read with bytes as `tree` reads it). No REF and no depth: a rooted module keeps the tree. An empty kind is a kind with no node: `rows 0`.
- **`rev`** is the node's revision number (`node-revision.md` "Revision"): none written is 1, an unreadable one `null`. `spec tree` serves a written `rev` alone, `null` otherwise. Shown as "Version N"; hashes stay out.
- **`state`** is the trace's `rollup` of the node (`spec-trace.md` "Marks and rollup"), its own cards only, the trace's reading built once for all rows (`node_work`). **`tasks`** are the node's cards, cancelled ones ignored, `tasks_open` the open ones.
- **`open_items`**: the `open` proposals of this repository (`same_repository` on `git_common_dir`; staged ones included) whose `target_id` is the row's ID or whose `node_ids` hold it, once per node (core `open_items_by_node`). A document row does not count its sections' items. A linked worktree's items count, another clone's do not.
- **`used_by`**, **`mentioned_by`**, **`orphan`**: core `usage_of` over the graph the rows' tree read built, one more pass over its edges, never stored (`spec-usage.md`); `orphan` `null` on a section.
- **`last_change`**, per file: the newest commit of `HEAD`'s first-parent line whose diff against its first parent names the row's path, read as node-history's commit entry (core `last_change`: `at`, `by`, `by_name`, `commit`, `proposal`; `spec-history.md` "Sources"); a hand commit's `proposal` is `null`. A section row carries its file's, `of_file: true`. The working tree never counts: an edited file reads `HEAD`'s, a new uncommitted one `null`. A merge on the line is the change.

## The walk

The CLI reads git once per call: `rev-parse` four times (`--local-env-vars`, `--show-toplevel`, `--git-common-dir`, `--verify -q --end-of-options HEAD^{commit}`), then, for the files no cache holds at that `HEAD`, the store's `WorktreeGit::last_commits_at(<that OID>, paths, LAST_CHANGE_SCAN_MAX)`. Walking from the OID read once, a commit made meanwhile cannot mix two `HEAD`s. `last_commits(paths, max)` does the same from `HEAD`; an unborn `HEAD` holds no path (all `absent`, nothing run). Paths are relative to the worktree's top, so a root in a subdirectory of the repository works; a root git places outside its worktree reads no last change.

1. **Presence from trees**: one `ls-tree -r -z --full-tree --name-only --end-of-options <oid>`; a path it does not list is `absent`, never walked for.
2. **The walk**: one `log --first-parent --diff-merges=first-parent --no-renames --name-only -z --root --no-relative --no-color --no-ext-diff --no-textconv --no-show-signature --format=%x01%H%x02%P --end-of-options <oid> --`, read as a stream. Once every present path is named, or a commit past `max` (20 000) comes, the child is killed and reaped; the paths not named are `unreached`. The cap, not the handle's deadline, bounds it.
3. **The commits**: one `log --no-walk=unsorted --stdin -z --no-show-signature --no-color --encoding=UTF-8 --format=%H%n%an%n%ae%n%ct%n%B --` over the distinct OIDs found.

No present path: no log runs; none found: the second does not. Every command is the store's read-only child (`GIT_NO_LAZY_FETCH=1`, `GIT_OPTIONAL_LOCKS=0`, `hub-git.md`) and reads commits and trees, never a blob, so a blobless partial clone answers as a full one. `LastCommits {found, absent, unreached, walked, capped, shallow}`: each path in exactly one set; `capped` when the walk stopped at `max` with commits left (a history of exactly `max` that ends is not); `shallow` when it ended at a shallow clone's boundary an answer depended on.

**The shallow boundary.** A commit the walk reads with no parent names every file it holds. A true root's are found there; a shallow clone's boundary's are `unreached`, never a false change. Which it is, is read only when an answer depends on it (a path named only there, or paths left when the walk ends there): `rev-parse --is-shallow-repository`, then, in a shallow clone, `cat-file --end-of-options commit <oid>`, whose raw object names a `parent` on a boundary and none on a true root.

## The cache

`LastChangeCache` (`Default`, `Sync`), the daemon's alone (`App.catalogs`); the CLI passes none. Keyed by (git common dir, `HEAD` OID), it holds per path one of three answers: the change, absent, or not reached within the cap. A path a shallow clone's history did not reach is never kept: a deepening fetch changes it under the same `HEAD`. A read takes the cached paths, walks for the rest alone and keeps them: the second read at one `HEAD` runs the four `rev-parse` alone of git, a new commit walks again, one more kind walks for its new files only. At most `LAST_CHANGE_HEADS` (4) heads, first in first out, one cache for every project the daemon serves; worktrees of one repository share a head's paths. A cached answer's text and notes byte-equal the CLI's.

## Text and JSON

Header `catalog <kinds joined ", "> | rows <n>`, ` [archive]` appended with `--archive`. A row: `<id, else path> | version <rev or -> | <state> | open items <n> | tasks <t>, open <o> | used by <u>, mentioned by <m>[, orphan] | <at>[ (file)] | <by> | <proposal or hand> | <title or ->`, `orphan` only on a judged orphan; no last change prints `- | - | -` for its three. Values pass `one_line`, lines escaped, the JSON raw. The cut at `OUTPUT_CAP_CHARS`: the header, rows whole while they fit (the first always), then `[truncated: <k> of <n> rows not shown]`.

JSON `{kinds, archive, notes, rows, truncated, omitted}`; a row's 16 keys `{id, kind, title, path, line, rev, status, archived, state, open_items, tasks, tasks_open, used_by, mentioned_by, orphan, last_change}`; `last_change` `{at, by, by_name, commit, proposal, of_file}`, or `null`: absent at `HEAD`, not reached, or git not read. `by` is raw, as node-history's CLI and MCP show it. `omitted` counts the rows cut, `null` uncut. The scenario's pinned text and JSON: CLI `tests/catalog_scene/mod.rs`.

## Notes and exits

Exit 0 answered, a note or no row too; 2 on no `--kind` (`` spec catalog: give at least one `--kind` ``, nothing read), no project, an index or a queue that cannot be read (a newer build's). Notes go to stderr as `note:` lines and raw into JSON `notes`, sorted, each once, then the tree's `warning:` lines on stderr alone; none changes the exit:

- `catalog: last change not read: <cause>`, every `last_change` `null`; the cause `git missing`, `not a repository`, git's first stderr line, or `the project root is not in its git worktree` (fixed words, no absolute path);
- `catalog: a shallow clone: <k> files its history does not reach read no last change`;
- `catalog: <k> files not reached in the last 20000 commits read no last change`;
- `catalog: no queue yet: no open item or task read`: the queue at schema 0, no `proposals` or `cards` table (`<slug>.db` exists after any refresh);
- `catalog: <id>: its row does not read (<column>: <reason>); not counted`: an open proposal row that does not decode.

These five are scoped to the rows shown: printed only when a row is shown, the files counted among them, so `--kind nothing` prints none. The tree's and the trace's own notes print as they do there.

## Daemon, hub

`GET /api/projects/:p/catalog?kinds=K[&kinds=…][&archive=true]`: `spec catalog --json` less its final LF, the browser view (never cut), in the project's turn, with the daemon's cache. It walks (`run`, not `run_read`: `spec-read-freshness.md`), its rows showing the trace's state. 400s, nothing run: `` `kinds` is required (repeat it for more) ``; an unknown name `` `<name>` is no query name of catalog: it takes kinds, archive ``. The 404 list names `catalog` after `delivery`. Key sets 138-140 of `fixtures/daemon-keys.json`: `Catalog` (6), `CatalogRow` (16 since `usage-orphans`), `CatalogChange` (6). A hub serves the route to every login over its checkout and its own queue (its `--push` clone neither shallow nor partial, `hub-git.md`). No MCP tool; `HUB_TOOLS`, the plugin version, config keys, the queue schema and the backup format unchanged. A daemon before this slice answers 404; the table shows that with Retry.

## Known limits

- **Per file, not per node**: a section row shows its file's last change (`of_file`); History (`spec-history.md`) has the node's own.
- **Merges**: the first-parent line makes a merge the change on the reader's line; node-history, following `--follow`'s simplification, may list the side commit.
- **The scan cap**: a file untouched in the last 20 000 first-parent commits reads none, with its note.
- **A shallow clone**: the files its history does not reach read none, with its note, until a deeper fetch.
- **Git not read** (git missing, no repository, a failure before the common dir is read): the open items of every repository sharing `<slug>.db` are counted, so nothing is hidden, and no note says so (the orchestrator's ruling, 2026-11-12).
- **The cache's memory**: one FIFO of four heads for every project, about 3.5 MB a head at 10k files; a fifth head drops the oldest, however busy.
- **Emails**: `by` carries them in the CLI and the JSON; the UI draws `by_name` (`spec-history.md` "Known limits").
- **Cost** (DC-11, release, warm, median of 5, all 10 001 commits read): at 10k nodes the whole read 410-588 ms, the git half 312-375 ms, peak 146 MB; at 1k 209-314 ms; at 100k 1.7-2.4 s, the git half 0.6-0.9 s, peak about 619 MB, the lower figures at load ~2.3, the higher at 8-15. The bars (git half ≤ 1 s, whole ≤ 2 s at 10k) hold, the uses too (UO12, `spec-usage.md` "Known limits": 1.37 s at 10k, the pass 5 ms); past them, the next step is a per-`HEAD` index of last changes.

## Tests

core `catalog.rs`; store `last_commits.rs`; CLI `catalog.rs` (its scene `catalog_scene/`), `src/catalog_tests.rs`; http `catalog.rs`, `daemon_keys.rs`, `hub.rs` (`GET_ROUTES`), `door.rs`, `no_domain.rs`; eval `catalog_scale.rs` (one check by default, three measures `#[ignore]`; it adds `[paths] spec = "areas"` to the generated tree), `workspace_names.rs`. UI: `tree/catalog.test.ts`, `tree/DirectionCatalog.test.tsx`, `mocks/catalog.test.ts`, `mocks/catalog.smoke.test.tsx`, `api/catalog.live.test.tsx`, `daemonKeys.test.ts`, `lazy.test.ts`, `policy.test.ts`.

Gaps: the root-outside note (not reachable in a test), the unreadable-row note, the hub's requests (only `GET_ROUTES` at 30); git not read is exercised on a single-repository copy alone.
