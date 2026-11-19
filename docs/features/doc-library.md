---
class: spec
status: shipped
shipped: 2026-11-19
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-http, crates/specengine-import, crates/specengine-eval, scripts, ui]
ref: RM-0127; the owner's word of 2026-11-19, wish 1, and his language decision (08 s2 Phase 7, "Owner's word, 2026-11-19"); the analyst's breakdown, slice 1, whose working answers await the owner's review (free tags, class starting at canon, the department's module as the lens, behind by commit time, no MCP tool); his word of 2026-11-15 on a team of up to 10
adrs: []
---

# doc-library: every document as a library, with its decisions and tags (RM-0127)

## Why

The owner (2026-11-19) wants "how it works now" shown as documents, with classes and tags, each with the decisions that changed it. His corpora already hold this: canon pages with `rev`, `reviewed`, `owner` and `parent`, and decisions pointing at them with `canon:`. What was missing:
- no read listed documents by their front-matter;
- the modal mixed accepted and superseded decisions as raw `canon` rows;
- there was no `tags` key;
- nothing said when an accepted decision landed after its page last changed.

This is the first of three shelves; RM-0128 adds practices and RM-0129 the guide.

## Rules and edge cases

The canon holds them:
- `docs/canon/spec-library.md`: the read, decisions, tags, notes, the hub, the screen and the known limits;
- `spec-check.md`: `tag-invalid`;
- `node-revision.md`: `tags` is never counted;
- the store README: `INDEX_FORMAT` 18;
- `workspaces.md` and `presets.md`: `documents` in `SECTIONS`.

Language, decided by the owner: this repository is in English; a consumer's titles, tags and statuses are shown as written.

## Acceptance criteria

The scene is a git copy of `fixtures/spec-a`, with dated commits:
- DEC-0007 gains `canon: MEC-STAMINA`, and a rejected DEC-0031 points at MEC-SPRINT;
- stamina gains `tags: [stamina, tuning]`;
- a generated list sits under a walked `docs/generated`;
- two modules: `movement` (root DOM-MOVEMENT) and `records` (kinds question, requirement).

- [x] DL1 — 13 rows (15 with `--archive`), 22 keys each, facets equal to `spec map`'s. M: `tags` dropped; generated documents left out.
- [x] DL2 — the route equals `spec library --json`; the 400s; the 404 list; five key sets. M: a key only the route adds.
- [x] DL3 — stamina lists DEC-0023 accepted (`RULE-STAM-REGEN`), then DEC-0007 superseded; sprint lists DEC-0031 rejected; any other status is `other`. M: superseded read as accepted; Tier 3 sources dropped.
- [x] DL4 — behind is false at one shared commit, true after a decision-only commit (an uncommitted page edit keeps it true), false after a page commit. M: `>=` for `>`; `date:` compared instead of commits.
- [x] DL5 — with no `.git`: every last change and `newer` null, `behind` null or false, one note.
- [x] DL6 — `tags` leaves `norm_hash` and `rev` unchanged; no `unknown-key`; `key-extra` under a closed contract; a bare string gives `frontmatter-type`. M: tags counted in the hash; tags left out of `TYPED_KEYS`.
- [x] DL7 — 38 entries give 6 `tag-invalid` and 32 kept tags. M: the repeat rule dropped.
- [x] DL8 — modules follow the Data rule; an unresolvable root gets its note. M: the root's own document left out.
- [x] DL9 — a format-17 index is re-read at 18. M: the format kept at 17.
- [x] DL10 — the hub's second read has `index=trusted`. M: the route on `run`.
- [x] DL11 — a warm read is `git=3`; eviction holds across six commits. M: no cache; no eviction.
- [x] DL12 — on this repository, through the route: each resolving ADR appears once, under its canon file.
- [x] DL13 — [measure] at 10k nodes: CLI 370 ms, warm `run_ms` 71 ms, 322 ms after a commit. At 50k: warm 315 ms, RSS 459 MB.
- [x] DL14 — [soak] two runs of 300 s, 10 readers, a commit every 10 s: slopes 0.0 and 0.062 MiB/min (budget 0.5), 3 727 and 3 901 reads, line `ms` p95 1 051 and 1 020, worst 2 176 and 1 968, no hang, walked lines 2 in each run (cap 2); the probe phase answered 5 of 5 writes, wait at most 740 ms, 0 walked. M: no eviction.
- [x] DL15 — [measure] 10 readers, 8 MCP writers, the Inbox every 5 s: 807 library reads, worst 2 065 ms; 641 writes, all stored; the writers' wait at most 2 668 ms.
- [x] DL16 — two roots and two `HOME`s, the second indexed half-way: byte-equal output. M: rows in index order.
- [x] DL17 — `SECTIONS` 10, the presets and their fixtures updated. M: `IMPLICIT_SECTIONS` without `documents`.
- [x] DL18 — facets AND across and OR within, counts, defaults one press from All, the module choice from storage, decisions, paging, sort. M: OR across facets; a canon default with no canon row.
- [x] DL19 — the states (the lone Include archive checkbox, "Showing the last answer…"), served values as written, `en` and `ru` keys, a lazy chunk; at 320 px only the table scrolls (294 px client, 1 155 px scroll).
- [x] DL20 — guards green; MCP, plugin, queue schema 14 and backup format 7 untouched. M: a preset name in `library.rs`.

