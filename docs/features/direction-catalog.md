---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-eval, fixtures, ui]
ref: RM-0106, v5 core slice 7 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11; the analyst's analysis of 2026-11-12 (section 2, DC-1 to DC-6); the orchestrator's working answers, the owner's review open
adrs: []
---

# direction-catalog: a direction as a table of its documents, with version, state, open items, tasks and last change (RM-0106)

## Why

The owner's vision of 2026-11-11 asks for a game's catalog of mechanics, items, sound and maps, each with its version and its fixes (`08-roadmap.md` Phase 7). A direction today is a flat list of names in the Spec tree (`docs/canon/directions.md` "Tree"): to learn a document's state, its open questions, its tasks or who changed it last, a person opens each one. The browser cannot build the table itself: at 100k nodes the whole trace takes about a second and its JSON runs to tens of megabytes.

After this slice `spec catalog --kind K… [--archive] [--json]` and the daemon's `GET …/catalog` serve one row per node of a kinds direction: title, version, work state, open items, tasks and last change (when and who). In the Spec tree, choosing a direction shows that table where the node pane stands, 50 rows then Show more, sortable by column, a title opening the document modal. Computed per read, never stored; it moves, refuses and gates nothing (ADR-0012). No ADR: it rests on ADR-0076 (a direction is a module the kind reads apply), ADR-0073 (a lens hiding no work), ADR-0056, ADR-0005 (the decision history is readable from git), ADR-0008 and ADR-0012.

## Description and interactions

### Decisions

- **D1. Kinds directions only** (Q2): the rows are exactly `spec tree --kind K… [--archive]`'s nodes with no root, in its order, sections included. A rooted module keeps the tree; no kind is named in code (ADR-0008).
- **D2. Four reads, one call.** The tree (rows, title, `rev`); the trace's reading (each row's `rollup` and its cards, `docs/canon/spec-trace.md` "Marks and rollup"), built once as `spec trace` builds it; the queue's open proposals of this repository, once; git, once.
- **D3. Version N openly** (Q1): the row's `rev` (`docs/canon/node-revision.md`, none written is 1), shown as "Version N"; hashes stay out.
- **D4. State** is the trace's `rollup` for the row's node: `no card`, `departed`, `suspect`, `in work`, `confirmed`, its own cards only. **Tasks** are that node's cards, cancelled ones ignored, and how many are open.
- **D5. Open items** are `open` proposals of this repository (staged ones included) whose `target_id` is the row's ID or whose `node_ids` holds it. A document row does not count its sections' items.
- **D6. Last change per file**, from one walk of `HEAD`'s first-parent line: the newest commit whose diff against its first parent names the row's path, read as node-history's commit entry (`docs/features/node-history.md` D3, D6: `applied_trailers`, `identity_name`). A section row's change is its file's, marked `of_file`. Hand commits count, `proposal` `null`.
- **D7. Cached per `HEAD` in the daemon**: a path's last commit at one `HEAD` never changes, so the daemon keeps them and walks only for paths it has not seen. The CLI keeps none.
- **D8. A served read, no MCP tool**, no plugin bump, no config key, no schema step.
- **D9. The table** fills the node column at a direction's start while no node is open, in a lazy chunk; no table library.

"Data" fixes every name, so the Rust and UI lanes never wait on each other.

## Data

### Command, core, store, CLI library

