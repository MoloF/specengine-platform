---
class: spec
status: shipped
shipped: 2026-11-22
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0136; the owner's request of 2026-11-21 and his decisions of 2026-11-22 (08 s2 Phase 7, "Owner's word, 2026-11-21/22"); the analyst's breakdown of 2026-11-22, slice 1 of 3, every working answer decided on his "start and go to the end"; plugin 0.50.0
adrs: [ADR-0087]
---

# comments: talk under cards, nodes and proposals, as queue data (RM-0136)

## Why

People discuss their tasks outside SpecEngine, in chats and calls. What they settle there rarely becomes a record, and the agent that executes the task never sees the discussion. The owner asked for comments and notes under tasks "like Jira", so that SpecEngine becomes the one place where work is run and discussed (2026-11-21).

He accepted one constraint with it: decisions must not drown in chatter. A comment is therefore never truth. It lives in the queue beside the card, node or proposal it discusses, reaches the executing agent as marked context, and becomes a question, a decision or a discrepancy only when someone turns it into one, which a person then decides through the one writing door (ADR-0004). This is slice 1 of three: `comments` (0.50.0), then `work-references` (RM-0137, uploads) and `reference-promotion` (RM-0138, references into git).

Canon: `docs/canon/comments.md` (the rule, threads, authors, delete, mentions, items, agents' context, store, limits) and `docs/canon/comments-surfaces.md` (documents, CLI, MCP, HTTP, events, the screen). Decision: ADR-0087. The decisions table (D1-D14) and the full formats are in git history.

## Data

Every format moved into the canon; each part here names where.

### Queue schema 15

`STEP_15`: `comments` (15 columns), `comment_versions` (5), `proposals.comment_id` (`PROPOSAL_COLUMNS` 51), indexes `comments_on` and `proposals_comment`; `STATE_FORMAT` 8. `comments.md` "Store and backup"; `proposal-queue.md` "Store"; `queue-backup.md` "Format".

### The comment document

13 keys, a list `{on, total, comments, next_before, notes}`, mentions `{login, comments, next_before, notes}`, the text forms: `comments-surfaces.md` "Documents".

### The card context and the task package

`context.comments:` and `context.comment:` lines in `U`, outside the bundle, the newest 20 within 8 192 characters, each body cut at 1 024 then escaped; JSON entry `{id, reply_to, actor, author, created_at, updated_at, version, body, cut}`, the package's adding `on`; the brief's "Comments": `comments.md` "Agents' context", `card-context.md`, `task-package.md`.

### MCP

`list_comments`, `add_comment` on stdio (35 tools) and a hub (21, 8 trusted reads); `comment_id` on the three intake tools: `comments-surfaces.md` "MCP", `mcp-read.md`, `hub-mcp.md`.

### HTTP

Two trusted reads and four writes, their statuses and events: `comments-surfaces.md` "HTTP"; the fence: `hub-team.md` "Acts".

### Mentions grammar

`comments.md` "Mentions".

### UI

The Comments part, Turn into…, the Mentions view and badge, live re-reads, the lazy chunks: `comments-surfaces.md` "Screen".

## Acceptance criteria

Each criterion names the mutation that must turn it red (M).

- [x] CM1 — Schema step: a schema-14 queue holding proposals, cards and events steps to 15 in one transaction. `PRAGMA user_version` reads 15, both tables exist, and every `comment_id` is NULL. `COMMENT_COLUMNS`, `COMMENT_VERSION_COLUMNS` and `PROPOSAL_COLUMNS` (51) equal `PRAGMA table_info`. A failure inside the step leaves the queue at 14. M: a column dropped from `STEP_15`.
- [x] CM2 — Targets: `spec comment add --on X` stores the canonical ID of an ID, a section, or `slug/ID`. An alias exits 1 naming the canonical ID. A look-alike exits 2 with its Latin form. `C-0099` and `PR-0099` exit 1. None of them stores a row or an event. A node's `on_rev` equals its revision in the index. M: the card existence check skipped.
- [x] CM3 — Replies: a reply naming a top-level comment on the same target is stored. A reply to a reply, or to another target's comment, exits 1 (409 on the route), nothing stored. M: a reply to another target's comment accepted.
- [x] CM4 — Authors: on a hub, `POST .../comments` stores `actor` and `author.login` as the login, a body `as` naming anyone else a 403 storing nothing; over the hub's `/mcp`, `add_comment` stores `{type: agent, role, model, run, login}` with `actor` the login; over stdio the same without `login`, `actor` `agent`; on a terminal `--as N` or `local`. M: `as` read from the body on a hub.
- [x] CM5 — Edits: an edit at the stored version stores version n + 1 and puts the old body, with its time, in `comment_versions`; a stale `--version` exits 1, or gets 409, nothing changed; an edit by another login gets 403, an `owner` included; an agent's comment is refused. M: an overwrite without a version row.
- [x] CM6 — Delete rights, on a hub with `[team]` naming an owner, a lead and a member: the author's login and the `owner` get 200; the lead, the member and a login outside `[team]` get 403 with the line; with no `[team]`, or a bad one, only the author; `spec comment delete` with stdin not a terminal exits 2 and a declined prompt exits 1, both with no event; `tools/list` holds no delete or edit tool, stdio or hub. M: any `[team]` login may delete; the author check dropped; a delete tool added (`HUB_TOOLS` pin).
- [x] CM7 — Erasure: write a comment whose two versions each hold a unique marker, delete it, then run `PRAGMA wal_checkpoint(TRUNCATE)`. Neither marker occurs in the database file, the WAL or `spec export state`'s output (a byte search). The row keeps `deleted_by`, `deleted_at` and `version`. M: `secure_delete` left off (on the edit too, after the review).
- [x] CM8 — Turn into an item: an intake with `--comment CM-0012` (or `comment_id`, or `POST .../comments/CM-0012/items`) stores `comment_id`; approve, reject, stage, unstage and reopen never change it; the comment document's `items` lists the item; a card comment's item stores `card_id` too; a deleted or absent comment exits 1, nothing stored. M: the column not written.
- [x] CM9 — Nothing blocks: with 100 comments on a card, its move, accept, cancel and clear give the same documents and events as with none. `change_card`, `card_accept` and `approve` execute no statement on `comments` (a statement trace). M: `change_card` reads comments.
- [x] CM10 — Card context: on a card with 25 comments, `spec card show --context` and `get_card {context: true}` list the newest 20 that fit 8 192 characters, oldest first, with the note for the rest. A 3 000-character body shows cut at 1 024 with ` [cut]`. `U` grows by the lines' characters + 1. `bundle_hash` equals the hash with no comments. A plain `show` executes no statement on `comments`. M: comments folded into the bundle, or left out of `U`.
- [x] CM11 — Task package: `get_task` on a task listed by two cards carries both cards' comments under the same bounds, each naming its card; `schema_version` stays 1. M: the key dropped.
- [x] CM12 — Events and feed: each comment write adds exactly one `comment.*` event whose payload holds no body or mention; `git status`, `HEAD` and the files are unchanged; `spec feed` prints a word line for each of the three types. M: the body in a payload.
- [x] CM13 — Backup: a queue with live, edited and deleted comments dumps as format 8, imports into an empty data directory, and dumps again byte-identical; a format 7 fixture dump imports with no comments; `docs/generated/queue.md` is byte-identical with and without comments. M: `comment_versions` left out of the dump.
- [x] CM14 — Hub fence: every new write route is listed in `hub.rs` `class`, and an unlisted method on them gets 403. `GET_ROUTES` pins 39. The local fence refuses a cross-site POST. M: an unlisted route.
- [x] CM15 — MCP texts and schemas: every hub text is ASCII and at most 2 048 B. `list_comments`' content is byte-equal to `spec comment list … 2>&1` on stdio and on a hub. No tool schema holds `as`, `login` or `actor`. `HUB_TOOLS` is 21; stdio has 35 tools. M: `as` added to `add_comment`.
- [x] CM16 — Body cap: a body of 8 193 bytes, a blank one, or one holding U+0000 or 17 distinct mentions is refused (400, or exit 1) with its line. 8 192 bytes are stored byte for byte. M: the cap check removed.
- [x] CM17 — Mention grammar: `x@mira`, `` `@mira` ``, `Bob@mira.org` and `@Mira` mention nobody; `@mira.` mentions `mira`. `GET .../mentions` on a hub answers the asking login's mentions, newest first. An edit removing a mention removes the row from them. M: the boundary rule dropped.
- [x] CM18 — UI: a body renders only through `ui/src/markdown/` (`policy.test.ts`); Delete shows `ActOff` on another's comment when `allowed` lacks `delete_comment`; a 409 on edit keeps the draft; Turn into… prefills `node_ids` per target; the Mentions badge counts unseen `id@version` rows; the first load stays under the bar. M: `dangerouslySetInnerHTML` in the Comments part.
- [x] CM19 — Soak "CM" (`write_soak.rs`, `WRITE_SOAK_VARIANT=CM`): 10 logins over 2 000 pre-filled comments on 20 cards; 8 `add_comment` writers over the hub's `/mcp`, beside readers of `get_card {context}`, `GET .../comments` and `GET .../mentions`; two runs of 300 s; live heap at most 0.25 MiB/min (Theil-Sen, last two thirds) and quiet footprint at most 5 MiB/min; every call answers as expected; no hang; the row in `soak-runs.md` "Budgets". M: a planted per-write leak (2 KiB kept per `add_comment`). Passed 2026-11-22: "Release".
- [x] CM20 — Plugin version: `plugin.json` 0.50.0, with `PINS` and `SCHEMA_PINS` appended. M: the schemas change while the version stays 0.49.3 (`tool_schemas_follow_the_plugin_version`).
- [ ] CM21 — Owner's gate: on the live hub, upgraded after a dump (`hub-ops.md` "Upgrade"), a comment and a mention appear under his login, and the mentioned login's badge lights. It blocks nothing (ADR-0012).

## Out of scope

Watchers, notifications, reactions, sprints, time tracking, custom fields; comments in `get_node`, the bundle, search, the `cited` scan, `spec history` or `spec delivery`; a version-history view; an owner editing others' words, any edit or delete tool over MCP; uploads and references (RM-0137, RM-0138); merging a terminal's comments with a hub's; validating a mention; replacing a card's `evidence`.

## Implementation

| Module | What it does |
|---|---|
| core `comment.rs`, `team_toml.rs` | `CM-` IDs, look-alikes and the `[ids]` clash; `CommentOn`; the caps; `mentions`, `body_problem`, `context_body`; `raised_discrepancy`; `Role::deletes_comments` and `delete_comment` in an owner's `allowed` |
| model `task.rs` | `PackageComments`: the task package's `comments` |
| store `queue.rs`, `queue/state.rs`, `queue/comments.rs` | `STEP_15` and schema 15, `PROPOSAL_COLUMNS` 51, `comment_id` on intakes; add, edit, delete with `erasing` and `checkpoint_now`, get, list, mentions, the card context, each read stepped; the three events |
| cli `comment.rs` | `spec comment add/list/edit/delete/mentions`; the target resolve and its statuses; the delete's judge; the raise; the context lines |
| cli `card.rs`, `package.rs`, `propose.rs`, `intake.rs`, `proposals.rs`, `feed.rs`, `state.rs`, `state_file.rs` | the discrepancy kind (`raise_shape`), comments in `show --context` and `U`; the package's `comments` and the brief's "Comments"; `Unresolved`'s other form and several nodes; `--comment`; `comment_id` in review; the feed's words; `STATE_FORMAT` 8 |
| mcp `comments.rs`, `server.rs`, `hub.rs`, `intake.rs`, `cards.rs`, `tasks.rs`, `mirror.rs` | the two tools, `comment_id`, the texts and their cuts, `HUB_TOOLS` 21, `HUB_READ_TOOLS` 8, the mirrors |
| http `app.rs`, `answer.rs`, `hub.rs` | the six routes, their statuses, the fence's class |
| eval `write_soak.rs` | the `CM` variant, read against `soak-runs.md` "Budgets" |
| ui `src/comments/`, `api/commentsHttp.ts`, `commentQueries.ts`, `queries.ts`, `app/Shell.tsx`, `routes.ts`, `i18n/enComments.ts`, board, doc, inbox | the Comments part, Turn into…, Mentions and its badge, the live re-reads, the lazy English part, the Agent tasks view lazy |
| plugin, `scripts/deploy/.env.example` | 0.50.0; `read-spec` names `list_comments`, `ask-owner` `add_comment` and `comment_id` |

**Canon:** `comments.md`, `comments-surfaces.md` (new); `architecture.md` (`#storage`, `#ui`), `hub-team.md`, `hub.md`, `hub-mcp.md`, `hub-ops.md`; `proposal-queue.md`, `proposal-review.md` (with `diff_hunks`, moved there), `proposal-decision.md`, `queue-backup.md`, `queue-export.md` (the import steps, moved there), `agent-intake.md`; `cards.md`, `card-questions.md`, `card-context.md`, `task-package.md`; `mcp-read.md`, `spec-read-freshness.md`, `serve.md`, `spec-feed.md`; `plugin.md`, `plugin-files.md`; `ui-languages.md`, `ui-design.md`, `ui-client.md` (with the browser view, moved there), `board-ui.md`, `doc-view.md`; `soak-runs.md`; the guide's `cards.md`; the READMEs of core, model, store, cli, mcp, http and `scripts/deploy/`; 07, 08.

**Deviations:**
1. `secure_delete` is on for every comment write, not the delete alone: an edit with it off left version 1 in freed pages (the review's major).
2. A delete's commit is followed by a best-effort `wal_checkpoint(TRUNCATE)`, `busy_timeout` 0 for that call.
3. The delete's judge runs in the library call, in the project's turn, not in the handler.
4. A node REF in another form, or naming several nodes, is a 409 (`a comment names one node`), any other miss a 404; body caps are judged before a node resolves.
5. `STEP_15` also creates `proposals_comment (project, comment_id)`.
6. Context and package bodies are escaped in JSON too, cut before escaping; `list_comments`' `structuredContent` keeps raw Markdown for the UI.
7. The texts' cuts to fit 2 048 B (`mcp-read.md`, `hub-mcp.md`); the Agent tasks view and the English comment strings went lazy to keep first load under the bar.
8. "No byte remains" was softened into known limits: a `rebuildPage` fragment until the page is rewritten, `-wal` frames, older dumps and snapshots, an item's prefilled text (`comments.md` "Known limits").
9. The second canon page is `comments-surfaces.md`, not `comments-ui.md`: the shard had room for one line.

## Release

Plugin 0.50.0 (MINOR), tag `v0.50.0`; queue schema 15 and `STATE_FORMAT` 8, `INDEX_FORMAT` unchanged at 21; the hub's upgrade takes a dump first and every binary from the tag (`hub-ops.md` "Upgrade"). First load 815 001 B (838 738 before; bar 845 kB).

- The full run: 6 643 tests, 6 641 passed, 2 failed, both doc-reading pins caught at the ship: `crafted_yaml` `max_depth_is_the_cap_the_spec_names` (a core README line compacted, restored verbatim) and `doc_pointers` (a test comment citing bold text, "Worst W", as a heading, now "Findings, debt, verdict"); the 213 doc-reading and affected tests then green. `cargo clippy --all-targets -D warnings` and `cargo fmt --check` clean; UI 255 files, 6 597 tests; first load 815 001 B; `spec check` clean.
- Iterations: three. Rust review CHANGES, the erasure fixed, re-review ACCEPT; UI review ACCEPT with minors, fixed.
- CM19, two 300 s runs, 8 `add_comment` writers and 10 readers over 2 000 comments on 20 cards by 10 logins, both green:
  - run 1: 4 459.0 calls/min (1 981.6 writes, 2 477.4 reads), 22 295 of 22 295 answered; live heap 0.0, quiet footprint −1.416 MiB/min; RSS 5.2 → 81.1 MiB; 0 hangs; p50/p95/worst 214/438/641 ms;
  - run 2: 4 406.2 (1 957.4, 2 448.8), 22 031 of 22 031; 0.0, −2.963; 5.2 → 80.2 MiB; 0 hangs; 220/403/620 ms;
  - the planted leak (2 KiB kept per `add_comment`): red, live heap 4.396 MiB/min (> 0.25); the quiet-footprint judge alone read 1.863, under its 5, so the live heap is the judge that catches a steady leak.
- Open for the owner: CM21 on the live hub; the laptop's reinstall from the tag.
