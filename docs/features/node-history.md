---
class: spec
status: shipped
shipped: 2026-11-12
scope: [crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, plugin, scripts, fixtures, ui]
ref: RM-0035, v5 core slice 2 (08 s2 Phase 7, "Owner's word, 2026-11-12"); the owner's vision of 2026-11-11 (each item with its version, its fixes, who changed it and when); the analyst's analysis of 2026-11-12 (section 1, NH-1 to NH-7, here NH1 to NH7); the orchestrator's working answers, the owner's review open
adrs: []
---

# node-history: a node's changes, who made them and when, on the CLI, over MCP and in the document view (RM-0035)

## Why

`spec show` printed a node's text and `rev`, but not who changed it, when, through which proposal or why. The owner's vision of 2026-11-11 asks for each item with its version, its fixes, and who changed it and when (`08-roadmap.md` Phase 7). ADR-0005 promises "the decision history is readable from git", and every apply commit carries its trailers (`docs/canon/proposal-apply.md` "Apply steps"), yet nothing read them back.

Now `spec show R-12 --history`, `get_node {id, with: ["history"]}` and the document view's History part list a node's changes, newest first: the commits that changed its span (hand commits marked), the tasks accepted on it and the questions, discrepancies and decisions applied to it. Built per read from git and the queue, never stored, it feeds no rollup, mark or gate. No ADR: the slice rests on ADR-0005, ADR-0001, ADR-0046 (a layer that cannot be read is a note) and ADR-0012.

## Description and interactions

Each decision is canon now, in `docs/canon/spec-history.md`: D1 opt-in per node: "The rule"; D2 the commits that changed the span, D3 the trailers parsed in core, D4 the queue joins: "Sources"; D5 one list of at most 64: "Order and caps"; D6 raw on the CLI and MCP: "Entries"; D7 a note, never an exit: "Notes"; D8 plugin 0.40.0: "MCP, daemon, hub, plugin". D6's names in the UI and D9 the History part: `docs/canon/doc-view.md` "History part".

The owner's questions, open, their working answers built: **Q-A** hand commits are listed, "Edited outside a proposal"; **Q-B** "Version N" is shown openly, no hash in the UI; **Q-C** names only in the UI, the identity as written on the CLI, MCP and in the served JSON (a Known limit); **Q-D** git missing or a shallow clone is one note, the exit unchanged.

## Data

In the canon: `spec-history.md` "Entries", "Text and JSON", "MCP, daemon, hub, plugin"; `spec-cli-read.md` "Output", "The cap"; `doc-view.md` "History part", "Reads"; the key sets `ShownNode` (21 keys), `ShownHistory`, `HistoryEntry` (`fixtures/daemon-keys.json`, 124 sets).

## Rules and edge cases

In the canon: `spec-history.md` "The rule", "Sources", "Order and caps", "Notes", "Known limits".

## Acceptance criteria

A browser look is marked **[look]**, a timed or counted run **[measure]**. Every other criterion is a test that the named mutation turned red. The Rust tests build their repositories at run time with invented names and fixed 2026-01 dates; apply messages come from core's `commit_message_raising`, and one apply runs through `spec approve` end to end.

