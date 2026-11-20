---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-20
---

# spec library: every document as a library, with its decisions and tags

Built by `doc-library` (RM-0127, 2026-11-19), no ADR. It rests on ADR-0022 (the four classes), ADR-0073 and ADR-0076 (a workspace is a lens), ADR-0004, ADR-0005, ADR-0008 and ADR-0012. Related: `spec-catalog.md` (last changes, their cache), `spec-map.md`, `spec-usage.md`, `doc-view.md`. Code: model `node.rs` `Fields.tags`; core `library.rs` (`library`, `judge_tags`, `kept_tags`, `decision_group`, `newer`, `behind`), `map.rs` `CanonicalNames`, `check/engine.rs` (`tag-invalid`), `SpecGraph::listed_under`; CLI `library.rs`, `catalog.rs` (`git_read`, `GitRead::notes_of`); http `app.rs`; UI `src/documents/`, `tree/UsedCell.tsx`.

## The rule

`spec library [--archive] [--json]` and `GET /api/projects/:p/library[?archive=true]` serve one row per document, computed per read, never stored. The read moves, refuses and gates nothing; "behind its decision" is a hint, never a finding (ADR-0012). It writes only the index's own refresh; the cache lives in the daemon's memory. One corpus, config and git state gives one text and one JSON at any root and `HOME`; every date is stored data.

- **Input**: the index as stored after the CLI's freshness step (`indexed(…, false)`), one spec graph over its decoded rows, never `spec tree`'s re-read of each listed file. The library prints no file, so nothing it shows is stale text, and a commit in a hub's checkout never makes the hub doubt its index (`spec-read-freshness.md`). `[[module]]` tables: `WorkspaceConfig::from_toml`.
- **Rows**: each walked file with a document node (ord 0), live and generated always, Tier 3 under `--archive` (`is_tier3`), by path in byte order. A file with no document node (not UTF-8, unreadable) is none.

## Rows

JSON `{archive, notes, counts, rows, truncated, omitted}`; `counts {documents, archived, decisions, behind, orphans}` over the rows shown, after the cut. A row's 23 keys `{name, id, path, title, class, kind, status, archived, tier, owner, reviewed, scope, tags, disciplines, rev, parent, modules, used_by, mentioned_by, orphan, last_change, decisions, behind}`:
- `name`: `CanonicalNames`, the map's rule for any node (`slug/ID`, the ID, else the path); the modal's REF.
- `id`, `title`, `class`, `kind`, `status`, `tier`, `owner`, `reviewed` (text as written), `scope`: as indexed, `null` when absent. `tags`: "Tags"; `disciplines`: the kept ones (`card-practices.md`). `rev`: `spec catalog`'s.
- `parent`: the canonical name of the node the document is listed under (`listed_under`, `ancestors`' first step); `null` with no `parent:`, a dangling or another project's one, or a cycle broken at it.
- `modules`: the served `[[module]]` names holding the row, config order: a direction whose `kinds` list its `kind`; a rooted module whose root is the row's document or one of its `ancestors` (each chain walked once), the root resolved as `spec show` resolves a REF, first holder by (path, ord).
- `used_by`, `mentioned_by`, `orphan`: core `usage_of`, as the map's.
- `last_change`: `spec catalog`'s less `of_file`, or `null`; git read once per call for the rows' and their decisions' files (`git_read`, the daemon's `LastChangeCache`).

## Decisions

Under a row: each resolved front-matter `canon` link whose source has `class: decision` (live or Tier 3, whatever `--archive` says) and lands on the row's document or an ID section in it (an anchor: the innermost ID section holding it). A link with several holders lands on its first holder that is a row, by (path, ord), so a Tier 3 first holder never drops it. Each decision once, by its first such link. A `canon:` lands only on a canon file (`canon-file`): only canon rows hold decisions. Keys (10) `{name, id, title, path, status, group, date, section, last_change, newer}`:
- `group` (`decision_group`): `accepted` (exactly), `superseded` (`superseded-by `…), `rejected` (exactly), `other` (anything else, absent too): the convention's words, the only status words read.
- `date` as written; `section`: the ID section landed on, else `null`.
- `newer`: `true` when both last changes are read and the decision's `at` is later, to the second; `false` when read and not (one commit for both: the promotion rule kept); else `null`.
- Order: accepted, superseded, rejected, other; `date` descending as text, `null` last; `name`.