## Out of scope

- a tag editor;
- an MCP tool;
- other projects' documents (RM-0015);
- CLI filters;
- a Decisions part in the modal;
- reads outside the turn (the concurrent-trusted-reads ADR);
- a per-`HEAD` index of last changes;
- practices (RM-0128) and the guide (RM-0129);
- the write path's memory growth.

## Implementation

| Module | What it does |
|---|---|
| model `node.rs`; core `front_matter.rs`; import `layout/typed.rs` | `Fields.tags`; `TYPED_KEYS` 32 |
| core `library.rs`, `map.rs`, `check/`, `workspace_toml.rs` | rows, modules, decisions, tags; `CanonicalNames`; `tag-invalid` (39 codes); `SECTIONS` 10 |
| store `lib.rs` | `INDEX_FORMAT` 18 |
| CLI `library.rs`, `catalog.rs` | `spec library`; `git_read` and `GitRead::notes_of`, shared with the catalog |
| http `app.rs` | the route, on `run_read`, with `App.catalogs` |
| `scripts/presets/*.toml` | `documents` after `tree` |
| UI `documents/`, `tree/{UsedCell,NodePane}.tsx`, `app/`, `api/`, `i18n/`, `mocks/library.ts` | the Documents section; the Bindings tab made lazy |
| tests | core, CLI, http, store, eval (`library_scale.rs`, `library_soak.rs`), UI; the goldens' base IDs re-pinned after the preset fixtures changed; `auto-mini.json`'s date back to its literal (a flake that passed only on 2026-11-15) |

**Release.** Plugin 0.47.2, a version-only bump (`plugin.json`, `scripts/deploy/.env.example`, the test pins; `docs/canon/plugin-files.md` "Changing it"), so the hub's image carries `INDEX_FORMAT` 18; the next tag `v0.47.2`. DL20 holds: no plugin text, tool or schema changed. Published by the main session on the owner's word of 2026-11-20.

**Deviations, accepted:**
- The index is read as stored, never re-read per file. Otherwise a commit in a hub's checkout makes the next read walk; and since the library prints no file content, nothing it shows can be stale.
- Core takes `(&SpecGraph, &MapNaming, &WorkspaceConfig, LibraryRequest)`.
- A decision lands on its first holder that is a row.
- Git notes cover the rows and their decisions' files; parent warnings go to stderr, holders named `path (ID)`.
- The tree's Bindings tab is lazy; first load is 829 030 B against the 845 kB bar. UI suite: 241 files, 6 373 tests.

**Process.** Three iterations. DL14's walked cap was kept; its diagnosis is in the canon's "Daemon, hub". The full run: 6 426 tests: 6 412 passed; the 13 failures from the index not yet regenerated and two README lines cleared at ship (rerun green); 1, `autopilot rn4_until`, timing-sensitive under the parallel suite, passes alone, untouched by this slice.

**Open nits:**
- the soak's probe-phase wording;
- after a failed Retry, focus may land on the table;
- the "last answer" line has no live region;
- `behind: null` looks the same as `false` (the owner's question).