- [x] NH1 — **Only commits that changed the span.** `docs/rules.md` holds `{#R-12}` and `{#R-13}`; its commits are A, by hand, creating both; B, PR-0001 raising R-12 1 -> 2; C, PR-0002 editorial on R-12; D, by hand on R-12; E, by hand on R-13 only. `spec show R-12 --history --json` lists exactly D, C, B, A (`hand`, `editorial`, `raised`, `hand`), `created` on A alone, revs 2/2, 2/2, 2/1 and 1/null, `truncated` false; the text is the canon's block, byte for byte; `spec show docs/rules.md --history` lists E too. Mutation, red: every commit of the file listed.
- [x] NH2 — **The trailer block is the last paragraph.** B's rationale holds `Proposal: PR-9999`, yet its `proposal` is `PR-0001`. A hand commit ending in a `Co-Authored-By:` paragraph is `hand`, and so is a `spec: apply PR-0003` subject with no `Proposal:` trailer. Core unit tests cover `applied_trailers`, `identity_name` (`"Ann <a@b>"`, `"mira"`, `"x <y"`) and `proposer` (`role=unknown` gives `"agent"`). Mutation, red: every body line scanned.
- [x] NH3 — **Renames.** After `git mv docs/rules.md docs/law/rules.md` and one more edit of R-12, the history lists that edit and all of NH1's. Mutation, red: `--follow` dropped.
- [x] NH4 — **Not asked, no git.** With a `git` shim first on `PATH` that logs its arguments and runs git, `spec show R-12 --json` exits 0, `history` `null`, no `note: history:`, and no `log` or `cat-file` in the log. Mutation, red: the history always read.
- [x] NH5 — **No history to read.** In a `git clone --depth 1`, `spec show R-12 --history` exits 0 and prints the text, with `history: null` and exactly one `note: history: docs/rules.md: not read: a shallow clone`. With no `git` on `PATH`, the cause is git missing. Outside any repository, it is not a repository. Mutation, red: git's error passed through (exit 2).
- [x] NH6 — **MCP parity and schemas.** Stdio `get_node {id: "R-12", with: ["history"]}` content is byte-equal to `spec show R-12 --history 2>&1`, and `structuredContent` equals `--json`. The same holds with all three `with` values and in NH5's clone. The schema walker passes with exact key sets (`history`'s 3, the entry's 15), and the input enum has three values. A hub's `/mcp/<slug>` answers alike with 18 tools. Mutation, red: `rationale` dropped from `ShowHistoryEntry`.
- [x] NH7 — **The UI reads on expansion only.** Opening the document view sends no `with=history`. The first expansion sends one, and a collapse and a second expansion send none. With `historyOpen` held, it reads on open. Mutation, red: read on open.
- [x] NH8 — **The queue joins.** Rows written through the store at fixed `now`s: C-0001 (R-12, accepted), C-0002 (R-12, not accepted), C-0003 (R-13, accepted); PR-0040, a decision on R-12 applied with DEC-0031; PR-0041 open and PR-0042 rejected, both on R-12; PR-0001, an applied update of R-12. The list holds C-0001 `accepted` and PR-0040 `decision` (record DEC-0031), placed by `at`, none of the others, PR-0001 once, as B. An unreadable queue file gives the queue note and the commits. Mutations, each red: open items taken; an unaccepted card taken; the dedup dropped.
- [x] NH9 — **The cap and the scan.** (a) 70 commits changing R-12 give 64 entries, `truncated: true`; (b) R-12 created at the first commit, then 130 commits touching R-13 only: `[]`, `truncated: true`; (c) R-12 created at commit 3 of 5: `truncated` false, no blob of commits 1-2 asked (a test seam). Mutations, each red: no entry cap; an unbounded scan ((b) lists the creation); no stop at creation (c).
- [x] NH10 — **The text cap.** A node with 64 entries, each with a 300-character rationale, and a long text: the text is cut first. When bindings and history pass the cap, the cut falls after a whole entry. The tail ends `; history not shown: <k>`, and the JSON holds the printed entries with `omitted` k, within 40 000 characters plus the tail. The daemon serves all 64, `omitted` 0. Mutation, red: a cut inside an entry.
- [x] NH11 — **Plugin 0.40.0 and the texts.** `plugin.json` and `.env.example` hold 0.40.0, and `tag-check.sh v0.40.0` prints it. `PINS` and `SCHEMA_PINS` end with 0.40.0. `NODE_DESCRIPTION` is 1 909 B and the hub's 2 000 B, each with the sentence once. `INSTRUCTIONS` (stdio and hub) and `read-spec` are byte-equal to before. Mutations, each red: the schema changed with no `SCHEMA_PINS` entry; `HISTORY_MAX` 65 (the build fails).
- [x] NH12 — **The daemon serves it, the UI types it.** `GET …/nodes/R-12?with=history` serves NH1's entries. `with=bogus` is a 400 naming the three. The fixture names 124 sets, `ShownNode` 21 keys. `provisional.ts` types the three shapes. UI lint has 0 problems, `tsc` is clean, and `pnpm test` is green, `daemonKeys.test.ts` included, `AHEAD_OF_FIXTURE` empty at the end. Mutations, each red: `history` out of the fixture; `with=history` refused.
- [x] NH13 — **The History part.** On the mock's sample the labels read "Version 2", "Wording fixed, version 2 kept" and "Edited outside a proposal", a created entry is marked; C-0001 links `#/<p>/board/C-0001`, PR-0040's record is a `DocLink`; an unknown `change` is raw `lang="en"`; `truncated`, `null` and `[]` each show their line; `by` `Ann Owner <ann@example.org>` renders "Ann Owner", no `@` or OID anywhere in the part; the keys are in both catalogs (13 asked, 18 built). Mutations, each red: `by` rendered; a key missing in `ru.ts`.
- [x] NH14 — **[measure] Time and size.** A `git fast-import` seed, one ~100 KB file of 200 sections, 1 000 commits, every fourth changing R-12: `spec show R-12 --history --json`, warm, scans 128 commits, release ≤ 1 s, the times in "Implementation" (past the bar, a daemon memo per (HEAD, path), then an index table). `mcp_size` pins `get_node`'s maximum with all three `with` values on NH10's node. First-load JS grows by at most 1.5 kB and stays ≤ 845 kB, `DocHistory` in `DocBody`'s chunk.
- [x] NH15 — **[look]** The orchestrator's, in Chrome, both themes and languages, at 1440x900 and 1100x800: the modal and page of a node with a raise, an editorial fix, a hand commit, an accepted task and a decision, History collapsed on open, then expanded; a document with no history; the shallow-clone note on `spec serve` over a `--depth 1` clone. Done in part: "Implementation".
- [x] NH16 — **Gates.** `cargo nextest run` for `specengine-core`, `-store`, `-cli`, `-mcp` and `-http` is green, then the full run once. The eval tests (`doc_pointers`, `ui_policy`, `anonymity`) and `spec check` pass. UI lint, `tsc`, `pnpm test` and `pnpm build` pass.

