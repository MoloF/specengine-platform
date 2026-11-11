---
class: spec
status: draft
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui]
ref: RM-0035, v5 core slice 2 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11 (each item with its version, its fixes, who changed it and when); the analyst's analysis of 2026-11-12 (section 1, NH-1 to NH-7, here NH1 to NH7); the orchestrator's working answers, the owner's review open
adrs: []
---

# node-history: a node's changes, who made them and when, on the CLI, over MCP and in the document view (RM-0035)

## Why

`spec show` prints a node's text and `rev`, but not who changed it, when, through which proposal or why. The owner's vision of 2026-11-11 asks for each item with its version, its fixes, and who changed it and when (`08-roadmap.md` Phase 7). ADR-0005 promises "the decision history is readable from git", and every apply commit carries its trailers (`docs/canon/proposal-apply.md` "Commit"), yet nothing reads them back. 07 §2 lists `spec show --history` as unbuilt, and 07 §1.2 has `get_node with: history` "still to come".

After this slice, `spec show R-12 --history`, `get_node {id, with: ["history"]}` and the document view's History part list a node's changes, newest first. They show the commits that changed its span (hand commits included, read from their trailers), the tasks accepted on it, and the questions, discrepancies and decisions applied to it. The list is built per read from git and the queue. Never stored, it feeds no rollup, mark or gate (ADR-0012). No ADR: the slice rests on ADR-0005, ADR-0001, ADR-0046 (a layer that cannot be read is a note) and ADR-0012.

## Description and interactions

### Decisions

- **D1. Opt-in, per node,** beside links and bindings: `spec show REF [--links] [--bindings] [--history] [--archive]`, `get_node`'s `with` gaining `"history"`, and the daemon's `GET …/nodes/<REF>?with=history`. Each shown node gains `history`, `null` when not asked. Without the flag, no git runs and no queue row is read for it.
- **D2. Commits that changed the span.** One `git log --follow` reads at most `HISTORY_SCAN_MAX` (128) commits of the holder's file, newest first. One `cat-file --batch` session gives each commit's blob and its first parent's. Core parses both and lists the commit only when the node's span bytes differ. The scan stops where the node first appears.
- **D3. Trailers parsed in core** from `%B`, never with git's trailer options. An apply commit has the subject `spec: apply <ID>`, and its last paragraph holds `Proposal: <the same ID>`. Any other commit is a hand commit, listed and marked.
- **D4. The queue joins.** Cards with `acceptance` set and a target equal to the node's ID or a section's ID inside it. Applied questions, discrepancies and decisions of this repository whose `node_ids` meet those IDs. An item already listed as a commit's `Proposal:` is not listed again.
- **D5. One list,** newest first, at most `HISTORY_MAX` (64). `truncated` is set when more existed or the scan stopped short.
- **D6. Raw on the CLI and MCP, names in the UI.** `by` is the value as written (`Name <email>`), and `by_name` its name part. The UI draws `by_name` alone, on a daemon too: no hub branch.
- **D7. Degrades to a note.** Git missing, not a repository, a shallow clone or a missing blob gives one `note: history:` line per holder and `history: null`. The exit is unchanged, as bindings degrade (`docs/canon/code-reads.md` "Degrade").
- **D8. Plugin 0.40.0.** `with` takes a value, and `get_node` returns a key: a MINOR. `INSTRUCTIONS` and `read-spec` are unchanged.
- **D9. The document view.** A collapsed History part after Inside, read on its first expansion through the node route, inside `DocBody.tsx`'s lazy chunk (`docs/canon/doc-view.md` "Inside" is the pattern).

"Data" fixes every name, so the lanes never wait on each other.

## Data

### Command, core, store

`spec show REF [--links] [--bindings] [--history] [--archive]`. Exits are unchanged: 0 answered (a note included), 1 nothing found, 2 as today. `ShowRequest` gains `pub history: bool`, set by CLI `main.rs` from the flag, MCP `read.rs` and http `app.rs` from `with`, and `false` at any other construction. `ShownNode` gains `pub history: Option<ShownHistory>` after `bindings`.

Core, `crates/specengine-core/src/history.rs`, pure:

```rust
pub const HISTORY_MAX: usize = 64;        // entries per node
pub const HISTORY_SCAN_MAX: usize = 128;  // commits of the file read at most
pub const HISTORY_TEXT_MAX: usize = 300;  // characters of a rationale or a title
pub fn applied_trailers(message: &str) -> Option<AppliedCommit>; // None: a hand commit
pub fn identity_name(value: &str) -> &str;  // "Ann Owner <ann@example.org>" -> "Ann Owner"; no " <…>" suffix: the whole
pub fn proposer(proposed_by: &str) -> Option<String>; // "agent role=spec-writer model=m run=r" -> "agent spec-writer"; role unknown -> "agent"
```

`AppliedCommit {proposal, decided_by, proposed_by, revisions: Vec<(String, Revised)>, rationale}`, where `Revised` is `Raised {from, to}` or `Editorial {rev}` (`editorial_revision`'s grammar). `rationale` is the text between the subject's blank line and the trailer block, `None` when empty. The judge finds the node's span and revision in one version of the file, by ID, or the whole file for an ID-less REF. It uses `spec show`'s parse and `node-revision.md`'s "Revision" (none written is 1, unknown `null`).

Store, `WorktreeGit`, next to `branch_commits_with_trailer`:
- `file_log(path, max)` gives per commit its OID, first parent, author name and email, `%ct`, raw message, and its path before and after (a rename's old path from the log's own record).
- The blobs `<commit>:<path>` and `<commit>^:<old path>` come from one `cat-file --batch` session, each OID once (`verify_git.rs`'s pattern).
- The calls run with `GIT_NO_LAZY_FETCH=1`, so a partial clone never fetches.

### JSON (`--json`, `structuredContent`, the daemon's node route)

Per node, after `bindings`: `"history": null`, or `{entries, truncated, omitted}`. `omitted` is the text cap's count, as `bindings`' is, and the daemon (`View::Browser`) never cuts. Two of the four sources' shapes:

```json
{"entries": [
  {"source": "commit", "at": "2026-01-02T10:00:00Z", "id": "<40 or 64 hex>", "change": "raised", "created": false,
   "rev": 2, "rev_before": 1, "by": "Ann Owner <ann@example.org>", "by_name": "Ann Owner",
   "proposal": "PR-0001", "proposed_by": "agent role=spec-writer model=m run=r", "proposer": "agent spec-writer",
   "record": null, "title": null, "rationale": "Tighten the window."},
  {"source": "item", "at": "2026-01-01T12:00:00Z", "id": "PR-0040", "change": "decision", "created": false,
   "rev": null, "rev_before": null, "by": "Ann Owner <ann@example.org>", "by_name": "Ann Owner",
   "proposal": null, "proposed_by": null, "proposer": null, "record": "DEC-0031", "title": null, "rationale": null}],
 "truncated": false, "omitted": 0}
```

| Key | commit | card | item |
|---|---|---|---|
| `source` | `commit` | `card` | `item` |
| `at` | committer time, UTC (`utc_timestamp`) | `acceptance.at` | `decided_at` |
| `id` | the full OID | its ID | its ID |
| `change` | `raised`, `editorial`, `applied` (no `Revision:` for this node), `hand` | `accepted` | `question`, `discrepancy`, `decision` |
| `created` | absent in the parent's version, or a root commit | `false` | `false` |
| `rev`, `rev_before` | after and before (`rev_before` `null` when created) | `null` | `null` |
| `by` | `Decided-by` as written, else the author `Name <email>` | `acceptance.by` | `decided_by` |
| `by_name` | `identity_name(by)` | the same | the same |
| `proposal`, `proposed_by`, `proposer` | the trailers as written, `proposer(…)`; `null` on a hand commit | `null` | `null` |
| `record` | `null` | `null` | `record_id` |
| `title` | a hand commit's subject, else `null` | its title | `null` |
| `rationale` | an apply's, else `null` | `null` | `null` |

`title` and `rationale` are one line, as `one_line` prints them, cut at `HISTORY_TEXT_MAX` characters on a character boundary plus `…`. The JSON holds exactly what the text prints.

### Text

The history block follows the bindings block (else links, else the header line), before the node's text. It counts toward the cap as those blocks do (`docs/canon/spec-cli-read.md` "The cap"), one entry being one unit. NH1's repository prints:

```text
  2026-01-04T10:00:00Z | hand | rev 2 | - | Bob Hand <bob@example.org> | - | 4f1c2ab3d5e6
    Fix a typo in the window rule
  2026-01-03T10:00:00Z | editorial | rev 2 | PR-0002 | Ann Owner <ann@example.org> | agent role=spec-writer model=m run=r | 9a8b7c6d5e4f
    Wording only.
  2026-01-02T10:00:00Z | raised | rev 1 -> 2 | PR-0001 | Ann Owner <ann@example.org> | agent role=spec-writer model=m run=r | 1a2b3c4d5e6f
    Tighten the window.
  2026-01-01T10:00:00Z | hand created | rev 1 | - | Bob Hand <bob@example.org> | - | 0f9e8d7c6b5a
    Add the rules
  history 4
```

- Commit: `<at> | <change>[ created] | rev <r> | <proposal or -> | <by> | <proposed_by or -> | <OID's first 12 hex>`, where `<r>` is `<before> -> <after>` when both are known and differ, else `<after>`, else `-`. Then `    <rationale or title>` when one is set.
- Card: `<at> | accepted | <id> | <by> | <title>`. Item: `<at> | <kind> | <id> | <record or -> | <by>`.
- Closing: `  history <n>[, truncated]`. A cut in the block falls after the last whole entry that fits. The text is then not shown, and the tail gains `; history not shown: <k>` after its bindings part.

### Order, cap, notes

- **Order:** by `at`, newest first. On a tie, commits come before cards and cards before items, commits in log order, the others by ID.
- **The scan** reads up to 129 commits. It stops at the node's creation or the file's first commit. If it reaches the 129th, `truncated` is set and queue entries older than the last commit read are dropped. Past `HISTORY_MAX` entries, the rest drop and `truncated` is set.
- **What git does not hold:** an uncommitted file or edit shows no commit entry, and an unborn branch gives no commit entry and no note.
- **Notes,** one stderr line per holder not read, its `history` `null`:
  - `note: history: <path>: not read: <cause>`: git missing, not a repository, or git's first stderr line;
  - `…: not read: a shallow clone` (`rev-parse --is-shallow-repository`, once per read);
  - `…: not read: a blob is missing (a partial clone?)`.

  A queue that cannot be read gives `note: history: the queue not read: <why>; commits only`. One not yet created gives nothing.

### MCP, daemon, hub, plugin

- **`get_node`:** `With` gains `History`, so the inlined enum is `["links", "bindings", "history"]`. `WITH_DOC` gains `; ["history"]: each node's changes, newest first.`
- **The mirror** (`mirror.rs`): `ShowNode.history: Option<ShowHistory>` after `bindings`. `ShowHistory {entries, truncated, omitted}`, and `ShowHistoryEntry` holds the 15 keys, `source` and `change` closed enums.
- **`node_body!`** (stdio's and the hub's): the head becomes `spec show REF [--links] [--bindings] [--history] [--archive]`. Before `archive:` goes `with: ["history"] lists the commits that changed each node's text, then the cards accepted on it and the items decided on it, newest first, at most 64, trailers as written; no git history gives a note and null, never an error.` That makes stdio 1 909 B and the hub 2 000 B (≤ 2 048), ASCII, with "at most 64" asserted against `HISTORY_MAX`.
- **The daemon:** `args.rs` `WITH_VALUES` holds the three, and its 400 reads `` `with=<v>`: the values are `links`, `bindings` and `history` ``. `nodes` passes `history`. Daemon keys: `ShownNode` has 21 keys, `history` after `bindings`. New sets: `ShownHistory` (`entries, truncated, omitted`) and `HistoryEntry` (the 15, in the table's order). `fixtures/daemon-keys.json` names 124 sets.
- **The hub** serves the same route and tool. The git half runs inside the project's turn, as every read does (W2). Its `--push` clone is neither shallow nor partial (`docs/canon/hub-git.md`), and it reads the hub's queue. `HUB_TOOLS` (18) and `INSTRUCTIONS` are unchanged.
- **The plugin:** `plugin.json` and `scripts/deploy/.env.example` go to 0.40.0. `PINS` and `SCHEMA_PINS` (`plugin_files.rs`) append 0.40.0 entries. `read-spec` is unchanged (4 091 of 4 096 B after `card-context`).

### The UI line

- **Types** (`provisional.ts`): `ShownNode.history: ShownHistory | null` after `bindings`. `ShownHistory {entries: HistoryEntry[]; truncated: boolean; omitted: number}`. `HistoryEntry` holds the 15 keys, `source` and `change` `Known… | Unlisted`, citing NH12. `NodeOptions.with` takes `"history"`. The mock's `getNode` serves a sample for it, else `null`.
- **The read** (`queries.ts`): `useNodeHistory(project, ref, enabled)` calls `getNode(…, {with: ["history"]})`, its key among `READS_OF_THE_SPEC`, so an apply re-reads it.
- **The part** (`src/doc/DocHistory.tsx`, labels in `src/doc/history.ts`, inside `DocBody.tsx`'s chunk):
  - a collapsed `Disclosure` (`doc.history.title`) in Main after Inside, read on the first expansion, from the first holder; the Shell keeps it open per project (`DocState {insideOpen, historyOpen}`);
  - a skeleton while reading; on a failure, the daemon's words and Retry; `history: null` shows the read's notes, `lang="en"`; no entries shows `doc.history.none`;
  - an `ol`, newest first. Each entry says what changed: `doc.history.version` (rev); `doc.history.wording` (editorial); `doc.history.outside` (hand), with `doc.history.created` before it when created; `doc.history.task` (its ID linking `sectionHash(p, "board", id)`); `doc.history.question|discrepancy|decision` (the record a `DocLink`). An unknown `change` is raw, `lang="en"`;
  - each entry also shows a `<time>` date (`src/ui/time.ts`), `doc.history.by` with `by_name` in a `bdi`, and on an apply `doc.history.proposedBy` with `proposer` (`lang="en"`) and the proposal linking `sectionHash(p, "inbox", id)`. The rationale or title follows on its own line, and `truncated` adds `doc.history.truncated`;
  - no OID and no `by` is ever drawn. "Version N" is open.
- **Strings** (`en.ts`, `ru.ts`, the Russian a draft): `doc.history.{title, version, wording, outside, created, task, question, discrepancy, decision, by, proposedBy, none, truncated}`.

## Rules and edge cases

- WHEN `--history` is not given, the read SHALL run no git command and read no queue row for it. `history` SHALL be `null`, and no `note: history:` SHALL print.
- WHEN a commit touched the file but not the node's span bytes, it SHALL NOT be listed. A document's span is its file, so it lists every commit of the file.
- WHEN a `Proposal:` line stands in a rationale, it SHALL never count. Only the last paragraph of an apply-subject commit does.
- WHEN the file was renamed, `--follow` SHALL carry the history, each commit judged against its own paths.
- Determinism: one corpus, git and queue state gives one text and one JSON, byte-identical at other roots and `HOME`s. Every date is stored data, and none comes from the clock (`docs/canon/mcp-read.md` "Parity").
- Writes: the index's own refresh only. History writes no queue row and no event, and nothing under the root.
- Merges: git's default simplification decides which show. A listed merge is judged against its first parent.

**Rollout.** An older `specengine-mcp` (a terminal, or a hub before 0.40.0) refuses `with: ["history"]` through the schema's enum. There is no schema step, backup format or config key. The hub takes it at its next upgrade, the owner's call.

**Test churn.**
- Every `spec show --json` pin and daemon `ShownNode` key list gains `history`: CLI `show*.rs`, `norm_show.rs`, `doc_modal.rs`'s golden if it lists keys, MCP read tests, http `daemon_keys.rs`.
- http `query.rs` (about 378, 451) takes the 400's text.
- `plugin_files.rs`, `plugin_roles.rs` and `spec_ids_hub.rs` take the version, and `mcp_hub.rs` the hub's `get_node` texts and schema if pinned. `mcp_size.rs` gains a case.

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red.

The new Rust tests are `node_history.rs` in CLI and MCP, plus core's `#[cfg(test)]`. Their temp repositories are built at run time with invented names, and their commits carry fixed 2026-01 author and committer dates. Apply messages come from core's `commit_message_raising`, and one apply runs through `spec approve` end to end.

- [ ] NH1 — **Only commits that changed the span.** `docs/rules.md` holds `{#R-12}` and `{#R-13}`. Its commits are A, by hand, creating both; B, PR-0001 raising R-12 1 -> 2; C, PR-0002 editorial on R-12; D, by hand on R-12; E, by hand on R-13 only.
  - `spec show R-12 --history --json` lists exactly D, C, B, A: `hand`, `editorial`, `raised`, `hand`, with `created` on A alone. Revs are 2/2, 2/2, 2/1 and 1/null, and `truncated` is false.
  - The text is "Text"'s, byte for byte, with the real OIDs.
  - `spec show docs/rules.md --history` lists E too.

  Mutation, red: every commit of the file listed.
- [ ] NH2 — **The trailer block is the last paragraph.** B's rationale holds `Proposal: PR-9999`, yet its `proposal` is `PR-0001`. A hand commit ending in a `Co-Authored-By:` paragraph is `hand`, and so is a `spec: apply PR-0003` subject with no `Proposal:` trailer. Core unit tests cover `applied_trailers`, `identity_name` (`"Ann <a@b>"`, `"mira"`, `"x <y"`) and `proposer` (`role=unknown` gives `"agent"`). Mutation, red: every body line scanned.
- [ ] NH3 — **Renames.** After `git mv docs/rules.md docs/law/rules.md` and one more edit of R-12, the history lists that edit and all of NH1's. Mutation, red: `--follow` dropped.
- [ ] NH4 — **Not asked, no git.** With a `git` shim first on `PATH` that logs its arguments and runs git, `spec show R-12 --json` exits 0, `history` `null`, no `note: history:`, and no `log` or `cat-file` in the log. Mutation, red: the history always read.
- [ ] NH5 — **No history to read.** In a `git clone --depth 1`, `spec show R-12 --history` exits 0 and prints the text, with `history: null` and exactly one `note: history: docs/rules.md: not read: a shallow clone`. With no `git` on `PATH`, the cause is git missing. Outside any repository, it is not a repository. Mutation, red: git's error passed through (exit 2).
- [ ] NH6 — **MCP parity and schemas.** Stdio `get_node {id: "R-12", with: ["history"]}` content is byte-equal to `spec show R-12 --history 2>&1`, and `structuredContent` equals `--json`. The same holds with all three `with` values and in NH5's clone. The schema walker passes with exact key sets (`history`'s 3, the entry's 15), and the input enum has three values. A hub's `/mcp/<slug>` answers alike with 18 tools. Mutation, red: `rationale` dropped from `ShowHistoryEntry`.
- [ ] NH7 — **The UI reads on expansion only.** Opening the document view sends no `with=history`. The first expansion sends one, and a collapse and a second expansion send none. With `historyOpen` held, it reads on open. Mutation, red: read on open.
- [ ] NH8 — **The queue joins.** Queue rows are written through the store with fixed `now`s:
  - C-0001 targets R-12, accepted; C-0002 targets R-12, not accepted; C-0003 targets R-13, accepted;
  - PR-0040, a decision naming R-12, applied with DEC-0031; PR-0041, an open question naming R-12; PR-0042, a rejected decision naming R-12; PR-0001, an applied update naming R-12.

  The list holds C-0001 `accepted` and PR-0040 `decision` (record DEC-0031), placed by `at`, and none of C-0002, C-0003, PR-0041 or PR-0042. PR-0001 appears once, as B. With the queue file unreadable, the read gives the queue note and the commits. Mutations, each red: open items taken; an unaccepted card taken; the dedup dropped.
- [ ] NH9 — **The cap and the scan.**
  - (a) 70 commits changing R-12 give 64 entries, `truncated: true`.
  - (b) R-12 is created at the first commit, then 130 commits touch R-13 only: `[]`, `truncated: true`.
  - (c) R-12 is created at commit 3 of 5: `truncated` false, and a test seam shows no blob of commits 1-2 was asked.

  Mutations, each red: no entry cap; an unbounded scan ((b) lists the creation); no stop at creation (c).
- [ ] NH10 — **The text cap.** A node with 64 entries, each with a 300-character rationale, and a long text: the text is cut first. When bindings and history pass the cap, the cut falls after a whole entry. The tail ends `; history not shown: <k>`, and the JSON holds the printed entries with `omitted` k, within 40 000 characters plus the tail. The daemon serves all 64, `omitted` 0. Mutation, red: a cut inside an entry.
- [ ] NH11 — **Plugin 0.40.0 and the texts.** `plugin.json` and `.env.example` hold 0.40.0, and `tag-check.sh v0.40.0` prints it. `PINS` and `SCHEMA_PINS` end with 0.40.0. `NODE_DESCRIPTION` is 1 909 B and the hub's 2 000 B, each with the sentence once. `INSTRUCTIONS` (stdio and hub) and `read-spec` are byte-equal to before. Mutations, each red: the schema changed with no `SCHEMA_PINS` entry; `HISTORY_MAX` 65 (the build fails).
- [ ] NH12 — **The daemon serves it, the UI types it.** `GET …/nodes/R-12?with=history` serves NH1's entries. `with=bogus` is a 400 naming the three. The fixture names 124 sets, `ShownNode` 21 keys. `provisional.ts` types the three shapes. UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green, `daemonKeys.test.ts` included, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `history` out of the fixture; `with=history` refused.
- [ ] NH13 — **The History part.** On the mock's sample:
  - the labels read "Version 2", "Wording fixed, version 2 kept" and "Edited outside a proposal", and a created entry is marked;
  - C-0001 links `#/<p>/board/C-0001`, and PR-0040's record is a `DocLink`;
  - an unknown `change` is raw `lang="en"`; `truncated`, `null` and `[]` each show their line;
  - an entry with `by` `Ann Owner <ann@example.org>` renders "Ann Owner", with no `@` or OID anywhere in the part;
  - the 13 keys are in both catalogs.

  Mutations, each red: `by` rendered; a key missing in `ru.ts`.
- [ ] NH14 — **[measure] Time and size.**
  - A seeded repository (`git fast-import`, invented names): one ~100 KB file of 200 sections, 1 000 commits, every fourth changing R-12. `spec show R-12 --history --json`, warm, in debug and release, scans 128 commits. The bar is release ≤ 1 s, the times recorded in "Implementation". Past the bar, "Known limits" names a daemon memo per (HEAD, path), then an index table.
  - `mcp_size` pins `get_node`'s maximum with all three `with` values on NH10's node.
  - After `pnpm build`, first-load JS grows by at most 1.5 kB and stays ≤ 845 kB, with `DocHistory` in `DocBody`'s chunk.
- [ ] NH15 — **[look]** The orchestrator's, in Chrome, both themes and languages, at 1440x900 and 1100x800:
  - the document modal and page of a node with a raise, an editorial fix, a hand commit, an accepted task and a decision, History collapsed on open and then expanded;
  - a document with no history;
  - the shallow-clone note on `spec serve` over a `--depth 1` clone.
- [ ] NH16 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

- Code-side history (markers, lock accepts) belongs to RM-0056.
- A section moved to another file, an ID renamed (its older entries end at a creation), and an alias's old history.
- Conflict resolution inside merges beyond git's default.
- A History tab in the Spec tree: the tree opens the document view.
- A memo or an index table, unless NH14 passes 1 s.
- Emails hidden from the CLI, MCP or the served JSON, and an event on a hand commit.

## Implementation

To be filled in after implementation. The plan follows.

### Roles and files

| Role | Files |
|---|---|
| rust-developer | `crates/specengine-core/src/{history,lib}.rs`; `crates/specengine-store/src/worktree.rs` (and `verify_git.rs` if the batch reader is shared); `crates/specengine-cli/src/{show,cap,main,lib}.rs` and a new `history.rs` (git and queue halves, notes); `crates/specengine-mcp/src/{read,mirror}.rs`; `crates/specengine-http/src/{args,app}.rs`; `plugin.json`; `scripts/deploy/.env.example` |
| test-engineer, Rust | `node_history.rs` in CLI and MCP (new), core `#[cfg(test)]`, NH9 (c)'s seam, `mcp_size.rs`, the churn, the plugin tests, http `daemon_keys.rs`, `query.rs`, `spec_ids_hub.rs`. Also `fixtures/daemon-keys.json` (`SPECENGINE_WRITE_DAEMON_KEYS=1`), and in the same change `daemonKeys.test.ts`'s two new `KEYS` sets, its names, the count 124, and `["ShownNode", "history"]` out of `AHEAD_OF_FIXTURE` |
| ui-developer | "The UI line": `provisional.ts`, `client.ts`, `http.ts`, the mock, `queries.ts`, `src/doc/{DocHistory.tsx,history.ts,DocBody.tsx}`, the Shell's `DocState`, `en.ts`, `ru.ts`, with NH7 and NH13. `["ShownNode", "history"]` stays in `AHEAD_OF_FIXTURE` until the fixture lands |
| test-engineer, UI | the UI gates, NH7's and NH13's mutations, NH14's first-load figure |

Comments cite `docs/features/node-history.md` "Acceptance criteria" with a criterion ID, or a canon heading, never "Data", which compaction drops (`doc_pointers`).

### Work order

1. Start after `card-context` ships: both edit `show` and `card` paths, MCP and the plugin version. It stays 0.40.0.
2. The ui-developer lands the UI line behind `AHEAD_OF_FIXTURE`, gates green. The rust-developer goes core, then store, then CLI, MCP, http.
3. The Rust test-engineer overlaps in order: NH2, then NH1, NH3-NH5 and NH8-NH10, then NH6 and NH11, then the fixture (NH12) and NH14. Then the Rust and UI reviews.
4. The orchestrator's look (NH15). Ship: the spec-writer ("Canon at ship").

`design-sweep` runs beside it in the same tree, sharing documents and `en.ts` and `ru.ts` in other groups; commits split by hunk. At most three iterations a lane.

### Risks

- **The judge's cost:** one parse per distinct blob, at most 129. NH14 measures it, and its follow-up is named.
- **`--follow`:** reading a parent by the new name instead of the log's old path gives a false creation (NH3).

### Canon at ship

Free bytes now in brackets; "net not positive" means trimmed in the same file.

| File | Edit |
|---|---|
| `docs/canon/spec-history.md` (new, Tier 2, `spec-*` shard) | the rule: sources, judge, trailer parse, entry keys, order, cap, scan, notes, text block, daemon, hub, the UI's names. Known limits: ID renames, moved sections, merges, stale after a hand commit until reopened, emails in the JSON, the measured cost. Tests |
| `docs/canon/spec-cli-read.md` [5 601] | "Output": the `show --history` block, a pointer; "The cap": `; history not shown: <k>` |
| `docs/canon/mcp-read.md` [12] | `get_node`'s row `[--history]`; "Schemas": the three `with` values; "Texts": 1 909; "Size": the maximum. Net not positive |
| `docs/canon/doc-view.md` [1 849] | "The view", Main: History after Inside; a History rule beside "Inside"; its "Reads" row; "Known limits": stale after a hand commit |
| `docs/canon/plugin-files.md` | **Version**: current 0.40.0, with `with`'s `history` |
| `docs/canon/hub-deploy.md` [12], `docs/canon/plugin.md` [316] | the version (net zero) |
| READMEs: core [9], store [3], cli [1], mcp [7], http [4] | a pointer to `spec-history.md`; http's `with` values. Net not positive |
| `docs/specs/specengine-platform/07-interfaces.md` | §1.2 and §2: `history` built |
| `CLAUDE.md`; 08 "Owner's word, 2026-11-12" row 2; RM-0035 | shipped |
| this spec | compacted to at most 3 KB |

### Working answers and the owner's questions

**The orchestrator's**, the analyst's recommendations; the owner has not answered:

| Question | Working answer | Cost of the other answer |
|---|---|---|
| Q-A. List hand commits, which carry no trailers? | Yes, marked "Edited outside a proposal" | History leaves out most changes in a repository that commits by hand, as this one does |
| Q-B. Show "Version N" openly, though v5 folds revisions away? | Yes; hashes stay out of the UI | Folding it hides the version his catalog asks for |
| Q-C. Emails on a hub? | Names only in the UI; CLI and MCP show the trailer as written | Every login sees everyone's email on the page. Stripping them from the served JSON as well gives the hub a shape of its own and breaks parity with `--json`. As it stands, a login can still read them in the network panel or over MCP |
| Q-D. Git missing or a shallow clone? | One `note: history:` line, the exit unchanged | Exit 2 fails every read that asks history on a CI clone, against ADR-0046 |

**The spec-writer's:**
- W1: the queue joins are in. "Who changed it" includes a task accepted and a decision taken on the node.
- W2: on a hub the git half runs inside the project's turn, as every read does. Moving it out needs a turn-free read path, and NH14 measures first.
- W3: the scan of 128 caps the blobs at 2 × 128, and the stop at creation ends most scans sooner.
- W4: one closed entry shape for the three sources keeps one MCP mirror and one UI table.
- W5: 300-character texts let 64 entries fit the 40 000 characters with room for the node's text.
- W6: NH1-NH7 are the analysis's NH-1 to NH-7, sharpened. NH1 counts the creating commit, so four entries, not three.