`behind`: `true` when an accepted entry is `newer`; else `null` when an accepted entry's `newer` is `null`; else `false`.

## Tags

`tags:`, typed (`TYPED_KEYS` 32, `List`: core README "Front-matter"), never counted (`node-revision.md`). A tag is 1 to 64 UTF-8 bytes (`TAG_MAX_BYTES`), any script, no control character, no whitespace at an end. Entries are judged in written order; a passing one is kept while fewer than 32 (`TAGS_KEPT_MAX`) are. `spec check`'s `tag-invalid`, a warning, subject `tags` at the key's line, per entry left out:

```text
tags: entry <n> is empty or has a space at an end
tags: entry <n> is <b> bytes; a tag has at most 64
tags: entry <n> holds a control character
tags: entry <n> repeats entry <m>
tags: entry <n> is past the 32nd kept tag; a document keeps 32
```

The row serves the kept entries, compared byte for byte. A bare string or a mapping: `frontmatter-type`, `[]`, the document's `norm_hash` null. A closed `[classes]` contract without `tags`: `key-extra` (this repository lists none). A tag changes only as front-matter, by a proposal or a hand commit; no screen writes one. `INDEX_FORMAT` 18: a file the old build stored, unchanged since, holds `tags` in `extra`.

## Text, notes, exits

```text
library | documents <n>, archived <a> | decisions <d> | behind <b> | orphans <o>[ [archive]]
<name> | <class or -> | <kind or -> | <status or -> | version <rev or -> | tier <t or -> | owner <o or -> | reviewed <r or -> | tags <t, … or -> | used by <u>, mentioned by <m>[, orphan] | <at or -> | <by_name or -> | decisions <k> accepted, <s> superseded, <r> rejected, <x> other[ | behind] | <title or ->
    decision <name> | <group> | <date or -> | <at or -> | <section or ->[ | newer]
[truncated: <k> of <n> rows not shown; the daemon's library is whole]
```