## Out of scope

Code-side history (markers, lock accepts: RM-0056); a section moved to another file, an ID renamed, an alias's old history; conflict resolution inside merges beyond git's default; a History tab in the Spec tree (the tree opens the document view); a memo or an index table (NH14 passed); emails hidden from the CLI, MCP or the served JSON; an event on a hand commit.

## Implementation

The Rust review REWORK and the UI review ACCEPT (five minors), each settled in its iteration 2; the orchestrator's genre finding, an ADR number in `history.rs`' module doc, a one-line fix. NH16 is the owner's full run, ticked on his word.

| Module | Does |
|---|---|
| core `history.rs` (new) | the caps, the trailer parse, `identity_name`, `proposer`, `judge` |
| store `worktree.rs`, `git.rs` | `WorktreeGit::{is_shallow, file_log, blob_batch}`, the shared `cat-file` session |
| cli `history.rs` (new), `show.rs`, `cap.rs`, `main.rs`, `lib.rs` | `add_history`; `--history`, the block, the cut, the tail; `NODE_HISTORY_MAX` |
| mcp `read.rs`, `mirror.rs`; http `args.rs`, `app.rs` | `With::History`, the texts, the mirror; `WITH_VALUES` |
| `plugin.json`, `.env.example` | 0.40.0 |
| ui `api/`, `doc/{DocHistory.tsx,history.ts}` (new), `app/Shell.tsx`, `i18n/`, `mocks/history.ts` | the types, `useNodeHistory`; the part, the change words, `DocState.historyOpen`; 18 keys; the mock |

**Rust departures**, each canon now (`spec-history.md`): (1) blobs by OID from `log --raw`, a partial clone's missing blob told from an absent path; (2) more `log` flags, `--diff-merges=first-parent` needing git 2.31 (the code needs 2.36), an LF-split format since iteration 2; (3) `NODE_HISTORY_MAX`, core's `card::HISTORY_MAX` taken; (4) a stop at a version lacking the node; (5) "the last commit read" is the 128th; (6) an ID-less node meets cards by path; (7) `WITH_DOC`'s backticks; (8) the queue opened as it is; (9) a parser panic a note.

**UI departures** (`doc-view.md` "History part"): (1) 18 keys, not 13: `reading`, `failed`, three `…Plain` for no record; (2) `applied` "Version N", also with `rev_before` equal to `rev`, a `null` `rev` the raw word; (3) Created on any commit with `created`; (4) no `proposer`: the link alone; (5) `history: ` notes only, beside a list too; (6) `KnownHistoryChange` an interface's keys (`kinds.test`); (7) the mock's own IDs; (8) empty and truncated: that line alone. Dropped: a `by_name` cut at `@`.

**Findings.** Rust: text lines through `one_line` only, an ESC passing (major): each line now escaped, the cap counting it, JSON raw; a U+001F header split (minor): LF framing; the nits fixed (the subject, an empty `by` `-`, dead code, one-line errors); a branch's change shown twice: a Known limit; its eight probes passed. UI: the five minors fixed: the `@` cut, `…Plain`, (4), (8), a per-project `DocState` test.

**Numbers.** `node_history.rs` CLI 17, core 9, MCP 5, http 2, green; 21 of the test-engineer's 22 mutations red, one an equivalent mutant; the UI's nine red; UI 183 files, 5 258 tests, green twice; eval 45 of 45. NH14: release 0.169 s median, debug 1.33 s (1 000 commits, a 103 KB file); `NODE_HISTORY_MAXIMUM` 110 216; first load 825.02 kB (+1.14), `DocBody-*.js` 17.62 kB, `ru-*.js` 153.31 kB.

**NH15**, 2026-11-12: a full clone of this repository (287 commits, hand commits alone) on the debug daemon, light, Russian, 1440x900, a document page: History collapsed after Inside, one read on expansion, newest first. Not looked at in a browser: an apply, a task, a decision, no history, the shallow-clone note, the modal, dark, English, 1100x800; the smoke test and the part's tests cover them.

**Gaps**: the parser-panic note untested; the hub's call in-process only; applies in the UI seen only in tests and the mock; the store's new methods tested through the CLI alone. **Known limits**: `spec-history.md`, `doc-view.md`.
