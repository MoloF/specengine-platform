---
class: spec
status: shipped
shipped: 2026-11-24
scope: [crates/specengine-model, crates/specengine-core, crates/specengine-store, crates/specengine-cli, crates/specengine-mcp, crates/specengine-http, crates/specengine-eval, plugin, scripts, ui]
ref: RM-0138; the owner's request of 2026-11-21 and his decisions of 2026-11-22 (08 s2 Phase 7, "Owner's word, 2026-11-21/22"); the analyst's breakdown of 2026-11-22, slice 3 of 3, every working answer decided on his "start and go to the end"; plugin 0.52.0
adrs: [ADR-0089]
---

# reference-promotion: an approved reference enters git beside its document (RM-0138)

## Why

`work-references` (RM-0137, 0.51.0) put photos, sounds and models beside the work as queue data, kept on the hub alone. What a team approves must outlive the hub and belong to the spec: the owner asked that approved references become canon beside their entity, in git, with LFS (2026-11-21), and on 2026-11-22 decided that the image ships git-lfs and skips the smudge, the push is verified at the gate, and the link is a typed, uncounted `references:` key, each entry `<path> | <what matters>`. So a reference's file enters git only through a proposal a person confirms, the one writing door (ADR-0004), and every card on its document inherits it. Slice 3 of three, after `comments` (RM-0136) and `work-references` (RM-0137).

Canon: `docs/canon/work-references-promotion.md` (the rule, homes, the key, propose, apply, LFS, reads, the hub, limits) and `work-references-promotion-surfaces.md` (formats, documents, MCP, HTTP, UI, the upgrade). Decision: ADR-0089. The working answers (D1-D20) and the full formats are in git history.

## Acceptance criteria