`spec catalog --kind K… [--archive] [--json]`, at least one `--kind` (free, repeatable, as `tree`'s). Exits: 0 answered, a note included, no row too; 2 on no `--kind` (`` spec catalog: give at least one `--kind` ``), no project, an index that cannot be read, a queue that cannot be read (a newer schema). No REF and no depth: a direction has no root.

Core, `crates/specengine-core/src/catalog.rs`, pure:

```rust
pub const LAST_CHANGE_SCAN_MAX: usize = 20_000;  // first-parent commits walked at most
pub struct CommitFacts<'a> { pub oid: &'a str, pub author_name: &'a str, pub author_email: &'a str,
    pub committed_at: i64, pub message: &'a str }
pub fn last_change(commit: &CommitFacts<'_>, of_file: bool) -> LastChange;
pub struct LastChange { pub at: String, pub by: String, pub by_name: String, pub commit: String,
    pub proposal: Option<String>, pub of_file: bool }
pub struct OpenItem<'a> { pub target_id: Option<&'a str>, pub node_ids: &'a [String] }
pub fn open_items_by_node(items: &[OpenItem<'_>]) -> BTreeMap<String, u64>; // an item once per node it names
```

`last_change`: `at` `utc_timestamp(committed_at)`; `by` the `Decided-by` trailer as written when `applied_trailers` reads the message, else `<author_name> <<author_email>>`; `by_name` `identity_name(by)`; `commit` the full OID; `proposal` the `Proposal:` trailer, `null` on a hand commit. One commit gives one value, exactly node-history's commit entry's `at`, `by`, `by_name`, `id`, `proposal`.

Store, `WorktreeGit`, beside `file_log`:

```rust
/// The newest commit of HEAD's first-parent line whose diff against its first parent names
/// each path, at most `max` commits walked; paths absent at HEAD are never walked for.
pub fn last_commits(&self, paths: &BTreeSet<String>, max: usize) -> Result<LastCommits, GitError>;
pub struct LastCommits { pub found: BTreeMap<String, LoggedCommit>, pub absent: BTreeSet<String>,
    pub unreached: BTreeSet<String>, pub walked: usize }
```

1. One `cat-file --batch-check` session asks `HEAD:<path>` for each path: missing ones go to `absent`.
2. One `log --first-parent --diff-merges=first-parent --no-renames --name-only -z --format=%x01%H HEAD`, read as a stream and stopped (the child killed) once every present path is found or `max` commits are read; the rest go to `unreached`.
3. One `log --no-walk=unsorted --stdin -z` with `file_log`'s format over the distinct found OIDs, `change` `None`.

Each call runs with `GIT_NO_LAZY_FETCH=1`. An unborn `HEAD`: every path `absent`, no git log run.

CLI library, new `crates/specengine-cli/src/catalog.rs`: `CatalogRequest {kinds: Vec<String>, archive: bool}`; `catalog(env, globals, request)` and `catalog_with_view(env, globals, request, view, cache: Option<&LastChangeCache>)`. `LastChangeCache` (`Default`, `Sync`): per (git common dir, `HEAD` OID), each path's `Option<LastChange>`, `absent` and `unreached` as `None`, at most `LAST_CHANGE_HEADS` (4) heads, the oldest dropped. A call reads `HEAD` once (`rev-parse`), takes the cached paths and walks only for the rest, then stores them. The daemon's `AppState` holds one; the CLI passes `None`.

### JSON (`--json`, the daemon's route)

On a git copy of `fixtures/studio-lamp` with two commits on top of its first (`Ann Owner <ann@example.org>` applying PR-0001 to `docs/spec/scn-arrival.md`, then `Bob Hand <bob@example.org>` editing `scn-storm.md` by hand), C-0001 open on `SCN-ARRIVAL`, PR-0040 an open question naming it:

```json
{"kinds": ["scene"], "archive": false, "notes": [], "rows": [
  {"id": "SCN-ARRIVAL", "kind": "scene", "title": "Arrival", "path": "docs/spec/scn-arrival.md", "line": 1,
   "rev": 1, "status": null, "archived": false, "state": "in work", "open_items": 1, "tasks": 1, "tasks_open": 1,
   "last_change": {"at": "2026-01-02T10:00:00Z", "by": "Ann Owner <ann@example.org>", "by_name": "Ann Owner",
     "commit": "<40 or 64 hex>", "proposal": "PR-0001", "of_file": false}},
  {"id": "SCN-STORM", "kind": "scene", "title": "The storm", "path": "docs/spec/scn-storm.md", "line": 1,
   "rev": 1, "status": null, "archived": false, "state": "no card", "open_items": 0, "tasks": 0, "tasks_open": 0,
   "last_change": {"at": "2026-01-03T10:00:00Z", "by": "Bob Hand <bob@example.org>", "by_name": "Bob Hand",
     "commit": "<40 or 64 hex>", "proposal": null, "of_file": false}}],
 "truncated": false, "omitted": null}
```

- A row keeps `spec tree`'s `id`, `kind`, `title`, `path`, `line`, `rev`, `status`, `archived`, as served there, then `state` (the rollup as `spec trace` prints it), `open_items`, `tasks`, `tasks_open`, `last_change`.
- `last_change` is `null` for a path absent at `HEAD` (uncommitted), not reached within `LAST_CHANGE_SCAN_MAX`, or when git is not read; `of_file` is `true` on a section row. `by` is raw, as node-history's CLI and MCP show it; the UI draws `by_name` alone.
- `omitted` counts the rows cut, `null` uncut. The daemon (`View::Browser`) never cuts.

### Text

```text
catalog scene | rows 2
SCN-ARRIVAL | version 1 | in work | open items 1 | tasks 1, open 1 | 2026-01-02T10:00:00Z | Ann Owner <ann@example.org> | PR-0001 | Arrival
SCN-STORM | version 1 | no card | open items 0 | tasks 0, open 0 | 2026-01-03T10:00:00Z | Bob Hand <bob@example.org> | hand | The storm
```

- Header `catalog <kinds joined ", "> | rows <n>`, `[archive]` appended with `--archive`. A row: `<id> | version <rev or -> | <state> | open items <n> | tasks <t>, open <o> | <at>[ (file)] | <by> | <proposal or hand> | <title>`; `last_change` `null` prints `- | - | -` for the three.
- Values pass `one_line`. The cut at `OUTPUT_CAP_CHARS`: rows whole while they fit, then `[truncated: <k> of <n> rows not shown]`.
- Notes, `note: <n>` on stderr and JSON `notes`, sorted, each once: `catalog: last change not read: <cause>` (git missing, not a repository, git's first stderr line), every `last_change` `null`; `catalog: a shallow clone: <k> files its history does not reach read no last change`; `catalog: <k> files not reached in the last 20000 commits read no last change`; `catalog: no queue yet: no open item or task read`; the tree's and trace's own notes as they print them.

### Daemon, hub

- **Route**: `GET /api/projects/:p/catalog`, query `kinds` (repeated, required), `archive`; `spec catalog --json`, browser view, in the project's turn, the daemon's `LastChangeCache` passed. No `kinds` is a 400 naming it; an unknown parameter the 400 listing `kinds`, `archive`. `tests/door.rs` and `no_domain.rs` hold.
- **Keys**, `fixtures/daemon-keys.json`, 3 new sets in "JSON"'s order: `Catalog` (6), `CatalogRow` (13), `CatalogChange` (6).
- **The hub** serves the same route over its checkout (its `--push` clone is neither shallow nor partial, `docs/canon/hub-git.md`) and its own queue. No MCP tool; `HUB_TOOLS` and the plugin version unchanged.

### The UI line

- **Types** (`provisional.ts`, citing DC-9): `CatalogDocument {kinds; archive; notes; rows: CatalogRow[]; truncated; omitted}`, `CatalogRow` the 13 keys, `state` `KnownRollup | Unlisted`, `CatalogChange` the 6. `client.ts` `getCatalog(project, {kinds, archive}, signal)`, `http.ts` the repeated `kinds`. The mock (`src/mocks/catalog.ts`): harbor-sim's direction with 130 rows in every shape (each state, an unknown state, `rev` null, a section row `of_file`, `last_change` null, open items and tasks above 0), and an empty direction.
- **The read** (`queries.ts`): `useCatalog(project, kinds, archive)`, key `["catalog", p, kinds, archive]`; `catalogsOf(p)` joins `READS_OF_THE_SPEC` and is re-read on every `card.*` event and every `proposal.*` but a stage, a read in flight aborted.
- **Where**: `TreeView` at a direction's start (`listing !== null`), with no node open, renders `DirectionCatalog` in the node column in place of `NoNodePane`, through `const DirectionCatalog = lazy(() => import("./DirectionCatalog")…)` in a `Suspense` with a skeleton; `lazy.test.ts` pins it. The flat list on the left stays as design-sweep leaves it; opening a node shows its pane as today; leaving the node returns the table.
- **The table** (`src/tree/DirectionCatalog.tsx`; labels, sort keys and the one compare of a state word in `src/tree/catalog.ts`, the state's look `src/flow/labels.ts` `rollupLook`):
  - a `<table>` captioned by the direction's title (a `bdi`) and its row count; columns Title, Version, State, Open items, Tasks, Last change;
  - a row: the title (else the ID) as a `DocLink` opening the document modal, the ID below it in `mono`; "Version N", or "No version"; a `Badge` of `rollupLook`, an unknown state raw `lang="en"`; the open items' count; `{{count}}, {{open}} open`; a `<time>` (`src/ui/time.ts` `formatUtc`), "by" and `by_name` in a `bdi`, "(file)" on `of_file` with its hint, "Not committed" on `null`; no OID, email or proposal drawn;
  - **sort**: each `<th>` holds a subtle compact `Button` and carries `aria-sort`; Title (`Intl.Collator` in the viewer's language) and State (`ROLLUP_WORDS`' order, unknown last) sort ascending first, Version, Open items, Tasks and Last change (the `at` text, `null` last) descending first; a second press reverses; ties keep served order; the served order stands until a press, and a new direction resets it. Sorting is presentation of served values, no core logic (`docs/canon/architecture.md#ui`);
  - **Show more**: the first `CATALOG_SHOWN` (50) rows of the current order, then a `Button` "Show {{count}} more" (the next 50 or fewer), focus moving to the first newly shown row's title link; a sort keeps the count shown;
  - loading a skeleton; a failure `ErrorPanel` with the daemon's words and Retry; no row an `EmptyState` (level 3); `notes` a note list `lang="en"`.
- **Strings** (`en.ts`, `ru.ts`, the Russian a draft with plural forms), under `tree.catalog`: `caption_one|_other`, `loading`, `failed`, `column.{title,version,state,open,tasks,changed}`, `version`, `noVersion`, `tasks`, `changed`, `ofFile`, `ofFileHint`, `noChange`, `more`, `empty`, `emptyText`, `notes`.
- **CSS**: scale-step tokens only, no legacy alias, no `SCALE_EXCEPTIONS` entry; `.catalog-*` in `app.css`. The primitives as design-sweep leaves them (`Button`, `Badge`, `EmptyState`).

## Rules and edge cases

- WHEN no `--kind` is given, the command SHALL exit 2 and read nothing; WHEN the kinds match no node, it SHALL answer `rows 0`, exit 0.
- WHEN a row's file has uncommitted edits, its last change SHALL be `HEAD`'s, never the working tree's; a new uncommitted file SHALL read `null`.
- WHEN a merge brought a file's change from another branch, the merge SHALL be its last change (its committer time and trailers, its author when none), where node-history may list the side commit (W2).
- WHEN the same `HEAD` is read twice by one daemon, the second read SHALL run `rev-parse` alone of git, and its answer SHALL be byte-equal to the CLI's at that state.
- Determinism: one corpus, queue and git state gives one text and JSON at any root and `HOME`; every date is stored data, no clock read.
- The trace's reading needs the index: an index that cannot be read is exit 2, the daemon's 503, never a clean row (`spec-trace.md` "The read").
- Writes: the index's own refresh only; the cache lives in the daemon's memory.

**Rollout.** No schema step, backup format, config key or tool. A daemon or hub before this slice answers the route 404, and the UI then shows the failure with Retry. The hub takes it at its next upgrade, the owner's call.

**Test churn.** The daemon's 404 route listing and its pins; `daemon_keys.rs` and `daemonKeys.test.ts` (3 sets); the CLI usage golden if it lists commands; `TreeView.test.tsx`'s direction cases (the node column holds the table).

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turns red. Test repositories are built at run time with invented names; "the copy" is "JSON"'s git copy of `fixtures/studio-lamp`.

- [ ] DC-1 — **Rows are the tree's.** On the copy, `spec catalog --kind scene --json`'s row IDs equal `spec tree --kind scene --json`'s node IDs in order (`SCN-ARRIVAL`, `SCN-STORM`); with a section of kind `scene` added, it is listed where the tree lists it; `--archive` matches `tree --archive`. Mutation, red: rows taken from the whole tree.
- [ ] DC-2 — **State and tasks are the trace's.** For each row, `state` equals `spec trace <ID> --json`'s first node's `rollup`, and `tasks` its card count; after `spec card accept`, `in work` becomes `confirmed`, `tasks_open` 0; a cancelled card is in neither. Mutations, each red: `state` always `no card`; cancelled cards counted.
- [ ] DC-3 — **Open items.** On `SCN-ARRIVAL`: an open question naming it in `node_ids` and an open update targeting it count 2; one staged counts; an applied, a rejected, one naming `SCN-STORM`, one of another repository's worktree do not; an item naming both scenes counts once on each. Mutations, each red: applied items counted; the repository filter dropped.
- [ ] DC-4 — **Last change.** On a linear temp repository, each document row's `last_change` equals `spec show <ID> --history --json`'s newest `source: commit` entry (`at`, `by`, `by_name`, `id`, `proposal`), a hand commit included; touching the file's mtime changes nothing; a section row carries its file's with `of_file: true`; an uncommitted new file reads `null`; on a merged branch the merge is the change. Mutations, each red: the file's mtime used; the oldest commit taken; `--first-parent` dropped (the merge case).
- [ ] DC-5 — **No kind in code.** `workspace_names.rs` and http `no_domain.rs` stay green. Mutation, red: `"scene"` planted in `catalog.rs`; the guard names it.
- [ ] DC-6 — **50, then Show more**, on the mock. 130 rows show 50; Show more shows 100 and moves focus to row 51's title link; again, 130 and the button gone. Mutation, red: every row rendered at once.
- [ ] DC-7 — **Text and JSON as pinned.** On the copy, "Text"'s lines byte for byte and "JSON"'s document (OIDs matched by shape); no `--kind`: exit 2, nothing read; `--kind nothing`: `rows 0`, exit 0; git absent from `PATH`: every `last_change` `null`, the note, exit 0. Mutation, red: `by_name` printed for `by`.
- [ ] DC-8 — **One walk, then the cache.** A seam counts git commands, trace readings and proposal reads: one CLI call over 40 rows in 12 files runs one `cat-file --batch-check`, one walking `log`, one `--no-walk` `log` and `rev-parse`, one trace reading, one proposal read. The daemon's second request at the same `HEAD` runs `rev-parse` alone; after a new commit, the walk again; a request with one more kind walks only for the new paths. Every answer byte-equals the CLI's. Mutations, each red: a `file_log` per path; the cache keyed without `HEAD`.
- [ ] DC-9 — **The daemon serves it, the UI types it.** `GET …/catalog?kinds=scene` byte-equals `spec catalog --kind scene --json`; no `kinds` and an unknown parameter are 400s. The fixture holds the 3 sets; `provisional.ts` types them, `daemonKeys.test.ts` green, `AHEAD_OF_FIXTURE` empty at the end. Mutation, red: `archive` not passed through.
- [ ] DC-10 — **The table**, on the mock. A direction chosen, no node open: the table, captioned, in the node column; a rooted module or an open node: none. Pressing Version sorts descending (`aria-sort="descending"`), again ascending; Title by the viewer's collation; ties in served order. A title opens the document modal, its anchor the opener. An unknown state is raw under `lang="en"`; no email or OID is in the DOM. The keys in `en.ts`, the Russian forms in `ru.ts`; `policy.test.ts` and `scale.test.ts` pass, the slice's CSS naming no legacy alias. After `pnpm build`, the table's chunk is not in first load, first-load JS grows at most 2 kB, ≤ 845 kB. Mutations, each red: a static import of `DirectionCatalog`; a state word compared outside `catalog.ts`; `by` drawn.
- [ ] DC-11 — **[measure] At scale.** `specengine-eval scale --corpus-only` trees committed into git with a seeded history (`crates/specengine-eval/tests/catalog_scale.rs`, `#[ignore]`: the eval writes a `git fast-import` stream of 10 000 commits, each editing 1 to 5 seeded part files): `spec catalog --kind node --json`, warm, release, at 1k and 10k nodes, the git half timed by DC-8's seam, at 100k reported. Bars: the git half ≤ 1 s at 10k, the whole ≤ 2 s at 10k; times in "Implementation". Past a bar, "Known limits" names the next step (a per-`HEAD` index of last changes).
- [ ] DC-12 — **[look]** The orchestrator's, in Chrome, both themes and languages, 1440x900 and 1100x800: the mock's direction table, a sort, Show more, a section row's "(file)", a title's modal, an empty direction.
- [ ] DC-13 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`, `workspace_names`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Rooted modules (Q2); a new nav section (`SECTIONS` untouched); an MCP tool; per-section last change (W1); history beyond the newest commit (node-history's part); filtering rows by state, status or text; editing from the table; a stored or committed catalog; Flow drawn through a direction (RM-0102).

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{catalog,lib}.rs`; `crates/specengine-store/src/worktree.rs` (`last_commits`); `crates/specengine-cli/src/{main,lib}.rs` and a new `catalog.rs` (and `trace.rs` if its reading needs a shared entry); `crates/specengine-http/src/{app,args}.rs` (the route, `AppState`'s cache) |
| test-engineer, Rust | `catalog.rs` in core `tests/`, CLI and http (new); store `worktree` tests of `last_commits`; DC-8's seam; `crates/specengine-eval/tests/catalog_scale.rs`; `daemon_keys.rs`; `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`) with, in the same change, `daemonKeys.test.ts`'s 3 sets, names and count, the catalog entries out of `AHEAD_OF_FIXTURE`; the churn |
| ui-developer | "The UI line": `provisional.ts`, `client.ts`, `http.ts`, the mock, `queries.ts`, `src/tree/{TreeView,DirectionCatalog}.tsx`, `src/tree/catalog.ts`, `src/app/lazy.test.ts`, `src/styles/app.css`, `en.ts`, `ru.ts`. The 3 sets in `AHEAD_OF_FIXTURE` until the fixture lands |
| test-engineer, UI | the UI gates, DC-6's and DC-10's mutations, the first-load figure |

Comments cite `docs/features/direction-catalog.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after `delivery-analytics` part a ships (one Rust slice at a time), with `node-history` shipped (its core `applied_trailers`, `identity_name`, store `file_log`'s record parser) and `design-sweep` part a merged (the Spec tree on the primitives, the pane).
2. The ui-developer lands the UI line behind `AHEAD_OF_FIXTURE`; the rust-developer goes core, store, CLI, http.
3. The Rust test-engineer overlaps: core and store (DC-4's halves), CLI (DC-1 to DC-3, DC-7, DC-8), http (DC-9), DC-11. Then the Rust and UI reviews.
4. The orchestrator's look (DC-12). Ship: the spec-writer ("Canon at ship").

At most three iterations a lane.

### Risks

- **A long history**: the walk ends at the oldest last change among the rows; a file untouched since the root walks to the root, so `LAST_CHANGE_SCAN_MAX` bounds it and DC-11 measures it.
- **10k rows in the browser**: only the shown rows render; the sort runs over served values once per press.
- **A killed walk**: stopping `git log` early must not leave a zombie or a stalled pipe; the store's tests cover it.

### Canon at ship

Free bytes now in brackets; "net" means trimmed in the same file to fit.

| File | Edit |
|---|---|
| `docs/canon/spec-catalog.md` (new, Tier 2, `spec-*` shard) | the rule: rows, state, tasks, open items, last change and its walk, the cache; JSON and text; daemon, hub; the table; "Known limits" (per-file change, merges, the scan cap, the measured cost); "Tests" |
| `docs/canon/directions.md` [4 504] | "Tree": the table at a direction's start; "Rules": the read; drop "It adds no … read, route" |
| `docs/canon/spec-history.md` (node-history's) | a pointer: the catalog reads its commit entry |
| `docs/canon/ui-languages.md` [9] | "Size": the first-load figure. Net |
| READMEs: core [9], store [3], cli [1], http [4], ui [2 049] | a pointer; store's `last_commits`; http's route row and the cache. Net |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 7; RM-0106 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q1. Show "Version N" openly, though v5 folds revisions? | Yes, in the catalog as in History; hashes stay out | Folding it hides the very version he asked for |
| Q2. Rooted modules as well as kinds directions? | Kinds directions only, for now | 1-2 more hours, and a rule for how deep a subtree's rows go |
| Q3. Last change from where? | node-history's commit reading, per file, the newest commit of `HEAD`'s first-parent line, cached per `HEAD` in the daemon | Per node (span bytes judged per commit): a blob parse per row per commit, the cost node-history pays for one node |
| Q4. What counts as an open item? | Open proposals naming the node, as target or in `node_ids` | Including its sections' items needs `within` per row, and a document's count would repeat its sections' |

**The spec-writer's:**
- W1: per file, since a section's own change needs a parse of every version; `of_file` says so, and History has the exact answer.
- W2: the first-parent line makes a merge the change on the line the reader is on; node-history follows `--follow`'s simplification. Both are named, DC-4 pins both.
- W3: the table stands in the node column, so the flat list keeps its keyboard path and Show as root; the table never hides work (ADR-0073).
- W4: `rev-parse` per call keeps the cache honest after a commit, a checkout or a hub's pull.
- W5: keys under `tree.catalog`, the screen's namespace.
