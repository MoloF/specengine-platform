---
class: canon
tier: 2
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, ui]
owner: owner
reviewed: 2026-11-12
---

# spec show --history: a node's changes, who made them and when

Built by `node-history` (RM-0035, plugin 0.40.0) on accepted decisions alone: the decision history is readable from git (ADR-0005), files are the truth (ADR-0001), a layer that cannot be read is a note (ADR-0046), nothing is blocked (ADR-0012). Built per read from git and the queue, never stored; no rollup, mark or gate reads it. Related: `spec-cli-read.md` (the cap), `proposal-apply.md` "Apply steps" (the trailers), `mcp-read.md`, `doc-view.md` "History part". Code: core `history.rs`; store `worktree.rs` (`WorktreeGit::{is_shallow, file_log, blob_batch}`); CLI `history.rs` (`add_history`), `show.rs`, `cap.rs`; MCP `read.rs`, `mirror.rs`; http `args.rs`, `app.rs`.

## The rule

`spec show REF [--links] [--bindings] [--history] [--archive]`, MCP `get_node {id, with: ["history"]}`, the daemon's `GET …/nodes/<REF>?with=history`. Opt-in: unasked, each node's `history` is `null` and the read runs no git command and reads no queue row for it. Asked, each shown node's `history` is `{entries, truncated, omitted}`, or `null` with a note; exits are unchanged. Writes: the index's own refresh alone; no queue row, no event, nothing under the root. One corpus, git and queue state gives one text and one JSON, byte-equal across roots and `HOME`s: every date is stored data.

## Sources

- **Commits that changed the span.** Per holder file, one `git log --follow --raw --diff-merges=first-parent` of `HEAD`, at most `HISTORY_SCAN_MAX` + 1 (129) commits, newest first, format `%H%n%an%n%ae%n%ct%n%B` split on LF; a rename's old path from the raw record. Each commit's blob and its first parent's come from one `cat-file --batch` session by OID, each once, opened at the first needed. Git runs with `GIT_NO_LAZY_FETCH=1`: a partial clone never fetches. Core parses both versions with `spec show`'s parse and lists the commit only when the node's span bytes differ (`judge`): a section by its ID, an ID-less node or a path REF by its whole file; revisions as `node-revision.md` "Revision". The scan stops at the node's creation (absent in the parent, or a root commit: `created`), at a version that lacks the node (a deletion, or not yet committed: nothing listed), or at the file's first commit.
- **Trailers**, parsed in core from the raw message, never with git's trailer options: an apply's subject is `spec: apply <ID>` and its last paragraph, after a blank line, holds `Proposal: <the same ID>`. Only that paragraph is read, so a `Proposal:` in the rationale never counts. Its `Revision: <ID> a -> b` gives `raised`, `<ID> n (editorial)` `editorial`, none for the node `applied`. Any other commit is `hand`. The rationale is the text between the subject's blank line and the trailers.
- **Cards** with `acceptance` set and a target equal to the node's ID or a section's ID inside it; an ID-less document by its path, as a card stores it.
- **Items** of this repository (`git_common_dir`): applied questions, discrepancies and decisions whose `node_ids` meet those IDs. One a listed commit's `Proposal:` names is not listed again.

The queue is opened as it is, no schema step, nothing written: none, or schema 0, joins nothing and says nothing; a schema below `QUEUE_SCHEMA_VERSION` gives the queue note, its why `the queue stands at schema N; a queue command upgrades it`; a row that does not decode is skipped silently.

## Entries

Fifteen keys, in this order, every one present:

| Key | commit | card | item |
|---|---|---|---|
| `source` | `commit` | `card` | `item` |
| `at`, UTC (`utc_timestamp`) | committer time | `acceptance.at` | `decided_at`, else `updated_at` |
| `id` | the full OID | its ID | its ID |
| `change` | `raised`, `editorial`, `applied`, `hand` | `accepted` | `question`, `discrepancy`, `decision` |
| `created` | absent in the parent, or a root commit | `false` | `false` |
| `rev`, `rev_before` | after, before (`null` when created) | `null` | `null` |
| `by` | `Decided-by` as written, else the author `Name <email>` | `acceptance.by` | `decided_by`, `""` when none |
| `by_name` | `identity_name(by)`: the part before a final ` <…>`, else the whole | the same | the same |
| `proposal`, `proposed_by`, `proposer` | the trailers as written, `proposer(…)`; `null` on a hand commit | `null` | `null` |
| `record` | `null` | `null` | `record_id` |
| `title` | a hand commit's subject, else `null` | its title | `null` |
| `rationale` | an apply's, else `null` | `null` | `null` |