Each names the mutation that turned it red (M). Fixture `fixtures/promote-mini/`: kinds `location` (LOC), `persona` (CHR; not `character`, which `http no_domain ac08` reads as a fixture kind in the daemon's "control character"), `mechanic`, `rule`; homes `location` `media` (LFS by `.gitattributes`), `persona` `art`; `[team]` owner `ann`, leads `lev` (`docs/spec/world/*.md`) and `mira` (`docs/spec/world/**`).

- [x] PR1 — Only a proposal: `spec propose reference`, `POST .../references/:id/promote` and `propose_change {kind: "reference"}` store one row, `ref_id` set, writing nothing under the root; only apply writes a reference's bytes there; no promote tool. M: the route writing the file at propose.
- [x] PR2 — Apply: one commit, one parent, `M` the document and `A` the file, trailers through `Reference:`; the file BLAKE3-equal, its blob ID `git hash-object`'s; the entry added, every other byte equal; `HEAD`'s blob of other bytes refused (`holds other bytes`), `HEAD`'s file dirty on disk `in place`, `M` alone; equal bytes on disk or staged added as they stand; a deleted reference or a gone blob writes nothing. M: the occupancy check skipped.
- [x] PR3 — LFS: a v1 pointer committed, never the bytes; `git lfs ls-files` and `fsck --objects` clean; `lfs.storage` honoured; a matching object untouched, another rewritten; no hook or `lfs.*` key added; git-lfs off `PATH` exit 2, nothing written; plain over 50 MiB, another filter, `text` refused. M: the raw blob committed.
- [x] PR4 — No home: exit 2 naming the fix, 503 on the route, no row; a bad `homes` refuses reference writes, never `list`, `get`, `delete`. M: a fallback to the document's own directory.
- [x] PR5 — Uncounted: a locked document's `norm_hash` and `rev` unchanged, no raise, `spec verify` `ok`, no `rev-not-raised`; bad entries one `reference-entry` each, 32 kept; a non-list `frontmatter-type`. M: `references` in the norm recipe.
- [x] PR6 — Inheritance: cards on the document and on its section print `context.spec-reference:`, the other `-`; MCP byte-equal; `bundle_hash` equals `spec bundle`'s over the same targets; `U` grows; the package's `spec_references` and "In the spec"; the list's `in_spec`; 20 of 25 and a note. M: a section target not resolved to its document.
- [x] PR7 — `[auto]`: a pack's and a hub owner's `reference` judged `proposal-kind`, no `Delegated-by:` commit. M: the kind in the covered set.
- [x] PR8 — Cover: a lead whose zones miss the file 403, nothing written; a covering lead and the owner 200. M: the cover on `target_path` alone.
- [x] PR9 — Genre: no fixture or preset kind and no `media`, `art`, `docs/`, `spec/` literal in the promotion code; the kind from `REFERENCE_KIND`. M: a default `dir` in core.
- [ ] PR10 — Owner's gate: on the live hub, upgraded, `--push` on after gate 11b-3, a promotion confirmed into an LFS home of pilot A; the sync uploads, then pushes; a fresh clone's `git lfs pull` gives a file whose `b3sum` equals the original's; the deploy key carried both. It blocks nothing.
- [x] PR11 — Schema 17 and dump 10: one transaction, `ref_id`, `proposals_ref`, 52 columns; a byte-identical round trip; format 9 imports with `ref_id` null; a misplaced `ref_id` is a corrupt row. M: `ref_id` left out of the dump.
- [x] PR12 — Index 22: rebuilt at the first write; `promoted` follows edits and deletes. M: `update` skipping `promoted`.
- [x] PR13 — Memory: a 256 MiB LFS and a 50 MiB plain confirm raise RSS at most 16 MiB; the hashes equal. M: the blob read whole.
- [x] PR14 — Rebase and form: an edit kept, `rebases`; a flow key refused; `[]` a block; CRLF kept; an entry already listed refused (the entry's path); a live twin refused; a failed commit removes only what the run wrote, a person's staging kept; completion by the trailer. M: the propose-time text written over the edit.
- [x] PR15 — Names: a link refused; an extension unlike the media refused, `--name` fixing it; `|`, a leading dot, a trailing dot, a `.git` part refused; a section noted; a bare ID resolves to the non-feature document; 513 characters refused; `[ids]` claiming `RF` exit 2. M: the extension rule dropped.
- [x] PR16 — MCP and HTTP: CLI parity; -32602 naming each misplaced key; tool counts unchanged; texts within 2 048 B; the route's 400, 403, 404, 409, 503 and one `proposal.created` with its `as`; `ref_id` in the documents. M: `reference_id` accepted on an `update`.
- [x] PR17 — UI (`ui/src/references/promote.test.tsx`): Promote to canon on a stored file alone, Document preselected, the Inbox link, the dialog kept on 409 and 503, "In the spec" rows, the raw kind; first load 817 011 B. M: the button on a link.
- [x] PR18 — Image and sync: git-lfs, no `git lfs install`, `GIT_LFS_SKIP_SMUDGE=1`, the smoke; a merge leaves pointers; the upload before the push; a failed upload `retrying`, no push; only git-lfs's exact template hooks removed. M: the push before the upload.
- [x] PR19 — Soak RP, two 300 s runs green (`soak-runs.md` "Budgets"). M: a planted 64 KiB per promotion red at 0.751 MiB/min.
- [x] PR20 — Plugin 0.52.0: `PINS`, `SCHEMA_PINS`, `propose-spec-change` within budget. M: the schemas changed at 0.51.0.

## Implementation

| Module | What it does |
|---|---|
| core `promote.rs`, `digest.rs`, `reference.rs`, `front_matter.rs`, `check/` | homes, entries, `insert_entry`, the list and context lines; streaming SHA-256 and SHA-1; the typed key and `reference-entry` |
| model `node.rs`, `task.rs` | `Fields.references`; `PackageSpecReferences` |
| store `schema.rs`, `promoted.rs`, `write.rs`, `queue.rs`, `queue/state.rs`, `dump.rs`, `sync.rs`, `worktree.rs` | index 22 and `promoted_of`; `STEP_17`, `create_promotion`, dump 10; the sync's LFS upload and hook clean-up; git reads |
| cli `promote.rs`, `apply.rs`, `confirm.rs`, `preflight.rs`, `card.rs`, `package.rs`, `reference.rs`, `review.rs`, `sync.rs` | propose and apply of the kind, the confirm's cover, the context's, package's and list's spec references, the review's note |
| mcp `intake.rs`, `mirror.rs`, `server.rs`, `hub.rs`; http `app.rs`, `hub.rs`, `answer.rs` | `propose_change`'s fifth kind; the promote route |
| eval `write_soak.rs`; ui `src/references/PromoteDialog.tsx`, `ReferencesPart.tsx`, `inbox/`, `api/` | `Variant::RP`; Promote to canon, In the spec, the kind |
| plugin, `scripts/deploy/`, `scripts/presets/gamedev.toml`, `.github/workflows/image.yml` | 0.52.0; git-lfs, the clone tag `v0.52.0`; the homes example; the smoke |

**Deviations**, each now canon: LFS objects and pointers are written by apply itself, never `git lfs clean`, which writes hooks and config into the repository (3.8); "in place" judges `HEAD`'s blob alone; the sync removes only git-lfs's exact template hooks; `bundle_hash` moves with the document's bytes (the main session's decision); RP has no page bar, a confirm holding the turn by design; `ref_id` a parameter of `create_promotion`; a page promotion is authored by its person; the fixture's `persona`.

## Release

Plugin 0.52.0 (MINOR), tag `v0.52.0`; queue schema 17 and `STATE_FORMAT` 10, so the hub dumps first; `INDEX_FORMAT` 22; git-lfs in the image (`work-references-promotion-surfaces.md` "Upgrade"). The full run: 6 827 passed, 126 skipped, clippy and fmt clean; UI lint clean, 6 731 tests; reviews accepted after three iterations. RP twice, live heap 0.002 and 0.002 MiB/min, 24 of 24 confirms verified each. Open: PR10, the owner's.