Values through `one_line`; the cut at `OUTPUT_CAP_CHARS` falls between rows, each whole with its decision lines, the first always; the CLI's JSON holds the rows its text prints. Notes, stderr `note:` lines and JSON `notes`, sorted, once each:
- `spec catalog`'s three git notes worded `library:`, over the rows shown and their decisions' files;
- `` library: module `<m>`: its root `<REF>` names nothing; no row is in it `` (it resolves nothing, is another project's or holds a look-alike);
- `` library: module `<m>`: its root `<REF>`: `@<rev>` is ignored: `spec library` reads the current files ``.

Then, stderr alone, the input's and the parents' warnings in `spec tree`'s words, but a parent's several holders each `path (ID)` (the path alone when it is the name), no line: the index holds none. Exit 0 answered, no row too; 2 on usage, no project, a config `ProjectConfig` refuses, a `HOME` or store error (a newer build's index); never 1.

## Daemon, hub

`GET …/library[?archive=true]`: `spec library --json` less its final LF, the browser view (never cut), through `run_read` with `App.catalogs`, the one `LastChangeCache`, shared with `…/catalog` (at most `LAST_CHANGE_HEADS`, 4, heads). Another name: 400 `` `<name>` is no query name of library: it takes archive ``; a non-boolean `archive`: a 400 naming it; exit 2: 503. The 404 list names it after `rings`. Key sets `Library` (6), `LibraryCounts` (5), `LibraryRow` (23), `LibraryDecision` (10), `LibraryChange` (5). No MCP tool (`get_node`, `search`, `get_tree`, bundle layer 6 serve them); `HUB_TOOLS`, MCP texts, config keys, queue schema 14, backup format 7 unchanged.

On a hub the read trusts (`spec-read-freshness.md` "Routes and tools") and holds the project's turn while it runs, as every spec read. A hand or empty commit in the checkout never makes it walk: a new `HEAD` costs `git=7` once, a warm read `git=3`. It walks when the trust is due (R8, 300 s), after an MCP write (which walks on its own line), or after a `tree` read found changed bytes (R13); until a reconcile it shows the trusted index's fields for a hand-edited file (R8 "The window").

## The screen

`#/<p>/documents`, `documents` after `tree` in `SECTIONS` (`workspaces.md`); `DocumentsView`, a lazy chunk; `useLibrary`, key `["library", p, archive]`, in `READS_OF_THE_SPEC`.
- An `h1`; "N documents" or "K of N documents", "B behind their decision"; a case-folded search over title, name, path, tags.
- Facets, in order Class, Kind, Status, Tags, Discipline, Module, Owner, Tier, Scope: ANDed, values ORed, each with the count the other facets keep, by count then `Intl.Collator`, "No <facet>" last. Class starts at `canon` when a row has it, the rest at All. Module is one choice, the module bar's stored `specengine.module.<p>` (`.<login>` on a hub), else the department's `module`; All modules one press. Toggles Behind its decision, Orphans; Include archive, a second read keeping the table; Clear filters, all to All. State per project for the page's life.
- Columns: Title (`DocLink` by `name`, the name in mono below when it differs), Class, Kind, Status, Version, Owner, Reviewed, Last change ("Not committed"), Used by (`UsedCell`, the catalog's), Decisions ("N accepted · M superseded", a "Behind its decision" `Lozenge`; a button opening a full-width row of groups, each entry a `DocLink`, `date` or "No date", "Committed <time>", its section, "Newer than this document"; an unknown group raw, `lang="en"`), Tags (chips). Sort as the catalog's (Version, Reviewed, Last change, Used by descending first). 200 a page (`LIBRARY_SHOWN`), "Show 200 more".
- States: a skeleton; an `ErrorPanel` with Retry; with nothing answered, any failure (a pre-slice daemon's 404 too) keeps the lone Include archive checkbox above it; "Showing the last answer, with (without) the archive" when the kept rows' archive differs from the checkbox; an `EmptyState` for none, one with Clear filters for no match. The table scrolls in its own region; at 320 px the page never does. Served values as written, never translated; strings `documents.*`, `app.sections.documents.*`; CSS `.documents-*`.

## Known limits

- **Commit times, not topology**: a rebase or a skewed clock can misorder `newer`; a later edit of an accepted decision (a typo) reads newer; a decision landed before its canon diff reads not behind, one after it behind until the page's next commit (`proposal-decision.md`).
- Decisions outside `[paths]` (an excluded archive) are not listed, unsaid. `behind: null` looks as `false`, no mark: the owner's question. A mistyped `tags:` nulls its document's `norm_hash` until fixed.
- UI: after a failed Retry, focus may land on the table, not the Include archive checkbox; the "Showing the last answer" line has no live region.
- **Cost** (release, `catalog_scale.rs`'s tree, 10 000 commits): 10k nodes (1 631 documents): CLI 370 ms, the hub's warm `run_ms` 71, after a commit 322, the first walked read 1 291, RSS 92 MB, body 843 599 B (about 0.5 KB a row); 50k (8 115): warm 315 ms, after a commit 874, walked 6 202, RSS 459 MB. 10 readers, 8 MCP writers and the Inbox every 5 s for 120 s: library worst 2 065 ms, 641 writes stored. The soak (2×300 s, 10 readers, a commit every 10 s): RSS slopes 0.0 and 0.062 MiB/min (bar 0.5). Past 10 000 documents the next step is a served facet query.

## Tests

core `tests/library.rs`, `norm_recipe.rs`, `check_output.rs`, `workspace_toml.rs`; CLI `tests/library.rs`, its scene `library_scene/`; http `tests/library.rs`, `daemon_keys.rs`, `hub.rs`; store `tests/format.rs`, `format_history.txt`; eval `library_scale.rs`, `library_soak.rs` (`#[ignore]`, `library_common/`), `ui_policy.rs`, `layout_cli.rs` (typed keys). UI `documents/{library.test.ts,DocumentsView.test.tsx}`, `mocks/library{,.smoke}.test.*`, `api/library.live.test.tsx`, `daemonKeys.test.ts`, `lazy.test.ts`, `policy.test.ts`, `i18n/catalog.test.ts`.