`proposer`: `agent role=spec-writer model=m run=r` gives `agent spec-writer`, an `unknown` or absent role the type alone. A subject is git's `%s`: the first paragraph, its lines joined by one space. `title` and `rationale` go through `one_line`, cut at `HISTORY_TEXT_MAX` (300) characters on a character boundary, then `…`.

## Order and caps

Newest first by `at`; a tie puts commits (in log order) before cards before items, each by ID. At most `HISTORY_MAX` (64) entries: the rest drop and `truncated` is set. A scan reaching the 129th commit sets `truncated` and drops queue entries older than the 128th, the last judged. Core's `card::HISTORY_MAX` is a card's events: the CLI re-exports this one as `NODE_HISTORY_MAX`.

## Text and JSON

The block follows the bindings block (else links, else the header line), before the node's text:

```text
  2026-01-04T10:00:00Z | hand | rev 2 | - | Bob Hand <bob@example.org> | - | 4f1c2ab3d5e6
    Fix a typo in the window rule
  2026-01-02T10:00:00Z | raised | rev 1 -> 2 | PR-0001 | Ann Owner <ann@example.org> | agent role=spec-writer model=m run=r | 1a2b3c4d5e6f
    Tighten the window.
  2026-01-01T12:00:00Z | accepted | C-0001 | Ann Owner <ann@example.org> | Open the window
  2026-01-01T12:00:00Z | decision | PR-0040 | DEC-0031 | Ann Owner <ann@example.org>
  2026-01-01T10:00:00Z | hand created | rev 1 | - | Bob Hand <bob@example.org> | - | 0f9e8d7c6b5a
    Add the rules
  history 5
```

- Commit: `<at> | <change>[ created] | rev <r> | <proposal or -> | <by> | <proposed_by or -> | <OID's first 12 hex>`, `<r>` `<before> -> <after>` when both are known and differ, else `<after>`, else `-`; then `    <rationale or title>` when set. Card: `<at> | accepted | <id> | <by> | <title>`. Item: `<at> | <kind> | <id> | <record or -> | <by>`. `-` stands for `null` and for an empty `by`.
- Each line goes through `escape_controls`, so the cap counts what prints; the JSON keeps the values raw.
- Closing: `  history <n>[, truncated]`. The block counts toward the cap with the links and bindings blocks, one entry (its line and the line under it) one unit; a cut in it falls after the last whole entry that fits, and the text is not shown. The tail gains `; history not shown: <k>` after its bindings part whenever any node's `history` is not `null`, `0` when none of it was cut. JSON takes the text's cut: `entries` the printed ones, `omitted` k on the cut node, 0 on the others. The daemon (`View::Browser`) never cuts.
- Library: `ShowRequest.history: bool`, `ShownNode.history: Option<ShownHistory>` after `bindings`; `ShownHistory`, `HistoryEntry` exported.

## Notes

Each prints as a stderr `note: ` line, in JSON `notes` without that prefix; the holder's nodes get `history: null`.

| When | Note |
|---|---|
| git cannot run | `history: <path>: not read: git missing` |
| the root is in no repository | `…: not read: not a repository` |
| `rev-parse --is-shallow-repository`, once per read | `…: not read: a shallow clone` |
| `cat-file` answers `missing` for an OID | `…: not read: a blob is missing (a partial clone?)` |
| the spec parser panics on an old blob | `…: not read: the spec parser failed on its blob <12 hex>` |
| another git failure | `…: not read: <git's first non-empty stderr line>` |
| the queue cannot be read, and some node's history was | `history: the queue not read: <why>; commits only` |

An unborn `HEAD` gives no commit entry and no note; an uncommitted file or edit shows no commit entry.

## MCP, daemon, hub, plugin

- `get_node`: `With::History`, the inlined enum `["links", "bindings", "history"]`; `WITH_DOC` ends ``; `["history"]`: each node's changes, newest first.`` `node_body!`'s head is `spec show REF [--links] [--bindings] [--history] [--archive]`, and one sentence before `archive:` says what `with: ["history"]` lists, its "at most 64" asserted against `NODE_HISTORY_MAX`: stdio 1 909 B, the hub 2 000 B. Mirror: `ShowNode.history: Option<ShowHistory>`, `ShowHistory {entries, truncated, omitted}`, `ShowHistoryEntry` the 15 keys, `HistorySource` and `HistoryChange` closed. `INSTRUCTIONS`, `HUB_INSTRUCTIONS`, `HUB_TOOLS` (18) and `read-spec` are unchanged.
- The daemon: `args.rs` `WITH_VALUES` holds the three; another value is a 400 `` `with=<v>`: the values are `links`, `bindings` and `history` `` before any call; `nodes` passes `history`. `ShownNode` has 21 keys; `ShownHistory`, `HistoryEntry` are key sets (`fixtures/daemon-keys.json`, 124 sets).
- A hub serves the same route and tool, the git half inside the project's turn as every read (`hub-mcp.md` "Turn"); its `--push` clone is neither shallow nor partial (`hub-git.md`), and it reads the hub's queue. It takes `history` at its next upgrade, the owner's call; an older server refuses the value through its enum.
- Plugin 0.40.0 (`plugin-files.md` "Changing it").

## Cost

NH14 on a seeded repository (`git fast-import`, invented names): 1 000 commits, one 103 KB file of 200 sections, every fourth commit changing R-12; `spec show R-12 --history --json`, warm, scans 128 commits: release 0.169 s median, debug 1.33 s. On the developer's 90 KB seed `git log` alone took 0.02 s of 1.55 s debug: the cost is parsing, one parse per distinct blob, at most 2 × 128. Under the 1 s bar, so no memo; past it, a daemon memo per (`HEAD`, path), then an index table. `get_node` with all three `with` values: at most 110 216 characters, text and JSON (`mcp_size`'s `NODE_HISTORY_MAXIMUM`).

## Known limits

- **A change made on a branch shows twice**: the branch commit, and the merge judged against its first parent, both `hand`. Which merges show at all is git's default simplification.
- **An open History goes stale**: `doc-view.md` "Known limits".
- **Emails**: `by` is the identity as written, so the CLI, MCP and the served JSON carry emails; the UI draws `by_name` as served, even when that name is an email, and a hub login can read the JSON (the owner's Q-C, open).
- **A subject is the first paragraph**, joined: a hand commit whose first paragraph wraps shows it on one line.
- **An empty `by`** (an item with no decider) prints `-`, its JSON `""`.
- **The tail** holds `; history not shown: 0` whenever any history was read, as links and bindings do.
- **Text lines are escaped and JSON is raw**: the two differ on a message holding control characters.
- **IDs and moves**: a renamed ID's older entries end at a creation, a section moved to another file starts anew, an alias's old history is not followed. Code-side history (markers, lock accepts) is RM-0056's.

## Tests

`node_history.rs` in CLI (17: NH1-NH5, NH8-NH10, a merge, escaping, a U+001F author, a wrapped subject, an item with no decider; a `git` shim, `git fast-import` seeds), core (9), MCP (5: NH6, NH11, the hub) and http (2: NH12, the uncut daemon); `mcp_size`, `daemon_keys.rs`, `query.rs`, `plugin_files.rs`; store `worktree_git`. UI: `doc/DocHistory.test.tsx`, `doc/history.test.ts`, `mocks/history.smoke.test.tsx`, `api/history.live.test.tsx`, `daemonKeys.test.ts`. Gaps: the parser-panic note is untested; the hub's call runs in-process, not over HTTP; the store's `file_log` and `blob_batch` have no store test of their own, only the CLI's; an apply, a task or an item in the History part is seen only in tests and the mock (the 2026-11-12 look's repository holds hand commits alone).
